//! The facts pass: reads what the walk only listed, a batch at a time.
//!
//! Each file costs one header read through the decoders (no pixels), the thumbnail key's
//! samples (48 KiB at most), its XMP sidecar when it has one, and a lookup per size class in
//! the thumbnail cache. A batch is read on a few threads and written in one transaction.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use rusqlite::{Connection, params};
use specular_thumbs::{CLASSES, DiskCache, ThumbKey};

use crate::{Config, Fold, Mark, Probed, Shared, store};

/// A file waiting for its facts.
pub(crate) struct Todo {
    id: i64,
    path: PathBuf,
    dir: PathBuf,
    xmp: Option<String>,
    xmp_mtime: Option<i64>,
    /// Every sidecar's name and time stamp (`files.sides`).
    sides: Option<String>,
    /// Other apps' sidecars have the last word in its folder (the user's choice).
    prefer_foreign: bool,
    /// It has its full-text row (`files.fts`); a new file gets it with its facts.
    fts: bool,
}

/// Another app's sidecar of a file, read.
struct Foreign {
    name: String,
    /// The darktable duplicate it describes; 0 for the photo itself.
    duplicate: u32,
    mtime: i64,
    read: specular_metadata::sidecars::Read,
}

/// What was read of it.
enum Read {
    /// Not an image the decoders read, or not readable now.
    Unreadable,
    Facts {
        key: ThumbKey,
        probed: Box<Probed>,
        classes: u32,
        /// The sidecar's marks and keywords (None: it names none, and the file's own count),
        /// and its time stamp, when it is newer than the one read before.
        side: Option<(Mark, Option<Vec<String>>, i64)>,
        /// Keywords of a sidecar read before (kept when it did not change).
        side_kept: bool,
        /// Other apps' sidecars, in their order of weight.
        foreign: Vec<Foreign>,
        /// The matched XMP sidecar is another app's: its time stamp (it is in `foreign`).
        side_foreign: Option<i64>,
        /// The populated place nearest the GPS position (offline reverse geocoding).
        place: Option<Box<specular_metadata::Place>>,
        /// The IPTC fields (the sidecar's, else the file's), as search reads them.
        iptc: String,
    },
}

/// Files waiting for their facts in online locations: unread ones, and those read before a
/// fact was added to the catalogue (the colour space, the position and place), which are
/// read again after them.
pub fn pending(c: &Connection) -> rusqlite::Result<u64> {
    c.query_row(
        "SELECT COUNT(*) FROM files f JOIN dirs d ON d.id = f.dir_id
         JOIN folders o ON o.id = d.folder_id
         WHERE (f.state = 0 OR (f.state = 1 AND (f.space IS NULL OR f.geo IS NULL
             OR f.exif IS NULL OR f.iptc IS NULL OR (f.astro IS NULL AND f.format IN ('FITS', 'XISF')))))
         AND o.offline = 0",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n as u64)
}

/// The next batch: unread files first, then files read before the colour space or the
/// position was kept (their other facts stay in place, so they never read as unknown
/// meanwhile).
fn todo(c: &Connection, limit: usize) -> rusqlite::Result<Vec<Todo>> {
    let (out, steps) = todo_counted(c, limit)?;
    crate::work_counts::add_todo_steps(steps);
    Ok(out)
}

/// [`todo`], and the SQLite steps it took.
///
/// Each query is pinned to its partial index, files first: without statistics SQLite took
/// `files_state` and walked every file read, for each query of every batch (0.6 s per batch
/// at a million files with nothing left to read).
pub(crate) fn todo_counted(c: &Connection, limit: usize) -> rusqlite::Result<(Vec<Todo>, u64)> {
    let (mut out, mut steps) = (Vec::new(), 0);
    for sql in [
        "SELECT f.id, d.path, f.name, f.xmp, f.xmp_mtime, f.fts, f.sides FROM files f
         INDEXED BY files_state CROSS JOIN dirs d ON d.id = f.dir_id
         CROSS JOIN folders o ON o.id = d.folder_id
         WHERE f.state = 0 AND o.offline = 0 LIMIT ?1",
        "SELECT f.id, d.path, f.name, f.xmp, f.xmp_mtime, f.fts, f.sides FROM files f
         INDEXED BY files_space_unread CROSS JOIN dirs d ON d.id = f.dir_id
         CROSS JOIN folders o ON o.id = d.folder_id
         WHERE f.state = 1 AND f.space IS NULL AND o.offline = 0 LIMIT ?1",
        "SELECT f.id, d.path, f.name, f.xmp, f.xmp_mtime, f.fts, f.sides FROM files f
         INDEXED BY files_geo_unread CROSS JOIN dirs d ON d.id = f.dir_id
         CROSS JOIN folders o ON o.id = d.folder_id
         WHERE f.state = 1 AND f.geo IS NULL AND o.offline = 0 LIMIT ?1",
        "SELECT f.id, d.path, f.name, f.xmp, f.xmp_mtime, f.fts, f.sides FROM files f
         INDEXED BY files_astro_unread CROSS JOIN dirs d ON d.id = f.dir_id
         CROSS JOIN folders o ON o.id = d.folder_id
         WHERE f.state = 1 AND f.astro IS NULL AND f.format IN ('FITS', 'XISF')
         AND o.offline = 0 LIMIT ?1",
        "SELECT f.id, d.path, f.name, f.xmp, f.xmp_mtime, f.fts, f.sides FROM files f
         INDEXED BY files_exif_unread CROSS JOIN dirs d ON d.id = f.dir_id
         CROSS JOIN folders o ON o.id = d.folder_id
         WHERE f.state = 1 AND f.exif IS NULL AND o.offline = 0 LIMIT ?1",
        "SELECT f.id, d.path, f.name, f.xmp, f.xmp_mtime, f.fts, f.sides FROM files f
         INDEXED BY files_iptc_unread CROSS JOIN dirs d ON d.id = f.dir_id
         CROSS JOIN folders o ON o.id = d.folder_id
         WHERE f.state = 1 AND f.iptc IS NULL AND o.offline = 0 LIMIT ?1",
    ] {
        let mut st = c.prepare_cached(sql)?;
        out = st
            .query_map([limit as i64], |r| {
                let dir = PathBuf::from(r.get::<_, String>(1)?);
                Ok(Todo {
                    id: r.get(0)?,
                    path: dir.join(r.get::<_, String>(2)?),
                    dir,
                    xmp: r.get(3)?,
                    xmp_mtime: r.get(4)?,
                    fts: r.get(5)?,
                    sides: r.get(6)?,
                    prefer_foreign: false,
                })
            })?
            .collect::<Result<_, _>>()?;
        steps += st.reset_status(rusqlite::StatementStatus::VmStep).max(0) as u64;
        if !out.is_empty() {
            break;
        }
    }
    Ok((out, steps))
}

/// Reads and writes one batch. Returns how many files were done (None: nothing waits; 0:
/// the user is browsing).
pub fn batch(c: &Connection, cfg: &Config, shared: &Shared) -> rusqlite::Result<Option<usize>> {
    let mut todo = todo(c, cfg.batch)?;
    if todo.is_empty() {
        return Ok(None);
    }
    let prefer = shared.prefer_foreign.lock().clone();
    if !prefer.is_empty() {
        for t in &mut todo {
            t.prefer_foreign = prefer.iter().any(|f| t.dir.starts_with(f));
        }
    }
    let next = AtomicUsize::new(0);
    let threads = cfg.threads.clamp(1, todo.len());
    let mut results: Vec<(usize, Read)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|wi| {
                specular_thread::spawn_scoped(s, format!("lib-facts-{wi}"), || {
                    if let Some(f) = cfg.on_thread_start {
                        f();
                    }
                    let mut out = Vec::new();
                    loop {
                        // Back off at once when the user starts browsing.
                        if shared.held() || shared.halting() || shared.removal_waits() {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(t) = todo.get(i) else { break };
                        // A panic on this file (a decoder bug) marks it unreadable, so it
                        // is not tried again on every pass and the rest of the batch goes on.
                        let what = format_args!("library facts of {}", t.path.display());
                        let r = specular_core::isolate::run(what, || read(t, cfg));
                        out.push((i, r.unwrap_or(Read::Unreadable)));
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
    let r = write(c, &todo, &results, cfg.fold);
    if r.is_err() {
        let _ = c.execute_batch("ROLLBACK");
        return r.map(|_| None);
    }
    c.execute_batch("COMMIT")?;
    Ok(Some(results.len()))
}

fn read(t: &Todo, cfg: &Config) -> Read {
    let Ok(key) = ThumbKey::of_file(&t.path) else {
        return Read::Unreadable;
    };
    let Some(mut probed) = cfg.prober.probe(&t.path) else {
        return Read::Unreadable;
    };
    // An astronomy subframe's star metrics: from the thumbnail cache when the gallery
    // measured it, else measured here (and kept there for the gallery and cull mode).
    if let Some(a) = probed.meta.astro.as_mut()
        && a.metrics.is_none()
    {
        let cache = cfg.cache.as_deref();
        a.metrics = cache
            .and_then(|c| c.get_blob(&key, specular_thumbs::ASTRO_BLOB))
            .and_then(|b| specular_decode::astro::FrameMetrics::from_bytes(&b));
        if a.metrics.is_none() {
            match cfg.prober.measure(&t.path) {
                Some(m) => {
                    if let Some(c) = cache {
                        let _ = c.put_blob(&key, specular_thumbs::ASTRO_BLOB, &m.to_bytes());
                    }
                    a.metrics = Some(m);
                }
                // Not measurable: no stars, rather than metrics still to come.
                None => a.metrics = Some(Default::default()),
            }
        }
    }
    let classes = cfg
        .cache
        .as_deref()
        .map_or(0, |c| cached(c, &key, cfg.cache_flags));
    let mut side = None;
    let mut side_kept = false;
    let mut side_foreign = None;
    let mut foreign = Vec::new();
    if let Some(x) = &t.xmp {
        let p = t.dir.join(x);
        let mtime = std::fs::metadata(&p)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|m| crate::scan::mtime_ns(Some(m)));
        if let Some(m) = mtime {
            // Read each time: whose it is decides how it counts (another app may have taken
            // over a sidecar Specular wrote).
            match read_regular(&p) {
                Ok(bytes) => {
                    let r = specular_metadata::sidecars::read_xmp(&bytes);
                    if r.app.is_some() {
                        side_foreign = Some(m);
                        foreign.push(Foreign {
                            name: x.clone(),
                            duplicate: 0,
                            mtime: m,
                            read: r,
                        });
                    } else if Some(m) != t.xmp_mtime {
                        let (mark, kw) = store::sidecar(&bytes);
                        side = Some((mark, kw, m));
                    } else {
                        side_kept = true;
                    }
                }
                Err(_) => side_kept = Some(m) == t.xmp_mtime,
            }
        }
    }
    if let Some(s) = &t.sides {
        let own_name = t.path.file_name().and_then(|n| n.to_str());
        for (name, mtime) in crate::scan::sides_names(s) {
            if Some(name) == t.xmp.as_deref() {
                continue;
            }
            let Some(kind) = specular_metadata::sidecars::Kind::of_name(name) else {
                continue;
            };
            let duplicate = specular_metadata::sidecars::duplicate_of(name)
                .filter(|(photo, _)| Some(photo.as_str()) == own_name)
                .map_or(0, |(_, n)| n);
            let Ok(bytes) = read_regular(&t.dir.join(name)) else {
                continue;
            };
            let read = specular_metadata::sidecars::read(kind, &bytes);
            if read.app.is_some() {
                foreign.push(Foreign {
                    name: name.to_owned(),
                    duplicate,
                    mtime,
                    read,
                });
            }
        }
    }
    // Reverse geocoding here, on the indexer's threads: the first lookup builds the index
    // (a few hundred milliseconds), later ones are microseconds.
    let iptc = iptc_lines(&t.path, t.xmp.as_ref().map(|x| t.dir.join(x)).as_deref());
    let place = probed
        .meta
        .position
        .and_then(|(lat, lon)| place_of(lat, lon))
        .map(Box::new);
    Read::Facts {
        key,
        probed: Box::new(probed),
        classes,
        side,
        side_kept,
        foreign,
        side_foreign,
        place,
        iptc,
    }
}

/// The IPTC fields of `path` as search reads them: its sidecar's, else its own XMP's, else
/// its IIM's (`specular_metadata::edit`).
pub(crate) fn iptc_lines(path: &std::path::Path, sidecar: Option<&std::path::Path>) -> String {
    use specular_metadata::edit;
    let own = specular_core::open_regular(path)
        .ok()
        .and_then(|mut f| edit::read_embedded(&mut f).ok())
        .map(|e| e.record())
        .unwrap_or_default();
    let side = sidecar
        .and_then(|p| read_regular(p).ok())
        .map(|b| edit::from_xmp_text(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    side.over(&own).to_lines()
}

/// The whole of a regular file (a sidecar found by name: a FIFO named `x.xmp` must not block
/// the pass).
fn read_regular(p: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let mut v = Vec::new();
    std::io::Read::read_to_end(&mut specular_core::open_regular(p)?, &mut v)?;
    Ok(v)
}

/// Farther than this from every populated place (km), a position has no place: a photo out at
/// sea is not filed under the nearest harbour town.
pub const PLACE_MAX_KM: f64 = 150.0;

/// The place a position is filed under, if any.
pub fn place_of(lat: f32, lon: f32) -> Option<specular_metadata::Place> {
    if !(lat.is_finite() && lon.is_finite()) || lat.abs() > 90.0 || lon.abs() > 180.0 {
        return None;
    }
    specular_metadata::geocode(f64::from(lat), f64::from(lon))
        .filter(|p| p.distance_km <= PLACE_MAX_KM && !p.city.is_empty())
}

/// Bit n set when the cache holds the thumbnail of size class `CLASSES[n]`.
pub fn cached(cache: &DiskCache, key: &ThumbKey, flags: u32) -> u32 {
    CLASSES
        .iter()
        .enumerate()
        .filter(|(_, c)| cache.contains(key, **c, flags))
        .fold(0, |m, (i, _)| m | 1 << i)
}

/// `files.props`: the schema's bits, then 128 Motion Photo, 256 Live Photo, 512 movie (or
/// sound file), 1024 sound without a picture.
fn props(m: &specular_thumbs::Meta) -> i64 {
    let mut p = 0;
    for (b, on) in [
        (1, m.animated),
        (2, m.hdr),
        (4, m.has_alpha),
        (8, m.raw),
        (16, m.has_profile),
        (32, m.gps.is_some()),
        (64, m.gps == Some(true)),
        (128, m.motion),
        (256, m.live_photo),
        (512, m.video),
        (1024, m.sound),
    ] {
        if on {
            p |= b;
        }
    }
    p
}

fn write(
    c: &Connection,
    todo: &[Todo],
    results: &[(usize, Read)],
    fold: Fold,
) -> rusqlite::Result<()> {
    for (i, r) in results {
        let t = &todo[*i];
        match r {
            Read::Unreadable => {
                c.prepare_cached("UPDATE files SET state = 2 WHERE id = ?1")?
                    .execute([t.id])?;
                // Its name is still searched.
                if !t.fts {
                    store::fts_first(c, t.id, fold)?;
                }
            }
            Read::Facts {
                key,
                probed,
                classes,
                side,
                side_kept,
                foreign,
                side_foreign,
                place,
                iptc,
            } => {
                let m = &probed.meta;
                c.prepare_cached(
                    "UPDATE files SET state = 1, size = ?2, mtime_ns = ?3, format = ?4,
                     width = ?5, height = ?6, taken = ?7, camera = ?8, lens = ?9, iso = ?10,
                     f_number = ?11, focal = ?12, exposure = ?13, pages = ?14, bits = ?15,
                     props = ?16, space = ?17, astro = ?18, exif = ?19, exif_tags = ?20,
                     iptc = ?21,
                     xmp_mtime = CASE WHEN xmp IS NULL THEN NULL ELSE xmp_mtime END
                     WHERE id = ?1",
                )?
                .execute(params![
                    t.id,
                    key.stamp.len as i64,
                    key.stamp.mtime_ns.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                    m.format,
                    m.width,
                    m.height,
                    m.date_taken,
                    m.camera,
                    m.lens,
                    m.iso,
                    m.f_number,
                    m.focal_length,
                    m.exposure,
                    m.pages,
                    m.bit_depth,
                    props(m),
                    // Empty when the prober did not look (never NULL: it would be read again).
                    m.space.as_deref().unwrap_or(""),
                    // Empty: not a subframe (never NULL: it would be read again).
                    m.astro.as_ref().map(|a| a.to_bytes()).unwrap_or_default(),
                    // Never NULL either: a file without Exif has empty fields and tags.
                    m.exif
                        .as_ref()
                        .map(|e| e.to_bytes())
                        .unwrap_or_else(|| { specular_thumbs::ExifMeta::default().to_bytes() }),
                    m.exif
                        .as_ref()
                        .and_then(|e| e.tags.as_deref())
                        .unwrap_or(""),
                    // Never NULL: a file without IPTC fields has none.
                    iptc,
                ])?;
                if let Some((mark, _, mtime)) = side {
                    // Specular's own sidecar: its marks are the user's.
                    c.prepare_cached(
                        "UPDATE files SET rating = ?2, label = ?3, flag = ?4, xmp_mtime = ?5,
                         foreign_bits = foreign_bits & ~7 WHERE id = ?1",
                    )?
                    .execute(params![
                        t.id,
                        mark.rating,
                        mark.label,
                        mark.flag,
                        mtime
                    ])?;
                }
                if let Some(m) = side_foreign {
                    c.prepare_cached("UPDATE files SET xmp_mtime = ?2 WHERE id = ?1")?
                        .execute(params![t.id, m])?;
                }
                let merged = write_foreign(c, t.id, foreign)?;
                foreign_marks(c, t.id, &merged, t.prefer_foreign)?;
                c.prepare_cached(
                    "INSERT INTO thumbs (file_id, content, classes) VALUES (?1, ?2, ?3)
                     ON CONFLICT(file_id) DO UPDATE SET content = excluded.content, classes = excluded.classes",
                )?
                .execute(params![t.id, key.content as i64, classes])?;
                // Keywords from the file itself, and from the sidecar when it was read again.
                c.prepare_cached("DELETE FROM file_tags WHERE file_id = ?1 AND source = 0")?
                    .execute([t.id])?;
                if !side_kept {
                    c.prepare_cached("DELETE FROM file_tags WHERE file_id = ?1 AND source = 1")?
                        .execute([t.id])?;
                }
                // A sidecar that names keywords has the last word on them (also when it
                // names none: Specular writes an empty list when all were taken off).
                let side_rules =
                    match side {
                        Some((_, kw, _)) => kw.is_some(),
                        None => *side_kept
                            && c.prepare_cached(
                                "SELECT 1 FROM file_tags WHERE file_id = ?1 AND source = 1 LIMIT 1",
                            )?
                            .exists([t.id])?,
                    };
                if !side_rules {
                    for k in &probed.keywords {
                        tag(c, t.id, k, 0)?;
                    }
                }
                if let Some((_, Some(kw), _)) = side {
                    for k in kw {
                        tag(c, t.id, k, 1)?;
                    }
                }
                foreign_keywords(c, t.id, &merged, t.prefer_foreign, side_rules)?;
                write_place(c, t.id, m.position, place.as_deref())?;
                match probed.text.as_deref().filter(|s| !s.is_empty()) {
                    Some(text) => c
                        .prepare_cached(
                            "INSERT INTO file_text (file_id, text) VALUES (?1, ?2)
                             ON CONFLICT(file_id) DO UPDATE SET text = excluded.text",
                        )?
                        .execute(params![t.id, text])?,
                    None => c
                        .prepare_cached("DELETE FROM file_text WHERE file_id = ?1")?
                        .execute([t.id])?,
                };
                if t.fts {
                    store::fts_refresh(c, t.id, fold)?;
                } else {
                    store::fts_first(c, t.id, fold)?;
                }
            }
        }
    }
    Ok(())
}

/// `file_tags.source` of keywords from another app's sidecar.
pub(crate) const SOURCE_FOREIGN: i64 = 3;

/// Bits of `files.foreign_bits`: the value came from another app's sidecar.
const BIT_RATING: i64 = 1;
const BIT_LABEL: i64 = 2;
const BIT_FLAG: i64 = 4;

/// Stores what other apps' sidecars of `file` said (replacing what was stored), and returns
/// the photo's own values merged, the weightiest sidecar first (duplicates left out).
fn write_foreign(
    c: &Connection,
    file: i64,
    foreign: &[Foreign],
) -> rusqlite::Result<specular_metadata::sidecars::Values> {
    c.prepare_cached("DELETE FROM foreign_sides WHERE file_id = ?1")?
        .execute([file])?;
    let mut merged = specular_metadata::sidecars::Values::default();
    for f in foreign {
        let v = &f.read.values;
        c.prepare_cached(
            "INSERT OR REPLACE INTO foreign_sides (file_id, name, app, duplicate, mtime_ns,
             rating, label, flag, keywords, title, caption, version_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?
        .execute(params![
            file,
            f.name,
            f.read.app.map_or("other", |a| a.id()),
            f.duplicate,
            f.mtime,
            v.rating,
            v.label,
            v.flag,
            v.keywords.as_ref().map(|k| k.join("\n")),
            v.title,
            v.caption,
            f.read.version_name,
        ])?;
        if f.duplicate == 0 {
            merged.fill_from(v);
        }
    }
    Ok(merged)
}

/// Takes the marks other apps' sidecars give where the user has set none (and where they
/// came from a sidecar before, so they follow it); with `prefer`, wherever a sidecar says.
fn foreign_marks(
    c: &Connection,
    file: i64,
    v: &specular_metadata::sidecars::Values,
    prefer: bool,
) -> rusqlite::Result<()> {
    let (rating, label, flag, bits): (i64, i64, i64, i64) = c
        .prepare_cached("SELECT rating, label, flag, foreign_bits FROM files WHERE id = ?1")?
        .query_row([file], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let pick = |cur: i64, bit: i64, val: Option<i64>| -> (i64, i64) {
        let from_sidecar = bits & bit != 0;
        match val {
            Some(v) if prefer || from_sidecar || cur == 0 => (v, bit),
            // The sidecar no longer says: what it said goes.
            None if from_sidecar => (0, 0),
            _ => (cur, bits & bit),
        }
    };
    let (r, rb) = pick(rating, BIT_RATING, v.rating.map(i64::from));
    let (l, lb) = pick(label, BIT_LABEL, v.label.map(i64::from));
    let (f, fb) = pick(flag, BIT_FLAG, v.flag.map(i64::from));
    let nb = (bits & !(BIT_RATING | BIT_LABEL | BIT_FLAG)) | rb | lb | fb;
    if (r, l, f, nb) != (rating, label, flag, bits) {
        c.prepare_cached(
            "UPDATE files SET rating = ?2, label = ?3, flag = ?4, foreign_bits = ?5 WHERE id = ?1",
        )?
        .execute(params![file, r, l, f, nb])?;
    }
    Ok(())
}

/// Keywords from other apps' sidecars: they stand in for the file's embedded ones when the
/// user's sidecar names none; with `prefer`, they stand in for the user's too.
fn foreign_keywords(
    c: &Connection,
    file: i64,
    v: &specular_metadata::sidecars::Values,
    prefer: bool,
    own: bool,
) -> rusqlite::Result<()> {
    c.prepare_cached("DELETE FROM file_tags WHERE file_id = ?1 AND source = ?2")?
        .execute(params![file, SOURCE_FOREIGN])?;
    let Some(kw) = v.keywords.as_ref().filter(|k| !k.is_empty()) else {
        return Ok(());
    };
    if own && !prefer {
        return Ok(());
    }
    c.prepare_cached("DELETE FROM file_tags WHERE file_id = ?1 AND source IN (0, 1)")?
        .execute([file])?;
    for k in kw {
        tag(c, file, k, SOURCE_FOREIGN)?;
    }
    Ok(())
}

/// The position and place of one file (`geo` 1 with a position, 0 without).
fn write_place(
    c: &Connection,
    file: i64,
    position: Option<(f32, f32)>,
    place: Option<&specular_metadata::Place>,
) -> rusqlite::Result<()> {
    let place_id: Option<i64> = match place {
        Some(p) => {
            c.prepare_cached(
                "INSERT INTO places (city, admin1, admin2, cc) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(city, admin1, admin2, cc) DO NOTHING",
            )?
            .execute(params![p.city, p.admin1, p.admin2, p.country_code])?;
            Some(
                c.prepare_cached(
                    "SELECT id FROM places WHERE city = ?1 AND admin1 = ?2 AND admin2 = ?3 AND cc = ?4",
                )?
                .query_row(params![p.city, p.admin1, p.admin2, p.country_code], |r| {
                    r.get(0)
                })?,
            )
        }
        None => None,
    };
    let pos = position.filter(|(a, o)| a.is_finite() && o.is_finite());
    c.prepare_cached("UPDATE files SET lat = ?2, lon = ?3, geo = ?4, place_id = ?5 WHERE id = ?1")?
        .execute(params![
            file,
            pos.map(|p| f64::from(p.0)),
            pos.map(|p| f64::from(p.1)),
            i64::from(pos.is_some()),
            place_id,
        ])?;
    Ok(())
}

/// Tags `file` with keyword `name` (a path: its levels are tags too, linked by `parent`,
/// but only the keyword itself is the file's).
pub(crate) fn tag(c: &Connection, file: i64, name: &str, source: i64) -> rusqlite::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(());
    }
    let mut parent: Option<i64> = None;
    let mut path = String::new();
    for level in specular_metadata::keywords::levels(name) {
        if !path.is_empty() {
            path.push(specular_metadata::keywords::SEP);
        }
        path.push_str(level.trim());
        c.prepare_cached(
            "INSERT INTO tags (name, parent) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE
             SET parent = excluded.parent WHERE parent IS NULL AND excluded.parent IS NOT NULL",
        )?
        .execute(params![path, parent])?;
        parent = Some(
            c.prepare_cached("SELECT id FROM tags WHERE name = ?1")?
                .query_row([&path], |r| r.get(0))?,
        );
    }
    c.prepare_cached(
        "INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?1, ?2, ?3)",
    )?
    .execute(params![file, parent, source])?;
    Ok(())
}

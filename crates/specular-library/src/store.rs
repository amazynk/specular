//! Small catalogue helpers shared by the walk, the facts pass and the indexer.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{Fold, Location, Mark};

/// The files waiting for their facts, by location (from the index of unread files: cheap
/// while few wait).
pub fn pending_by_location(
    c: &Connection,
) -> rusqlite::Result<std::collections::HashMap<i64, u64>> {
    let mut st = c.prepare_cached(
        "SELECT d.folder_id, COUNT(*) FROM files f JOIN dirs d ON d.id = f.dir_id
         WHERE f.state = 0 GROUP BY d.folder_id",
    )?;
    let rows = st.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as u64))
    })?;
    rows.collect()
}

/// The locations with their counts.
pub fn locations(c: &Connection) -> rusqlite::Result<Vec<Location>> {
    let mut counts = std::collections::HashMap::<i64, (u64, u64)>::new();
    {
        let mut st = c.prepare_cached(
            "SELECT d.folder_id, COUNT(*), SUM(f.state = 0) FROM files f
             JOIN dirs d ON d.id = f.dir_id GROUP BY d.folder_id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)? as u64,
                r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
            ))
        })?;
        for row in rows {
            let (id, n, p) = row?;
            counts.insert(id, (n, p));
        }
    }
    let mut st =
        c.prepare_cached("SELECT id, path, offline, last_scan FROM folders ORDER BY path")?;
    let rows = st.query_map([], |r| {
        let id: i64 = r.get(0)?;
        let (files, pending) = counts.get(&id).copied().unwrap_or_default();
        Ok(Location {
            id,
            path: PathBuf::from(r.get::<_, String>(1)?),
            offline: r.get::<_, i64>(2)? != 0,
            files,
            pending,
            last_scan: r.get(3)?,
        })
    })?;
    rows.collect()
}

/// A path as the catalogue stores it (None for names that are not UTF-8, which are skipped).
pub fn text(p: &Path) -> Option<&str> {
    p.to_str()
}

/// The canonical extension of `name` ("" when none).
pub fn ext_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, e)) if !stem.is_empty() => specular_query::canon_ext(e).into_owned(),
        _ => String::new(),
    }
}

/// Rewrites the full-text row of file `id` from what the catalogue holds of it. A file
/// without one yet (`fts` 0: its facts are not read) gets it from the facts pass instead.
pub fn fts_refresh(c: &Connection, id: i64, fold: Fold) -> rusqlite::Result<()> {
    let has: Option<bool> = c
        .prepare_cached("SELECT fts FROM files WHERE id = ?1")?
        .query_row([id], |r| r.get(0))
        .optional()?;
    if has != Some(true) {
        return Ok(());
    }
    crate::work_counts::fts_rewrite();
    c.prepare_cached("DELETE FROM fts_index WHERE rowid = ?1")?
        .execute([id])?;
    fts_put(c, id, fold)
}

/// Writes the first full-text row of file `id` (`fts` 0: it has none) and notes it.
pub fn fts_first(c: &Connection, id: i64, fold: Fold) -> rusqlite::Result<()> {
    fts_put(c, id, fold)?;
    c.prepare_cached("UPDATE files SET fts = 1 WHERE id = ?1")?
        .execute([id])?;
    Ok(())
}

/// Writes the full-text row of file `id` (it has none).
fn fts_put(c: &Connection, id: i64, fold: Fold) -> rusqlite::Result<()> {
    let row = c
        .prepare_cached("SELECT name, camera, lens FROM files WHERE id = ?1")?
        .query_row([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .optional()?;
    let Some((name, camera, lens)) = row else {
        return Ok(());
    };
    let mut tags = String::new();
    {
        let mut st = c.prepare_cached(
            "SELECT t.name FROM file_tags ft JOIN tags t ON t.id = ft.tag_id WHERE ft.file_id = ?1",
        )?;
        for t in st.query_map([id], |r| r.get::<_, String>(0))? {
            if !tags.is_empty() {
                tags.push('\n');
            }
            tags.push_str(&t?);
        }
        // Titles and captions from other apps' sidecars are searched with the keywords.
        let mut st = c.prepare_cached(
            "SELECT title, caption FROM foreign_sides WHERE file_id = ?1 AND duplicate = 0",
        )?;
        let rows = st.query_map([id], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        })?;
        for r in rows {
            let (t, d) = r?;
            for s in [t, d].into_iter().flatten() {
                if !tags.is_empty() {
                    tags.push('\n');
                }
                tags.push_str(&s);
            }
        }
    }
    c.prepare_cached(
        "INSERT INTO fts_index (rowid, name, camera, lens, tags) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        id,
        fold(&name),
        fold(camera.as_deref().unwrap_or("")),
        fold(lens.as_deref().unwrap_or("")),
        fold(&tags),
    ])?;
    Ok(())
}

/// How far a rebuild of the full-text index (another fold) has got: the files, then the
/// folders, each in id order. Rows up to the id are in the new fold; rows after it may still
/// be in the old one (or already in the new one, where a walk rewrote them meanwhile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FtsRebuild {
    Files(i64),
    Dirs(i64),
}

impl FtsRebuild {
    pub const START: FtsRebuild = FtsRebuild::Files(0);
}

/// Rebuilds the full-text rows of up to `rows` more files or folders after `at`, inside the
/// caller's transaction (rows of ids that are gone go too). Returns where to go on, or None
/// when the whole index is in the new fold.
pub fn fts_rebuild_some(
    c: &Connection,
    at: FtsRebuild,
    rows: usize,
    fold: Fold,
) -> rusqlite::Result<Option<FtsRebuild>> {
    let rows = rows.max(1);
    let (after, files) = match at {
        FtsRebuild::Files(a) => (a, true),
        FtsRebuild::Dirs(a) => (a, false),
    };
    // Files: the path is NULL for a file without a row yet (`fts` 0), which gets none.
    let ids: Vec<(i64, Option<String>)> = c
        .prepare_cached(if files {
            "SELECT id, CASE WHEN fts THEN '' END FROM files WHERE id > ?1 ORDER BY id LIMIT ?2"
        } else {
            "SELECT id, path FROM dirs WHERE id > ?1 ORDER BY id LIMIT ?2"
        })?
        .query_map(params![after, rows as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let last = ids.len() < rows;
    let end = match ids.last() {
        Some(&(id, _)) if !last => id,
        _ => i64::MAX,
    };
    c.prepare_cached(if files {
        "DELETE FROM fts_index WHERE rowid > ?1 AND rowid <= ?2"
    } else {
        "DELETE FROM fts_dirs WHERE rowid > ?1 AND rowid <= ?2"
    })?
    .execute([after, end])?;
    for (id, path) in ids {
        match path {
            None => {}
            Some(_) if files => fts_put(c, id, fold)?,
            Some(path) => {
                c.prepare_cached("INSERT INTO fts_dirs (rowid, path) VALUES (?1, ?2)")?
                    .execute(params![id, fold(&path)])?;
            }
        }
    }
    Ok(match (files, last) {
        (true, false) => Some(FtsRebuild::Files(end)),
        (true, true) => Some(FtsRebuild::Dirs(0)),
        (false, false) => Some(FtsRebuild::Dirs(end)),
        (false, true) => None,
    })
}

pub fn info_get(c: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    c.query_row("SELECT value FROM info WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn info_set(c: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO info (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// The gallery's label names (`xmp:Label`), 1-based in [`Mark::label`].
pub const LABELS: [&str; 4] = ["Red", "Yellow", "Green", "Blue"];

/// RAW extensions, for the sidecar naming rule (as the gallery's).
pub fn is_raw_ext(ext: &str) -> bool {
    specular_metadata::sidecars::is_raw_ext(ext)
}

/// Marks and keywords of an XMP sidecar (keyword paths, None when it names none), read as
/// the gallery reads them: `xmp:Rating` (-1 is
/// a reject), `xmp:Label` by name, `xmpDM:pick`.
pub fn sidecar(bytes: &[u8]) -> (Mark, Option<Vec<String>>) {
    let x = specular_metadata::xmp::parse(bytes);
    let num = |k: &str| x.get(k).and_then(|v| v.trim().parse::<i32>().ok());
    let rating = num("xmp:Rating");
    let pick = num("xmpDM:pick");
    let label = x.get("xmp:Label").and_then(|v| {
        LABELS
            .iter()
            .position(|l| l.eq_ignore_ascii_case(v.trim()))
            .map(|i| i as u8 + 1)
    });
    let mark = Mark {
        rating: rating.map_or(0, |r| r.clamp(0, 5) as u8),
        label: label.unwrap_or(0),
        flag: if rating == Some(-1) || pick == Some(-1) {
            -1
        } else if pick == Some(1) {
            1
        } else {
            0
        },
    };
    let text = String::from_utf8_lossy(bytes);
    let keywords = specular_metadata::keywords::from_xmp_text(&text, &|_| false);
    (mark, keywords)
}

/// Which `.xmp` in a folder belongs to image `name`, by the gallery's rule: the stem form
/// (`IMG_1.xmp`) is the image's own unless another image shares its stem (RAW+JPEG), where it
/// is the RAW's; the long form (`IMG_1.JPG.xmp`) always; either case of `.xmp`.
pub fn sidecar_for<'a>(
    name: &str,
    stems: &std::collections::HashMap<String, (u32, u32)>,
    xmp: &'a std::collections::HashSet<String>,
) -> Option<&'a str> {
    let stem = match name.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => name,
    };
    let own_stem = stem_is_own(name, stems);
    let short = [format!("{stem}.xmp"), format!("{stem}.XMP")];
    let long = [format!("{name}.xmp"), format!("{name}.XMP")];
    let candidates: Vec<&String> = if own_stem {
        short.iter().chain(long.iter()).collect()
    } else {
        long.iter().collect()
    };
    candidates
        .into_iter()
        .find_map(|c| xmp.get(c.as_str()).map(String::as_str))
}

/// Whether the stem form of a sidecar (`IMG_1.xmp`) is image `name`'s: unless another image
/// shares its stem (RAW+JPEG), where it is the RAW's. `stems`: stem → (images with it, RAW
/// images with it).
pub fn stem_is_own(name: &str, stems: &std::collections::HashMap<String, (u32, u32)>) -> bool {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, e),
        _ => (name, ""),
    };
    let (n, raws) = stems.get(stem).copied().unwrap_or((1, 0));
    n <= 1 || (is_raw_ext(ext) && raws == 1)
}

/// Keywords written into sidecars by the app, mirrored into the catalogue: the sidecar now
/// has the last word, so the file's own keywords and the sidecar's earlier ones are replaced.
/// Returns how many catalogued files changed.
pub fn set_keywords(
    c: &Connection,
    keywords: &[(PathBuf, Vec<String>)],
    fold: Fold,
) -> rusqlite::Result<usize> {
    let mut n = 0;
    for (p, kw) in keywords {
        let (Some(dir), Some(name)) = (
            p.parent().and_then(text),
            p.file_name().and_then(|n| n.to_str()),
        ) else {
            continue;
        };
        let id: Option<i64> = c
            .prepare_cached(
                "SELECT id FROM files WHERE dir_id = (SELECT id FROM dirs WHERE path = ?1) AND name = ?2",
            )?
            .query_row(params![dir, name], |r| r.get(0))
            .optional()?;
        let Some(id) = id else { continue };
        c.prepare_cached("DELETE FROM file_tags WHERE file_id = ?1")?
            .execute([id])?;
        for k in kw {
            crate::facts::tag(c, id, k, 1)?;
        }
        fts_refresh(c, id, fold)?;
        n += 1;
    }
    Ok(n)
}

/// IPTC fields written by the app, mirrored into the catalogue. Returns how many files
/// changed.
pub fn set_iptc(c: &Connection, fields: &[(PathBuf, String)]) -> rusqlite::Result<usize> {
    let mut n = 0;
    for (p, lines) in fields {
        let (Some(dir), Some(name)) = (
            p.parent().and_then(text),
            p.file_name().and_then(|n| n.to_str()),
        ) else {
            continue;
        };
        n += c
            .prepare_cached(
                "UPDATE files SET iptc = ?3
                 WHERE dir_id = (SELECT id FROM dirs WHERE path = ?1) AND name = ?2",
            )?
            .execute(params![dir, name, lines])?;
    }
    Ok(n)
}

/// The catalogued files with keyword `path` or one below it (`Places|France` finds
/// `Places|France|Paris` too; case does not count), in path order.
pub fn files_with_tag(c: &Connection, path: &str) -> rusqlite::Result<Vec<PathBuf>> {
    let want = specular_metadata::keywords::key(path);
    let below = format!("{want}{}", specular_metadata::keywords::SEP);
    let mut st = c.prepare_cached(
        "SELECT DISTINCT d.path, f.name, t.name FROM file_tags ft
         JOIN tags t ON t.id = ft.tag_id JOIN files f ON f.id = ft.file_id
         JOIN dirs d ON d.id = f.dir_id
         WHERE f.state <> 2 AND (t.name = ?1 COLLATE NOCASE
             OR substr(t.name, 1, length(?2)) = ?2 COLLATE NOCASE)
         ORDER BY d.path, f.name",
    )?;
    let rows = st.query_map(params![path, below], |r| {
        Ok((
            Path::new(&r.get::<_, String>(0)?).join(r.get::<_, String>(1)?),
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut out: Vec<PathBuf> = Vec::new();
    for row in rows {
        let (p, tag) = row?;
        // SQLite folds ASCII only: the keyword's own comparison decides.
        if specular_metadata::keywords::is_within(&tag, path) && out.last() != Some(&p) {
            out.push(p);
        }
    }
    Ok(out)
}

/// Every keyword the catalogue knows (tags of files and the levels above them), by name.
pub fn tag_names(c: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut st = c.prepare_cached("SELECT name FROM tags ORDER BY name")?;
    st.query_map([], |r| r.get::<_, String>(0))?.collect()
}

/// Marks of the gallery mirrored into the catalogue. Returns how many files changed. A value
/// the user changed is the user's from then on: another app's sidecar no longer sets it.
pub fn set_marks(c: &Connection, marks: &[(PathBuf, Mark)]) -> rusqlite::Result<usize> {
    let mut st = c.prepare_cached(
        "UPDATE files SET rating = ?3, label = ?4, flag = ?5,
             foreign_bits = foreign_bits & ~((rating IS NOT ?3) | ((label IS NOT ?4) << 1)
                 | ((flag IS NOT ?5) << 2))
         WHERE dir_id = (SELECT id FROM dirs WHERE path = ?1) AND name = ?2
           AND (rating, label, flag) IS NOT (?3, ?4, ?5)",
    )?;
    let mut n = 0;
    for (p, m) in marks {
        let (Some(dir), Some(name)) = (
            p.parent().and_then(text),
            p.file_name().and_then(|n| n.to_str()),
        ) else {
            continue;
        };
        n += st.execute(params![dir, name, m.rating, m.label, m.flag])?;
    }
    Ok(n)
}

/// Files in `folders` (and below) with sidecars are read again: other apps' sidecars now
/// count differently there. Their own sidecar is read again too (it may have to win again).
/// Returns how many files wait.
pub fn read_foreign_again(c: &Connection, folders: &[PathBuf]) -> rusqlite::Result<usize> {
    let mut n = 0;
    for f in folders {
        let Some(path) = text(f) else { continue };
        let (lo, hi) = crate::scan::below(path);
        n += c
            .prepare_cached(
                "UPDATE files SET state = 0, xmp_mtime = NULL WHERE sides IS NOT NULL
                 AND dir_id IN (SELECT id FROM dirs WHERE path = ?1 OR (path >= ?2 AND path < ?3))",
            )?
            .execute(params![path, lo, hi])?;
    }
    Ok(n)
}

/// What other apps' sidecars of `file` said when last read, the photo's own first in their
/// order of weight, then the duplicates by number.
pub fn foreign_sidecars(c: &Connection, file: i64) -> rusqlite::Result<Vec<crate::ForeignSidecar>> {
    let order: Option<String> = c
        .prepare_cached("SELECT sides FROM files WHERE id = ?1")?
        .query_row([file], |r| r.get(0))
        .optional()?
        .flatten();
    let mut st = c.prepare_cached(
        "SELECT name, app, duplicate, rating, label, flag, keywords, title, caption, version_name
         FROM foreign_sides WHERE file_id = ?1",
    )?;
    let mut out: Vec<crate::ForeignSidecar> = st
        .query_map([file], |r| {
            let small = |i: usize| -> rusqlite::Result<Option<i64>> { r.get(i) };
            Ok(crate::ForeignSidecar {
                name: r.get(0)?,
                app: specular_metadata::sidecars::App::from_id(&r.get::<_, String>(1)?),
                duplicate: r.get(2)?,
                values: specular_metadata::sidecars::Values {
                    rating: small(3)?.map(|v| v.clamp(0, 5) as u8),
                    label: small(4)?.map(|v| v.clamp(0, 4) as u8),
                    flag: small(5)?.map(|v| v.clamp(-1, 1) as i8),
                    keywords: r
                        .get::<_, Option<String>>(6)?
                        .map(|k| k.lines().map(str::to_owned).collect()),
                    title: r.get(7)?,
                    caption: r.get(8)?,
                },
                version_name: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let rank = |n: &str| {
        order
            .as_deref()
            .and_then(|o| crate::scan::sides_names(o).position(|(x, _)| x == n))
            .unwrap_or(usize::MAX)
    };
    out.sort_by_key(|f| (f.duplicate, rank(&f.name)));
    Ok(out)
}

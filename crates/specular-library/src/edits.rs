//! Develop edits in the catalogue: the current edit
//! of each version of a file, and its history steps.
//!
//! The blobs are `specular_develop`'s codec forms (`encode_edit`, `encode_step`); this crate
//! stores them without reading them. The hash is the edit's output hash (edited thumbnails
//! key on it). Files outside the watched locations have no row here: their edits live in
//! the `.specular.xmp` sidecar only.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::store::text;

/// One version's edit as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredEdit {
    /// `specular_develop::model::codec::encode_edit`.
    pub edit: Vec<u8>,
    /// `specular_develop::model::hash::edit_hash`.
    pub hash: Vec<u8>,
    /// History steps in order (`encode_step`) and how many are applied.
    pub steps: Vec<Vec<u8>>,
    pub position: u32,
    /// Seconds since the epoch.
    pub updated: i64,
    /// The version's name (empty: none).
    pub name: String,
    /// What the edit uses, for search: the modules' stable ids and the styles applied (from
    /// its history), one per line. None: not known (saved by an older build).
    pub modules: Option<String>,
    pub styles: Option<String>,
}

/// What search knows of one file's Develop edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditFacts {
    /// The first version's (the photo's own edit) modules and styles, one per line; None
    /// when not known (an edit saved by an older build) or there is no first version.
    pub modules: Option<String>,
    pub styles: Option<String>,
    /// Whether the first version has an edit.
    pub edited: bool,
    /// How many versions have an edit.
    pub versions: u32,
}

/// Every catalogued file with a Develop edit, and what its edits use: one pass over the
/// table, for search.
pub fn all_facts(c: &Connection) -> rusqlite::Result<std::collections::HashMap<i64, EditFacts>> {
    let mut out: std::collections::HashMap<i64, EditFacts> = std::collections::HashMap::new();
    let mut st = c.prepare_cached("SELECT file_id, version, modules, styles FROM edits")?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        let f = out.entry(r.get(0)?).or_default();
        f.versions += 1;
        if r.get::<_, i64>(1)? == 0 {
            f.edited = true;
            f.modules = r.get(2)?;
            f.styles = r.get(3)?;
        }
    }
    // darktable's duplicates are versions of the photo too.
    let mut st = c.prepare_cached(
        "SELECT file_id, COUNT(*) FROM foreign_sides WHERE duplicate > 0 GROUP BY file_id",
    )?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        let n: i64 = r.get(1)?;
        let f = out.entry(r.get(0)?).or_insert_with(|| EditFacts {
            modules: Some(String::new()),
            styles: Some(String::new()),
            ..EditFacts::default()
        });
        f.versions = f.versions.max(1) + n as u32;
    }
    Ok(out)
}

/// How many darktable duplicates (`IMG_01.CR2.xmp`) each of `files` has, for those with any.
pub fn duplicate_counts(c: &Connection, files: &[i64]) -> rusqlite::Result<Vec<(i64, u32)>> {
    let mut st = c.prepare_cached(
        "SELECT COUNT(*) FROM foreign_sides WHERE file_id = ?1 AND duplicate > 0",
    )?;
    let mut out = Vec::new();
    for &f in files {
        let n: i64 = st.query_row([f], |r| r.get(0))?;
        if n > 0 {
            out.push((f, n as u32));
        }
    }
    Ok(out)
}

/// The catalogue id of the file at `path`, when it is catalogued.
pub fn file_id(c: &Connection, path: &Path) -> rusqlite::Result<Option<i64>> {
    let (Some(dir), Some(name)) = (
        path.parent().and_then(text),
        path.file_name().and_then(|n| n.to_str()),
    ) else {
        return Ok(None);
    };
    c.prepare_cached(
        "SELECT f.id FROM files f JOIN dirs d ON d.id = f.dir_id WHERE d.path = ?1 AND f.name = ?2",
    )?
    .query_row(params![dir, name], |r| r.get(0))
    .optional()
}

/// Writes version `version` of `file`'s edit and its history, replacing what was there.
pub fn save(c: &mut Connection, file: i64, version: u32, e: &StoredEdit) -> rusqlite::Result<()> {
    let tx = c.transaction()?;
    tx.execute(
        "INSERT INTO edits (file_id, version, edit, hash, position, updated, name, modules, styles)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT (file_id, version) DO UPDATE SET
            edit = excluded.edit, hash = excluded.hash, position = excluded.position,
            updated = excluded.updated, name = excluded.name, modules = excluded.modules,
            styles = excluded.styles",
        params![
            file, version, e.edit, e.hash, e.position, e.updated, e.name, e.modules, e.styles
        ],
    )?;
    tx.execute(
        "DELETE FROM edit_steps WHERE file_id = ?1 AND version = ?2",
        params![file, version],
    )?;
    {
        let mut st = tx.prepare_cached(
            "INSERT INTO edit_steps (file_id, version, seq, step) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for (i, s) in e.steps.iter().enumerate() {
            st.execute(params![file, version, i as i64, s])?;
        }
    }
    tx.commit()
}

/// Stores version `version` of `file`'s edit unless it has one, inside the caller's
/// transaction (the walk's sidecar import).
pub fn insert(c: &Connection, file: i64, version: u32, e: &StoredEdit) -> rusqlite::Result<()> {
    let n = c.execute(
        "INSERT OR IGNORE INTO edits (file_id, version, edit, hash, position, updated, name,
         modules, styles) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            file, version, e.edit, e.hash, e.position, e.updated, e.name, e.modules, e.styles
        ],
    )?;
    if n == 0 {
        return Ok(());
    }
    c.execute(
        "DELETE FROM edit_steps WHERE file_id = ?1 AND version = ?2",
        params![file, version],
    )?;
    let mut st = c.prepare_cached(
        "INSERT INTO edit_steps (file_id, version, seq, step) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (i, s) in e.steps.iter().enumerate() {
        st.execute(params![file, version, i as i64, s])?;
    }
    Ok(())
}

/// The photos in the folder `dir` with an edit (any version).
pub fn edited_in(c: &Connection, dir: &str) -> rusqlite::Result<Vec<String>> {
    c.prepare_cached(
        "SELECT DISTINCT f.name FROM edits e JOIN files f ON f.id = e.file_id
         JOIN dirs d ON d.id = f.dir_id WHERE d.path = ?1",
    )?
    .query_map([dir], |r| r.get(0))?
    .collect()
}

/// Reads version `version` of `file`'s edit.
pub fn load(c: &Connection, file: i64, version: u32) -> rusqlite::Result<Option<StoredEdit>> {
    let Some((edit, hash, position, updated, name, modules, styles)) = c
        .prepare_cached(
            "SELECT edit, hash, position, updated, coalesce(name, ''), modules, styles
             FROM edits WHERE file_id = ?1 AND version = ?2",
        )?
        .query_row(params![file, version], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .optional()?
    else {
        return Ok(None);
    };
    let steps = c
        .prepare_cached(
            "SELECT step FROM edit_steps WHERE file_id = ?1 AND version = ?2 ORDER BY seq",
        )?
        .query_map(params![file, version], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<Vec<u8>>>>()?;
    Ok(Some(StoredEdit {
        edit,
        hash,
        steps,
        position,
        updated,
        name,
        modules,
        styles,
    }))
}

/// The versions `file` has edits for, in order.
pub fn versions(c: &Connection, file: i64) -> rusqlite::Result<Vec<u32>> {
    c.prepare_cached("SELECT version FROM edits WHERE file_id = ?1 ORDER BY version")?
        .query_map([file], |r| r.get(0))?
        .collect()
}

/// The versions `file` has edits for, with their names (empty: none), in order.
pub fn named_versions(c: &Connection, file: i64) -> rusqlite::Result<Vec<(u32, String)>> {
    c.prepare_cached(
        "SELECT version, coalesce(name, '') FROM edits WHERE file_id = ?1 ORDER BY version",
    )?
    .query_map([file], |r| Ok((r.get(0)?, r.get(1)?)))?
    .collect()
}

/// How many versions each of `files` has (files without edits are left out). For the
/// gallery's badge on photos with more than one.
pub fn version_counts(c: &Connection, files: &[i64]) -> rusqlite::Result<Vec<(i64, u32)>> {
    let mut st = c.prepare_cached("SELECT count(*) FROM edits WHERE file_id = ?1")?;
    let mut out = Vec::new();
    for &f in files {
        let n: u32 = st.query_row([f], |r| r.get(0))?;
        if n > 0 {
            out.push((f, n));
        }
    }
    Ok(out)
}

/// Removes version `version` of `file`'s edit and its history.
pub fn delete(c: &Connection, file: i64, version: u32) -> rusqlite::Result<bool> {
    c.execute(
        "DELETE FROM edit_steps WHERE file_id = ?1 AND version = ?2",
        params![file, version],
    )?;
    Ok(c.execute(
        "DELETE FROM edits WHERE file_id = ?1 AND version = ?2",
        params![file, version],
    )? > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::schema::configure(&c).unwrap();
        crate::schema::migrate(&mut c).unwrap();
        c.execute_batch(
            "INSERT INTO folders (id, path, added) VALUES (1, '/photos', 0);
             INSERT INTO dirs (id, folder_id, path) VALUES (1, 1, '/photos');
             INSERT INTO files (id, dir_id, name, ext, size, mtime_ns)
                 VALUES (7, 1, 'IMG_1.CR2', 'cr2', 10, 0);",
        )
        .unwrap();
        c
    }

    fn stored(n: u8) -> StoredEdit {
        StoredEdit {
            edit: vec![n; 40],
            hash: vec![n; 32],
            steps: (0..n).map(|i| vec![i; 5]).collect(),
            position: u32::from(n),
            updated: 1_730_000_000,
            name: if n == 1 {
                "Black & white".into()
            } else {
                String::new()
            },
            modules: (n != 2).then(|| "exposure\ncolour_balance".into()),
            styles: (n == 3).then(|| "Punchy".into()),
        }
    }

    #[test]
    fn edits_round_trip_replace_and_go_with_the_file() {
        let mut c = catalogue();
        let id = file_id(&c, Path::new("/photos/IMG_1.CR2"))
            .unwrap()
            .unwrap();
        assert_eq!(id, 7);
        assert_eq!(
            file_id(&c, Path::new("/elsewhere/IMG_1.CR2")).unwrap(),
            None
        );
        assert_eq!(load(&c, id, 0).unwrap(), None);
        save(&mut c, id, 0, &stored(3)).unwrap();
        save(&mut c, id, 1, &stored(1)).unwrap();
        assert_eq!(load(&c, id, 0).unwrap(), Some(stored(3)));
        // A shorter history replaces the longer one.
        save(&mut c, id, 0, &stored(2)).unwrap();
        assert_eq!(load(&c, id, 0).unwrap(), Some(stored(2)));
        assert_eq!(versions(&c, id).unwrap(), vec![0, 1]);
        assert_eq!(
            named_versions(&c, id).unwrap(),
            vec![(0, String::new()), (1, "Black & white".into())]
        );
        assert_eq!(version_counts(&c, &[7, 8]).unwrap(), vec![(7, 2)]);
        // What search sees: the first version's summary (not known: saved without one).
        let f = all_facts(&c).unwrap();
        assert_eq!(
            f.get(&7),
            Some(&EditFacts {
                modules: None,
                styles: None,
                edited: true,
                versions: 2,
            })
        );
        save(&mut c, id, 0, &stored(3)).unwrap();
        let f = all_facts(&c).unwrap();
        assert_eq!(f[&7].modules.as_deref(), Some("exposure\ncolour_balance"));
        assert_eq!(f[&7].styles.as_deref(), Some("Punchy"));
        save(&mut c, id, 0, &stored(2)).unwrap();
        assert!(delete(&c, id, 1).unwrap());
        assert_eq!(versions(&c, id).unwrap(), vec![0]);
        // The file leaving the catalogue takes its edits and steps.
        c.execute("DELETE FROM files WHERE id = 7", []).unwrap();
        let n: i64 = c
            .query_row(
                "SELECT (SELECT count(*) FROM edits) + (SELECT count(*) FROM edit_steps)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }
}

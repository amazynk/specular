//! The Local AI pack's tables (DESIGN §10 "Storage", "Local AI pack").
//!
//! The pack is optional and each of its features is installed on its own, so its tables are
//! not part of the core migrations: a feature's tables are created when the feature is
//! installed and dropped again when it is removed (removing a feature or the pack leaves no
//! trace in the catalogue). The core schema only holds the register, `ai_schema` (migration
//! 2): which features have tables, at which version of their own schema. A feature's tables
//! are versioned like the core: [`install`] brings an older version forward, step by step.
//!
//! Every table refers to `files(id)` with `ON DELETE CASCADE`, so a file leaving the
//! catalogue takes its embeddings, faces, text and scores with it.
//!
//! Work is marked done per file and model: a row in the feature's done table (`embeddings`,
//! `ocr_text`, `ai_scores`, `face_scans`) for the current model. A new model version makes
//! every file pending again; the old rows are replaced as the new model reaches them.

use rusqlite::{Connection, OptionalExtension, params};

/// One feature's tables.
pub struct Tables {
    /// The feature's id (`specular_ai::Feature::id`).
    pub feature: &'static str,
    /// Its schema steps, oldest first; the version is the number applied.
    pub steps: &'static [&'static str],
    /// Dropped (children first) when the feature is removed.
    pub tables: &'static [&'static str],
    /// The table with one row per file done, and its model column.
    pub done: &'static str,
}

/// Every feature's tables.
pub const FEATURES: &[Tables] = &[
    Tables {
        feature: "search",
        steps: &[SEARCH_V1, SEARCH_V2],
        tables: &["embeddings"],
        done: "embeddings",
    },
    Tables {
        feature: "text",
        steps: &[TEXT_V1, TEXT_V2],
        tables: &["ocr_text"],
        done: "ocr_text",
    },
    Tables {
        feature: "culling",
        steps: &[CULLING_V1, CULLING_V2],
        tables: &["ai_scores"],
        done: "ai_scores",
    },
    Tables {
        feature: "people",
        steps: &[PEOPLE_V1, PEOPLE_V2, PEOPLE_V3],
        tables: &["faces", "face_scans", "people"],
        done: "face_scans",
    },
];

const SEARCH_V1: &str = r#"
-- One image embedding per file (search by description, similar, clustering): `dims` f16
-- values, little-endian, in `vec`.
CREATE TABLE embeddings (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    model   TEXT NOT NULL,
    dims    INTEGER NOT NULL,
    vec     BLOB NOT NULL,
    at      INTEGER NOT NULL
);
CREATE INDEX embeddings_model ON embeddings(model);
"#;

/// One embedding per file and model: switching the Search model's size re-embeds into new
/// rows while the old model's rows keep answering searches, and the old rows go only when
/// the new model has every file ([`prune_embeddings`]). The rowid stays, so a reader can
/// pick up rows added since it last looked ([`embeddings_since`]).
const SEARCH_V2: &str = r#"
CREATE TABLE embeddings_v2 (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    model   TEXT NOT NULL,
    dims    INTEGER NOT NULL,
    vec     BLOB NOT NULL,
    at      INTEGER NOT NULL,
    PRIMARY KEY (file_id, model)
);
INSERT INTO embeddings_v2(file_id, model, dims, vec, at)
    SELECT file_id, model, dims, vec, at FROM embeddings;
DROP TABLE embeddings;
ALTER TABLE embeddings_v2 RENAME TO embeddings;
CREATE INDEX embeddings_model ON embeddings(model);
"#;

const TEXT_V1: &str = r#"
-- Text found in the image (OCR): the text, and its boxes as JSON.
CREATE TABLE ocr_text (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    model   TEXT NOT NULL,
    text    TEXT NOT NULL,
    boxes   TEXT NOT NULL DEFAULT '[]',
    at      INTEGER NOT NULL
);
CREATE INDEX ocr_text_model ON ocr_text(model);
"#;

const TEXT_V2: &str = r#"
-- What a reading found, so a script add-on installed or removed re-reads only the files it
-- matters to: space-separated tokens, a script ("latn", "arab", …) per line read, "addon:<model>"
-- per add-on that read a line, "?" when a line found could not be read; "?" for older rows.
ALTER TABLE ocr_text ADD COLUMN scripts TEXT NOT NULL DEFAULT '?';
"#;

const CULLING_V1: &str = r#"
-- Culling assistance (DESIGN §8.5): scores from 0 to 1, NULL where the model has no opinion
-- (no faces: no eyes to be open).
CREATE TABLE ai_scores (
    file_id    INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    model      TEXT NOT NULL,
    sharpness  REAL,
    eyes_open  REAL,
    eye_focus  REAL,
    expression REAL,
    at         INTEGER NOT NULL
);
CREATE INDEX ai_scores_model ON ai_scores(model);
"#;

const CULLING_V2: &str = r#"
-- Exposure (mean luminance; the fraction clipped or crushed), 0 to 1, and the faces seen as
-- JSON [[x, y, w, h, closed], …] (box in fractions of the image, closed eyes 0 to 1).
ALTER TABLE ai_scores ADD COLUMN exposure REAL;
ALTER TABLE ai_scores ADD COLUMN clipped REAL;
ALTER TABLE ai_scores ADD COLUMN faces TEXT;
"#;

const PEOPLE_V1: &str = r#"
-- People the user named or that face clustering formed.
CREATE TABLE people (
    id         INTEGER PRIMARY KEY,
    name       TEXT,
    -- The face shown for the person.
    cover_face INTEGER,
    created    INTEGER NOT NULL
);

-- Faces found: box (fractions of the image), detector score, embedding (f16 LE), and the
-- person it belongs to (none until clustered or named).
CREATE TABLE faces (
    id        INTEGER PRIMARY KEY,
    file_id   INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    person_id INTEGER REFERENCES people(id) ON DELETE SET NULL,
    x REAL NOT NULL, y REAL NOT NULL, w REAL NOT NULL, h REAL NOT NULL,
    score     REAL NOT NULL,
    vec       BLOB,
    model     TEXT NOT NULL
);
CREATE INDEX faces_file ON faces(file_id);
CREATE INDEX faces_person ON faces(person_id);

-- Files looked at by the face detector (also those with no face): the done marks. Kept when
-- a person is forgotten, so their faces are not found and grouped again.
CREATE TABLE face_scans (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    model   TEXT NOT NULL,
    faces   INTEGER NOT NULL,
    at      INTEGER NOT NULL
);
"#;

const PEOPLE_V3: &str = r#"
-- The face's roll: the angle of its eye line in the image, radians, clockwise (0 upright,
-- ±π/2 lying on its side, π upside down), so its box is drawn turned with it.
ALTER TABLE faces ADD COLUMN roll REAL NOT NULL DEFAULT 0;
"#;

const PEOPLE_V2: &str = r#"
-- 1: the user placed this face (gave it to a person, or took it from one): grouping never
-- moves it.
ALTER TABLE faces ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
"#;

/// The tables of feature `id`.
pub fn tables(feature: &str) -> Option<&'static Tables> {
    FEATURES.iter().find(|t| t.feature == feature)
}

fn unknown(feature: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(format!("unknown AI feature {feature:?}"))
}

/// Features with tables, and their schema version.
pub fn installed(conn: &Connection) -> rusqlite::Result<Vec<(String, u32)>> {
    let mut st = conn.prepare("SELECT feature, version FROM ai_schema ORDER BY feature")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Whether feature `id` has its tables.
pub fn has(conn: &Connection, feature: &str) -> rusqlite::Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM ai_schema WHERE feature = ?1",
            [feature],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// A write transaction that takes the write lock at `BEGIN`, waiting out another writer.
///
/// Never a deferred one: a transaction that reads first and then writes while another
/// connection holds the write lock fails at once with "database is locked" (SQLite does not
/// run the busy handler for a read transaction that wants to become a write one in WAL).
fn write_tx(conn: &mut Connection) -> rusqlite::Result<rusqlite::Transaction<'_>> {
    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
}

/// Creates feature `id`'s tables, or brings them up to the current version. Returns the
/// version found (0: not installed).
pub fn install(conn: &mut Connection, feature: &str) -> rusqlite::Result<u32> {
    specular_platform::assert_not_ui_thread("ai::install");
    let t = tables(feature).ok_or_else(|| unknown(feature))?;
    let tx = write_tx(conn)?;
    let found: u32 = tx
        .query_row(
            "SELECT version FROM ai_schema WHERE feature = ?1",
            [feature],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    for step in t.steps.iter().skip(found as usize) {
        tx.execute_batch(step)?;
    }
    let version = t.steps.len() as u32;
    if found < version {
        tx.execute(
            "INSERT INTO ai_schema(feature, version) VALUES (?1, ?2)
             ON CONFLICT(feature) DO UPDATE SET version = excluded.version",
            params![feature, version],
        )?;
        specular_log::info!(target: specular_log::FOLDER, "library: AI tables of {feature} at {version}");
    }
    tx.commit()?;
    Ok(found)
}

/// Drops feature `id`'s tables and everything in them. Not installed is not an error.
pub fn drop(conn: &mut Connection, feature: &str) -> rusqlite::Result<()> {
    specular_platform::assert_not_ui_thread("ai::drop");
    let t = tables(feature).ok_or_else(|| unknown(feature))?;
    let tx = write_tx(conn)?;
    for table in t.tables {
        tx.execute_batch(&format!("DROP TABLE IF EXISTS {table};"))?;
    }
    tx.execute("DELETE FROM ai_schema WHERE feature = ?1", [feature])?;
    tx.commit()?;
    specular_log::info!(target: specular_log::FOLDER, "library: AI tables of {feature} dropped");
    Ok(())
}

/// Drops every feature's tables (the pack was removed).
pub fn drop_all(conn: &mut Connection) -> rusqlite::Result<()> {
    specular_platform::assert_not_ui_thread("ai::drop_all");
    for t in FEATURES {
        drop(conn, t.feature)?;
    }
    Ok(())
}

/// A file waiting for a feature, with what finds its cached thumbnail
/// (`specular_thumbs::ThumbKey` from size, time stamp and content hash) without reading it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub file_id: i64,
    pub size: u64,
    pub mtime_ns: i128,
    /// `thumbs.content`; `None` when the indexer has not recorded the thumbnail yet.
    pub content: Option<u64>,
    /// `thumbs.classes`: size classes found in the cache (bit n: `CLASSES[n]`).
    pub classes: u32,
}

/// Up to `limit` readable files after `after` (file id) that `model` has not done yet for
/// `feature`, in file order: a job walks the catalogue once with this cursor and starts over
/// from 0 when it reaches the end (files whose thumbnails came since).
pub fn pending(
    conn: &Connection,
    feature: &str,
    model: &str,
    after: i64,
    limit: usize,
) -> rusqlite::Result<Vec<Pending>> {
    let t = tables(feature).ok_or_else(|| unknown(feature))?;
    let sql = format!(
        "SELECT f.id, f.size, f.mtime_ns, t.content, t.classes FROM files f
         LEFT JOIN thumbs t ON t.file_id = f.id
         LEFT JOIN {done} d ON d.file_id = f.id AND d.model = ?1
         WHERE f.state = 1 AND f.id > ?2 AND d.file_id IS NULL
         ORDER BY f.id LIMIT ?3",
        done = t.done
    );
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(params![model, after, limit as i64], |r| {
        Ok(Pending {
            file_id: r.get(0)?,
            size: r.get::<_, i64>(1)? as u64,
            mtime_ns: r.get::<_, i64>(2)? as i128,
            content: r.get::<_, Option<i64>>(3)?.map(|c| c as u64),
            classes: r.get::<_, Option<i64>>(4)?.unwrap_or(0) as u32,
        })
    })?;
    rows.collect()
}

/// Readable files in the catalogue, and how many of them `model` has done for `feature`.
pub fn counts(conn: &Connection, feature: &str, model: &str) -> rusqlite::Result<(u64, u64)> {
    specular_platform::assert_not_ui_thread("ai::counts");
    let t = tables(feature).ok_or_else(|| unknown(feature))?;
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM files WHERE state = 1", [], |r| {
        r.get(0)
    })?;
    let done: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM {} WHERE model = ?1", t.done),
        [model],
        |r| r.get(0),
    )?;
    Ok((total as u64, done as u64))
}

/// Seconds since the epoch.
fn now() -> i64 {
    crate::now_s()
}

/// f16 bits as the little-endian BLOB stored.
pub fn f16_blob(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The f16 bits of a stored BLOB (a trailing odd byte is ignored).
pub fn blob_f16(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

// -- search ------------------------------------------------------------------------------

/// Stores (replacing) a file's embedding: f16 bits.
pub fn put_embedding(
    conn: &Connection,
    file_id: i64,
    model: &str,
    vec: &[u16],
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT OR REPLACE INTO embeddings(file_id, model, dims, vec, at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![file_id, model, vec.len() as i64, f16_blob(vec), now()])?;
    Ok(())
}

/// Every embedding of `model` with `dims` values, in file order: (file id, f16 bits).
pub fn embeddings(
    conn: &Connection,
    model: &str,
    dims: usize,
) -> rusqlite::Result<Vec<(i64, Vec<u16>)>> {
    let mut st = conn.prepare(
        "SELECT file_id, vec FROM embeddings WHERE model = ?1 AND dims = ?2 ORDER BY file_id",
    )?;
    let rows = st.query_map(params![model, dims as i64], |r| {
        Ok((r.get(0)?, blob_f16(&r.get::<_, Vec<u8>>(1)?)))
    })?;
    rows.collect()
}

/// Embeddings read since a rowid: (file id, f16 bits) in store order, and the last rowid read.
pub type EmbeddingsSince = (Vec<(i64, Vec<u16>)>, i64);

/// Embeddings of `model` with `dims` values stored after rowid `after` (0: all), in the
/// order they were stored: (file id, f16 bits), and the last rowid read (`after` when none).
/// A file embedded again shows up again (the reader keeps the later one).
pub fn embeddings_since(
    conn: &Connection,
    model: &str,
    dims: usize,
    after: i64,
) -> rusqlite::Result<EmbeddingsSince> {
    specular_platform::assert_not_ui_thread("ai::embeddings_since");
    let mut st = conn.prepare_cached(
        "SELECT rowid, file_id, vec FROM embeddings
         WHERE model = ?1 AND dims = ?2 AND rowid > ?3 ORDER BY rowid",
    )?;
    let mut last = after;
    let rows = st.query_map(params![model, dims as i64, after], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            blob_f16(&r.get::<_, Vec<u8>>(2)?),
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (rowid, id, v) = r?;
        last = last.max(rowid);
        out.push((id, v));
    }
    Ok((out, last))
}

/// Rows of `model` (any size); cheap with the model index.
pub fn embedding_count(conn: &Connection, model: &str) -> rusqlite::Result<u64> {
    specular_platform::assert_not_ui_thread("ai::embedding_count");
    conn.query_row(
        "SELECT COUNT(*) FROM embeddings WHERE model = ?1",
        [model],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n as u64)
}

/// Deletes every embedding whose model is not one of `keep` (a size switched away from,
/// once the new size has every file). Returns the rows deleted.
pub fn prune_embeddings(conn: &Connection, keep: &[&str]) -> rusqlite::Result<usize> {
    specular_platform::assert_not_ui_thread("ai::prune_embeddings");
    if keep.is_empty() {
        return conn.execute("DELETE FROM embeddings", []);
    }
    let marks = vec!["?"; keep.len()].join(", ");
    conn.execute(
        &format!("DELETE FROM embeddings WHERE model NOT IN ({marks})"),
        rusqlite::params_from_iter(keep.iter()),
    )
}

// -- text --------------------------------------------------------------------------------

/// Stores (replacing) the text found in a file; `boxes` is JSON, `scripts` what the reading
/// found (see the `scripts` column).
pub fn put_text(
    conn: &Connection,
    file_id: i64,
    model: &str,
    text: &str,
    boxes: &str,
    scripts: &str,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT OR REPLACE INTO ocr_text(file_id, model, text, boxes, at, scripts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![file_id, model, text, boxes, now(), scripts])?;
    Ok(())
}

/// Marks as done by `model` the readings of other models that `still_good(their model,
/// their scripts)` says need no new reading (a script add-on changed that none of their lines
/// need), so only the rest are read again. How many were kept.
pub fn rekey_text(
    conn: &mut Connection,
    model: &str,
    still_good: &dyn Fn(&str, &str) -> bool,
) -> rusqlite::Result<usize> {
    specular_platform::assert_not_ui_thread("ai::rekey_text");
    let tx = write_tx(conn)?;
    let rows: Vec<(i64, String, String)> = {
        let mut st =
            tx.prepare("SELECT file_id, model, scripts FROM ocr_text WHERE model != ?1")?;
        st.query_map([model], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut kept = 0;
    {
        let mut up = tx.prepare("UPDATE ocr_text SET model = ?1 WHERE file_id = ?2")?;
        for (id, m, sc) in rows {
            if still_good(&m, &sc) {
                up.execute(params![model, id])?;
                kept += 1;
            }
        }
    }
    tx.commit()?;
    Ok(kept)
}

// -- culling -----------------------------------------------------------------------------

/// Culling scores of one file, each from 0 to 1 where the model gives one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scores {
    pub sharpness: Option<f32>,
    pub eyes_open: Option<f32>,
    pub eye_focus: Option<f32>,
    pub expression: Option<f32>,
    /// Mean luminance.
    pub exposure: Option<f32>,
    /// Fraction of pixels clipped or crushed.
    pub clipped: Option<f32>,
    /// Faces seen: box in fractions of the image and how closed the eyes are.
    pub faces: Vec<EyeFace>,
}

/// A face culling looked at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EyeFace {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 0 open to 1 closed (the more open eye decides).
    pub closed: f32,
}

fn faces_json(f: &[EyeFace]) -> String {
    let rows: Vec<String> = f
        .iter()
        .map(|e| {
            format!(
                "[{:.4},{:.4},{:.4},{:.4},{:.3}]",
                e.x, e.y, e.w, e.h, e.closed
            )
        })
        .collect();
    format!("[{}]", rows.join(","))
}

/// Reads [`faces_json`]'s output (anything else gives no faces).
fn json_faces(s: &str) -> Vec<EyeFace> {
    s.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split("],")
        .filter_map(|row| {
            let v: Vec<f32> = row
                .trim_matches(|c| c == '[' || c == ']' || c == ' ')
                .split(',')
                .map(|x| x.trim().parse().ok())
                .collect::<Option<_>>()?;
            let [x, y, w, h, closed] = v[..] else {
                return None;
            };
            Some(EyeFace { x, y, w, h, closed })
        })
        .collect()
}

/// The culling scores of a file (None: not scored).
pub fn scores(conn: &Connection, file_id: i64) -> rusqlite::Result<Option<Scores>> {
    conn.prepare_cached(
        "SELECT sharpness, eyes_open, eye_focus, expression, exposure, clipped, faces
         FROM ai_scores WHERE file_id = ?1",
    )?
    .query_row([file_id], |r| {
        Ok(Scores {
            sharpness: r.get(0)?,
            eyes_open: r.get(1)?,
            eye_focus: r.get(2)?,
            expression: r.get(3)?,
            exposure: r.get(4)?,
            clipped: r.get(5)?,
            faces: r
                .get::<_, Option<String>>(6)?
                .map(|s| json_faces(&s))
                .unwrap_or_default(),
        })
    })
    .optional()
}

pub fn put_scores(
    conn: &Connection,
    file_id: i64,
    model: &str,
    s: &Scores,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT OR REPLACE INTO ai_scores(file_id, model, sharpness, eyes_open, eye_focus, expression, at, exposure, clipped, faces)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?
    .execute(params![
        file_id,
        model,
        s.sharpness,
        s.eyes_open,
        s.eye_focus,
        s.expression,
        now(),
        s.exposure,
        s.clipped,
        faces_json(&s.faces),
    ])?;
    Ok(())
}

// -- people ------------------------------------------------------------------------------

/// A face found: box in fractions of the image, detector score, embedding (f16 bits).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Face {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
    pub vec: Vec<u16>,
    /// Radians, clockwise (see the `roll` column).
    pub roll: f32,
}

/// Records the faces found in a file (replacing those of an earlier scan) and marks it
/// scanned, in one transaction.
pub fn put_faces(
    conn: &mut Connection,
    file_id: i64,
    model: &str,
    faces: &[Face],
) -> rusqlite::Result<()> {
    let tx = write_tx(conn)?;
    tx.execute("DELETE FROM faces WHERE file_id = ?1", [file_id])?;
    {
        let mut st = tx.prepare_cached(
            "INSERT INTO faces(file_id, x, y, w, h, score, vec, model, roll)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for f in faces {
            let vec = (!f.vec.is_empty()).then(|| f16_blob(&f.vec));
            st.execute(params![
                file_id, f.x, f.y, f.w, f.h, f.score, vec, model, f.roll
            ])?;
        }
    }
    tx.execute(
        "INSERT OR REPLACE INTO face_scans(file_id, model, faces, at) VALUES (?1, ?2, ?3, ?4)",
        params![file_id, model, faces.len() as i64, now()],
    )?;
    tx.commit()
}

/// A person and how many faces are theirs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub id: i64,
    pub name: Option<String>,
    pub faces: u64,
    /// The face shown for them (their largest).
    pub cover: Option<i64>,
}

/// Adds a person (named or not). Returns their id.
pub fn add_person(conn: &Connection, name: Option<&str>) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO people(name, created) VALUES (?1, ?2)",
        params![name, now()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Gives faces to a person (`None`: to nobody).
pub fn assign_faces(
    conn: &mut Connection,
    person: Option<i64>,
    faces: &[i64],
) -> rusqlite::Result<()> {
    let tx = write_tx(conn)?;
    {
        let mut st = tx.prepare_cached("UPDATE faces SET person_id = ?1 WHERE id = ?2")?;
        for f in faces {
            st.execute(params![person, f])?;
        }
    }
    tx.commit()
}

/// Every person, named first (by name), then by number of faces.
pub fn people(conn: &Connection) -> rusqlite::Result<Vec<Person>> {
    specular_platform::assert_not_ui_thread("ai::people");
    let mut st = conn.prepare(
        "SELECT p.id, p.name, COUNT(f.id),
                (SELECT g.id FROM faces g WHERE g.person_id = p.id ORDER BY g.w * g.h DESC, g.id LIMIT 1)
         FROM people p LEFT JOIN faces f ON f.person_id = p.id
         GROUP BY p.id ORDER BY p.name IS NULL, p.name COLLATE NOCASE, COUNT(f.id) DESC, p.id",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Person {
            id: r.get(0)?,
            name: r.get(1)?,
            faces: r.get::<_, i64>(2)? as u64,
            cover: r.get(3)?,
        })
    })?;
    rows.collect()
}

/// The ids of every face in file `file_id`, in the order found.
pub fn faces_of(conn: &Connection, file_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut st = conn.prepare("SELECT id FROM faces WHERE file_id = ?1 ORDER BY id")?;
    let rows = st.query_map([file_id], |r| r.get(0))?;
    rows.collect()
}

/// Forgets one person: their name, their faces (boxes and embeddings). The files stay marked
/// as scanned, so the faces are not found and grouped again. Returns the faces deleted.
pub fn forget_person(conn: &mut Connection, person: i64) -> rusqlite::Result<usize> {
    specular_platform::assert_not_ui_thread("ai::forget_person");
    let tx = write_tx(conn)?;
    let n = tx.execute("DELETE FROM faces WHERE person_id = ?1", [person])?;
    tx.execute("DELETE FROM people WHERE id = ?1", [person])?;
    tx.commit()?;
    Ok(n)
}

/// Forgets every face and person, and which files were scanned. The tables stay (the
/// feature is still installed); the caller turns People off so nothing is found again.
pub fn forget_all_faces(conn: &mut Connection) -> rusqlite::Result<()> {
    specular_platform::assert_not_ui_thread("ai::forget_all_faces");
    let tx = write_tx(conn)?;
    tx.execute_batch("DELETE FROM faces; DELETE FROM people; DELETE FROM face_scans;")?;
    tx.commit()
}

/// The file `file_id` names on disk (None: not catalogued).
pub fn file_path(conn: &Connection, file_id: i64) -> rusqlite::Result<Option<std::path::PathBuf>> {
    use rusqlite::OptionalExtension;
    conn.prepare_cached(
        "SELECT d.path, f.name FROM files f JOIN dirs d ON d.id = f.dir_id WHERE f.id = ?1",
    )?
    .query_row([file_id], |r| {
        let (dir, name): (String, String) = (r.get(0)?, r.get(1)?);
        Ok(std::path::Path::new(&dir).join(name))
    })
    .optional()
}

/// The catalogue's `info` key of [`text_indexing`].
const TEXT_INDEXING: &str = "ai.text_indexing";

/// Whether the text in this library's photos is read for search: off until the user turns
/// it on for the library (as face grouping).
pub fn text_indexing(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(crate::store::info_get(conn, TEXT_INDEXING)?.as_deref() == Some("on"))
}

/// Turns reading the text in this library's photos on or off. Off stops reading; text
/// already found stays searchable until the Text feature is removed.
pub fn set_text_indexing(conn: &Connection, on: bool) -> rusqlite::Result<()> {
    crate::store::info_set(conn, TEXT_INDEXING, if on { "on" } else { "off" })
}

/// Files whose text (any model's) holds every word of `words` (compared case-folded, as
/// `fold` folds), with how often the words occur, most first. At most `limit`.
pub fn find_text(
    conn: &Connection,
    words: &[String],
    fold: &dyn Fn(&str) -> String,
    limit: usize,
) -> rusqlite::Result<Vec<(i64, f32)>> {
    specular_platform::assert_not_ui_thread("ai::find_text");
    let words: Vec<String> = words
        .iter()
        .map(|w| fold(w))
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let mut st = conn.prepare_cached("SELECT file_id, text FROM ocr_text")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut hits = Vec::new();
    for row in rows {
        let (id, text) = row?;
        let t = fold(&text);
        let mut n = 0usize;
        for w in &words {
            let c = t.matches(w.as_str()).count();
            if c == 0 {
                n = 0;
                break;
            }
            n += c;
        }
        if n > 0 {
            hits.push((id, n as f32));
        }
    }
    hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    hits.truncate(limit);
    Ok(hits)
}

/// A face as grouping and the views read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaceRow {
    pub id: i64,
    pub file_id: i64,
    pub person: Option<i64>,
    /// Box in fractions of the image.
    pub rect: [f32; 4],
    pub score: f32,
    /// Embedding (f16 bits; empty without one).
    pub vec: Vec<u16>,
    /// The user placed it: grouping never moves it.
    pub locked: bool,
    /// The model key it was found with.
    pub model: String,
    /// Radians, clockwise (see the `roll` column).
    pub roll: f32,
}

fn face_row(r: &rusqlite::Row) -> rusqlite::Result<FaceRow> {
    Ok(FaceRow {
        id: r.get(0)?,
        file_id: r.get(1)?,
        person: r.get(2)?,
        rect: [r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?],
        score: r.get(7)?,
        vec: r
            .get::<_, Option<Vec<u8>>>(8)?
            .map(|b| blob_f16(&b))
            .unwrap_or_default(),
        locked: r.get::<_, i64>(9)? != 0,
        model: r.get(10)?,
        roll: r.get(11)?,
    })
}

const FACE_COLS: &str = "id, file_id, person_id, x, y, w, h, score, vec, locked, model, roll";

/// Every face with an embedding, for grouping.
pub fn all_faces(conn: &Connection) -> rusqlite::Result<Vec<FaceRow>> {
    specular_platform::assert_not_ui_thread("ai::all_faces");
    let mut st = conn.prepare(&format!(
        "SELECT {FACE_COLS} FROM faces WHERE vec IS NOT NULL ORDER BY id"
    ))?;
    let rows = st.query_map([], face_row)?;
    rows.collect()
}

/// The faces of one file, in the order found.
pub fn file_faces(conn: &Connection, file_id: i64) -> rusqlite::Result<Vec<FaceRow>> {
    let mut st = conn.prepare_cached(&format!(
        "SELECT {FACE_COLS} FROM faces WHERE file_id = ?1 ORDER BY id"
    ))?;
    let rows = st.query_map([file_id], face_row)?;
    rows.collect()
}

/// The faces of one person (largest boxes first: the clearest to show).
pub fn person_faces(conn: &Connection, person: i64) -> rusqlite::Result<Vec<FaceRow>> {
    let mut st = conn.prepare_cached(&format!(
        "SELECT {FACE_COLS} FROM faces WHERE person_id = ?1 ORDER BY w * h DESC, id"
    ))?;
    let rows = st.query_map([person], face_row)?;
    rows.collect()
}

/// Files with a face of `person`, in file order.
pub fn person_files(conn: &Connection, person: i64) -> rusqlite::Result<Vec<i64>> {
    let mut st = conn.prepare_cached(
        "SELECT DISTINCT file_id FROM faces WHERE person_id = ?1 ORDER BY file_id",
    )?;
    let rows = st.query_map([person], |r| r.get(0))?;
    rows.collect()
}

/// For every file with a person: their names and `#<id>` (how unnamed people are shown),
/// one per line (for `person:`).
pub fn file_people(conn: &Connection) -> rusqlite::Result<std::collections::HashMap<i64, String>> {
    let mut st = conn.prepare(
        "SELECT DISTINCT f.file_id, p.id, p.name FROM faces f JOIN people p ON p.id = f.person_id
         ORDER BY f.file_id, p.id",
    )?;
    let mut out: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    for row in rows {
        let (f, id, name) = row?;
        let e = out.entry(f).or_default();
        for s in name.into_iter().chain([format!("#{id}")]) {
            if !e.is_empty() {
                e.push('\n');
            }
            e.push_str(&s);
        }
    }
    Ok(out)
}

/// The result of grouping: faces to give to people. `None` as a person asks for a new,
/// unnamed one (one per distinct `new` number).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grouping {
    /// (face, existing person).
    pub to_person: Vec<(i64, i64)>,
    /// (face, new group number).
    pub to_new: Vec<(i64, usize)>,
    /// Faces left with nobody.
    pub to_nobody: Vec<i64>,
}

/// Applies a grouping in one transaction, never touching locked faces; unnamed people left
/// without faces are deleted. Returns the people created.
pub fn apply_grouping(conn: &mut Connection, g: &Grouping) -> rusqlite::Result<Vec<i64>> {
    specular_platform::assert_not_ui_thread("ai::apply_grouping");
    let tx = write_tx(conn)?;
    let mut created = std::collections::BTreeMap::new();
    {
        let mut set =
            tx.prepare_cached("UPDATE faces SET person_id = ?1 WHERE id = ?2 AND locked = 0")?;
        for &(face, person) in &g.to_person {
            set.execute(params![person, face])?;
        }
        for &f in &g.to_nobody {
            set.execute(params![Option::<i64>::None, f])?;
        }
        for &(face, n) in &g.to_new {
            let p = match created.get(&n) {
                Some(&p) => p,
                None => {
                    tx.execute(
                        "INSERT INTO people(name, created) VALUES (NULL, ?1)",
                        [now()],
                    )?;
                    let p = tx.last_insert_rowid();
                    created.insert(n, p);
                    p
                }
            };
            set.execute(params![p, face])?;
        }
    }
    tx.execute(
        "DELETE FROM people WHERE name IS NULL AND id NOT IN
         (SELECT person_id FROM faces WHERE person_id IS NOT NULL)",
        [],
    )?;
    tx.commit()?;
    Ok(created.into_values().collect())
}

/// Names a person (`None` or blank: unnamed again).
pub fn name_person(conn: &Connection, person: i64, name: Option<&str>) -> rusqlite::Result<()> {
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    conn.execute(
        "UPDATE people SET name = ?1 WHERE id = ?2",
        params![name, person],
    )?;
    Ok(())
}

/// Moves every face of `from` to `into` (locked: the user grouped them) and deletes `from`.
/// `into` keeps its name, or takes `from`'s when it has none.
pub fn merge_people(conn: &mut Connection, from: i64, into: i64) -> rusqlite::Result<()> {
    if from == into {
        return Ok(());
    }
    let tx = write_tx(conn)?;
    tx.execute(
        "UPDATE people SET name = COALESCE(name, (SELECT name FROM people WHERE id = ?1))
         WHERE id = ?2",
        params![from, into],
    )?;
    tx.execute(
        "UPDATE faces SET person_id = ?1, locked = 1 WHERE person_id = ?2",
        params![into, from],
    )?;
    tx.execute("DELETE FROM people WHERE id = ?1", [from])?;
    tx.commit()
}

/// Gives one face to a person, or takes it from everyone (`None`: "not this person"); the
/// face is locked so grouping leaves it there.
pub fn place_face(conn: &Connection, face: i64, person: Option<i64>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE faces SET person_id = ?1, locked = 1 WHERE id = ?2",
        params![person, face],
    )?;
    Ok(())
}

/// The catalogue's `info` key of [`face_grouping`].
const FACE_GROUPING: &str = "ai.face_grouping";

/// Whether faces are grouped into people in this library: off until the user turns it on
/// for the library (grouping faces is biometric processing, even on the device).
pub fn face_grouping(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(crate::store::info_get(conn, FACE_GROUPING)?.as_deref() == Some("on"))
}

/// Turns grouping faces into people on or off for this library. Off stops grouping; faces
/// and people already found stay until deleted ([`forget_all_faces`]).
pub fn set_face_grouping(conn: &Connection, on: bool) -> rusqlite::Result<()> {
    crate::store::info_set(conn, FACE_GROUPING, if on { "on" } else { "off" })
}

/// Opens a connection to the catalogue at `path` for the AI pack's own writer (the indexer
/// keeps its own): configured, migrated. A catalogue of a newer schema is refused.
pub fn open(path: &std::path::Path) -> Result<Connection, crate::Error> {
    specular_platform::assert_not_ui_thread("ai::open");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut conn = Connection::open(path)?;
    crate::schema::configure(&conn)?;
    let found = crate::schema::migrate_checked(&mut conn)?;
    if found > crate::schema::VERSION {
        return Err(crate::Error::Newer { found });
    }
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::schema::configure(&c).unwrap();
        crate::schema::migrate(&mut c).unwrap();
        c.execute_batch(
            "INSERT INTO folders(id, path, added) VALUES (1, '/p', 0);
             INSERT INTO dirs(id, folder_id, path) VALUES (1, 1, '/p');
             INSERT INTO files(id, dir_id, name, ext, size, mtime_ns, state) VALUES
                (1, 1, 'a.jpg', 'jpg', 10, 100, 1),
                (2, 1, 'b.jpg', 'jpg', 20, 200, 1),
                (3, 1, 'c.jpg', 'jpg', 30, 300, 2),
                (4, 1, 'd.jpg', 'jpg', 40, 400, 1);
             INSERT INTO thumbs(file_id, content, classes) VALUES (1, 11, 1), (2, 22, 3);",
        )
        .unwrap();
        c
    }

    fn table_exists(c: &Connection, name: &str) -> bool {
        c.query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |_| Ok(()),
        )
        .optional()
        .unwrap()
        .is_some()
    }

    #[test]
    fn face_grouping_is_off_until_turned_on() {
        let c = db();
        assert!(!face_grouping(&c).unwrap());
        set_face_grouping(&c, true).unwrap();
        assert!(face_grouping(&c).unwrap());
        set_face_grouping(&c, false).unwrap();
        assert!(!face_grouping(&c).unwrap());
    }

    #[test]
    fn install_and_drop_each_feature() {
        let mut c = db();
        assert!(installed(&c).unwrap().is_empty());
        for t in FEATURES {
            for table in t.tables {
                assert!(!table_exists(&c, table), "{table} before install");
            }
            assert_eq!(install(&mut c, t.feature).unwrap(), 0);
            assert_eq!(
                install(&mut c, t.feature).unwrap(),
                t.steps.len() as u32,
                "idempotent"
            );
            assert!(has(&c, t.feature).unwrap());
            for table in t.tables {
                assert!(table_exists(&c, table), "{table} after install");
            }
        }
        assert_eq!(installed(&c).unwrap().len(), FEATURES.len());
        drop(&mut c, "people").unwrap();
        for table in ["faces", "people", "face_scans"] {
            assert!(!table_exists(&c, table));
        }
        assert!(table_exists(&c, "embeddings"));
        drop_all(&mut c).unwrap();
        for t in FEATURES {
            for table in t.tables {
                assert!(!table_exists(&c, table));
            }
        }
        assert!(installed(&c).unwrap().is_empty());
        assert!(install(&mut c, "nonsense").is_err());
    }

    #[test]
    fn pending_and_counts_follow_done_rows() {
        let mut c = db();
        install(&mut c, "search").unwrap();
        let p = pending(&c, "search", "m1", 0, 10).unwrap();
        // File 3 is not readable; file 4 has no thumbnail recorded yet.
        assert_eq!(
            p.iter().map(|p| p.file_id).collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
        assert_eq!(p[0].content, Some(11));
        assert_eq!(p[1].classes, 3);
        assert_eq!(p[2].content, None);
        assert_eq!(counts(&c, "search", "m1").unwrap(), (3, 0));
        put_embedding(&c, 1, "m1", &[0x3c00, 0x0000]).unwrap();
        assert_eq!(counts(&c, "search", "m1").unwrap(), (3, 1));
        let p = pending(&c, "search", "m1", 0, 10).unwrap();
        assert_eq!(p.iter().map(|p| p.file_id).collect::<Vec<_>>(), vec![2, 4]);
        // Cursor.
        let p = pending(&c, "search", "m1", 2, 10).unwrap();
        assert_eq!(p.iter().map(|p| p.file_id).collect::<Vec<_>>(), vec![4]);
        // A new model version makes everything pending again.
        assert_eq!(pending(&c, "search", "m2", 0, 10).unwrap().len(), 3);
        assert_eq!(embeddings(&c, "m1", 2).unwrap(), vec![(1, vec![0x3c00, 0])]);
        // A file leaving the catalogue takes its rows.
        c.execute("DELETE FROM files WHERE id = 1", []).unwrap();
        assert!(embeddings(&c, "m1", 2).unwrap().is_empty());
    }

    #[test]
    fn search_v1_rows_survive_v2_and_models_coexist() {
        let mut c = db();
        // A catalogue whose Search tables are at version 1, with a row.
        let tx = c.transaction().unwrap();
        tx.execute_batch(SEARCH_V1).unwrap();
        tx.execute(
            "INSERT INTO ai_schema(feature, version) VALUES ('search', 1)",
            [],
        )
        .unwrap();
        tx.commit().unwrap();
        put_embedding(&c, 1, "old-1", &[0x3c00, 0]).unwrap();
        assert_eq!(install(&mut c, "search").unwrap(), 1);
        assert_eq!(
            embeddings(&c, "old-1", 2).unwrap(),
            vec![(1, vec![0x3c00, 0])]
        );
        // Two models' rows for one file, side by side.
        put_embedding(&c, 1, "small/m-1", &[0, 0x3c00]).unwrap();
        put_embedding(&c, 2, "small/m-1", &[0, 0x3c00]).unwrap();
        assert_eq!(counts(&c, "search", "old-1").unwrap(), (3, 1));
        assert_eq!(counts(&c, "search", "small/m-1").unwrap(), (3, 2));
        let (rows, last) = embeddings_since(&c, "small/m-1", 2, 0).unwrap();
        assert_eq!(rows.len(), 2);
        let (rows, last2) = embeddings_since(&c, "small/m-1", 2, last).unwrap();
        assert!(rows.is_empty());
        assert_eq!(last2, last);
        // Embedded again: shows up again after the cursor.
        put_embedding(&c, 2, "small/m-1", &[0x3c00, 0]).unwrap();
        let (rows, _) = embeddings_since(&c, "small/m-1", 2, last).unwrap();
        assert_eq!(rows, vec![(2, vec![0x3c00, 0])]);
        assert_eq!(embedding_count(&c, "small/m-1").unwrap(), 2);
        assert_eq!(prune_embeddings(&c, &["small/m-1", "it's"]).unwrap(), 1);
        assert!(embeddings(&c, "old-1", 2).unwrap().is_empty());
        assert_eq!(embedding_count(&c, "small/m-1").unwrap(), 2);
        // Files leaving the catalogue still take their rows.
        c.execute("DELETE FROM files WHERE id = 2", []).unwrap();
        assert_eq!(embedding_count(&c, "small/m-1").unwrap(), 1);
    }

    #[test]
    fn text_and_scores() {
        let mut c = db();
        install(&mut c, "text").unwrap();
        install(&mut c, "culling").unwrap();
        put_text(&c, 2, "ocr", "EXIT", "[]", "latn").unwrap();
        put_scores(
            &c,
            2,
            "cull",
            &Scores {
                sharpness: Some(0.5),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(counts(&c, "text", "ocr").unwrap(), (3, 1));
        // A new models' key keeps the readings `still_good` approves, the rest pend again.
        put_text(&c, 1, "ocr", "خروج", "[]", "arab").unwrap();
        let kept = rekey_text(&mut c, "ocr+ar", &|old, t| {
            old == "ocr" && !t.contains("arab")
        })
        .unwrap();
        assert_eq!(kept, 1);
        assert_eq!(counts(&c, "text", "ocr+ar").unwrap(), (3, 1));
        let p: Vec<i64> = pending(&c, "text", "ocr+ar", 0, 10)
            .unwrap()
            .iter()
            .map(|p| p.file_id)
            .collect();
        assert_eq!(p, [1, 4]);
        assert_eq!(counts(&c, "culling", "cull").unwrap(), (3, 1));
    }

    #[test]
    fn forget_one_person_and_all_faces() {
        let mut c = db();
        install(&mut c, "people").unwrap();
        let face = |x| Face {
            x,
            y: 0.1,
            w: 0.2,
            h: 0.2,
            score: 0.9,
            vec: vec![0x3c00; 4],
            roll: x - 0.1,
        };
        put_faces(&mut c, 1, "det", &[face(0.1), face(0.5)]).unwrap();
        // The roll is kept.
        let rolls: Vec<f32> = file_faces(&c, 1).unwrap().iter().map(|f| f.roll).collect();
        assert!(
            (rolls[0] - 0.0).abs() < 1e-6 && (rolls[1] - 0.4).abs() < 1e-6,
            "{rolls:?}"
        );
        put_faces(&mut c, 2, "det", &[face(0.3)]).unwrap();
        put_faces(&mut c, 4, "det", &[]).unwrap();
        let ana = add_person(&c, Some("Ana")).unwrap();
        let ben = add_person(&c, None).unwrap();
        let f1 = faces_of(&c, 1).unwrap();
        let f2 = faces_of(&c, 2).unwrap();
        assign_faces(&mut c, Some(ana), &[f1[0], f2[0]]).unwrap();
        assign_faces(&mut c, Some(ben), &[f1[1]]).unwrap();
        let p = people(&c).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(
            (p[0].name.as_deref(), p[0].faces),
            (Some("Ana"), 2),
            "named first"
        );
        assert_eq!(counts(&c, "people", "det").unwrap(), (3, 3));

        assert_eq!(forget_person(&mut c, ana).unwrap(), 2);
        let p = people(&c).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].id, ben);
        assert_eq!(faces_of(&c, 2).unwrap(), Vec::<i64>::new());
        // Still scanned: not found again.
        assert!(pending(&c, "people", "det", 0, 10).unwrap().is_empty());

        forget_all_faces(&mut c).unwrap();
        assert!(people(&c).unwrap().is_empty());
        assert!(faces_of(&c, 1).unwrap().is_empty());
        assert_eq!(counts(&c, "people", "det").unwrap(), (3, 0));
        assert!(has(&c, "people").unwrap());
    }

    #[test]
    fn grouping_naming_merging_and_placing_faces() {
        let mut c = db();
        install(&mut c, "people").unwrap();
        let face = |x| Face {
            x,
            y: 0.1,
            w: 0.2,
            h: 0.2,
            score: 0.9,
            vec: vec![0x3c00; 4],
            roll: x - 0.1,
        };
        put_faces(&mut c, 1, "det", &[face(0.1), face(0.5)]).unwrap();
        put_faces(&mut c, 2, "det", &[face(0.3)]).unwrap();
        let all = all_faces(&c).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!((all[0].model.as_str(), all[0].locked), ("det", false));
        let ids: Vec<i64> = all.iter().map(|f| f.id).collect();
        // Two new groups; nobody for the third.
        let made = apply_grouping(
            &mut c,
            &Grouping {
                to_person: vec![],
                to_new: vec![(ids[0], 0), (ids[2], 0)],
                to_nobody: vec![ids[1]],
            },
        )
        .unwrap();
        assert_eq!(made.len(), 1);
        let p = made[0];
        assert_eq!(person_files(&c, p).unwrap(), [1, 2]);
        let fp = file_people(&c).unwrap();
        assert_eq!(fp[&1], format!("#{p}"));
        name_person(&c, p, Some(" Ana ")).unwrap();
        assert_eq!(file_people(&c).unwrap()[&2], format!("Ana\n#{p}"));
        assert_eq!(people(&c).unwrap()[0].cover, Some(ids[0]));
        // Merge another person into Ana: their faces are locked to her.
        let ben = add_person(&c, None).unwrap();
        assign_faces(&mut c, Some(ben), &[ids[1]]).unwrap();
        merge_people(&mut c, ben, p).unwrap();
        assert_eq!(person_faces(&c, p).unwrap().len(), 3);
        assert!(file_faces(&c, 1).unwrap()[1].locked);
        // "Not this person": locked to nobody; grouping leaves it.
        place_face(&c, ids[2], None).unwrap();
        apply_grouping(
            &mut c,
            &Grouping {
                to_person: vec![(ids[2], p)],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(file_faces(&c, 2).unwrap()[0].person, None);
        // An unnamed person left without faces goes.
        let empty = add_person(&c, None).unwrap();
        apply_grouping(&mut c, &Grouping::default()).unwrap();
        assert!(people(&c).unwrap().iter().all(|x| x.id != empty));
    }

    #[test]
    fn culling_scores_and_faces_round_trip() {
        let mut c = db();
        install(&mut c, "culling").unwrap();
        let s = Scores {
            sharpness: Some(0.5),
            eyes_open: Some(0.25),
            eye_focus: Some(0.7),
            expression: None,
            exposure: Some(0.4),
            clipped: Some(0.02),
            faces: vec![
                EyeFace {
                    x: 0.1,
                    y: 0.2,
                    w: 0.3,
                    h: 0.4,
                    closed: 0.75,
                },
                EyeFace::default(),
            ],
        };
        put_scores(&c, 1, "cull", &s).unwrap();
        assert_eq!(scores(&c, 1).unwrap(), Some(s));
        assert_eq!(scores(&c, 2).unwrap(), None);
        put_scores(&c, 2, "cull", &Scores::default()).unwrap();
        assert_eq!(scores(&c, 2).unwrap(), Some(Scores::default()));
    }

    #[test]
    fn blobs_round_trip() {
        let v = [0u16, 1, 0x3c00, 0xfbff, 0x8000];
        assert_eq!(blob_f16(&f16_blob(&v)), v);
    }
}

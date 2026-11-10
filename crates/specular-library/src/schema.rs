//! The catalogue's schema, as versioned migrations applied in order on open.
//!
//! `PRAGMA user_version` holds the number of migrations applied. A migration is never edited
//! once released: a change is a new entry at the end of [`MIGRATIONS`], so a catalogue of any
//! age is brought forward step by step, each step in its own transaction. Later lanes append
//! theirs (DESIGN §10 "Storage"): `collections`, the `dicom_*` tables and `pacs_nodes`,
//! `image_hashes` and `match_groups`, and the AI pack's `embeddings`, `faces`, `ocr_text`
//! (which the pack's removal drops again with a migration of its own). Tables of a feature
//! reference `files(id)` with `ON DELETE CASCADE`, so a file leaving the catalogue takes its
//! rows everywhere with it.
//!
//! A catalogue written by a newer Specular (more migrations than this build knows) is opened
//! read-only for search and never written: an older build cannot know what the newer tables
//! promise.

use rusqlite::Connection;

/// One step of the schema.
pub struct Migration {
    /// What it adds, for the log.
    pub what: &'static str,
    pub sql: &'static str,
}

/// Every migration, oldest first; the schema version is the number applied.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        what: "locations, folders, files, thumbnail references, tags, full-text index",
        sql: V1,
    },
    Migration {
        what: "register of the Local AI pack's feature tables",
        sql: V2,
    },
    Migration {
        what: "colour space of each file",
        sql: V3,
    },
    Migration {
        what: "image signatures, duplicate and similar-shot groups",
        sql: V4,
    },
    Migration {
        what: "positions and place names of files with GPS",
        sql: V5,
    },
    Migration {
        what: "full-text rows of new files written with their facts",
        sql: V6,
    },
    Migration {
        what: "duplicate and similar-shot links kept across runs",
        sql: V7,
    },
    Migration {
        what: "astronomy subframes' header facts and star metrics",
        sql: V8,
    },
    Migration {
        what: "Develop edits and their history",
        sql: V9,
    },
    Migration {
        what: "words of files for text search (lyrics)",
        sql: V10,
    },
    Migration {
        what: "names of Develop versions",
        sql: V11,
    },
    Migration {
        what: "lyrics sidecars' modification times",
        sql: V12,
    },
    Migration {
        what: "modules and styles of Develop edits, for search",
        sql: V13,
    },
    Migration {
        what: "albums and smart albums",
        sql: V14,
    },
    Migration {
        what: "Exif fields and tags for search",
        sql: V15,
    },
    Migration {
        what: "IPTC fields for search",
        sql: V16,
    },
    Migration {
        what: "other apps' sidecars: their values, names and time stamps",
        sql: V17,
    },
    Migration {
        what: "local copies for offline editing, and changes waiting for their originals",
        sql: V18,
    },
    Migration {
        what: "indexes of files whose Exif or IPTC fields are still to read",
        sql: V19,
    },
];

/// The schema version this build writes.
pub const VERSION: u32 = MIGRATIONS.len() as u32;

const V1: &str = r#"
-- Watched locations (DESIGN §10 "folders"): roots indexed where they are.
CREATE TABLE folders (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL UNIQUE,
    added     INTEGER NOT NULL,          -- seconds since the epoch
    -- 0 online, 1 offline (the root is missing or another volume is mounted there).
    offline   INTEGER NOT NULL DEFAULT 0,
    -- The root's device when last seen: an empty mount point is told from the volume.
    dev       INTEGER,
    last_scan INTEGER                    -- seconds; NULL until the first walk finishes
);

-- Every directory below a location (the location's root included). Files refer to their
-- directory, so a path is stored once per folder, and `in:` searches folders, not files.
CREATE TABLE dirs (
    id        INTEGER PRIMARY KEY,
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    -- The directory it is in (NULL for a location's root): a vanished directory takes its
    -- subtree with it.
    parent    INTEGER REFERENCES dirs(id) ON DELETE CASCADE,
    path      TEXT NOT NULL UNIQUE,
    -- Directory modification time when last listed (ns).
    mtime_ns  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX dirs_folder ON dirs(folder_id);
CREATE INDEX dirs_parent ON dirs(parent);

CREATE TABLE files (
    id        INTEGER PRIMARY KEY,
    dir_id    INTEGER NOT NULL REFERENCES dirs(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    ext       TEXT NOT NULL,             -- canonical, lower case, no dot ("jpg")
    size      INTEGER NOT NULL,
    mtime_ns  INTEGER NOT NULL,
    -- File identity (Unix device and inode; NULL where the platform gives none), what a move
    -- or rename is recognised by.
    dev       INTEGER,
    inode     INTEGER,
    -- 0 listed, facts not read yet; 1 facts read; 2 not readable as an image.
    state     INTEGER NOT NULL DEFAULT 0,
    -- Facts (specular_thumbs::Meta), NULL until read.
    format    TEXT,
    width     INTEGER,
    height    INTEGER,
    taken     INTEGER,                   -- capture time, local seconds
    camera    TEXT,
    lens      TEXT,
    iso       INTEGER,
    f_number  REAL,
    focal     REAL,
    exposure  REAL,
    pages     INTEGER,
    bits      INTEGER,
    -- 1 animated, 2 HDR, 4 alpha, 8 RAW, 16 ICC profile, 32 position looked for, 64 position.
    props     INTEGER NOT NULL DEFAULT 0,
    -- Culling marks: the gallery's, mirrored, or a sidecar's when it changes.
    rating    INTEGER NOT NULL DEFAULT 0,
    label     INTEGER NOT NULL DEFAULT 0,
    flag      INTEGER NOT NULL DEFAULT 0,
    -- The XMP sidecar's name and modification time when read (a newer one is read again).
    xmp       TEXT,
    xmp_mtime INTEGER,
    UNIQUE (dir_id, name)
);
CREATE INDEX files_identity ON files(dev, inode);
CREATE INDEX files_state ON files(state);
CREATE INDEX files_taken ON files(taken);
CREATE INDEX files_rating ON files(rating);
CREATE INDEX files_ext ON files(ext);

-- References into the thumbnail cache (specular-thumbs' DiskCache), which keeps the pixels:
-- the partial content hash that, with the file's size and time stamp, makes its ThumbKey.
-- With it a tile finds its cached thumbnail without reading the file, also while the
-- location is offline, and a moved file is recognised by its content.
CREATE TABLE thumbs (
    file_id   INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    content   INTEGER NOT NULL,
    -- Size classes found in the cache when the facts were read (bit n: CLASSES[n]).
    classes   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX thumbs_content ON thumbs(content);

CREATE TABLE tags (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    -- For hierarchical keywords (lr:hierarchicalSubject) later.
    parent    INTEGER REFERENCES tags(id) ON DELETE SET NULL
);

CREATE TABLE file_tags (
    file_id   INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    -- 0 from the file's own XMP, 1 from its sidecar, 2 added in Specular.
    source    INTEGER NOT NULL,
    PRIMARY KEY (file_id, tag_id)
) WITHOUT ROWID;
CREATE INDEX file_tags_tag ON file_tags(tag_id);

-- Full text: the folded name, camera, lens and tags of each file (rowid = files.id).
-- Trigram tokens make every substring of three or more characters searchable, as the
-- gallery's filter matches; the text is folded (case, accents) before it goes in, so the
-- index agrees with the query's own folding.
CREATE VIRTUAL TABLE fts_index USING fts5(
    name, camera, lens, tags,
    content = '', contentless_delete = 1, tokenize = 'trigram'
);

-- Full text of folder paths below each location (rowid = dirs.id): `in:` and plain words
-- match a folder once rather than every file in it.
CREATE VIRTUAL TABLE fts_dirs USING fts5(
    path,
    content = '', contentless_delete = 1, tokenize = 'trigram'
);

-- Full-text rows leave with their file or folder, also when a cascade removes it.
CREATE TRIGGER files_fts_delete AFTER DELETE ON files BEGIN
    DELETE FROM fts_index WHERE rowid = old.id;
END;
CREATE TRIGGER dirs_fts_delete AFTER DELETE ON dirs BEGIN
    DELETE FROM fts_dirs WHERE rowid = old.id;
END;

-- Small facts about the catalogue itself.
CREATE TABLE info (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// The AI pack's features keep their tables out of the core migrations: they are created
/// when a feature is installed and dropped when it is removed ([`crate::ai`]). The core only
/// registers which exist, at which version of the feature's own schema.
const V2: &str = r#"
CREATE TABLE ai_schema (
    feature TEXT PRIMARY KEY,
    version INTEGER NOT NULL
);
"#;

/// The colour space (`specular_thumbs::Meta::space`): NULL until looked for, empty when nothing
/// names one. Files whose facts were read before it was kept are read again by the facts pass
/// (after the unread ones), found through the partial index.
const V3: &str = r#"
ALTER TABLE files ADD COLUMN space TEXT;
CREATE INDEX files_space_unread ON files(id) WHERE state = 1 AND space IS NULL;
"#;

/// Duplicates and similar shots across the catalogue ([`crate::matches`]).
///
/// `image_hashes` keeps each image's pixel signature (`specular_similar::Signature`, the
/// bytes the gallery's scan stores with its thumbnails), valid while the file's size and time
/// stamp are the ones it was made from and `version` is the signature's
/// (`specular_similar::scan::SIG_DIGEST_ID`); `sig` is NULL when the pixels could not be read
/// (tried again once the file changes). `seq` orders the rows written, so the matcher takes in
/// only what is new. `blake3` is the whole file's hash, read only when another file has the
/// same size and partial content hash.
///
/// `match_groups` holds the roles a search matches with `dupes:` and `similar:`: the group
/// (the smallest file id among its members) and whether the suggestion keeps the file.
/// `match_verify` remembers what checking a duplicate candidate found (SSIM of the two
/// thumbnails, or 1/0 for byte-identical or not), so a pair is never checked twice.
const V4: &str = r#"
CREATE TABLE image_hashes (
    file_id   INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    size      INTEGER NOT NULL,
    mtime_ns  INTEGER NOT NULL,
    version   INTEGER NOT NULL,
    sig       BLOB,
    blake3    BLOB,
    seq       INTEGER NOT NULL
);
CREATE INDEX image_hashes_seq ON image_hashes(seq);

CREATE TABLE match_groups (
    file_id   INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    -- 0 duplicates, 1 similar shots.
    cat       INTEGER NOT NULL,
    grp       INTEGER NOT NULL,
    keep      INTEGER NOT NULL,
    PRIMARY KEY (file_id, cat)
) WITHOUT ROWID;
CREATE INDEX match_groups_grp ON match_groups(cat, grp);

CREATE TABLE match_verify (
    a         INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    b         INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    -- specular_similar::Tier: 0 bytes, 1 pixels, 2 visual.
    tier      INTEGER NOT NULL,
    score     REAL NOT NULL,
    PRIMARY KEY (a, b, tier)
) WITHOUT ROWID;
CREATE INDEX match_verify_b ON match_verify(b);
"#;

/// Where each photo was taken (DESIGN §6.7 "Places from GPS", §10 "Map view").
///
/// `lat`/`lon` are the GPS position in degrees (south and west negative); `geo` is NULL until
/// looked for, 0 when the file has no position, 1 when it has one. `place_id` is the nearest
/// populated place found by offline reverse geocoding (GeoNames), shared by every photo taken
/// near it: the `places` table stays small (one row per town), so a search matches place names
/// in memory, in the viewer's language, and selects files by `place_id`.
///
/// Files whose facts were read before this existed and that have no position are marked at
/// once; the facts pass reads the rest again (after the unread ones), found through the
/// partial index.
const V5: &str = r#"
CREATE TABLE places (
    id        INTEGER PRIMARY KEY,
    city      TEXT NOT NULL,
    admin1    TEXT NOT NULL,
    admin2    TEXT NOT NULL,
    -- ISO 3166-1 alpha-2; the country's name is the viewer's language's, not stored.
    cc        TEXT NOT NULL,
    UNIQUE (city, admin1, admin2, cc)
);
ALTER TABLE files ADD COLUMN lat REAL;
ALTER TABLE files ADD COLUMN lon REAL;
ALTER TABLE files ADD COLUMN geo INTEGER;
ALTER TABLE files ADD COLUMN place_id INTEGER REFERENCES places(id) ON DELETE SET NULL;
UPDATE files SET geo = 0 WHERE state = 1 AND (props & 32) != 0 AND (props & 64) = 0;
CREATE INDEX files_geo_unread ON files(id) WHERE state = 1 AND geo IS NULL;
CREATE INDEX files_place ON files(place_id) WHERE place_id IS NOT NULL;
CREATE INDEX files_geo ON files(id) WHERE geo = 1;
"#;

/// A new file's full-text row is written once, by the facts pass, rather than at the walk
/// (its name) and again with its facts: the index's delete markers made the second write
/// cost more as the catalogue grew (a million files: half the indexer's time in the facts
/// pass). `fts` is 0 while a file has no row yet (only before its facts are read); searches
/// let those files through to the matcher (`files_fts_pending`), and the delete trigger
/// leaves the index alone for them.
const V6: &str = r#"
ALTER TABLE files ADD COLUMN fts INTEGER NOT NULL DEFAULT 1;
CREATE INDEX files_fts_pending ON files(id) WHERE fts = 0;
DROP TRIGGER files_fts_delete;
CREATE TRIGGER files_fts_delete AFTER DELETE ON files WHEN old.fts = 1 BEGIN
    DELETE FROM fts_index WHERE rowid = old.id;
END;
"#;

/// The duplicate and similar-shot matcher's state, kept so that a restart does no matching
/// ([`crate::matches`]).
///
/// `match_links` holds the links between files (`specular_similar::live::Link`), each once,
/// in the row of its smaller file id: `links` is a run of varints, each the step from the
/// previous file id (from the row's own) shifted left by two with the link's kind in the low
/// bits. No foreign key: a row of a file that left is found and dropped when the links are
/// next loaded, and the files it linked are grouped again. `match_mark_log` and
/// `match_rename_log` name the files whose marks or names changed since the groups were last
/// written (the indexer's triggers fill them), so a restart forms only their groups again.
/// `info.match_state` names the rules and the last signature (`image_hashes.seq`) the stored
/// links, logs and `match_groups` agree with.
/// Astronomy subframes (FITS, XISF): `astro` holds their header facts and star metrics
/// (`specular_thumbs::AstroMeta::to_bytes`); NULL until looked for, empty for a file that is
/// none. FITS and XISF files read before are read again.
const V8: &str = r#"
ALTER TABLE files ADD COLUMN astro BLOB;
CREATE INDEX files_astro_unread ON files(id)
    WHERE state = 1 AND astro IS NULL AND format IN ('FITS', 'XISF');
"#;

/// Words a file holds beyond its name and metadata, for plain-word search: a song's lyrics
/// (`specular_decode::Session::search_text`). Searched by the matcher only (a file with a
/// row is always a candidate for a plain word), so the row is the text as read. Sound files
/// read before are read again for theirs.
const V10: &str = r#"
CREATE TABLE file_text (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    text    TEXT NOT NULL
);
UPDATE files SET state = 0 WHERE state = 1 AND (props & 1536) = 1536;
"#;

/// Develop edits ([`crate::edits`]): the current edit of each version of a file
/// (`specular_develop`'s codec blob and its output hash) and its history steps in order.
/// Versions (virtual copies) number from 0.
const V9: &str = r#"
CREATE TABLE edits (
    file_id   INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    version   INTEGER NOT NULL,
    edit      BLOB NOT NULL,
    hash      BLOB NOT NULL,
    position  INTEGER NOT NULL DEFAULT 0,   -- history steps applied
    updated   INTEGER NOT NULL,             -- seconds since the epoch
    PRIMARY KEY (file_id, version)
) WITHOUT ROWID;
CREATE TABLE edit_steps (
    file_id   INTEGER NOT NULL,
    version   INTEGER NOT NULL,
    seq       INTEGER NOT NULL,
    step      BLOB NOT NULL,
    PRIMARY KEY (file_id, version, seq),
    FOREIGN KEY (file_id, version) REFERENCES edits(file_id, version) ON DELETE CASCADE
) WITHOUT ROWID;
"#;

/// A name for each Develop version (virtual copy); none for the first by default.
const V11: &str = r#"
ALTER TABLE edits ADD COLUMN name TEXT;
"#;

/// The modification time of the `.lrc` lyrics beside a sound file when its facts were read
/// (NULL: none), so a new or changed one has its words read again.
const V12: &str = r#"
ALTER TABLE files ADD COLUMN lrc_mtime INTEGER;
"#;

/// What each Develop edit uses, for `module:` and `style:` searches: the modules' stable
/// ids and the styles applied, one per line (NULL for edits saved before: not known until
/// saved again).
const V13: &str = r#"
ALTER TABLE edits ADD COLUMN modules TEXT;
ALTER TABLE edits ADD COLUMN styles TEXT;
"#;

/// Albums (DESIGN §10 "collections"): manual albums hold files; smart albums hold rules
/// (`specular_query::rules`, stored as text). The name is the full path, parts joined by
/// `/` (`Client X/selects`): groups are the names' leading parts, not rows. A file leaving
/// the catalogue leaves its albums.
const V14: &str = r#"
CREATE TABLE collections (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL COLLATE NOCASE UNIQUE,
    kind     INTEGER NOT NULL,
    rules    TEXT,
    created  INTEGER NOT NULL
);
CREATE TABLE collection_files (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    file_id       INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    added         INTEGER NOT NULL,
    PRIMARY KEY (collection_id, file_id)
) WITHOUT ROWID;
CREATE INDEX collection_files_file ON collection_files(file_id);
"#;

/// The Exif fields search names (`specular_thumbs::ExifMeta::to_bytes`) and every tag
/// `exif:` names, one `key=value` per line (`specular_query::exif`). NULL until read: files
/// read before are read again in the background, their other facts kept meanwhile.
/// The IPTC fields search names (`specular_metadata::edit::Record::to_lines`: the XMP
/// sidecar's, else the file's XMP, else its IIM). NULL until read: files read before are read
/// again in the background, their other facts kept meanwhile.
const V16: &str = r#"
ALTER TABLE files ADD COLUMN iptc TEXT;
"#;

const V15: &str = r#"
ALTER TABLE files ADD COLUMN exif BLOB;
ALTER TABLE files ADD COLUMN exif_tags TEXT;
"#;

/// Other apps' sidecars beside the images (`specular_metadata::sidecars`): darktable, Lightroom
/// and the other Adobe apps, Capture One, digiKam, RawTherapee. Specular writes only marks into them.
/// - `files.sides`: the names and time stamps of every sidecar of the file as the walk listed
///   them (`name \t mtime_ns` lines; NULL: none), so a change reads the file again.
/// - `files.foreign_bits`: which of the file's values came from another app's sidecar (1
///   rating, 2 label, 4 flag): those follow the sidecar; the others are the user's and win.
/// - `foreign_sides`: what each of them said, as last read; `duplicate` is the darktable
///   duplicate it describes (`IMG_01.CR2.xmp`: 1), 0 for the photo itself; keywords one per
///   line.
///
/// Marks read from the matched XMP sidecar before this are not known to be the user's: they
/// are marked as from a sidecar, and every matched sidecar is read again once.
const V17: &str = r#"
ALTER TABLE files ADD COLUMN sides TEXT;
ALTER TABLE files ADD COLUMN foreign_bits INTEGER NOT NULL DEFAULT 0;
UPDATE files SET foreign_bits = 7, xmp_mtime = NULL WHERE xmp IS NOT NULL;
CREATE TABLE foreign_sides (
    file_id      INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    app          TEXT NOT NULL,
    duplicate    INTEGER NOT NULL,
    mtime_ns     INTEGER NOT NULL,
    rating       INTEGER,
    label        INTEGER,
    flag         INTEGER,
    keywords     TEXT,
    title        TEXT,
    caption      TEXT,
    version_name TEXT,
    PRIMARY KEY (file_id, name)
) WITHOUT ROWID;
"#;

/// Local copies for offline editing ([`crate::local`]). A copy (the original, or a compact
/// proxy of a raw) lives under the app's data folder, never beside the original, and is
/// held by the scopes the user asked for (a location, an album, a selection): it goes when
/// no scope holds it. Changes made while the original is offline wait in `local_changes`
/// against the original's identity (path, size, time stamp, partial content hash) and its
/// sidecars as they were, until they are written to the original's sidecars or dropped.
/// They do not reference `files`: they outlive the file's row (the user decides).
const V18: &str = r#"
CREATE TABLE local_scopes (
    id       INTEGER PRIMARY KEY,
    kind     INTEGER NOT NULL,          -- 0 location, 1 album, 2 selection
    target   INTEGER,                   -- folders.id or collections.id; NULL for a selection
    name     TEXT NOT NULL,             -- what the user knows it by
    mode     INTEGER NOT NULL,          -- 0 originals, 1 proxies of raws
    created  INTEGER NOT NULL
);
CREATE TABLE local_copies (
    file_id  INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    kind     INTEGER NOT NULL,          -- 0 the original, 1 a proxy
    local    TEXT NOT NULL,             -- relative to the store
    bytes    INTEGER NOT NULL,
    -- The original when copied: size, time stamp (ns), partial content hash.
    size     INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    content  INTEGER NOT NULL,
    -- Its sidecars when copied: `name TAB mtime_ns TAB hash` lines.
    sidecars TEXT NOT NULL,
    made     INTEGER NOT NULL
);
CREATE TABLE local_scope_files (
    scope_id INTEGER NOT NULL REFERENCES local_scopes(id) ON DELETE CASCADE,
    file_id  INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    PRIMARY KEY (scope_id, file_id)
) WITHOUT ROWID;
CREATE INDEX local_scope_files_file ON local_scope_files(file_id);
CREATE TABLE local_changes (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL,
    size      INTEGER NOT NULL,
    mtime_ns  INTEGER NOT NULL,
    content   INTEGER NOT NULL,
    -- The sidecars the change was made against (as `local_copies.sidecars`); NULL when only
    -- the catalogue's sidecar time stamp was known (`xmp_mtime`).
    sidecars  TEXT,
    xmp_mtime INTEGER,
    kind      INTEGER NOT NULL,         -- 0 marks, 1 keywords, 2 IPTC fields, 3 a Develop edit
    version   INTEGER NOT NULL DEFAULT 0,
    payload   BLOB NOT NULL,
    queued    INTEGER NOT NULL,
    UNIQUE (path, kind, version)
);
"#;

/// The files read before their Exif (V15) or IPTC (V16) fields were kept, found through
/// these partial indexes as the colour space's and the position's are: without them the
/// facts pass walked every file read for each batch.
const V19: &str = r#"
CREATE INDEX files_exif_unread ON files(id) WHERE state = 1 AND exif IS NULL;
CREATE INDEX files_iptc_unread ON files(id) WHERE state = 1 AND iptc IS NULL;
"#;

const V7: &str = r#"
CREATE TABLE match_links (
    file_id   INTEGER PRIMARY KEY,
    links     BLOB NOT NULL
);
CREATE TABLE match_mark_log (id INTEGER PRIMARY KEY);
CREATE TABLE match_rename_log (id INTEGER PRIMARY KEY);
"#;

/// Brings `conn` up to [`VERSION`]. Returns the version found before.
pub fn migrate(conn: &mut Connection) -> rusqlite::Result<u32> {
    specular_platform::assert_not_ui_thread("catalogue migration");
    let found: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, m) in MIGRATIONS.iter().enumerate().skip(found as usize) {
        // Under the write lock, the version again: another connection (the Library's and the
        // AI pack's opening at once) may have run this step while this one waited for it.
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if now > i as u32 {
            continue;
        }
        tx.execute_batch(m.sql)?;
        tx.pragma_update(None, "user_version", i as u32 + 1)?;
        tx.commit()?;
        specular_log::info!(target: specular_log::FOLDER, "library: schema {} ({})", i + 1, m.what);
    }
    Ok(found)
}

/// Like [`migrate`], but leaves a catalogue of a newer schema as it is. Returns the version
/// found (greater than [`VERSION`] for a newer catalogue).
pub fn migrate_checked(conn: &mut Connection) -> rusqlite::Result<u32> {
    let found: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if found > VERSION {
        return Ok(found);
    }
    migrate(conn)?;
    Ok(found)
}

/// Connection settings for every connection: WAL (readers never wait for the indexer),
/// foreign keys, and a busy timeout for the rare moment two writers meet.
///
/// Every connection to the catalogue passes here: never on the UI thread (debug builds panic).
pub fn configure(conn: &Connection) -> rusqlite::Result<()> {
    specular_platform::assert_not_ui_thread("catalogue connection");
    // First: switching a new catalogue to WAL takes a lock another connection may hold.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(())
}

/// The search connection reads the whole `files` table for a catalogue-wide query; SQLite's
/// default 2 MB page cache thrashed (page allocation and its mutexes were a fifth of a
/// 100k-file search). Memory-map the file (pages shared with the OS cache, not copied) and
/// give the cache room for the hot tables.
pub fn configure_reader(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "mmap_size", 256i64 << 20)?;
    conn.pragma_update(None, "cache_size", -16384i64)?;
    Ok(())
}

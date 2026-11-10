//! Catalogue-wide search with the gallery's query language.
//!
//! The query is parsed in the Library's scope (`tag:` and `in:` are allowed) and split in two:
//! what SQL can decide is pushed into the `WHERE` clause, so only candidates leave SQLite,
//! and every candidate is then checked with the very [`Matcher`] the gallery uses, so the
//! results are exactly the gallery's for the same facts. A pushed-down condition only ever
//! lets more through than the term (it is a superset), never fewer:
//!
//! * words of three characters or more go to the FTS5 trigram index (name, camera, lens, tags,
//!   and the folder's path), as do `name:`, `camera:`, `lens:`, `tag:` and `in:`; words and
//!   `place:` also select the files of the places they name ([`crate::places`]);
//! * formats, ratings, labels, sizes and modification dates, both ways (`-ext:png` too);
//! * the metadata numbers, capture dates, orientations and `is:`/`has:` flags as a match or
//!   "facts not read yet" (which the gallery's tri-state lets through as unknown).
//!
//! While the indexer rebuilds the full-text index for another fold, text is not pushed down.
//!
//! Negated text, `dupes:`/`similar:` (each file's role in its group, from `match_groups`;
//! see [`crate::matches`]) and words shorter than a trigram are left to the matcher. With facets on, a facet's own terms are not pushed down either: the counts of a
//! facet are of the files matching everything else.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use rusqlite::types::Value;
use specular_query::facet::{Facet, FacetField, Tally};
use specular_query::parse::Range;
use specular_query::{
    Facts, Field, Flag, Fold, Kind, Matcher, Orient, Pred, Query, Scope, Term, parse_in,
};
use specular_thumbs::{Meta, Stamp, ThumbKey};

use crate::{Error, Mark};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    /// Capture time, else modification time.
    Date,
    Modified,
    Size,
    Rating,
    /// Folder path, then name.
    Folder,
    /// No order (the catalogue's): for a caller that orders every row itself, as the grid
    /// does. SQLite's sort was most of a million-row search (about 1.2 of 1.5 s of SQL);
    /// indexes per sort key made it slower still (each row then a random table read) and
    /// the catalogue 18 % larger.
    Unordered,
}

/// A search of the catalogue.
#[derive(Clone, Debug, Default)]
pub struct Search {
    /// In the gallery's query language.
    pub query: String,
    pub sort: Sort,
    pub descending: bool,
    /// At most this many rows (all that match are still counted).
    pub limit: Option<usize>,
    /// Count the facets of the matches.
    pub facets: bool,
    /// Only this location.
    pub location: Option<i64>,
    /// Push nothing down (checks the pushdown: the results must not change).
    pub no_pushdown: bool,
    /// The viewer's language: countries are found by their names in it as well as in
    /// English.
    pub language: Option<specular_metadata::LanguageIdentifier>,
    /// A search by description (the AI pack's Search, `like:`): only these files, best
    /// first, in this order instead of `sort` (file id, score). The query's other terms
    /// still narrow them.
    pub ranked: Option<std::sync::Arc<Vec<(i64, f32)>>>,
    /// Only the files of this manual album ([`crate::albums`]).
    pub album: Option<i64>,
    /// Only the files matching these rules (a smart album's); the query narrows them
    /// further.
    pub rules: Option<specular_query::rules::Rules>,
    /// Count the matches per day for the date timeline ([`Found::timeline`]).
    pub timeline: bool,
}

/// One catalogued image.
#[derive(Clone, Debug)]
pub struct Row {
    pub id: i64,
    pub path: PathBuf,
    pub name: String,
    /// The folder it is in.
    pub folder: String,
    /// Canonical extension.
    pub ext: String,
    pub size: u64,
    pub mtime_ns: i64,
    /// 0 listed, facts not read yet; 1 facts read; 2 not readable as an image.
    pub state: u8,
    /// Its facts, once read.
    pub meta: Option<Meta>,
    pub mark: Mark,
    /// Keywords, one per line.
    pub tags: String,
    /// The thumbnail cache's content hash and the size classes it held when last looked.
    pub content: Option<u64>,
    pub classes: u32,
    /// In an offline location.
    pub offline: bool,
    /// In a group of exact duplicates: Some(kept, as suggested).
    pub dupes: Option<bool>,
    /// In a group of similar shots: Some(kept, as suggested).
    pub similar: Option<bool>,
    /// Where it was taken, one name per line ([`specular_query::Facts::place`]): empty
    /// without a position, None until looked for.
    pub place: Option<String>,
    /// The named people in it, one per line: filled for a `person:` search when People is
    /// installed (empty: nobody named), else None.
    pub people: Option<String>,
    /// Words it holds (a song's lyrics): filled for a search with plain words, else None.
    pub words: Option<String>,
    /// Its Develop edits, when it has any.
    pub develop: Option<crate::edits::EditFacts>,
    /// The manual albums it is in, full names one per line.
    pub albums: Option<String>,
    /// Its IPTC fields (`specular_metadata::edit::Record::to_lines`): the sidecar's, else
    /// the file's; None until read.
    pub iptc: Option<String>,
}

impl Row {
    /// Modification time, seconds.
    pub fn modified(&self) -> i64 {
        self.mtime_ns.div_euclid(1_000_000_000)
    }

    pub fn modified_time(&self) -> std::time::SystemTime {
        let t = std::time::UNIX_EPOCH;
        if self.mtime_ns >= 0 {
            t + Duration::from_nanos(self.mtime_ns as u64)
        } else {
            t - Duration::from_nanos(self.mtime_ns.unsigned_abs())
        }
    }

    /// The thumbnail cache's key, without reading the file (also while it is offline).
    pub fn thumb_key(&self) -> Option<ThumbKey> {
        Some(ThumbKey {
            stamp: Stamp {
                len: self.size,
                mtime_ns: i128::from(self.mtime_ns),
            },
            content: self.content?,
        })
    }

    /// What plain words search: name, folder, camera, lens, keywords and place names, one
    /// per line.
    pub fn text(&self) -> String {
        let mut t = String::with_capacity(self.name.len() + self.folder.len() + 2);
        t.push_str(&self.name);
        t.push('\n');
        t.push_str(&self.folder);
        if let Some(m) = &self.meta {
            for s in [&m.camera, &m.lens].into_iter().flatten() {
                t.push('\n');
                t.push_str(s);
            }
        }
        if !self.tags.is_empty() {
            t.push('\n');
            t.push_str(&self.tags);
        }
        if let Some(p) = self.place.as_deref().filter(|p| !p.is_empty()) {
            t.push('\n');
            t.push_str(p);
        }
        if let Some(w) = self.words.as_deref().filter(|w| !w.is_empty()) {
            t.push('\n');
            t.push_str(w);
        }
        t
    }

    /// The query's view of it (`text` from [`Row::text`]).
    pub fn facts<'a>(&'a self, text: &'a str) -> Facts<'a> {
        let mut f = Facts {
            name: &self.name,
            ext: &self.ext,
            container: false,
            size: Some(self.size),
            modified: Some(self.modified()),
            rating: self.mark.rating,
            label: self.mark.label,
            pick: self.mark.flag == 1,
            reject: self.mark.flag == -1,
            known: self.state != 0,
            folder: Some(&self.folder),
            text: Some(text),
            tags: (self.state != 0).then_some(self.tags.as_str()),
            ..Facts::default()
        };
        if let Some(m) = &self.meta {
            f.width = m.width;
            f.height = m.height;
            f.taken = m.date_taken;
            f.camera = m.camera.as_deref();
            f.lens = m.lens.as_deref();
            f.iso = m.iso;
            f.f_number = m.f_number;
            f.focal = m.focal_length;
            f.exposure = m.exposure;
            f.animated = m.animated;
            f.hdr = m.hdr;
            f.alpha = m.has_alpha;
            f.raw = m.raw;
            f.profile = m.has_profile;
            f.pages = m.pages;
            f.bit_depth = m.bit_depth;
            f.gps = m.gps;
            f.space = m.space.as_deref();
            f.sound = m.video.then_some(m.sound);
            if let Some(a) = &m.astro {
                f.subframe = true;
                f.object = a.object.as_deref();
                f.filter = a.filter.as_deref();
                f.frame_type = a.frame_type.as_deref();
                f.gain = a.gain;
                f.ccd_temp = a.ccd_temp;
                f.stars = a.metrics.map(|s| specular_query::StarFacts {
                    count: s.stars,
                    hfd: s.hfd,
                    fwhm: s.fwhm,
                    seeing: s.fwhm_arcsec(),
                    ecc: s.eccentricity,
                    background: s.background,
                    snr: s.snr,
                });
            }
        }
        f.dupes = self.dupes;
        f.similar = self.similar;
        f.place = self.place.as_deref();
        f.people = self.people.as_deref();
        f.albums = self.albums.as_deref();
        f.iptc = self.iptc.as_deref();
        f.exif = self
            .meta
            .as_ref()
            .and_then(|m| m.exif.as_deref())
            .map(specular_thumbs::ExifMeta::facts);
        match &self.develop {
            Some(d) => {
                f.edited = Some(d.edited);
                // Saved by an older build: not known, so `module:` lets it through.
                f.modules = d.modules.as_deref();
                f.styles = d.styles.as_deref();
                f.versions = d.versions;
            }
            None => {
                f.edited = Some(false);
                f.modules = Some("");
                f.styles = Some("");
            }
        }
        f
    }
}

/// What a search found.
#[derive(Debug, Default)]
pub struct Found {
    pub rows: Vec<Row>,
    /// Every match (also past the limit).
    pub total: usize,
    /// Rows SQL returned for the matcher to check.
    pub candidates: usize,
    pub facets: Vec<Facet>,
    /// The parsed query (its errors are shown in the search bar).
    pub query: Query,
    /// With [`Search::timeline`]: matches per day (day number of the capture date, else the
    /// modification date; see `specular_query::timeline`), by day. The query's own `date:`
    /// terms are left out of these counts, so the timeline shows where else to go.
    pub timeline: Vec<(i64, u64)>,
    pub elapsed: Duration,
}

const COLUMNS: &str = "f.id, d.path, f.name, f.ext, f.size, f.mtime_ns, f.state, f.format,
    f.width, f.height, f.taken, f.camera, f.lens, f.iso, f.f_number, f.focal, f.exposure,
    f.pages, f.bits, f.props, f.rating, f.label, f.flag, th.content, th.classes, o.offline,
    f.space, f.lat, f.lon, f.geo, f.place_id, f.astro, f.exif, f.iptc";

/// Whether a query names `exif:` (in a term, or one being typed: the picker counts the
/// tags for `exif:` with no value yet), so the tags are loaded (they are most of a row).
fn asks_exif(q: &Query) -> bool {
    q.terms.iter().any(|t| t.field() == Some(Field::Exif))
        || q.errors.iter().any(|e| e.field == "exif")
}

/// Runs `req`; `fold_id` names `fold`. Text goes to the full-text index only when the index
/// is in that fold (not while the indexer rebuilds it for another one: every candidate is
/// then checked by the matcher alone, slower but the same results).
pub fn run(conn: &Connection, req: &Search, fold: Fold, fold_id: &str) -> Result<Found, Error> {
    let mut rows = Vec::new();
    let mut found = run_each(conn, req, fold, fold_id, &mut |r| rows.push(r))?;
    found.rows = rows;
    Ok(found)
}

/// [`run`], handing each row to `emit` in order instead of collecting them (`rows` stays
/// empty): a caller that makes its own items from the rows never holds them all (about
/// 70 MB at 100 000 files). Only a search by description (`ranked`) holds its matches, to
/// order them.
pub fn run_each(
    conn: &Connection,
    req: &Search,
    fold: Fold,
    fold_id: &str,
    emit: &mut dyn FnMut(Row),
) -> Result<Found, Error> {
    run_each_cached(conn, req, fold, fold_id, None, emit)
}

/// [`run_each`], keeping the tables every search reads whole in `cache` (one per
/// connection) while the catalogue does not change.
pub fn run_each_cached(
    conn: &Connection,
    req: &Search,
    fold: Fold,
    fold_id: &str,
    cache: Option<&SideCache>,
    emit: &mut dyn FnMut(Row),
) -> Result<Found, Error> {
    let t0 = Instant::now();
    let query = parse_in(&req.query, Scope::Library);
    let matcher = Matcher::new(&query, fold);
    let fts = crate::store::info_get(conn, "fold")?.as_deref() == Some(fold_id);
    let places = crate::places::Places::load(conn, req.language.as_ref())?;
    let scope = req
        .rules
        .as_ref()
        .filter(|r| !r.is_empty())
        .map(|r| r.matcher(Scope::Library, fold));
    let mut w = Where::default();
    if let Some(a) = req.album {
        w.and(
            "f.id IN (SELECT file_id FROM collection_files WHERE collection_id = ?)".into(),
            vec![Value::Integer(a)],
        );
    }
    if let Some(l) = req.location {
        w.and("d.folder_id = ?".into(), vec![Value::Integer(l)]);
    }
    // Relevance order: the rank of each file the description found.
    let rank: Option<HashMap<i64, usize>> = req.ranked.as_ref().map(|r| {
        let mut m = HashMap::with_capacity(r.len());
        for (i, (id, _)) in r.iter().enumerate() {
            m.entry(*id).or_insert(i);
        }
        m
    });
    if let Some(r) = &rank {
        if r.is_empty() {
            w.and("0".into(), Vec::new());
        } else {
            // Integers only: written into the statement (no parameter limit to mind).
            let ids: Vec<String> = r.keys().map(i64::to_string).collect();
            w.and(format!("f.id IN ({})", ids.join(",")), Vec::new());
        }
    }
    if !req.no_pushdown {
        for t in &query.terms {
            let own_facet = t
                .field()
                .is_some_and(|f| !t.negated && FacetField::of(f).is_some());
            if req.facets && own_facet {
                continue;
            }
            // The timeline counts the matches of the rest of the query.
            if req.timeline && !t.negated && t.field() == Some(Field::Date) {
                continue;
            }
            push_down(&mut w, t, fold, fts, &places);
        }
        if let Some(s) = &scope {
            for q in s.narrowing() {
                for t in &q.terms {
                    push_down(&mut w, t, fold, fts, &places);
                }
            }
        }
    }
    let order = match req.sort {
        Sort::Name => "f.name COLLATE NOCASE",
        Sort::Date => "COALESCE(f.taken, f.mtime_ns / 1000000000)",
        Sort::Modified => "f.mtime_ns",
        Sort::Size => "f.size",
        Sort::Rating => "f.rating",
        Sort::Folder => "d.path, f.name COLLATE NOCASE",
        Sort::Unordered => "",
    };
    let dir = if req.descending { "DESC" } else { "ASC" };
    let order_by = if order.is_empty() {
        String::new()
    } else {
        format!("ORDER BY {order} {dir}, f.id {dir}")
    };
    let exif_tags = asks_exif(&query)
        || req
            .rules
            .as_ref()
            .is_some_and(|r| r.parsed(Scope::Library).iter().any(|(_, q)| asks_exif(q)));
    let tags_col = if exif_tags { "f.exif_tags" } else { "NULL" };
    let sql = format!(
        "SELECT {COLUMNS}, {tags_col} FROM files f JOIN dirs d ON d.id = f.dir_id
         JOIN folders o ON o.id = d.folder_id LEFT JOIN thumbs th ON th.file_id = f.id
         {} {order_by}",
        w.sql()
    );
    // Tags, group roles, Develop edits and albums (few next to files: one pass over each
    // table beats a correlated subquery per row).
    let side = Side::load(conn, cache)?;
    // Named people, only when asked for and known.
    let asks_person = query
        .terms
        .iter()
        .any(|t| t.field() == Some(specular_query::Field::Person));
    let mut people = if asks_person && crate::ai::has(conn, "people")? {
        Some(crate::ai::file_people(conn)?)
    } else {
        None
    };
    // Files' words (lyrics), only when plain words are searched for.
    let asks_words = query.terms.iter().any(|t| matches!(t.kind, Kind::Text(_)));
    let mut words: HashMap<i64, String> = HashMap::new();
    if asks_words {
        let mut st = conn.prepare_cached("SELECT file_id, text FROM file_text")?;
        let mut q = st.query([])?;
        while let Some(r) = q.next()? {
            words.insert(r.get(0)?, r.get(1)?);
        }
        // Titles and captions from other apps' sidecars.
        let mut st = conn.prepare_cached(
            "SELECT file_id, coalesce(title, '') || char(10) || coalesce(caption, '')
             FROM foreign_sides WHERE duplicate = 0 AND (title IS NOT NULL OR caption IS NOT NULL)",
        )?;
        let mut q = st.query([])?;
        while let Some(r) = q.next()? {
            let s = words.entry(r.get(0)?).or_default();
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(r.get_ref(1)?.as_str().unwrap_or_default());
        }
    }
    let mut st = conn.prepare(&sql)?;
    let mut rows = st.query(rusqlite::params_from_iter(w.params.iter()))?;
    let mut found = Found::default();
    let mut ranked_rows = Vec::new();
    let mut emitted = 0usize;
    let mut tally = Tally::default();
    // Nothing to check, count or rank: rows past the limit are only counted.
    let count_only =
        matcher.is_empty() && !req.facets && rank.is_none() && scope.is_none() && !req.timeline;
    let mut days: HashMap<i64, u64> = HashMap::new();
    let date_bit = specular_query::facet::FacetField::Year.bit();
    while let Some(r) = rows.next()? {
        if count_only && req.limit.is_some_and(|l| found.rows.len() >= l) {
            found.candidates += 1;
            found.total += 1;
            continue;
        }
        let mut row = row_of(r)?;
        // Where: NULL not looked for, 0 no position, 1 a position (and maybe a place).
        let geo: Option<i64> = r.get(29)?;
        if geo.is_some() {
            let id: Option<i64> = r.get(30)?;
            row.place = Some(
                id.and_then(|id| places.text(id))
                    .unwrap_or_default()
                    .to_owned(),
            );
        }
        side.fill(&mut row);
        if let Some(p) = people.as_mut() {
            row.people = Some(p.remove(&row.id).unwrap_or_default());
        }
        if let Some(w) = words.remove(&row.id) {
            row.words = Some(w);
        }
        found.candidates += 1;
        if let Some(s) = &scope {
            let text = row.text();
            if !s.matches(&row.facts(&text)) {
                continue;
            }
        }
        // Nothing to check nor count (everything, as the grid's "All locations" asks): no
        // text and facts made per row (a fifth of a million-row search).
        let mask = if matcher.is_empty() && !req.facets {
            if req.timeline {
                // No facts made for this alone (most of an "everything" search).
                let t = row.meta.as_ref().and_then(|m| m.date_taken);
                let t = t.unwrap_or_else(|| row.modified());
                *days
                    .entry(t.div_euclid(specular_query::date::DAY))
                    .or_default() += 1;
            }
            0
        } else {
            let text = row.text();
            let facts = row.facts(&text);
            let mask = matcher.fail_mask(&facts);
            if req.facets {
                tally.add(&facts, mask);
            }
            if req.timeline
                && mask & !date_bit == 0
                && let Some(t) = facts.taken.or(facts.modified)
            {
                *days
                    .entry(t.div_euclid(specular_query::date::DAY))
                    .or_default() += 1;
            }
            mask
        };
        if mask == 0 {
            found.total += 1;
            if rank.is_some() {
                ranked_rows.push(row);
            } else if req.limit.is_none_or(|l| emitted < l) {
                emitted += 1;
                emit(row);
            }
        }
    }
    drop(rows);
    if let Some(r) = &rank {
        ranked_rows.sort_by_key(|row| r.get(&row.id).copied().unwrap_or(usize::MAX));
        if let Some(l) = req.limit {
            ranked_rows.truncate(l);
        }
        for row in ranked_rows {
            emit(row);
        }
    }
    if req.facets {
        found.facets = tally.finish(&query);
    }
    if req.timeline {
        let mut t: Vec<(i64, u64)> = days.into_iter().collect();
        t.sort_unstable();
        found.timeline = t;
    }
    found.query = query;
    found.elapsed = t0.elapsed();
    Ok(found)
}

/// The facts a row takes from other tables, each read whole.
pub struct Side {
    tags: HashMap<i64, String>,
    roles: HashMap<i64, [Option<bool>; 2]>,
    develop: HashMap<i64, crate::edits::EditFacts>,
    albums: HashMap<i64, String>,
}

/// The last [`Side`] a connection read, with the catalogue's version it was read at: kept
/// while the catalogue does not change, so searching again (each typed letter) does not read
/// the tables again. One per connection.
#[derive(Default)]
pub struct SideCache(std::sync::Mutex<Option<(SideVersion, std::sync::Arc<Side>)>>);

/// The catalogue's `data_version` and the connection's `total_changes`.
type SideVersion = (i64, u64);

impl Side {
    fn load(
        conn: &Connection,
        cache: Option<&SideCache>,
    ) -> rusqlite::Result<std::sync::Arc<Side>> {
        // `data_version` changes with another connection's commit, `total_changes` with
        // this one's own writes.
        let version: i64 = conn.pragma_query_value(None, "data_version", |r| r.get(0))?;
        let key = (version, conn.total_changes());
        if let Some(c) = cache
            && let Some((k, side)) = c.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            && *k == key
        {
            return Ok(side.clone());
        }
        let side = std::sync::Arc::new(Side {
            tags: tags_all(conn)?,
            roles: crate::matches::roles(conn)?,
            develop: crate::edits::all_facts(conn)?,
            albums: crate::albums::file_albums(conn)?,
        });
        if let Some(c) = cache {
            *c.0.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, side.clone()));
        }
        Ok(side)
    }

    fn fill(&self, row: &mut Row) {
        if let Some(t) = self.tags.get(&row.id) {
            row.tags = t.clone();
        }
        if let Some([d, s]) = self.roles.get(&row.id) {
            row.dupes = *d;
            row.similar = *s;
        }
        row.develop = self.develop.get(&row.id).cloned();
        row.albums = Some(self.albums.get(&row.id).cloned().unwrap_or_default());
    }
}

/// Each file's tags, one per line in name order.
fn tags_all(conn: &Connection) -> rusqlite::Result<HashMap<i64, String>> {
    let mut tags: HashMap<i64, String> = HashMap::new();
    let mut st = conn.prepare_cached(
        "SELECT ft.file_id, t.name FROM file_tags ft JOIN tags t ON t.id = ft.tag_id
         ORDER BY ft.file_id, t.name",
    )?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        let s = tags.entry(r.get(0)?).or_default();
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(r.get_ref(1)?.as_str().unwrap_or_default());
    }
    Ok(tags)
}

fn row_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    let folder: String = r.get(1)?;
    let name: String = r.get(2)?;
    let state: i64 = r.get(6)?;
    let props: i64 = r.get(19)?;
    let meta = if state != 0 {
        Some(Meta {
            format: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
            width: r.get::<_, Option<u32>>(8)?.unwrap_or(0),
            height: r.get::<_, Option<u32>>(9)?.unwrap_or(0),
            animated: props & 1 != 0,
            hdr: props & 2 != 0,
            has_alpha: props & 4 != 0,
            raw: props & 8 != 0,
            pages: r.get::<_, Option<u32>>(17)?.unwrap_or(0),
            bit_depth: r.get::<_, Option<u8>>(18)?.unwrap_or(0),
            has_profile: props & 16 != 0,
            date_taken: r.get(10)?,
            camera: r.get(11)?,
            lens: r.get(12)?,
            iso: r.get(13)?,
            exposure: r.get::<_, Option<f64>>(16)?.map(|v| v as f32),
            f_number: r.get::<_, Option<f64>>(14)?.map(|v| v as f32),
            focal_length: r.get::<_, Option<f64>>(15)?.map(|v| v as f32),
            gps: (props & 32 != 0).then_some(props & 64 != 0),
            position: match (r.get::<_, Option<f64>>(27)?, r.get::<_, Option<f64>>(28)?) {
                (Some(lat), Some(lon)) => Some((lat as f32, lon as f32)),
                _ => None,
            },
            motion: props & 128 != 0,
            live_photo: props & 256 != 0,
            space: r.get(26)?,
            video: props & 512 != 0,
            sound: props & 1536 == 1536,
            // The catalogue keeps no length: the tile shows a play mark without it.
            duration_us: None,
            astro: r
                .get::<_, Option<Vec<u8>>>(31)?
                .filter(|b| !b.is_empty())
                .and_then(|b| specular_thumbs::AstroMeta::from_bytes(&b))
                .map(Box::new),
            // NULL: read before the Exif fields were kept (unknown until read again).
            exif: r
                .get::<_, Option<Vec<u8>>>(32)?
                .and_then(|b| specular_thumbs::ExifMeta::from_bytes(&b))
                .map(|mut e| {
                    e.tags = r.get::<_, Option<String>>(34).ok().flatten();
                    Box::new(e)
                }),
        })
    } else {
        None
    };
    Ok(Row {
        id: r.get(0)?,
        path: PathBuf::from(&folder).join(&name),
        name,
        folder,
        ext: r.get(3)?,
        size: r.get::<_, i64>(4)? as u64,
        mtime_ns: r.get(5)?,
        state: state as u8,
        meta,
        mark: Mark {
            rating: r.get(20)?,
            label: r.get(21)?,
            flag: r.get(22)?,
        },
        tags: String::new(),
        content: r.get::<_, Option<i64>>(23)?.map(|c| c as u64),
        classes: r.get::<_, Option<u32>>(24)?.unwrap_or(0),
        offline: r.get::<_, i64>(25)? != 0,
        dupes: None,
        similar: None,
        place: None,
        people: None,
        words: None,
        develop: None,
        albums: None,
        iptc: r.get(33)?,
    })
}

#[derive(Default)]
struct Where {
    clauses: Vec<String>,
    params: Vec<Value>,
}

impl Where {
    fn and(&mut self, clause: String, params: Vec<Value>) {
        self.clauses.push(clause);
        self.params.extend(params);
    }

    fn sql(&self) -> String {
        if self.clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", self.clauses.join(" AND "))
        }
    }
}

/// An FTS5 phrase for `s` (any characters, quotes doubled).
fn phrase(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// The folded needles of a text term, when every one of them can use the trigram index.
fn needles(any: &[String], exact: bool, fold: Fold) -> Option<Vec<String>> {
    let v: Vec<String> = any
        .iter()
        .map(|s| {
            let f = fold(s).into_owned();
            if exact { f.trim().to_owned() } else { f }
        })
        .collect();
    let ok = !v.is_empty()
        && v.iter()
            .all(|n| n.chars().count() >= 3 && !n.contains('\n'));
    ok.then_some(v)
}

fn ranges(expr: &str, rs: &[Range]) -> String {
    let parts: Vec<String> = rs
        .iter()
        .map(|r| {
            let mut c = Vec::new();
            if r.lo.is_finite() {
                c.push(format!(
                    "{expr} {} {}",
                    if r.lo_open { ">" } else { ">=" },
                    num(r.lo)
                ));
            }
            if r.hi.is_finite() {
                c.push(format!(
                    "{expr} {} {}",
                    if r.hi_open { "<" } else { "<=" },
                    num(r.hi)
                ));
            }
            if c.is_empty() {
                format!("{expr} IS NOT NULL")
            } else {
                format!("({})", c.join(" AND "))
            }
        })
        .collect();
    if parts.is_empty() {
        "0".into()
    } else {
        format!("({})", parts.join(" OR "))
    }
}

/// A number as an SQL literal (finite, so `{:?}` gives a valid one).
fn num(v: f64) -> String {
    format!("{v:?}")
}

/// Facts not read yet pass a metadata condition (unknown), as in the gallery.
fn meta_or_unread(cond: String) -> String {
    format!("(f.state = 0 OR COALESCE({cond}, 0))")
}

fn orient_sql(o: Orient) -> &'static str {
    match o {
        Orient::Landscape => "f.width > f.height",
        Orient::Portrait => "f.width < f.height",
        Orient::Square => "f.width = f.height",
    }
}

fn flag_sql(fl: Flag) -> String {
    let meta = |c: &str| meta_or_unread(c.to_owned());
    let orient = |o| {
        meta_or_unread(format!(
            "(f.width > 0 AND f.height > 0 AND {})",
            orient_sql(o)
        ))
    };
    // Read as a movie or sound file: `read`; else by the extension, one of `exts`.
    let av = |read: &str, exts: &[&str]| {
        let list = exts
            .iter()
            .map(|e| format!("'{e}'"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "(CASE WHEN f.state <> 0 AND (f.props & 512) <> 0 THEN {read} \
             ELSE lower(f.ext) IN ({list}) END)"
        )
    };
    match fl {
        Flag::Pick => "f.flag = 1".into(),
        Flag::Reject => "f.flag = -1".into(),
        Flag::Flagged => "f.flag <> 0".into(),
        Flag::Rated => "f.rating > 0".into(),
        Flag::Unrated => "f.rating = 0".into(),
        Flag::Labelled => "f.label > 0".into(),
        Flag::Folder => "0".into(),
        Flag::Image => "1".into(),
        Flag::Raw => meta("(f.props & 8) <> 0"),
        Flag::Hdr => meta("(f.props & 2) <> 0"),
        Flag::Animated => meta("(f.props & 1) <> 0"),
        Flag::MultiPage => meta("f.pages > 1"),
        Flag::Landscape => orient(Orient::Landscape),
        Flag::Portrait => orient(Orient::Portrait),
        Flag::Square => orient(Orient::Square),
        Flag::Alpha => meta("(f.props & 4) <> 0"),
        Flag::Profile => meta("(f.props & 16) <> 0"),
        Flag::Camera => meta("f.camera IS NOT NULL"),
        Flag::Lens => meta("f.lens IS NOT NULL"),
        Flag::Date => meta("f.taken IS NOT NULL"),
        // Unknown also where the position was not looked for.
        Flag::Gps => "(f.state = 0 OR (f.props & 32) = 0 OR (f.props & 64) <> 0)".into(),
        // Movies and sound files by their content once read (512: a movie or sound file,
        // 1024: no picture), else their extension.
        Flag::Video => av("(f.props & 1024) = 0", &specular_query::VIDEO_EXTS),
        Flag::Sound => av("(f.props & 1024) <> 0", &specular_query::SOUND_EXTS),
    }
}

/// The files of `places` (ids), as an SQL condition.
fn place_in(ids: &[i64]) -> String {
    if ids.is_empty() {
        return "0".into();
    }
    let list = ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("f.place_id IN ({list})")
}

/// `fts`: the full-text index may be used (it is in `fold`).
fn push_down(w: &mut Where, t: &Term, fold: Fold, fts: bool, places: &crate::places::Places) {
    let neg = t.negated;
    match &t.kind {
        Kind::Text(s) => {
            if neg || !fts {
                return;
            }
            if let Some(n) = needles(std::slice::from_ref(s), false, fold) {
                let p = phrase(&n[0]);
                let at = if places.is_empty() {
                    String::new()
                } else {
                    format!(
                        " UNION SELECT id FROM files f WHERE {}",
                        place_in(&places.matching(&n, fold))
                    )
                };
                w.and(
                    // Files whose row comes with their facts pass (`fts` 0) too. One set of
                    // ids (a UNION, not ORs on `f`) so the index drives the search: with ORs
                    // SQLite scanned every file (273 ms against 1 ms at 1M, a selective word).
                    format!(
                        "f.id IN (SELECT rowid FROM fts_index WHERE fts_index MATCH ?
                          UNION SELECT id FROM files
                            WHERE dir_id IN (SELECT rowid FROM fts_dirs WHERE fts_dirs MATCH ?)
                          UNION SELECT id FROM files WHERE fts = 0{at}
                          UNION SELECT file_id FROM file_text)"
                    ),
                    vec![Value::Text(p.clone()), Value::Text(p)],
                );
            }
        }
        // Not looked for yet passes (unknown), as in the matcher.
        Kind::Field {
            field: Field::Place,
            pred: Pred::Text { any, .. },
            ..
        } => {
            if neg {
                return;
            }
            let n: Vec<String> = any.iter().map(|s| fold(s).trim().to_owned()).collect();
            if n.iter().any(String::is_empty) {
                return;
            }
            w.and(
                format!(
                    // IPTC locations: the matcher looks at those files.
                    "(f.state = 0 OR f.geo IS NULL OR f.iptc IS NULL
                      OR f.iptc LIKE '%shown-%' OR f.iptc LIKE '%created-%' OR {})",
                    place_in(&places.matching(&n, fold))
                ),
                vec![],
            );
        }
        Kind::Field { field, pred, .. } => match (field, pred) {
            (Field::Name | Field::Camera | Field::Lens | Field::Tag, Pred::Text { exact, any }) => {
                if neg || !fts {
                    return;
                }
                let Some(n) = needles(any, *exact, fold) else {
                    return;
                };
                let col = match field {
                    Field::Name => "name",
                    Field::Camera => "camera",
                    Field::Lens => "lens",
                    _ => "tags",
                };
                let expr = n
                    .iter()
                    .map(|n| format!("{col} : {}", phrase(n)))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                // A file without its row (`fts` 0) has no facts yet either. A UNION of ids,
                // as for plain words, so the index drives the search.
                let pending = if *field == Field::Name {
                    "fts = 0"
                } else {
                    "state = 0"
                };
                let clause = format!(
                    "f.id IN (SELECT rowid FROM fts_index WHERE fts_index MATCH ?
                      UNION SELECT id FROM files WHERE {pending})"
                );
                w.and(clause, vec![Value::Text(expr)]);
            }
            (Field::Folder, Pred::Text { exact, any }) => {
                if neg || !fts {
                    return;
                }
                let Some(n) = needles(any, *exact, fold) else {
                    return;
                };
                let expr = n.iter().map(|n| phrase(n)).collect::<Vec<_>>().join(" OR ");
                w.and(
                    "f.dir_id IN (SELECT rowid FROM fts_dirs WHERE fts_dirs MATCH ?)".into(),
                    vec![Value::Text(expr)],
                );
            }
            (Field::Ext, Pred::Ext(v)) => {
                if v.is_empty() {
                    return;
                }
                let q = vec!["?"; v.len()].join(", ");
                let not = if neg { "NOT " } else { "" };
                w.and(
                    format!("f.ext {not}IN ({q})"),
                    v.iter().map(|e| Value::Text(e.clone())).collect(),
                );
            }
            (Field::Rating | Field::Size, Pred::Num(rs)) => {
                let col = if *field == Field::Rating {
                    "f.rating"
                } else {
                    "f.size"
                };
                let c = ranges(col, rs);
                w.and(if neg { format!("NOT {c}") } else { c }, vec![]);
            }
            (Field::Label, Pred::Label(v)) => {
                let list = v.iter().map(u8::to_string).collect::<Vec<_>>().join(", ");
                let not = if neg { "NOT " } else { "" };
                w.and(format!("f.label {not}IN ({list})"), vec![]);
            }
            (Field::Modified, Pred::Date(rs)) => {
                let c = ranges("(f.mtime_ns / 1000000000)", rs);
                w.and(if neg { format!("NOT {c}") } else { c }, vec![]);
            }
            (Field::Date, Pred::Date(rs)) if !neg => {
                w.and(
                    meta_or_unread(ranges("COALESCE(f.taken, f.mtime_ns / 1000000000)", rs)),
                    vec![],
                );
            }
            (_, Pred::Num(rs)) if !neg => {
                let expr = match field {
                    Field::Iso => "f.iso",
                    Field::Aperture => "f.f_number",
                    Field::Focal => "f.focal",
                    Field::Exposure => "f.exposure",
                    Field::Width => "NULLIF(f.width, 0)",
                    Field::Height => "NULLIF(f.height, 0)",
                    Field::Megapixels => "(NULLIF(f.width, 0) * NULLIF(f.height, 0) / 1000000.0)",
                    Field::Bits => "NULLIF(f.bits, 0)",
                    Field::Pages => "MAX(COALESCE(f.pages, 0), 1)",
                    _ => return,
                };
                w.and(meta_or_unread(ranges(expr, rs)), vec![]);
            }
            (Field::Orientation, Pred::Orient(v)) if !neg => {
                let any = v
                    .iter()
                    .map(|o| orient_sql(*o))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if any.is_empty() {
                    return;
                }
                w.and(
                    meta_or_unread(format!("(f.width > 0 AND f.height > 0 AND ({any}))")),
                    vec![],
                );
            }
            (_, Pred::Flags(v)) if !neg => {
                if v.is_empty() {
                    return;
                }
                let any = v
                    .iter()
                    .map(|f| flag_sql(*f))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                w.and(format!("({any})"), vec![]);
            }
            // Develop: edited photos are few next to the rest; `module:` and `style:` need
            // an edit (their text is the matcher's).
            (Field::Edited, Pred::Words(v)) if v.len() == 1 => {
                let yes = (v[0] == "yes") != neg;
                let not = if yes { "" } else { "NOT " };
                w.and(
                    format!("f.id {not}IN (SELECT file_id FROM edits WHERE version = 0)"),
                    vec![],
                );
            }
            (Field::Module | Field::Style, Pred::Text { .. }) if !neg => {
                w.and(
                    "f.id IN (SELECT file_id FROM edits WHERE version = 0)".into(),
                    vec![],
                );
            }
            (Field::Version, Pred::Words(v)) if !neg && v.iter().all(|w| *w == "yes") => {
                w.and(
                    "f.id IN (SELECT file_id FROM edits GROUP BY file_id HAVING count(*) > 1)"
                        .into(),
                    vec![],
                );
            }
            _ => {}
        },
    }
}

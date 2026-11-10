//! Albums: manual albums (files put in by hand) and smart albums (saved rules over the
//! query language, [`specular_query::rules`]).
//!
//! An album's name is its full path, parts joined by `/`: `Client X/selects` is the album
//! `selects` in the group `Client X`. Groups are only the names' leading parts (no rows of
//! their own), so a group appears with its first album and goes with its last. Names are
//! unique without regard to case.
//!
//! Files are held by catalogue id: a file leaving the catalogue leaves its albums, and the
//! files themselves are never touched.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use specular_query::rules::Rules;

/// Which kind of album.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Files put in by hand.
    Manual,
    /// Whatever matches its rules.
    Smart,
}

impl Kind {
    fn code(self) -> i64 {
        match self {
            Kind::Manual => 1,
            Kind::Smart => 2,
        }
    }

    fn of(code: i64) -> Kind {
        if code == 2 { Kind::Smart } else { Kind::Manual }
    }
}

/// One album.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Album {
    pub id: i64,
    /// The full name, parts joined by `/` without spaces around them.
    pub name: String,
    pub kind: Kind,
    /// A smart album's rules (empty for a manual one).
    pub rules: Rules,
    /// How many files a manual album holds (0 for a smart one: its count is a search).
    pub files: u64,
}

impl Album {
    /// The last part of the name (what the sidebar shows).
    pub fn leaf(&self) -> &str {
        leaf(&self.name)
    }

    /// The groups it is in, outermost first.
    pub fn groups(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.name.split('/').collect();
        v.pop();
        v
    }
}

/// The last part of a full name.
pub fn leaf(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// What is wrong with a name typed for an album.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    /// Another album has it (its full name).
    Taken(String),
    /// It would make an album a group, or put it inside one (the album's full name): an
    /// album can't hold albums.
    Clash(String),
}

/// A typed name in its stored form: parts split at `/`, trimmed, empty ones dropped
/// (`" Client X / selects/"` → `Client X/selects`). None when nothing is left.
pub fn normalize(name: &str) -> Option<String> {
    let parts: Vec<&str> = name
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Same name without regard to case (as the catalogue compares them: ASCII only).
fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Why `name` can't be used for an album (None: it can), given the albums there are;
/// `except` is the album being renamed.
pub fn name_problem(albums: &[Album], name: &str, except: Option<i64>) -> Option<NameProblem> {
    let Some(n) = normalize(name) else {
        return Some(NameProblem::Empty);
    };
    let others = || albums.iter().filter(|a| Some(a.id) != except);
    if let Some(a) = others().find(|a| same(&a.name, &n)) {
        return Some(NameProblem::Taken(a.name.clone()));
    }
    // "A/b" with an album "A", or "A" with an album "A/b".
    let inside = |outer: &str, inner: &str| {
        inner.len() > outer.len()
            && inner.is_char_boundary(outer.len())
            && same(&inner[..outer.len()], outer)
            && inner.as_bytes()[outer.len()] == b'/'
    };
    others()
        .find(|a| inside(&a.name, &n) || inside(&n, &a.name))
        .map(|a| NameProblem::Clash(a.name.clone()))
}

/// The albums whose names are `group` or lie inside it.
pub fn in_group<'a>(albums: &'a [Album], group: &'a str) -> impl Iterator<Item = &'a Album> {
    albums.iter().filter(move |a| {
        same(&a.name, group)
            || (a.name.len() > group.len()
                && a.name.is_char_boundary(group.len())
                && same(&a.name[..group.len()], group)
                && a.name.as_bytes()[group.len()] == b'/')
    })
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Every album, by name.
pub fn list(c: &Connection) -> rusqlite::Result<Vec<Album>> {
    let mut st = c.prepare_cached(
        "SELECT c.id, c.name, c.kind, c.rules,
                (SELECT COUNT(*) FROM collection_files cf WHERE cf.collection_id = c.id)
         FROM collections c ORDER BY c.name COLLATE NOCASE",
    )?;
    let rows = st.query_map([], |r| {
        let kind = Kind::of(r.get(2)?);
        Ok(Album {
            id: r.get(0)?,
            name: r.get(1)?,
            kind,
            rules: r
                .get::<_, Option<String>>(3)?
                .map(|t| Rules::from_text(&t))
                .unwrap_or_default(),
            files: if kind == Kind::Manual {
                r.get::<_, i64>(4)? as u64
            } else {
                0
            },
        })
    })?;
    rows.collect()
}

/// Makes an album. `name` is normalized; the caller has checked [`name_problem`] (a taken
/// name fails here with the catalogue's constraint error).
pub fn create(c: &Connection, name: &str, kind: Kind, rules: &Rules) -> rusqlite::Result<i64> {
    let name = normalize(name).ok_or(rusqlite::Error::InvalidQuery)?;
    let rules = (kind == Kind::Smart).then(|| rules.to_text());
    c.execute(
        "INSERT INTO collections (name, kind, rules, created) VALUES (?1, ?2, ?3, ?4)",
        params![name, kind.code(), rules, now()],
    )?;
    Ok(c.last_insert_rowid())
}

/// Renames an album (a new group part moves it).
pub fn rename(c: &Connection, id: i64, name: &str) -> rusqlite::Result<()> {
    let name = normalize(name).ok_or(rusqlite::Error::InvalidQuery)?;
    c.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    Ok(())
}

/// Renames a group: every album in it gets the new leading parts. Returns how many moved.
/// Fails (changing nothing) when a new name is taken.
pub fn rename_group(c: &mut Connection, group: &str, to: &str) -> rusqlite::Result<usize> {
    let to = normalize(to).ok_or(rusqlite::Error::InvalidQuery)?;
    let albums = list(c)?;
    let tx = c.transaction()?;
    let mut n = 0;
    for a in in_group(&albums, group) {
        let rest = &a.name[group.len()..];
        tx.execute(
            "UPDATE collections SET name = ?2 WHERE id = ?1",
            params![a.id, format!("{to}{rest}")],
        )?;
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}

/// Replaces a smart album's rules.
pub fn set_rules(c: &Connection, id: i64, rules: &Rules) -> rusqlite::Result<()> {
    c.execute(
        "UPDATE collections SET rules = ?2 WHERE id = ?1 AND kind = 2",
        params![id, rules.to_text()],
    )?;
    Ok(())
}

/// Deletes albums (their files stay in the catalogue and on disk).
pub fn delete(c: &mut Connection, ids: &[i64]) -> rusqlite::Result<()> {
    let tx = c.transaction()?;
    for id in ids {
        tx.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    }
    tx.commit()
}

/// Puts files in a manual album. Returns how many were not in it already.
pub fn add(c: &mut Connection, id: i64, files: &[i64]) -> rusqlite::Result<usize> {
    let t = now();
    let tx = c.transaction()?;
    let mut n = 0;
    {
        let mut st = tx.prepare_cached(
            "INSERT OR IGNORE INTO collection_files (collection_id, file_id, added)
             SELECT ?1, ?2, ?3 FROM collections WHERE id = ?1 AND kind = 1",
        )?;
        for f in files {
            n += st.execute(params![id, f, t])?;
        }
    }
    tx.commit()?;
    Ok(n)
}

/// Takes files out of a manual album. Returns how many were in it.
pub fn remove(c: &mut Connection, id: i64, files: &[i64]) -> rusqlite::Result<usize> {
    let tx = c.transaction()?;
    let mut n = 0;
    {
        let mut st = tx.prepare_cached(
            "DELETE FROM collection_files WHERE collection_id = ?1 AND file_id = ?2",
        )?;
        for f in files {
            n += st.execute(params![id, f])?;
        }
    }
    tx.commit()?;
    Ok(n)
}

/// The manual albums of every file in one: full names, one per line, by name.
pub fn file_albums(c: &Connection) -> rusqlite::Result<HashMap<i64, String>> {
    let mut out: HashMap<i64, String> = HashMap::new();
    let mut st = c.prepare_cached(
        "SELECT cf.file_id, c.name FROM collection_files cf
         JOIN collections c ON c.id = cf.collection_id
         ORDER BY cf.file_id, c.name COLLATE NOCASE",
    )?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        let s = out.entry(r.get(0)?).or_default();
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(r.get_ref(1)?.as_str().unwrap_or_default());
    }
    Ok(out)
}

/// The catalogue ids of `paths` (those it holds), and the paths it does not hold.
pub fn ids_of(c: &Connection, paths: &[PathBuf]) -> rusqlite::Result<(Vec<i64>, Vec<PathBuf>)> {
    let mut ids = Vec::with_capacity(paths.len());
    let mut missing = Vec::new();
    for p in paths {
        match crate::edits::file_id(c, p)? {
            Some(id) => ids.push(id),
            None => missing.push(p.clone()),
        }
    }
    Ok((ids, missing))
}

/// An album by id.
pub fn get(c: &Connection, id: i64) -> rusqlite::Result<Option<Album>> {
    Ok(list(c)?.into_iter().find(|a| a.id == id))
}

/// Whether the album holds the file at `path`.
pub fn holds(c: &Connection, id: i64, path: &Path) -> rusqlite::Result<bool> {
    let Some(f) = crate::edits::file_id(c, path)? else {
        return Ok(false);
    };
    c.prepare_cached("SELECT 1 FROM collection_files WHERE collection_id = ?1 AND file_id = ?2")?
        .query_row(params![id, f], |_| Ok(()))
        .optional()
        .map(|o| o.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: i64, name: &str) -> Album {
        Album {
            id,
            name: name.into(),
            kind: Kind::Manual,
            rules: Rules::default(),
            files: 0,
        }
    }

    #[test]
    fn names_are_paths() {
        assert_eq!(
            normalize(" Client X / selects/").as_deref(),
            Some("Client X/selects")
        );
        assert_eq!(normalize(" / "), None);
        let a = album(1, "Client X/selects");
        assert_eq!(a.leaf(), "selects");
        assert_eq!(a.groups(), vec!["Client X"]);
        let all = [a, album(2, "Client XY"), album(3, "Other/x")];
        assert_eq!(
            name_problem(&all, "client x / SELECTS", None),
            Some(NameProblem::Taken("Client X/selects".into()))
        );
        assert_eq!(name_problem(&all, "Client X/selects", Some(1)), None);
        assert_eq!(name_problem(&all, "Client X/picks", None), None);
        assert_eq!(name_problem(&all, "  ", None), Some(NameProblem::Empty));
        // An album can't be a group as well.
        assert_eq!(
            name_problem(&all, "client x", None),
            Some(NameProblem::Clash("Client X/selects".into()))
        );
        assert_eq!(
            name_problem(&all, "Client XY/a", None),
            Some(NameProblem::Clash("Client XY".into()))
        );
        let g: Vec<i64> = in_group(&all, "client x").map(|a| a.id).collect();
        assert_eq!(g, vec![1]);
    }
}

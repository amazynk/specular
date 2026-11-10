//! Local copies of offline originals (the Library's "Keep a local copy"): while a file's
//! location is offline, reads of it go to the copy the app registered, so the viewer, the
//! gallery and Develop work on the copy while everything else (marks, keywords, edits,
//! the catalogue) keeps the original's path.
//!
//! The app sets one [`set_resolver`]; [`crate::Source::open`] and [`read`] ask it only when
//! the original cannot be read, so a reachable original is always what is read.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

/// Gives the copy standing for an original, if one is kept.
pub type Resolver = Arc<dyn Fn(&Path) -> Option<PathBuf> + Send + Sync>;

static RESOLVER: RwLock<Option<Resolver>> = RwLock::new(None);

/// Sets (or with `None` clears) the process's resolver.
pub fn set_resolver(r: Option<Resolver>) {
    *RESOLVER.write() = r;
}

/// The copy to read for `original`, when the original cannot be read and a copy is kept.
pub fn stand_in(original: &Path) -> Option<PathBuf> {
    let r = RESOLVER.read().clone()?;
    if std::fs::metadata(original).is_ok() {
        return None;
    }
    r(original).filter(|p| p != original)
}

/// `std::fs::read`, from the local copy when the original cannot be read.
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    match std::fs::read(path) {
        Ok(b) => Ok(b),
        Err(e) => match stand_in(path) {
            Some(c) => std::fs::read(c),
            None => Err(e),
        },
    }
}

/// Whether `path` would be read from a local copy now.
pub fn is_standing_in(path: &Path) -> bool {
    stand_in(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_stand_in_only_for_missing_originals() {
        let d = std::env::temp_dir().join(format!("specular-core-local-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let copy = d.join("copy.jpg");
        let here = d.join("here.jpg");
        std::fs::write(&copy, b"copy").unwrap();
        std::fs::write(&here, b"here").unwrap();
        let gone = d.join("gone/IMG.jpg");
        let (c, g) = (copy.clone(), gone.clone());
        // Answers for this test's paths only (other tests run alongside).
        set_resolver(Some(Arc::new(move |p: &Path| {
            (p == g || p == c.with_file_name("here.jpg")).then(|| c.clone())
        })));
        assert_eq!(read(&gone).unwrap(), b"copy");
        assert_eq!(read(&here).unwrap(), b"here", "a reachable original wins");
        assert!(is_standing_in(&gone) && !is_standing_in(&here));
        let s = crate::Source::open(&gone).unwrap();
        assert_eq!(s.bytes(), b"copy");
        set_resolver(None);
        assert!(read(&gone).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}

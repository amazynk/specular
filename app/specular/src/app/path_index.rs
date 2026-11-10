//! The viewer's list by path without a second copy of every path: each position is filed
//! under its path's hash, and a look-up compares against the list itself. A map keyed by
//! the paths took about 12 MB more at 100,000 files.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub(crate) struct PathIndex {
    hasher: RandomState,
    /// Path hash → position in the list (the last of equal paths).
    first: HashMap<u64, u32>,
    /// Further positions of hashes shared by different paths (rare).
    more: HashMap<u64, Vec<u32>>,
    /// Tests: every path gets the same hash.
    #[cfg(test)]
    collide: bool,
}

impl PathIndex {
    /// Indexes `list` (positions past `u32::MAX` are not indexed).
    pub(crate) fn new(list: &[PathBuf]) -> PathIndex {
        PathIndex {
            first: HashMap::with_capacity(list.len()),
            ..PathIndex::default()
        }
        .filled(list)
    }

    fn filled(mut self, list: &[PathBuf]) -> PathIndex {
        for (i, p) in list.iter().enumerate() {
            let Ok(i) = u32::try_from(i) else { break };
            let h = self.hash(p);
            match self.first.get(&h).copied() {
                None => {
                    self.first.insert(h, i);
                }
                // The same path again: the later position wins (as a map by path did).
                Some(j) if list[j as usize] == *p => {
                    self.first.insert(h, i);
                }
                Some(_) => {
                    let more = self.more.entry(h).or_default();
                    match more.iter_mut().find(|j| list[**j as usize] == *p) {
                        Some(j) => *j = i,
                        None => more.push(i),
                    }
                }
            }
        }
        self
    }

    fn hash(&self, p: &Path) -> u64 {
        #[cfg(test)]
        if self.collide {
            return 0;
        }
        self.hasher.hash_one(p)
    }

    /// The position of `p` in `list` (the list this index was made for).
    pub(crate) fn get(&self, list: &[PathBuf], p: &Path) -> Option<usize> {
        let h = self.hash(p);
        let at =
            |i: u32| (list.get(i as usize).map(PathBuf::as_path) == Some(p)).then_some(i as usize);
        at(*self.first.get(&h)?).or_else(|| self.more.get(&h)?.iter().find_map(|&j| at(j)))
    }

    pub(crate) fn contains(&self, list: &[PathBuf], p: &Path) -> bool {
        self.get(list, p).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(v: &[&str]) -> Vec<PathBuf> {
        v.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn finds_every_position() {
        let list: Vec<PathBuf> = (0..1000)
            .map(|i| PathBuf::from(format!("/d/{i}.jpg")))
            .collect();
        let ix = PathIndex::new(&list);
        for (i, p) in list.iter().enumerate() {
            assert_eq!(ix.get(&list, p), Some(i));
        }
        assert_eq!(ix.get(&list, Path::new("/d/1000.jpg")), None);
        assert!(!ix.contains(&list, Path::new("/e/1.jpg")));
        assert_eq!(PathIndex::new(&[]).get(&[], Path::new("/a")), None);
    }

    /// The last of equal paths, as a map from path to position gave.
    #[test]
    fn repeated_paths_take_the_last_position() {
        let list = paths(&["/a", "/b", "/a"]);
        assert_eq!(PathIndex::new(&list).get(&list, Path::new("/a")), Some(2));
    }

    /// Paths that share a hash are told apart by comparing with the list.
    #[test]
    fn hash_collisions_are_told_apart() {
        let list = paths(&["/a", "/b", "/c", "/b", "/a"]);
        let ix = PathIndex {
            collide: true,
            ..PathIndex::default()
        }
        .filled(&list);
        let pos = |p: &str| ix.get(&list, Path::new(p));
        assert_eq!(pos("/a"), Some(4));
        assert_eq!(pos("/b"), Some(3));
        assert_eq!(pos("/c"), Some(2));
        assert_eq!(pos("/d"), None);
    }
}

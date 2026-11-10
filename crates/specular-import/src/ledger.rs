//! The import ledger: every file imported before, keyed by its content key (not its name or
//! where it went), so it is skipped next time even if it was renamed or moved at the
//! destination. One line per file, appended and synced as each copy is verified:
//!
//! ```text
//! # specular import ledger 1
//! <key hex>\t<full hash hex>\t<size>\t<destination path>
//! ```
//!
//! Damaged lines are skipped (a crash mid-append loses at most that file's entry, which only
//! means it would be offered again).

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::{Hash, Key, hex, unhex};

/// The ledger's file name in the app's data folder.
pub const FILE_NAME: &str = "import-ledger.tsv";
const HEADER: &str = "# specular import ledger 1";

/// One imported file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// blake3 of the whole file, as verified after the copy.
    pub hash: Hash,
    pub size: u64,
    /// Where it was copied to (it may have been renamed or moved since).
    pub dest: PathBuf,
}

/// What was imported before.
#[derive(Debug, Default)]
pub struct Ledger {
    path: Option<PathBuf>,
    entries: HashMap<Key, Entry>,
}

fn escape(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

fn unescape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('t') => o.push('\t'),
                Some('n') => o.push('\n'),
                Some('r') => o.push('\r'),
                Some(c) => o.push(c),
                None => {}
            }
        } else {
            o.push(c);
        }
    }
    o
}

impl Ledger {
    /// A ledger that is not saved (tests, and when there is no data folder).
    pub fn in_memory() -> Ledger {
        Ledger::default()
    }

    /// The ledger at `path` (empty if the file does not exist yet).
    pub fn open(path: &Path) -> std::io::Result<Ledger> {
        let mut l = Ledger {
            path: Some(path.to_path_buf()),
            entries: HashMap::new(),
        };
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(l),
            Err(e) => return Err(e),
        };
        for line in BufReader::new(f).lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }
            let mut p = line.splitn(4, '\t');
            let (Some(k), Some(h), Some(n), Some(d)) = (p.next(), p.next(), p.next(), p.next())
            else {
                continue;
            };
            let (Some(k), Some(h), Ok(n)) = (unhex(k), unhex(h), n.parse()) else {
                continue;
            };
            l.entries.insert(
                k,
                Entry {
                    hash: h,
                    size: n,
                    dest: PathBuf::from(unescape(d)),
                },
            );
        }
        Ok(l)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, key: &Key) -> bool {
        self.entries.contains_key(key)
    }

    pub fn get(&self, key: &Key) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Records an imported file, appending it to the file and syncing before returning.
    pub fn record(&mut self, key: Key, entry: Entry) -> std::io::Result<()> {
        if self.entries.get(&key) == Some(&entry) {
            return Ok(());
        }
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut f = OpenOptions::new()
                .create(true)
                .read(true)
                .append(true)
                .open(path)?;
            let len = f.metadata()?.len();
            let mut line = String::new();
            if len == 0 {
                line.push_str(HEADER);
                line.push('\n');
            } else {
                // A line torn by a crash: start on a fresh one.
                let mut last = [0u8];
                f.seek(SeekFrom::Start(len - 1))?;
                f.read_exact(&mut last)?;
                if last[0] != b'\n' {
                    line.push('\n');
                }
            }
            line.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                hex(&key),
                hex(&entry.hash),
                entry.size,
                escape(&entry.dest)
            ));
            f.write_all(line.as_bytes())?;
            f.sync_data()?;
        }
        self.entries.insert(key, entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::fresh;

    #[test]
    fn persists_across_opens_and_skips_damage() {
        let dir = fresh("ledger");
        let path = dir.join("data").join(FILE_NAME);
        let mut l = Ledger::open(&path).unwrap();
        assert!(l.is_empty());
        let e = Entry {
            hash: [7; 32],
            size: 1234,
            dest: PathBuf::from("/pics/2024/a\tb\\c.jpg"),
        };
        l.record([1; 32], e.clone()).unwrap();
        l.record(
            [2; 32],
            Entry {
                size: 9,
                ..e.clone()
            },
        )
        .unwrap();
        drop(l);

        // A torn last line from a crash mid-append, and junk.
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"zz\tnot hex\n0303030303").unwrap();
        drop(f);

        let mut l = Ledger::open(&path).unwrap();
        assert_eq!(l.len(), 2);
        l.record([4; 32], e.clone()).unwrap();
        let l = Ledger::open(&path).unwrap();
        assert_eq!(l.len(), 3, "an entry after a torn line is kept");
        assert_eq!(l.get(&[1; 32]), Some(&e));
        assert_eq!(l.get(&[2; 32]).unwrap().size, 9);
        assert!(!l.contains(&[3; 32]));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(HEADER));
        let _ = std::fs::remove_dir_all(dir);
    }
}

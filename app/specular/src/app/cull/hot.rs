//! Hot folder: files that appear in the culled folder (a tethered camera, an import) join the
//! run once they are complete. A file being written keeps growing or being touched, so it is
//! taken only after its size and modification time stayed the same for [`QUIET`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// How long a new file must stay unchanged before it is shown.
pub(crate) const QUIET: Duration = Duration::from_millis(600);
/// How often settling files are looked at again.
pub(crate) const POLL: Duration = Duration::from_millis(200);

/// What a look at a file says: its size and modification time.
pub(crate) type Stat = (u64, Option<SystemTime>);

pub(crate) fn stat(p: &Path) -> Option<Stat> {
    let m = std::fs::metadata(p).ok()?;
    m.is_file().then(|| (m.len(), m.modified().ok()))
}

struct Seen {
    stat: Option<Stat>,
    since: Instant,
}

/// New files waiting to be complete.
#[derive(Default)]
pub(crate) struct Settle {
    pending: HashMap<PathBuf, Seen>,
    next: Option<Instant>,
}

impl Settle {
    /// A file appeared or changed (again).
    pub fn saw(&mut self, p: PathBuf, now: Instant) {
        self.pending.entry(p).or_insert(Seen {
            stat: None,
            since: now,
        });
        self.next.get_or_insert(now);
    }

    /// When to call [`Settle::poll`] next.
    pub fn due(&self) -> Option<Instant> {
        self.next.filter(|_| !self.pending.is_empty())
    }

    /// Looks at the waiting files (when due): the ones complete now, in name order. A file
    /// that vanished is forgotten; an empty one waits (it is being created).
    pub fn poll(&mut self, now: Instant, stat: impl Fn(&Path) -> Option<Stat>) -> Vec<PathBuf> {
        if self.next.is_none_or(|t| now < t) {
            return Vec::new();
        }
        let mut ready = Vec::new();
        self.pending.retain(|p, seen| {
            let Some(s) = stat(p) else {
                return false;
            };
            if seen.stat != Some(s) {
                seen.stat = Some(s);
                seen.since = now;
                return true;
            }
            if s.0 > 0 && now.duration_since(seen.since) >= QUIET {
                ready.push(p.clone());
                return false;
            }
            true
        });
        ready.sort();
        self.next = (!self.pending.is_empty()).then_some(now + POLL);
        ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn a_file_is_taken_once_it_stops_changing() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let sizes: RefCell<HashMap<PathBuf, u64>> = RefCell::new(HashMap::new());
        let st = |p: &Path| sizes.borrow().get(p).map(|&n| (n, None));
        let a = PathBuf::from("/hot/a.jpg");
        let b = PathBuf::from("/hot/b.cr3");
        let mut s = Settle::default();
        assert!(s.due().is_none());
        sizes.borrow_mut().insert(a.clone(), 0);
        s.saw(a.clone(), ms(0));
        assert_eq!(s.due(), Some(ms(0)));
        // Created empty, then growing.
        assert!(s.poll(ms(0), st).is_empty());
        sizes.borrow_mut().insert(a.clone(), 4096);
        assert!(s.poll(ms(200), st).is_empty());
        sizes.borrow_mut().insert(a.clone(), 9000);
        assert!(s.poll(ms(400), st).is_empty());
        // Not due yet: nothing is looked at.
        assert!(s.poll(ms(450), st).is_empty());
        sizes.borrow_mut().insert(b.clone(), 100);
        s.saw(b.clone(), ms(500));
        assert!(s.poll(ms(600), st).is_empty());
        // `a` unchanged since 400 is taken at the first look after 1000; `b`, unchanged
        // since 600, after 1200.
        assert!(s.poll(ms(900), st).is_empty());
        assert!(s.poll(ms(1000), st).is_empty());
        assert_eq!(s.poll(ms(1100), st), vec![a.clone()]);
        assert_eq!(s.poll(ms(1300), st), vec![b.clone()]);
        assert!(s.pending.is_empty());
        assert!(s.due().is_none());
    }

    #[test]
    fn vanished_and_empty_files() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut s = Settle::default();
        s.saw(PathBuf::from("/hot/gone.jpg"), ms(0));
        s.saw(PathBuf::from("/hot/empty.jpg"), ms(0));
        let st = |p: &Path| p.ends_with("empty.jpg").then_some((0, None));
        assert!(s.poll(ms(0), st).is_empty());
        assert!(s.poll(ms(5000), st).is_empty());
        // The empty one still waits; the vanished one is forgotten.
        assert!(!s.pending.is_empty());
        assert_eq!(s.pending.len(), 1);
    }

    #[test]
    fn stat_reads_size_of_files_only() {
        let d = std::env::var_os("SPECULAR_TEST_TMP")
            .map_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-tmp"),
                PathBuf::from,
            )
            .join(format!("cull-hot-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("x.jpg"), b"12345").unwrap();
        assert_eq!(stat(&d.join("x.jpg")).map(|s| s.0), Some(5));
        assert!(stat(&d).is_none());
        assert!(stat(&d.join("missing.jpg")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}

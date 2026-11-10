//! Files still being filled: a network stream's web file played while it is fetched.
//!
//! The stream layer makes the file at its full length (a sparse file: the parts not yet
//! fetched read as zeros) and registers a [`Filling`] for its path, which records the byte
//! ranges written so far. A [`crate::Source`] opened on that path keeps the filling with its
//! mapping, and the demuxers ask [`wait`] before they read a sample's bytes: a read of a part
//! not there yet blocks until the fetcher writes it (the fetcher sees the wanted range and
//! fetches it first), or fails after the filling's timeout, never reading zeros.
//!
//! Nothing registered (every ordinary file): [`wait`] is one atomic load.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

/// Readers followed at most (picture, sound, subtitles, a scan).
const READERS: usize = 8;

/// A file being filled: which of its bytes are there.
pub struct Filling {
    len: u64,
    timeout: Duration,
    st: Mutex<State>,
    cv: Condvar,
}

#[derive(Default)]
struct State {
    /// Sorted, disjoint, non-adjacent ranges written.
    have: Vec<(u64, u64)>,
    /// What a blocked reader waits for (the latest).
    want: Option<(u64, u64)>,
    /// Where each reader read last (its end), the latest last.
    readers: Vec<(ThreadId, u64)>,
    /// Readers blocked now.
    waiting: usize,
    /// No more bytes will come (stopped or failed): waits fail at once.
    closed: bool,
    /// A wait timed out or failed.
    stalled: bool,
    /// Bumped by every change (the fetcher's wake-up check).
    changes: u64,
}

impl State {
    /// The end of what is there from `at` on (`at` itself when it is a hole).
    fn filled_to(&self, at: u64) -> u64 {
        let i = self.have.partition_point(|&(_, e)| e <= at);
        match self.have.get(i) {
            Some(&(s, e)) if s <= at => e,
            _ => at,
        }
    }

    fn has(&self, start: u64, end: u64) -> bool {
        start >= end || self.filled_to(start) >= end
    }

    /// The first hole at or after `at`, before `len`.
    fn hole_after(&self, at: u64, len: u64) -> Option<(u64, u64)> {
        let s = self.filled_to(at);
        if s >= len {
            return None;
        }
        let i = self.have.partition_point(|&(b, _)| b <= s);
        let e = self.have.get(i).map_or(len, |&(b, _)| b.min(len));
        Some((s, e))
    }
}

impl Filling {
    /// A file of `len` bytes, none of them there yet; reads wait up to `timeout`.
    pub fn new(len: u64, timeout: Duration) -> Arc<Filling> {
        Arc::new(Filling {
            len,
            timeout,
            st: Mutex::new(State::default()),
            cv: Condvar::new(),
        })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes `start..end` were written.
    pub fn add(&self, start: u64, end: u64) {
        let end = end.min(self.len);
        if start >= end {
            return;
        }
        let mut st = self.st.lock();
        let (mut s, mut e) = (start, end);
        // Merge with every range touching it.
        let first = st.have.partition_point(|&(_, b)| b < s);
        let mut last = first;
        while last < st.have.len() && st.have[last].0 <= e {
            s = s.min(st.have[last].0);
            e = e.max(st.have[last].1);
            last += 1;
        }
        st.have.splice(first..last, [(s, e)]);
        if st.want.is_some_and(|(a, b)| st.has(a, b)) {
            st.want = None;
        }
        st.changes += 1;
        drop(st);
        self.cv.notify_all();
    }

    /// Whether bytes `start..end` are there.
    pub fn has(&self, start: u64, end: u64) -> bool {
        self.st.lock().has(start, end.min(self.len))
    }

    /// Bytes there in all.
    pub fn filled(&self) -> u64 {
        self.st.lock().have.iter().map(|&(s, e)| e - s).sum()
    }

    /// Every byte is there.
    pub fn complete(&self) -> bool {
        self.st.lock().has(0, self.len)
    }

    /// No more bytes will come: blocked and later reads of missing parts fail at once.
    pub fn close(&self) {
        let mut st = self.st.lock();
        st.closed = true;
        st.changes += 1;
        drop(st);
        self.cv.notify_all();
    }

    /// A read waited longer than the timeout (or the filling was closed under it).
    pub fn stalled(&self) -> bool {
        self.st.lock().stalled
    }

    /// A reader is blocked now (buffering).
    pub fn waiting(&self) -> bool {
        self.st.lock().waiting > 0
    }

    /// What a blocked reader waits for, if one does.
    pub fn wanted(&self) -> Option<(u64, u64)> {
        self.st.lock().want
    }

    /// What to fetch next, at most `run` bytes: what a blocked reader waits for; else the
    /// hole after the reader with the least read ahead; else the first hole. `None` when
    /// the file is whole.
    pub fn next_fetch(&self, run: u64) -> Option<(u64, u64)> {
        let st = self.st.lock();
        let cap = |(s, e): (u64, u64)| (s, e.min(s.saturating_add(run.max(1))));
        if let Some((a, _)) = st.want
            && let Some(h) = st.hole_after(a, self.len)
        {
            return Some(cap(h));
        }
        let mut best: Option<(u64, (u64, u64))> = None;
        for &(_, at) in &st.readers {
            if let Some(h) = st.hole_after(at, self.len) {
                let ahead = h.0 - at;
                if best.is_none_or(|(b, _)| ahead < b) {
                    best = Some((ahead, h));
                }
            }
        }
        best.map(|(_, h)| h)
            .or_else(|| st.hole_after(0, self.len))
            .map(cap)
    }

    /// Waits up to `d` for a reader to ask for something or for the filling to close.
    /// Whether something changed.
    pub fn wait_change(&self, d: Duration) -> bool {
        let mut st = self.st.lock();
        let seen = st.changes;
        if st.want.is_some() || st.closed {
            return true;
        }
        self.cv.wait_for(&mut st, d);
        st.changes != seen
    }

    /// Waits up to `d` until bytes `start..end` are there, asking the fetcher for them, for a
    /// side read (a fingerprint): not followed as a reader, and a wait that runs out is not
    /// a stall. False when they did not come in time or the filling was closed.
    pub fn wait_quietly(&self, start: u64, end: u64, d: Duration) -> bool {
        let end = end.min(self.len);
        let mut st = self.st.lock();
        if st.has(start, end) {
            return true;
        }
        let deadline = Instant::now() + d;
        let ok = loop {
            if st.has(start, end) {
                break true;
            }
            if st.closed {
                break false;
            }
            if st.want.is_none() {
                // Asked when no reader is blocked (a blocked reader's part comes first).
                st.want = Some((start, end));
                st.changes += 1;
                self.cv.notify_all();
            }
            if self.cv.wait_until(&mut st, deadline).timed_out() {
                break st.has(start, end);
            }
        };
        if st.want == Some((start, end)) {
            st.want = None;
        }
        ok
    }

    /// Waits until bytes `start..end` are there: false when they did not come within the
    /// timeout or the filling was closed.
    pub fn wait(&self, start: u64, end: u64) -> bool {
        let end = end.min(self.len);
        let mut st = self.st.lock();
        let me = std::thread::current().id();
        st.readers.retain(|&(t, _)| t != me);
        st.readers.push((me, start));
        if st.readers.len() > READERS {
            st.readers.remove(0);
        }
        if st.has(start, end) {
            if let Some(r) = st.readers.last_mut() {
                r.1 = end;
            }
            return true;
        }
        if st.closed {
            st.stalled = true;
            return false;
        }
        st.want = Some((start, end));
        st.waiting += 1;
        st.changes += 1;
        self.cv.notify_all();
        let deadline = Instant::now() + self.timeout;
        let ok = loop {
            if st.has(start, end) {
                break true;
            }
            if st.closed || self.cv.wait_until(&mut st, deadline).timed_out() {
                break st.has(start, end);
            }
            if st.want.is_none() && !st.has(start, end) {
                // Another reader's range came: ask again for this one.
                st.want = Some((start, end));
                self.cv.notify_all();
            }
        };
        st.waiting -= 1;
        if ok {
            if let Some(r) = st.readers.iter_mut().find(|r| r.0 == me) {
                r.1 = end;
            }
        } else {
            st.stalled = true;
            if st.want == Some((start, end)) {
                st.want = None;
            }
        }
        st.changes += 1;
        drop(st);
        self.cv.notify_all();
        ok
    }
}

/// Paths being filled, by their absolute form.
static PATHS: Mutex<Vec<(PathBuf, Weak<Filling>)>> = Mutex::new(Vec::new());
/// Mapped sources of files being filled: (start address, length).
static MAPS: Mutex<Vec<(usize, usize, Arc<Filling>)>> = Mutex::new(Vec::new());
static MAPPED: AtomicUsize = AtomicUsize::new(0);

fn key(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Keeps `path` registered as being filled while it lives.
pub struct Registered(PathBuf);

impl Drop for Registered {
    fn drop(&mut self) {
        PATHS
            .lock()
            .retain(|(p, w)| p != &self.0 && w.strong_count() > 0);
    }
}

/// Registers `path` as being filled by `f`: sources opened on it wait for its bytes.
pub fn register(path: &Path, f: &Arc<Filling>) -> Registered {
    let k = key(path);
    let mut paths = PATHS.lock();
    paths.retain(|(p, w)| p != &k && w.strong_count() > 0);
    paths.push((k.clone(), Arc::downgrade(f)));
    Registered(k)
}

/// The filling of `path`, when it is being filled.
pub fn of_path(path: &Path) -> Option<Arc<Filling>> {
    let paths = PATHS.lock();
    if paths.is_empty() {
        return None;
    }
    let k = key(path);
    paths
        .iter()
        .find(|(p, _)| *p == k)
        .and_then(|(_, w)| w.upgrade())
}

/// Keeps a mapping known to [`wait`] while it lives.
pub(crate) struct Mapped(usize);

impl Drop for Mapped {
    fn drop(&mut self) {
        let mut maps = MAPS.lock();
        if let Some(i) = maps.iter().position(|m| m.0 == self.0) {
            maps.swap_remove(i);
            MAPPED.fetch_sub(1, Ordering::Release);
        }
    }
}

/// `bytes` (a mapping of a file being filled) read through `f`.
pub(crate) fn map(bytes: &[u8], f: Arc<Filling>) -> Mapped {
    let base = bytes.as_ptr() as usize;
    MAPS.lock().push((base, bytes.len(), f));
    MAPPED.fetch_add(1, Ordering::Release);
    Mapped(base)
}

/// Whether `data` lies in a file still being filled (some of its bytes may not be there).
pub fn pending(data: &[u8]) -> bool {
    if MAPPED.load(Ordering::Acquire) == 0 {
        return false;
    }
    let p = data.as_ptr() as usize;
    let f = MAPS
        .lock()
        .iter()
        .find(|m| p >= m.0 && p < m.0 + m.1.max(1))
        .map(|m| Arc::clone(&m.2));
    f.is_some_and(|f| !f.complete())
}

/// Waits until `range` of `data` is there when `data` lies in a file being filled
/// (otherwise true at once). False when the bytes did not come in time: the caller treats
/// them as missing.
pub fn wait(data: &[u8], range: Range<usize>) -> bool {
    if MAPPED.load(Ordering::Acquire) == 0 || range.is_empty() {
        return true;
    }
    let p = data.as_ptr() as usize;
    let found = MAPS
        .lock()
        .iter()
        .find(|m| p >= m.0 && p < m.0 + m.1.max(1))
        .map(|m| (p - m.0, Arc::clone(&m.2)));
    match found {
        Some((off, f)) => f.wait((off + range.start) as u64, (off + range.end) as u64),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_merge_and_holes_are_found() {
        let f = Filling::new(100, Duration::from_millis(10));
        f.add(10, 20);
        f.add(30, 40);
        assert!(f.has(10, 20) && !f.has(10, 21) && f.has(5, 5));
        assert_eq!(f.next_fetch(1000), Some((0, 10)));
        f.add(0, 10);
        assert_eq!(f.next_fetch(5), Some((20, 25)));
        f.add(20, 30);
        assert!(f.has(0, 40));
        assert_eq!(f.filled(), 40);
        f.add(35, 100);
        assert!(f.complete());
        assert_eq!(f.next_fetch(10), None);
    }

    #[test]
    fn a_missing_part_blocks_then_times_out_or_arrives() {
        let f = Filling::new(1000, Duration::from_millis(50));
        let t0 = Instant::now();
        assert!(!f.wait(100, 200), "nothing there: times out");
        assert!(t0.elapsed() >= Duration::from_millis(50));
        assert!(f.stalled());
        let f2 = Filling::new(1000, Duration::from_secs(5));
        let g2 = Arc::clone(&f2);
        let filler = specular_thread::spawn("test-fill", move || {
            while g2.wanted().is_none() {
                g2.wait_change(Duration::from_millis(5));
            }
            assert_eq!(g2.next_fetch(1 << 20), Some((500, 1000)));
            g2.add(500, 700);
        });
        assert!(f2.wait(500, 600));
        filler.join().unwrap();
        // The reader read to 600: the next fetch is the hole after where it reads.
        assert_eq!(f2.next_fetch(50), Some((700, 750)));
        f2.close();
        assert!(!f2.wait(0, 10), "closed: fails at once");
    }

    #[test]
    fn a_side_read_asks_without_stalling_or_leading() {
        let f = Filling::new(1000, Duration::from_secs(5));
        assert!(!f.wait_quietly(900, 1000, Duration::from_millis(20)));
        assert!(!f.stalled(), "a side read running out is no stall");
        assert_eq!(f.wanted(), None, "nor asked for any longer");
        let g = Arc::clone(&f);
        let filler = specular_thread::spawn("test-fill-side", move || {
            while g.wanted().is_none() {
                g.wait_change(Duration::from_millis(5));
            }
            assert_eq!(g.next_fetch(1 << 20), Some((900, 1000)));
            g.add(900, 1000);
        });
        assert!(f.wait_quietly(900, 1000, Duration::from_secs(5)));
        filler.join().unwrap();
        // Not followed as a reader: the next fetch is still the first hole.
        assert_eq!(f.next_fetch(10), Some((0, 10)));
    }

    #[test]
    fn mapped_bytes_wait_through_the_registry() {
        let data = vec![0u8; 64];
        assert!(wait(&data, 0..10), "nothing registered");
        let f = Filling::new(64, Duration::from_millis(20));
        f.add(0, 32);
        let m = map(&data, Arc::clone(&f));
        assert!(wait(&data, 0..32));
        assert!(wait(&data[8..], 0..24));
        assert!(pending(&data[8..]));
        assert!(!wait(&data[8..], 20..30), "crosses into the hole");
        drop(m);
        assert!(wait(&data, 40..50), "unmapped: not followed");
        // Only a key: nothing is written there.
        let p = Path::new("fill-registry-test/movie.mp4");
        assert!(of_path(p).is_none());
        let r = register(p, &f);
        assert!(of_path(p).is_some_and(|g| Arc::ptr_eq(&g, &f)));
        drop(r);
        assert!(of_path(p).is_none());
    }
}

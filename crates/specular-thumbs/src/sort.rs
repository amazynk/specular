//! Gallery ordering. Folders (and archives) come first unless sorting by type; ties fall back
//! to the name order, so the order is total and stable.

use std::cmp::Ordering;

use crate::list::{Entry, EntryKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SortKey {
    #[default]
    Name,
    DateTaken,
    Modified,
    Size,
    Type,
    Dimensions,
}

impl SortKey {
    pub const ALL: [SortKey; 6] = [
        SortKey::Name,
        SortKey::DateTaken,
        SortKey::Modified,
        SortKey::Size,
        SortKey::Type,
        SortKey::Dimensions,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SortKey::Name => "natural",
            SortKey::DateTaken => "date_taken",
            SortKey::Modified => "modified",
            SortKey::Size => "size",
            SortKey::Type => "type",
            SortKey::Dimensions => "dimensions",
        }
    }

    pub fn from_id(s: &str) -> Option<SortKey> {
        SortKey::ALL.into_iter().find(|k| k.id() == s)
    }

    /// Needs a per-file probe (header or EXIF) before the order is final.
    pub fn needs_probe(self) -> bool {
        matches!(self, SortKey::DateTaken | SortKey::Dimensions)
    }
}

/// Sorts `entries`. `name_cmp` is the locale's natural comparison.
pub fn sort(
    entries: &mut [Entry],
    key: SortKey,
    descending: bool,
    name_cmp: &dyn Fn(&str, &str) -> Ordering,
) {
    entries.sort_by(|a, b| compare(a, b, key, descending, name_cmp));
}

pub fn compare(
    a: &Entry,
    b: &Entry,
    key: SortKey,
    descending: bool,
    name_cmp: &dyn Fn(&str, &str) -> Ordering,
) -> Ordering {
    compare_by(a, b, key, descending, name_cmp, || {
        name_cmp(&a.name, &b.name)
    })
}

/// [`compare`] with the name order given by `by_name` (`a` against `b`), for callers that
/// compare precomputed sort keys rather than the names themselves.
pub fn compare_by(
    a: &Entry,
    b: &Entry,
    key: SortKey,
    descending: bool,
    name_cmp: &dyn Fn(&str, &str) -> Ordering,
    by_name: impl FnOnce() -> Ordering,
) -> Ordering {
    // Containers first, always (in both directions), like file managers.
    let ca = a.is_container();
    let cb = b.is_container();
    if ca != cb {
        return cb.cmp(&ca);
    }
    let primary = match key {
        SortKey::Name => Ordering::Equal,
        SortKey::Modified => a.modified.cmp(&b.modified),
        SortKey::Size => a.len.cmp(&b.len),
        SortKey::Type => kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then_with(|| name_cmp(&a.ext(), &b.ext())),
        // Unknown dates sort after known ones; files without EXIF use the modification time.
        SortKey::DateTaken => taken(a).cmp(&taken(b)),
        SortKey::Dimensions => pixels(a).cmp(&pixels(b)),
    };
    let o = primary.then_with(by_name);
    if descending { o.reverse() } else { o }
}

fn kind_rank(k: EntryKind) -> u8 {
    match k {
        EntryKind::Folder => 0,
        EntryKind::Archive => 1,
        EntryKind::Image => 2,
        EntryKind::Other => 3,
    }
}

fn taken(e: &Entry) -> Option<i64> {
    e.taken.or_else(|| {
        e.modified
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    })
}

fn pixels(e: &Entry) -> u64 {
    e.dims.map_or(0, |(w, h)| w as u64 * h as u64)
}

/// Natural order without a locale: digit runs compare by value ("img2" < "img10"), the rest
/// case-insensitively. The app passes the locale-aware comparison instead.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = x.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    x.next();
                }
                let mut nb = String::new();
                while let Some(d) = y.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(d);
                    y.next();
                }
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Below this many items [`par_sort_unstable_by`] sorts on the calling thread.
const PAR_MIN: usize = 16 * 1024;

/// `v.sort_unstable_by(cmp)` on up to `threads` threads: the parts are sorted side by side,
/// then merged pairwise (also side by side). For large folders, where one thread needs ~30 ms
/// for 100 000 names. `cmp` must be a total order for the result to match the serial sort
/// exactly (the gallery's is: names are unique).
pub fn par_sort_unstable_by<T, F>(v: &mut Vec<T>, threads: usize, cmp: F)
where
    T: Copy + Send + Sync,
    F: Fn(&T, &T) -> Ordering + Sync,
{
    let parts = threads.clamp(1, 8).min(v.len() / (PAR_MIN / 2)).max(1);
    if parts == 1 || v.len() < PAR_MIN {
        v.sort_unstable_by(cmp);
        return;
    }
    let step = v.len().div_ceil(parts);
    let cmp = &cmp;
    std::thread::scope(|s| {
        for (wi, part) in v.chunks_mut(step).enumerate() {
            specular_thread::spawn_scoped(s, format!("thumb-sort-{wi}"), move || {
                part.sort_unstable_by(cmp)
            });
        }
    });
    // Merge runs of `run` items pairwise until one is left.
    let mut src = std::mem::take(v);
    let mut dst: Vec<T> = Vec::with_capacity(src.len());
    let mut run = step;
    while run < src.len() {
        dst.clear();
        let pieces: Vec<&[T]> = src.chunks(run * 2).collect();
        let mut outs: Vec<Vec<T>> = Vec::with_capacity(pieces.len());
        std::thread::scope(|s| {
            let hs: Vec<_> = pieces
                .iter()
                .enumerate()
                .map(|(wi, p)| {
                    specular_thread::spawn_scoped(s, format!("thumb-merge-{wi}"), move || {
                        let (a, b) = p.split_at(run.min(p.len()));
                        merge(a, b, cmp)
                    })
                })
                .collect();
            // A panic goes on to the caller's boundary (the listing fails and says so),
            // never a result with entries silently left out.
            outs.extend(
                hs.into_iter()
                    .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p))),
            );
        });
        for o in outs {
            dst.extend_from_slice(&o);
        }
        std::mem::swap(&mut src, &mut dst);
        run *= 2;
    }
    *v = src;
}

/// Merges two sorted slices (ties from `a` first).
fn merge<T: Copy>(a: &[T], b: &[T], cmp: &impl Fn(&T, &T) -> Ordering) -> Vec<T> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if cmp(&b[j], &a[i]) == Ordering::Less {
            out.push(b[j]);
            j += 1;
        } else {
            out.push(a[i]);
            i += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    fn e(name: &str, kind: EntryKind, len: u64, mtime: u64) -> Entry {
        Entry {
            path: PathBuf::from(name),
            name: name.into(),
            kind,
            len,
            modified: Some(UNIX_EPOCH + Duration::from_secs(mtime)),
            hidden: false,
            taken: None,
            dims: None,
        }
    }

    fn names(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn orders() {
        let mut v = vec![
            e("img10.jpg", EntryKind::Image, 30, 3),
            e("img2.png", EntryKind::Image, 10, 1),
            e("Photos", EntryKind::Folder, 0, 9),
            e("img1.jpg", EntryKind::Image, 20, 2),
        ];
        sort(&mut v, SortKey::Name, false, &natural_cmp);
        assert_eq!(names(&v), ["Photos", "img1.jpg", "img2.png", "img10.jpg"]);
        sort(&mut v, SortKey::Name, true, &natural_cmp);
        assert_eq!(names(&v), ["Photos", "img10.jpg", "img2.png", "img1.jpg"]);
        sort(&mut v, SortKey::Size, false, &natural_cmp);
        assert_eq!(names(&v), ["Photos", "img2.png", "img1.jpg", "img10.jpg"]);
        sort(&mut v, SortKey::Modified, true, &natural_cmp);
        assert_eq!(names(&v), ["Photos", "img10.jpg", "img1.jpg", "img2.png"]);
        sort(&mut v, SortKey::Type, false, &natural_cmp);
        assert_eq!(names(&v), ["Photos", "img1.jpg", "img10.jpg", "img2.png"]);
        v[1].taken = Some(5);
        v[2].dims = Some((100, 100));
        sort(&mut v, SortKey::DateTaken, false, &natural_cmp);
        // img1 has no EXIF date (mtime 2), img10 mtime 3 → before the one taken at 5.
        assert_eq!(names(&v), ["Photos", "img2.png", "img10.jpg", "img1.jpg"]);
        sort(&mut v, SortKey::Dimensions, true, &natural_cmp);
        assert_eq!(v[1].dims, Some((100, 100)));
    }

    #[test]
    fn parallel_sort_matches_serial() {
        // A cheap deterministic shuffle; sizes around the thresholds and uneven parts.
        for n in [0, 1, 7, PAR_MIN - 1, PAR_MIN, 50_001, 100_000] {
            let v: Vec<u64> = (0..n as u64)
                .map(|i| i.wrapping_mul(2_654_435_761) % 1_000_003)
                .collect();
            for threads in [1, 2, 3, 4, 8] {
                let mut a = v.clone();
                let mut b = v.clone();
                a.sort_unstable();
                par_sort_unstable_by(&mut b, threads, |x, y| x.cmp(y));
                assert_eq!(a, b, "n {n} threads {threads}");
            }
        }
    }

    #[test]
    fn natural() {
        assert_eq!(natural_cmp("a2", "a10"), Ordering::Less);
        assert_ne!(natural_cmp("a010", "a10"), Ordering::Equal);
        assert_eq!(natural_cmp("B", "a"), Ordering::Greater);
        assert_eq!(natural_cmp("x", "x1"), Ordering::Less);
        assert_eq!(SortKey::from_id("date_taken"), Some(SortKey::DateTaken));
    }
}

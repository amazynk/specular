//! Column checkpoints learned by region decodes.
//!
//! A [`RowIndex`](crate::RowIndex) has a decoder checkpoint at the start of every scan row, so a
//! region decode only entropy-decodes the region's rows, but every row from its start up to the
//! region's right edge: on a wide panorama a 512-pixel tile in the middle parses half the
//! row. Region decodes therefore record the decoder state every [`STEP_PX`] pixels along the
//! rows they parse, and later regions in the same rows (a pan, the next tile, a zoom) start at
//! the nearest recorded column instead of the row start. The states are exact: they are what a
//! parse from the row start reaches.
//!
//! Entry `i` of a row is the state before unit `(i + 1) * step` (units are MCUs for interleaved
//! scans, blocks of the scan's component otherwise). Rows only grow by contiguous extension,
//! from the row start or from a recorded entry.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::marker::Parsed;
use crate::sequential::Checkpoint;

/// Output-pixel spacing (at full scale) of the recorded column checkpoints.
pub(crate) const STEP_PX: u32 = 256;

pub(crate) struct ColCache {
    /// Per scan: first row slot in `rows`, and units per checkpoint.
    scans: Vec<(usize, u32)>,
    rows: Vec<Mutex<Vec<Checkpoint>>>,
    entries: AtomicUsize,
}

impl ColCache {
    /// Cache for the single scan of a sequential file (`rows` MCU rows of MCUs).
    pub fn sequential(p: &Parsed) -> ColCache {
        let f = &p.frame;
        let step = (STEP_PX / (8 * f.max_h)).max(1);
        Self::new(&[(f.mcus_y, step)])
    }

    /// Cache for a coefficient-path index with `rows[si]` rows for scan `si`.
    pub fn scans(p: &Parsed, rows: &[usize]) -> ColCache {
        let f = &p.frame;
        let per: Vec<(u32, u32)> = p
            .scans
            .iter()
            .zip(rows)
            .map(|(s, &n)| {
                let step = match s.comps.as_slice() {
                    [one] => {
                        let h = f.comps.get(one.ci).map_or(1, |c| c.h);
                        STEP_PX * h / (8 * f.max_h)
                    }
                    _ => STEP_PX / (8 * f.max_h),
                };
                (n as u32, step.max(1))
            })
            .collect();
        Self::new(&per)
    }

    fn new(per: &[(u32, u32)]) -> ColCache {
        let mut scans = Vec::with_capacity(per.len());
        let mut n = 0usize;
        for &(rows, step) in per {
            scans.push((n, step));
            n += rows as usize;
        }
        ColCache {
            scans,
            rows: (0..n).map(|_| Mutex::new(Vec::new())).collect(),
            entries: AtomicUsize::new(0),
        }
    }

    /// Same shape, nothing recorded.
    pub fn empty_like(&self) -> ColCache {
        ColCache {
            scans: self.scans.clone(),
            rows: (0..self.rows.len())
                .map(|_| Mutex::new(Vec::new()))
                .collect(),
            entries: AtomicUsize::new(0),
        }
    }

    /// Units between checkpoints of scan `si` (0 if the scan is unknown).
    pub fn step(&self, si: usize) -> u32 {
        self.scans.get(si).map_or(0, |s| s.1)
    }

    fn row(&self, si: usize, row: u32) -> Option<&Mutex<Vec<Checkpoint>>> {
        let &(base, _) = self.scans.get(si)?;
        let next = self.scans.get(si + 1).map_or(self.rows.len(), |s| s.0);
        let i = base + row as usize;
        if i < next { self.rows.get(i) } else { None }
    }

    /// The recorded state at the last checkpoint column `<= unit` of `row` of scan `si`.
    pub fn best(&self, si: usize, row: u32, unit: u32) -> Option<(u32, Checkpoint)> {
        let step = self.step(si);
        if step == 0 || unit < step {
            return None;
        }
        let r = self.row(si, row)?.lock().unwrap_or_else(|e| e.into_inner());
        let i = ((unit / step) as usize).min(r.len());
        (i > 0).then(|| (i as u32 * step, r[i - 1]))
    }

    /// Adds the states before units `(first + j + 1) * step` (`cps[j]`) to `row`, if they extend
    /// it contiguously.
    pub fn extend(&self, si: usize, row: u32, first: usize, cps: &[Checkpoint]) {
        if cps.is_empty() {
            return;
        }
        let Some(m) = self.row(si, row) else {
            return;
        };
        let mut r = m.lock().unwrap_or_else(|e| e.into_inner());
        let have = r.len();
        if have < first || have >= first + cps.len() {
            return;
        }
        r.extend_from_slice(&cps[have - first..]);
        self.entries
            .fetch_add(first + cps.len() - have, Ordering::Relaxed);
    }

    /// Checkpoints recorded so far.
    pub fn len(&self) -> usize {
        self.entries.load(Ordering::Relaxed)
    }
}

/// Records the decoder state at every checkpoint column of one row while it is parsed.
pub(crate) struct ColRec {
    step: u32,
    /// Next unit to record at.
    next: u32,
    /// Entry index of the first recorded state.
    first: usize,
    pub cps: Vec<Checkpoint>,
}

impl ColRec {
    /// Recording for a row parsed from unit `from` (0 or a checkpoint column) with `step`.
    pub fn new(step: u32, from: u32) -> ColRec {
        let step = step.max(1);
        let next = (from / step + 1) * step;
        ColRec {
            step,
            next,
            first: (next / step) as usize - 1,
            cps: Vec::new(),
        }
    }

    /// Reuse for another row.
    pub fn reset(&mut self, from: u32) {
        self.next = (from / self.step + 1) * self.step;
        self.first = (self.next / self.step) as usize - 1;
        self.cps.clear();
    }

    /// Call before parsing `unit` (before its restart handling).
    #[inline(always)]
    pub fn at(&mut self, unit: u32, cp: impl FnOnce() -> Checkpoint) {
        if unit == self.next {
            self.cps.push(cp());
            self.next += self.step;
        }
    }

    pub fn flush(&mut self, cache: &ColCache, si: usize, row: u32) {
        cache.extend(si, row, self.first, &self.cps);
        self.cps.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(pos: usize) -> Checkpoint {
        Checkpoint::at(pos, 0)
    }

    #[test]
    fn extend_and_best() {
        let c = ColCache::new(&[(2, 4), (3, 2)]);
        assert!(c.best(0, 0, 100).is_none());
        // Not contiguous: ignored.
        c.extend(0, 0, 1, &[cp(1)]);
        assert!(c.best(0, 0, 100).is_none());
        c.extend(0, 0, 0, &[cp(10), cp(20)]);
        assert_eq!(c.best(0, 0, 3), None);
        assert_eq!(c.best(0, 0, 4).map(|x| (x.0, x.1.pos)), Some((4, 10)));
        assert_eq!(c.best(0, 0, 11).map(|x| (x.0, x.1.pos)), Some((8, 20)));
        // Overlapping extension adds only the new tail.
        c.extend(0, 0, 1, &[cp(20), cp(30)]);
        assert_eq!(c.best(0, 0, 100).map(|x| (x.0, x.1.pos)), Some((12, 30)));
        assert_eq!(c.len(), 3);
        // Rows of other scans are separate; out-of-range rows are ignored.
        c.extend(1, 2, 0, &[cp(5)]);
        assert_eq!(c.best(1, 2, 2).map(|x| x.1.pos), Some(5));
        assert!(c.best(0, 1, 100).is_none());
        c.extend(1, 3, 0, &[cp(5)]);
        assert!(c.best(1, 3, 100).is_none());
        assert!(c.best(2, 0, 100).is_none());
    }

    #[test]
    fn recorder() {
        let c = ColCache::new(&[(1, 4)]);
        let mut r = ColRec::new(4, 0);
        for u in 0..10 {
            r.at(u, || cp(u as usize));
        }
        r.flush(&c, 0, 0);
        assert_eq!(c.best(0, 0, 9).map(|x| (x.0, x.1.pos)), Some((8, 8)));
        // From a recorded column: continues the row.
        let (u, _) = c.best(0, 0, 9).unwrap();
        r.reset(u);
        for u in u..13 {
            r.at(u, || cp(u as usize));
        }
        r.flush(&c, 0, 0);
        assert_eq!(c.best(0, 0, 100).map(|x| (x.0, x.1.pos)), Some((12, 12)));
        assert_eq!(c.len(), 3);
    }
}

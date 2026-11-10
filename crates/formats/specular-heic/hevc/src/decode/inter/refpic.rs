//! A decoded picture held for reference, published one CTB row at a time.
//!
//! A picture of a sequence is decoded on one thread while later pictures,
//! on other threads, already predict from its finished rows. The picture
//! owns one write-once slot per CTB row, filled once the row is
//! loop-filtered for good, and a count of the rows filled so far that
//! readers wait on: motion compensation for the lines its filter taps
//! reach, temporal motion vector prediction for the row of the collocated
//! block, concealment for the block it copies, output for all of them.
//!
//! Every slot is filled, or the picture declared finished with the rest
//! grey, whatever happens to the decode (damage, cancellation, a panic), so
//! no reader waits forever; a line nobody published reads as mid-grey.

use super::ColMv;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

/// The samples of one CTB row: its lines of each plane, top to bottom,
/// each as wide as the plane.
#[derive(Debug, Default)]
pub struct Row {
    /// Luma, Cb, Cr (both empty for monochrome).
    pub planes: [Box<[u16]>; 3],
}

/// How the decode of a picture ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    /// CTUs concealed.
    pub damaged: usize,
    /// CTUs in the picture.
    pub total: usize,
    /// The decode was cancelled: the picture is not worth showing.
    pub cancelled: bool,
    /// A reference of the picture, or the picture its lost CTBs were
    /// copied from, was cancelled or is tainted: not worth showing either.
    pub tainted: bool,
}

/// A decoded picture held for reference and output.
#[derive(Debug)]
pub struct RefPic {
    /// `PicOrderCntVal`.
    pub poc: i32,
    /// Width and height of luma, Cb and Cr (zero for monochrome chroma).
    dims: [(usize, usize); 3],
    /// `log2` of the lines per row: luma, chroma.
    shift: [u32; 2],
    /// The rows, filled in order.
    rows: Box<[OnceLock<Arc<Row>>]>,
    /// Motion per 16x16 block, one slot per row of blocks; none when the
    /// picture has no inter slice (or stands in for a missing one).
    cols: Box<[OnceLock<Box<[ColMv]>>]>,
    /// The line a row nobody published reads as: luma, chroma.
    grey: [Box<[u16]>; 2],
    /// Rows published.
    progress: AtomicUsize,
    /// Set once every row is in.
    status: OnceLock<Status>,
    lock: Mutex<()>,
    cv: Condvar,
}

impl RefPic {
    /// A picture of the given plane sizes, its CTB rows `1 << ctb_log2`
    /// luma lines high, to be published row by row. `col` when temporal
    /// motion will be stored with it.
    pub fn new(
        poc: i32,
        dims: [(usize, usize); 3],
        bit_depth: (u8, u8),
        ctb_log2: usize,
        sub_h: usize,
        col: bool,
    ) -> RefPic {
        let ctb_log2 = ctb_log2.clamp(3, 6) as u32;
        let n_rows = dims[0].1.div_ceil(1 << ctb_log2);
        let n16 = if col { dims[0].1.div_ceil(16) } else { 0 };
        let grey = |w: usize, bd: u8| vec![1u16 << (bd.clamp(1, 16) - 1); w].into_boxed_slice();
        RefPic {
            poc,
            dims,
            shift: [ctb_log2, ctb_log2 - u32::from(sub_h == 2)],
            rows: (0..n_rows).map(|_| OnceLock::new()).collect(),
            cols: (0..n16).map(|_| OnceLock::new()).collect(),
            grey: [grey(dims[0].0, bit_depth.0), grey(dims[1].0, bit_depth.1)],
            progress: AtomicUsize::new(0),
            status: OnceLock::new(),
            lock: Mutex::new(()),
            cv: Condvar::new(),
        }
    }

    /// A finished mid-grey picture with no motion, standing in for a
    /// missing reference (8.3.3.2).
    pub fn generated(poc: i32, dims: [(usize, usize); 3], bit_depth: (u8, u8)) -> RefPic {
        let p = RefPic::new(poc, dims, bit_depth, 6, 2, false);
        p.finish(Status::default());
        p
    }

    /// Width and height of plane `c`.
    #[inline]
    pub fn dims(&self, c: usize) -> (usize, usize) {
        self.dims[c]
    }

    /// The plane sizes.
    pub fn all_dims(&self) -> [(usize, usize); 3] {
        self.dims
    }

    /// CTB rows.
    pub fn n_rows(&self) -> usize {
        self.rows.len()
    }

    /// The first line of row `r` of plane `c`, and one past its last.
    pub fn row_lines(&self, c: usize, r: usize) -> (usize, usize) {
        let s = self.shift[usize::from(c > 0)];
        let h = self.dims[c].1;
        ((r << s).min(h), ((r + 1) << s).min(h))
    }

    /// The rows of 16x16 motion blocks whose top line lies in CTB row `r`.
    pub fn col_rows(&self, r: usize) -> core::ops::Range<usize> {
        let s = self.shift[0];
        let a = (r << s).div_ceil(16).min(self.cols.len());
        let b = ((r + 1) << s).div_ceil(16).min(self.cols.len());
        a..b
    }

    /// True when motion is stored with the picture.
    #[inline]
    pub fn has_col(&self) -> bool {
        !self.cols.is_empty()
    }

    /// Rows published so far.
    #[inline]
    pub fn progress(&self) -> usize {
        self.progress.load(Ordering::Acquire)
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Publishes row `r` (the next one) with the motion of its 16x16
    /// blocks ([`RefPic::col_rows`]); `None` leaves it grey.
    pub fn publish(&self, r: usize, row: Option<Row>, cols: Vec<Box<[ColMv]>>) {
        if let (Some(row), Some(slot)) = (row, self.rows.get(r)) {
            let _ = slot.set(Arc::new(row));
        }
        for (k, c) in self.col_rows(r).zip(cols) {
            let _ = self.cols[k].set(c);
        }
        let _g = self.guard();
        self.progress.fetch_max(r + 1, Ordering::Release);
        self.cv.notify_all();
    }

    /// Publishes row `r` as the same samples as row `r` of `src`, a
    /// picture of the same plane sizes and row height, without copying.
    pub fn share_row(&self, r: usize, src: &RefPic) {
        if let (Some(row), Some(slot)) = (src.rows.get(r).and_then(OnceLock::get), self.rows.get(r))
        {
            let _ = slot.set(Arc::clone(row));
        }
        let _g = self.guard();
        self.progress.fetch_max(r + 1, Ordering::Release);
        self.cv.notify_all();
    }

    /// True when `other` stores its rows the way this picture does.
    pub fn same_layout(&self, other: &RefPic) -> bool {
        self.dims == other.dims && self.shift == other.shift && self.rows.len() == other.rows.len()
    }

    /// True when the picture was made by [`RefPic::new`] with these plane
    /// sizes, CTB size and `SubHeightC`.
    pub fn layout_is(&self, dims: [(usize, usize); 3], ctb_log2: usize, sub_h: usize) -> bool {
        let s = ctb_log2.clamp(3, 6) as u32;
        self.dims == dims && self.shift == [s, s - u32::from(sub_h == 2)]
    }

    /// Ends the picture: rows not published stay grey, and readers waiting
    /// for them go on.
    pub fn finish(&self, status: Status) {
        let _g = self.guard();
        let _ = self.status.set(status);
        self.progress.store(self.rows.len(), Ordering::Release);
        self.cv.notify_all();
    }

    /// How the decode ended, once it has.
    pub fn status(&self) -> Option<Status> {
        self.status.get().copied()
    }

    /// True once every row is in.
    pub fn is_done(&self) -> bool {
        self.status.get().is_some()
    }

    /// Waits until every row is in.
    pub fn wait_done(&self) {
        let mut g = self.guard();
        while self.status.get().is_none() {
            g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// [`RefPic::wait_done`] for at most `t`; true when done.
    pub fn wait_done_for(&self, t: Duration) -> bool {
        let g = self.guard();
        if self.status.get().is_some() {
            return true;
        }
        let _g = self
            .cv
            .wait_timeout(g, t)
            .unwrap_or_else(PoisonError::into_inner);
        self.status.get().is_some()
    }

    /// Waits until the first `n` rows are in.
    #[inline]
    pub fn wait_rows(&self, n: usize) {
        let n = n.min(self.rows.len());
        if self.progress.load(Ordering::Acquire) < n {
            self.block_until(n);
        }
    }

    /// [`RefPic::wait_rows`] once the rows are found missing.
    #[cold]
    #[inline(never)]
    fn block_until(&self, n: usize) {
        let mut g = self.guard();
        while self.progress.load(Ordering::Acquire) < n {
            g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Waits until line `y` of plane `c` (clamped to the plane) is in.
    #[inline]
    pub fn wait_line(&self, c: usize, y: usize) {
        let y = y.min(self.dims[c].1.saturating_sub(1));
        self.wait_rows((y >> self.shift[usize::from(c > 0)]) + 1);
    }

    /// Line `y` of plane `c`, as wide as the plane. Grey when the row was
    /// never published; call [`RefPic::wait_line`] first.
    #[inline]
    pub fn line(&self, c: usize, y: usize) -> &[u16] {
        let s = self.shift[usize::from(c > 0)];
        let w = self.dims[c].0;
        let r = y >> s;
        if let Some(row) = self.rows.get(r).and_then(OnceLock::get) {
            let o = (y - (r << s)) * w;
            if let Some(l) = row.planes[c].get(o..o + w) {
                return l;
            }
        }
        &self.grey[usize::from(c > 0)][..w]
    }

    /// The collocated motion covering luma position `(x, y)`, once its row
    /// is in.
    #[inline]
    pub fn col_at(&self, x: usize, y: usize) -> ColMv {
        if self.cols.is_empty() {
            return ColMv::default();
        }
        self.wait_line(0, y & !15);
        self.cols
            .get(y >> 4)
            .and_then(OnceLock::get)
            .and_then(|r| r.get(x >> 4))
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    #[test]
    fn rows_publish_in_order_and_unpublished_ones_read_grey() {
        let p = RefPic::new(3, [(8, 40), (4, 20), (4, 20)], (8, 8), 4, 2, true);
        assert_eq!(p.n_rows(), 3);
        assert_eq!(p.row_lines(0, 2), (32, 40));
        assert_eq!(p.row_lines(1, 1), (8, 16));
        assert_eq!(p.col_rows(0), 0..1);
        assert_eq!(p.col_rows(2), 2..3);
        let row = Row {
            planes: [
                vec![7u16; 8 * 16].into(),
                vec![9u16; 4 * 8].into(),
                vec![9u16; 4 * 8].into(),
            ],
        };
        p.publish(0, Some(row), vec![vec![ColMv::default(); 1].into()]);
        assert_eq!(p.progress(), 1);
        p.wait_line(0, 15);
        assert_eq!(p.line(0, 15), &[7u16; 8]);
        assert_eq!(p.line(1, 7), &[9u16; 4]);
        let t = std::thread::scope(|s| {
            let h = s.spawn(|| {
                p.wait_line(0, 39);
                p.line(0, 39).to_vec()
            });
            p.finish(Status::default());
            h.join().expect("reader")
        });
        assert_eq!(t, vec![128u16; 8]);
        assert!(p.is_done());
    }
}

//! Safe mutable 2D views.
//!
//! Upstream jxl-grid implements `MutableSubgrid` with a raw pointer + stride so that disjoint
//! rectangles of one buffer can be handed to different threads. This fork builds the same API on
//! safe slices instead:
//!
//! * a view whose rows are evenly spaced in one slice is `Contig` (`&mut [V]` + stride), which
//!   is what full grids, vertical splits and row bands are;
//! * a view made by cutting rows horizontally is `Rows` (one `&mut [V]` per row, built with
//!   `split_at_mut`).
//!
//! Views may be *logically* smaller than the memory they own (`shrink_*` / `regrow`), which is
//! how the squeeze transform hides and later restores the right/bottom half of a channel without
//! having to merge two independent borrows back together.

use std::ops::RangeBounds;

use crate::grid::SharedSubgrid;

pub(crate) enum Repr<'g, V> {
    /// Row `y` is `buf[y * stride..][..width]`.
    Contig { buf: &'g mut [V], stride: usize },
    /// Row `y` is `rows[y][..width]`.
    Rows(Vec<&'g mut [V]>),
}

/// A mutable subgrid of the underlying buffer.
pub struct MutableSubgrid<'g, V = f32> {
    repr: Repr<'g, V>,
    width: usize,
    height: usize,
}

impl<V> std::fmt::Debug for MutableSubgrid<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutableSubgrid")
            .field("width", &self.width)
            .field("height", &self.height)
            .field(
                "repr",
                &match self.repr {
                    Repr::Contig { stride, .. } => format!("contig(stride {stride})"),
                    Repr::Rows(ref r) => format!("rows({})", r.len()),
                },
            )
            .finish()
    }
}

impl<'g, V> From<&'g mut crate::grid::AlignedGrid<V>> for MutableSubgrid<'g, V> {
    fn from(grid: &'g mut crate::grid::AlignedGrid<V>) -> Self {
        let width = grid.width();
        let height = grid.height();
        Self::from_buf(grid.buf_mut(), width, height, width)
    }
}

#[inline]
pub(crate) fn bounds(
    range_x: impl RangeBounds<usize>,
    range_y: impl RangeBounds<usize>,
    width: usize,
    height: usize,
) -> (usize, usize, usize, usize) {
    use std::ops::Bound;
    let left = match range_x.start_bound() {
        Bound::Included(&v) => v,
        Bound::Excluded(&v) => v + 1,
        Bound::Unbounded => 0,
    };
    let right = match range_x.end_bound() {
        Bound::Included(&v) => v + 1,
        Bound::Excluded(&v) => v,
        Bound::Unbounded => width,
    };
    let top = match range_y.start_bound() {
        Bound::Included(&v) => v,
        Bound::Excluded(&v) => v + 1,
        Bound::Unbounded => 0,
    };
    let bottom = match range_y.end_bound() {
        Bound::Included(&v) => v + 1,
        Bound::Excluded(&v) => v,
        Bound::Unbounded => height,
    };
    assert!(
        left <= right && right <= width,
        "subgrid x range out of bounds"
    );
    assert!(
        top <= bottom && bottom <= height,
        "subgrid y range out of bounds"
    );
    (left, right, top, bottom)
}

pub(crate) use bounds as subgrid_bounds;

impl<'g, V> MutableSubgrid<'g, V> {
    /// Creates a empty, zero-sized subgrid.
    pub fn empty() -> Self {
        Self {
            repr: Repr::Contig {
                buf: &mut [],
                stride: 0,
            },
            width: 0,
            height: 0,
        }
    }

    /// Create a `MutableSubgrid` from buffer slice, width, height and stride.
    ///
    /// # Panic
    /// Panics if `width` is greater than `stride`, or the area specified by `width`, `height` and
    /// `stride` is larger than `buf`.
    pub fn from_buf(buf: &'g mut [V], width: usize, height: usize, stride: usize) -> Self {
        assert!(width <= stride);
        if width != 0 && height != 0 {
            let required_len = stride
                .checked_mul(height - 1)
                .and_then(|offset| offset.checked_add(width))
                .expect("subgrid area overflows usize");
            assert!(buf.len() >= required_len);
        }
        Self {
            repr: Repr::Contig { buf, stride },
            width,
            height,
        }
    }

    /// Creates a subgrid from one slice per row (each at least `width` long).
    pub fn from_rows(rows: Vec<&'g mut [V]>, width: usize) -> Self {
        assert!(rows.iter().all(|r| r.len() >= width));
        let height = rows.len();
        Self {
            repr: Repr::Rows(rows),
            width,
            height,
        }
    }

    /// Returns the width of the subgrid.
    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the subgrid.
    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns a reference to the sample at the given location.
    ///
    /// # Panics
    /// Panics if the coordinate is out of bounds.
    #[inline]
    pub fn get_ref(&self, x: usize, y: usize) -> &V {
        let (width, height) = (self.width, self.height);
        let Some(r) = self.try_get_ref(x, y) else {
            panic!("coordinate out of range: ({x}, {y}) not in {width}x{height}");
        };
        r
    }

    /// Returns a reference to the sample at the given location, or `None` if it is out of bounds.
    #[inline]
    pub fn try_get_ref(&self, x: usize, y: usize) -> Option<&V> {
        if x >= self.width {
            return None;
        }
        self.try_get_row(y).map(|row| &row[x])
    }

    /// Returns a slice of a row of samples.
    ///
    /// # Panics
    /// Panics if the row index is out of bounds.
    #[inline]
    pub fn get_row(&self, row: usize) -> &[V] {
        let height = self.height;
        let Some(slice) = self.try_get_row(row) else {
            panic!("row index out of range: height is {height} but index is {row}");
        };
        slice
    }

    /// Returns a slice of a row of samples, or `None` if it is out of bounds.
    #[inline]
    pub fn try_get_row(&self, row: usize) -> Option<&[V]> {
        if row >= self.height {
            return None;
        }
        let w = self.width;
        Some(match &self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 {
                    &[]
                } else {
                    &buf[row * stride..][..w]
                }
            }
            Repr::Rows(rows) => &rows[row][..w],
        })
    }

    /// Returns a mutable reference to the sample at the given location.
    ///
    /// # Panics
    /// Panics if the coordinate is out of bounds.
    #[inline]
    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut V {
        let (width, height) = (self.width, self.height);
        let Some(r) = self.try_get_mut(x, y) else {
            panic!("coordinate out of range: ({x}, {y}) not in {width}x{height}");
        };
        r
    }

    /// Returns a mutable reference to the sample at the given location, or `None` if it is out of
    /// bounds.
    #[inline]
    pub fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut V> {
        if x >= self.width {
            return None;
        }
        self.try_get_row_mut(y).map(|row| &mut row[x])
    }

    /// Returns a mutable slice of a row of samples.
    ///
    /// # Panics
    /// Panics if the row index is out of bounds.
    #[inline]
    pub fn get_row_mut(&mut self, row: usize) -> &mut [V] {
        let height = self.height;
        let Some(slice) = self.try_get_row_mut(row) else {
            panic!("row index out of range: height is {height} but index is {row}");
        };
        slice
    }

    /// Returns a mutable slice of a row of samples, or `None` if it is out of bounds.
    #[inline]
    pub fn try_get_row_mut(&mut self, row: usize) -> Option<&mut [V]> {
        if row >= self.height {
            return None;
        }
        let w = self.width;
        Some(match &mut self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 {
                    &mut []
                } else {
                    &mut buf[row * *stride..][..w]
                }
            }
            Repr::Rows(rows) => &mut rows[row][..w],
        })
    }

    /// Returns row `y - 1` (shared) and row `y` (mutable).
    ///
    /// # Panics
    /// Panics if `y == 0` or `y >= height`.
    #[inline]
    pub fn prev_and_row_mut(&mut self, y: usize) -> (&[V], &mut [V]) {
        assert!(y > 0 && y < self.height);
        let w = self.width;
        match &mut self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 {
                    return (&[], &mut []);
                }
                let (u, d) = buf.split_at_mut(y * *stride);
                (&u[(y - 1) * *stride..][..w], &mut d[..w])
            }
            Repr::Rows(rows) => {
                let (u, d) = rows.split_at_mut(y);
                (&u[y - 1][..w], &mut d[0][..w])
            }
        }
    }

    /// Returns rows `y0` and `y1` mutably, `y0 < y1`.
    #[inline]
    pub fn two_rows_mut(&mut self, y0: usize, y1: usize) -> (&mut [V], &mut [V]) {
        assert!(y0 < y1 && y1 < self.height);
        let w = self.width;
        match &mut self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 {
                    return (&mut [], &mut []);
                }
                let (u, d) = buf.split_at_mut(y1 * *stride);
                (&mut u[y0 * *stride..][..w], &mut d[..w])
            }
            Repr::Rows(rows) => {
                let (u, d) = rows.split_at_mut(y1);
                (&mut u[y0][..w], &mut d[0][..w])
            }
        }
    }

    /// Swaps two samples at the given locations.
    ///
    /// # Panics
    /// Panics if either one of the locations is out of bounds.
    #[inline]
    pub fn swap(&mut self, (ax, ay): (usize, usize), (bx, by): (usize, usize)) {
        assert!(ax < self.width && bx < self.width);
        if ay == by {
            self.get_row_mut(ay).swap(ax, bx);
        } else if ay < by {
            let (a, b) = self.two_rows_mut(ay, by);
            std::mem::swap(&mut a[ax], &mut b[bx]);
        } else {
            let (b, a) = self.two_rows_mut(by, ay);
            std::mem::swap(&mut a[ax], &mut b[bx]);
        }
    }

    /// Reborrows the mutable subgrid, and returns a mutable subgrid with shorter lifetime.
    pub fn borrow_mut(&mut self) -> MutableSubgrid<'_, V> {
        let repr = match &mut self.repr {
            Repr::Contig { buf, stride } => Repr::Contig {
                buf: &mut **buf,
                stride: *stride,
            },
            Repr::Rows(rows) => Repr::Rows(rows.iter_mut().map(|r| &mut **r).collect()),
        };
        MutableSubgrid {
            repr,
            width: self.width,
            height: self.height,
        }
    }

    /// Reborrows the mutable subgrid, and returns a shared subgrid.
    pub fn as_shared(&self) -> SharedSubgrid<'_, V> {
        match &self.repr {
            Repr::Contig { buf, stride } => {
                SharedSubgrid::from_buf_unchecked(buf, self.width, self.height, *stride)
            }
            Repr::Rows(rows) => SharedSubgrid::from_mut_rows(rows, 0, self.width, self.height),
        }
    }

    /// Creates a subgrid of this subgrid.
    ///
    /// # Panics
    /// Panics if the range is out of bounds.
    pub fn subgrid(
        self,
        range_x: impl RangeBounds<usize>,
        range_y: impl RangeBounds<usize>,
    ) -> MutableSubgrid<'g, V> {
        let (left, right, top, bottom) = bounds(range_x, range_y, self.width, self.height);
        let (w, h) = (right - left, bottom - top);
        let repr = match self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 || h == 0 {
                    Repr::Contig {
                        buf: &mut [],
                        stride,
                    }
                } else {
                    let start = top * stride + left;
                    let len = (h - 1) * stride + w;
                    Repr::Contig {
                        buf: &mut buf[start..start + len],
                        stride,
                    }
                }
            }
            Repr::Rows(rows) => Repr::Rows(
                rows.into_iter()
                    .skip(top)
                    .take(h)
                    .map(|r| &mut r[left..right])
                    .collect(),
            ),
        };
        MutableSubgrid {
            repr,
            width: w,
            height: h,
        }
    }

    /// Split the grid horizontally at an index.
    ///
    /// # Panics
    /// Panics if `x > self.width()`.
    pub fn split_horizontal(&mut self, x: usize) -> (MutableSubgrid<'_, V>, MutableSubgrid<'_, V>) {
        assert!(x <= self.width);
        let width = self.width;
        let height = self.height;
        let mut left = Vec::with_capacity(height);
        let mut right = Vec::with_capacity(height);
        for row in self.borrow_mut().into_row_slices() {
            let (l, r) = row.split_at_mut(x);
            left.push(l);
            right.push(&mut r[..width - x]);
        }
        (
            MutableSubgrid::from_rows(left, x),
            MutableSubgrid::from_rows(right, width - x),
        )
    }

    /// Split the grid vertically at an index.
    ///
    /// # Panics
    /// Panics if `y > self.height()`.
    pub fn split_vertical(&mut self, y: usize) -> (MutableSubgrid<'_, V>, MutableSubgrid<'_, V>) {
        assert!(y <= self.height);
        let mut top = self.borrow_mut();
        let bottom = top.split_vertical_in_place(y);
        (top, bottom)
    }

    /// Split the grid vertically at an index in-place; `self` keeps the top part.
    ///
    /// # Panics
    /// Panics if `y > self.height()`.
    pub fn split_vertical_in_place(&mut self, y: usize) -> MutableSubgrid<'g, V> {
        assert!(y <= self.height);
        let width = self.width;
        let bottom_height = self.height - y;
        let repr = match &mut self.repr {
            Repr::Contig { buf, stride } => {
                let stride = *stride;
                let all = std::mem::take(buf);
                let at = (y * stride).min(all.len());
                let (t, b) = all.split_at_mut(at);
                *buf = t;
                Repr::Contig { buf: b, stride }
            }
            Repr::Rows(rows) => {
                let b = rows.split_off(y.min(rows.len()));
                Repr::Rows(b)
            }
        };
        self.height = y;
        MutableSubgrid {
            repr,
            width,
            height: bottom_height,
        }
    }

    /// Consumes the view and returns its rows (`width` samples each).
    pub fn into_row_slices(self) -> Vec<&'g mut [V]> {
        let MutableSubgrid {
            repr,
            width,
            height,
        } = self;
        match repr {
            Repr::Contig { buf, stride } => {
                if width == 0 || height == 0 {
                    return (0..height).map(|_| <&mut [V]>::default()).collect();
                }
                let mut out = Vec::with_capacity(height);
                let mut rest = buf;
                for y in 0..height {
                    let take = if y + 1 == height { width } else { stride };
                    let (row, r) = rest.split_at_mut(take);
                    out.push(&mut row[..width]);
                    rest = r;
                }
                out
            }
            Repr::Rows(rows) => rows
                .into_iter()
                .take(height)
                .map(|r| &mut r[..width])
                .collect(),
        }
    }

    /// Logically hides columns from `width` onwards; the memory stays owned by this view and can
    /// be brought back with [`regrow`](Self::regrow).
    pub fn shrink_width(&mut self, width: usize) {
        assert!(width <= self.width);
        self.width = width;
    }

    /// Logically hides rows from `height` onwards (see [`shrink_width`](Self::shrink_width)).
    pub fn shrink_height(&mut self, height: usize) {
        assert!(height <= self.height);
        self.height = height;
    }

    /// Restores a logical size previously reduced with `shrink_*`.
    ///
    /// # Panics
    /// Panics if the view does not own enough memory for the size.
    pub fn regrow(&mut self, width: usize, height: usize) {
        match &self.repr {
            Repr::Contig { buf, stride } => {
                assert!(width <= *stride || height <= 1);
                if width != 0 && height != 0 {
                    assert!((height - 1) * stride + width <= buf.len());
                }
            }
            Repr::Rows(rows) => {
                assert!(height <= rows.len());
                assert!(rows[..height].iter().all(|r| r.len() >= width));
            }
        }
        self.width = width;
        self.height = height;
    }
}

impl<V: Copy> MutableSubgrid<'_, V> {
    /// Returns a copy of sample at the given location.
    ///
    /// # Panics
    /// Panics if the coordinate is out of range.
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> V {
        *self.get_ref(x, y)
    }
}

impl<'g, V> MutableSubgrid<'g, V> {
    /// Returns a list of subgrids split into groups of given size, in row-first order.
    ///
    /// Groups at the edge of the grid may have smaller sizes.
    ///
    /// # Panics
    /// Panics if either `group_width` or `group_height` is zero.
    pub fn into_groups(
        self,
        group_width: usize,
        group_height: usize,
    ) -> Vec<MutableSubgrid<'g, V>> {
        assert!(
            group_width > 0 && group_height > 0,
            "expected group width and height to be nonzero, got width = {group_width}, height = {group_height}"
        );

        let num_cols = self.width.div_ceil(group_width);
        let num_rows = self.height.div_ceil(group_height);
        self.into_groups_with_fixed_count(group_width, group_height, num_cols, num_rows)
    }

    /// Returns a list of subgrids split into groups of given size, with `num_rows` rows and
    /// `num_cols` columns, in row-first order.
    ///
    /// Unlike `into_groups`, this method will always return vector of length
    /// `num_cols * num_rows`. Remaining rows or columns will be truncated, and groups out of
    /// bounds will be zero-sized.
    pub fn into_groups_with_fixed_count(
        mut self,
        group_width: usize,
        group_height: usize,
        num_cols: usize,
        num_rows: usize,
    ) -> Vec<MutableSubgrid<'g, V>> {
        let width = self.width;
        let height = self.height;
        let group_count = num_cols
            .checked_mul(num_rows)
            .expect("subgrid group count overflows usize");
        let mut groups = Vec::with_capacity(group_count);
        for gy in 0..num_rows {
            let y = gy.saturating_mul(group_height).min(height);
            let gh = (height - y).min(group_height);
            // `self` holds rows [y, height); peel this band off the top.
            let rest = self.split_vertical_in_place(gh);
            let band = std::mem::replace(&mut self, rest);

            let col_widths = (0..num_cols)
                .map(|gx| (width - gx.saturating_mul(group_width).min(width)).min(group_width));
            if num_cols == 1 {
                let mut band = band;
                let gw = col_widths.clone().next().unwrap_or(0);
                band.shrink_width(gw);
                groups.push(band);
                continue;
            }
            let mut cols: Vec<Vec<&'g mut [V]>> =
                (0..num_cols).map(|_| Vec::with_capacity(gh)).collect();
            for row in band.into_row_slices() {
                let mut rest = row;
                for (col, gw) in cols.iter_mut().zip(col_widths.clone()) {
                    let (piece, r) = rest.split_at_mut(gw);
                    col.push(piece);
                    rest = r;
                }
            }
            for (col, gw) in cols.into_iter().zip(col_widths) {
                if gh == 0 {
                    groups.push(MutableSubgrid {
                        repr: Repr::Rows(Vec::new()),
                        width: gw,
                        height: 0,
                    });
                } else {
                    groups.push(MutableSubgrid::from_rows(col, gw));
                }
            }
        }
        groups
    }
}

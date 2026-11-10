//! Safe shared 2D views (see `mutable_subgrid.rs` for the design).

use std::ops::RangeBounds;

use super::mutable_subgrid::subgrid_bounds;

enum Repr<'g, V> {
    /// Row `y` is `buf[y * stride..][..width]`.
    Contig { buf: &'g [V], stride: usize },
    /// Row `y` is `rows[y][x0..][..width]`.
    Rows { rows: &'g [&'g mut [V]], x0: usize },
}

impl<V> Clone for Repr<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<V> Copy for Repr<'_, V> {}

/// An immutable shared subgrid of the underlying buffer.
pub struct SharedSubgrid<'g, V = f32> {
    repr: Repr<'g, V>,
    width: usize,
    height: usize,
}

impl<V> Clone for SharedSubgrid<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<V> Copy for SharedSubgrid<'_, V> {}

impl<V> std::fmt::Debug for SharedSubgrid<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedSubgrid")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl<'g, V> From<&'g crate::grid::AlignedGrid<V>> for SharedSubgrid<'g, V> {
    fn from(value: &'g crate::grid::AlignedGrid<V>) -> Self {
        SharedSubgrid::from_buf(value.buf(), value.width(), value.height(), value.width())
    }
}

impl<'g, V> SharedSubgrid<'g, V> {
    /// Create a `SharedSubgrid` from buffer slice, width, height and stride.
    ///
    /// # Panic
    /// Panics if `width` is greater than `stride`, or the area specified by `width`, `height` and
    /// `stride` is larger than `buf`.
    pub fn from_buf(buf: &'g [V], width: usize, height: usize, stride: usize) -> Self {
        assert!(width <= stride);
        if width != 0 && height != 0 {
            let required_len = stride
                .checked_mul(height - 1)
                .and_then(|offset| offset.checked_add(width))
                .expect("subgrid area overflows usize");
            assert!(buf.len() >= required_len);
        }
        Self::from_buf_unchecked(buf, width, height, stride)
    }

    #[inline]
    pub(crate) fn from_buf_unchecked(
        buf: &'g [V],
        width: usize,
        height: usize,
        stride: usize,
    ) -> Self {
        Self {
            repr: Repr::Contig { buf, stride },
            width,
            height,
        }
    }

    #[inline]
    pub(crate) fn from_mut_rows(
        rows: &'g [&'g mut [V]],
        x0: usize,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            repr: Repr::Rows { rows, x0 },
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
    pub fn get_ref(&self, x: usize, y: usize) -> &'g V {
        let (width, height) = (self.width, self.height);
        let Some(r) = self.try_get_ref(x, y) else {
            panic!("coordinate out of range: ({x}, {y}) not in {width}x{height}");
        };
        r
    }

    /// Returns a reference to the sample at the given location, or `None` if it is out of bounds.
    #[inline]
    pub fn try_get_ref(&self, x: usize, y: usize) -> Option<&'g V> {
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
    pub fn get_row(&self, row: usize) -> &'g [V] {
        let height = self.height;
        let Some(slice) = self.try_get_row(row) else {
            panic!("row index out of range: height is {height} but index is {row}");
        };
        slice
    }

    /// Returns a slice of a row of samples, or `None` if it is out of bounds.
    #[inline]
    pub fn try_get_row(&self, row: usize) -> Option<&'g [V]> {
        if row >= self.height {
            return None;
        }
        let w = self.width;
        Some(match self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 {
                    &[]
                } else {
                    &buf[row * stride..][..w]
                }
            }
            Repr::Rows { rows, x0 } => &rows[row][x0..][..w],
        })
    }

    /// Split the grid horizontally at an index.
    ///
    /// # Panics
    /// Panics if `x > self.width()`.
    pub fn split_horizontal(&self, x: usize) -> (SharedSubgrid<'g, V>, SharedSubgrid<'g, V>) {
        assert!(x <= self.width);
        (self.subgrid(..x, ..), self.subgrid(x.., ..))
    }

    /// Split the grid vertically at an index.
    ///
    /// # Panics
    /// Panics if `y > self.height()`.
    pub fn split_vertical(&self, y: usize) -> (SharedSubgrid<'g, V>, SharedSubgrid<'g, V>) {
        assert!(y <= self.height);
        (self.subgrid(.., ..y), self.subgrid(.., y..))
    }

    /// Creates a subgrid of this subgrid.
    ///
    /// # Panics
    /// Panics if the range is out of bounds.
    pub fn subgrid(
        &self,
        range_x: impl RangeBounds<usize>,
        range_y: impl RangeBounds<usize>,
    ) -> SharedSubgrid<'g, V> {
        let (left, right, top, bottom) = subgrid_bounds(range_x, range_y, self.width, self.height);
        let (w, h) = (right - left, bottom - top);
        let repr = match self.repr {
            Repr::Contig { buf, stride } => {
                if w == 0 || h == 0 {
                    Repr::Contig { buf: &[], stride }
                } else {
                    let start = top * stride + left;
                    let len = (h - 1) * stride + w;
                    Repr::Contig {
                        buf: &buf[start..start + len],
                        stride,
                    }
                }
            }
            Repr::Rows { rows, x0 } => Repr::Rows {
                rows: &rows[top..bottom],
                x0: x0 + left,
            },
        };
        SharedSubgrid {
            repr,
            width: w,
            height: h,
        }
    }
}

impl<V: Copy> SharedSubgrid<'_, V> {
    /// Returns a copy of sample at the given location.
    ///
    /// # Panics
    /// Panics if the coordinate is out of range.
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> V {
        *self.get_ref(x, y)
    }
}

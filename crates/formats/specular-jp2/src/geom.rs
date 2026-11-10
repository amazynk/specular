//! Integer geometry helpers (Annex B coordinate arithmetic).

/// Half-open rectangle `[x0, x1) x [y0, y1)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Rect {
    pub fn new(x0: u32, y0: u32, x1: u32, y1: u32) -> Rect {
        Rect {
            x0,
            y0,
            x1: x1.max(x0),
            y1: y1.max(y0),
        }
    }
    #[inline]
    pub fn w(&self) -> u32 {
        self.x1 - self.x0
    }
    #[inline]
    pub fn h(&self) -> u32 {
        self.y1 - self.y0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
    pub fn area(&self) -> u64 {
        self.w() as u64 * self.h() as u64
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        Rect::new(
            self.x0.max(o.x0),
            self.y0.max(o.y0),
            self.x1.min(o.x1),
            self.y1.min(o.y1),
        )
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        !self.intersect(o).is_empty()
    }
    /// Grow by `m` on every side, clipped to `bounds`.
    pub fn grow_within(&self, m: u32, bounds: &Rect) -> Rect {
        Rect::new(
            self.x0.saturating_sub(m),
            self.y0.saturating_sub(m),
            self.x1.saturating_add(m),
            self.y1.saturating_add(m),
        )
        .intersect(bounds)
    }
}

/// `ceil(a / 2^s)` for any shift (`s >= 32` gives 0 or 1).
#[inline]
pub(crate) fn ceil_shr(a: u32, s: u32) -> u32 {
    if s >= 63 {
        return (a > 0) as u32;
    }
    ((a as u64 + (1u64 << s) - 1) >> s) as u32
}

/// `ceil((a - off) / 2^s)` with a possibly negative numerator, clamped at 0.
#[inline]
pub(crate) fn ceil_shr_off(a: u32, off: u64, s: u32) -> u32 {
    let n = a as i64 - off as i64;
    if s >= 62 {
        return (n > 0) as u32;
    }
    let d = 1i64 << s;
    let q = n.div_euclid(d) + (n.rem_euclid(d) != 0) as i64;
    q.max(0) as u32
}

/// Low-pass (even) half of an interleaved interval: `[ceil(a/2), ceil(b/2))`.
#[inline]
pub(crate) fn low_half(a: u32, b: u32) -> (u32, u32) {
    (a.div_ceil(2), b.div_ceil(2))
}

/// High-pass (odd) half of an interleaved interval: `[floor(a/2), floor(b/2))`.
#[inline]
pub(crate) fn high_half(a: u32, b: u32) -> (u32, u32) {
    (a / 2, b / 2)
}

/// Fallible zeroed allocation.
pub(crate) fn try_vec<T: Clone + Default>(n: usize) -> crate::Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| crate::Jp2Error::TooLarge((n * std::mem::size_of::<T>()) as u64))?;
    v.resize(n, T::default());
    Ok(v)
}

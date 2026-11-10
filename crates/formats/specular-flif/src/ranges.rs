//! The colour ranges each transform leaves behind: per channel a global range, a range
//! conditional on the values of the channels decoded before it, and the snap function that
//! moves a prediction onto a valid value. Transforms stack: each layer refers to the one
//! below it by index.

/// One layer of the chain.
pub(crate) enum Layer {
    /// Explicit ranges (the image's own, or Bounds over static ranges).
    Static(Vec<(i32, i32)>),
    /// Same as the layer below (frame transforms).
    Dup(usize),
    /// ChannelCompact: 0..=nb per channel.
    Compact(usize, [i32; 4]),
    /// YCoCg with its `par` (max / 4 + 1).
    YCoCg(usize, i32),
    Permute(usize, Vec<usize>),
    PermuteSub(usize, Vec<usize>),
    Bounds(usize, Vec<(i32, i32)>),
    Palette(usize, i32),
    PaletteA(usize, i32),
    Buckets(usize, Box<ColorBuckets>),
    /// FrameLookback: 5 planes, alpha range, number of frames to look back.
    Lookback(usize, i32, i32, i32),
}

pub(crate) struct Ranges {
    pub(crate) layers: Vec<Layer>,
}

fn get_max_y(par: i32) -> i32 {
    par * 4 - 1
}
fn get_min_co(par: i32, y: i32) -> i32 {
    if y < par - 1 {
        -3 - 4 * y
    } else if y >= 3 * par {
        4 * (1 + y - 4 * par)
    } else {
        -4 * par + 1
    }
}
fn get_max_co(par: i32, y: i32) -> i32 {
    if y < par - 1 {
        3 + 4 * y
    } else if y >= 3 * par {
        4 * par - 4 * (1 + y - 3 * par)
    } else {
        4 * par - 1
    }
}
fn get_min_cg(par: i32, y: i32, co: i32) -> i32 {
    if co < get_min_co(par, y) || co > get_max_co(par, y) {
        return 8 * par;
    }
    if y < par - 1 {
        -(2 * y + 1)
    } else if y >= 3 * par {
        -(2 * (4 * par - 1 - y) - ((1 + co.abs()) / 2) * 2)
    } else {
        -std::cmp::min(
            2 * par - 1 + (y - par + 1) * 2,
            2 * par + (3 * par - 1 - y) * 2 - ((1 + co.abs()) / 2) * 2,
        )
    }
}
fn get_max_cg(par: i32, y: i32, co: i32) -> i32 {
    if co < get_min_co(par, y) || co > get_max_co(par, y) {
        return -8 * par;
    }
    if y < par - 1 {
        1 + 2 * y - (co.abs() / 2) * 2
    } else if y >= 3 * par {
        2 * (4 * par - 1 - y)
    } else {
        -std::cmp::max(
            -4 * par + (1 + y - 2 * par) * 2,
            -2 * par - (y - par) * 2 - 1 + (co.abs() / 2) * 2,
        )
    }
}

#[inline]
fn pp(v: &[i32], i: usize) -> i32 {
    v.get(i).copied().unwrap_or(0)
}

impl Ranges {
    pub(crate) fn top(&self) -> usize {
        self.layers.len() - 1
    }

    pub(crate) fn num_planes_at(&self, l: usize) -> usize {
        match &self.layers[l] {
            Layer::Static(r) => r.len(),
            Layer::Bounds(_, b) => b.len(),
            Layer::Lookback(..) => 5,
            Layer::Dup(s)
            | Layer::Compact(s, _)
            | Layer::YCoCg(s, _)
            | Layer::Permute(s, _)
            | Layer::PermuteSub(s, _)
            | Layer::Palette(s, _)
            | Layer::PaletteA(s, _)
            | Layer::Buckets(s, _) => self.num_planes_at(*s),
        }
    }

    pub(crate) fn is_static(&self, l: usize) -> bool {
        match &self.layers[l] {
            Layer::Static(_) | Layer::Compact(..) => true,
            Layer::Dup(s) => self.is_static(*s),
            _ => false,
        }
    }

    pub(crate) fn previous(&self, l: usize) -> Option<usize> {
        match &self.layers[l] {
            Layer::Dup(s)
            | Layer::Palette(s, _)
            | Layer::PaletteA(s, _)
            | Layer::Lookback(s, ..) => Some(*s),
            _ => None,
        }
    }

    pub(crate) fn min_at(&self, l: usize, p: usize) -> i32 {
        match &self.layers[l] {
            Layer::Static(r) => r.get(p).map_or(0, |r| r.0),
            Layer::Dup(s) | Layer::Buckets(s, _) => self.min_at(*s, p),
            Layer::Compact(..) => 0,
            Layer::YCoCg(s, par) => match p {
                0 => 0,
                1 | 2 => -4 * par + 1,
                _ => self.min_at(*s, p),
            },
            Layer::Permute(s, perm) => self.min_at(*s, perm.get(p).copied().unwrap_or(p)),
            Layer::PermuteSub(s, perm) => {
                let q = perm.get(p).copied().unwrap_or(p);
                if p == 0 || p > 2 {
                    self.min_at(*s, q)
                } else {
                    self.min_at(*s, q) - self.max_at(*s, perm[0])
                }
            }
            Layer::Bounds(s, b) => self.min_at(*s, p).max(b.get(p).map_or(0, |b| b.0)),
            Layer::Palette(s, _) => {
                if p < 3 {
                    0
                } else {
                    self.min_at(*s, p)
                }
            }
            Layer::PaletteA(s, _) => match p {
                0..=2 => 0,
                3 => 1,
                _ => self.min_at(*s, p),
            },
            Layer::Lookback(s, amin, ..) => match p {
                0..=2 => self.min_at(*s, p),
                3 => *amin,
                _ => 0,
            },
        }
    }

    pub(crate) fn max_at(&self, l: usize, p: usize) -> i32 {
        match &self.layers[l] {
            Layer::Static(r) => r.get(p).map_or(0, |r| r.1),
            Layer::Dup(s) | Layer::Buckets(s, _) => self.max_at(*s, p),
            Layer::Compact(_, nb) => nb.get(p).copied().unwrap_or(0),
            Layer::YCoCg(s, par) => match p {
                0..=2 => 4 * par - 1,
                _ => self.max_at(*s, p),
            },
            Layer::Permute(s, perm) => self.max_at(*s, perm.get(p).copied().unwrap_or(p)),
            Layer::PermuteSub(s, perm) => {
                let q = perm.get(p).copied().unwrap_or(p);
                if p == 0 || p > 2 {
                    self.max_at(*s, q)
                } else {
                    self.max_at(*s, q) - self.min_at(*s, perm[0])
                }
            }
            Layer::Bounds(s, b) => self.max_at(*s, p).min(b.get(p).map_or(0, |b| b.1)),
            Layer::Palette(s, nb) => match p {
                1 => nb - 1,
                0 | 2 => 0,
                _ => self.max_at(*s, p),
            },
            Layer::PaletteA(s, nb) => match p {
                1 => nb - 1,
                0 | 2 => 0,
                3 => 1,
                _ => self.max_at(*s, p),
            },
            Layer::Lookback(s, _, amax, nprev) => match p {
                0..=2 => self.max_at(*s, p),
                3 => *amax,
                _ => *nprev,
            },
        }
    }

    pub(crate) fn minmax_at(&self, l: usize, p: usize, v: &[i32]) -> (i32, i32) {
        match &self.layers[l] {
            Layer::Dup(s) => self.minmax_at(*s, p, v),
            Layer::Compact(_, nb) => (0, nb.get(p).copied().unwrap_or(0)),
            Layer::YCoCg(s, par) => match p {
                0 => (0, get_max_y(*par)),
                1 => (get_min_co(*par, pp(v, 0)), get_max_co(*par, pp(v, 0))),
                2 => (
                    get_min_cg(*par, pp(v, 0), pp(v, 1)),
                    get_max_cg(*par, pp(v, 0), pp(v, 1)),
                ),
                _ => self.minmax_at(*s, p, v),
            },
            Layer::PermuteSub(s, perm) => {
                if p == 0 || p > 2 {
                    (self.min_at(l, p), self.max_at(l, p))
                } else {
                    let q = perm.get(p).copied().unwrap_or(p);
                    (self.min_at(*s, q) - pp(v, 0), self.max_at(*s, q) - pp(v, 0))
                }
            }
            Layer::Bounds(s, b) => {
                let bp = b.get(p).copied().unwrap_or((0, 0));
                if p == 0 || p == 3 {
                    return bp;
                }
                let (mut mn, mut mx) = self.minmax_at(*s, p, v);
                mn = mn.max(bp.0);
                mx = mx.min(bp.1);
                if mn > mx { bp } else { (mn, mx) }
            }
            Layer::Palette(s, nb) => match p {
                1 => (0, nb - 1),
                0 | 2 => (0, 0),
                _ => self.minmax_at(*s, p, v),
            },
            Layer::PaletteA(s, nb) => match p {
                1 => (0, nb - 1),
                0 | 2 => (0, 0),
                3 => (1, 1),
                _ => self.minmax_at(*s, p, v),
            },
            Layer::Buckets(_, cb) => {
                let b = cb.find(p, v);
                if b.min > b.max {
                    (self.min_at(l, p), self.max_at(l, p))
                } else {
                    (b.min, b.max)
                }
            }
            Layer::Lookback(s, ..) => {
                if p >= 3 {
                    (self.min_at(l, p), self.max_at(l, p))
                } else {
                    self.minmax_at(*s, p, v)
                }
            }
            Layer::Static(_) | Layer::Permute(..) => (self.min_at(l, p), self.max_at(l, p)),
        }
    }

    /// The conditional range of `p` given the earlier channels' values `v` and the
    /// prediction `guess` snapped into it: `(min, max, guess)`.
    pub(crate) fn snap_at(&self, l: usize, p: usize, v: &[i32], guess: i32) -> (i32, i32, i32) {
        match &self.layers[l] {
            Layer::Dup(s) => self.snap_at(*s, p, v, guess),
            Layer::Bounds(s, b) => {
                let bp = b.get(p).copied().unwrap_or((0, 0));
                let (mn, mx, mut g) = if p == 0 || p == 3 {
                    (bp.0, bp.1, guess)
                } else {
                    let (mut mn, mut mx, g) = self.snap_at(*s, p, v, guess);
                    mn = mn.max(bp.0);
                    mx = mx.min(bp.1);
                    if mn > mx {
                        (bp.0, bp.1, g)
                    } else {
                        (mn, mx, g)
                    }
                };
                if g > mx {
                    g = mx;
                }
                if g < mn {
                    g = mn;
                }
                (mn, mx.max(mn), g)
            }
            Layer::Buckets(_, cb) => {
                let b = cb.find(p, v);
                if b.min > b.max {
                    let mn = self.min_at(l, p);
                    (mn, self.max_at(l, p), mn)
                } else {
                    (b.min, b.max, b.snap(guess))
                }
            }
            Layer::Lookback(s, ..) if p < 3 => self.snap_at(*s, p, v, guess),
            _ => {
                let (mn, mut mx) = self.minmax_at(l, p, v);
                if mn > mx {
                    mx = mn;
                }
                (mn, mx, guess.clamp(mn, mx))
            }
        }
    }

    pub(crate) fn num_planes(&self) -> usize {
        self.num_planes_at(self.top())
    }
    pub(crate) fn min(&self, p: usize) -> i32 {
        self.min_at(self.top(), p)
    }
    pub(crate) fn max(&self, p: usize) -> i32 {
        self.max_at(self.top(), p)
    }
}

/// A set of values of one channel in one colour bucket.
#[derive(Clone)]
pub(crate) struct ColorBucket {
    /// Sorted, when `discrete`.
    pub(crate) values: Vec<i32>,
    pub(crate) min: i32,
    pub(crate) max: i32,
    pub(crate) discrete: bool,
}

impl Default for ColorBucket {
    fn default() -> Self {
        ColorBucket {
            values: Vec::new(),
            min: 10000,
            max: -10000,
            discrete: true,
        }
    }
}

impl ColorBucket {
    pub(crate) fn empty(&self) -> bool {
        self.min > self.max
    }

    /// The value of the bucket closest to `c` (the lower one on a tie).
    pub(crate) fn snap(&self, c: i32) -> i32 {
        if c <= self.min {
            return self.min;
        }
        if c >= self.max {
            return self.max;
        }
        if self.discrete && !self.values.is_empty() {
            let i = self.values.partition_point(|&v| v < c);
            if self.values.get(i) == Some(&c) {
                return c;
            }
            if i == 0 {
                return self.values[0];
            }
            let lo = self.values[i - 1];
            match self.values.get(i) {
                Some(&hi) if hi - c < c - lo => hi,
                _ => lo,
            }
        } else {
            c
        }
    }

    pub(crate) fn remove(&mut self, c: i32) {
        if self.discrete {
            if let Some(pos) = self.values.iter().position(|&v| v == c) {
                self.values.remove(pos);
            }
            if self.values.is_empty() {
                self.min = 10000;
                self.max = -10000;
                return;
            }
            if c == self.min {
                self.min = self.values[0];
            }
            if c == self.max {
                self.max = self.values[self.values.len() - 1];
            }
        } else {
            if c == self.min {
                self.min += 1;
            }
            if c == self.max {
                self.max -= 1;
            }
            if c > self.max || c < self.min {
                return;
            }
            self.discrete = true;
            self.values = (self.min..=self.max).filter(|&x| x != c).collect();
        }
    }
}

/// The colour buckets of the ColorBuckets transform.
pub(crate) struct ColorBuckets {
    pub(crate) bucket0: ColorBucket,
    pub(crate) min0: i32,
    pub(crate) min1: i32,
    pub(crate) bucket1: Vec<ColorBucket>,
    /// `bucket2[v0 - min0][(v1 - min1) / 4]`, flattened.
    pub(crate) bucket2: Vec<ColorBucket>,
    pub(crate) b2_cols: usize,
    pub(crate) bucket3: ColorBucket,
    empty: ColorBucket,
}

impl ColorBuckets {
    pub(crate) fn new(min0: i32, max0: i32, min1: i32, max1: i32) -> ColorBuckets {
        let n0 = (max0 - min0 + 1).max(0) as usize;
        let n1 = ((max1 - min1) / 4 + 1).max(0) as usize;
        ColorBuckets {
            bucket0: ColorBucket::default(),
            min0,
            min1,
            bucket1: vec![ColorBucket::default(); n0],
            bucket2: vec![ColorBucket::default(); n0 * n1],
            b2_cols: n1,
            bucket3: ColorBucket::default(),
            empty: ColorBucket::default(),
        }
    }

    pub(crate) fn index2(&self, v0: i32, v1: i32) -> Option<usize> {
        let i = v0 - self.min0;
        let j = (v1 - self.min1) / 4;
        if i < 0 || j < 0 || i as usize >= self.bucket1.len() || j as usize >= self.b2_cols {
            return None;
        }
        Some(i as usize * self.b2_cols + j as usize)
    }

    pub(crate) fn find(&self, p: usize, v: &[i32]) -> &ColorBucket {
        match p {
            0 => &self.bucket0,
            1 => {
                let i = pp(v, 0) - self.min0;
                if i >= 0 && (i as usize) < self.bucket1.len() {
                    &self.bucket1[i as usize]
                } else {
                    &self.empty
                }
            }
            2 => match self.index2(pp(v, 0), pp(v, 1)) {
                Some(k) => &self.bucket2[k],
                None => &self.empty,
            },
            _ => &self.bucket3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_snap_matches_linear_search() {
        let b = ColorBucket {
            values: vec![-1, 3, 4, 6, 8],
            min: -1,
            max: 8,
            discrete: true,
        };
        assert_eq!(b.snap(-100), -1);
        assert_eq!(b.snap(5), 4);
        assert_eq!(b.snap(6), 6);
        assert_eq!(b.snap(100), 8);
        assert_eq!(b.snap(1), -1);
        assert_eq!(b.snap(2), 3);
        assert_eq!(b.snap(7), 6);
    }

    #[test]
    fn ycocg_ranges_8bit() {
        let r = Ranges {
            layers: vec![Layer::Static(vec![(0, 255); 3]), Layer::YCoCg(0, 64)],
        };
        assert_eq!((r.min(0), r.max(0)), (0, 255));
        assert_eq!((r.min(1), r.max(1)), (-255, 255));
        let (mn, mx, g) = r.snap_at(1, 1, &[0], 50);
        assert_eq!((mn, mx, g), (-3, 3, 3));
    }
}

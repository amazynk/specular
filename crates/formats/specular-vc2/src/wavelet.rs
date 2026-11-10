//! The inverse wavelet transform (Dirac specification 2.2.3 §15.6): at each level the four
//! subbands are interleaved, synthesised vertically (columns) and then horizontally (rows) by
//! the wavelet's lifting stages (Tables 15.1–15.7), and the filter's accuracy bits shifted out.
//!
//! The vertical pass keeps the even rows (LL, HL) and odd rows (LH, HH) in two arrays, so a
//! stage updates one array from the other a whole row at a time and rows run in parallel.
//! Arithmetic wraps: damaged streams give damaged pictures, never a panic.

use rayon::prelude::*;

/// Which samples a lifting stage updates and how (§15.6.2, types 1–4).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Type 1: even samples += filtered odd samples.
    EvenAdd,
    /// Type 2: even samples −= filtered odd samples.
    EvenSub,
    /// Type 3: odd samples += filtered even samples.
    OddAdd,
    /// Type 4: odd samples −= filtered even samples.
    OddSub,
}

struct Stage {
    kind: Kind,
    /// Scale (right shift) S.
    s: u32,
    /// Offset D of the first tap.
    d: i32,
    taps: &'static [i32],
}

struct Filter {
    stages: &'static [Stage],
    shift: u32,
}

const fn st(kind: Kind, s: u32, d: i32, taps: &'static [i32]) -> Stage {
    Stage { kind, s, d, taps }
}

/// Tables 15.1–15.7, by wavelet index.
static FILTERS: [Filter; 7] = [
    // 0: Deslauriers-Dubuc (9,7)
    Filter {
        stages: &[
            st(Kind::EvenSub, 2, 0, &[1, 1]),
            st(Kind::OddAdd, 4, -1, &[-1, 9, 9, -1]),
        ],
        shift: 1,
    },
    // 1: LeGall (5,3)
    Filter {
        stages: &[
            st(Kind::EvenSub, 2, 0, &[1, 1]),
            st(Kind::OddAdd, 1, 0, &[1, 1]),
        ],
        shift: 1,
    },
    // 2: Deslauriers-Dubuc (13,7)
    Filter {
        stages: &[
            st(Kind::EvenSub, 5, -1, &[-1, 9, 9, -1]),
            st(Kind::OddAdd, 4, -1, &[-1, 9, 9, -1]),
        ],
        shift: 1,
    },
    // 3: Haar with no shift
    Filter {
        stages: &[st(Kind::EvenSub, 1, 1, &[1]), st(Kind::OddAdd, 0, 0, &[1])],
        shift: 0,
    },
    // 4: Haar with a single shift per level
    Filter {
        stages: &[st(Kind::EvenSub, 1, 1, &[1]), st(Kind::OddAdd, 0, 0, &[1])],
        shift: 1,
    },
    // 5: Fidelity
    Filter {
        stages: &[
            st(Kind::OddAdd, 8, -3, &[-2, 10, -25, 81, 81, -25, 10, -2]),
            st(Kind::EvenSub, 8, -3, &[-8, 21, -46, 161, 161, -46, 21, -8]),
        ],
        shift: 0,
    },
    // 6: Daubechies (9,7) integer approximation
    Filter {
        stages: &[
            st(Kind::EvenSub, 12, 0, &[1817, 1817]),
            st(Kind::OddSub, 12, 0, &[3616, 3616]),
            st(Kind::EvenAdd, 12, 0, &[217, 217]),
            st(Kind::OddAdd, 12, 0, &[6497, 6497]),
        ],
        shift: 1,
    },
];

/// The number of wavelet filters (valid indices are below it).
pub(crate) const COUNT: u32 = FILTERS.len() as u32;

impl Stage {
    fn rounding(&self) -> i32 {
        if self.s > 0 { 1 << (self.s - 1) } else { 0 }
    }

    fn even(&self) -> bool {
        matches!(self.kind, Kind::EvenAdd | Kind::EvenSub)
    }

    fn apply(&self, target: i32, sum: i32) -> i32 {
        let v = sum.wrapping_add(self.rounding()) >> self.s;
        match self.kind {
            Kind::EvenAdd | Kind::OddAdd => target.wrapping_add(v),
            Kind::EvenSub | Kind::OddSub => target.wrapping_sub(v),
        }
    }
}

/// One-dimensional synthesis of an interleaved row of even length.
fn synth_row(f: &Filter, a: &mut [i32]) {
    let m = a.len() / 2;
    if m == 0 {
        return;
    }
    let last = m as i32 - 1;
    for stage in f.stages {
        let even = stage.even();
        for n in 0..m {
            let mut sum = 0i32;
            for (k, &t) in stage.taps.iter().enumerate() {
                let i = n as i32 + stage.d + k as i32;
                // Even updates read odd samples 2(n+i)−1, odd ones even samples 2(n+i),
                // both clamped to the array (edge extension within the phase).
                let pos = if even {
                    2 * (i - 1).clamp(0, last) as usize + 1
                } else {
                    2 * i.clamp(0, last) as usize
                };
                sum = sum.wrapping_add(t.wrapping_mul(a[pos]));
            }
            let at = if even { 2 * n } else { 2 * n + 1 };
            a[at] = stage.apply(a[at], sum);
        }
    }
}

/// `vh_synth`: four `w`×`h` subbands to one `2w`×`2h` array.
fn vh_synth(
    f: &Filter,
    w: usize,
    h: usize,
    ll: &[i32],
    hl: &[i32],
    lh: &[i32],
    hh: &[i32],
) -> Vec<i32> {
    let w2 = 2 * w;
    // Even rows: LL and HL interleaved; odd rows: LH and HH.
    let mut e = vec![0i32; w2 * h];
    let mut o = vec![0i32; w2 * h];
    for y in 0..h {
        let (er, or) = (&mut e[y * w2..][..w2], &mut o[y * w2..][..w2]);
        for x in 0..w {
            er[2 * x] = ll[y * w + x];
            er[2 * x + 1] = hl[y * w + x];
            or[2 * x] = lh[y * w + x];
            or[2 * x + 1] = hh[y * w + x];
        }
    }
    let last = h as i32 - 1;
    let min_rows = (16_384 / w2.max(1)).max(1);
    for stage in f.stages {
        let (dst, src) = if stage.even() {
            (&mut e, &o)
        } else {
            (&mut o, &e)
        };
        let even = stage.even();
        dst.par_chunks_mut(w2)
            .with_min_len(min_rows)
            .enumerate()
            .for_each(|(n, row)| {
                let mut sum = vec![0i32; w2];
                for (k, &t) in stage.taps.iter().enumerate() {
                    let i = n as i32 + stage.d + k as i32;
                    let r = if even {
                        (i - 1).clamp(0, last)
                    } else {
                        i.clamp(0, last)
                    } as usize;
                    let s = &src[r * w2..][..w2];
                    for (a, &b) in sum.iter_mut().zip(s) {
                        *a = a.wrapping_add(t.wrapping_mul(b));
                    }
                }
                for (v, &s) in row.iter_mut().zip(&sum) {
                    *v = stage.apply(*v, s);
                }
            });
    }
    let mut out = vec![0i32; w2 * 2 * h];
    let shift = f.shift;
    out.par_chunks_mut(w2)
        .with_min_len(min_rows)
        .enumerate()
        .for_each(|(y, row)| {
            let src = if y % 2 == 0 { &e } else { &o };
            row.copy_from_slice(&src[(y / 2) * w2..][..w2]);
            synth_row(f, row);
            if shift > 0 {
                let r = 1i32 << (shift - 1);
                for v in row.iter_mut() {
                    *v = v.wrapping_add(r) >> shift;
                }
            }
        });
    out
}

/// The subbands of one component: level 0 holds LL; every other level HL, LH, HH. `w`×`h` is
/// the size of the level 0 band; each level doubles it.
pub(crate) struct Bands {
    pub w: usize,
    pub h: usize,
    pub depth: usize,
    /// `[level][orient]`, orient 0 for LL at level 0, else 0 = HL, 1 = LH, 2 = HH.
    pub data: Vec<[Vec<i32>; 3]>,
}

impl Bands {
    /// Zeroed subbands for a padded `pw`×`ph` component at `depth` levels.
    pub fn new(pw: usize, ph: usize, depth: usize) -> Bands {
        let (w, h) = (pw >> depth, ph >> depth);
        let mut data = Vec::with_capacity(depth + 1);
        data.push([vec![0; w * h], Vec::new(), Vec::new()]);
        for level in 1..=depth {
            let (bw, bh) = (w << (level - 1), h << (level - 1));
            data.push([vec![0; bw * bh], vec![0; bw * bh], vec![0; bw * bh]]);
        }
        Bands { w, h, depth, data }
    }

    /// The size of the bands at `level`.
    pub fn size(&self, level: usize) -> (usize, usize) {
        let k = level.saturating_sub(1);
        (self.w << k, self.h << k)
    }

    /// The inverse transform with wavelet `index`: the padded component.
    pub fn synthesise(mut self, index: u32) -> Vec<i32> {
        let f = &FILTERS[index as usize];
        let mut ll = std::mem::take(&mut self.data[0][0]);
        for level in 1..=self.depth {
            let (w, h) = self.size(level);
            let [hl, lh, hh] = &self.data[level];
            ll = vh_synth(f, w, h, &ll, hl, lh, hh);
        }
        ll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat picture: only the DC band set; the interpolating filters give it back flat
    /// (Fidelity and Daubechies scale DC by other factors).
    #[test]
    fn flat_inverts_to_flat() {
        assert_eq!(COUNT, 7);
        for index in 0..5 {
            let mut b = Bands::new(16, 8, 2);
            let f = &FILTERS[index as usize];
            // Each level's synthesis halves by its shift; the DC is scaled up to match.
            let dc = 100 << (2 * f.shift);
            b.data[0][0].fill(dc);
            let out = b.synthesise(index);
            assert_eq!(out.len(), 128);
            assert!(out.iter().all(|&v| v == 100), "wavelet {index}: {out:?}");
        }
    }
}

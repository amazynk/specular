//! Inverse wavelet transform: each region of the tree is rebuilt from its low and high bands,
//! first down the columns, then along the rows, with the symmetric extension the
//! specification gives at both ends (whole- or half-sample, by filter and line parity; even
//! filters use antisymmetric extension for the high band).
//!
//! For a line length the position of every input sample and the tap it meets are the same on
//! every line, so they are worked out once per pass as a [`Plan`]: per output sample, the list
//! of `(input index, coefficient)` terms in the order the sum is formed. Applying a plan is
//! then a plain multiply-add over whole rows (columns pass) or along each row (rows pass),
//! split over threads by rows. Sums are formed term by term in single precision, low band
//! first, in the reference's order, so the result matches it bit for bit.

use crate::parse::Filters;
use crate::tree::Node;
use rayon::prelude::*;

pub(crate) struct Plan {
    /// Terms of output sample `i`: `terms[starts[i]..starts[i + 1]]`.
    starts: Vec<u32>,
    terms: Vec<(i32, f32)>,
}

/// The walk over the bands of one line of `len` samples (low band first unless `inv`).
struct Walk<'f> {
    lo: &'f [f32],
    /// The high synthesis filter, negated for even-length filters.
    hi: Vec<f32>,
    asym: bool,
    odd_len: bool,
    llen: isize,
    hlen: isize,
    inv: bool,
    /// Terms per output position, as the sums are formed.
    out: Vec<Vec<(i32, f32)>>,
    /// Next position the low / high band writes.
    limg: usize,
    himg: usize,
    /// The running edge factor of even-filter high bands (as the reference keeps it).
    fhre: i32,
}

impl Walk<'_> {
    // Positions past a band's ends only occur on lines of a few samples (tiny images); they
    // read whatever lies there, as the reference does, so they are kept as they are.
    fn low_index(&self, j: isize) -> Option<i32> {
        Some((if self.inv { self.hlen + j } else { j }) as i32)
    }
    fn high_index(&self, j: isize) -> Option<i32> {
        Some((if self.inv { j } else { self.llen + j }) as i32)
    }

    /// One low-band output: starts a new sum at the next low position.
    fn low(&mut self, tap: usize, start: isize, step: isize, mut le: bool, mut re: bool) {
        let mut px = start;
        let mut st = step;
        let mut terms = Vec::with_capacity(self.lo.len() / 2 + 1);
        if let Some(i) = self.low_index(px) {
            terms.push((i, self.lo[tap]));
        }
        let mut k = tap + 2;
        while k < self.lo.len() {
            if px == 0 {
                if le {
                    st = 0;
                    le = false;
                } else {
                    st = 1;
                }
            }
            if px == self.llen - 1 {
                if re {
                    st = 0;
                    re = false;
                } else {
                    st = -1;
                }
            }
            px += st;
            if let Some(i) = self.low_index(px) {
                terms.push((i, self.lo[k]));
            }
            k += 2;
        }
        if let Some(o) = self.out.get_mut(self.limg) {
            *o = terms;
        }
        self.limg += 1;
    }

    /// One high-band output: adds to the sum at the next high position.
    #[allow(clippy::too_many_arguments)]
    fn high(
        &mut self,
        tap: usize,
        start: isize,
        step: isize,
        mut le: bool,
        mut re: bool,
        mut sfac: f32,
    ) {
        let mut px = start;
        let mut st = step;
        let mut terms = Vec::with_capacity(self.hi.len() / 2 + 1);
        let mut k = tap;
        while k < self.hi.len() {
            if px == 0 {
                if le {
                    st = 0;
                    le = false;
                } else {
                    st = 1;
                    sfac = 1.0;
                }
            }
            if px == self.hlen - 1 {
                if re {
                    st = 0;
                    re = false;
                    if self.asym && self.odd_len {
                        re = true;
                        self.fhre -= 1;
                        sfac = self.fhre as f32;
                        if sfac == 0.0 {
                            re = false;
                        }
                    }
                } else {
                    st = -1;
                    if self.asym {
                        sfac = -1.0;
                    }
                }
            }
            // (sample · tap) · factor with factor ±1 or 0 equals sample · (tap · factor).
            if let Some(i) = self.high_index(px) {
                terms.push((i, self.hi[k] * sfac));
            }
            px += st;
            k += 2;
        }
        if let Some(o) = self.out.get_mut(self.himg) {
            o.extend(terms);
        }
        self.himg += 1;
    }
}

impl Plan {
    pub fn new(len: usize, inv: bool, f: &Filters) -> Plan {
        let odd_len = len % 2 == 1;
        let llen = len.div_ceil(2) as isize;
        let hlen = (len / 2) as isize;
        let lsz = f.lo.len() as isize;
        let hsz = f.hi.len() as isize;
        let asym = lsz % 2 == 0;
        let (mut loc, mut hoc, lotap, hotap, mut olle, olre, mut ohle, ohre, ssfac, ofhre);
        if !asym {
            loc = (lsz - 1) / 4;
            hoc = (hsz + 1) / 4 - 1;
            lotap = (((lsz - 1) / 2) % 2) as usize;
            hotap = (((hsz + 1) / 2) % 2) as usize;
            (olle, olre, ohle, ohre) = if odd_len {
                (false, false, true, true)
            } else {
                (false, true, true, false)
            };
            ssfac = 1.0f32;
            ofhre = 0;
        } else {
            loc = lsz / 4 - 1;
            hoc = hsz / 4 - 1;
            lotap = ((lsz / 2) % 2) as usize;
            hotap = ((hsz / 2) % 2) as usize;
            (olle, olre, ohle, ohre) = if odd_len {
                (true, false, true, true)
            } else {
                (true, true, true, true)
            };
            if loc == -1 {
                loc = 0;
                olle = false;
            }
            if hoc == -1 {
                hoc = 0;
                ohle = false;
            }
            ssfac = -1.0;
            ofhre = 2;
        }
        let mut w = Walk {
            lo: &f.lo,
            hi: if asym {
                f.hi.iter().map(|v| -v).collect()
            } else {
                f.hi.clone()
            },
            asym,
            odd_len,
            llen,
            hlen,
            inv,
            out: vec![Vec::new(); len],
            limg: 0,
            himg: 0,
            fhre: 0,
        };
        let (mut lspx, mut lspxstr, mut lstap, mut lle2, lre2) = (loc, -1isize, lotap, olle, olre);
        let (mut hspx, mut hspxstr, mut hstap, mut hle2, hre2) = (hoc, -1isize, hotap, ohle, ohre);
        let mut osfac = ssfac;
        for _ in 0..hlen {
            for tap in (0..=lstap).rev() {
                w.low(tap, lspx, lspxstr, lle2, lre2);
            }
            if lspx == 0 {
                if lle2 {
                    lspxstr = 0;
                    lle2 = false;
                } else {
                    lspxstr = 1;
                }
            }
            lspx += lspxstr;
            lstap = 1;
            for tap in (0..=hstap).rev() {
                w.fhre = ofhre;
                w.high(tap, hspx, hspxstr, hle2, hre2, osfac);
            }
            if hspx == 0 {
                if hle2 {
                    hspxstr = 0;
                    hle2 = false;
                } else {
                    hspxstr = 1;
                    osfac = 1.0;
                }
            }
            hspx += hspxstr;
            hstap = 1;
        }
        let last_l = match (odd_len, lotap == 1) {
            (true, true) => 1,
            (true, false) => 0,
            (false, true) => 2,
            (false, false) => 1,
        };
        for tap in (last_l..=1).rev() {
            w.low(tap, lspx, lspxstr, lle2, lre2);
        }
        let last_h = match (odd_len, hotap == 1) {
            (true, true) => 1,
            (true, false) => 0,
            (false, true) => 2,
            (false, false) => 1,
        };
        if odd_len && hsz == 2 {
            hspx -= hspxstr;
            w.fhre = 1;
        }
        for tap in (last_h..=1).rev() {
            if hsz != 2 {
                w.fhre = ofhre;
            }
            w.high(tap, hspx, hspxstr, hle2, hre2, osfac);
        }
        let mut starts = Vec::with_capacity(len + 1);
        let mut terms = Vec::new();
        for o in w.out {
            starts.push(terms.len() as u32);
            terms.extend(o);
        }
        starts.push(terms.len() as u32);
        Plan { starts, terms }
    }

    #[inline]
    fn of(&self, i: usize) -> &[(i32, f32)] {
        &self.terms[self.starts[i] as usize..self.starts[i + 1] as usize]
    }
}

/// Rebuilds the image in place from its subbands (`img`: `width` wide, coefficients at their
/// subband positions).
///
/// The columns pass writes into a scratch image of the same size, placed at its top left, and
/// the rows pass reads from there: lines of a few samples can reach past their band, and then
/// read the neighbouring samples of these buffers, as the reference does.
pub(crate) fn reconstruct(img: &mut [f32], width: usize, tree: &[Node; 20], f: &Filters) {
    let height = img.len() / width.max(1);
    let mut tmp = vec![0f32; img.len()];
    for node in tree.iter().rev() {
        let r = node.r;
        if r.w == 0 || r.h == 0 || r.x + r.w > width || r.y + r.h > height {
            continue;
        }
        // Down the columns: output row k of the region from whole input rows.
        let plan = Plan::new(r.h, node.inv_cl, f);
        {
            let src = &*img;
            let row = |k: i32| -> Option<&[f32]> {
                let y = r.y as isize + k as isize;
                (0..height as isize)
                    .contains(&y)
                    .then(|| &src[y as usize * width + r.x..][..r.w])
            };
            tmp.par_chunks_mut(width)
                .take(r.h)
                .enumerate()
                .for_each(|(k, out)| {
                    let out = &mut out[..r.w];
                    out.fill(0.0);
                    let mut first = true;
                    for &(i, c) in plan.of(k) {
                        if let Some(s) = row(i) {
                            if first {
                                for (o, s) in out.iter_mut().zip(s) {
                                    *o = s * c;
                                }
                            } else {
                                for (o, s) in out.iter_mut().zip(s) {
                                    *o += s * c;
                                }
                            }
                        }
                        first = false;
                    }
                });
        }
        // Along the rows, back into the image.
        let plan = Plan::new(r.w, node.inv_rw, f);
        let tmp = &tmp;
        img.par_chunks_mut(width)
            .skip(r.y)
            .take(r.h)
            .enumerate()
            .for_each(|(k, line)| {
                let base = (k * width) as isize;
                let at = |i: i32| -> f32 {
                    let p = base + i as isize;
                    if (0..tmp.len() as isize).contains(&p) {
                        tmp[p as usize]
                    } else {
                        0.0
                    }
                };
                let inside = &tmp[k * width..][..r.w];
                for (p, o) in line[r.x..r.x + r.w].iter_mut().enumerate() {
                    let mut v = 0f32;
                    for (n, &(i, c)) in plan.of(p).iter().enumerate() {
                        let s = match inside.get(i as usize) {
                            Some(&s) if i >= 0 => s,
                            _ => at(i),
                        };
                        if n == 0 {
                            v = s * c;
                        } else {
                            v += s * c;
                        }
                    }
                    *o = v;
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every output sample gets terms, all inside the line, for odd and even filters.
    #[test]
    fn plans_cover_every_sample() {
        for (h, l) in [(9, 7), (7, 9), (5, 3), (2, 2), (4, 4), (6, 2), (8, 6)] {
            let f = Filters {
                hi: (0..h).map(|i| 1.0 + i as f32).collect(),
                lo: (0..l).map(|i| 1.0 + i as f32).collect(),
            };
            for len in 2..40 {
                for inv in [false, true] {
                    let p = Plan::new(len, inv, &f);
                    for i in 0..len {
                        let t = p.of(i);
                        assert!(!t.is_empty(), "{h}/{l} len {len} inv {inv} at {i}");
                        if len >= 2 * h.max(l) {
                            assert!(t.iter().all(|&(s, _)| (0..len as i32).contains(&s)));
                        }
                    }
                }
            }
        }
    }
}

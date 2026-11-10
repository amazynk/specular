//! Pixel data: the non-interlaced and interlaced scans, prediction and MANIAC properties,
//! and the interpolation that finishes a truncated interlaced file.

use crate::Error;
use crate::image::{Img, colps, rowps};
use crate::maniac::{PropertyCoder, Tree, read_tree};
use crate::rac::{ChanceTable, Rac};
use crate::ranges::Ranges;
use crate::transform::{Undo, undo};

const PLANE_ORDER: [usize; 5] = [4, 3, 0, 1, 2];
const NB_PROPS_SCAN: [usize; 5] = [7, 8, 9, 7, 7];
const NB_PROPS_SCAN_A: [usize; 5] = [8, 9, 10, 7, 7];
const NB_PROPS: [usize; 5] = [8, 10, 9, 8, 8];
const NB_PROPS_A: [usize; 5] = [9, 11, 10, 8, 8];

#[inline]
fn median3(a: i32, b: i32, c: i32) -> i32 {
    if a < b {
        if b < c {
            b
        } else if a < c {
            c
        } else {
            a
        }
    } else if a < c {
        a
    } else if b < c {
        c
    } else {
        b
    }
}

/// How the decode ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    /// Every pixel decoded.
    Complete,
    /// The data ran out; the rest was interpolated.
    Truncated,
    /// Stopped once the zoomlevel asked for was complete (a downscaled decode).
    Scaled,
}

pub(crate) struct Dec<'a, 'r> {
    pub(crate) rac: &'r mut Rac<'a>,
    pub(crate) table: ChanceTable,
    pub(crate) ranges: &'r Ranges,
    pub(crate) frames: &'r mut Vec<Img>,
    /// The images' `alpha_zero_special` (after the transforms adjusted it).
    pub(crate) alpha_zero: bool,
    pub(crate) inv_pred: usize,
    pub(crate) undos: &'r mut Vec<Undo>,
    /// Interlaced: stop once every channel has zoomlevel `stop_zl` (0 = full).
    pub(crate) stop_zl: usize,
    pub(crate) cancel: Option<&'r (dyn Fn() -> bool + Sync)>,
    pub(crate) pixels_done: u64,
    pub(crate) pixels_todo: u64,
    pub(crate) max_tree_nodes: usize,
}

fn prop_ranges_scan(r: &Ranges, p: usize) -> Vec<(i32, i32)> {
    let (min, max) = (r.min(p), r.max(p));
    let (mind, maxd) = (min - max, max - min);
    let mut v = Vec::new();
    if p < 3 {
        for pp in 0..p {
            v.push((r.min(pp), r.max(pp)));
        }
        if r.num_planes() > 3 {
            v.push((r.min(3), r.max(3)));
        }
    }
    v.push((min, max));
    v.push((0, 2));
    for _ in 0..5 {
        v.push((mind, maxd));
    }
    v
}

fn prop_ranges_interlaced(r: &Ranges, p: usize) -> Vec<(i32, i32)> {
    let (min, max) = (r.min(p), r.max(p));
    let (mind, maxd) = (min - max, max - min);
    let mut v = Vec::new();
    if p < 3 {
        for pp in 0..p {
            v.push((r.min(pp), r.max(pp)));
        }
        if r.num_planes() > 3 {
            v.push((r.min(3), r.max(3)));
        }
    }
    v.push((0, 2));
    if p == 1 || p == 2 {
        v.push((r.min(0) - r.max(0), r.max(0) - r.min(0)));
    }
    for _ in 0..4 {
        v.push((mind, maxd));
    }
    v.push((min, max));
    if p != 2 {
        v.push((mind, maxd));
        v.push((mind, maxd));
    }
    v
}

impl Dec<'_, '_> {
    fn nump(&self) -> usize {
        self.ranges.num_planes()
    }
    fn trivial(&self, p: usize) -> bool {
        self.ranges.min(p) >= self.ranges.max(p)
    }
    fn check_cancel(&self) -> Result<(), Error> {
        match self.cancel {
            Some(c) if c() => Err(Error::Cancelled),
            _ => Ok(()),
        }
    }

    pub(crate) fn read_trees(&mut self, interlaced: bool) -> Result<Vec<Tree>, Error> {
        let n = self.nump();
        let mut forest = vec![Tree::default(); n];
        for (p, tree) in forest.iter_mut().enumerate() {
            let pr = if interlaced {
                prop_ranges_interlaced(self.ranges, p)
            } else {
                prop_ranges_scan(self.ranges, p)
            };
            if !self.trivial(p) {
                *tree = read_tree(self.rac, &pr, self.max_tree_nodes)?;
            }
        }
        Ok(forest)
    }

    // ---------------------------------------------------------------- non-interlaced

    pub(crate) fn scanlines(&mut self, forest: &[Tree]) -> Result<End, Error> {
        let nump = self.nump();
        let fra = nump == 5;
        let (w, h) = (self.frames[0].w, self.frames[0].h);
        for &p in &PLANE_ORDER {
            if p >= nump || self.trivial(p) {
                continue;
            }
            let mut coder = PropertyCoder::new(&forest[p]);
            let nprops = if nump > 3 {
                NB_PROPS_SCAN_A[p]
            } else {
                NB_PROPS_SCAN[p]
            };
            let mut props = vec![0i32; nprops];
            let min_p = self.ranges.min(p);
            let grey = (self.ranges.min(p) + self.ranges.max(p)) / 2;
            for r in 0..h {
                self.check_cancel()?;
                for fr in 0..self.frames.len() {
                    let (before, rest) = self.frames.split_at_mut(fr);
                    let img = &mut rest[0];
                    if fr > 0 && img.seen_before >= 0 {
                        let sb = img.seen_before as usize;
                        let src = &before[sb.min(fr - 1)].planes[p];
                        for c in 0..w {
                            let i = r * w + c;
                            let v = src.get(i);
                            img.planes[p].set(i, v);
                        }
                        continue;
                    }
                    let mut plane = std::mem::take(&mut img.planes[p].data);
                    let store = img.planes[p].store;
                    let alpha_zero = self.alpha_zero && p < 3 && nump > 3;
                    let alpha_at = |img: &Img, i: usize| if nump > 3 { img.at(3, i) } else { 1 };
                    let (mut begin, mut end) = (0usize, w);
                    if fr > 0 {
                        begin = (img.col_begin[r] as usize).min(w);
                        end = (img.col_end[r] as usize).clamp(begin, w);
                        let prev = &before[fr - 1];
                        for c in 0..begin {
                            let i = r * w + c;
                            if alpha_zero {
                                if alpha_at(img, i) == 0 {
                                    plane[i] = store.wrap(predict_scan(&plane, w, r, c, grey));
                                } else {
                                    plane[i] = store.wrap(prev.at(p, i));
                                }
                            } else if p != 4 {
                                plane[i] = store.wrap(prev.at(p, i));
                            }
                        }
                    }
                    for c in begin..end {
                        let i = r * w + c;
                        if alpha_zero && alpha_at(img, i) == 0 {
                            plane[i] = store.wrap(predict_scan(&plane, w, r, c, grey));
                            continue;
                        }
                        if fra && p < 4 {
                            let k = img.at(4, i) as usize;
                            if k > 0 {
                                let v = if k <= fr { before[fr - k].at(p, i) } else { 0 };
                                plane[i] = store.wrap(v);
                                continue;
                            }
                        }
                        let (guess, min, mut max) =
                            Self::props_scan(self.ranges, &mut props, img, &plane, p, r, c, min_p);
                        if fra && p == 4 && max > fr as i32 {
                            max = fr as i32;
                        }
                        let v = coder.read(self.rac, &self.table, &props, min - guess, max - guess)
                            + guess;
                        plane[i] = store.wrap(v);
                    }
                    if fr > 0 {
                        let prev = &before[fr - 1];
                        for c in end..w {
                            let i = r * w + c;
                            if alpha_zero {
                                if alpha_at(img, i) == 0 {
                                    plane[i] = store.wrap(predict_scan(&plane, w, r, c, grey));
                                } else {
                                    plane[i] = store.wrap(prev.at(p, i));
                                }
                            } else if p != 4 {
                                plane[i] = store.wrap(prev.at(p, i));
                            }
                        }
                    }
                    img.planes[p].data = plane;
                }
            }
        }
        Ok(if self.rac.eof() {
            End::Truncated
        } else {
            End::Complete
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn props_scan(
        ranges: &Ranges,
        props: &mut [i32],
        img: &Img,
        plane: &[i32],
        p: usize,
        r: usize,
        c: usize,
        fallback: i32,
    ) -> (i32, i32, i32) {
        let w = img.w;
        let i = r * w + c;
        let mut k = 0;
        if p < 3 {
            for pp in 0..p {
                props[k] = img.at(pp, i);
                k += 1;
            }
            if img.num() > 3 {
                props[k] = img.at(3, i);
                k += 1;
            }
        }
        let left = if c > 0 {
            plane[i - 1]
        } else if r > 0 {
            plane[i - w]
        } else {
            fallback
        };
        let top = if r > 0 { plane[i - w] } else { left };
        let topleft = if r > 0 && c > 0 {
            plane[i - w - 1]
        } else if r > 0 {
            top
        } else {
            left
        };
        let grad = left + top - topleft;
        let guess0 = median3(grad, left, top);
        let (min, max, guess) =
            ranges.snap_at(ranges.top(), p, &props[..k.max(2).min(props.len())], guess0);
        let which = if guess == grad {
            0
        } else if guess == left {
            1
        } else if guess == top {
            2
        } else {
            0
        };
        props[k] = guess;
        props[k + 1] = which;
        if c > 0 && r > 0 {
            props[k + 2] = left - topleft;
            props[k + 3] = topleft - top;
        } else {
            props[k + 2] = 0;
            props[k + 3] = 0;
        }
        props[k + 4] = if c + 1 < w && r > 0 {
            top - plane[i - w + 1]
        } else {
            0
        };
        props[k + 5] = if r > 1 { plane[i - 2 * w] - top } else { 0 };
        props[k + 6] = if c > 1 { plane[i - 2] - left } else { 0 };
        (guess, min, max)
    }

    // ---------------------------------------------------------------- interlaced

    pub(crate) fn interlaced(&mut self) -> Result<End, Error> {
        let nump = self.nump();
        let zooms = self.frames[0].zooms();
        let rough = self.rac.uniform(0, zooms as i32) as usize;
        let forest1 = vec![Tree::default(); nump];
        // The top-left pixel of every channel first.
        for p in 0..nump {
            if !self.trivial(p) {
                let (min, max) = (self.ranges.min(p), self.ranges.max(p));
                for f in self.frames.iter_mut() {
                    let v = self.rac.uniform(min, max - min);
                    f.planes[p].set(0, v);
                }
                self.pixels_done += 1;
            }
        }
        match self.pass(&forest1, zooms, rough + 1)? {
            PassEnd::Done => {}
            PassEnd::Eof => {
                let mut zl = vec![rough as i64; nump];
                self.interpolate(0, 0, None, &mut zl)?;
                return Ok(End::Truncated);
            }
            PassEnd::Scaled => return Ok(End::Scaled),
        }
        if self.pixels_done >= self.pixels_todo && self.pixels_todo > 1 {
            let mut zl = vec![rough as i64; nump];
            self.interpolate(0, 0, None, &mut zl)?;
            return Ok(End::Complete);
        }
        let forest = self.read_trees(true)?;
        match self.pass(&forest, rough, 0)? {
            PassEnd::Done => Ok(End::Complete),
            PassEnd::Eof => Ok(End::Truncated),
            PassEnd::Scaled => Ok(End::Scaled),
        }
    }

    fn default_order(&self, begin: usize, end: usize) -> Vec<(usize, i64)> {
        let np = self.nump();
        let n = np * (begin + 1 - end);
        let mut max_behind = [0i64, 2, 4, 0, 0];
        if self.trivial(0) {
            max_behind[1] = 0;
            max_behind[2] = 1;
        }
        let mut czl = vec![begin as i64 + 1; np];
        let hp = if np >= 5 {
            4
        } else if np >= 4 {
            3
        } else {
            0
        };
        let mut out = Vec::with_capacity(n);
        let mut nextp = hp;
        let end = end as i64;
        for i in 0..n {
            czl[nextp] -= 1;
            out.push((nextp, czl[nextp]));
            if i + 1 == n {
                break;
            }
            nextp = hp;
            for p in 0..np {
                if czl[p] > czl[hp] + max_behind[p] {
                    nextp = p;
                }
            }
            let mut guard = 0;
            while czl[nextp] <= end && guard < np {
                nextp = (nextp + 1) % np;
                guard += 1;
            }
        }
        out
    }

    fn pass(&mut self, forest: &[Tree], begin: usize, end: usize) -> Result<PassEnd, Error> {
        let nump = self.nump();
        let mut coders: Vec<PropertyCoder> = forest.iter().map(PropertyCoder::new).collect();
        let mut zl = vec![begin as i64; nump];
        let default_order = self.rac.uniform(0, 1) != 0;
        let mut pred = [0i32; 5];
        for pr in pred.iter_mut().take(nump) {
            *pr = self.rac.uniform(-1, 3);
        }
        let n = nump * (begin + 1).saturating_sub(end);
        let order = if default_order {
            self.default_order(begin, end)
        } else {
            Vec::new()
        };
        for i in 0..n {
            let p = if default_order {
                order.get(i).map_or(0, |o| o.0)
            } else {
                let p = self.rac.uniform(0, nump as i32 - 1) as usize;
                if nump > 3 && self.alpha_zero && p < 3 && zl[p] <= zl[3] {
                    return Err(Error::Malformed(
                        "FLIF channel order puts colour before alpha".into(),
                    ));
                }
                if nump > 4 && p < 4 && zl[p] <= zl[4] {
                    return Err(Error::Malformed(
                        "FLIF channel order puts pixels before lookback".into(),
                    ));
                }
                p
            };
            let z = zl[p];
            if z < 0 {
                return Err(Error::Malformed(
                    "FLIF channel zoomlevel out of range".into(),
                ));
            }
            if !self.trivial(p) {
                // A downscaled decode stops once every channel is complete down to the
                // zoomlevel asked for.
                if (z as usize) < self.stop_zl
                    && (0..nump).all(|q| self.trivial(q) || zl[q] < self.stop_zl as i64)
                {
                    return Ok(PassEnd::Scaled);
                }
                let predictor = if pred[p] < 0 {
                    self.rac.uniform(0, 2)
                } else {
                    pred[p]
                } as usize;
                let ok = if z % 2 == 0 {
                    self.horizontal(&mut coders[p], p, z as usize, predictor, end, &mut zl)?
                } else {
                    self.vertical(&mut coders[p], p, z as usize, predictor, end, &mut zl)?
                };
                if !ok {
                    return Ok(PassEnd::Eof);
                }
            }
            zl[p] -= 1;
        }
        if self.stop_zl > 0
            && (0..nump).all(|q| self.trivial(q) || zl[q] < self.stop_zl as i64)
            && end == 0
        {
            return Ok(PassEnd::Scaled);
        }
        Ok(PassEnd::Done)
    }

    fn horizontal(
        &mut self,
        coder: &mut PropertyCoder,
        p: usize,
        z: usize,
        predictor: usize,
        end_zl: usize,
        zl: &mut [i64],
    ) -> Result<bool, Error> {
        let nump = self.nump();
        let fra = nump == 5;
        let nprops = if nump > 3 { NB_PROPS_A[p] } else { NB_PROPS[p] };
        let mut props = vec![0i32; nprops];
        let rows = self.frames[0].rows(z);
        let cols = self.frames[0].cols(z);
        let w = self.frames[0].w;
        let (rs, cs) = (rowps(z), colps(z));
        let mut r = 1;
        while r < rows {
            self.check_cancel()?;
            self.pixels_done += cols as u64;
            if self.rac.eof() {
                let from = if r > 1 { r - 2 } else { r };
                self.interpolate(p, end_zl, Some(from), zl)?;
                return Ok(false);
            }
            for fr in 0..self.frames.len() {
                let (before, rest) = self.frames.split_at_mut(fr);
                let img = &mut rest[0];
                if fr > 0 && img.seen_before >= 0 {
                    let sb = (img.seen_before as usize).min(fr - 1);
                    for c in 0..cols {
                        let i = img.idx(z, r, c);
                        let v = before[sb].at(p, i);
                        img.planes[p].set(i, v);
                    }
                    continue;
                }
                let mut plane = std::mem::take(&mut img.planes[p].data);
                let store = img.planes[p].store;
                let alpha_zero = self.alpha_zero && p < 3 && nump > 3;
                let (mut begin, mut end) = (0usize, cols);
                if fr > 0 {
                    let fr_row = r * rs;
                    begin = (img.col_begin[fr_row] as usize / cs).min(cols);
                    end = (1 + (img.col_end[fr_row].max(1) as usize - 1) / cs).clamp(begin, cols);
                    let prev = &before[fr - 1];
                    if alpha_zero {
                        for c in 0..begin {
                            let i = img.idx(z, r, c);
                            plane[i] = if img.at(3, i) == 0 {
                                store.wrap(pred_h(
                                    &plane,
                                    w,
                                    rs,
                                    cs,
                                    r,
                                    c,
                                    rows,
                                    predictor_for(p, self.inv_pred),
                                ))
                            } else {
                                store.wrap(prev.at(p, i))
                            };
                        }
                    } else if p != 4 {
                        for c in (0..begin).chain(end..cols) {
                            let i = img.idx(z, r, c);
                            plane[i] = store.wrap(prev.at(p, i));
                        }
                    }
                }
                for c in begin..end {
                    let i = img.idx(z, r, c);
                    if alpha_zero && img.at(3, i) == 0 {
                        plane[i] = store.wrap(pred_h(
                            &plane,
                            w,
                            rs,
                            cs,
                            r,
                            c,
                            rows,
                            predictor_for(p, self.inv_pred),
                        ));
                        continue;
                    }
                    if fra && p < 4 {
                        let k = img.at(4, i) as usize;
                        if k > 0 {
                            let v = if k <= fr { before[fr - k].at(p, i) } else { 0 };
                            plane[i] = store.wrap(v);
                            continue;
                        }
                    }
                    let (mut guess, min, mut max) = Self::props_h(
                        self.ranges,
                        &mut props,
                        img,
                        &plane,
                        p,
                        z,
                        r,
                        c,
                        rows,
                        cols,
                        predictor,
                    );
                    if fra && p == 4 && max > fr as i32 {
                        max = fr as i32;
                    }
                    if fra && (guess > max || guess < min) {
                        guess = min;
                    }
                    let v =
                        coder.read(self.rac, &self.table, &props, min - guess, max - guess) + guess;
                    plane[i] = store.wrap(v);
                }
                if fr > 0 && alpha_zero {
                    let prev = &before[fr - 1];
                    for c in end..cols {
                        let i = img.idx(z, r, c);
                        plane[i] = if img.at(3, i) == 0 {
                            store.wrap(pred_h(
                                &plane,
                                w,
                                rs,
                                cs,
                                r,
                                c,
                                rows,
                                predictor_for(p, self.inv_pred),
                            ))
                        } else {
                            store.wrap(prev.at(p, i))
                        };
                    }
                }
                img.planes[p].data = plane;
            }
            r += 2;
        }
        Ok(true)
    }

    fn vertical(
        &mut self,
        coder: &mut PropertyCoder,
        p: usize,
        z: usize,
        predictor: usize,
        end_zl: usize,
        zl: &mut [i64],
    ) -> Result<bool, Error> {
        let nump = self.nump();
        let fra = nump == 5;
        let nprops = if nump > 3 { NB_PROPS_A[p] } else { NB_PROPS[p] };
        let mut props = vec![0i32; nprops];
        let rows = self.frames[0].rows(z);
        let cols = self.frames[0].cols(z);
        let w = self.frames[0].w;
        let (rs, cs) = (rowps(z), colps(z));
        for r in 0..rows {
            self.check_cancel()?;
            self.pixels_done += (cols / 2) as u64;
            if self.rac.eof() {
                let from = r.saturating_sub(1);
                self.interpolate(p, end_zl, Some(from), zl)?;
                return Ok(false);
            }
            for fr in 0..self.frames.len() {
                let (before, rest) = self.frames.split_at_mut(fr);
                let img = &mut rest[0];
                if fr > 0 && img.seen_before >= 0 {
                    let sb = (img.seen_before as usize).min(fr - 1);
                    for c in (1..cols).step_by(2) {
                        let i = img.idx(z, r, c);
                        let v = before[sb].at(p, i);
                        img.planes[p].set(i, v);
                    }
                    continue;
                }
                let mut plane = std::mem::take(&mut img.planes[p].data);
                let store = img.planes[p].store;
                let alpha_zero = self.alpha_zero && p < 3 && nump > 3;
                let (mut begin, mut end) = (1usize, cols);
                if fr > 0 {
                    let fr_row = r * rs;
                    begin = img.col_begin[fr_row] as usize / cs;
                    end = (1 + (img.col_end[fr_row].max(1) as usize - 1) / cs) | 1;
                    if begin > 1 && begin % 2 == 0 {
                        begin -= 1;
                    }
                    if begin == 0 {
                        begin = 1;
                    }
                    let prev = &before[fr - 1];
                    if alpha_zero {
                        for c in (1..begin.min(cols)).step_by(2) {
                            let i = img.idx(z, r, c);
                            plane[i] = if img.at(3, i) == 0 {
                                store.wrap(pred_v(
                                    &plane,
                                    w,
                                    rs,
                                    cs,
                                    r,
                                    c,
                                    cols,
                                    predictor_for(p, self.inv_pred),
                                ))
                            } else {
                                store.wrap(prev.at(p, i))
                            };
                        }
                    } else if p != 4 {
                        for c in (1..begin.min(cols))
                            .step_by(2)
                            .chain((end..cols).step_by(2))
                        {
                            let i = img.idx(z, r, c);
                            plane[i] = store.wrap(prev.at(p, i));
                        }
                    }
                }
                let mut c = begin;
                while c < end && c < cols {
                    let i = img.idx(z, r, c);
                    if alpha_zero && img.at(3, i) == 0 {
                        plane[i] = store.wrap(pred_v(
                            &plane,
                            w,
                            rs,
                            cs,
                            r,
                            c,
                            cols,
                            predictor_for(p, self.inv_pred),
                        ));
                        c += 2;
                        continue;
                    }
                    if fra && p < 4 {
                        let k = img.at(4, i) as usize;
                        if k > 0 {
                            let v = if k <= fr { before[fr - k].at(p, i) } else { 0 };
                            plane[i] = store.wrap(v);
                            c += 2;
                            continue;
                        }
                    }
                    let (mut guess, min, mut max) = Self::props_v(
                        self.ranges,
                        &mut props,
                        img,
                        &plane,
                        p,
                        z,
                        r,
                        c,
                        rows,
                        cols,
                        predictor,
                    );
                    if fra && p == 4 && max > fr as i32 {
                        max = fr as i32;
                    }
                    if fra && (guess > max || guess < min) {
                        guess = min;
                    }
                    let v =
                        coder.read(self.rac, &self.table, &props, min - guess, max - guess) + guess;
                    plane[i] = store.wrap(v);
                    c += 2;
                }
                if fr > 0 && alpha_zero {
                    let prev = &before[fr - 1];
                    let mut c = end;
                    while c < cols {
                        let i = img.idx(z, r, c);
                        plane[i] = if img.at(3, i) == 0 {
                            store.wrap(pred_v(
                                &plane,
                                w,
                                rs,
                                cs,
                                r,
                                c,
                                cols,
                                predictor_for(p, self.inv_pred),
                            ))
                        } else {
                            store.wrap(prev.at(p, i))
                        };
                        c += 2;
                    }
                }
                img.planes[p].data = plane;
            }
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn props_h(
        ranges: &Ranges,
        props: &mut [i32],
        img: &Img,
        plane: &[i32],
        p: usize,
        z: usize,
        r: usize,
        c: usize,
        rows: usize,
        cols: usize,
        predictor: usize,
    ) -> (i32, i32, i32) {
        let w = img.w;
        let (rs, cs) = (rowps(z) * w, colps(z));
        let at = |rr: usize, cc: usize| plane[rr * rs + cc * cs];
        let y = |rr: usize, cc: usize| img.at(0, rr * rs + cc * cs);
        let mut k = 0;
        if p < 3 {
            if p > 0 {
                props[k] = y(r, c);
                k += 1;
            }
            if p > 1 {
                props[k] = img.at(1, r * rs + c * cs);
                k += 1;
            }
            if img.num() > 3 {
                props[k] = img.at(3, r * rs + c * cs);
                k += 1;
            }
        }
        let nprev = k;
        let bottom_present = r + 1 < rows;
        let right_present = c + 1 < cols;
        let top = at(r - 1, c);
        let left = if c > 0 { at(r, c - 1) } else { top };
        let topleft = if c > 0 { at(r - 1, c - 1) } else { top };
        let topright = if right_present { at(r - 1, c + 1) } else { top };
        let bottomleft = if bottom_present && c > 0 {
            at(r + 1, c - 1)
        } else {
            left
        };
        let bottom = if bottom_present { at(r + 1, c) } else { left };
        let avg = (top + bottom) >> 1;
        let tlg = left + top - topleft;
        let median = median3(avg, tlg, left + bottom - bottomleft);
        props[k] = if median == avg {
            0
        } else if median == tlg {
            1
        } else {
            2
        };
        k += 1;
        if p == 1 || p == 2 {
            let below = if bottom_present { r + 1 } else { r - 1 };
            props[k] = y(r, c) - ((y(r - 1, c) + y(below, c)) >> 1);
            k += 1;
        }
        let g = match predictor {
            0 => avg,
            1 => median,
            _ => median3(top, bottom, left),
        };
        let (min, max, guess) =
            ranges.snap_at(ranges.top(), p, &props[..nprev.max(2).min(props.len())], g);
        props[k] = top - bottom;
        props[k + 1] = top - ((topleft + topright) >> 1);
        props[k + 2] = left - ((bottomleft + topleft) >> 1);
        let bottomright = if right_present && bottom_present {
            at(r + 1, c + 1)
        } else {
            bottom
        };
        props[k + 3] = bottom - ((bottomleft + bottomright) >> 1);
        props[k + 4] = guess;
        k += 5;
        if p != 2 {
            props[k] = if r > 1 { at(r - 2, c) - top } else { 0 };
            props[k + 1] = if c > 1 { at(r, c - 2) - left } else { 0 };
        }
        (guess, min, max)
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn props_v(
        ranges: &Ranges,
        props: &mut [i32],
        img: &Img,
        plane: &[i32],
        p: usize,
        z: usize,
        r: usize,
        c: usize,
        rows: usize,
        cols: usize,
        predictor: usize,
    ) -> (i32, i32, i32) {
        let w = img.w;
        let (rs, cs) = (rowps(z) * w, colps(z));
        let at = |rr: usize, cc: usize| plane[rr * rs + cc * cs];
        let y = |rr: usize, cc: usize| img.at(0, rr * rs + cc * cs);
        let mut k = 0;
        if p < 3 {
            if p > 0 {
                props[k] = y(r, c);
                k += 1;
            }
            if p > 1 {
                props[k] = img.at(1, r * rs + c * cs);
                k += 1;
            }
            if img.num() > 3 {
                props[k] = img.at(3, r * rs + c * cs);
                k += 1;
            }
        }
        let nprev = k;
        let bottom_present = r + 1 < rows;
        let right_present = c + 1 < cols;
        let left = at(r, c - 1);
        let top = if r > 0 { at(r - 1, c) } else { left };
        let topleft = if r > 0 { at(r - 1, c - 1) } else { left };
        let topright = if r > 0 && right_present {
            at(r - 1, c + 1)
        } else {
            top
        };
        let bottomleft = if bottom_present {
            at(r + 1, c - 1)
        } else {
            left
        };
        let right = if right_present { at(r, c + 1) } else { top };
        let avg = (left + right) >> 1;
        let tlg = left + top - topleft;
        let median = median3(avg, tlg, right + top - topright);
        props[k] = if median == avg {
            0
        } else if median == tlg {
            1
        } else {
            2
        };
        k += 1;
        if p == 1 || p == 2 {
            let other = if right_present { c + 1 } else { c - 1 };
            props[k] = y(r, c) - ((y(r, c - 1) + y(r, other)) >> 1);
            k += 1;
        }
        let g = match predictor {
            0 => avg,
            1 => median,
            _ => median3(top, left, right),
        };
        let (min, max, guess) =
            ranges.snap_at(ranges.top(), p, &props[..nprev.max(2).min(props.len())], g);
        props[k] = left - right;
        props[k + 1] = left - ((bottomleft + topleft) >> 1);
        props[k + 2] = top - ((topleft + topright) >> 1);
        let bottomright = if right_present && bottom_present {
            at(r + 1, c + 1)
        } else {
            right
        };
        props[k + 3] = right - ((bottomright + topright) >> 1);
        props[k + 4] = guess;
        k += 5;
        if p != 2 {
            props[k] = if r > 1 { at(r - 2, c) - top } else { 0 };
            props[k + 1] = if c > 1 { at(r, c - 2) - left } else { 0 };
        }
        (guess, min, max)
    }

    // ---------------------------------------------------------------- interpolation

    /// Finishes the image after the data ran out (the reference's interpolation): the rest
    /// of zoomlevel `zl[p]` of channel `p` from row `from`, then every zoomlevel still
    /// missing down to `end_zl`, predicted from what was decoded.
    fn interpolate(
        &mut self,
        p: usize,
        end_zl: usize,
        from: Option<usize>,
        zl: &mut [i64],
    ) -> Result<(), Error> {
        let end = end_zl as i64;
        let w = self.frames[0].w;
        if let Some(from) = from {
            let z = zl[p];
            if z >= end {
                zl[p] -= 1;
            }
            if z >= 0 {
                let z = z as usize;
                let rows = self.frames[0].rows(z);
                let cols = self.frames[0].cols(z);
                let (rs, cs) = (rowps(z), colps(z));
                for img in self.frames.iter_mut() {
                    let pal = img.palette;
                    let mut plane = std::mem::take(&mut img.planes[p].data);
                    if plane.is_empty() {
                        img.planes[p].data = plane;
                        continue;
                    }
                    let store = img.planes[p].store;
                    if z.is_multiple_of(2) {
                        let mut r = from.max(1);
                        while r < rows {
                            for c in 0..cols {
                                let i = img.idx(z, r, c);
                                plane[i] = store.wrap(if pal {
                                    plane[img.idx(z, r - 1, c)]
                                } else {
                                    pred_h(
                                        &plane,
                                        w,
                                        rs,
                                        cs,
                                        r,
                                        c,
                                        rows,
                                        if p == 4 { 9 } else { 0 },
                                    )
                                });
                            }
                            r += 2;
                        }
                    } else {
                        for r in from..rows {
                            for c in (1..cols).step_by(2) {
                                let i = img.idx(z, r, c);
                                plane[i] = store.wrap(if pal {
                                    plane[img.idx(z, r, c - 1)]
                                } else {
                                    pred_v(
                                        &plane,
                                        w,
                                        rs,
                                        cs,
                                        r,
                                        c,
                                        cols,
                                        if p == 4 { 9 } else { 0 },
                                    )
                                });
                            }
                        }
                    }
                    img.planes[p].data = plane;
                }
            }
        }
        // Palette images are turned into colours before the remaining zoomlevels are
        // interpolated.
        let mut layer = self.ranges.top();
        if self.frames[0].palette {
            while self.frames[0].palette {
                let Some(u) = self.undos.pop() else { break };
                undo(&u, self.frames);
                match self.ranges.previous(layer) {
                    Some(l) => layer = l,
                    None => break,
                }
            }
            let z1 = zl.get(1).copied().unwrap_or(0);
            zl[0] = z1;
            if zl.len() > 2 {
                zl[2] = z1;
            }
            if zl.len() > 3 {
                zl[3] = z1;
            }
        }
        let np = self.ranges.num_planes_at(layer);
        let mut p = 0;
        while p < np && p < zl.len() {
            let z = zl[p];
            if z < end {
                p += 1;
                continue;
            }
            zl[p] -= 1;
            if p == 4 || self.ranges.min_at(layer, p) >= self.ranges.max_at(layer, p) || z < 0 {
                continue;
            }
            let z = z as usize;
            let rows = self.frames[0].rows(z);
            let cols = self.frames[0].cols(z);
            let (rs, cs) = (rowps(z), colps(z));
            for img in self.frames.iter_mut() {
                if p >= img.num() {
                    continue;
                }
                let mut plane = std::mem::take(&mut img.planes[p].data);
                if plane.is_empty() {
                    img.planes[p].data = plane;
                    continue;
                }
                let store = img.planes[p].store;
                if z.is_multiple_of(2) {
                    for r in (1..rows).step_by(2) {
                        for c in 0..cols {
                            let i = img.idx(z, r, c);
                            plane[i] = store.wrap(pred_h(&plane, w, rs, cs, r, c, rows, 0));
                        }
                    }
                } else {
                    for r in 0..rows {
                        for c in (1..cols).step_by(2) {
                            let i = img.idx(z, r, c);
                            plane[i] = store.wrap(pred_v(&plane, w, rs, cs, r, c, cols, 0));
                        }
                    }
                }
                img.planes[p].data = plane;
            }
        }
        Ok(())
    }
}

enum PassEnd {
    Done,
    Eof,
    Scaled,
}

/// The invisible-pixel predictor of channel `p` (the lookback channel predicts 0).
fn predictor_for(p: usize, inv: usize) -> usize {
    if p == 4 { 9 } else { inv }
}

/// The non-interlaced prediction (for invisible pixels).
#[inline]
fn predict_scan(plane: &[i32], w: usize, r: usize, c: usize, grey: i32) -> i32 {
    let i = r * w + c;
    let left = if c > 0 {
        plane[i - 1]
    } else if r > 0 {
        plane[i - w]
    } else {
        grey
    };
    let top = if r > 0 { plane[i - w] } else { left };
    let topleft = if r > 0 && c > 0 {
        plane[i - w - 1]
    } else {
        top
    };
    median3(left + top - topleft, left, top)
}

/// Prediction in a horizontal zoomlevel (`pred` 9: the lookback channel, always 0).
#[allow(clippy::too_many_arguments)]
#[inline]
fn pred_h(
    plane: &[i32],
    w: usize,
    rs: usize,
    cs: usize,
    r: usize,
    c: usize,
    rows: usize,
    pred: usize,
) -> i32 {
    if pred == 9 {
        return 0;
    }
    let at = |rr: usize, cc: usize| plane[rr * rs * w + cc * cs];
    let top = at(r - 1, c);
    let bottom = if r + 1 < rows { at(r + 1, c) } else { top };
    match pred {
        0 => (top + bottom) >> 1,
        1 => {
            let avg = (top + bottom) >> 1;
            let left = if c > 0 { at(r, c - 1) } else { top };
            let topleft = if c > 0 { at(r - 1, c - 1) } else { top };
            let bottomleft = if c > 0 && r + 1 < rows {
                at(r + 1, c - 1)
            } else {
                left
            };
            median3(avg, left + top - topleft, left + bottom - bottomleft)
        }
        _ => {
            let left = if c > 0 { at(r, c - 1) } else { top };
            median3(top, bottom, left)
        }
    }
}

/// Prediction in a vertical zoomlevel.
#[allow(clippy::too_many_arguments)]
#[inline]
fn pred_v(
    plane: &[i32],
    w: usize,
    rs: usize,
    cs: usize,
    r: usize,
    c: usize,
    cols: usize,
    pred: usize,
) -> i32 {
    if pred == 9 {
        return 0;
    }
    let at = |rr: usize, cc: usize| plane[rr * rs * w + cc * cs];
    let left = at(r, c - 1);
    let right = if c + 1 < cols { at(r, c + 1) } else { left };
    match pred {
        0 => (left + right) >> 1,
        1 => {
            let avg = (left + right) >> 1;
            let top = if r > 0 { at(r - 1, c) } else { left };
            let topleft = if r > 0 { at(r - 1, c - 1) } else { left };
            let topright = if r > 0 && c + 1 < cols {
                at(r - 1, c + 1)
            } else {
                top
            };
            median3(avg, left + top - topleft, right + top - topright)
        }
        _ => {
            let top = if r > 0 { at(r - 1, c) } else { left };
            median3(top, left, right)
        }
    }
}

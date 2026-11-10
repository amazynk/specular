//! The transforms of the second header: how each is read, the colour ranges it leaves, and
//! how it is undone after the pixels are decoded.

use crate::Error;
use crate::image::Img;
use crate::rac::{ChanceTable, Rac, SimpleCoder};
use crate::ranges::{ColorBucket, ColorBuckets, Layer, Ranges};

const MAX_PALETTE_SIZE: i32 = 30000;
const MAX_PER_BUCKET: [i32; 4] = [255, 510, 5, 255];

/// What is left to undo once the pixels are decoded.
pub(crate) enum Undo {
    /// ChannelCompact: the value of each index, per channel.
    Compact(Vec<Vec<i32>>),
    /// YCoCg: the RGB maxima to clip to.
    YCoCg([i32; 3]),
    Permute {
        perm: Vec<usize>,
        subtract: bool,
        min: Vec<i32>,
        max: Vec<i32>,
    },
    /// Palette: (Y, I, Q) per index.
    Palette(Vec<[i32; 3]>),
    /// PaletteAlpha: (A, Y, I, Q) per index.
    PaletteA(Vec<[i32; 4]>),
    /// Lookback: whether the source had no alpha / no colour planes.
    Lookback { was_flat: bool, was_grayscale: bool },
    /// Nothing to undo (Bounds, ColorBuckets, DuplicateFrame, FrameShape).
    None,
}

/// The decoder's view of the image while reading the transforms.
pub(crate) struct Ctx<'a> {
    pub(crate) frames: &'a mut [Img],
    pub(crate) alpha_zero: &'a mut bool,
}

pub(crate) fn name(id: i32) -> &'static str {
    match id {
        0 => "ChannelCompact",
        1 => "YCoCg",
        3 => "PermutePlanes",
        4 => "Bounds",
        5 => "PaletteAlpha",
        6 => "Palette",
        7 => "ColorBuckets",
        10 => "DuplicateFrame",
        11 => "FrameShape",
        12 => "FrameLookback",
        _ => "unknown",
    }
}

fn bad(id: i32) -> Error {
    Error::Malformed(format!(
        "FLIF {} transform does not fit the image",
        name(id)
    ))
}

/// Reads transform `id`, pushes its colour ranges onto `ranges` and returns what to undo.
pub(crate) fn load(
    id: i32,
    rac: &mut Rac,
    table: &ChanceTable,
    ranges: &mut Ranges,
    ctx: &mut Ctx,
) -> Result<Undo, Error> {
    let src = ranges.top();
    let np = ranges.num_planes();
    let (min, max) = (|p| ranges.min_at(src, p), |p| ranges.max_at(src, p));
    let nframes = ctx.frames.len();
    match id {
        0 => {
            if np > 4 {
                return Err(bad(id));
            }
            let mut c = SimpleCoder::new();
            let mut pal = vec![Vec::new(); np];
            let mut nb = [0i32; 4];
            for p in 0..np {
                let n = c.read(rac, table, 0, max(p) - min(p)) + 1;
                let mut lo = min(p);
                let mut remaining = n - 1;
                for _ in 0..n {
                    let v = lo + c.read(rac, table, 0, max(p) - lo - remaining);
                    pal[p].push(v);
                    lo = v + 1;
                    remaining -= 1;
                }
                nb[p] = n - 1;
            }
            ranges.layers.push(Layer::Compact(src, nb));
            Ok(Undo::Compact(pal))
        }
        1 => {
            if np < 3 || (0..3).any(|p| min(p) < 0) || (0..3).any(|p| min(p) == max(p)) {
                return Err(bad(id));
            }
            let m = max(0).max(max(1)).max(max(2));
            let par = m / 4 + 1;
            let maxs = [max(0), max(1), max(2)];
            ranges.layers.push(Layer::YCoCg(src, par));
            Ok(Undo::YCoCg(maxs))
        }
        3 => {
            if np < 3 || (0..3).any(|p| min(p) < 0) {
                return Err(bad(id));
            }
            let mut c = SimpleCoder::new();
            let subtract = c.read2(rac, table, 0, 1) != 0;
            let mut from = [false; 5];
            let mut to = [false; 5];
            let mut perm = vec![0usize; np];
            for (p, slot) in perm.iter_mut().enumerate() {
                let v = c.read2(rac, table, 0, np as i32 - 1);
                let v = v.clamp(0, np as i32 - 1) as usize;
                *slot = v;
                from[p] = true;
                to[v] = true;
            }
            if (0..np).any(|p| !from[p] || !to[p]) {
                return Err(Error::Malformed(
                    "FLIF PermutePlanes is not a permutation".into(),
                ));
            }
            let mins = (0..np).map(min).collect();
            let maxs = (0..np).map(max).collect();
            ranges.layers.push(if subtract {
                Layer::PermuteSub(src, perm.clone())
            } else {
                Layer::Permute(src, perm.clone())
            });
            Ok(Undo::Permute {
                perm,
                subtract,
                min: mins,
                max: maxs,
            })
        }
        4 => {
            if np > 4 {
                return Err(bad(id));
            }
            let mut c = SimpleCoder::new();
            let mut b = Vec::with_capacity(np);
            for p in 0..np {
                let lo = c.read2(rac, table, min(p), max(p));
                let hi = c.read2(rac, table, lo, max(p));
                if lo > hi || lo < min(p) || hi > max(p) {
                    return Err(Error::Malformed("FLIF Bounds out of range".into()));
                }
                b.push((lo, hi));
            }
            if ranges.is_static(src) {
                ranges.layers.push(Layer::Static(b));
            } else {
                ranges.layers.push(Layer::Bounds(src, b));
            }
            Ok(Undo::None)
        }
        5 => {
            if np < 4 || min(3) == max(3) {
                return Err(bad(id));
            }
            let az = *ctx.alpha_zero;
            let mut c = SimpleCoder::new();
            let (mut cy, mut ci, mut cq, mut ca) = (
                SimpleCoder::new(),
                SimpleCoder::new(),
                SimpleCoder::new(),
                SimpleCoder::new(),
            );
            let size = c.read2(rac, table, 1, MAX_PALETTE_SIZE);
            let sorted = c.read2(rac, table, 0, 1) != 0;
            let mut pal: Vec<[i32; 4]> = Vec::with_capacity(size.max(0) as usize);
            let mut pp = [0i32; 2];
            if sorted {
                let mut mn = [min(3), min(0), min(1), min(2)];
                let mut mx = [max(3), max(0), max(1), max(2)];
                let mut prev = [-1i32; 4];
                for _ in 0..size {
                    let a = ca.read2(rac, table, mn[0], mx[0]);
                    if az && a == 0 {
                        pal.push([0; 4]);
                        continue;
                    }
                    let y = cy.read2(
                        rac,
                        table,
                        if prev[0] == a { prev[1] } else { mn[1] },
                        mx[1],
                    );
                    pp[0] = y;
                    (mn[2], mx[2]) = ranges.minmax_at(src, 1, &pp);
                    let i = ci.read2(rac, table, mn[2], mx[2]);
                    pp[1] = i;
                    (mn[3], mx[3]) = ranges.minmax_at(src, 2, &pp);
                    let q = cq.read2(rac, table, mn[3], mx[3]);
                    let col = [a, y, i, q];
                    pal.push(col);
                    mn[0] = a;
                    prev = col;
                }
            } else {
                for _ in 0..size {
                    let a = ca.read2(rac, table, min(3), max(3));
                    if az && a == 0 {
                        pal.push([0; 4]);
                        continue;
                    }
                    let (lo, hi) = ranges.minmax_at(src, 0, &pp);
                    let y = cy.read2(rac, table, lo, hi);
                    pp[0] = y;
                    let (lo, hi) = ranges.minmax_at(src, 1, &pp);
                    let i = ci.read2(rac, table, lo, hi);
                    pp[1] = i;
                    let (lo, hi) = ranges.minmax_at(src, 2, &pp);
                    let q = cq.read2(rac, table, lo, hi);
                    pal.push([a, y, i, q]);
                }
            }
            for f in ctx.frames.iter_mut() {
                f.palette = true;
            }
            *ctx.alpha_zero = false;
            ranges.layers.push(Layer::PaletteA(src, pal.len() as i32));
            Ok(Undo::PaletteA(pal))
        }
        6 => {
            if np < 3
                || (max(0) == 0 && max(2) == 0 && np > 3 && min(3) == 1 && max(3) == 1)
                || (min(1) == max(1) && min(2) == max(2))
            {
                return Err(bad(id));
            }
            let mut c = SimpleCoder::new();
            let (mut cy, mut ci, mut cq) =
                (SimpleCoder::new(), SimpleCoder::new(), SimpleCoder::new());
            let size = c.read2(rac, table, 1, MAX_PALETTE_SIZE);
            let sorted = c.read2(rac, table, 0, 1) != 0;
            let mut pal: Vec<[i32; 3]> = Vec::with_capacity(size.max(0) as usize);
            let mut pp = [0i32; 2];
            if sorted {
                let mut mn = [min(0), min(1), min(2)];
                let mut mx = [max(0), max(1), max(2)];
                let mut prev = [-1i32; 3];
                for _ in 0..size {
                    let y = cy.read2(rac, table, mn[0], mx[0]);
                    pp[0] = y;
                    (mn[1], mx[1]) = ranges.minmax_at(src, 1, &pp);
                    let i = ci.read2(
                        rac,
                        table,
                        if prev[0] == y { prev[1] } else { mn[1] },
                        mx[1],
                    );
                    pp[1] = i;
                    (mn[2], mx[2]) = ranges.minmax_at(src, 2, &pp);
                    let q = cq.read2(rac, table, mn[2], mx[2]);
                    let col = [y, i, q];
                    pal.push(col);
                    mn[0] = y;
                    prev = col;
                }
            } else {
                for _ in 0..size {
                    let (lo, hi) = ranges.minmax_at(src, 0, &pp);
                    let y = cy.read2(rac, table, lo, hi);
                    pp[0] = y;
                    let (lo, hi) = ranges.minmax_at(src, 1, &pp);
                    let i = ci.read2(rac, table, lo, hi);
                    pp[1] = i;
                    let (lo, hi) = ranges.minmax_at(src, 2, &pp);
                    let q = cq.read2(rac, table, lo, hi);
                    pal.push([y, i, q]);
                }
            }
            for f in ctx.frames.iter_mut() {
                f.palette = true;
            }
            ranges.layers.push(Layer::Palette(src, pal.len() as i32));
            Ok(Undo::Palette(pal))
        }
        7 => {
            if np < 3
                || (min(1) == max(1) && min(2) == max(2))
                || (min(0) == 0 && max(0) == 0 && min(2) == 0 && max(2) == 0)
                || (min(0) == max(0) && min(1) == max(1) && min(2) == max(2))
                || (0..3).any(|p| max(p) - min(p) > 1023)
                || min(1) == max(1)
            {
                return Err(bad(id));
            }
            let cb = load_buckets(rac, table, ranges, src, np)?;
            ranges.layers.push(Layer::Buckets(src, Box::new(cb)));
            Ok(Undo::None)
        }
        10 => {
            if nframes < 2 {
                return Err(bad(id));
            }
            let mut c = SimpleCoder::new();
            ctx.frames[0].seen_before = -1;
            for i in 1..nframes {
                let v = c.read(rac, table, -1, i as i32 - 1);
                ctx.frames[i].seen_before = v.clamp(-1, i as i32 - 1);
            }
            ranges.layers.push(Layer::Dup(src));
            Ok(Undo::None)
        }
        11 => {
            if nframes < 2 {
                return Err(bad(id));
            }
            let unique = ctx.frames[1..].iter().filter(|f| f.seen_before < 0).count();
            if unique < 1 {
                return Err(bad(id));
            }
            let rows = ctx.frames[0].h;
            let cols = ctx.frames[0].w as i32;
            let nb = unique * rows;
            let mut c = SimpleCoder::new();
            let mut b = Vec::with_capacity(nb);
            for _ in 0..nb {
                b.push(c.read(rac, table, 0, cols));
                if rac.eof() && b.len() > 4096 {
                    return Err(Error::Truncated("FLIF frame shapes cut short".into()));
                }
            }
            let mut e = Vec::with_capacity(nb);
            for &bi in &b {
                let ei = cols - c.read(rac, table, 0, cols - bi);
                if ei > cols || ei < bi || ei <= 0 {
                    return Err(Error::Malformed(
                        "FLIF FrameShape has an invalid end column".into(),
                    ));
                }
                e.push(ei);
            }
            let mut pos = 0;
            for f in ctx.frames[1..].iter_mut() {
                if f.seen_before >= 0 {
                    continue;
                }
                for r in 0..rows {
                    f.col_begin[r] = b[pos] as u32;
                    f.col_end[r] = e[pos] as u32;
                    pos += 1;
                }
            }
            ranges.layers.push(Layer::Dup(src));
            Ok(Undo::None)
        }
        12 => {
            if nframes < 2 || np > 4 {
                return Err(bad(id));
            }
            let mut c = SimpleCoder::new();
            let max_lookback = c.read2(rac, table, 1, nframes as i32 - 1);
            let was_grayscale = np < 2;
            let was_flat = np < 4;
            let small = ctx.frames[0].planes[0].store == crate::image::Store::U8;
            for f in ctx.frames.iter_mut() {
                f.ensure_frame_lookbacks(small);
            }
            let lookback = (nframes as i32 - 1).min(max_lookback);
            let (amin, amax) = if np == 4 { (min(3), max(3)) } else { (1, 1) };
            ranges
                .layers
                .push(Layer::Lookback(src, amin, amax, lookback));
            Ok(Undo::Lookback {
                was_flat,
                was_grayscale,
            })
        }
        _ => Err(Error::Unsupported(format!(
            "FLIF transform {id} is not part of FLIF16"
        ))),
    }
}

/// The range of channel `p` over the earlier-channel boxes `lower..=upper`.
fn box_minmax(ranges: &Ranges, src: usize, p: usize, lower: &[i32], upper: &[i32]) -> (i32, i32) {
    let mut smin = 10000;
    let mut smax = -10000;
    let mut px = lower.to_vec();
    match p {
        1 => {
            for v0 in lower[0]..=upper[0] {
                px[0] = v0;
                let (a, b) = ranges.minmax_at(src, p, &px);
                smin = smin.min(a);
                smax = smax.max(b);
            }
        }
        2 => {
            for v0 in lower[0]..=upper[0] {
                for v1 in lower[1]..=upper[1] {
                    px[0] = v0;
                    px[1] = v1;
                    let (a, b) = ranges.minmax_at(src, p, &px);
                    smin = smin.min(a);
                    smax = smax.max(b);
                }
            }
        }
        _ => return ranges.minmax_at(src, p, &px),
    }
    (smin, smax)
}

/// Whether value `v[p]` can occur given the earlier channels `v` (the source ranges and the
/// buckets read so far).
fn exists(ranges: &Ranges, src: usize, cb: &ColorBuckets, p: usize, v: &[i32]) -> bool {
    if p > 0 && (v[0] < cb.min0 || v[0] > ranges.max_at(src, 0)) {
        return false;
    }
    if p > 1 && (v[1] < cb.min1 || v[1] > ranges.max_at(src, 1)) {
        return false;
    }
    let (_, _, s) = ranges.snap_at(src, p, v, v[p]);
    if s != v[p] {
        return false;
    }
    let b = cb.find(p, v);
    let slow = if v[p] <= b.min {
        b.min
    } else if v[p] >= b.max {
        b.max
    } else {
        b.snap(v[p])
    };
    slow == v[p]
}

fn exists_box(
    ranges: &Ranges,
    src: usize,
    cb: &ColorBuckets,
    p: usize,
    lower: &[i32],
    upper: &[i32],
) -> bool {
    let mut px = lower.to_vec();
    match p {
        0 => {
            for v0 in lower[0]..=upper[0] {
                px[0] = v0;
                if exists(ranges, src, cb, p, &px) {
                    return true;
                }
            }
        }
        1 => {
            for v0 in lower[0]..=upper[0] {
                for v1 in lower[1]..=upper[1] {
                    px[0] = v0;
                    px[1] = v1;
                    if exists(ranges, src, cb, p, &px) {
                        return true;
                    }
                }
            }
        }
        _ => {}
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn load_bucket(
    coders: &mut [SimpleCoder; 6],
    rac: &mut Rac,
    table: &ChanceTable,
    ranges: &Ranges,
    src: usize,
    cb: &ColorBuckets,
    plane: usize,
    lower: &[i32],
    upper: &[i32],
) -> ColorBucket {
    let mut b = ColorBucket::default();
    if plane < 3 {
        for p in 0..plane {
            if !exists_box(ranges, src, cb, p, lower, upper) {
                return b;
            }
        }
    }
    let (smin, smax) = box_minmax(ranges, src, plane, lower, upper);
    if coders[0].read2(rac, table, 0, 1) == 0 {
        return b;
    }
    if smin == smax {
        b.min = smin;
        b.max = smin;
        b.discrete = false;
        return b;
    }
    b.min = coders[1].read2(rac, table, smin, smax);
    b.max = coders[2].read2(rac, table, b.min, smax);
    if b.min == b.max || b.min + 1 == b.max {
        b.discrete = false;
        return b;
    }
    b.discrete = coders[3].read2(rac, table, 0, 1) != 0;
    if b.discrete {
        let nb = coders[4].read2(rac, table, 2, MAX_PER_BUCKET[plane].min(b.max - b.min));
        b.values.push(b.min);
        let mut v = b.min;
        for p in 1..(nb - 1).max(1) {
            let x = coders[5].read2(rac, table, v + 1, b.max + 1 - nb + p);
            b.values.push(x);
            v = x;
        }
        if b.min < b.max {
            b.values.push(b.max);
        }
        // Keep the values sorted and inside the bucket also on hostile input.
        b.values.retain(|&x| x >= b.min && x <= b.max);
        b.values.sort_unstable();
        b.values.dedup();
    }
    b
}

fn load_buckets(
    rac: &mut Rac,
    table: &ChanceTable,
    ranges: &Ranges,
    src: usize,
    np: usize,
) -> Result<ColorBuckets, Error> {
    let (min0, max0) = (ranges.min_at(src, 0), ranges.max_at(src, 0));
    let (min1, max1) = (ranges.min_at(src, 1), ranges.max_at(src, 1));
    let mut cb = ColorBuckets::new(min0, max0, min1, max1);
    let mut coders = std::array::from_fn(|_| SimpleCoder::new());
    let b0 = load_bucket(&mut coders, rac, table, ranges, src, &cb, 0, &[], &[]);
    cb.bucket0 = b0;
    let n0 = cb.bucket1.len();
    for i in 0..n0 {
        let v = min0 + i as i32;
        let b = load_bucket(&mut coders, rac, table, ranges, src, &cb, 1, &[v], &[v]);
        cb.bucket1[i] = b;
    }
    let last_lower;
    let last_upper;
    let q_used = ranges.min_at(src, 2) < ranges.max_at(src, 2);
    if q_used {
        for i in 0..n0 {
            let v0 = min0 + i as i32;
            for j in 0..cb.b2_cols {
                let lo1 = min1 + 4 * j as i32;
                let lower = [v0, lo1];
                let upper = [v0, lo1 + 3];
                let b = load_bucket(&mut coders, rac, table, ranges, src, &cb, 2, &lower, &upper);
                cb.bucket2[i * cb.b2_cols + j] = b;
            }
            if rac.eof() && i > 64 {
                return Err(Error::Truncated("FLIF colour buckets cut short".into()));
            }
        }
        // The reference leaves the box past the last bucket: one step on in both axes.
        last_lower = vec![min0 + n0 as i32, min1 + 4 * cb.b2_cols as i32];
        last_upper = vec![last_lower[0], last_lower[1] + 3];
    } else {
        last_lower = vec![min0 + n0 as i32];
        last_upper = last_lower.clone();
    }
    if np > 3 {
        cb.bucket3 = load_bucket(
            &mut coders,
            rac,
            table,
            ranges,
            src,
            &cb,
            3,
            &last_lower,
            &last_upper,
        );
    }
    // Colours of channel 1 without any channel-2 bucket cannot occur.
    if q_used {
        for i in 0..n0 {
            for j in 0..cb.b2_cols {
                if cb.bucket2[i * cb.b2_cols + j].empty() {
                    let lo1 = min1 + 4 * j as i32;
                    for c in lo1..=lo1 + 3 {
                        cb.bucket1[i].remove(c);
                        cb.bucket1[i].remove(c);
                    }
                }
            }
        }
    }
    Ok(cb)
}

fn require_real(f: &mut Img, p: usize) {
    let n = f.len();
    if let Some(pl) = f.planes.get_mut(p) {
        pl.make_real(n);
    }
}

/// Undoes one transform on every frame.
pub(crate) fn undo(u: &Undo, frames: &mut [Img]) {
    for f in frames.iter_mut() {
        let n = f.len();
        match u {
            Undo::Compact(pal) => {
                for (p, pal) in pal.iter().enumerate().take(f.num()) {
                    require_real(f, p);
                    if pal.is_empty() {
                        continue;
                    }
                    for v in f.planes[p].data.iter_mut() {
                        let i = if *v < 0 || *v as usize >= pal.len() {
                            0
                        } else {
                            *v as usize
                        };
                        *v = pal[i];
                    }
                }
            }
            Undo::YCoCg(max) => {
                if f.num() < 3 {
                    continue;
                }
                for p in 0..3 {
                    require_real(f, p);
                }
                let (a, rest) = f.planes.split_at_mut(1);
                let (b, c) = rest.split_at_mut(1);
                let (p0, p1, p2) = (&mut a[0], &mut b[0], &mut c[0]);
                for i in 0..n {
                    let y = p0.data[i];
                    let co = p1.data[i];
                    let cg = p2.data[i];
                    let g = y - ((-cg) >> 1);
                    let bl = y + ((1 - cg) >> 1) - (co >> 1);
                    let r = co + bl;
                    p0.set(i, r.clamp(0, max[0].max(0)));
                    p1.set(i, g.clamp(0, max[1].max(0)));
                    p2.set(i, bl.clamp(0, max[2].max(0)));
                }
            }
            Undo::Permute {
                perm,
                subtract,
                min,
                max,
            } => {
                let np = perm.len().min(f.num());
                for p in 0..np {
                    require_real(f, p);
                }
                let mut px = [0i32; 5];
                for i in 0..n {
                    for (p, slot) in px.iter_mut().enumerate().take(np) {
                        *slot = f.planes[p].data[i];
                    }
                    for p in 0..np {
                        let q = perm[p];
                        let v = if *subtract && (p == 1 || p == 2) {
                            (px[p] + px[0]).clamp(min[q], max[q].max(min[q]))
                        } else {
                            px[p]
                        };
                        f.planes[q].set(i, v);
                    }
                }
            }
            Undo::Palette(pal) => {
                if f.num() < 3 {
                    continue;
                }
                for p in 0..3 {
                    require_real(f, p);
                }
                for i in 0..n {
                    let idx = f.planes[1].data[i];
                    let idx = if idx < 0 || idx as usize >= pal.len() {
                        0
                    } else {
                        idx as usize
                    };
                    let col = pal.get(idx).copied().unwrap_or([0; 3]);
                    for (p, v) in col.iter().enumerate() {
                        f.planes[p].set(i, *v);
                    }
                }
                f.palette = false;
            }
            Undo::PaletteA(pal) => {
                if f.num() < 4 {
                    continue;
                }
                for p in 0..4 {
                    require_real(f, p);
                }
                for i in 0..n {
                    let idx = f.planes[1].data[i];
                    let idx = if idx < 0 || idx as usize >= pal.len() {
                        0
                    } else {
                        idx as usize
                    };
                    let [a, y, ii, q] = pal.get(idx).copied().unwrap_or([0; 4]);
                    f.planes[0].set(i, y);
                    f.planes[1].set(i, ii);
                    f.planes[2].set(i, q);
                    f.planes[3].set(i, a);
                }
                f.palette = false;
            }
            Undo::Lookback {
                was_flat,
                was_grayscale,
            } => {
                f.planes.truncate(4);
                if *was_flat {
                    f.planes.truncate(3);
                }
                if *was_grayscale {
                    f.planes.truncate(1);
                }
            }
            Undo::None => {}
        }
    }
}

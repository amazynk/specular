//! Reading boxes of a dataset: only the chunks that intersect the box are decoded, in
//! parallel, through a byte-capped cache of decoded chunks.

use std::sync::Arc;

use rayon::prelude::*;

use crate::chunks::{self, Index};
use crate::layout::Layout;
use crate::types::{Sample, convert};
use crate::vds::Resolve;
use crate::{Dataset, Error, File, Result, filters, limits};

/// No other files: virtual mappings from other files read as the fill value.
pub(crate) fn none<'a>(_: &str) -> Option<(&'a File, &'a [u8])> {
    None
}

/// Which plane of a dataset to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaneSpec {
    /// The dimension that runs along x (columns).
    pub x_dim: usize,
    /// The dimension that runs down y (rows).
    pub y_dim: usize,
    /// The position on every other dimension (entries for x and y are ignored).
    pub index: Vec<u64>,
}

/// A plane read as real values, row-major.
#[derive(Clone, Debug)]
pub struct Plane<T> {
    pub width: usize,
    pub height: usize,
    pub values: Vec<T>,
    /// Some chunks were missing from the file or failed to decode; their samples are NaN.
    pub truncated: bool,
}

/// A cached chunk: its (dataset, chunk) key, bytes, last-use tick and reservation from the
/// memory budget (held while it is cached).
type CacheEntry = ((usize, u64), Arc<Vec<u8>>, u64, specular_core::Reservation);

/// Decoded chunks, least recently used dropped first; a chunk the memory budget cannot hold
/// is not cached.
pub(crate) struct ChunkCache {
    cap: usize,
    used: usize,
    tick: u64,
    entries: Vec<CacheEntry>,
}

impl ChunkCache {
    pub fn new(cap: usize) -> ChunkCache {
        ChunkCache {
            cap,
            used: 0,
            tick: 0,
            entries: Vec::new(),
        }
    }

    fn get(&mut self, key: (usize, u64)) -> Option<Arc<Vec<u8>>> {
        self.tick += 1;
        let t = self.tick;
        self.entries.iter_mut().find(|e| e.0 == key).map(|e| {
            e.2 = t;
            e.1.clone()
        })
    }

    fn put(&mut self, key: (usize, u64), v: Arc<Vec<u8>>) {
        if v.len() > self.cap / 4 || self.entries.iter().any(|e| e.0 == key) {
            return;
        }
        while self.used + v.len() > self.cap && !self.entries.is_empty() {
            let (i, _) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.2)
                .unwrap_or((0, &self.entries[0]));
            let e = self.entries.swap_remove(i);
            self.used -= e.1.len();
        }
        let Ok(res) = specular_core::budget::reserve(v.len() as u64) else {
            return;
        };
        self.tick += 1;
        self.used += v.len();
        self.entries.push((key, v, self.tick, res));
    }
}

/// Where the bytes of one stored piece (a chunk, or the whole contiguous dataset) come from.
enum Src<'a> {
    Borrowed(&'a [u8]),
    Owned(Arc<Vec<u8>>),
    Fill,
    Bad,
}

struct Piece<'a> {
    origin: Vec<u64>,
    dims: Vec<u64>,
    src: Src<'a>,
}

fn index<'d>(file: &File, data: &[u8], ds: &'d Dataset) -> Result<&'d Index> {
    if let Some(r) = ds.index.get() {
        return r.as_ref().map_err(Clone::clone);
    }
    match chunks::build(data, &file.ctx, ds) {
        // A cancelled build is not remembered: the next read tries again.
        Err(Error::Cancelled) => Err(Error::Cancelled),
        r => ds.index.get_or_init(|| r).as_ref().map_err(Clone::clone),
    }
}

/// Feeds every piece of storage that intersects the box to `sink`, in batches whose decoded
/// size stays bounded.
#[allow(clippy::too_many_arguments)]
fn pieces<'a>(
    file: &File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    start: &[u64],
    count: &[u64],
    cancel: &(dyn Fn() -> bool + Sync),
    sink: &mut dyn FnMut(&Piece<'a>) -> Result<()>,
) -> Result<()> {
    let c = &file.ctx;
    let dims = &ds.space.dims;
    let elem = ds.elem_size(c) as u64;
    match &ds.layout {
        Layout::Compact { data: d } => {
            // Compact data is copied into the metadata; it is small.
            let owned = Arc::new(d.clone());
            sink(&Piece {
                origin: vec![0; dims.len()],
                dims: dims.clone(),
                src: Src::Owned(owned),
            })
        }
        Layout::Contiguous { addr, size } => {
            let src = match addr {
                None => Src::Fill,
                Some(a) => match c.pos(data, *a) {
                    Ok(p) => {
                        let end = (p as u64).saturating_add(*size).min(data.len() as u64) as usize;
                        Src::Borrowed(&data[p..end])
                    }
                    Err(_) => Src::Bad,
                },
            };
            sink(&Piece {
                origin: vec![0; dims.len()],
                dims: dims.clone(),
                src,
            })
        }
        Layout::Virtual { .. } => Err(Error::Malformed("virtual dataset read as stored".into())),
        Layout::Chunked { dims: cdims, .. } => {
            filters::supported(&ds.filters).map_err(Error::Unsupported)?;
            let idx = index(file, data, ds)?;
            let grid = idx.grid();
            let chunk_bytes = cdims.iter().product::<u64>() * elem;
            // The grid range the box covers.
            let lo: Vec<u64> = start.iter().zip(cdims).map(|(s, c)| s / c).collect();
            let hi: Vec<u64> = start
                .iter()
                .zip(count)
                .zip(cdims)
                .zip(grid)
                .map(|(((s, n), c), g)| ((s + n).div_ceil(*c)).min(*g))
                .collect();
            let mut coords: Vec<Vec<u64>> = Vec::new();
            if lo.iter().zip(&hi).all(|(l, h)| l < h) {
                let mut cur = lo.clone();
                'grid: loop {
                    if coords.len() as u64 >= limits::MAX_CHUNKS {
                        return Err(Error::TooLarge);
                    }
                    coords.push(cur.clone());
                    let mut d = cur.len();
                    loop {
                        if d == 0 {
                            break 'grid;
                        }
                        d -= 1;
                        cur[d] += 1;
                        if cur[d] < hi[d] {
                            break;
                        }
                        cur[d] = lo[d];
                    }
                }
            }
            let batch = ((128u64 << 20) / chunk_bytes.max(1)).max(1) as usize;
            for group in coords.chunks(batch) {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                let decoded: Vec<Src<'a>> = group
                    .par_iter()
                    .map(|scaled| {
                        if cancel() {
                            return Src::Bad;
                        }
                        chunk_src(file, data, ds_index, ds, idx, scaled, chunk_bytes)
                    })
                    .collect();
                if cancel() {
                    return Err(Error::Cancelled);
                }
                for (scaled, src) in group.iter().zip(decoded) {
                    sink(&Piece {
                        origin: scaled.iter().zip(cdims).map(|(s, c)| s * c).collect(),
                        dims: cdims.clone(),
                        src,
                    })?;
                }
            }
            Ok(())
        }
    }
}

/// The bytes of one chunk (by grid position): borrowed when stored unfiltered, else decoded
/// through the file's chunk cache.
fn chunk_src<'a>(
    file: &File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    idx: &Index,
    scaled: &[u64],
    chunk_bytes: u64,
) -> Src<'a> {
    let c = &file.ctx;
    let Layout::Chunked {
        dims: cdims,
        unfiltered_edges,
        ..
    } = &ds.layout
    else {
        return Src::Bad;
    };
    let dims = &ds.space.dims;
    let elem = ds.elem_size(c) as u64;
    let Some(loc) = idx.get(scaled) else {
        return Src::Fill;
    };
    let edge = scaled
        .iter()
        .zip(cdims)
        .zip(dims)
        .any(|((s, c), d)| (s + 1) * c > *d);
    let raw = match c.slice(data, loc.addr, loc.size) {
        Ok(r) => r,
        Err(_) => return Src::Bad,
    };
    if ds.filters.is_empty() || (edge && *unfiltered_edges) {
        return if raw.len() as u64 >= chunk_bytes {
            Src::Borrowed(&raw[..chunk_bytes as usize])
        } else {
            Src::Bad
        };
    }
    let grid = idx.grid();
    let key = (
        ds_index,
        scaled.iter().zip(grid).fold(0u64, |a, (s, g)| a * g + s),
    );
    if let Some(v) = file.cache.lock().ok().and_then(|mut c| c.get(key)) {
        return Src::Owned(v);
    }
    match filters::decode(
        &ds.filters,
        loc.mask,
        raw,
        chunk_bytes as usize,
        elem as usize,
    ) {
        Ok(v) if v.len() as u64 == chunk_bytes => {
            let v = Arc::new(v);
            if let Ok(mut c) = file.cache.lock() {
                c.put(key, v.clone());
            }
            Src::Owned(v)
        }
        _ => Src::Bad,
    }
}

/// Calls `run(src_elem, dst_elem, len)` for every contiguous run of the intersection of a
/// piece with the box.
fn runs(
    origin: &[u64],
    pdims: &[u64],
    start: &[u64],
    count: &[u64],
    run: &mut dyn FnMut(usize, usize, usize),
) {
    let rank = origin.len();
    if rank == 0 {
        run(0, 0, 1);
        return;
    }
    let mut lo = vec![0u64; rank];
    let mut hi = vec![0u64; rank];
    for d in 0..rank {
        lo[d] = origin[d].max(start[d]);
        hi[d] = origin[d].saturating_add(pdims[d]).min(start[d] + count[d]);
        if lo[d] >= hi[d] {
            return;
        }
    }
    // A contiguous or compact piece spans the whole dataspace, whose element count may
    // overflow: positions saturate, and a saturated source position is past any data (the
    // box itself, `count`, is bounded by `check_box`).
    let mut ps = vec![1u64; rank];
    let mut bs = vec![1u64; rank];
    for d in (0..rank - 1).rev() {
        ps[d] = ps[d + 1].saturating_mul(pdims[d + 1]);
        bs[d] = bs[d + 1] * count[d + 1];
    }
    let last = rank - 1;
    let len = (hi[last] - lo[last]) as usize;
    let mut cur = lo.clone();
    loop {
        let s: u64 = (0..rank).fold(0u64, |a, d| {
            a.saturating_add((cur[d] - origin[d]).saturating_mul(ps[d]))
        });
        let t: u64 = (0..rank).map(|d| (cur[d] - start[d]) * bs[d]).sum();
        run(s as usize, t as usize, len);
        // Advance over every dimension but the last.
        let mut d = last;
        loop {
            if d == 0 {
                return;
            }
            d -= 1;
            cur[d] += 1;
            if cur[d] < hi[d] {
                break;
            }
            cur[d] = lo[d];
        }
    }
}

fn check_box(ds: &Dataset, start: &[u64], count: &[u64], elem: u64) -> Result<usize> {
    let dims = &ds.space.dims;
    if start.len() != dims.len() || count.len() != dims.len() {
        return Err(Error::Malformed("box rank".into()));
    }
    for d in 0..dims.len() {
        if start[d].checked_add(count[d]).is_none_or(|e| e > dims[d]) {
            return Err(Error::Malformed("box outside the dataset".into()));
        }
    }
    let n = count.iter().try_fold(1u64, |a, &c| a.checked_mul(c));
    match n {
        Some(n) if n.saturating_mul(elem) <= limits::MAX_READ_BYTES => Ok(n as usize),
        _ => Err(Error::TooLarge),
    }
}

/// Reads a box as real values; failed pieces become NaN.
#[allow(clippy::too_many_arguments)]
pub(crate) fn typed_box<'a, T: Sample>(
    file: &'a File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    start: &[u64],
    count: &[u64],
    cancel: &(dyn Fn() -> bool + Sync),
    resolve: &Resolve<'a>,
) -> Result<(Vec<T>, bool)> {
    let (num, be) = ds
        .dtype
        .num()
        .ok_or_else(|| Error::Unsupported(format!("{} data", ds.dtype.describe())))?;
    let esize = num.size();
    // A compound field: its offset in each stored element of `rec` bytes.
    let (rec, off) = ds.field.unwrap_or((esize, 0));
    let n = check_box(ds, start, count, esize as u64)?;
    let _res = specular_core::budget::reserve_n(n as u64, size_of::<T>() as u64)?;
    let fill = match &ds.fill.value {
        Some(f) if f.len() >= off + esize => {
            let mut v = [T::default()];
            convert(&f[off..], num, be, esize, &mut v);
            v[0]
        }
        _ => T::default(),
    };
    if let Layout::Virtual { .. } = ds.layout {
        let b = box_bytes_in(file, data, ds_index, ds, start, count, cancel, resolve, 0)?;
        let mut out = vec![T::default(); n];
        convert(&b, num, be, esize, &mut out);
        return Ok((out, false));
    }
    let mut out = vec![T::default(); n];
    let mut truncated = false;
    pieces(file, data, ds_index, ds, start, count, cancel, &mut |p| {
        let src: Option<&[u8]> = match &p.src {
            Src::Borrowed(b) => Some(b),
            Src::Owned(v) => Some(v.as_slice()),
            Src::Fill => None,
            Src::Bad => {
                truncated = true;
                runs(&p.origin, &p.dims, start, count, &mut |_, t, len| {
                    out[t..t + len].fill(T::NAN);
                });
                return Ok(());
            }
        };
        runs(
            &p.origin,
            &p.dims,
            start,
            count,
            &mut |s, t, len| match src {
                None => out[t..t + len].fill(fill),
                Some(b) => {
                    let avail = match b.len().checked_sub(off + esize) {
                        Some(r) => (r / rec + 1).saturating_sub(s).min(len),
                        None => 0,
                    };
                    if avail > 0 {
                        convert(&b[s * rec + off..], num, be, rec, &mut out[t..t + avail]);
                    }
                    if avail < len {
                        truncated = true;
                        out[t + avail..t + len].fill(T::NAN);
                    }
                }
            },
        );
        Ok(())
    })?;
    Ok((out, truncated))
}

pub(crate) fn plane<'a, T: Sample>(
    file: &'a File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    spec: &PlaneSpec,
    cancel: &(dyn Fn() -> bool + Sync),
    resolve: &Resolve<'a>,
) -> Result<Plane<T>> {
    let dims = &ds.space.dims;
    let rank = dims.len();
    if spec.x_dim >= rank
        || spec.y_dim >= rank
        || spec.x_dim == spec.y_dim
        || spec.index.len() != rank
    {
        return Err(Error::Malformed("plane outside the dataset".into()));
    }
    let mut start = spec.index.clone();
    let mut count = vec![1u64; rank];
    for d in [spec.x_dim, spec.y_dim] {
        start[d] = 0;
        count[d] = dims[d];
    }
    let (values, truncated) =
        typed_box::<T>(file, data, ds_index, ds, &start, &count, cancel, resolve)?;
    let width = dims[spec.x_dim] as usize;
    let height = dims[spec.y_dim] as usize;
    let values = if spec.y_dim < spec.x_dim {
        values
    } else {
        // Stored with x slower than y: transpose.
        let mut t = vec![T::default(); values.len()];
        for x in 0..width {
            let col = &values[x * height..(x + 1) * height];
            for (y, &v) in col.iter().enumerate() {
                t[y * width + x] = v;
            }
        }
        t
    };
    Ok(Plane {
        width,
        height,
        values,
        truncated,
    })
}

/// Where one sampled column (or row) reads from: a slot in the table of pieces and the
/// position inside that piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tap {
    slot: usize,
    local: u64,
}

/// Picks the pieces one axis samples from. `n` is the axis length, `p` the piece length
/// along it, `step` the sampling step and `keep` the most distinct pieces allowed. Returns
/// the grid positions of the pieces used and a tap per sample.
fn taps(n: u64, p: u64, step: u64, keep: usize) -> (Vec<u64>, Vec<Tap>) {
    let p = p.max(1);
    let samples = n.div_ceil(step);
    let mut used: Vec<u64> = Vec::new();
    let mut out = Vec::with_capacity(samples as usize);
    for i in 0..samples {
        let x = i * step;
        let g = x / p;
        if used.last() != Some(&g) {
            used.push(g);
        }
        out.push(Tap {
            slot: used.len() - 1,
            local: x % p,
        });
    }
    if used.len() <= keep {
        return (used, out);
    }
    // Too many pieces: split the axis into `keep` blocks, read the middle piece of each and
    // squeeze it across its block.
    let grid = n.div_ceil(p);
    let keep = keep.max(1) as u64;
    let mut used = Vec::with_capacity(keep as usize);
    let mut spans = Vec::with_capacity(keep as usize);
    for b in 0..keep {
        let lo = b * n / keep;
        let hi = ((b + 1) * n / keep).max(lo + 1);
        let g = ((lo + hi) / 2 / p).min(grid - 1);
        used.push(g);
        spans.push((lo, hi, p.min(n - g * p)));
    }
    let out = (0..samples)
        .map(|i| {
            let x = i * step;
            let b = ((x * keep) / n).min(keep - 1);
            let (lo, hi, width) = spans[b as usize];
            Tap {
                slot: b as usize,
                local: ((x - lo) * width / (hi - lo)).min(width - 1),
            }
        })
        .collect();
    (used, out)
}

/// Reads every `step`-th sample of a plane along x and y: `ceil(w / step)` by
/// `ceil(h / step)` values, row-major. Only the pieces of storage holding samples are
/// touched. When the compressed chunks holding samples would decode to more than `budget`
/// bytes, a spread of fewer chunks stands in for their neighbours (a thumbnail's worth of
/// accuracy); a single chunk of four budgets or more is refused.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plane_strided<'a, T: Sample>(
    file: &'a File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    spec: &PlaneSpec,
    step: u64,
    budget: u64,
    cancel: &(dyn Fn() -> bool + Sync),
    resolve: &Resolve<'a>,
) -> Result<Plane<T>> {
    let c = &file.ctx;
    let dims = &ds.space.dims;
    let rank = dims.len();
    if spec.x_dim >= rank
        || spec.y_dim >= rank
        || spec.x_dim == spec.y_dim
        || spec.index.len() != rank
        || (0..rank).any(|d| d != spec.x_dim && d != spec.y_dim && spec.index[d] >= dims[d])
    {
        return Err(Error::Malformed("plane outside the dataset".into()));
    }
    let step = step.max(1);
    let (num, be) = ds
        .dtype
        .num()
        .ok_or_else(|| Error::Unsupported(format!("{} data", ds.dtype.describe())))?;
    let esize = num.size();
    let (xd, yd) = (spec.x_dim, spec.y_dim);
    let (w, h) = (dims[xd], dims[yd]);
    let (ow, oh) = (w.div_ceil(step), h.div_ceil(step));
    let n = ow.checked_mul(oh).ok_or(Error::TooLarge)?;
    if n.saturating_mul(esize as u64) > limits::MAX_READ_BYTES {
        return Err(Error::TooLarge);
    }
    if let Layout::Virtual { .. } = ds.layout {
        // Mapped from other datasets: the whole plane, then every step-th sample.
        let p = plane::<T>(file, data, ds_index, ds, spec, cancel, resolve)?;
        let step = step as usize;
        let mut values = Vec::with_capacity(n as usize);
        for y in (0..p.height).step_by(step) {
            for x in (0..p.width).step_by(step) {
                values.push(p.values[y * p.width + x]);
            }
        }
        return Ok(Plane {
            width: ow as usize,
            height: oh as usize,
            values,
            truncated: p.truncated,
        });
    }
    let (rec, off) = ds.field.unwrap_or((esize, 0));
    let fill = match &ds.fill.value {
        Some(f) if f.len() >= off + esize => {
            let mut v = [T::default()];
            convert(&f[off..], num, be, esize, &mut v);
            v[0]
        }
        _ => T::default(),
    };
    let mut out = vec![T::default(); n as usize];
    if n == 0 {
        return Ok(Plane {
            width: ow as usize,
            height: oh as usize,
            values: out,
            truncated: false,
        });
    }
    // The piece shape: a chunk, or the whole dataset when it is stored in one piece.
    let (pdims, chunked) = match &ds.layout {
        Layout::Chunked { dims: cdims, .. } if cdims.len() >= rank => {
            filters::supported(&ds.filters).map_err(Error::Unsupported)?;
            (cdims[..rank].to_vec(), true)
        }
        Layout::Chunked { .. } => return Err(Error::Malformed("chunk rank".into())),
        _ => (dims.clone(), false),
    };
    if pdims.contains(&0) {
        return Err(Error::Malformed("chunk size".into()));
    }
    let piece_bytes = pdims
        .iter()
        .try_fold(rec as u64, |a, &c| a.checked_mul(c))
        .ok_or(Error::TooLarge)?;
    let decoded = chunked && !ds.filters.is_empty();
    let (keep_x, keep_y) = if decoded {
        if piece_bytes >= budget.saturating_mul(4) {
            return Err(Error::TooLarge);
        }
        let allowed = (budget / piece_bytes).max(1);
        let gx = ow.min(w.div_ceil(pdims[xd]));
        let gy = oh.min(h.div_ceil(pdims[yd]));
        if gx.saturating_mul(gy) <= allowed {
            (usize::MAX, usize::MAX)
        } else {
            // Share the allowance between the axes in proportion to their chunk counts.
            let mx = ((allowed as f64 * gx as f64 / gy as f64).sqrt().round() as u64)
                .clamp(1, gx.min(allowed));
            let my = (allowed / mx).clamp(1, gy);
            (mx as usize, my as usize)
        }
    } else {
        (usize::MAX, usize::MAX)
    };
    let (used_x, taps_x) = taps(w, pdims[xd], step, keep_x);
    let (used_y, taps_y) = taps(h, pdims[yd], step, keep_y);
    // Element strides inside a piece.
    let mut ps = vec![1u64; rank];
    for d in (0..rank.saturating_sub(1)).rev() {
        ps[d] = ps[d + 1].saturating_mul(pdims[d + 1]);
    }
    let mut base = 0u64;
    let mut scaled = vec![0u64; rank];
    for d in 0..rank {
        if d != xd && d != yd {
            base += (spec.index[d] % pdims[d]) * ps[d];
            scaled[d] = spec.index[d] / pdims[d];
        }
    }
    // The pieces, a row of x slots per y slot.
    let mut srcs: Vec<Src> = Vec::with_capacity(used_x.len() * used_y.len());
    match &ds.layout {
        Layout::Compact { data: d } => srcs.push(Src::Owned(Arc::new(d.clone()))),
        Layout::Contiguous { addr, size } => srcs.push(match addr {
            None => Src::Fill,
            Some(a) => match c.pos(data, *a) {
                Ok(p) => {
                    let end = (p as u64).saturating_add(*size).min(data.len() as u64) as usize;
                    Src::Borrowed(&data[p..end])
                }
                Err(_) => Src::Bad,
            },
        }),
        _ => {
            let idx = index(file, data, ds)?;
            if idx.grid().len() < rank {
                return Err(Error::Malformed("chunk grid".into()));
            }
            let total = used_x.len() as u64 * used_y.len() as u64;
            if total > limits::MAX_CHUNKS {
                return Err(Error::TooLarge);
            }
            let mut coords = Vec::with_capacity(total as usize);
            for &gy in &used_y {
                for &gx in &used_x {
                    let mut s = scaled.clone();
                    s[xd] = gx;
                    s[yd] = gy;
                    coords.push(s);
                }
            }
            let batch = if decoded {
                ((128u64 << 20) / piece_bytes).max(1) as usize
            } else {
                coords.len().max(1)
            };
            for group in coords.chunks(batch) {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                let got: Vec<Src> = group
                    .par_iter()
                    .map(|s| {
                        if cancel() {
                            Src::Bad
                        } else {
                            chunk_src(file, data, ds_index, ds, idx, s, piece_bytes)
                        }
                    })
                    .collect();
                srcs.extend(got);
            }
            if cancel() {
                return Err(Error::Cancelled);
            }
        }
    }
    let mut truncated = false;
    let stride_x = used_x.len();
    for (j, ty) in taps_y.iter().enumerate() {
        if j % 64 == 0 && cancel() {
            return Err(Error::Cancelled);
        }
        let row = &mut out[j * ow as usize..(j + 1) * ow as usize];
        for (o, tx) in row.iter_mut().zip(&taps_x) {
            let src = &srcs[ty.slot * stride_x + tx.slot];
            let bytes: &[u8] = match src {
                Src::Borrowed(b) => b,
                Src::Owned(v) => v.as_slice(),
                Src::Fill => {
                    *o = fill;
                    continue;
                }
                Src::Bad => {
                    truncated = true;
                    *o = T::NAN;
                    continue;
                }
            };
            let at = base + ty.local * ps[yd] + tx.local * ps[xd];
            match usize::try_from(at.saturating_mul(rec as u64).saturating_add(off as u64)) {
                Ok(at) if at.saturating_add(esize) <= bytes.len() => {
                    convert(&bytes[at..], num, be, esize, std::slice::from_mut(o));
                }
                _ => {
                    truncated = true;
                    *o = T::NAN;
                }
            }
        }
    }
    Ok(Plane {
        width: ow as usize,
        height: oh as usize,
        values: out,
        truncated,
    })
}

pub(crate) fn all<'a, T: Sample>(
    file: &'a File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    resolve: &Resolve<'a>,
) -> Result<Vec<T>> {
    let dims = ds.space.dims.clone();
    let start = vec![0; dims.len()];
    let (v, truncated) =
        typed_box::<T>(file, data, ds_index, ds, &start, &dims, &|| false, resolve)?;
    if truncated {
        return Err(Error::Malformed("dataset data missing".into()));
    }
    Ok(v)
}

/// Reads a box as stored bytes (a compound field's own bytes for a field view); virtual
/// datasets read their mappings, `depth` deep in virtual-from-virtual chains.
#[allow(clippy::too_many_arguments)]
pub(crate) fn box_bytes_in<'a>(
    file: &'a File,
    data: &'a [u8],
    ds_index: usize,
    ds: &Dataset,
    start: &[u64],
    count: &[u64],
    cancel: &(dyn Fn() -> bool + Sync),
    resolve: &Resolve<'a>,
    depth: u32,
) -> Result<Vec<u8>> {
    let esize = ds.elem_size(&file.ctx);
    let n = check_box(ds, start, count, esize as u64)?;
    let _res = specular_core::budget::reserve_n(n as u64, esize as u64)?;
    let mut out = vec![0u8; n * esize];
    let fill = ds.fill.value.clone();
    if let Layout::Virtual { .. } = ds.layout {
        if let Some(f) = fill.as_ref().filter(|f| f.len() == esize) {
            for e in out.chunks_exact_mut(esize) {
                e.copy_from_slice(f);
            }
        }
        crate::vds::read_box(
            file, data, ds, start, count, esize, &mut out, cancel, resolve, depth,
        )?;
        return Ok(out);
    }
    pieces(file, data, ds_index, ds, start, count, cancel, &mut |p| {
        let src: Option<&[u8]> = match &p.src {
            Src::Borrowed(b) => Some(b),
            Src::Owned(v) => Some(v.as_slice()),
            Src::Fill => None,
            Src::Bad => return Err(Error::Malformed("chunk could not be read".into())),
        };
        let mut bad = false;
        runs(&p.origin, &p.dims, start, count, &mut |s, t, len| {
            let dst = &mut out[t * esize..(t + len) * esize];
            match src {
                Some(b) => match s
                    .checked_mul(esize)
                    .and_then(|a| Some(a..s.checked_add(len)?.checked_mul(esize)?))
                    .and_then(|r| b.get(r))
                {
                    Some(r) => dst.copy_from_slice(r),
                    None => bad = true,
                },
                None => {
                    if let Some(f) = &fill {
                        for e in dst.chunks_exact_mut(esize) {
                            e.copy_from_slice(f);
                        }
                    }
                }
            }
        });
        if bad {
            return Err(Error::Malformed("dataset data missing".into()));
        }
        Ok(())
    })?;
    if let Some((rec, off)) = ds.field {
        let size = ds.dtype.size(&file.ctx);
        out = out
            .chunks_exact(rec)
            .flat_map(|r| r[off..off + size].iter().copied())
            .collect();
    }
    Ok(out)
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn cached_chunks_hold_their_reservation() {
        use specular_core::budget::{MemoryBudget, with_budget};
        let b = Arc::new(MemoryBudget::fixed(1 << 20));
        let mut c = ChunkCache::new(1 << 30);
        with_budget(b.clone(), || {
            c.put((0, 0), Arc::new(vec![0; 1000]));
            // Over what the budget has free: not cached.
            c.put((0, 1), Arc::new(vec![0; 2 << 20]));
        });
        assert_eq!(b.reserved(), 1000);
        assert!(c.get((0, 1)).is_none());
        // Evicted to make room: its reservation goes with it.
        let mut small = ChunkCache::new(4000);
        with_budget(b.clone(), || {
            small.put((1, 0), Arc::new(vec![0; 1000]));
            small.put((1, 1), Arc::new(vec![0; 1000]));
            small.put((1, 2), Arc::new(vec![0; 1000]));
            small.put((1, 3), Arc::new(vec![0; 1000]));
            small.put((1, 4), Arc::new(vec![0; 1000]));
        });
        assert_eq!(b.reserved(), 1000 + 4000);
        drop((c, small));
        assert_eq!(b.reserved(), 0);
    }

    #[test]
    fn runs_over_an_overflowing_dataspace() {
        // A contiguous piece spanning 2 x 2^40 x 2^40 elements: its strides overflow.
        let mut seen = Vec::new();
        runs(
            &[0, 0, 0],
            &[2, 1 << 40, 1 << 40],
            &[1, 5, 7],
            &[1, 1, 2],
            &mut |s, t, len| seen.push((s, t, len)),
        );
        assert_eq!(seen, [(usize::MAX, 0, 2)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{ROOT, Storage, Ty, Writer};

    /// A zlib stream of stored (uncompressed) deflate blocks.
    pub(crate) fn zlib_stored(raw: &[u8]) -> Vec<u8> {
        let mut out = vec![0x78, 0x01];
        if raw.is_empty() {
            out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
        }
        let mut blocks = raw.chunks(65535).peekable();
        while let Some(b) = blocks.next() {
            out.push(u8::from(blocks.peek().is_none()));
            let n = b.len() as u16;
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(&(!n).to_le_bytes());
            out.extend_from_slice(b);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in raw {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        out.extend_from_slice(&((b << 16) | a).to_be_bytes());
        out
    }

    /// Splits a row-major array of f32 into chunks (row-major chunk order, edge chunks
    /// padded to full size).
    fn chunked(shape: &[u64], chunk: &[u64], v: &[f32]) -> Vec<Vec<u8>> {
        let rank = shape.len();
        let grid: Vec<u64> = shape
            .iter()
            .zip(chunk)
            .map(|(s, c)| s.div_ceil(*c))
            .collect();
        let n: u64 = grid.iter().product();
        let csize: u64 = chunk.iter().product();
        let mut out = Vec::new();
        for k in 0..n {
            let mut g = vec![0u64; rank];
            let mut r = k;
            for d in (0..rank).rev() {
                g[d] = r % grid[d];
                r /= grid[d];
            }
            let mut bytes = Vec::with_capacity(csize as usize * 4);
            for e in 0..csize {
                let mut r = e;
                let mut l = vec![0u64; rank];
                for d in (0..rank).rev() {
                    l[d] = r % chunk[d];
                    r /= chunk[d];
                }
                let mut at = 0u64;
                let mut inside = true;
                for d in 0..rank {
                    let x = g[d] * chunk[d] + l[d];
                    inside &= x < shape[d];
                    at = at * shape[d] + x.min(shape[d] - 1);
                }
                let x = if inside { v[at as usize] } else { 0.0 };
                bytes.extend_from_slice(&x.to_le_bytes());
            }
            out.push(bytes);
        }
        out
    }

    /// A 512 x 512 box read as f64 (2 MiB, under the read cap) is refused under a 1 MiB
    /// memory budget before it is allocated, and read under 64 MiB; so are its raw bytes
    /// (1 MiB) under 512 KiB.
    #[test]
    fn box_within_memory_budget() {
        use specular_core::budget::with_limit;
        let mut w = Writer::new();
        w.f32s(ROOT, "v", &[512, 512], &values(&[512, 512]));
        let data = w.finish();
        let f = File::open(&data).unwrap();
        let (i, _, _) = f.datasets().find(|(_, o, _)| o.path == "/v").unwrap();
        let read = || f.read_box::<f64>(&data, i, &[0, 0], &[512, 512], &|| false);
        let Err(Error::NeedsMemory(n)) = with_limit(1 << 20, read) else {
            panic!("read")
        };
        assert!(n.needed >= 2 << 20 && n.needed > n.free, "{n:?}");
        let (v, truncated) = with_limit(64 << 20, read).unwrap();
        assert!(!truncated && v.len() == 512 * 512);
        let bytes = || f.read_box_bytes(&data, i, &[0, 0], &[512, 512], &|| false);
        assert!(matches!(
            with_limit(512 << 10, bytes),
            Err(Error::NeedsMemory(_))
        ));
        assert_eq!(with_limit(64 << 20, bytes).unwrap().len(), 1 << 20);
    }

    fn values(shape: &[u64]) -> Vec<f32> {
        let n: u64 = shape.iter().product();
        (0..n).map(|i| ((i * 7919) % 10007) as f32).collect()
    }

    /// Counts the strided samples that differ from every `step`-th value of the full plane.
    fn mismatches(f: &File, data: &[u8], i: usize, spec: &PlaneSpec, step: u64) -> usize {
        let full = f.read_plane::<f32>(data, i, spec, &|| false).unwrap();
        let got = f
            .read_plane_strided::<f32>(data, i, spec, step, u64::MAX, &|| false)
            .unwrap();
        assert!(!got.truncated);
        let s = step as usize;
        assert_eq!(got.width, full.width.div_ceil(s));
        assert_eq!(got.height, full.height.div_ceil(s));
        let mut bad = 0;
        for y in 0..got.height {
            for x in 0..got.width {
                let want = full.values[y * s * full.width + x * s];
                if got.values[y * got.width + x].to_bits() != want.to_bits() {
                    bad += 1;
                }
            }
        }
        bad
    }

    #[test]
    fn strided_reads_match_the_full_plane() {
        let shape = [3u64, 37, 53];
        let v = values(&shape);
        let raw: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let chunk = [1u64, 8, 16];
        let mut w = Writer::new();
        w.f32s(ROOT, "contig", &shape, &v);
        w.dataset(
            ROOT,
            "plain",
            Ty::F32,
            &shape,
            raw.clone(),
            Storage::Chunks {
                chunk: chunk.to_vec(),
                filters: Vec::new(),
                stored: chunked(&shape, &chunk, &v),
            },
        );
        w.dataset(
            ROOT,
            "deflate",
            Ty::F32,
            &shape,
            raw.clone(),
            Storage::Chunks {
                chunk: chunk.to_vec(),
                filters: vec![(1, vec![6])],
                stored: chunked(&shape, &chunk, &v)
                    .iter()
                    .map(|c| zlib_stored(c))
                    .collect(),
            },
        );
        w.dataset(
            ROOT,
            "single",
            Ty::F32,
            &shape,
            raw.clone(),
            Storage::Single {
                filters: vec![(1, vec![6])],
                stored: zlib_stored(&raw),
            },
        );
        let data = w.finish();
        let f = File::open(&data).unwrap();
        let mut bad = 0;
        let mut checked = 0;
        for name in ["/contig", "/plain", "/deflate", "/single"] {
            let i = f.index_of(name).unwrap();
            for step in [1, 2, 3, 5, 16, 60] {
                let mut specs = Vec::new();
                for plane in 0..3 {
                    specs.push((2, 1, vec![plane, 0, 0]));
                    specs.push((1, 2, vec![plane, 0, 0]));
                }
                specs.push((2, 0, vec![0, 20, 0]));
                for (x_dim, y_dim, index) in specs {
                    let spec = PlaneSpec {
                        x_dim,
                        y_dim,
                        index,
                    };
                    bad += mismatches(&f, &data, i, &spec, step);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 4 * 6 * 7);
        assert_eq!(bad, 0, "{bad} mismatches");
    }

    #[test]
    fn strided_reads_keep_to_the_budget() {
        let shape = [64u64, 96];
        let v = values(&shape);
        let raw: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let chunk = [8u64, 8];
        let mut w = Writer::new();
        w.dataset(
            ROOT,
            "d",
            Ty::F32,
            &shape,
            raw,
            Storage::Chunks {
                chunk: chunk.to_vec(),
                filters: vec![(1, vec![6])],
                stored: chunked(&shape, &chunk, &v)
                    .iter()
                    .map(|c| zlib_stored(c))
                    .collect(),
            },
        );
        let data = w.finish();
        let f = File::open(&data).unwrap();
        let i = f.index_of("/d").unwrap();
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: 0,
            index: vec![0, 0],
        };
        // 96 chunks of 256 bytes against a budget of six.
        let p = f
            .read_plane_strided::<f32>(&data, i, &spec, 2, 6 * 256, &|| false)
            .unwrap();
        assert_eq!((p.width, p.height), (48, 32));
        assert!(!p.truncated);
        // Every sample is a value of the dataset, from no more than six chunks.
        let mut chunks = std::collections::HashSet::new();
        for &s in &p.values {
            let at = v.iter().position(|&x| x == s).unwrap();
            chunks.insert((at / 96 / 8, at % 96 / 8));
        }
        assert!(chunks.len() <= 6, "{} chunks", chunks.len());
        assert!(chunks.len() >= 4, "{} chunks", chunks.len());
        // One chunk of four budgets or more is refused.
        assert!(matches!(
            f.read_plane_strided::<f32>(&data, i, &spec, 2, 32, &|| false),
            Err(Error::TooLarge)
        ));
        assert!(matches!(
            f.read_plane_strided::<f32>(&data, i, &spec, 2, u64::MAX, &|| true),
            Err(Error::Cancelled)
        ));
    }

    #[test]
    fn taps_squeeze_blocks_onto_their_middle_piece() {
        let (used, t) = taps(100, 10, 5, usize::MAX);
        assert_eq!(used, (0..10).collect::<Vec<_>>());
        assert_eq!(t[3], Tap { slot: 1, local: 5 });
        let (used, t) = taps(100, 10, 5, 2);
        assert_eq!(used, vec![2, 7]);
        assert_eq!(t[0], Tap { slot: 0, local: 0 });
        assert_eq!(t[19], Tap { slot: 1, local: 9 });
        assert!(t.iter().all(|t| t.local < 10));
    }
}

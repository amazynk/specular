//! Virtual datasets: a dataset whose elements are mapped from selections of other datasets,
//! in this file or in others.
//!
//! The layout message names a global heap object holding the mappings: for each, the source
//! file ("." or a flag for this file) and dataset, the source selection and the selection of
//! the virtual dataset it fills. The k-th element of one selection (in row-major order) is the
//! k-th of the other. Elements no mapping covers hold the fill value.
//!
//! Selections read: all, points, regular hyperslabs and irregular ones whose blocks make up a
//! grid. Unlimited mappings (and the printf-style source names they come with) are refused.

use crate::io::{Ctx, Cur};
use crate::{Error, File, Result};

/// The most selected positions kept per selection (all dimensions together).
const MAX_POSITIONS: u64 = 1 << 24;
/// Mappings of one virtual dataset.
const MAX_MAPPINGS: u64 = 1 << 16;
/// Virtual datasets reading from virtual datasets.
pub(crate) const MAX_DEPTH: u32 = 4;

/// A selection of a dataspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Sel {
    /// Every element.
    All,
    /// No element.
    None,
    /// The Cartesian product of ascending coordinate lists, one per dimension.
    Grid(Vec<Vec<u64>>),
    /// Points, in selection order.
    Points(Vec<Vec<u64>>),
}

impl Sel {
    /// The selection as a grid or points over a dataspace of `dims`.
    fn resolve(&self, dims: &[u64]) -> Result<Sel> {
        Ok(match self {
            Sel::All => {
                let total: u64 = dims.iter().sum();
                if total > MAX_POSITIONS {
                    return Err(Error::TooLarge);
                }
                Sel::Grid(dims.iter().map(|&n| (0..n).collect()).collect())
            }
            s => {
                let rank = match s {
                    Sel::Grid(g) => g.len(),
                    Sel::Points(p) => p.first().map_or(dims.len(), Vec::len),
                    _ => dims.len(),
                };
                if rank != dims.len() {
                    return Err(Error::Malformed("virtual selection rank".into()));
                }
                s.clone()
            }
        })
    }

    fn count(&self) -> u64 {
        match self {
            Sel::None => 0,
            Sel::All => 0,
            Sel::Grid(g) => g
                .iter()
                .try_fold(1u64, |a, l| a.checked_mul(l.len() as u64))
                .unwrap_or(u64::MAX),
            Sel::Points(p) => p.len() as u64,
        }
    }
}

/// One mapping of a virtual dataset.
#[derive(Clone, Debug)]
pub(crate) struct Mapping {
    /// The source file as named, `None` for this file.
    pub file: Option<String>,
    /// The source dataset's path.
    pub dataset: String,
    pub src: Sel,
    pub dst: Sel,
}

fn cstr(r: &mut Cur) -> Result<String> {
    let mut s = Vec::new();
    loop {
        let b = r.u8()?;
        if b == 0 {
            break;
        }
        if s.len() < crate::limits::MAX_NAME_BYTES {
            s.push(b);
        }
    }
    Ok(String::from_utf8_lossy(&s).into_owned())
}

/// The mappings held in global heap object (`addr`, `index`).
pub(crate) fn parse(data: &[u8], c: &Ctx, addr: u64, index: u32) -> Result<Vec<Mapping>> {
    let b = crate::gheap::object(data, c, addr, index)?;
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    if version > 1 {
        return Err(Error::Unsupported(format!(
            "virtual dataset mapping version {version}"
        )));
    }
    let n = r.len(c)?;
    if n > MAX_MAPPINGS {
        return Err(Error::TooLarge);
    }
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let flags = if version >= 1 { r.u8()? } else { 0 };
        if flags & !0x04 != 0 {
            return Err(Error::Unsupported(format!(
                "virtual dataset mapping flags {flags:#x}"
            )));
        }
        let file = if flags & 0x04 != 0 {
            None
        } else {
            Some(cstr(&mut r)?)
        };
        // Paths are relative to the source file's root group.
        let dataset = cstr(&mut r)?;
        let dataset = if dataset.starts_with('/') {
            dataset
        } else {
            format!("/{dataset}")
        };
        let src = selection(&mut r)?;
        let dst = selection(&mut r)?;
        let file = file.filter(|f| f != ".");
        if dataset.contains('%') || file.as_deref().is_some_and(|f| f.contains('%')) {
            return Err(Error::Unsupported(
                "virtual dataset with printf-style source names".into(),
            ));
        }
        out.push(Mapping {
            file,
            dataset,
            src,
            dst,
        });
    }
    Ok(out)
}

/// One encoded selection.
fn selection(r: &mut Cur) -> Result<Sel> {
    let kind = r.u32()?;
    let version = r.u32()?;
    match kind {
        0 | 3 => {
            r.skip(8)?;
            Ok(if kind == 0 { Sel::None } else { Sel::All })
        }
        1 => {
            let (enc, rank, n) = match version {
                1 => {
                    r.skip(8)?;
                    let rank = r.u32()? as usize;
                    (4, rank, u64::from(r.u32()?))
                }
                2 => {
                    let enc = r.u8()? as usize;
                    let rank = r.u32()? as usize;
                    check_enc(enc)?;
                    (enc, rank, r.uint(enc)?)
                }
                v => return Err(Error::Unsupported(format!("point selection version {v}"))),
            };
            check_rank(rank)?;
            if n.saturating_mul(rank as u64) > MAX_POSITIONS {
                return Err(Error::TooLarge);
            }
            let mut pts = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let mut p = Vec::with_capacity(rank);
                for _ in 0..rank {
                    p.push(r.uint(enc)?);
                }
                pts.push(p);
            }
            Ok(Sel::Points(pts))
        }
        2 => hyperslab(r, version),
        k => Err(Error::Malformed(format!("selection type {k}"))),
    }
}

fn check_enc(enc: usize) -> Result<()> {
    if matches!(enc, 2 | 4 | 8) {
        Ok(())
    } else {
        Err(Error::Malformed("selection encoding size".into()))
    }
}

fn check_rank(rank: usize) -> Result<()> {
    if (1..=32).contains(&rank) {
        Ok(())
    } else {
        Err(Error::Malformed(format!("selection of rank {rank}")))
    }
}

fn unlimited(v: u64, enc: usize) -> bool {
    crate::io::is_undef(v, enc)
}

fn hyperslab(r: &mut Cur, version: u32) -> Result<Sel> {
    let (regular, enc, rank) = match version {
        1 => {
            r.skip(8)?;
            (false, 4, r.u32()? as usize)
        }
        2 => {
            let flags = r.u8()?;
            r.skip(4)?;
            (flags & 1 != 0, 8, r.u32()? as usize)
        }
        3 => {
            let flags = r.u8()?;
            let enc = r.u8()? as usize;
            check_enc(enc)?;
            (flags & 1 != 0, enc, r.u32()? as usize)
        }
        v => {
            return Err(Error::Unsupported(format!(
                "hyperslab selection version {v}"
            )));
        }
    };
    check_rank(rank)?;
    if version == 2 && !regular {
        return Err(Error::Malformed("hyperslab selection".into()));
    }
    let mut total = 0u64;
    if regular {
        let mut grid = Vec::with_capacity(rank);
        for _ in 0..rank {
            let (start, stride, count, block) =
                (r.uint(enc)?, r.uint(enc)?, r.uint(enc)?, r.uint(enc)?);
            if unlimited(count, enc) || unlimited(block, enc) {
                return Err(Error::Unsupported(
                    "virtual dataset with an unlimited mapping".into(),
                ));
            }
            if count > 1 && stride < block {
                return Err(Error::Malformed("hyperslab stride".into()));
            }
            let n = count.saturating_mul(block);
            total = total.saturating_add(n);
            if total > MAX_POSITIONS {
                return Err(Error::TooLarge);
            }
            let mut l = Vec::with_capacity(n as usize);
            for i in 0..count {
                let at = start
                    .checked_add(i.checked_mul(stride).ok_or(Error::TooLarge)?)
                    .ok_or(Error::TooLarge)?;
                for j in 0..block {
                    l.push(at.checked_add(j).ok_or(Error::TooLarge)?);
                }
            }
            grid.push(l);
        }
        return Ok(Sel::Grid(grid));
    }
    // Irregular: a list of blocks (start and end corners, inclusive).
    let n = if version == 1 {
        u64::from(r.u32()?)
    } else {
        r.uint(enc)?
    };
    if n.saturating_mul(rank as u64 * 2) > MAX_POSITIONS {
        return Err(Error::TooLarge);
    }
    let mut blocks = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let mut lo = Vec::with_capacity(rank);
        let mut hi = Vec::with_capacity(rank);
        for _ in 0..rank {
            lo.push(r.uint(enc)?);
        }
        for _ in 0..rank {
            hi.push(r.uint(enc)?);
        }
        if lo.iter().zip(&hi).any(|(a, b)| a > b) {
            return Err(Error::Malformed("hyperslab block".into()));
        }
        blocks.push((lo, hi));
    }
    // The blocks (disjoint, as the library stores them) make up a grid when their volumes
    // add up to the product of the coordinates each dimension covers.
    let mut grid: Vec<Vec<u64>> = vec![Vec::new(); rank];
    let mut volume = 0u64;
    for (lo, hi) in &blocks {
        let mut v = 1u64;
        for d in 0..rank {
            let len = hi[d] - lo[d] + 1;
            total = total.saturating_add(len);
            if total > MAX_POSITIONS {
                return Err(Error::TooLarge);
            }
            v = v.saturating_mul(len);
            grid[d].extend(lo[d]..=hi[d]);
        }
        volume = volume.saturating_add(v);
    }
    for l in grid.iter_mut() {
        l.sort_unstable();
        l.dedup();
    }
    let product = grid
        .iter()
        .try_fold(1u64, |a, l| a.checked_mul(l.len() as u64));
    if blocks.is_empty() {
        return Ok(Sel::None);
    }
    if product != Some(volume) {
        return Err(Error::Unsupported(
            "virtual dataset with an irregular selection".into(),
        ));
    }
    Ok(Sel::Grid(grid))
}

/// Finds the source datasets of the mappings: (file, its bytes) for a file name, `None` when
/// it cannot be reached.
pub type Resolve<'a> = dyn Fn(&str) -> Option<(&'a File, &'a [u8])> + Sync + 'a;

/// Calls `f(rank in the selection, coordinates)` for every element of a grid or point
/// selection that lies inside the box, in row-major order.
fn visit(sel: &Sel, start: &[u64], count: &[u64], f: &mut dyn FnMut(u64, &[u64])) {
    let rank = start.len();
    match sel {
        Sel::Grid(g) => {
            if g.len() != rank {
                return;
            }
            // Per dimension, the range of list positions inside the box.
            let mut lo = vec![0usize; rank];
            let mut hi = vec![0usize; rank];
            for d in 0..rank {
                let l = &g[d];
                lo[d] = l.partition_point(|&x| x < start[d]);
                hi[d] = l.partition_point(|&x| x < start[d] + count[d]);
                if lo[d] >= hi[d] {
                    return;
                }
            }
            let mut stride = vec![1u64; rank];
            for d in (0..rank.saturating_sub(1)).rev() {
                stride[d] = stride[d + 1] * g[d + 1].len() as u64;
            }
            let mut cur = lo.clone();
            let mut c: Vec<u64> = (0..rank).map(|d| g[d][cur[d]]).collect();
            loop {
                let k = (0..rank).map(|d| cur[d] as u64 * stride[d]).sum();
                f(k, &c);
                let mut d = rank;
                loop {
                    if d == 0 {
                        return;
                    }
                    d -= 1;
                    cur[d] += 1;
                    if cur[d] < hi[d] {
                        c[d] = g[d][cur[d]];
                        break;
                    }
                    cur[d] = lo[d];
                    c[d] = g[d][cur[d]];
                }
            }
        }
        Sel::Points(p) => {
            for (k, c) in p.iter().enumerate() {
                if c.len() == rank
                    && c.iter()
                        .zip(start)
                        .zip(count)
                        .all(|((&x, &s), &n)| x >= s && x - s < n)
                {
                    f(k as u64, c);
                }
            }
        }
        Sel::None | Sel::All => {}
    }
}

/// The coordinates of the `k`-th element of a resolved selection.
fn nth(sel: &Sel, mut k: u64, out: &mut [u64]) {
    match sel {
        Sel::Grid(g) => {
            for d in (0..out.len()).rev() {
                let n = g[d].len() as u64;
                out[d] = g[d][(k % n) as usize];
                k /= n;
            }
        }
        Sel::Points(p) => out.copy_from_slice(&p[k as usize]),
        Sel::None | Sel::All => {}
    }
}

/// Reads the box `start`/`count` of a virtual dataset as stored bytes into `out` (filled with
/// the fill value beforehand). Mappings whose source cannot be reached are left at the fill
/// value; their number is returned.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_box<'a>(
    file: &'a File,
    data: &'a [u8],
    ds: &crate::Dataset,
    start: &[u64],
    count: &[u64],
    esize: usize,
    out: &mut [u8],
    cancel: &(dyn Fn() -> bool + Sync),
    resolve: &Resolve<'a>,
    depth: u32,
) -> Result<usize> {
    let maps = match &ds.vds {
        Some(Ok(m)) => m,
        Some(Err(e)) => return Err(e.clone()),
        None => return Err(Error::Malformed("virtual dataset without mappings".into())),
    };
    if depth >= MAX_DEPTH {
        return Err(Error::Unsupported(
            "virtual datasets nested too deeply".into(),
        ));
    }
    let rank = start.len();
    let mut dstride = vec![1u64; rank];
    for d in (0..rank.saturating_sub(1)).rev() {
        dstride[d] = dstride[d + 1] * count[d + 1];
    }
    let mut missing = 0;
    for m in maps {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let dst = m.dst.resolve(&ds.space.dims)?;
        let mut any = false;
        visit(&dst, start, count, &mut |_, _| any = true);
        if !any {
            continue;
        }
        let (sf, sdata) = match &m.file {
            None => (file, data),
            Some(name) => match resolve(name) {
                Some(x) => x,
                None => {
                    missing += 1;
                    continue;
                }
            },
        };
        let Some(si) = sf.index_of(&m.dataset) else {
            missing += 1;
            continue;
        };
        let sds = sf
            .dataset_at(si)
            .map_err(|_| Error::Malformed("virtual dataset source is not a dataset".into()))?;
        if sds.dtype != ds.dtype || sds.elem_size(&sf.ctx) != esize {
            return Err(Error::Unsupported(
                "virtual dataset whose source has another type".into(),
            ));
        }
        let srank = sds.space.dims.len();
        let src = m.src.resolve(&sds.space.dims)?;
        if src.count() != dst.count() {
            return Err(Error::Malformed("virtual mapping sizes differ".into()));
        }
        // The bounding box of the source elements needed.
        let mut lo = vec![u64::MAX; srank];
        let mut hi = vec![0u64; srank];
        let mut sc = vec![0u64; srank];
        visit(&dst, start, count, &mut |k, _| {
            nth(&src, k, &mut sc);
            for d in 0..srank {
                lo[d] = lo[d].min(sc[d]);
                hi[d] = hi[d].max(sc[d]);
            }
        });
        if (0..srank).any(|d| hi[d] >= sds.space.dims[d]) {
            return Err(Error::Malformed(
                "virtual source selection outside its dataset".into(),
            ));
        }
        let scount: Vec<u64> = (0..srank).map(|d| hi[d] - lo[d] + 1).collect();
        let sbox = crate::read::box_bytes_in(
            sf,
            sdata,
            si,
            sds,
            &lo,
            &scount,
            cancel,
            resolve,
            depth + 1,
        )?;
        let mut sstride = vec![1u64; srank];
        for d in (0..srank.saturating_sub(1)).rev() {
            sstride[d] = sstride[d + 1] * scount[d + 1];
        }
        visit(&dst, start, count, &mut |k, dc| {
            nth(&src, k, &mut sc);
            let s: u64 = (0..srank).map(|d| (sc[d] - lo[d]) * sstride[d]).sum();
            let t: u64 = (0..rank).map(|d| (dc[d] - start[d]) * dstride[d]).sum();
            let (s, t) = (s as usize * esize, t as usize * esize);
            out[t..t + esize].copy_from_slice(&sbox[s..s + esize]);
        });
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_order_maps_elements() {
        // A 2 x 3 block of a 4 x 6 space, against a strided 3 x 2 source grid.
        let dst = Sel::Grid(vec![vec![1, 2], vec![3, 4, 5]]);
        let src = Sel::Grid(vec![vec![0, 2, 4], vec![1, 3]]);
        let mut got = Vec::new();
        let mut sc = [0u64; 2];
        visit(&dst, &[0, 4], &[4, 2], &mut |k, c| {
            nth(&src, k, &mut sc);
            got.push((c.to_vec(), sc));
        });
        assert_eq!(
            got,
            vec![
                (vec![1, 4], [0, 3]),
                (vec![1, 5], [2, 1]),
                (vec![2, 4], [4, 1]),
                (vec![2, 5], [4, 3]),
            ]
        );
    }
}

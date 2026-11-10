//! Chunk indexes: B-tree v1, single chunk, implicit, fixed array, extensible array and
//! B-tree v2, all flattened into one map from chunk grid position to the stored chunk.

use std::collections::HashMap;

use crate::io::{Ctx, Cur, log2_floor};
use crate::layout::{IndexKind, Layout};
use crate::{Dataset, Error, Result, limits};

/// Where a chunk is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Loc {
    pub addr: u64,
    /// Stored (filtered) size.
    pub size: u64,
    /// Filters skipped for this chunk (bit i = filter i).
    pub mask: u32,
}

#[derive(Debug)]
pub(crate) struct Index {
    /// Chunks per dimension over the current extent.
    grid: Vec<u64>,
    map: HashMap<u64, Loc>,
    /// Implicit index: the address of chunk 0, the chunk size, and the linear-index strides.
    implicit: Option<(u64, u64, Vec<u64>)>,
}

impl Index {
    pub fn grid(&self) -> &[u64] {
        &self.grid
    }

    /// The chunk at a grid position, `None` when it was never written.
    pub fn get(&self, scaled: &[u64]) -> Option<Loc> {
        if let Some((base, size, down)) = &self.implicit {
            // The strides come from the maximum extent and may be huge: an address that
            // overflows cannot be in the file, so it becomes one that fails to read.
            let addr = scaled
                .iter()
                .zip(down)
                .try_fold(0u64, |a, (s, d)| a.checked_add(s.checked_mul(*d)?))
                .and_then(|lin| lin.checked_mul(*size)?.checked_add(*base))
                .unwrap_or(u64::MAX);
            return Some(Loc {
                addr,
                size: *size,
                mask: 0,
            });
        }
        self.map.get(&linear(scaled, &self.grid)?).copied()
    }
}

/// Row-major linear position in a grid, `None` outside it.
fn linear(scaled: &[u64], grid: &[u64]) -> Option<u64> {
    let mut lin = 0u64;
    for (&s, &g) in scaled.iter().zip(grid) {
        if s >= g {
            return None;
        }
        lin = lin * g + s;
    }
    Some(lin)
}

/// Suffix products of `n`: the stride of each dimension in a row-major linear index.
fn down(n: &[u64]) -> Vec<u64> {
    let mut d = vec![1u64; n.len()];
    for i in (0..n.len().saturating_sub(1)).rev() {
        d[i] = d[i + 1].saturating_mul(n[i + 1]);
    }
    d
}

/// Splits a linear index by strides.
fn unlinear(mut lin: u64, down: &[u64]) -> Vec<u64> {
    down.iter()
        .map(|&d| {
            let s = lin / d.max(1);
            lin %= d.max(1);
            s
        })
        .collect()
}

pub(crate) fn build(data: &[u8], c: &Ctx, ds: &Dataset) -> Result<Index> {
    let Layout::Chunked {
        dims: cdims,
        index,
        addr,
        ..
    } = &ds.layout
    else {
        return Err(Error::Malformed("not chunked".into()));
    };
    let rank = cdims.len();
    let elem = ds.elem_size(c) as u64;
    let chunk_bytes = cdims
        .iter()
        .try_fold(elem, |a, &d| a.checked_mul(d))
        .filter(|&b| b <= limits::MAX_CHUNK_BYTES)
        .ok_or(Error::TooLarge)?;
    let grid: Vec<u64> = ds
        .space
        .dims
        .iter()
        .zip(cdims)
        .map(|(&d, &cd)| d.div_ceil(cd))
        .collect();
    let total = grid.iter().try_fold(1u64, |a, &g| a.checked_mul(g));
    if total.is_none_or(|t| t > limits::MAX_CHUNKS) {
        return Err(Error::TooLarge);
    }
    // The grid over the maximum extent (current where unlimited), for the array indexes.
    let max_grid: Vec<u64> = ds
        .space
        .max
        .iter()
        .zip(&ds.space.dims)
        .zip(cdims)
        .map(|((&m, &d), &cd)| if m == u64::MAX { d } else { m.max(d) }.div_ceil(cd))
        .collect();
    let mut idx = Index {
        grid: grid.clone(),
        map: HashMap::new(),
        implicit: None,
    };
    let Some(addr) = *addr else {
        return Ok(idx);
    };
    let insert = |idx: &mut Index, scaled: &[u64], loc: Loc| -> Result<()> {
        if idx.map.len() as u64 >= limits::MAX_CHUNKS {
            return Err(Error::TooLarge);
        }
        if let Some(l) = linear(scaled, &idx.grid) {
            idx.map.insert(l, loc);
        }
        Ok(())
    };
    match *index {
        IndexKind::BTree1 => {
            let key = 8 + 8 * (rank + 1);
            let mut found: Vec<(Vec<u64>, Loc)> = Vec::new();
            crate::group::btree1_leaves(data, c, addr, 1, key, &mut |k, child| {
                let mut r = Cur::new(k, 0);
                let size = u64::from(r.u32()?);
                let mask = r.u32()?;
                let mut scaled = Vec::with_capacity(rank);
                for &cd in cdims {
                    scaled.push(r.u64()? / cd);
                }
                if found.len() as u64 >= limits::MAX_CHUNKS {
                    return Err(Error::TooLarge);
                }
                found.push((
                    scaled,
                    Loc {
                        addr: child,
                        size,
                        mask,
                    },
                ));
                Ok(())
            })?;
            for (s, l) in found {
                insert(&mut idx, &s, l)?;
            }
        }
        IndexKind::Single { filtered } => {
            let (size, mask) = filtered.unwrap_or((chunk_bytes, 0));
            insert(&mut idx, &vec![0; rank], Loc { addr, size, mask })?;
        }
        IndexKind::Implicit => {
            idx.implicit = Some((addr, chunk_bytes, down(&max_grid)));
        }
        IndexKind::FixedArray => {
            let strides = down(&max_grid);
            fixed_array(data, c, addr, chunk_bytes, &mut |i, loc| {
                insert(&mut idx, &unlinear(i, &strides), loc)
            })?;
        }
        IndexKind::ExtensibleArray => {
            let unlim: Vec<usize> = (0..rank).filter(|&d| ds.space.max[d] == u64::MAX).collect();
            let &[u] = unlim.as_slice() else {
                return Err(Error::Malformed(
                    "extensible array index without one unlimited dimension".into(),
                ));
            };
            // The unlimited dimension is moved first ("swizzled") for the linear index.
            let mut sw = vec![max_grid[u]];
            sw.extend((0..rank).filter(|&d| d != u).map(|d| max_grid[d]));
            let strides = down(&sw);
            extensible_array(data, c, addr, chunk_bytes, &mut |i, loc| {
                let s = unlinear(i, &strides);
                let mut scaled = vec![0u64; rank];
                scaled[u] = s[0];
                for (k, d) in (0..rank).filter(|&d| d != u).enumerate() {
                    scaled[d] = s[k + 1];
                }
                insert(&mut idx, &scaled, loc)
            })?;
        }
        IndexKind::BTree2 => {
            let so = c.so as usize;
            let mut found: Vec<(Vec<u64>, Loc)> = Vec::new();
            crate::btree2::records(data, c, addr, &mut |kind, rec| {
                let mut r = Cur::new(rec, 0);
                let a = r.uint(so)?;
                let (size, mask) = match kind {
                    11 => {
                        let n = rec
                            .len()
                            .checked_sub(so + 4 + 8 * rank)
                            .filter(|n| (1..=8).contains(n))
                            .ok_or_else(|| Error::Malformed("chunk record size".into()))?;
                        (r.uint(n)?, r.u32()?)
                    }
                    10 => (chunk_bytes, 0),
                    k => return Err(Error::Malformed(format!("B-tree v2 type {k} for chunks"))),
                };
                let mut scaled = Vec::with_capacity(rank);
                for _ in 0..rank {
                    scaled.push(r.u64()?);
                }
                if found.len() as u64 >= limits::MAX_CHUNKS {
                    return Err(Error::TooLarge);
                }
                found.push((
                    scaled,
                    Loc {
                        addr: a,
                        size,
                        mask,
                    },
                ));
                Ok(())
            })?;
            for (s, l) in found {
                insert(&mut idx, &s, l)?;
            }
        }
    }
    Ok(idx)
}

/// Decodes one array element (a chunk address, and for filtered chunks a size and mask).
fn element(e: &[u8], c: &Ctx, filtered: bool, chunk_bytes: u64) -> Result<Option<Loc>> {
    let mut r = Cur::new(e, 0);
    let Some(addr) = r.addr(c)? else {
        return Ok(None);
    };
    if filtered {
        let n = e
            .len()
            .checked_sub(c.so as usize + 4)
            .filter(|n| (1..=8).contains(n))
            .ok_or_else(|| Error::Malformed("filtered chunk entry".into()))?;
        let size = r.uint(n)?;
        let mask = r.u32()?;
        Ok(Some(Loc { addr, size, mask }))
    } else {
        Ok(Some(Loc {
            addr,
            size: chunk_bytes,
            mask: 0,
        }))
    }
}

fn bit_msb(bitmap: &[u8], i: u64) -> bool {
    bitmap
        .get((i / 8) as usize)
        .is_some_and(|b| b & (0x80 >> (i % 8)) != 0)
}

fn fixed_array(
    data: &[u8],
    c: &Ctx,
    addr: u64,
    chunk_bytes: u64,
    f: &mut dyn FnMut(u64, Loc) -> Result<()>,
) -> Result<()> {
    let so = c.so as u64;
    let mut r = c.cur(data, addr)?;
    r.sig(b"FAHD", "fixed array header")?;
    r.skip(1)?;
    let client = r.u8()?;
    let esize = u64::from(r.u8()?);
    let page_bits = r.u8()?;
    let n = r.len(c)?;
    let Some(dblk) = r.addr(c)? else {
        return Ok(());
    };
    if esize < so || page_bits > 32 || n > limits::MAX_CHUNKS {
        return Err(Error::Malformed("fixed array header".into()));
    }
    let filtered = client == 1;
    let mut r = c.cur(data, dblk)?;
    r.sig(b"FADB", "fixed array data block")?;
    let page = 1u64 << page_bits;
    let prefix = 4 + 1 + 1 + so;
    if n > page {
        let npages = n.div_ceil(page);
        let bitmap = c.slice(data, dblk + prefix, npages.div_ceil(8))?;
        let mut pos = dblk + prefix + npages.div_ceil(8) + 4;
        for p in 0..npages {
            let count = page.min(n - p * page);
            if bit_msb(bitmap, p) {
                let b = c.slice(data, pos, count * esize)?;
                for (k, e) in b.chunks_exact(esize as usize).enumerate() {
                    if let Some(loc) = element(e, c, filtered, chunk_bytes)? {
                        f(p * page + k as u64, loc)?;
                    }
                }
            }
            pos += page * esize + 4;
        }
    } else {
        let b = c.slice(data, dblk + prefix, n * esize)?;
        for (k, e) in b.chunks_exact(esize as usize).enumerate() {
            if let Some(loc) = element(e, c, filtered, chunk_bytes)? {
                f(k as u64, loc)?;
            }
        }
    }
    Ok(())
}

fn extensible_array(
    data: &[u8],
    c: &Ctx,
    addr: u64,
    chunk_bytes: u64,
    f: &mut dyn FnMut(u64, Loc) -> Result<()>,
) -> Result<()> {
    let so = c.so as u64;
    let mut r = c.cur(data, addr)?;
    r.sig(b"EAHD", "extensible array header")?;
    r.skip(1)?;
    let client = r.u8()?;
    let esize = u64::from(r.u8()?);
    let max_bits = u32::from(r.u8()?);
    let iblk_elmts = u64::from(r.u8()?);
    let dblk_min = u64::from(r.u8()?);
    let sblk_min_ptrs = u64::from(r.u8()?);
    let page_bits = u32::from(r.u8()?);
    for _ in 0..4 {
        r.len(c)?;
    }
    let max_idx_set = r.len(c)?;
    r.len(c)?;
    let Some(iblk) = r.addr(c)? else {
        return Ok(());
    };
    if esize < so
        || !(1..=64).contains(&max_bits)
        || dblk_min == 0
        || !dblk_min.is_power_of_two()
        || sblk_min_ptrs == 0
        || !sblk_min_ptrs.is_power_of_two()
        || page_bits > 32
        || log2_floor(dblk_min) > max_bits
    {
        return Err(Error::Malformed("extensible array header".into()));
    }
    let filtered = client == 1;
    let es = esize as usize;
    let arr_off = u64::from(max_bits.div_ceil(8));
    let page = 1u64 << page_bits;
    let nsblks = 1 + (max_bits - log2_floor(dblk_min)) as u64;
    // Per super block: data blocks, elements per data block, first element index.
    let mut info = Vec::with_capacity(nsblks as usize);
    let mut start = 0u64;
    for u in 0..nsblks {
        let ndblks = 1u64 << (u / 2);
        let nelmts = (1u64 << u.div_ceil(2).min(63)).saturating_mul(dblk_min);
        info.push((ndblks, nelmts, start));
        start = start.saturating_add(ndblks.saturating_mul(nelmts));
    }
    let iblk_nsblks = (2 * log2_floor(sblk_min_ptrs)) as u64;
    let ndblk_addrs = 2 * (sblk_min_ptrs - 1);
    let nsblk_addrs = nsblks.saturating_sub(iblk_nsblks);
    let mut r = c.cur(data, iblk)?;
    r.sig(b"EAIB", "extensible array index block")?;
    r.skip(2)?;
    r.addr(c)?;
    let elems = r.bytes(es * iblk_elmts as usize)?;
    for (k, e) in elems.chunks_exact(es).enumerate() {
        if (k as u64) < max_idx_set
            && let Some(loc) = element(e, c, filtered, chunk_bytes)?
        {
            f(k as u64, loc)?;
        }
    }
    let mut dblk_addrs = Vec::with_capacity(ndblk_addrs as usize);
    for _ in 0..ndblk_addrs {
        dblk_addrs.push(r.addr(c)?);
    }
    let mut sblk_addrs = Vec::with_capacity(nsblk_addrs as usize);
    for _ in 0..nsblk_addrs {
        sblk_addrs.push(r.addr(c)?);
    }
    let mut emitted = 0u64;
    // One data block: `nelmts` elements from global index `first`.
    let mut data_block =
        |a: u64, nelmts: u64, first: u64, pages_init: Option<&dyn Fn(u64) -> bool>| -> Result<()> {
            if first >= max_idx_set {
                return Ok(());
            }
            let mut r = c.cur(data, a)?;
            r.sig(b"EADB", "extensible array data block")?;
            let prefix = 4 + 1 + 1 + so + arr_off;
            // The block holds all its elements (and page checksums): it must fit in the file,
            // which also bounds the page loop below.
            if nelmts
                .checked_mul(esize)
                .is_none_or(|b| b > data.len() as u64)
            {
                return Err(Error::Malformed("extensible array data block".into()));
            }
            let mut emit = |b: &[u8], from: u64| -> Result<()> {
                for (k, e) in b.chunks_exact(es).enumerate() {
                    let i = first.saturating_add(from + k as u64);
                    if i >= max_idx_set {
                        break;
                    }
                    emitted += 1;
                    if emitted > limits::MAX_CHUNKS {
                        return Err(Error::TooLarge);
                    }
                    if let Some(loc) = element(e, c, filtered, chunk_bytes)? {
                        f(i, loc)?;
                    }
                }
                Ok(())
            };
            if nelmts > page {
                let mut pos = a + prefix + 4;
                for p in 0..nelmts / page {
                    if pages_init.is_none_or(|init| init(p)) {
                        emit(c.slice(data, pos, page * esize)?, p * page)?;
                    }
                    pos += page * esize + 4;
                }
            } else {
                emit(c.slice(data, a + prefix, nelmts * esize)?, 0)?;
            }
            Ok(())
        };
    // Data blocks referenced by the index block.
    let mut d = 0usize;
    for &(ndblks, nelmts, first) in info.iter().take(iblk_nsblks as usize) {
        for j in 0..ndblks {
            if let Some(Some(a)) = dblk_addrs.get(d) {
                let at = iblk_elmts
                    .saturating_add(first)
                    .saturating_add(j.saturating_mul(nelmts));
                data_block(*a, nelmts, at, None)?;
            }
            d += 1;
        }
    }
    // Data blocks behind secondary blocks.
    for (k, sa) in sblk_addrs.iter().enumerate() {
        let Some(sa) = *sa else { continue };
        let (ndblks, nelmts, first) = info[iblk_nsblks as usize + k];
        if iblk_elmts.saturating_add(first) >= max_idx_set {
            break;
        }
        let mut r = c.cur(data, sa)?;
        r.sig(b"EASB", "extensible array secondary block")?;
        r.skip(2)?;
        r.addr(c)?;
        r.uint(arr_off as usize)?;
        let npages = if nelmts > page { nelmts / page } else { 0 };
        let bitmap = if npages > 0 {
            let bits = ndblks
                .checked_mul(npages)
                .ok_or_else(|| Error::Malformed("extensible array secondary block".into()))?;
            r.bytes(bits.div_ceil(8) as usize)?
        } else {
            &[]
        };
        for j in 0..ndblks {
            let Some(a) = r.addr(c)? else { continue };
            let init = |p: u64| bit_msb(bitmap, j * npages + p);
            let at = iblk_elmts
                .saturating_add(first)
                .saturating_add(j.saturating_mul(nelmts));
            data_block(a, nelmts, at, Some(&init))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod hostile {
    use super::*;

    const CTX: Ctx = Ctx {
        so: 8,
        sl: 8,
        base: 0,
    };

    #[test]
    fn implicit_index_with_huge_strides() {
        // Strides from a near-unlimited maximum extent: the linear index overflows.
        let idx = Index {
            grid: vec![3, 1],
            map: HashMap::new(),
            implicit: Some((16, 1, vec![u64::MAX - 1, 1])),
        };
        assert_eq!(idx.get(&[2, 0]).map(|l| l.addr), Some(u64::MAX));
    }

    #[test]
    fn extensible_array_secondary_bitmap_overflow() {
        // 64-bit indexes, data blocks of at least 2 elements, 1-element pages: the last
        // secondary block has 2^31 data blocks of 2^33 pages, 2^64 bitmap bits.
        let mut d = b"EAHD".to_vec();
        d.extend_from_slice(&[0, 0, 8, 64, 0, 2, 2, 0]);
        for v in [0u64, 0, 0, 0, u64::MAX, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        let iblk = d.len() as u64 + 8;
        d.extend_from_slice(&iblk.to_le_bytes());
        d.extend_from_slice(b"EAIB\0\0");
        d.extend_from_slice(&0u64.to_le_bytes());
        // Two data block addresses (undefined), 62 secondary block addresses: only the last
        // is defined.
        d.extend_from_slice(&[0xff; 16]);
        let sblk = d.len() as u64 + 62 * 8;
        for k in 0..62 {
            let a = if k == 61 { sblk } else { u64::MAX };
            d.extend_from_slice(&a.to_le_bytes());
        }
        d.extend_from_slice(b"EASB\0\0");
        d.extend_from_slice(&[0; 16]);
        d.extend_from_slice(&[0; 64]);
        let r = extensible_array(&d, &CTX, 0, 8, &mut |_, _| Ok(()));
        assert!(r.is_err());
    }
}

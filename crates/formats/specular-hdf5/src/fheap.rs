//! Fractal heaps: the objects behind dense links and dense attributes.

use crate::io::{Ctx, Cur, enc_size, log2_floor};
use crate::{Error, Result, limits};
use std::cell::OnceCell;
use std::collections::HashMap;

pub(crate) struct Heap {
    /// Bytes of a heap ID.
    pub id_len: usize,
    width: u64,
    start: u64,
    max_direct: u64,
    off_size: usize,
    len_size: usize,
    /// Root block address and its rows (0 = a direct block).
    root: Option<u64>,
    root_rows: u64,
    /// Direct blocks carry a checksum after their offset.
    checksummed: bool,
    /// The B-tree v2 of huge objects.
    huge: Option<u64>,
    /// Its records by key (address, length), read on the first lookup: a lookup per object
    /// scanning the whole tree was quadratic in the objects.
    huge_index: OnceCell<Result<HugeIndex>>,
}

/// Huge objects by key: (address, length).
type HugeIndex = HashMap<u64, (Option<u64>, u64)>;

#[cfg(test)]
thread_local! {
    /// Huge-object B-tree scans (tests count them).
    static HUGE_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Heap {
    pub fn open(data: &[u8], c: &Ctx, addr: u64) -> Result<Heap> {
        let mut r = c.cur(data, addr)?;
        r.sig(b"FRHP", "fractal heap")?;
        let _version = r.u8()?;
        let id_len = r.u16()? as usize;
        let filter_len = r.u16()?;
        let flags = r.u8()?;
        let max_managed = u64::from(r.u32()?);
        r.len(c)?; // next huge ID
        let huge = r.addr(c)?;
        r.len(c)?; // free space
        r.addr(c)?; // free-space manager
        r.len(c)?; // managed space
        r.len(c)?; // allocated managed space
        r.len(c)?; // direct block allocation iterator
        r.len(c)?; // managed objects
        r.len(c)?; // huge size
        r.len(c)?; // huge count
        r.len(c)?; // tiny size
        r.len(c)?; // tiny count
        let width = u64::from(r.u16()?);
        let start = r.len(c)?;
        let max_direct = r.len(c)?;
        let max_bits = u64::from(r.u16()?);
        let _start_rows = r.u16()?;
        let root = r.addr(c)?;
        let root_rows = u64::from(r.u16()?);
        if filter_len != 0 {
            return Err(Error::Unsupported("filtered fractal heap".into()));
        }
        if width == 0
            || !width.is_power_of_two()
            || start == 0
            || !start.is_power_of_two()
            || max_direct < start
            || !max_direct.is_power_of_two()
            || max_bits == 0
            || max_bits > 64
            || id_len < 2
        {
            return Err(Error::Malformed("fractal heap header".into()));
        }
        let off_size = max_bits.div_ceil(8) as usize;
        // As the library sizes it: the smaller of the bytes for a direct block offset and
        // the bytes for the largest managed object.
        let len_size = (log2_floor(max_direct) as usize)
            .div_ceil(8)
            .min(enc_size(max_managed));
        Ok(Heap {
            id_len,
            width,
            start,
            max_direct,
            off_size,
            len_size: len_size.clamp(1, 8),
            root,
            root_rows,
            checksummed: flags & 0x02 != 0,
            huge,
            huge_index: OnceCell::new(),
        })
    }

    /// Rows of an indirect block that hold direct blocks.
    fn max_direct_rows(&self) -> u64 {
        u64::from(log2_floor(self.max_direct) - log2_floor(self.start)) + 2
    }

    /// Size of the blocks in row `r`.
    fn row_size(&self, r: u64) -> u64 {
        if r == 0 {
            self.start
        } else {
            u32::try_from(r - 1)
                .ok()
                .and_then(|k| 1u64.checked_shl(k))
                .and_then(|m| self.start.checked_mul(m))
                .unwrap_or(u64::MAX)
        }
    }

    /// The object behind a heap ID.
    pub fn get<'a>(&self, data: &'a [u8], c: &Ctx, id: &'a [u8]) -> Result<&'a [u8]> {
        let mut r = Cur::new(id, 0);
        let flags = r.u8()?;
        match (flags >> 4) & 3 {
            0 => {
                let off = r.uint(self.off_size)?;
                let len = r.uint(self.len_size)?;
                self.managed(data, c, off, len)
            }
            2 => {
                let len = usize::from(flags & 0x0f) + 1;
                r.bytes(len)
            }
            1 => self.huge_object(data, c, r),
            _ => Err(Error::Malformed("heap ID".into())),
        }
    }

    /// A huge object: its address and length are in the ID when the ID is long enough,
    /// else the ID is a key into the huge-object B-tree (records: address, length, key).
    fn huge_object<'a>(&self, data: &'a [u8], c: &Ctx, mut r: Cur<'a>) -> Result<&'a [u8]> {
        let (so, sl) = (c.so as usize, c.sl as usize);
        if self.id_len > so + sl {
            let a = r
                .addr(c)?
                .ok_or_else(|| Error::Malformed("huge object".into()))?;
            let n = r.len(c)?;
            return c.slice(data, a, n);
        }
        let key = r.uint((self.id_len - 1).min(8))?;
        let bt = self
            .huge
            .ok_or_else(|| Error::Malformed("huge object without a B-tree".into()))?;
        let index = self
            .huge_index
            .get_or_init(|| huge_index(data, c, bt))
            .as_ref()
            .map_err(Clone::clone)?;
        match index.get(&key) {
            Some(&(Some(a), n)) => c.slice(data, a, n),
            _ => Err(Error::Malformed("huge object not found".into())),
        }
    }

    fn managed<'a>(&self, data: &'a [u8], c: &Ctx, off: u64, len: u64) -> Result<&'a [u8]> {
        let root = self
            .root
            .ok_or_else(|| Error::Malformed("empty fractal heap".into()))?;
        let mut block = root;
        let mut rows = self.root_rows;
        let mut block_off = 0u64;
        let mut steps = 0;
        // Descend through indirect blocks to the direct block holding `off`.
        while rows > 0 {
            steps += 1;
            if steps > 64 {
                return Err(Error::Malformed("fractal heap depth".into()));
            }
            let mut r = c.cur(data, block)?;
            r.sig(b"FHIB", "fractal heap indirect block")?;
            r.skip(1)?;
            r.addr(c)?;
            r.uint(self.off_size)?;
            let entries_start = r.pos;
            let so = c.so as usize;
            let mut found = None;
            let mut row_off = block_off;
            // The row holding `off`, then its column by division: rows double in size, so
            // the offsets saturate within ~64 rows whatever the header's row count and width.
            for row in 0..rows {
                let size = self.row_size(row);
                let row_len = size.saturating_mul(self.width);
                if off < row_off {
                    break;
                }
                if off - row_off < row_len {
                    let col = (off - row_off) / size;
                    let lo = row_off + col * size;
                    let i = (row * self.width + col) as usize;
                    let mut e = Cur::new(data, entries_start.saturating_add(i.saturating_mul(so)));
                    let a = e
                        .addr(c)?
                        .ok_or_else(|| Error::Malformed("heap object in a hole".into()))?;
                    found = Some((a, row, lo, size));
                    break;
                }
                row_off = row_off.saturating_add(row_len);
            }
            let (a, row, lo, size) =
                found.ok_or_else(|| Error::Malformed("heap offset outside the heap".into()))?;
            block = a;
            block_off = lo;
            rows = if row < self.max_direct_rows() {
                0
            } else {
                let child_rows = u64::from(log2_floor(size))
                    .checked_sub(u64::from(log2_floor(self.start) + log2_floor(self.width)))
                    .ok_or_else(|| Error::Malformed("heap row".into()))?
                    + 1;
                child_rows.min(64)
            };
        }
        let mut r = c.cur(data, block)?;
        r.sig(b"FHDB", "fractal heap direct block")?;
        let in_block = off
            .checked_sub(block_off)
            .ok_or_else(|| Error::Malformed("heap offset".into()))?;
        let header =
            5 + u64::from(c.so) + self.off_size as u64 + if self.checksummed { 4 } else { 0 };
        if in_block < header {
            return Err(Error::Malformed("heap offset inside a block header".into()));
        }
        let at = block
            .checked_add(in_block)
            .ok_or_else(|| Error::Malformed("heap offset".into()))?;
        c.slice(data, at, len)
    }
}

/// The huge-object B-tree at `bt` by key: (address, length); the first record of a key wins.
fn huge_index(data: &[u8], c: &Ctx, bt: u64) -> Result<HugeIndex> {
    #[cfg(test)]
    HUGE_SCANS.with(|n| n.set(n.get() + 1));
    let (so, sl) = (c.so as usize, c.sl as usize);
    let mut index = HashMap::new();
    crate::btree2::records(data, c, bt, &mut |kind, rec| {
        if kind != 1 {
            return Ok(());
        }
        if index.len() >= limits::MAX_LINKS {
            return Err(Error::TooLarge);
        }
        let mut q = Cur::new(rec, 0);
        let a = q.addr(c)?;
        let n = q.len(c)?;
        let id = q.uint(rec.len().saturating_sub(so + sl).min(8))?;
        index.entry(id).or_insert((a, n));
        Ok(())
    })?;
    Ok(index)
}

/// Reads every heap ID of a B-tree v2 name index and resolves the objects.
pub(crate) fn objects_by_index<'a>(
    data: &'a [u8],
    c: &Ctx,
    heap: &Heap,
    btree: u64,
    id_at: usize,
) -> Result<Vec<&'a [u8]>> {
    let mut ids: Vec<&'a [u8]> = Vec::new();
    crate::btree2::records(data, c, btree, &mut |_, rec| {
        if ids.len() >= limits::MAX_LINKS {
            return Err(Error::TooLarge);
        }
        let id = rec
            .get(id_at..id_at + heap.id_len)
            .ok_or_else(|| Error::Malformed("heap ID in record".into()))?;
        ids.push(id);
        Ok(())
    })?;
    ids.into_iter().map(|id| heap.get(data, c, id)).collect()
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn indirect_block_with_saturating_rows() {
        // 200 rows of doubling blocks: the row offsets and sizes overflow before the
        // (unreachable) offset is found.
        let heap = Heap {
            id_len: 8,
            width: 2,
            start: 1,
            max_direct: 2,
            off_size: 8,
            len_size: 1,
            root: Some(0),
            root_rows: 200,
            checksummed: false,
            huge: None,
            huge_index: OnceCell::new(),
        };
        let mut d = b"FHIB\0".to_vec();
        d.extend_from_slice(&[0; 16 + 64]);
        let c = Ctx {
            so: 8,
            sl: 8,
            base: 0,
        };
        assert!(heap.managed(&d, &c, u64::MAX - 1, 1).is_err());
        // The widest, tallest header still returns at once.
        let wide = Heap {
            width: 1 << 15,
            root_rows: u64::from(u16::MAX),
            ..heap
        };
        assert!(wide.managed(&d, &c, u64::MAX, 1).is_err());
    }

    /// 40,000 huge objects named through the B-tree: every lookup used to scan the whole tree
    /// (1.6 × 10⁹ records); the tree is now read once.
    #[test]
    fn huge_objects_index_read_once() {
        let n = 40_000u64;
        let c = Ctx {
            so: 8,
            sl: 8,
            base: 0,
        };
        // A leaf-only B-tree v2 of type 1 records (address, length, key), then the objects.
        let rec = 24u64;
        let node = 10 + n * rec;
        let leaf = 64u64;
        let objects = leaf + node;
        let mut d = b"BTHD\0\x01".to_vec();
        d.extend_from_slice(&u32::try_from(node).unwrap().to_le_bytes());
        d.extend_from_slice(&u16::try_from(rec).unwrap().to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(&leaf.to_le_bytes());
        d.extend_from_slice(&u16::try_from(n).unwrap().to_le_bytes());
        d.extend_from_slice(&n.to_le_bytes());
        d.resize(leaf as usize, 0);
        d.extend_from_slice(b"BTLF\0\x01");
        for k in 0..n {
            d.extend_from_slice(&(objects + k).to_le_bytes());
            d.extend_from_slice(&1u64.to_le_bytes());
            d.extend_from_slice(&k.to_le_bytes());
        }
        d.resize(objects as usize, 0);
        d.extend((0..n).map(|k| k as u8));
        let heap = Heap {
            id_len: 9,
            width: 2,
            start: 1,
            max_direct: 2,
            off_size: 8,
            len_size: 1,
            root: None,
            root_rows: 0,
            checksummed: false,
            huge: Some(0),
            huge_index: OnceCell::new(),
        };
        HUGE_SCANS.with(|s| s.set(0));
        for k in 0..n {
            let mut id = vec![0x10];
            id.extend_from_slice(&k.to_le_bytes());
            assert_eq!(heap.get(&d, &c, &id).unwrap(), [k as u8]);
        }
        assert_eq!(HUGE_SCANS.with(std::cell::Cell::get), 1);
    }
}

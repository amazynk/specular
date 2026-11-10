//! Global heap collections: the storage of variable-length data.

use crate::io::{Ctx, Cur};
use crate::{Error, Result};
use std::collections::HashMap;

/// The objects of the global heap collection at `addr` in collection order, up to the end
/// marker, and the error that stopped the walk early on a malformed object.
type Objects<'a> = (Vec<(u32, &'a [u8])>, Option<Error>);

fn collection<'a>(data: &'a [u8], c: &Ctx, addr: u64) -> Result<Objects<'a>> {
    let mut r = c.cur(data, addr)?;
    r.sig(b"GCOL", "global heap")?;
    r.skip(4)?;
    let size = r.len(c)?;
    let col = c.slice(data, addr, size)?;
    let mut r = Cur::new(col, r.pos - c.pos(data, addr)?);
    let mut out = Vec::new();
    let mut walk = || -> Result<()> {
        while r.remaining() >= 8 + c.sl as usize {
            let idx = r.u16()?;
            r.skip(6)?;
            let n = r.len(c)? as usize;
            if idx == 0 {
                break;
            }
            out.push((u32::from(idx), r.bytes(n)?));
            let pad = (8 - n % 8) % 8;
            if r.skip(pad).is_err() {
                break;
            }
        }
        Ok(())
    };
    let stop = walk().err();
    Ok((out, stop))
}

fn not_found(index: u32) -> Error {
    Error::Malformed(format!("global heap object {index} not found"))
}

/// The object `index` of the global heap collection at `addr`.
pub(crate) fn object<'a>(data: &'a [u8], c: &Ctx, addr: u64, index: u32) -> Result<&'a [u8]> {
    let (objs, stop) = collection(data, c, addr)?;
    match objs.iter().find(|(i, _)| *i == index) {
        Some((_, o)) => Ok(o),
        None => Err(stop.unwrap_or_else(|| not_found(index))),
    }
}

type Indexed<'a> = (HashMap<u32, &'a [u8]>, Option<Error>);

/// Global heap collections indexed once each: looking every element of a variable-length
/// attribute up by a walk of its collection was quadratic in the element count.
#[derive(Default)]
pub(crate) struct Heaps<'a> {
    cols: HashMap<u64, Result<Indexed<'a>>>,
}

impl<'a> Heaps<'a> {
    /// The object `index` of the collection at `addr`; the same result as [`object`].
    pub fn object(&mut self, data: &'a [u8], c: &Ctx, addr: u64, index: u32) -> Result<&'a [u8]> {
        let col = self.cols.entry(addr).or_insert_with(|| {
            collection(data, c, addr).map(|(objs, stop)| {
                let mut by_index = HashMap::with_capacity(objs.len());
                for (i, o) in objs {
                    by_index.entry(i).or_insert(o);
                }
                (by_index, stop)
            })
        });
        match col {
            Ok((by_index, stop)) => match by_index.get(&index) {
                Some(o) => Ok(o),
                None => Err(stop.clone().unwrap_or_else(|| not_found(index))),
            },
            Err(e) => Err(e.clone()),
        }
    }

    /// A variable-length element through the index; the same result as [`vlen`].
    pub fn vlen(&mut self, data: &'a [u8], c: &Ctx, elem: &[u8]) -> Result<&'a [u8]> {
        match vlen_ref(elem, c)? {
            None => Ok(&[]),
            Some((a, index)) => self.object(data, c, a, index),
        }
    }
}

/// The (collection address, object index) of a variable-length element; `None` for an
/// empty one.
fn vlen_ref(elem: &[u8], c: &Ctx) -> Result<Option<(u64, u32)>> {
    let mut r = Cur::new(elem, 0);
    let len = r.u32()?;
    let addr = r.addr(c)?;
    let index = r.u32()?;
    Ok(match addr {
        Some(a) if len != 0 => Some((a, index)),
        _ => None,
    })
}

/// A variable-length element: (length, collection address, object index).
pub(crate) fn vlen<'a>(data: &'a [u8], c: &Ctx, elem: &[u8]) -> Result<&'a [u8]> {
    match vlen_ref(elem, c)? {
        None => Ok(&[]),
        Some((a, index)) => object(data, c, a, index),
    }
}

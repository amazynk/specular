//! A dataset's storage: the layout message, the filter pipeline and the fill value.

use crate::io::{Ctx, Cur};
use crate::ohdr::{
    Header, MSG_DATASPACE, MSG_DATATYPE, MSG_FILL, MSG_FILL_OLD, MSG_FILTERS, MSG_LAYOUT,
};
use crate::types::{Dtype, Num, compound_fields, parse_dataspace, parse_dtype};
use crate::{Dataset, Error, Result};

/// How the chunks of a chunked dataset are indexed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IndexKind {
    BTree1,
    /// One chunk: its address, and for a filtered chunk its size and filter mask.
    Single {
        filtered: Option<(u64, u32)>,
    },
    Implicit,
    FixedArray,
    ExtensibleArray,
    BTree2,
}

#[derive(Clone, Debug)]
pub(crate) enum Layout {
    Compact {
        data: Vec<u8>,
    },
    Contiguous {
        addr: Option<u64>,
        size: u64,
    },
    Chunked {
        /// Chunk shape in elements, one per dataset dimension.
        dims: Vec<u64>,
        index: IndexKind,
        /// Address of the index (or of the single chunk / the implicit chunk array).
        addr: Option<u64>,
        /// Partial edge chunks are stored unfiltered.
        unfiltered_edges: bool,
    },
    /// Mappings from other datasets, kept in the global heap object (address, index).
    Virtual {
        heap: Option<(u64, u32)>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Filter {
    pub id: u16,
    pub params: Vec<u32>,
}

/// The fill value pattern (one element), when one is defined.
#[derive(Clone, Debug, Default)]
pub(crate) struct Fill {
    pub value: Option<Vec<u8>>,
}

pub(crate) fn dataset(c: &Ctx, h: &Header) -> Result<Dataset> {
    let space = parse_dataspace(
        h.find(MSG_DATASPACE)
            .ok_or_else(|| Error::Malformed("dataset without a dataspace".into()))?
            .data,
        c,
    )?;
    let dt_raw = h
        .find(MSG_DATATYPE)
        .ok_or_else(|| Error::Malformed("dataset without a datatype".into()))?
        .data;
    let dtype = parse_dtype(dt_raw)?.0;
    let members = compound_fields(dt_raw).unwrap_or_default();
    let rank = space.dims.len();
    let layout = layout(
        h.find(MSG_LAYOUT)
            .ok_or_else(|| Error::Malformed("dataset without a layout".into()))?
            .data,
        c,
        rank,
    )?;
    let mut filters = match h.find(MSG_FILTERS) {
        Some(m) => pipeline(m.data)?,
        None => Vec::new(),
    };
    // N-bit packs an integer's significant bits, which the file type places at a bit offset;
    // its parameters omit the sign, so pass it on for the filter to sign-extend.
    if let Dtype::Num { num, .. } = &dtype {
        let signed = matches!(num, Num::I8 | Num::I16 | Num::I32 | Num::I64);
        for f in filters.iter_mut().filter(|f| f.id == crate::filters::NBIT) {
            f.params.push(u32::from(signed));
        }
    }
    let elem = dtype.size(c);
    let fill = fill(h, elem)?;
    let layout = match layout {
        Layout::Contiguous { addr, size: 0 } => Layout::Contiguous {
            addr,
            size: space.count().saturating_mul(elem as u64),
        },
        l => l,
    };
    Ok(Dataset {
        dtype,
        space,
        layout,
        filters,
        fill,
        index: Default::default(),
        vds: None,
        field: None,
        members,
    })
}

fn chunk_dims(mut dims: Vec<u64>, rank: usize) -> Result<Vec<u64>> {
    // The stored dimensions end with the element size.
    if dims.len() == rank + 1 {
        dims.pop();
    }
    if dims.len() != rank || dims.contains(&0) {
        return Err(Error::Malformed("chunk dimensions".into()));
    }
    Ok(dims)
}

fn layout(b: &[u8], c: &Ctx, rank: usize) -> Result<Layout> {
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    match version {
        1 | 2 => {
            let nd = r.u8()? as usize;
            let class = r.u8()?;
            r.skip(5)?;
            let addr = if class != 0 { r.addr(c)? } else { None };
            let mut dims = Vec::with_capacity(nd);
            for _ in 0..nd {
                dims.push(u64::from(r.u32()?));
            }
            match class {
                0 => {
                    let n = r.u32()? as usize;
                    Ok(Layout::Compact {
                        data: r.bytes(n)?.to_vec(),
                    })
                }
                1 => Ok(Layout::Contiguous { addr, size: 0 }),
                2 => Ok(Layout::Chunked {
                    dims: chunk_dims(dims, rank)?,
                    index: IndexKind::BTree1,
                    addr,
                    unfiltered_edges: false,
                }),
                _ => Err(Error::Malformed(format!("layout class {class}"))),
            }
        }
        // Version 5 (HDF5 2.0) encodes the same fields as version 4.
        3..=5 => {
            let class = r.u8()?;
            match class {
                0 => {
                    let n = r.u16()? as usize;
                    Ok(Layout::Compact {
                        data: r.bytes(n)?.to_vec(),
                    })
                }
                1 => {
                    let addr = r.addr(c)?;
                    let size = r.len(c)?;
                    Ok(Layout::Contiguous { addr, size })
                }
                2 if version == 3 => {
                    let nd = r.u8()? as usize;
                    let addr = r.addr(c)?;
                    let mut dims = Vec::with_capacity(nd);
                    for _ in 0..nd {
                        dims.push(u64::from(r.u32()?));
                    }
                    Ok(Layout::Chunked {
                        dims: chunk_dims(dims, rank)?,
                        index: IndexKind::BTree1,
                        addr,
                        unfiltered_edges: false,
                    })
                }
                2 => {
                    let flags = r.u8()?;
                    let nd = r.u8()? as usize;
                    let enc = r.u8()? as usize;
                    if !(1..=8).contains(&enc) {
                        return Err(Error::Malformed("chunk dimension size".into()));
                    }
                    let mut dims = Vec::with_capacity(nd);
                    for _ in 0..nd {
                        dims.push(r.uint(enc)?);
                    }
                    let kind = r.u8()?;
                    let index = match kind {
                        1 => {
                            if flags & 2 != 0 {
                                let size = r.len(c)?;
                                let mask = r.u32()?;
                                IndexKind::Single {
                                    filtered: Some((size, mask)),
                                }
                            } else {
                                IndexKind::Single { filtered: None }
                            }
                        }
                        2 => IndexKind::Implicit,
                        3 => {
                            r.skip(1)?;
                            IndexKind::FixedArray
                        }
                        4 => {
                            r.skip(5)?;
                            IndexKind::ExtensibleArray
                        }
                        5 => {
                            r.skip(6)?;
                            IndexKind::BTree2
                        }
                        k => return Err(Error::Unsupported(format!("chunk index type {k}"))),
                    };
                    let addr = r.addr(c)?;
                    Ok(Layout::Chunked {
                        dims: chunk_dims(dims, rank)?,
                        index,
                        addr,
                        unfiltered_edges: flags & 1 != 0,
                    })
                }
                3 => {
                    let addr = r.addr(c)?;
                    let index = r.u32()?;
                    Ok(Layout::Virtual {
                        heap: addr.map(|a| (a, index)),
                    })
                }
                _ => Err(Error::Malformed(format!("layout class {class}"))),
            }
        }
        _ => Err(Error::Unsupported(format!("layout version {version}"))),
    }
}

fn pipeline(b: &[u8]) -> Result<Vec<Filter>> {
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    let n = r.u8()? as usize;
    if version == 1 {
        r.skip(6)?;
    } else if version != 2 {
        return Err(Error::Unsupported(format!(
            "filter pipeline version {version}"
        )));
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let id = r.u16()?;
        let name_len = if version == 1 || id >= 256 {
            r.u16()? as usize
        } else {
            0
        };
        let _flags = r.u16()?;
        let nparams = r.u16()? as usize;
        if version == 1 {
            r.skip(name_len.div_ceil(8) * 8)?;
        } else {
            r.skip(name_len)?;
        }
        let mut params = Vec::with_capacity(nparams.min(64));
        for _ in 0..nparams {
            params.push(r.u32()?);
        }
        if version == 1 && nparams % 2 == 1 {
            r.skip(4)?;
        }
        out.push(Filter { id, params });
    }
    Ok(out)
}

fn fill(h: &Header, elem: usize) -> Result<Fill> {
    let pick = |v: &[u8]| -> Option<Vec<u8>> { (v.len() == elem && elem > 0).then(|| v.to_vec()) };
    if let Some(m) = h.find(MSG_FILL) {
        let mut r = Cur::new(m.data, 0);
        let version = r.u8()?;
        let value = match version {
            1 | 2 => {
                r.skip(2)?;
                let defined = r.u8()?;
                if version == 1 || defined != 0 {
                    let n = r.u32()? as usize;
                    pick(r.bytes(n)?)
                } else {
                    None
                }
            }
            3 => {
                let flags = r.u8()?;
                if flags & 0x20 != 0 {
                    let n = r.u32()? as usize;
                    pick(r.bytes(n)?)
                } else {
                    None
                }
            }
            _ => None,
        };
        return Ok(Fill { value });
    }
    if let Some(m) = h.find(MSG_FILL_OLD) {
        let mut r = Cur::new(m.data, 0);
        let n = r.u32()? as usize;
        return Ok(Fill {
            value: pick(r.bytes(n)?),
        });
    }
    Ok(Fill::default())
}

//! The superblock and object headers (v1 and v2, with continuation blocks and shared
//! messages).

use crate::fheap::Heap;
use crate::io::{Ctx, Cur};
use crate::{Error, Result, find_superblock, limits};
use std::collections::HashMap;

pub(crate) const MSG_DATASPACE: u16 = 0x01;
pub(crate) const MSG_LINK_INFO: u16 = 0x02;
pub(crate) const MSG_DATATYPE: u16 = 0x03;
pub(crate) const MSG_FILL_OLD: u16 = 0x04;
pub(crate) const MSG_FILL: u16 = 0x05;
pub(crate) const MSG_LINK: u16 = 0x06;
pub(crate) const MSG_LAYOUT: u16 = 0x08;
pub(crate) const MSG_FILTERS: u16 = 0x0B;
pub(crate) const MSG_ATTRIBUTE: u16 = 0x0C;
pub(crate) const MSG_CONTINUATION: u16 = 0x10;
pub(crate) const MSG_SYMBOL_TABLE: u16 = 0x11;
pub(crate) const MSG_ATTRIBUTE_INFO: u16 = 0x15;
pub(crate) const MSG_SHARED_TABLE: u16 = 0x0F;

pub(crate) struct Superblock {
    pub version: u8,
    pub ctx: Ctx,
    /// Object header address of the root group.
    pub root: u64,
    /// The root group's cached B-tree and local heap addresses (v0/v1).
    pub root_symtab: Option<(u64, u64)>,
    /// The superblock extension's object header (v2/v3).
    pub ext: Option<u64>,
}

pub(crate) fn superblock(data: &[u8]) -> Result<Superblock> {
    let at = find_superblock(data).ok_or(Error::NotHdf5)?;
    let mut r = Cur::new(data, at + 8);
    let version = r.u8()?;
    match version {
        0 | 1 => {
            r.skip(3)?; // free-space, root symbol table and reserved versions
            r.skip(1)?; // shared header version
            let so = r.u8()?;
            let sl = r.u8()?;
            check_sizes(so, sl)?;
            r.skip(1)?;
            r.skip(4)?; // group leaf / internal node K
            r.skip(4)?; // consistency flags
            if version == 1 {
                r.skip(4)?; // indexed storage K, reserved
            }
            let c0 = Ctx {
                so,
                sl,
                base: at as u64,
            };
            let _base = r.uint(so as usize)?;
            r.skip(3 * so as usize)?; // free space, end of file, driver info
            // Root group symbol table entry.
            let _name = r.len(&c0)?;
            let root = r
                .addr(&c0)?
                .ok_or_else(|| Error::Malformed("no root group".into()))?;
            let cache = r.u32()?;
            r.skip(4)?;
            let scratch = r.bytes(16)?;
            let root_symtab = (cache == 1).then(|| {
                let mut s = Cur::new(scratch, 0);
                match (s.addr(&c0), s.addr(&c0)) {
                    (Ok(Some(b)), Ok(Some(h))) => Some((b, h)),
                    _ => None,
                }
            });
            Ok(Superblock {
                version,
                ctx: c0,
                root,
                root_symtab: root_symtab.flatten(),
                ext: None,
            })
        }
        2 | 3 => {
            let so = r.u8()?;
            let sl = r.u8()?;
            check_sizes(so, sl)?;
            r.skip(1)?; // flags
            let c = Ctx {
                so,
                sl,
                base: at as u64,
            };
            let _base = r.uint(so as usize)?;
            let ext = r.addr(&c)?;
            let _eof = r.addr(&c)?;
            let root = r
                .addr(&c)?
                .ok_or_else(|| Error::Malformed("no root group".into()))?;
            Ok(Superblock {
                version,
                ctx: c,
                root,
                root_symtab: None,
                ext,
            })
        }
        v => Err(Error::Unsupported(format!("superblock version {v}"))),
    }
}

fn check_sizes(so: u8, sl: u8) -> Result<()> {
    if !matches!(so, 2 | 4 | 8) || !matches!(sl, 2 | 4 | 8) {
        return Err(Error::Malformed(format!("offset/length sizes {so}/{sl}")));
    }
    Ok(())
}

/// One header message.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Msg<'a> {
    pub kind: u16,
    pub flags: u8,
    pub data: &'a [u8],
}

/// An object header's messages, in order, over all continuation blocks.
pub(crate) struct Header<'a> {
    pub msgs: Vec<Msg<'a>>,
}

impl<'a> Header<'a> {
    pub fn find(&self, kind: u16) -> Option<&Msg<'a>> {
        self.msgs.iter().find(|m| m.kind == kind)
    }

    pub fn all(&self, kind: u16) -> impl Iterator<Item = &Msg<'a>> {
        self.msgs.iter().filter(move |m| m.kind == kind)
    }
}

/// A header's first message body of each kind.
type FirstOfKind<'a> = HashMap<u16, &'a [u8]>;

/// Headers that shared messages point at, read once per file: each target's first message
/// of every kind, by (address, resolution depth). Re-reading the target per shared message
/// was quadratic when a header held thousands of shared messages (or attributes with a
/// committed datatype) pointing at one header with thousands of messages.
#[derive(Default)]
pub(crate) struct SharedTargets<'a> {
    map: HashMap<(u64, u32), Result<FirstOfKind<'a>>>,
    /// The file's shared-message indexes ([`sohm_table`]).
    sohm: Vec<SohmIndex>,
    /// Their fractal heaps, opened on first use.
    heaps: HashMap<u64, Result<Heap>>,
}

/// One index of the Shared Message Table: the message kinds it holds (bit `k` for message
/// type `k`, as the HDF5 library writes them: 1 dataspace, 3 datatype, 5 fill value,
/// 11 filter pipeline, 12 attribute) and the fractal heap that stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SohmIndex {
    pub kinds: u16,
    pub heap: u64,
}

/// Where a shared message lives.
enum SharedRef<'a> {
    /// The first message of its kind in the object header at this address.
    Header(u64),
    /// The object with this heap ID in the shared-message heap of its kind.
    Heap(&'a [u8]),
}

/// The Shared Message Table of the superblock extension at `ext` (HDF5 file format III.H,
/// IV.A.2.p): its indexes, empty when the file shares no messages that way. Which index's
/// list or B-tree a message is in does not matter for reading: its heap ID finds it.
pub(crate) fn sohm_table(data: &[u8], c: &Ctx, ext: u64) -> Result<Vec<SohmIndex>> {
    let h = read_header(data, c, ext, &mut SharedTargets::default())?;
    let Some(m) = h.find(MSG_SHARED_TABLE) else {
        return Ok(Vec::new());
    };
    let mut r = Cur::new(m.data, 0);
    if r.u8()? != 0 {
        return Err(Error::Malformed("shared message table version".into()));
    }
    let table = r
        .addr(c)?
        .ok_or_else(|| Error::Malformed("shared message table address".into()))?;
    let n = r.u8()?;
    let mut t = c.cur(data, table)?;
    t.sig(b"SMTB", "shared message table")?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let _version = t.u8()?;
        let _index_type = t.u8()?;
        let kinds = t.u16()?;
        t.skip(4 + 2 + 2 + 2)?; // minimum size, list and B-tree cutoffs, message count
        let _index = t.addr(c)?;
        if let Some(heap) = t.addr(c)? {
            out.push(SohmIndex { kinds, heap });
        }
    }
    Ok(out)
}

impl<'a> SharedTargets<'a> {
    /// Targets for a file whose shared-message indexes are `sohm`.
    pub fn with_sohm(sohm: Vec<SohmIndex>) -> Self {
        SharedTargets {
            sohm,
            ..Default::default()
        }
    }

    /// The body of the shared `kind` message `b` (a shared-message encoding) points at,
    /// read at `depth`: in another object header, or in the shared-message heap.
    pub fn message(
        &mut self,
        data: &'a [u8],
        c: &Ctx,
        b: &'a [u8],
        depth: u32,
        kind: u16,
    ) -> Result<Option<&'a [u8]>> {
        match shared_ref(b, c)? {
            SharedRef::Header(addr) => self.find(data, c, addr, depth, kind),
            SharedRef::Heap(id) => {
                let bit = match kind {
                    // The old fill value message shares the new one's index.
                    MSG_FILL_OLD => 1u16 << MSG_FILL,
                    MSG_DATASPACE | MSG_DATATYPE | MSG_FILL | MSG_FILTERS | MSG_ATTRIBUTE => {
                        1u16 << kind
                    }
                    _ => return Err(Error::Malformed(format!("shared message kind {kind}"))),
                };
                let heap = self
                    .sohm
                    .iter()
                    .find(|i| i.kinds & bit != 0)
                    .map(|i| i.heap)
                    .ok_or_else(|| Error::Malformed("shared message without its index".into()))?;
                let h = self
                    .heaps
                    .entry(heap)
                    .or_insert_with(|| Heap::open(data, c, heap));
                match h {
                    Ok(h) => h.get(data, c, id).map(Some),
                    Err(e) => Err(e.clone()),
                }
            }
        }
    }

    /// The body of the first `kind` message in the header at `addr` read at `depth`;
    /// `Ok(None)` when it has none.
    pub fn find(
        &mut self,
        data: &'a [u8],
        c: &Ctx,
        addr: u64,
        depth: u32,
        kind: u16,
    ) -> Result<Option<&'a [u8]>> {
        if !self.map.contains_key(&(addr, depth)) {
            let h = read_header_depth(data, c, addr, depth, self).map(|h| {
                let mut first = HashMap::new();
                for m in h.msgs {
                    first.entry(m.kind).or_insert(m.data);
                }
                first
            });
            self.map.insert((addr, depth), h);
        }
        match &self.map[&(addr, depth)] {
            Ok(first) => Ok(first.get(&kind).copied()),
            Err(e) => Err(e.clone()),
        }
    }
}

/// The object header at `addr`, its shared messages resolved through `shared` (kept across
/// the headers of one file).
pub(crate) fn read_header<'a>(
    data: &'a [u8],
    c: &Ctx,
    addr: u64,
    shared: &mut SharedTargets<'a>,
) -> Result<Header<'a>> {
    read_header_depth(data, c, addr, 0, shared)
}

fn read_header_depth<'a>(
    data: &'a [u8],
    c: &Ctx,
    addr: u64,
    depth: u32,
    shared: &mut SharedTargets<'a>,
) -> Result<Header<'a>> {
    let p = c.pos(data, addr)?;
    let mut msgs = Vec::new();
    // Continuation blocks still to read: (position, length).
    let mut todo: Vec<(usize, usize)> = Vec::new();
    let v2 = data[p..].starts_with(b"OHDR");
    let mut hflags = 0u8;
    if v2 {
        let mut r = Cur::new(data, p + 4);
        let version = r.u8()?;
        if version != 2 {
            return Err(Error::Malformed(format!("object header version {version}")));
        }
        hflags = r.u8()?;
        if hflags & 0x20 != 0 {
            r.skip(16)?;
        }
        if hflags & 0x10 != 0 {
            r.skip(4)?;
        }
        let size = r.uint(1 << (hflags & 3))? as usize;
        let start = r.pos;
        r.skip(size)?;
        messages_v2(data, c, start, size, hflags, &mut msgs, &mut todo)?;
    } else {
        let mut r = Cur::new(data, p);
        let version = r.u8()?;
        if version != 1 {
            return Err(Error::Malformed(format!("object header version {version}")));
        }
        r.skip(1)?;
        let _n = r.u16()?;
        r.skip(4)?;
        let size = r.u32()? as usize;
        r.skip(4)?; // pad to 8
        let start = r.pos;
        r.skip(size)?;
        messages_v1(data, c, start, size, &mut msgs, &mut todo)?;
    }
    let mut next = 0usize;
    while let Some(&(pos, len)) = todo.get(next) {
        next += 1;
        if next > limits::MAX_CONTINUATIONS {
            return Err(Error::TooLarge);
        }
        if v2 {
            let mut r = Cur::new(data, pos);
            r.sig(b"OCHK", "object header continuation")?;
            r.skip(
                len.checked_sub(4)
                    .ok_or_else(|| Error::Malformed("continuation".into()))?,
            )?;
            // The block holds messages and ends with a 4-byte checksum.
            messages_v2(
                data,
                c,
                pos + 4,
                len.saturating_sub(8),
                hflags,
                &mut msgs,
                &mut todo,
            )?;
        } else {
            Cur::new(data, pos).skip(len)?;
            messages_v1(data, c, pos, len, &mut msgs, &mut todo)?;
        }
        if msgs.len() > limits::MAX_MESSAGES {
            return Err(Error::TooLarge);
        }
    }
    // Resolve shared messages (committed datatypes, mostly) to the message they point at.
    for m in msgs.iter_mut() {
        if m.flags & 0x02 != 0 && m.kind != MSG_CONTINUATION {
            if depth > 2 {
                return Err(Error::Malformed("shared message chain".into()));
            }
            m.data = shared
                .message(data, c, m.data, depth + 1, m.kind)?
                .ok_or_else(|| Error::Malformed("shared message target".into()))?;
            m.flags &= !0x02;
        }
    }
    Ok(Header { msgs })
}

/// Where a shared message (its version 1-3 encoding) points: an object header, or, in
/// version 3 with type 1, the 8-byte ID of the message in the shared-message heap.
fn shared_ref<'a>(b: &'a [u8], c: &Ctx) -> Result<SharedRef<'a>> {
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    let kind = r.u8()?;
    match version {
        1 => r.skip(6)?,
        2 => {}
        3 if kind == 1 => return Ok(SharedRef::Heap(r.bytes(8)?)),
        3 => {}
        _ => {
            return Err(Error::Malformed(format!(
                "shared message version {version}"
            )));
        }
    }
    r.addr(c)?
        .map(SharedRef::Header)
        .ok_or_else(|| Error::Malformed("undefined shared message address".into()))
}

fn continuation(m: &[u8], c: &Ctx, data: &[u8]) -> Result<(usize, usize)> {
    let mut r = Cur::new(m, 0);
    let addr = r
        .addr(c)?
        .ok_or_else(|| Error::Malformed("continuation".into()))?;
    let len = r.len(c)?;
    let s = c.slice(data, addr, len)?;
    let pos = c.pos(data, addr)?;
    Ok((pos, s.len()))
}

fn messages_v1<'a>(
    data: &'a [u8],
    c: &Ctx,
    start: usize,
    size: usize,
    msgs: &mut Vec<Msg<'a>>,
    todo: &mut Vec<(usize, usize)>,
) -> Result<()> {
    let block = Cur::new(data, start).bytes(size)?;
    let mut r = Cur::new(block, 0);
    while r.remaining() >= 8 {
        let kind = r.u16()?;
        let len = r.u16()? as usize;
        let flags = r.u8()?;
        r.skip(3)?;
        let body = r.bytes(len)?;
        push(data, c, kind, flags, body, msgs, todo)?;
    }
    Ok(())
}

fn messages_v2<'a>(
    data: &'a [u8],
    c: &Ctx,
    start: usize,
    size: usize,
    hflags: u8,
    msgs: &mut Vec<Msg<'a>>,
    todo: &mut Vec<(usize, usize)>,
) -> Result<()> {
    let block = Cur::new(data, start).bytes(size)?;
    let mut r = Cur::new(block, 0);
    let head = if hflags & 0x04 != 0 { 6 } else { 4 };
    while r.remaining() >= head {
        let kind = u16::from(r.u8()?);
        let len = r.u16()? as usize;
        let flags = r.u8()?;
        if hflags & 0x04 != 0 {
            r.skip(2)?;
        }
        let body = r.bytes(len)?;
        push(data, c, kind, flags, body, msgs, todo)?;
    }
    Ok(())
}

fn push<'a>(
    data: &'a [u8],
    c: &Ctx,
    kind: u16,
    flags: u8,
    body: &'a [u8],
    msgs: &mut Vec<Msg<'a>>,
    todo: &mut Vec<(usize, usize)>,
) -> Result<()> {
    if msgs.len() >= limits::MAX_MESSAGES {
        return Err(Error::TooLarge);
    }
    match kind {
        0 => {}
        MSG_CONTINUATION => todo.push(continuation(body, c, data)?),
        _ => msgs.push(Msg {
            kind,
            flags,
            data: body,
        }),
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

    /// A v1 object header at the end of `d` holding `msgs` as (kind, flags, body).
    fn v1_header(d: &mut Vec<u8>, msgs: &[(u16, u8, Vec<u8>)]) -> u64 {
        let at = d.len() as u64;
        let mut body = Vec::new();
        for (kind, flags, b) in msgs {
            body.extend_from_slice(&kind.to_le_bytes());
            body.extend_from_slice(&(b.len() as u16).to_le_bytes());
            body.extend_from_slice(&[*flags, 0, 0, 0]);
            body.extend_from_slice(b);
        }
        d.extend_from_slice(&[1, 0]);
        d.extend_from_slice(&(msgs.len() as u16).to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&(body.len() as u32).to_le_bytes());
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(&body);
        at
    }

    /// 60,000 shared datatype messages pointing at one header of 60,000 messages: each
    /// was resolved by re-reading and scanning the target (3.6·10^9 message reads); the
    /// target is now read once.
    #[test]
    fn many_shared_messages_to_one_large_header() {
        let n = 60_000;
        let mut d = Vec::new();
        let mut target: Vec<(u16, u8, Vec<u8>)> =
            (0..n).map(|_| (MSG_DATASPACE, 0, Vec::new())).collect();
        target.push((MSG_DATATYPE, 0, vec![7; 8]));
        let t = v1_header(&mut d, &target);
        let mut shared_body = vec![2, 0];
        shared_body.extend_from_slice(&t.to_le_bytes());
        let msgs: Vec<_> = (0..n)
            .map(|_| (MSG_DATATYPE, 0x02, shared_body.clone()))
            .collect();
        let h = v1_header(&mut d, &msgs);
        let start = std::time::Instant::now();
        let header = read_header(&d, &CTX, h, &mut SharedTargets::default()).unwrap();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(header.msgs.len(), n);
        assert!(header.msgs.iter().all(|m| m.data == [7; 8] && m.flags == 0));
        // A target without the kind still fails, and so does a cached failing target.
        let mut d2 = d.clone();
        let bad = v1_header(&mut d2, &[(MSG_LAYOUT, 0x02, shared_body.clone())]);
        let mut far = vec![2, 0];
        far.extend_from_slice(&(1u64 << 40).to_le_bytes());
        let worse = v1_header(&mut d2, &[(MSG_DATATYPE, 0x02, far)]);
        let mut cache = SharedTargets::default();
        assert!(read_header(&d2, &CTX, bad, &mut cache).is_err());
        assert!(read_header(&d2, &CTX, worse, &mut cache).is_err());
        assert!(read_header(&d2, &CTX, worse, &mut cache).is_err());
        assert!(read_header(&d2, &CTX, h, &mut cache).is_ok());
    }
}

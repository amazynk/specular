//! Group members: symbol tables (B-tree v1 and local heap), link messages, and dense links
//! in a fractal heap indexed by a B-tree v2.

use crate::fheap::{Heap, objects_by_index};
use crate::io::{Ctx, Cur};
use crate::ohdr::{Header, MSG_LINK, MSG_LINK_INFO, MSG_SYMBOL_TABLE};
use crate::{Error, Result, limits};

pub(crate) enum Target {
    /// Object header address, and the cached B-tree / local heap of a symbol-table group.
    Hard(u64, Option<(u64, u64)>),
    Soft(String),
    External {
        file: String,
        path: String,
    },
}

pub(crate) struct Link {
    pub name: String,
    pub target: Target,
}

/// The links of a group, sorted by name.
pub(crate) fn links(
    data: &[u8],
    c: &Ctx,
    h: &Header,
    hint: Option<(u64, u64)>,
) -> Result<Vec<Link>> {
    let mut out = Vec::new();
    if let Some(m) = h.find(MSG_SYMBOL_TABLE) {
        let mut r = Cur::new(m.data, 0);
        let btree = r.addr(c)?;
        let heap = r.addr(c)?;
        if let (Some(b), Some(hp)) = (btree, heap) {
            symbol_table(data, c, b, hp, &mut out)?;
        }
    } else if let Some((b, hp)) = hint
        && h.find(MSG_LINK).is_none()
        && h.find(MSG_LINK_INFO).is_none()
    {
        symbol_table(data, c, b, hp, &mut out)?;
    }
    for m in h.all(MSG_LINK) {
        if out.len() >= limits::MAX_LINKS {
            return Err(Error::TooLarge);
        }
        out.push(link_message(m.data, c)?);
    }
    if let Some(m) = h.find(MSG_LINK_INFO) {
        let mut r = Cur::new(m.data, 0);
        r.skip(1)?;
        let flags = r.u8()?;
        if flags & 1 != 0 {
            r.skip(8)?;
        }
        let heap = r.addr(c)?;
        let names = r.addr(c)?;
        if let (Some(heap), Some(names)) = (heap, names) {
            let heap = Heap::open(data, c, heap)?;
            // Name index records: a 4-byte hash, then the heap ID.
            for obj in objects_by_index(data, c, &heap, names, 4)? {
                out.push(link_message(obj, c)?);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    Ok(out)
}

fn link_message(b: &[u8], c: &Ctx) -> Result<Link> {
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    if version != 1 {
        return Err(Error::Malformed(format!("link message version {version}")));
    }
    let flags = r.u8()?;
    let kind = if flags & 0x08 != 0 { r.u8()? } else { 0 };
    if flags & 0x04 != 0 {
        r.skip(8)?;
    }
    if flags & 0x10 != 0 {
        r.skip(1)?;
    }
    let n = r.uint(1 << (flags & 3))? as usize;
    let name = String::from_utf8_lossy(r.bytes(n)?).into_owned();
    if name.is_empty() || name.contains('/') {
        return Err(Error::Malformed("link name".into()));
    }
    let target = match kind {
        0 => Target::Hard(
            r.addr(c)?
                .ok_or_else(|| Error::Malformed("undefined link address".into()))?,
            None,
        ),
        1 => {
            let n = r.u16()? as usize;
            Target::Soft(String::from_utf8_lossy(r.bytes(n)?).into_owned())
        }
        _ => {
            let n = r.u16()? as usize;
            let v = r.bytes(n)?;
            let text: Vec<String> = v
                .get(1..)
                .unwrap_or_default()
                .split(|&x| x == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect();
            let mut text = text.into_iter();
            Target::External {
                file: text.next().unwrap_or_default(),
                path: text.next().unwrap_or_default(),
            }
        }
    };
    Ok(Link { name, target })
}

/// Calls `f(key, child)` for every child pointer in the leaves (level 0) of a version 1
/// B-tree, with the key that precedes the child.
pub(crate) fn btree1_leaves(
    data: &[u8],
    c: &Ctx,
    addr: u64,
    want_type: u8,
    key_size: usize,
    f: &mut dyn FnMut(&[u8], u64) -> Result<()>,
) -> Result<()> {
    let mut stack = vec![(addr, None::<u8>)];
    let mut nodes = 0usize;
    let mut seen = std::collections::HashSet::new();
    while let Some((addr, expect_level)) = stack.pop() {
        nodes += 1;
        if nodes > limits::MAX_NODES || !seen.insert(addr) {
            return Err(Error::Malformed("B-tree v1 too large or cyclic".into()));
        }
        let mut r = c.cur(data, addr)?;
        r.sig(b"TREE", "B-tree v1 node")?;
        let kind = r.u8()?;
        let level = r.u8()?;
        if kind != want_type || expect_level.is_some_and(|l| l != level) {
            return Err(Error::Malformed("B-tree v1 node type or level".into()));
        }
        let entries = r.u16()? as usize;
        r.addr(c)?;
        r.addr(c)?;
        let mut children = Vec::with_capacity(entries);
        for _ in 0..entries {
            let key = r.bytes(key_size)?;
            let child = r.addr(c)?;
            if let Some(child) = child {
                children.push((key, child));
            }
        }
        if level == 0 {
            for (key, child) in children {
                f(key, child)?;
            }
        } else {
            for &(_, child) in children.iter().rev() {
                stack.push((child, Some(level - 1)));
            }
        }
    }
    Ok(())
}

fn symbol_table(data: &[u8], c: &Ctx, btree: u64, heap: u64, out: &mut Vec<Link>) -> Result<()> {
    let mut r = c.cur(data, heap)?;
    r.sig(b"HEAP", "local heap")?;
    r.skip(4)?;
    let size = r.len(c)?;
    r.len(c)?;
    let seg_addr = r
        .addr(c)?
        .ok_or_else(|| Error::Malformed("local heap data".into()))?;
    let seg = c.slice(data, seg_addr, size)?;
    let sl = c.sl as usize;
    btree1_leaves(data, c, btree, 0, sl, &mut |_, snod| {
        let mut r = c.cur(data, snod)?;
        r.sig(b"SNOD", "symbol table node")?;
        r.skip(2)?;
        let n = r.u16()?;
        for _ in 0..n {
            let name_off = r.len(c)? as usize;
            let obj = r.addr(c)?;
            let cache = r.u32()?;
            r.skip(4)?;
            let scratch = r.bytes(16)?;
            let name = seg
                .get(name_off..)
                .map(|s| {
                    let s = &s[..s.len().min(limits::MAX_NAME_BYTES)];
                    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
                    String::from_utf8_lossy(&s[..end]).into_owned()
                })
                .ok_or_else(|| Error::Malformed("symbol name offset".into()))?;
            let hint = if cache == 1 {
                let mut s = Cur::new(scratch, 0);
                match (s.addr(c)?, s.addr(c)?) {
                    (Some(b), Some(h)) => Some((b, h)),
                    _ => None,
                }
            } else {
                None
            };
            if out.len() >= limits::MAX_LINKS {
                return Err(Error::TooLarge);
            }
            // A soft link: the scratch pad holds the offset of its value in the local heap.
            if cache == 2 && !name.is_empty() {
                let off = u32::from_le_bytes([scratch[0], scratch[1], scratch[2], scratch[3]]);
                if let Some(v) = seg.get(off as usize..) {
                    let end = v.iter().position(|&b| b == 0).unwrap_or(v.len());
                    out.push(Link {
                        name,
                        target: Target::Soft(String::from_utf8_lossy(&v[..end]).into_owned()),
                    });
                }
                continue;
            }
            if let (Some(obj), false) = (obj, name.is_empty()) {
                out.push(Link {
                    name,
                    target: Target::Hard(obj, hint),
                });
            }
        }
        Ok(())
    })
}

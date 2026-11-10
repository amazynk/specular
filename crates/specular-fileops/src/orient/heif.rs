//! HEIF and AVIF: the primary item's transformative properties `irot` (anticlockwise quarter
//! turns) and `imir` (axis 0 exchanges top and bottom, axis 1 left and right, as libheif,
//! libavif and our decoders read it).
//!
//! The primary's existing `irot`/`imir` associations are replaced by new ones (appended to
//! `ipco`, marked essential) giving the composed orientation; thumbnails of the primary get
//! the same treatment. `meta` grows by a few bytes, so file offsets in `iloc` that point past
//! it are moved by the same amount. An Exif item's Orientation is patched in place to match
//! (readers ignore it in HEIF, but other tools show it).

use specular_jpeg::Transform;

/// A box: type, start of the whole box, start of its payload, end.
#[derive(Clone, Copy, Debug)]
struct Bx {
    ty: [u8; 4],
    at: usize,
    body: usize,
    end: usize,
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}
fn be64(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

fn boxes(d: &[u8], from: usize, to: usize) -> Option<Vec<Bx>> {
    let mut out = Vec::new();
    let mut i = from;
    while i + 8 <= to {
        let size = be32(d, i)? as u64;
        let ty: [u8; 4] = d[i + 4..i + 8].try_into().ok()?;
        let (len, head) = match size {
            0 => ((to - i) as u64, 8),
            1 => (be64(d, i + 8)?, 16),
            n => (n, 8),
        };
        let end = i.checked_add(usize::try_from(len).ok()?)?;
        if len < head as u64 || end > to {
            return None;
        }
        out.push(Bx {
            ty,
            at: i,
            body: i + head,
            end,
        });
        i = end;
    }
    Some(out)
}

fn find<'a>(bs: &'a [Bx], ty: &[u8; 4]) -> Option<&'a Bx> {
    bs.iter().find(|b| &b.ty == ty)
}

/// One `ipma` entry.
#[derive(Clone, Debug)]
struct Assoc {
    item: u32,
    /// (essential, 1-based property index)
    props: Vec<(bool, u16)>,
}

struct Ipma {
    version: u8,
    flags: u32,
    entries: Vec<Assoc>,
}

fn parse_ipma(d: &[u8], b: &Bx) -> Option<Ipma> {
    let version = *d.get(b.body)?;
    let flags = be32(d, b.body)? & 0xFF_FFFF;
    let n = be32(d, b.body + 4)? as usize;
    let mut i = b.body + 8;
    let mut entries = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let item = if version < 1 {
            let v = u32::from(be16(d, i)?);
            i += 2;
            v
        } else {
            let v = be32(d, i)?;
            i += 4;
            v
        };
        let k = *d.get(i)? as usize;
        i += 1;
        let mut props = Vec::with_capacity(k);
        for _ in 0..k {
            if flags & 1 != 0 {
                let v = be16(d, i)?;
                i += 2;
                props.push((v & 0x8000 != 0, v & 0x7FFF));
            } else {
                let v = *d.get(i)?;
                i += 1;
                props.push((v & 0x80 != 0, u16::from(v & 0x7F)));
            }
        }
        entries.push(Assoc { item, props });
    }
    (i <= b.end).then_some(Ipma {
        version,
        flags,
        entries,
    })
}

fn write_ipma(m: &Ipma) -> Vec<u8> {
    let wide = m.flags & 1 != 0
        || m.entries
            .iter()
            .any(|e| e.props.iter().any(|&(_, i)| i > 0x7F));
    let version = if m.entries.iter().any(|e| e.item > 0xFFFF) {
        m.version.max(1)
    } else {
        m.version
    };
    let flags = (m.flags & !1) | u32::from(wide);
    let mut body = Vec::new();
    body.extend_from_slice(&((u32::from(version) << 24) | flags).to_be_bytes());
    body.extend_from_slice(&(m.entries.len() as u32).to_be_bytes());
    for e in &m.entries {
        if version < 1 {
            body.extend_from_slice(&(e.item as u16).to_be_bytes());
        } else {
            body.extend_from_slice(&e.item.to_be_bytes());
        }
        body.push(e.props.len() as u8);
        for &(ess, i) in &e.props {
            if wide {
                body.extend_from_slice(&((u16::from(ess) << 15) | i).to_be_bytes());
            } else {
                body.push((u8::from(ess) << 7) | i as u8);
            }
        }
    }
    make_box(b"ipma", &body)
}

fn make_box(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 8);
    v.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    v.extend_from_slice(ty);
    v.extend_from_slice(body);
    v
}

fn irot_t(a: u8) -> Transform {
    // Anticlockwise quarter turns.
    let mut t = Transform::IDENTITY;
    for _ in 0..(a & 3) {
        t = t.then(Transform::ROTATE_270);
    }
    t
}

fn imir_t(axis: u8) -> Transform {
    if axis & 1 == 1 {
        Transform::FLIP_H
    } else {
        Transform::FLIP_V
    }
}

/// `irot` angle and optional `imir` axis giving `t` (irot applied first).
fn decompose(t: Transform) -> (u8, Option<u8>) {
    for a in 0..4u8 {
        if irot_t(a) == t {
            return (a, None);
        }
    }
    for a in 0..4u8 {
        if irot_t(a).then(imir_t(1)) == t {
            return (a, Some(1));
        }
    }
    (0, None)
}

struct Meta {
    meta: Bx,
    children: Vec<Bx>,
    ipco: Bx,
    props: Vec<Bx>,
    ipma: Vec<Bx>,
    primary: u32,
}

fn parse_meta(d: &[u8]) -> Result<Meta, &'static str> {
    let top = boxes(d, 0, d.len()).ok_or("the file's boxes don't parse")?;
    if find(&top, b"moov").is_some() {
        return Err("image sequences are not rotated");
    }
    let meta = *find(&top, b"meta").ok_or("no meta box")?;
    let children = boxes(d, meta.body + 4, meta.end).ok_or("the meta box doesn't parse")?;
    let pitm = find(&children, b"pitm").ok_or("no primary item")?;
    let primary = if d[pitm.body] == 0 {
        u32::from(be16(d, pitm.body + 4).ok_or("bad pitm")?)
    } else {
        be32(d, pitm.body + 4).ok_or("bad pitm")?
    };
    let iprp = *find(&children, b"iprp").ok_or("no item properties")?;
    let ip = boxes(d, iprp.body, iprp.end).ok_or("the iprp box doesn't parse")?;
    let ipco = *find(&ip, b"ipco").ok_or("no ipco")?;
    let props = boxes(d, ipco.body, ipco.end).ok_or("the ipco box doesn't parse")?;
    let ipma: Vec<Bx> = ip.iter().filter(|b| &b.ty == b"ipma").copied().collect();
    if ipma.is_empty() {
        return Err("no ipma");
    }
    Ok(Meta {
        meta,
        children,
        ipco,
        props,
        ipma,
        primary,
    })
}

/// Items whose `thmb` reference points at the primary.
fn thumbnails(d: &[u8], m: &Meta) -> Vec<u32> {
    let Some(iref) = find(&m.children, b"iref") else {
        return Vec::new();
    };
    let v = d[iref.body];
    let Some(refs) = boxes(d, iref.body + 4, iref.end) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for r in refs.iter().filter(|r| &r.ty == b"thmb") {
        let w = if v == 0 { 2 } else { 4 };
        let rd = |o: usize| -> Option<u32> {
            if w == 2 {
                be16(d, o).map(u32::from)
            } else {
                be32(d, o)
            }
        };
        let (Some(from), Some(n)) = (rd(r.body), be16(d, r.body + w)) else {
            continue;
        };
        for k in 0..n as usize {
            if rd(r.body + w + 2 + k * w) == Some(m.primary) {
                out.push(from);
            }
        }
    }
    out
}

/// The 1-based index of a property already in `ipco` that is exactly `ty` with `value`
/// (repeated rotations reuse them instead of growing the file).
fn existing(d: &[u8], props: &[Bx], ty: &[u8; 4], value: u8) -> Option<u16> {
    props
        .iter()
        .position(|p| &p.ty == ty && p.end - p.body == 1 && d[p.body] == value)
        .and_then(|i| u16::try_from(i + 1).ok())
}

/// The current orientation (Exif numbering) of the primary item.
pub fn orientation(d: &[u8]) -> Option<u8> {
    let m = parse_meta(d).ok()?;
    let mut t = Transform::IDENTITY;
    for b in &m.ipma {
        let ipma = parse_ipma(d, b)?;
        for e in ipma.entries.iter().filter(|e| e.item == m.primary) {
            for &(_, i) in &e.props {
                let p = m.props.get(usize::from(i).checked_sub(1)?)?;
                match &p.ty {
                    b"irot" => t = t.then(irot_t(*d.get(p.body)?)),
                    b"imir" => t = t.then(imir_t(*d.get(p.body)?)),
                    _ => {}
                }
            }
        }
    }
    Some(t.to_exif())
}

/// The file with the primary item (and its thumbnails) shown with `user` applied after the
/// orientation it has now. Returns the new bytes and the new orientation.
pub fn rotate(d: &[u8], user: Transform) -> Result<(Vec<u8>, u8), &'static str> {
    let m = parse_meta(d)?;
    let mut targets = vec![m.primary];
    targets.extend(thumbnails(d, &m));
    let mut ipmas: Vec<Ipma> = m
        .ipma
        .iter()
        .map(|b| parse_ipma(d, b).ok_or("the ipma box doesn't parse"))
        .collect::<Result<_, _>>()?;
    let mut new_props: Vec<Vec<u8>> = Vec::new();
    let mut prop_index: std::collections::HashMap<(u8, u8), u16> = Default::default();
    let base = m.props.len();
    let mut primary_after = Transform::IDENTITY;
    for &item in &targets {
        // The item's current transform, in association order.
        let mut cur = Transform::IDENTITY;
        for e in ipmas
            .iter()
            .flat_map(|x| &x.entries)
            .filter(|e| e.item == item)
        {
            for &(_, i) in &e.props {
                let Some(p) = m.props.get(usize::from(i).wrapping_sub(1)) else {
                    continue;
                };
                match &p.ty {
                    b"irot" => cur = cur.then(irot_t(d[p.body])),
                    b"imir" => cur = cur.then(imir_t(d[p.body])),
                    _ => {}
                }
            }
        }
        let next = cur.then(user);
        if item == m.primary {
            primary_after = next;
        }
        let (angle, mirror) = decompose(next);
        let mut add: Vec<u16> = Vec::new();
        if angle != 0 {
            let idx = *prop_index.entry((b'r', angle)).or_insert_with(|| {
                existing(d, &m.props, b"irot", angle).unwrap_or_else(|| {
                    new_props.push(make_box(b"irot", &[angle]));
                    (base + new_props.len()) as u16
                })
            });
            add.push(idx);
        }
        if let Some(axis) = mirror {
            let idx = *prop_index.entry((b'm', axis)).or_insert_with(|| {
                existing(d, &m.props, b"imir", axis).unwrap_or_else(|| {
                    new_props.push(make_box(b"imir", &[axis]));
                    (base + new_props.len()) as u16
                })
            });
            add.push(idx);
        }
        // Drop the old transform associations; the new ones go at the end of the item's
        // first entry (transformative properties come after descriptive ones).
        let mut placed = false;
        for x in &mut ipmas {
            for e in x.entries.iter_mut().filter(|e| e.item == item) {
                e.props.retain(|&(_, i)| {
                    m.props
                        .get(usize::from(i).wrapping_sub(1))
                        .is_none_or(|p| &p.ty != b"irot" && &p.ty != b"imir")
                });
                if !placed {
                    if e.props.len() + add.len() > 255 {
                        return Err("too many properties");
                    }
                    e.props.extend(add.iter().map(|&i| (true, i)));
                    placed = true;
                }
            }
        }
        if !placed && !add.is_empty() {
            ipmas[0].entries.push(Assoc {
                item,
                props: add.iter().map(|&i| (true, i)).collect(),
            });
        }
    }
    if base + new_props.len() > 0x7FFF {
        return Err("too many properties");
    }
    // Rebuild ipco, iprp and meta around the new pieces.
    let mut ipco_body = d[m.ipco.body..m.ipco.end].to_vec();
    for p in &new_props {
        ipco_body.extend_from_slice(p);
    }
    let iprp = find(&m.children, b"iprp").ok_or("no item properties")?;
    let mut iprp_body = Vec::new();
    let mut ipma_written = false;
    for c in boxes(d, iprp.body, iprp.end).ok_or("bad iprp")? {
        match &c.ty {
            b"ipco" => iprp_body.extend_from_slice(&make_box(b"ipco", &ipco_body)),
            b"ipma" => {
                if !ipma_written {
                    for x in &ipmas {
                        iprp_body.extend_from_slice(&write_ipma(x));
                    }
                    ipma_written = true;
                }
            }
            _ => iprp_body.extend_from_slice(&d[c.at..c.end]),
        }
    }
    let mut meta_body = d[m.meta.body..m.meta.body + 4].to_vec();
    let mut iloc_at = None;
    for c in &m.children {
        if &c.ty == b"iprp" {
            meta_body.extend_from_slice(&make_box(b"iprp", &iprp_body));
        } else {
            if &c.ty == b"iloc" {
                iloc_at = Some((8 + meta_body.len(), *c));
            }
            meta_body.extend_from_slice(&d[c.at..c.end]);
        }
    }
    let new_meta = make_box(b"meta", &meta_body);
    let old_len = m.meta.end - m.meta.at;
    let delta = new_meta.len() as i64 - old_len as i64;
    let mut out = d[..m.meta.at].to_vec();
    let meta_at = out.len();
    out.extend_from_slice(&new_meta);
    out.extend_from_slice(&d[m.meta.end..]);
    if let Some((rel, c)) = iloc_at {
        let at = meta_at + rel;
        let len = c.end - c.at;
        shift_iloc(&mut out, at, at + len, m.meta.end, delta)?;
    }
    let o = primary_after.to_exif();
    patch_exif_item(&mut out, o);
    Ok((out, o))
}

/// Parsed `iloc` field widths.
struct IlocHead {
    version: u8,
    off: usize,
    len: usize,
    base: usize,
    idx: usize,
}

fn read_n(d: &[u8], o: usize, n: usize) -> Option<u64> {
    match n {
        0 => Some(0),
        4 => be32(d, o).map(u64::from),
        8 => be64(d, o),
        _ => None,
    }
}

fn write_n(d: &mut [u8], o: usize, n: usize, v: u64) -> Option<()> {
    match n {
        4 => d
            .get_mut(o..o + 4)?
            .copy_from_slice(&u32::try_from(v).ok()?.to_be_bytes()),
        8 => d.get_mut(o..o + 8)?.copy_from_slice(&v.to_be_bytes()),
        _ => return None,
    }
    Some(())
}

/// An item's location: id, construction method, where its base_offset field is, and where
/// each extent's offset field is.
type IlocItem = (u32, u16, usize, Vec<usize>);

/// Visit every item's location: (item id, construction method, offset of base_offset field,
/// list of extent offset field positions).
fn iloc_items(d: &[u8], at: usize, end: usize) -> Option<(IlocHead, Vec<IlocItem>)> {
    let body = at + 8;
    let version = *d.get(body)?;
    let a = *d.get(body + 4)?;
    let b = *d.get(body + 5)?;
    let h = IlocHead {
        version,
        off: usize::from(a >> 4),
        len: usize::from(a & 15),
        base: usize::from(b >> 4),
        idx: if version >= 1 { usize::from(b & 15) } else { 0 },
    };
    let mut i = body + 6;
    let count = if version < 2 {
        let v = u32::from(be16(d, i)?);
        i += 2;
        v
    } else {
        let v = be32(d, i)?;
        i += 4;
        v
    };
    let mut items = Vec::new();
    for _ in 0..count {
        let id = if version < 2 {
            let v = u32::from(be16(d, i)?);
            i += 2;
            v
        } else {
            let v = be32(d, i)?;
            i += 4;
            v
        };
        let method = if version >= 1 {
            let v = be16(d, i)? & 15;
            i += 2;
            v
        } else {
            0
        };
        i += 2; // data_reference_index
        let base_at = i;
        i += h.base;
        let n = be16(d, i)? as usize;
        i += 2;
        let mut ext = Vec::with_capacity(n.min(1024));
        for _ in 0..n {
            i += h.idx;
            ext.push(i);
            i += h.off + h.len;
        }
        if i > end {
            return None;
        }
        items.push((id, method, base_at, ext));
    }
    Some((h, items))
}

/// Add `delta` to every file offset (construction method 0) in the `iloc` at `at..end` that
/// points at or past `old_meta_end`.
fn shift_iloc(
    d: &mut [u8],
    at: usize,
    end: usize,
    old_meta_end: usize,
    delta: i64,
) -> Result<(), &'static str> {
    if delta == 0 {
        return Ok(());
    }
    let (h, items) = iloc_items(d, at, end).ok_or("the iloc box doesn't parse")?;
    let _ = h.version;
    let lim = old_meta_end as u64;
    let add = |v: u64| -> Option<u64> { u64::try_from(v as i64 + delta).ok() };
    for (_, method, base_at, ext) in items {
        if method != 0 {
            continue;
        }
        let base = read_n(d, base_at, h.base).ok_or("bad iloc")?;
        if h.base > 0 && base >= lim {
            write_n(d, base_at, h.base, add(base).ok_or("bad iloc")?)
                .ok_or("an offset no longer fits")?;
            continue;
        }
        for e in ext {
            let off = read_n(d, e, h.off).ok_or("bad iloc")?;
            if base + off >= lim {
                if h.off == 0 {
                    return Err("an item has no offset to move");
                }
                write_n(d, e, h.off, add(off).ok_or("bad iloc")?)
                    .ok_or("an offset no longer fits")?;
            }
        }
    }
    Ok(())
}

/// Set the Orientation of the Exif item, in place, when it has one.
fn patch_exif_item(d: &mut [u8], o: u8) {
    let Some((at, len)) = exif_item(d) else {
        return;
    };
    let Some(tiff_rel) = be32(d, at).map(|v| v as usize) else {
        return;
    };
    let start = at + 4 + tiff_rel;
    if start >= at + len {
        return;
    }
    let block = &mut d[start..at + len];
    let Some(t) = super::tiff::Tiff::detect(block) else {
        return;
    };
    let Some(ifd) = t.ifd0(block) else { return };
    if let Some(e) = t.find(block, ifd, super::tiff::ORIENTATION) {
        t.set_scalar(block, e, u64::from(o));
    }
}

/// The Exif item's bytes (one extent, in the file) as (offset, length).
fn exif_item(d: &[u8]) -> Option<(usize, usize)> {
    let m = parse_meta(d).ok()?;
    let iinf = find(&m.children, b"iinf")?;
    let v = d[iinf.body];
    let first = iinf.body + 4 + if v == 0 { 2 } else { 4 };
    let mut id = None;
    for e in boxes(d, first, iinf.end)?
        .iter()
        .filter(|b| &b.ty == b"infe")
    {
        let ver = d[e.body];
        if ver < 2 {
            continue;
        }
        let (iid, ty_at) = if ver == 2 {
            (u32::from(be16(d, e.body + 4)?), e.body + 8)
        } else {
            (be32(d, e.body + 4)?, e.body + 10)
        };
        if d.get(ty_at..ty_at + 4)? == b"Exif" {
            id = Some(iid);
        }
    }
    let id = id?;
    let iloc = find(&m.children, b"iloc")?;
    let (h, items) = iloc_items(d, iloc.at, iloc.end)?;
    let (_, method, base_at, ext) = items.into_iter().find(|x| x.0 == id)?;
    if method != 0 || ext.len() != 1 {
        return None;
    }
    let base = read_n(d, base_at, h.base)?;
    let off = read_n(d, ext[0], h.off)?;
    let len = read_n(d, ext[0] + h.off, h.len)?;
    let at = usize::try_from(base + off).ok()?;
    let len = usize::try_from(len).ok()?;
    d.get(at..at.checked_add(len)?)?;
    Some((at, len))
}

/// The Exif item's Orientation (for tests and the info panel).
pub fn exif_item_orientation(d: &[u8]) -> Option<u8> {
    let (at, len) = exif_item(d)?;
    let rel = be32(d, at)? as usize;
    super::tiff::orientation(d.get(at + 4 + rel..at + len)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposition_covers_the_group() {
        for o in 1..=8 {
            let t = Transform::from_exif(o);
            let (a, m) = decompose(t);
            let mut back = irot_t(a);
            if let Some(x) = m {
                back = back.then(imir_t(x));
            }
            assert_eq!(back, t, "orientation {o}");
        }
        // EXIF 6 (90° clockwise) is irot 3 (270° anticlockwise).
        assert_eq!(decompose(Transform::from_exif(6)), (3, None));
        assert_eq!(decompose(Transform::from_exif(8)), (1, None));
    }
}

//! What ravif's container leaves out: an ICC profile (a `colr` property of type `prof` on the
//! primary item) and XMP (a `mime` item of type `application/rdf+xml` that describes the
//! primary item). Neither ravif 0.13 nor avif-serialize 0.8 can write them, so the file ravif
//! made is taken apart here and written again with them: its boxes and item data are kept as
//! they are, and only `iinf`, `iref`, `iprp` and `iloc` are rebuilt.

/// One box: its type, and the whole box (header included) and its payload.
struct Bx<'a> {
    typ: [u8; 4],
    whole: &'a [u8],
    payload: &'a [u8],
}

fn boxes(mut d: &[u8]) -> Result<Vec<Bx<'_>>, String> {
    let mut out = Vec::new();
    while !d.is_empty() {
        let bad = || "AVIF: bad box".to_owned();
        let head = d.get(..8).ok_or_else(bad)?;
        let typ: [u8; 4] = head[4..8].try_into().map_err(|_| bad())?;
        let (hlen, len) = match u32::from_be_bytes(head[..4].try_into().map_err(|_| bad())?) {
            0 => (8, d.len()),
            1 => {
                let l = d.get(8..16).ok_or_else(bad)?;
                let l = u64::from_be_bytes(l.try_into().map_err(|_| bad())?);
                (16, usize::try_from(l).map_err(|_| bad())?)
            }
            l => (8, l as usize),
        };
        if len < hlen || len > d.len() {
            return Err(bad());
        }
        out.push(Bx {
            typ,
            whole: &d[..len],
            payload: &d[hlen..len],
        });
        d = &d[len..];
    }
    Ok(out)
}

/// A big-endian reader over a box payload.
struct Rd<'a>(&'a [u8]);

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if n > self.0.len() {
            return Err("AVIF: short box".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    /// An unsigned big-endian number of `n` (0–8) bytes.
    fn uint(&mut self, n: usize) -> Result<u64, String> {
        Ok(self
            .take(n)?
            .iter()
            .fold(0u64, |v, &b| (v << 8) | u64::from(b)))
    }
}

fn put_box(out: &mut Vec<u8>, typ: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(typ);
    out.extend_from_slice(payload);
}

/// Version 0, no flags.
fn full_box(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(payload.len() + 12);
    b.extend_from_slice(&((payload.len() + 12) as u32).to_be_bytes());
    b.extend_from_slice(typ);
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(payload);
    b
}

/// One item's location: its id and its `(offset, length)` extents.
type Location = (u32, Vec<(u64, u64)>);

/// Item locations as `(id, extents)`, in `iloc` order (file offsets, construction method 0).
fn parse_iloc(p: &[u8]) -> Result<Vec<Location>, String> {
    let mut r = Rd(p);
    let version = r.uint(1)?;
    r.take(3)?;
    let sizes = r.uint(2)?;
    let (off, len, base) = (
        (sizes >> 12) as usize,
        ((sizes >> 8) & 15) as usize,
        ((sizes >> 4) & 15) as usize,
    );
    let index = if version >= 1 {
        (sizes & 15) as usize
    } else {
        0
    };
    let count = r.uint(if version < 2 { 2 } else { 4 })?;
    let mut out = Vec::new();
    for _ in 0..count {
        let id = r.uint(if version < 2 { 2 } else { 4 })? as u32;
        if version >= 1 && r.uint(2)? & 15 != 0 {
            return Err("AVIF: item not in the file".into());
        }
        if r.uint(2)? != 0 {
            return Err("AVIF: item in another file".into());
        }
        let b = r.uint(base)?;
        let n = r.uint(2)?;
        let mut ext = Vec::new();
        for _ in 0..n {
            r.uint(index)?;
            let o = r.uint(off)?;
            let l = r.uint(len)?;
            ext.push((b + o, l));
        }
        out.push((id, ext));
    }
    Ok(out)
}

/// `ipma` as `(item, [(essential, property index)])`.
type Assoc = Vec<(u32, Vec<(bool, u16)>)>;

fn parse_ipma(p: &[u8]) -> Result<Assoc, String> {
    let mut r = Rd(p);
    let version = r.uint(1)?;
    let flags = r.uint(3)?;
    let count = r.uint(4)?;
    let mut out = Vec::new();
    for _ in 0..count {
        let id = r.uint(if version < 1 { 2 } else { 4 })? as u32;
        let n = r.uint(1)?;
        let mut a = Vec::new();
        for _ in 0..n {
            let v = if flags & 1 != 0 {
                let v = r.uint(2)?;
                (v & 0x8000 != 0, (v & 0x7fff) as u16)
            } else {
                let v = r.uint(1)?;
                (v & 0x80 != 0, (v & 0x7f) as u16)
            };
            a.push(v);
        }
        out.push((id, a));
    }
    Ok(out)
}

fn write_ipma(entries: &Assoc) -> Result<Vec<u8>, String> {
    let wide = entries.iter().flat_map(|e| &e.1).any(|&(_, i)| i > 127);
    let ids16 = entries.iter().all(|e| e.0 <= 0xffff);
    let mut p = vec![u8::from(!ids16), 0, 0, u8::from(wide)];
    p.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for (id, a) in entries {
        if ids16 {
            p.extend_from_slice(&(*id as u16).to_be_bytes());
        } else {
            p.extend_from_slice(&id.to_be_bytes());
        }
        p.push(u8::try_from(a.len()).map_err(|_| "AVIF: too many properties".to_owned())?);
        for &(ess, i) in a {
            if wide {
                p.extend_from_slice(&((u16::from(ess) << 15) | i).to_be_bytes());
            } else {
                p.push((u8::from(ess) << 7) | i as u8);
            }
        }
    }
    let mut b = Vec::new();
    put_box(&mut b, b"ipma", &p);
    Ok(b)
}

/// `avif` (as ravif wrote it) with `icc` on the primary item and `xmp` as an item describing
/// it; unchanged when there are neither.
pub fn add_icc_xmp(avif: &[u8], icc: Option<&[u8]>, xmp: Option<&[u8]>) -> Result<Vec<u8>, String> {
    if icc.is_none() && xmp.is_none() {
        return Ok(avif.to_vec());
    }
    let top = boxes(avif)?;
    let ftyp = top
        .iter()
        .find(|b| &b.typ == b"ftyp")
        .ok_or("AVIF: no ftyp")?;
    let meta = top
        .iter()
        .find(|b| &b.typ == b"meta")
        .ok_or("AVIF: no meta")?;
    let children = boxes(meta.payload.get(4..).ok_or("AVIF: short meta")?)?;
    let child = |t: &[u8; 4]| children.iter().find(|b| &b.typ == t);

    let pitm = child(b"pitm").ok_or("AVIF: no pitm")?;
    let primary = {
        let mut r = Rd(pitm.payload);
        let v = r.uint(1)?;
        r.take(3)?;
        r.uint(if v == 0 { 2 } else { 4 })? as u32
    };
    // Item data, read out of the file and stored again in the same order.
    let iloc = parse_iloc(child(b"iloc").ok_or("AVIF: no iloc")?.payload)?;
    let mut items: Vec<(u32, Vec<u8>)> = Vec::with_capacity(iloc.len() + 1);
    for (id, ext) in &iloc {
        let mut data = Vec::new();
        for &(o, l) in ext {
            let (o, l) = (o as usize, l as usize);
            let end = o.checked_add(l).ok_or("AVIF: bad extent")?;
            data.extend_from_slice(avif.get(o..end).ok_or("AVIF: extent outside the file")?);
        }
        items.push((*id, data));
    }
    let xmp_id = items.iter().map(|i| i.0).max().unwrap_or(0) + 1;
    if xmp_id > 0xffff {
        return Err("AVIF: too many items".into());
    }

    // iinf: the item infos as they are, then the XMP item's.
    let iinf = child(b"iinf").ok_or("AVIF: no iinf")?;
    let mut r = Rd(iinf.payload);
    let v = r.uint(1)?;
    r.take(3)?;
    r.uint(if v == 0 { 2 } else { 4 })?;
    // In item id order (ravif's aren't, which some readers warn about).
    let mut infes: Vec<(u32, Vec<u8>)> = Vec::new();
    for b in boxes(r.0)? {
        let mut r = Rd(b.payload);
        let v = r.uint(1)?;
        r.take(3)?;
        let id = r.uint(if v < 3 { 2 } else { 4 })? as u32;
        infes.push((id, b.whole.to_vec()));
    }
    infes.sort_by_key(|i| i.0);
    let mut infes: Vec<Vec<u8>> = infes.into_iter().map(|i| i.1).collect();
    if let Some(x) = xmp {
        let mut p = vec![2, 0, 0, 0];
        p.extend_from_slice(&(xmp_id as u16).to_be_bytes());
        p.extend_from_slice(&[0, 0]);
        p.extend_from_slice(b"mime");
        p.push(0);
        p.extend_from_slice(b"application/rdf+xml\0");
        let mut b = Vec::new();
        put_box(&mut b, b"infe", &p);
        infes.push(b);
        items.push((xmp_id, x.to_vec()));
    }
    let mut p = (infes.len() as u32).to_be_bytes().to_vec();
    infes.iter().for_each(|b| p.extend_from_slice(b));
    let mut new_iinf = Vec::new();
    put_box(&mut new_iinf, b"iinf", &[&[1, 0, 0, 0][..], &p].concat());

    // iref: as it is, plus "the XMP describes the primary item".
    let new_iref = xmp.map(|_| -> Result<Vec<u8>, String> {
        let mut refs: Vec<u8> = Vec::new();
        let version = match child(b"iref") {
            Some(b) => {
                let v = *b.payload.first().ok_or("AVIF: short iref")?;
                refs.extend_from_slice(b.payload.get(4..).ok_or("AVIF: short iref")?);
                v
            }
            None => 0,
        };
        let mut p = Vec::new();
        let id = |p: &mut Vec<u8>, i: u32| {
            if version == 0 {
                p.extend_from_slice(&(i as u16).to_be_bytes());
            } else {
                p.extend_from_slice(&i.to_be_bytes());
            }
        };
        id(&mut p, xmp_id);
        p.extend_from_slice(&1u16.to_be_bytes());
        id(&mut p, primary);
        put_box(&mut refs, b"cdsc", &p);
        let mut b = Vec::new();
        put_box(&mut b, b"iref", &[&[version, 0, 0, 0][..], &refs].concat());
        Ok(b)
    });
    let new_iref = new_iref.transpose()?;

    // iprp: the properties as they are, plus the profile for the primary item.
    let iprp = child(b"iprp").ok_or("AVIF: no iprp")?;
    let iprp_children = boxes(iprp.payload)?;
    let ipco = iprp_children
        .iter()
        .find(|b| &b.typ == b"ipco")
        .ok_or("AVIF: no ipco")?;
    let mut props: Vec<u8> = ipco.payload.to_vec();
    let mut assoc = parse_ipma(
        iprp_children
            .iter()
            .find(|b| &b.typ == b"ipma")
            .ok_or("AVIF: no ipma")?
            .payload,
    )?;
    if let Some(icc) = icc {
        put_box(&mut props, b"colr", &[&b"prof"[..], icc].concat());
        let index = u16::try_from(boxes(ipco.payload)?.len() + 1)
            .ok()
            .filter(|&i| i <= 0x7fff)
            .ok_or("AVIF: too many properties")?;
        match assoc.iter_mut().find(|e| e.0 == primary) {
            Some(e) => e.1.push((false, index)),
            None => assoc.push((primary, vec![(false, index)])),
        }
    }
    let mut iprp_p = Vec::new();
    put_box(&mut iprp_p, b"ipco", &props);
    iprp_p.extend_from_slice(&write_ipma(&assoc)?);
    let mut new_iprp = Vec::new();
    put_box(&mut new_iprp, b"iprp", &iprp_p);

    // iloc: one extent per item, 32-bit offsets into the new mdat.
    let iloc_for = |base: usize| -> Result<Vec<u8>, String> {
        let mut p = vec![0x44, 0x00];
        p.extend_from_slice(&(items.len() as u16).to_be_bytes());
        let mut at = base;
        for (id, data) in &items {
            p.extend_from_slice(&(*id as u16).to_be_bytes());
            p.extend_from_slice(&[0, 0, 0, 1]);
            let o = u32::try_from(at).map_err(|_| "AVIF over 4 GiB".to_owned())?;
            let l = u32::try_from(data.len()).map_err(|_| "AVIF over 4 GiB".to_owned())?;
            p.extend_from_slice(&o.to_be_bytes());
            p.extend_from_slice(&l.to_be_bytes());
            at += data.len();
        }
        Ok(full_box(b"iloc", &p))
    };
    if items.iter().any(|i| i.0 > 0xffff) {
        return Err("AVIF: item id too large".into());
    }

    // meta, in its own order, with the rebuilt boxes in place (iref after iinf if new).
    let meta_with = |iloc: &[u8]| {
        let mut p = meta.payload[..4].to_vec();
        for b in &children {
            match &b.typ {
                b"iloc" => p.extend_from_slice(iloc),
                b"iprp" => p.extend_from_slice(&new_iprp),
                b"iinf" => {
                    p.extend_from_slice(&new_iinf);
                    if child(b"iref").is_none()
                        && let Some(r) = &new_iref
                    {
                        p.extend_from_slice(r);
                    }
                }
                b"iref" => p.extend_from_slice(new_iref.as_deref().unwrap_or(b.whole)),
                _ => p.extend_from_slice(b.whole),
            }
        }
        let mut m = Vec::new();
        put_box(&mut m, b"meta", &p);
        m
    };
    // The offsets don't change the size of iloc: measure with zeros, then write for real.
    let meta_len = meta_with(&iloc_for(0)?).len();
    let data_len: usize = items.iter().map(|i| i.1.len()).sum();
    let mdat_start = ftyp.whole.len() + meta_len + 8;
    let new_meta = meta_with(&iloc_for(mdat_start)?);
    let mut out = Vec::with_capacity(mdat_start + data_len);
    out.extend_from_slice(ftyp.whole);
    out.extend_from_slice(&new_meta);
    let mdat_len = u32::try_from(data_len + 8).map_err(|_| "AVIF over 4 GiB".to_owned())?;
    out.extend_from_slice(&mdat_len.to_be_bytes());
    out.extend_from_slice(b"mdat");
    for (_, d) in &items {
        out.extend_from_slice(d);
    }
    Ok(out)
}

/// The primary item id of a parsed meta's `pitm`.
fn primary_of(pitm: &[u8]) -> Result<u32, String> {
    let mut r = Rd(pitm);
    let v = r.uint(1)?;
    r.take(3)?;
    Ok(r.uint(if v == 0 { 2 } else { 4 })? as u32)
}

/// One item's data, gathered from its extents.
fn item_data(file: &[u8], ext: &[(u64, u64)]) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    for &(o, l) in ext {
        let (o, l) = (o as usize, l as usize);
        let end = o.checked_add(l).ok_or("AVIF: bad extent")?;
        data.extend_from_slice(file.get(o..end).ok_or("AVIF: extent outside the file")?);
    }
    Ok(data)
}

/// An `infe` (version 2) for item `id` of `typ`, hidden when `hidden`.
fn infe(id: u32, typ: &[u8; 4], hidden: bool) -> Vec<u8> {
    let mut p = vec![2, 0, 0, u8::from(hidden)];
    p.extend_from_slice(&(id as u16).to_be_bytes());
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(typ);
    p.push(0);
    let mut b = Vec::new();
    put_box(&mut b, b"infe", &p);
    b
}

/// Patches the first `nclx` colour box's primaries and transfer (the AV1 sequence header
/// keeps what ravif wrote; readers take the container's box first).
pub fn set_nclx(avif: &mut [u8], primaries: u16, transfer: u16) -> Result<(), String> {
    let at = avif
        .windows(8)
        .position(|w| w == b"colrnclx")
        .ok_or("AVIF: no nclx colour box")?;
    let p = at + 8;
    let b = avif.get_mut(p..p + 4).ok_or("AVIF: short nclx box")?;
    b[..2].copy_from_slice(&primaries.to_be_bytes());
    b[2..].copy_from_slice(&transfer.to_be_bytes());
    Ok(())
}

/// Sets the colour primaries and transfer in the AV1 sequence headers of `avif` (every
/// `av1C` property's and every item's bitstream that has a described, non-monochrome one),
/// so they agree with the `nclx` box ([`set_nclx`]). Returns how many headers it set.
pub fn set_av1_colour(avif: &mut [u8], primaries: u8, transfer: u8) -> Result<usize, String> {
    // Byte ranges of OBU runs: av1C config OBUs and the items' data.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    {
        let file: &[u8] = avif;
        let base = file.as_ptr() as usize;
        let at = |s: &[u8]| s.as_ptr() as usize - base;
        let top = boxes(file)?;
        let meta = top
            .iter()
            .find(|b| &b.typ == b"meta")
            .ok_or("AVIF: no meta")?;
        let children = boxes(meta.payload.get(4..).ok_or("AVIF: short meta")?)?;
        if let Some(iprp) = children.iter().find(|b| &b.typ == b"iprp") {
            for ipco in boxes(iprp.payload)?.iter().filter(|b| &b.typ == b"ipco") {
                for p in boxes(ipco.payload)?.iter().filter(|b| &b.typ == b"av1C") {
                    if let Some(obus) = p.payload.get(4..) {
                        runs.push((at(obus), obus.len()));
                    }
                }
            }
        }
        if let Some(iloc) = children.iter().find(|b| &b.typ == b"iloc") {
            for (_, ext) in parse_iloc(iloc.payload)? {
                if let Some(&(o, l)) = ext.first() {
                    let (o, l) = (o as usize, l as usize);
                    if o.checked_add(l).is_some_and(|e| e <= file.len()) {
                        runs.push((o, l));
                    }
                }
            }
        }
    }
    Ok(runs
        .into_iter()
        .map(|(o, l)| crate::av1_seq::set_colour(&mut avif[o..o + l], primaries, transfer))
        .sum())
}

/// A gain-map AVIF (ISO 21496-1 in HEIF, ISO/IEC 23008-12 `tmap`): the base file ravif made
/// (its items kept, its primary the SDR base) plus the map file's primary item as a hidden
/// item, a `tmap` item derived from `[base, map]` whose body is `tmap_body` (a version byte
/// and the metadata), with the base's size and an `nclx` of `alt_primaries` (linear) for
/// the alternate rendition, and an `altr` group preferring the `tmap` item. Readers without
/// gain maps show the base.
pub fn with_gain_map(
    base: &[u8],
    map: &[u8],
    tmap_body: &[u8],
    alt_primaries: u16,
) -> Result<Vec<u8>, String> {
    struct Parsed<'a> {
        ftyp: Bx<'a>,
        children: Vec<Bx<'a>>,
        primary: u32,
        iloc: Vec<Location>,
        props: Vec<Bx<'a>>,
        assoc: Assoc,
    }
    fn parse<'a>(file: &'a [u8]) -> Result<Parsed<'a>, String> {
        let top = boxes(file)?;
        let ftyp = top
            .iter()
            .find(|b| &b.typ == b"ftyp")
            .ok_or("AVIF: no ftyp")?;
        let meta = top
            .iter()
            .find(|b| &b.typ == b"meta")
            .ok_or("AVIF: no meta")?;
        let children = boxes(meta.payload.get(4..).ok_or("AVIF: short meta")?)?;
        let child = |t: &[u8; 4]| children.iter().find(|b| &b.typ == t);
        let primary = primary_of(child(b"pitm").ok_or("AVIF: no pitm")?.payload)?;
        let iloc = parse_iloc(child(b"iloc").ok_or("AVIF: no iloc")?.payload)?;
        let iprp = boxes(child(b"iprp").ok_or("AVIF: no iprp")?.payload)?;
        let ipco = iprp
            .iter()
            .find(|b| &b.typ == b"ipco")
            .ok_or("AVIF: no ipco")?;
        let props = boxes(ipco.payload)?;
        let assoc = parse_ipma(
            iprp.iter()
                .find(|b| &b.typ == b"ipma")
                .ok_or("AVIF: no ipma")?
                .payload,
        )?;
        Ok(Parsed {
            ftyp: Bx {
                typ: ftyp.typ,
                whole: ftyp.whole,
                payload: ftyp.payload,
            },
            children,
            primary,
            iloc,
            props,
            assoc,
        })
    }
    let b = parse(base)?;
    let m = parse(map)?;
    let mut items: Vec<(u32, Vec<u8>)> = Vec::new();
    for (id, ext) in &b.iloc {
        items.push((*id, item_data(base, ext)?));
    }
    let map_ext = &m
        .iloc
        .iter()
        .find(|l| l.0 == m.primary)
        .ok_or("AVIF: the map has no data")?
        .1;
    let next = items.iter().map(|i| i.0).max().unwrap_or(0).max(b.primary) + 1;
    let (map_id, tmap_id, group_id) = (next, next + 1, next + 2);
    if group_id > 0xffff {
        return Err("AVIF: too many items".into());
    }
    items.push((map_id, item_data(map, map_ext)?));
    items.push((tmap_id, tmap_body.to_vec()));

    // Properties: the base's, then the map's (for its item), then the tmap's colour.
    let mut props: Vec<u8> = Vec::new();
    b.props
        .iter()
        .for_each(|p| props.extend_from_slice(p.whole));
    let mut assoc = b.assoc.clone();
    let base_ispe = assoc
        .iter()
        .find(|e| e.0 == b.primary)
        .and_then(|e| {
            e.1.iter().copied().find(|&(_, i)| {
                b.props
                    .get(usize::from(i).wrapping_sub(1))
                    .is_some_and(|p| &p.typ == b"ispe")
            })
        })
        .ok_or("AVIF: the base has no ispe")?;
    let mut n = b.props.len() as u16;
    let mut map_props = Vec::new();
    for &(ess, i) in m
        .assoc
        .iter()
        .find(|e| e.0 == m.primary)
        .map_or(&[][..], |e| &e.1[..])
    {
        let p = m
            .props
            .get(usize::from(i).wrapping_sub(1))
            .ok_or("AVIF: bad property index")?;
        props.extend_from_slice(p.whole);
        n += 1;
        map_props.push((ess, n));
    }
    assoc.push((map_id, map_props));
    let mut nclx = b"nclx".to_vec();
    nclx.extend_from_slice(&alt_primaries.to_be_bytes());
    nclx.extend_from_slice(&8u16.to_be_bytes());
    nclx.extend_from_slice(&0u16.to_be_bytes());
    nclx.push(0x80);
    put_box(&mut props, b"colr", &nclx);
    n += 1;
    assoc.push((tmap_id, vec![(false, base_ispe.1), (false, n)]));
    if n > 0x7fff {
        return Err("AVIF: too many properties".into());
    }
    let mut iprp_p = Vec::new();
    put_box(&mut iprp_p, b"ipco", &props);
    iprp_p.extend_from_slice(&write_ipma(&assoc)?);
    let mut new_iprp = Vec::new();
    put_box(&mut new_iprp, b"iprp", &iprp_p);

    // iinf: the base's entries, the map (hidden) and the tmap item.
    let child = |t: &[u8; 4]| b.children.iter().find(|x| &x.typ == t);
    let iinf = child(b"iinf").ok_or("AVIF: no iinf")?;
    let mut r = Rd(iinf.payload);
    let v = r.uint(1)?;
    r.take(3)?;
    r.uint(if v == 0 { 2 } else { 4 })?;
    let mut infes: Vec<Vec<u8>> = boxes(r.0)?.iter().map(|x| x.whole.to_vec()).collect();
    infes.push(infe(map_id, b"av01", true));
    infes.push(infe(tmap_id, b"tmap", false));
    let mut p = (infes.len() as u32).to_be_bytes().to_vec();
    infes.iter().for_each(|x| p.extend_from_slice(x));
    let mut new_iinf = Vec::new();
    put_box(&mut new_iinf, b"iinf", &[&[1, 0, 0, 0][..], &p].concat());

    // iref (16-bit ids): the base's, plus tmap dimg [base, map].
    let mut refs: Vec<u8> = Vec::new();
    if let Some(x) = child(b"iref") {
        if x.payload.first() != Some(&0) {
            return Err("AVIF: 32-bit item references".into());
        }
        refs.extend_from_slice(x.payload.get(4..).ok_or("AVIF: short iref")?);
    }
    let mut d = Vec::new();
    d.extend_from_slice(&(tmap_id as u16).to_be_bytes());
    d.extend_from_slice(&2u16.to_be_bytes());
    d.extend_from_slice(&(b.primary as u16).to_be_bytes());
    d.extend_from_slice(&(map_id as u16).to_be_bytes());
    put_box(&mut refs, b"dimg", &d);
    let mut new_iref = Vec::new();
    put_box(&mut new_iref, b"iref", &[&[0, 0, 0, 0][..], &refs].concat());

    // grpl: the tmap item first in an alternatives group with the base.
    let mut g = group_id.to_be_bytes().to_vec();
    g.extend_from_slice(&2u32.to_be_bytes());
    g.extend_from_slice(&tmap_id.to_be_bytes());
    g.extend_from_slice(&b.primary.to_be_bytes());
    let mut grpl = Vec::new();
    put_box(&mut grpl, b"grpl", &full_box(b"altr", &g));

    // ftyp: the base's brands plus `tmap`.
    let mut ft = b.ftyp.payload.to_vec();
    if !ft.chunks(4).skip(2).any(|c| c == b"tmap") {
        ft.extend_from_slice(b"tmap");
    }
    let mut ftyp = Vec::new();
    put_box(&mut ftyp, b"ftyp", &ft);

    let iloc_for = |base_at: usize| -> Result<Vec<u8>, String> {
        let mut p = vec![0x44, 0x00];
        p.extend_from_slice(&(items.len() as u16).to_be_bytes());
        let mut at = base_at;
        for (id, data) in &items {
            p.extend_from_slice(&(*id as u16).to_be_bytes());
            p.extend_from_slice(&[0, 0, 0, 1]);
            let o = u32::try_from(at).map_err(|_| "AVIF over 4 GiB".to_owned())?;
            let l = u32::try_from(data.len()).map_err(|_| "AVIF over 4 GiB".to_owned())?;
            p.extend_from_slice(&o.to_be_bytes());
            p.extend_from_slice(&l.to_be_bytes());
            at += data.len();
        }
        Ok(full_box(b"iloc", &p))
    };
    let meta_with = |iloc: &[u8]| {
        let mut p = vec![0u8; 4];
        for x in &b.children {
            match &x.typ {
                b"iloc" => p.extend_from_slice(iloc),
                b"iprp" => p.extend_from_slice(&new_iprp),
                b"iinf" => {
                    p.extend_from_slice(&new_iinf);
                    p.extend_from_slice(&new_iref);
                }
                b"iref" | b"grpl" => {}
                _ => p.extend_from_slice(x.whole),
            }
        }
        p.extend_from_slice(&grpl);
        let mut out = Vec::new();
        put_box(&mut out, b"meta", &p);
        out
    };
    let meta_len = meta_with(&iloc_for(0)?).len();
    let data_len: usize = items.iter().map(|i| i.1.len()).sum();
    let mdat_start = ftyp.len() + meta_len + 8;
    let new_meta = meta_with(&iloc_for(mdat_start)?);
    let mut out = Vec::with_capacity(mdat_start + data_len);
    out.extend_from_slice(&ftyp);
    out.extend_from_slice(&new_meta);
    let mdat_len = u32::try_from(data_len + 8).map_err(|_| "AVIF over 4 GiB".to_owned())?;
    out.extend_from_slice(&mdat_len.to_be_bytes());
    out.extend_from_slice(b"mdat");
    for (_, d) in &items {
        out.extend_from_slice(d);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_and_passes_through_without_extras() {
        assert_eq!(add_icc_xmp(b"xyz", None, None).unwrap(), b"xyz");
        assert!(add_icc_xmp(b"\0\0\0\x09ftyp", Some(b"p"), None).is_err());
        assert!(add_icc_xmp(&[0xff; 64], None, Some(b"x")).is_err());
    }
}

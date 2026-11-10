//! HEIF still-image structure: the `meta` box (items, locations, properties, references).

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::Error;
use crate::bmff::{Boxes, FourCc, Reader};

/// Most items / properties / references we accept (bounds parser work on hostile files).
const MAX_ENTRIES: usize = 1 << 16;

/// Most extents we accept over all `iloc` entries (each costs 16 bytes even when the box
/// codes it in zero bytes, with 0-byte offset and length fields).
const MAX_EXTENTS: usize = 1 << 20;

/// `colr` nclx values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Nclx {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}

/// `av1C` essentials.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Av1Config {
    pub bit_depth: u8,
    pub monochrome: bool,
    pub subsampling_x: bool,
    pub subsampling_y: bool,
}

impl Av1Config {
    pub(crate) fn parse(p: &[u8]) -> Result<Self, Error> {
        if p.len() < 4 {
            return Err(Error::Malformed("av1C too short".into()));
        }
        let b = p[2];
        let high = b & 0x40 != 0;
        let twelve = b & 0x20 != 0;
        Ok(Av1Config {
            bit_depth: if twelve && high {
                12
            } else if high {
                10
            } else {
                8
            },
            monochrome: b & 0x10 != 0,
            subsampling_x: b & 0x08 != 0,
            subsampling_y: b & 0x04 != 0,
        })
    }

    pub(crate) fn chroma(&self) -> &'static str {
        match (self.monochrome, self.subsampling_x, self.subsampling_y) {
            (true, _, _) => "4:0:0",
            (false, true, true) => "4:2:0",
            (false, true, false) => "4:2:2",
            (false, false, false) => "4:4:4",
            (false, false, true) => "4:4:0",
        }
    }
}

/// Clean-aperture crop in pixels (applied before scaling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// One parsed property from `ipco`.
#[derive(Clone, Debug)]
pub(crate) enum Prop {
    Ispe(u32, u32),
    Nclx(Nclx),
    Icc(Vec<u8>),

    Av1C(Av1Config),
    Irot(u8),
    /// 0 = mirror about a vertical axis (left/right), 1 = about a horizontal axis (top/bottom).
    Imir(u8),
    Clap([u32; 8]),
    AuxC(Vec<u8>),
    Clli(u16, u16),
    /// `a1op`: the AV1 operating point to decode.
    A1op(u8),
    /// `lsel`: the layer to output (0xFFFF: all, i.e. the highest).
    Lsel(u16),
    Other,
}

fn parse_prop(typ: &FourCc, p: &[u8]) -> Result<Prop, Error> {
    let mut r = Reader::new(p);
    Ok(match typ {
        b"ispe" => {
            r.full()?;
            Prop::Ispe(r.u32()?, r.u32()?)
        }
        b"colr" => {
            let kind = r.fourcc()?;
            match &kind {
                b"nclx" => {
                    let primaries = r.u16()?;
                    let transfer = r.u16()?;
                    let matrix = r.u16()?;
                    let full_range = r.u8()? & 0x80 != 0;
                    Prop::Nclx(Nclx {
                        primaries,
                        transfer,
                        matrix,
                        full_range,
                    })
                }
                b"rICC" | b"prof" => Prop::Icc(r.rest().to_vec()),
                _ => Prop::Other,
            }
        }
        b"av1C" => Prop::Av1C(Av1Config::parse(p)?),
        b"irot" => Prop::Irot(r.u8()? & 3),
        b"imir" => Prop::Imir(r.u8()? & 1),
        b"clap" => {
            let mut v = [0u32; 8];
            for x in &mut v {
                *x = r.u32()?;
            }
            Prop::Clap(v)
        }
        b"auxC" => {
            r.full()?;
            Prop::AuxC(r.cstr().to_vec())
        }
        b"clli" => Prop::Clli(r.u16()?, r.u16()?),
        b"a1op" => Prop::A1op(r.u8()?),
        b"lsel" => Prop::Lsel(r.u16()?),
        _ => Prop::Other,
    })
}

#[derive(Clone, Debug)]
struct Extent {
    offset: u64,
    /// 0 = to the end of the source.
    length: u64,
}

#[derive(Clone, Debug, Default)]
struct Location {
    construction: u8,
    base: u64,
    extents: Vec<Extent>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Item {
    pub id: u32,
    pub typ: FourCc,
    /// Property indexes (into [`Meta::props`], 0-based) in association order.
    pub props: Vec<usize>,
    /// `infe` flag 1: not meant to be shown on its own (grid tiles, gain maps).
    pub hidden: bool,
    loc: Option<Location>,
}

/// One `iref` reference: `from` --typ--> `to` (in order).
#[derive(Clone, Debug)]
pub(crate) struct Reference {
    pub typ: FourCc,
    pub from: u32,
    pub to: Vec<u32>,
}

/// The parsed `meta` box.
#[derive(Debug, Default)]
pub(crate) struct Meta {
    pub primary: Option<u32>,
    pub items: Vec<Item>,
    pub props: Vec<Prop>,
    pub refs: Vec<Reference>,
    idat: Vec<u8>,
    by_id: HashMap<u32, usize>,
    /// Indexes into `refs` by (from, type), in order: a scan of every reference per item
    /// was quadratic on files of thousands of grid items.
    refs_by_from: HashMap<(u32, FourCc), Vec<usize>>,
}

/// Grid descriptor (`grid` item payload).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Grid {
    pub rows: u32,
    pub cols: u32,
    pub width: u32,
    pub height: u32,
}

impl Grid {
    /// Longest descriptor [`Grid::parse`] reads.
    pub(crate) const MAX_LEN: usize = 12;

    pub(crate) fn parse(p: &[u8]) -> Result<Grid, Error> {
        let mut r = Reader::new(p);
        let _version = r.u8()?;
        let flags = r.u8()?;
        let rows = u32::from(r.u8()?) + 1;
        let cols = u32::from(r.u8()?) + 1;
        let (width, height) = if flags & 1 != 0 {
            (r.u32()?, r.u32()?)
        } else {
            (u32::from(r.u16()?), u32::from(r.u16()?))
        };
        Ok(Grid {
            rows,
            cols,
            width,
            height,
        })
    }
}

impl Meta {
    pub(crate) fn parse(payload: &[u8]) -> Result<Meta, Error> {
        let mut r = Reader::new(payload);
        r.full()?;
        let mut m = Meta::default();
        let mut locs: HashMap<u32, Location> = HashMap::new();
        let mut assoc: Vec<(u32, Vec<usize>)> = Vec::new();
        let mut infos: Vec<Item> = Vec::new();
        let mut handler_ok = false;
        for b in Boxes::new(r.rest()) {
            let b = b?;
            let mut r = Reader::new(b.payload);
            match &b.typ {
                b"hdlr" => {
                    r.full()?;
                    r.u32()?;
                    handler_ok = &r.fourcc()? == b"pict";
                }
                b"pitm" => {
                    let (v, _) = r.full()?;
                    m.primary = Some(if v == 0 {
                        u32::from(r.u16()?)
                    } else {
                        r.u32()?
                    });
                }
                b"iloc" => parse_iloc(b.payload, &mut locs)?,
                b"iinf" => {
                    let (v, _) = r.full()?;
                    let _count = if v == 0 {
                        u32::from(r.u16()?)
                    } else {
                        r.u32()?
                    };
                    for e in Boxes::new(r.rest()) {
                        let e = e?;
                        if &e.typ != b"infe" {
                            continue;
                        }
                        if infos.len() >= MAX_ENTRIES {
                            return Err(Error::Unsupported("too many items".into()));
                        }
                        if let Some(it) = parse_infe(e.payload)? {
                            infos.push(it);
                        }
                    }
                }
                b"iprp" => {
                    for c in Boxes::new(b.payload) {
                        let c = c?;
                        match &c.typ {
                            b"ipco" => {
                                for p in Boxes::new(c.payload) {
                                    let p = p?;
                                    if m.props.len() >= MAX_ENTRIES {
                                        return Err(Error::Unsupported(
                                            "too many properties".into(),
                                        ));
                                    }
                                    m.props.push(parse_prop(&p.typ, p.payload)?);
                                }
                            }
                            b"ipma" => parse_ipma(c.payload, &mut assoc)?,
                            _ => {}
                        }
                    }
                }
                b"iref" => {
                    let (v, _) = r.full()?;
                    for e in Boxes::new(r.rest()) {
                        let e = e?;
                        let mut rr = Reader::new(e.payload);
                        let id = |rr: &mut Reader| -> Result<u32, Error> {
                            if v == 0 {
                                rr.u16().map(u32::from)
                            } else {
                                rr.u32()
                            }
                        };
                        let from = id(&mut rr)?;
                        let n = rr.u16()? as usize;
                        let mut to = Vec::with_capacity(n.min(rr.remaining() / 2));
                        for _ in 0..n {
                            to.push(id(&mut rr)?);
                        }
                        if m.refs.len() >= MAX_ENTRIES {
                            return Err(Error::Unsupported("too many references".into()));
                        }
                        m.refs.push(Reference {
                            typ: e.typ,
                            from,
                            to,
                        });
                    }
                }
                b"idat" => m.idat = b.payload.to_vec(),
                _ => {}
            }
        }
        if !handler_ok {
            return Err(Error::NotAvif);
        }
        for mut it in infos {
            it.loc = locs.remove(&it.id);
            if m.by_id.contains_key(&it.id) {
                continue;
            }
            m.by_id.insert(it.id, m.items.len());
            m.items.push(it);
        }
        for (k, r) in m.refs.iter().enumerate() {
            m.refs_by_from.entry((r.from, r.typ)).or_default().push(k);
        }
        for (id, props) in assoc {
            if props.iter().any(|&p| p >= m.props.len()) {
                return Err(Error::Malformed(
                    "ipma references a missing property".into(),
                ));
            }
            if let Some(&i) = m.by_id.get(&id) {
                m.items[i].props.extend(props);
            }
        }
        Ok(m)
    }

    pub(crate) fn item(&self, id: u32) -> Option<&Item> {
        self.by_id.get(&id).map(|&i| &self.items[i])
    }

    /// The images of the file a viewer shows on their own, in item order with the primary
    /// first: coded (`av01`) and grid items that are not hidden, not an input of a derived
    /// image (grid tiles, a gain map's base and map) and not a thumbnail or auxiliary image
    /// (alpha, depth) of another. Empty without a primary item (a sequence-only file).
    pub(crate) fn top_level(&self) -> Vec<u32> {
        let Some(primary) = self.primary else {
            return Vec::new();
        };
        // Sets, not per-item scans: items × references is quadratic on hostile files.
        let mut inputs = HashSet::new();
        let mut referring = HashSet::new();
        for r in &self.refs {
            match &r.typ {
                b"dimg" => inputs.extend(r.to.iter().copied()),
                b"thmb" | b"auxl" if !r.to.is_empty() => {
                    referring.insert(r.from);
                }
                _ => {}
            }
        }
        let mut out = vec![primary];
        out.extend(
            self.items
                .iter()
                .filter(|it| {
                    matches!(&it.typ, b"av01" | b"grid")
                        && it.id != primary
                        && !it.hidden
                        && !inputs.contains(&it.id)
                        && !referring.contains(&it.id)
                })
                .map(|it| it.id),
        );
        out
    }

    /// Properties of `id` in association order.
    pub(crate) fn props_of(&self, id: u32) -> impl Iterator<Item = &Prop> {
        self.item(id)
            .into_iter()
            .flat_map(|it| it.props.iter().map(|&p| &self.props[p]))
    }

    /// References of type `typ` *from* `id`.
    pub(crate) fn refs_from(&self, id: u32, typ: &FourCc) -> Vec<u32> {
        self.refs_by_from
            .get(&(id, *typ))
            .into_iter()
            .flatten()
            .flat_map(|&k| self.refs[k].to.iter().copied())
            .collect()
    }

    /// Items with a reference of type `typ` pointing *to* `id`.
    pub(crate) fn refs_to(&self, id: u32, typ: &FourCc) -> Vec<u32> {
        self.refs
            .iter()
            .filter(|r| &r.typ == typ && r.to.contains(&id))
            .map(|r| r.from)
            .collect()
    }

    /// The bytes of item `id` (borrowed when it is a single extent).
    pub(crate) fn item_data<'a>(&'a self, file: &'a [u8], id: u32) -> Result<Cow<'a, [u8]>, Error> {
        self.item_prefix(file, id, usize::MAX)
    }

    /// The first `n` bytes of item `id` (fewer if it is shorter), with the errors of
    /// [`Meta::item_data`] but without copying the rest: checking every grid of a file
    /// copied each one whole.
    pub(crate) fn item_prefix<'a>(
        &'a self,
        file: &'a [u8],
        id: u32,
        n: usize,
    ) -> Result<Cow<'a, [u8]>, Error> {
        let slices = self.item_extents(file, id)?;
        match slices.as_slice() {
            [s] => Ok(Cow::Borrowed(&s[..n.min(s.len())])),
            ss => {
                let total: usize = ss.iter().map(|s| s.len()).sum();
                let mut v = Vec::with_capacity(total.min(n));
                for s in ss {
                    let take = (n - v.len()).min(s.len());
                    v.extend_from_slice(&s[..take]);
                    if v.len() == n {
                        break;
                    }
                }
                Ok(Cow::Owned(v))
            }
        }
    }

    /// The extents of item `id` in order, checked against their source.
    fn item_extents<'a>(&'a self, file: &'a [u8], id: u32) -> Result<Vec<&'a [u8]>, Error> {
        let it = self
            .item(id)
            .ok_or_else(|| Error::Malformed(format!("item {id} missing")))?;
        let loc = it
            .loc
            .as_ref()
            .ok_or_else(|| Error::Malformed(format!("item {id} has no location")))?;
        let src: &'a [u8] = match loc.construction {
            0 => file,
            1 => &self.idat,
            c => return Err(Error::Unsupported(format!("iloc construction method {c}"))),
        };
        let bad = || {
            if loc.construction == 0 {
                Error::Truncated
            } else {
                Error::Malformed("extent outside idat".into())
            }
        };
        let slice = |e: &Extent| -> Result<&'a [u8], Error> {
            let start = loc
                .base
                .checked_add(e.offset)
                .ok_or_else(|| Error::Malformed("extent offset overflow".into()))?;
            if start > src.len() as u64 {
                return Err(bad());
            }
            let start = start as usize;
            let end = if e.length == 0 {
                src.len()
            } else {
                let end = (start as u64).saturating_add(e.length);
                if end > src.len() as u64 {
                    return Err(bad());
                }
                end as usize
            };
            Ok(&src[start..end])
        };
        match loc.extents.as_slice() {
            [] => Err(Error::Malformed(format!("item {id} has no extents"))),
            [e] => Ok(vec![slice(e)?]),
            es => {
                let mut out = Vec::with_capacity(es.len());
                let mut total = 0usize;
                for e in es {
                    let s = slice(e)?;
                    // Extents of one item do not overlap, so together they fit in the source;
                    // more means the same bytes repeated (up to 65535 times the file).
                    if total + s.len() > src.len() {
                        return Err(Error::Malformed(format!("item {id} extents overlap")));
                    }
                    total += s.len();
                    out.push(s);
                }
                Ok(out)
            }
        }
    }
}

fn parse_infe(p: &[u8]) -> Result<Option<Item>, Error> {
    let mut r = Reader::new(p);
    let (v, flags) = r.full()?;
    if v < 2 {
        return Ok(None);
    }
    let id = if v == 2 {
        u32::from(r.u16()?)
    } else {
        r.u32()?
    };
    let _prot = r.u16()?;
    let typ = r.fourcc()?;
    Ok(Some(Item {
        id,
        typ,
        props: Vec::new(),
        hidden: flags & 1 != 0,
        loc: None,
    }))
}

fn parse_ipma(p: &[u8], out: &mut Vec<(u32, Vec<usize>)>) -> Result<(), Error> {
    let mut r = Reader::new(p);
    let (v, flags) = r.full()?;
    let n = r.u32()? as usize;
    for _ in 0..n {
        if out.len() >= MAX_ENTRIES {
            return Err(Error::Unsupported("too many ipma entries".into()));
        }
        let id = if v < 1 { u32::from(r.u16()?) } else { r.u32()? };
        let k = r.u8()?;
        let mut props = Vec::with_capacity(k as usize);
        for _ in 0..k {
            let idx = if flags & 1 != 0 {
                (r.u16()? & 0x7fff) as usize
            } else {
                (r.u8()? & 0x7f) as usize
            };
            if idx > 0 {
                props.push(idx - 1);
            }
        }
        out.push((id, props));
    }
    Ok(())
}

fn parse_iloc(p: &[u8], out: &mut HashMap<u32, Location>) -> Result<(), Error> {
    let mut r = Reader::new(p);
    let (v, _) = r.full()?;
    if v > 2 {
        return Err(Error::Unsupported(format!("iloc version {v}")));
    }
    let a = r.u8()?;
    let b = r.u8()?;
    let (off_sz, len_sz, base_sz) = (a >> 4, a & 15, b >> 4);
    let idx_sz = if v >= 1 { b & 15 } else { 0 };
    let n = if v < 2 { u32::from(r.u16()?) } else { r.u32()? } as usize;
    let mut total_extents = 0usize;
    for _ in 0..n {
        if out.len() >= MAX_ENTRIES {
            return Err(Error::Unsupported("too many iloc entries".into()));
        }
        let id = if v < 2 { u32::from(r.u16()?) } else { r.u32()? };
        let construction = if v >= 1 { (r.u16()? & 15) as u8 } else { 0 };
        let _dref = r.u16()?;
        let base = r.uint(base_sz)?;
        let ne = r.u16()? as usize;
        total_extents += ne;
        if total_extents > MAX_EXTENTS {
            return Err(Error::Unsupported("too many iloc extents".into()));
        }
        let mut extents = Vec::with_capacity(ne.min(r.remaining()));
        for _ in 0..ne {
            if idx_sz > 0 {
                r.uint(idx_sz)?;
            }
            let offset = r.uint(off_sz)?;
            let length = r.uint(len_sz)?;
            extents.push(Extent { offset, length });
        }
        out.insert(
            id,
            Location {
                construction,
                base,
                extents,
            },
        );
    }
    Ok(())
}

/// Composes `irot`/`imir` (in association order) into an EXIF orientation (1..=8).
pub(crate) fn orientation<'a>(props: impl Iterator<Item = &'a Prop>) -> u8 {
    // State: display = flipH^f ∘ rotCW^r (applied to the stored image).
    let (mut r, mut f) = (0u8, false);
    for p in props {
        match *p {
            Prop::Irot(a) => {
                // irot rotates anti-clockwise by a*90 => clockwise by (4-a)*90.
                let k = (4 - a) & 3;
                // rotCW^k ∘ flipH^f = flipH^f ∘ rotCW^(±k)
                r = if f { (r + 4 - k) & 3 } else { (r + k) & 3 };
            }
            Prop::Imir(axis) => {
                // axis 1 mirrors left/right, axis 0 top/bottom (libavif, libheif and ffmpeg agree).
                if axis == 1 {
                    f = !f;
                } else {
                    // flipV = flipH ∘ rot180
                    f = !f;
                    r = (r + 2) & 3;
                }
            }
            _ => {}
        }
    }
    match (r, f) {
        (0, false) => 1,
        (0, true) => 2,
        (2, false) => 3,
        (2, true) => 4,
        (1, true) => 5,
        (1, false) => 6,
        (3, true) => 7,
        _ => 8,
    }
}

/// Converts a `clap` property to an integer crop inside `w`×`h` (None = ignore it).
pub(crate) fn clap_crop(c: &[u32; 8], w: u32, h: u32) -> Option<Crop> {
    let [wn, wd, hn, hd, xn, xd, yn, yd] = *c;
    if wd == 0 || hd == 0 || xd == 0 || yd == 0 {
        return None;
    }
    let cw = f64::from(wn) / f64::from(wd);
    let ch = f64::from(hn) / f64::from(hd);
    let ox = f64::from(xn as i32) / f64::from(xd);
    let oy = f64::from(yn as i32) / f64::from(yd);
    let x = ox + (f64::from(w) - 1.0) / 2.0 - (cw - 1.0) / 2.0;
    let y = oy + (f64::from(h) - 1.0) / 2.0 - (ch - 1.0) / 2.0;
    let (x, y, cw, ch) = (x.round(), y.round(), cw.round(), ch.round());
    if x < 0.0 || y < 0.0 || cw < 1.0 || ch < 1.0 || x + cw > f64::from(w) || y + ch > f64::from(h)
    {
        return None;
    }
    let crop = Crop {
        x: x as u32,
        y: y as u32,
        w: cw as u32,
        h: ch as u32,
    };
    if crop == (Crop { x: 0, y: 0, w, h }) {
        return None;
    }
    Some(crop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_table() {
        let o = |v: &[Prop]| orientation(v.iter());
        assert_eq!(o(&[]), 1);
        assert_eq!(o(&[Prop::Irot(1)]), 8);
        assert_eq!(o(&[Prop::Irot(2)]), 3);
        assert_eq!(o(&[Prop::Irot(3)]), 6);
        assert_eq!(o(&[Prop::Imir(1)]), 2);
        assert_eq!(o(&[Prop::Imir(0)]), 4);
        // libheif writes EXIF 5 (transpose) as irot 3 + imir 1 and EXIF 7 as irot 1 + imir 1.
        assert_eq!(o(&[Prop::Irot(3), Prop::Imir(1)]), 5);
        assert_eq!(o(&[Prop::Irot(1), Prop::Imir(1)]), 7);
        assert_eq!(o(&[Prop::Irot(1), Prop::Imir(0)]), 5);
        assert_eq!(o(&[Prop::Irot(3), Prop::Imir(0)]), 7);
    }

    #[test]
    fn clap_centered() {
        // 100x80 cropped to 50x40 centred.
        let c = clap_crop(&[50, 1, 40, 1, 0, 1, 0, 1], 100, 80).unwrap();
        assert_eq!(
            c,
            Crop {
                x: 25,
                y: 20,
                w: 50,
                h: 40
            }
        );
        assert!(clap_crop(&[200, 1, 40, 1, 0, 1, 0, 1], 100, 80).is_none());
    }
}

//! JP2 file format (ISO/IEC 15444-1 Annex I) box parsing.

use crate::{Jp2Error, Result};

const JP2_SIGNATURE: [u8; 12] = [0, 0, 0, 12, b'j', b'P', b' ', b' ', 0x0D, 0x0A, 0x87, 0x0A];
/// Upper bound on the number of boxes / entries we are willing to look at.
const MAX_BOXES: usize = 1 << 16;

/// What kind of file the data is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    /// A raw codestream (`.j2k`, `.j2c`, `.jpc`).
    Codestream,
    /// A JP2 file (`.jp2`).
    Jp2,
    /// A JPX (Part 2) file, decoded one contiguous codestream at a time
    /// ([`crate::codestream_count`], [`crate::decode_at`]): the first with the JP2 header (as a
    /// JP2 reader would), codestream `n` > 0 with the JP2 header overridden by the `n`-th
    /// codestream header (`jpch`: palette, mapping, channels) and the `n`-th compositing layer
    /// header (`jplh`: colour, channels, resolution) where the file has them. Codestreams in
    /// fragment tables are joined by [`crate::defragment`] (fragments in the file itself);
    /// Part 2 colour spaces are not supported; the composition box is drawn by
    /// [`crate::decode_composite`].
    Jpx,
}

/// Colour specification of the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpace {
    /// sRGB (enumerated 16), or a 3-component codestream without colour box.
    Srgb,
    /// Greyscale (enumerated 17), or a 1/2-component codestream without colour box.
    Gray,
    /// sYCC (enumerated 18): converted to RGB on output.
    Sycc,
    /// CMYK (enumerated 12, JPX): converted naively to RGB on output.
    Cmyk,
    /// e-sYCC (enumerated 24, JPX): converted like sYCC.
    Esycc,
    /// Described by an ICC profile ([`crate::Jp2Info::icc_profile`]); samples are output as is
    /// (1 colour channel: grey, otherwise the first three as RGB).
    Icc,
    /// Some other enumerated space (or unknown); samples are output as is.
    Other(u32),
}

#[derive(Clone, Debug)]
pub(crate) struct Palette {
    /// Number of entries.
    pub entries: usize,
    /// Per column: (precision, signed).
    pub columns: Vec<(u8, bool)>,
    /// `entries x columns`, row-major, each value already masked to its precision.
    pub values: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CmapEntry {
    pub component: u16,
    /// `None`: direct use; `Some(col)`: palette column.
    pub palette_col: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CdefEntry {
    pub channel: u16,
    pub typ: u16,
    pub assoc: u16,
}

/// Capture / display resolution in pixels per metre (vertical, horizontal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub capture: Option<(f64, f64)>,
    pub display: Option<(f64, f64)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Boxes<'a> {
    pub container: Container,
    pub codestream: &'a [u8],
    /// An `ihdr` box was found (required in JP2; its values are superseded by SIZ).
    pub has_ihdr: bool,
    pub color: Option<ColorSpace>,
    pub icc: Option<Vec<u8>>,
    pub palette: Option<Palette>,
    pub cmap: Option<Vec<CmapEntry>>,
    pub cdef: Option<Vec<CdefEntry>>,
    pub resolution: Option<Resolution>,
}

pub(crate) fn is_jp2(data: &[u8]) -> bool {
    data.len() >= 12 && data[..12] == JP2_SIGNATURE
}

pub(crate) fn is_codestream(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0xFF && data[1] == 0x4F && data[2] == 0xFF && data[3] == 0x51
}

#[inline]
pub(crate) fn be16(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(i)?, *d.get(i + 1)?]))
}
#[inline]
pub(crate) fn be32(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// Iterate the boxes of a box list: yields `(type, contents)`.
pub(crate) fn boxes(data: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut pos = 0usize;
    let mut n = 0usize;
    std::iter::from_fn(move || {
        if pos + 8 > data.len() || n >= MAX_BOXES {
            return None;
        }
        n += 1;
        let lbox = be32(data, pos)? as u64;
        let tbox: [u8; 4] = data[pos + 4..pos + 8].try_into().ok()?;
        let (hdr, len) = match lbox {
            0 => (8u64, (data.len() - pos) as u64),
            1 => {
                let x = u64::from_be_bytes(data.get(pos + 8..pos + 16)?.try_into().ok()?);
                (16, x)
            }
            2..=7 => return None,
            l => (8, l),
        };
        if len < hdr {
            return None;
        }
        let end = (pos as u64).saturating_add(len).min(data.len() as u64) as usize;
        let start = pos + hdr as usize;
        if start > end {
            return None;
        }
        let contents = &data[start..end];
        pos = end;
        if lbox == 0 {
            pos = data.len();
        }
        Some((tbox, contents))
    })
}

/// The GeoJP2 box's UUID (a `uuid` box holding a degenerate GeoTIFF):
/// B14BF8BD-083D-4B43-A5AE-8CD7D5A6CE03, from the GeoJP2 specification (Mapping Science, 2004).
pub const GEOJP2_UUID: [u8; 16] = [
    0xB1, 0x4B, 0xF8, 0xBD, 0x08, 0x3D, 0x4B, 0x43, 0xA5, 0xAE, 0x8C, 0xD7, 0xD5, 0xA6, 0xCE, 0x03,
];

/// The georeferencing boxes of a JP2 file: GeoJP2, and GMLJP2 (OGC 05-047r3 and 08-085r4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeoBoxes<'a> {
    /// The GeoJP2 box's TIFF (its first IFD carries the GeoTIFF tags).
    pub geojp2: Option<&'a [u8]>,
    /// The XML boxes inside the association boxes labelled `gml.data` (GMLJP2), in file order.
    pub gml: Vec<&'a [u8]>,
}

/// Finds the GeoJP2 and GMLJP2 boxes at the top level of a JP2 file (box headers only, the
/// codestream is skipped). Empty for raw codestreams.
pub fn geo_boxes(data: &[u8]) -> GeoBoxes<'_> {
    let mut out = GeoBoxes::default();
    if !is_jp2(data) {
        return out;
    }
    for (t, c) in boxes(&data[12..]) {
        match &t {
            b"uuid" if c.len() > 16 && c[..16] == GEOJP2_UUID => {
                out.geojp2.get_or_insert(&c[16..]);
            }
            b"asoc" => {
                if let Some((l, label)) = boxes(c).next()
                    && &l == b"lbl "
                    && trim_nul(label) == b"gml.data"
                {
                    gml_xml(c, 0, &mut out.gml);
                }
            }
            _ => {}
        }
    }
    out
}

fn trim_nul(b: &[u8]) -> &[u8] {
    let end = b.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
    &b[..end]
}

/// The `xml ` boxes of an association box tree (at most four levels deep, 64 boxes).
fn gml_xml<'a>(asoc: &'a [u8], depth: usize, out: &mut Vec<&'a [u8]>) {
    for (t, c) in boxes(asoc) {
        match &t {
            b"xml " if out.len() < 64 => out.push(c),
            b"asoc" if depth < 4 => gml_xml(c, depth + 1, out),
            _ => {}
        }
    }
}

/// How many codestreams the file has: the top-level `jp2c` (and fragment table) boxes of a JPX file (at most
/// [`MAX_BOXES`]), 1 for anything else, 0 for data that is not JPEG 2000.
pub(crate) fn codestream_count(data: &[u8]) -> usize {
    if is_codestream(data) {
        return 1;
    }
    if !is_jp2(data) {
        return 0;
    }
    let mut jpx = false;
    let mut n = 0;
    for (t, c) in boxes(&data[12..]) {
        match &t {
            b"ftyp" => jpx = c.get(0..4) == Some(b"jpx "),
            b"jp2c" | b"ftbl" => n += 1,
            _ => {}
        }
    }
    if jpx { n } else { n.min(1) }
}

/// Most fragments one fragment table may list.
const MAX_FRAGMENTS: usize = 1 << 16;

/// The file with every fragment table (`ftbl`) it can join replaced by a contiguous
/// codestream box holding its fragments, or `None` when there is none. A table whose
/// fragments it does not hold (in other files: a data reference other than 0; past its end;
/// a damaged list) stays as it is, and that codestream does not decode.
pub(crate) fn defragment(data: &[u8]) -> Option<Vec<u8>> {
    if !is_jp2(data) || !boxes(&data[12..]).any(|(t, _)| &t == b"ftbl") {
        return None;
    }
    let mut joined = 0;
    let mut out = data[..12].to_vec();
    let put = |out: &mut Vec<u8>, t: &[u8; 4], c: &[u8]| {
        if c.len() + 8 > u32::MAX as usize {
            out.extend(1u32.to_be_bytes());
            out.extend_from_slice(t);
            out.extend((c.len() as u64 + 16).to_be_bytes());
        } else {
            out.extend((c.len() as u32 + 8).to_be_bytes());
            out.extend_from_slice(t);
        }
        out.extend_from_slice(c);
    };
    for (t, c) in boxes(&data[12..]) {
        if &t != b"ftbl" {
            put(&mut out, &t, c);
            continue;
        }
        match join(data, c) {
            Ok(cs) => {
                put(&mut out, b"jp2c", &cs);
                joined += 1;
            }
            Err(_) => put(&mut out, &t, c),
        }
    }
    (joined > 0).then_some(out)
}

/// The fragments a fragment table (`ftbl` contents) lists, joined.
pub(crate) fn join(data: &[u8], c: &[u8]) -> Result<Vec<u8>> {
    let flst = boxes(c)
        .find(|(t, _)| t == b"flst")
        .map(|(_, f)| f)
        .ok_or(Jp2Error::Malformed(
            "fragment table without a fragment list",
        ))?;
    let nf = usize::from(be16(flst, 0).ok_or(Jp2Error::Malformed("short fragment list"))?);
    if nf > MAX_FRAGMENTS {
        return Err(Jp2Error::Malformed("fragment list too long"));
    }
    let mut cs = Vec::new();
    for k in 0..nf {
        let e = flst
            .get(2 + 14 * k..16 + 14 * k)
            .ok_or(Jp2Error::Truncated("fragment list"))?;
        let off = u64::from_be_bytes(e[0..8].try_into().unwrap_or_default());
        let len = u64::from(u32::from_be_bytes(e[8..12].try_into().unwrap_or_default()));
        let dr = u16::from_be_bytes([e[12], e[13]]);
        if dr != 0 {
            return Err(Jp2Error::Unsupported(
                "JPX codestream fragments in another file",
            ));
        }
        let end = off.saturating_add(len);
        if end > data.len() as u64 {
            return Err(Jp2Error::Truncated("codestream fragment past the end"));
        }
        // Fragments repeated over and over could join to more than the file holds.
        if (cs.len() as u64).saturating_add(len) > data.len() as u64 {
            return Err(Jp2Error::TooLarge(cs.len() as u64 + len));
        }
        cs.extend_from_slice(&data[off as usize..end as usize]);
    }
    Ok(cs)
}

/// The boxes for codestream `index` (0 for the first; only a JPX file has others) with the
/// headers of compositing layer `layer` (its colour and channels, from `jplh`; the
/// codestream header `jpch` stays the codestream's). Usually `layer == index`.
pub(crate) fn parse_layer(data: &[u8], index: usize, layer: usize) -> Result<Boxes<'_>> {
    if is_codestream(data) {
        return Ok(Boxes {
            container: Container::Codestream,
            codestream: data,
            has_ihdr: false,
            color: None,
            icc: None,
            palette: None,
            cmap: None,
            cdef: None,
            resolution: None,
        });
    }
    if !is_jp2(data) {
        return Err(Jp2Error::NotJp2);
    }
    let mut out = Boxes {
        container: Container::Jp2,
        codestream: &[],
        has_ihdr: false,
        color: None,
        icc: None,
        palette: None,
        cmap: None,
        cdef: None,
        resolution: None,
    };
    let mut seen_ftyp = false;
    let mut found_cs = false;
    // Top-level codestreams, codestream headers and compositing layer headers seen so far.
    let (mut streams, mut jpch, mut jplh) = (0usize, 0usize, 0usize);
    // The `index`-th codestream / compositing layer headers, applied over the JP2 header.
    let (mut own_ch, mut own_lh): (Option<&[u8]>, Option<&[u8]>) = (None, None);
    for (t, c) in boxes(&data[12..]) {
        match &t {
            b"ftyp" => {
                seen_ftyp = true;
                let brand = c.get(0..4).unwrap_or(b"    ");
                let compat_jp2 = c
                    .get(8..)
                    .is_some_and(|cl| cl.chunks_exact(4).any(|b| b == b"jp2 "));
                match brand {
                    b"jp2 " => {}
                    b"jpx " => out.container = Container::Jpx,
                    _ if compat_jp2 => {}
                    b"jpm " => {
                        return Err(Jp2Error::Unsupported("JPM (compound image) file"));
                    }
                    b"jph " => {}
                    _ => return Err(Jp2Error::Unsupported("unknown JP2 family brand")),
                }
            }
            b"jp2h" => parse_jp2h(c, &mut out)?,
            b"res " => {}
            b"jp2c" if !found_cs => {
                if streams == index {
                    out.codestream = c;
                    found_cs = true;
                }
                streams += 1;
            }
            b"jpch" if index > 0 || layer > 0 => {
                if jpch == index {
                    own_ch = Some(c);
                }
                jpch += 1;
            }
            b"jplh" if index > 0 || layer > 0 => {
                if jplh == layer {
                    own_lh = Some(c);
                }
                jplh += 1;
            }
            // A fragmented codestream counts in the numbering; it decodes once joined.
            b"ftbl" if !found_cs => {
                if streams == index {
                    return Err(Jp2Error::Unsupported(
                        "JPX codestream in fragments this file does not hold",
                    ));
                }
                streams += 1;
            }
            _ => {}
        }
        if found_cs && out.has_ihdr && index == 0 && layer == 0 {
            break;
        }
    }
    if (index > 0 || layer > 0) && out.container != Container::Jpx {
        return Err(Jp2Error::Unsupported(
            "only a JPX file has more than one codestream",
        ));
    }
    if let Some(c) = own_ch {
        let mut own = Boxes {
            has_ihdr: out.has_ihdr,
            ..empty(out.container)
        };
        parse_jp2h(c, &mut own)?;
        out.palette = own.palette.or(out.palette);
        out.cmap = own.cmap.or(out.cmap);
        out.cdef = own.cdef.or(out.cdef);
    }
    if let Some(c) = own_lh {
        let mut own = Boxes {
            has_ihdr: out.has_ihdr,
            ..empty(out.container)
        };
        // Channel definitions and resolution sit in the layer header, colour in its `cgrp`.
        parse_jp2h(c, &mut own)?;
        for (t, g) in boxes(c) {
            if &t == b"cgrp" {
                parse_jp2h(g, &mut own)?;
            }
        }
        if own.color.is_some() {
            out.color = own.color;
            out.icc = own.icc;
        }
        out.cdef = own.cdef.or(out.cdef);
        out.resolution = own.resolution.or(out.resolution);
    }
    if !seen_ftyp {
        return Err(Jp2Error::Malformed("JP2 file without ftyp box"));
    }
    if !found_cs {
        return Err(if out.container == Container::Jpx {
            Jp2Error::Unsupported("JPX file without a contiguous codestream")
        } else {
            Jp2Error::Truncated("no codestream box")
        });
    }
    Ok(out)
}

/// No boxes yet.
pub(crate) fn empty<'a>(container: Container) -> Boxes<'a> {
    Boxes {
        container,
        codestream: &[],
        has_ihdr: false,
        color: None,
        icc: None,
        palette: None,
        cmap: None,
        cdef: None,
        resolution: None,
    }
}

pub(crate) fn parse_jp2h<'a>(data: &'a [u8], out: &mut Boxes<'a>) -> Result<()> {
    for (t, c) in boxes(data) {
        match &t {
            b"ihdr" => {
                if c.len() < 14 {
                    return Err(Jp2Error::Malformed("short ihdr box"));
                }
                out.has_ihdr = true;
            }
            b"colr" => {
                // The first colour box we understand wins (JP2 readers use the first one).
                if out.color.is_some() || c.len() < 3 {
                    continue;
                }
                let meth = c[0];
                match meth {
                    1 => {
                        if let Some(e) = be32(c, 3) {
                            out.color = Some(match e {
                                16 => ColorSpace::Srgb,
                                17 => ColorSpace::Gray,
                                18 => ColorSpace::Sycc,
                                12 => ColorSpace::Cmyk,
                                24 => ColorSpace::Esycc,
                                e => ColorSpace::Other(e),
                            });
                        }
                    }
                    2 | 3 if c.len() > 3 => {
                        out.color = Some(ColorSpace::Icc);
                        out.icc = Some(c[3..].to_vec());
                    }
                    _ => {}
                }
            }
            b"pclr" => out.palette = parse_pclr(c),
            b"cmap" => {
                let v: Vec<CmapEntry> = c
                    .chunks_exact(4)
                    .map(|e| CmapEntry {
                        component: u16::from_be_bytes([e[0], e[1]]),
                        palette_col: (e[2] == 1).then_some(e[3]),
                    })
                    .collect();
                if !v.is_empty() {
                    out.cmap = Some(v);
                }
            }
            b"cdef" => {
                let n = be16(c, 0).unwrap_or(0) as usize;
                let v: Vec<CdefEntry> = c
                    .get(2..)
                    .unwrap_or(&[])
                    .chunks_exact(6)
                    .take(n)
                    .map(|e| CdefEntry {
                        channel: u16::from_be_bytes([e[0], e[1]]),
                        typ: u16::from_be_bytes([e[2], e[3]]),
                        assoc: u16::from_be_bytes([e[4], e[5]]),
                    })
                    .collect();
                if !v.is_empty() {
                    out.cdef = Some(v);
                }
            }
            b"res " => {
                let mut r = Resolution {
                    capture: None,
                    display: None,
                };
                for (t2, c2) in boxes(c) {
                    let v = parse_res(c2);
                    match &t2 {
                        b"resc" => r.capture = v,
                        b"resd" => r.display = v,
                        _ => {}
                    }
                }
                out.resolution = Some(r);
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_res(c: &[u8]) -> Option<(f64, f64)> {
    if c.len() < 10 {
        return None;
    }
    let vn = be16(c, 0)? as f64;
    let vd = be16(c, 2)? as f64;
    let hn = be16(c, 4)? as f64;
    let hd = be16(c, 6)? as f64;
    let ve = c[8] as i8 as i32;
    let he = c[9] as i8 as i32;
    if vd == 0.0 || hd == 0.0 {
        return None;
    }
    Some((vn / vd * 10f64.powi(ve), hn / hd * 10f64.powi(he)))
}

fn parse_pclr(c: &[u8]) -> Option<Palette> {
    let ne = be16(c, 0)? as usize;
    let npc = *c.get(2)? as usize;
    if ne == 0 || ne > 1024 || npc == 0 {
        return None;
    }
    let mut columns = Vec::with_capacity(npc);
    for i in 0..npc {
        let b = *c.get(3 + i)?;
        let prec = (b & 0x7F) + 1;
        if prec > 38 {
            return None;
        }
        columns.push((prec, b & 0x80 != 0));
    }
    let mut pos = 3 + npc;
    let mut values = Vec::with_capacity(ne * npc);
    for _ in 0..ne {
        for &(prec, _) in &columns {
            let nb = (prec as usize).div_ceil(8);
            let bytes = c.get(pos..pos + nb)?;
            pos += nb;
            let mut v: u64 = 0;
            for &b in bytes {
                v = (v << 8) | b as u64;
            }
            let mask = if prec >= 32 {
                u32::MAX as u64
            } else {
                (1u64 << prec) - 1
            };
            values.push((v & mask) as u32);
        }
    }
    Some(Palette {
        entries: ne,
        columns,
        values,
    })
}

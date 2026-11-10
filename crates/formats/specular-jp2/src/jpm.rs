//! JPM files (JPEG 2000 Part 6, ITU-T T.805 | ISO/IEC 15444-6): compound pages built from
//! layout objects, each an image (or a flat base colour) shown through an opacity mask.
//!
//! A page box holds a page header (size, orientation, page colour) and layout objects in
//! drawing order. A layout object places its image and mask objects in a window of the page;
//! each object points at its codestream (a contiguous codestream box or a fragment table
//! anywhere in the file), carries a JP2 header for it (`ihdr`'s compression type says which
//! coder made it) and may be scaled by a rational factor, offset into, and backed by a base
//! colour. Shared-data boxes (`sdat` / `sref`) stand in for repeated JP2 headers.
//!
//! [`decode_jpm_page`] draws a page as in T.805 clause 5 (equations 1-12) onto 8-bit sRGB:
//! JPEG 2000 objects are decoded here, uncompressed ones (1 or 8 bits) unpacked, and objects
//! of the other coders (MMR, JBIG2, JPEG, ...) handed to [`JpmOptions::foreign`] when given.
//! Scaled objects are resampled bilinearly (centre-aligned; T.805 leaves the method open);
//! with [`JpmOptions::reduce`] each JPEG 2000 object is decoded at the coarsest resolution
//! that still covers its share of the reduced page.

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::boxes::{self, Container, be16, be32};
use crate::geom::try_vec;
use crate::{
    DecodeOptions, Decoder, Depth, Image, Jp2Error, MAX_OUTPUT_PIXELS, Parallelism, PixelFormat,
    Pixels, Result,
};

/// Most layout objects drawn on one page.
const MAX_OBJECTS: usize = 1 << 14;

/// One page of a JPM file (from its page header box).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JpmPage {
    /// Page size in page grid units, before any rotation.
    pub width: u32,
    pub height: u32,
    /// T.805 Table B.11: 0 unspecified, 1 upright, 2/3/4 rotate 90/180/270 degrees clockwise
    /// for a right-reading image ([`decode_jpm_page`] applies it).
    pub orientation: u16,
    /// Number of layout objects the header announces.
    pub layout_objects: u16,
}

/// Header facts of a JPM file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JpmInfo {
    /// The page boxes at the top level of the file, in file order.
    pub pages: Vec<JpmPage>,
    /// Profile of the compound image header (0 none, 1 web).
    pub profile: u8,
}

/// Coder of a JPM object (the `C` field of its `ihdr`, T.805 Table B.24).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JpmCoder {
    Uncompressed,
    /// ITU-T T.4 one-dimensional (MH).
    Mh,
    /// ITU-T T.4 two-dimensional (MR).
    Mr,
    /// ITU-T T.6 (MMR).
    Mmr,
    /// ITU-T T.82 (JBIG), bi-level (4) or any depth (9).
    Jbig,
    /// ITU-T T.81 (JPEG).
    Jpeg,
    /// ITU-T T.87 (JPEG-LS).
    JpegLs,
    /// ISO/IEC 15444-1 (JPEG 2000).
    Jpeg2000,
    /// ITU-T T.88 (JBIG2).
    Jbig2,
    Other(u8),
}

impl JpmCoder {
    fn from_u8(c: u8) -> Self {
        match c {
            0 => JpmCoder::Uncompressed,
            1 => JpmCoder::Mh,
            2 => JpmCoder::Mr,
            3 => JpmCoder::Mmr,
            4 | 9 => JpmCoder::Jbig,
            5 => JpmCoder::Jpeg,
            6 => JpmCoder::JpegLs,
            7 => JpmCoder::Jpeg2000,
            8 => JpmCoder::Jbig2,
            c => JpmCoder::Other(c),
        }
    }

    fn refusal(self) -> &'static str {
        match self {
            JpmCoder::Mh | JpmCoder::Mr => "JPM object coded with fax T.4 (MH/MR)",
            JpmCoder::Mmr => "JPM object coded with fax T.6 (MMR)",
            JpmCoder::Jbig => "JPM object coded with JBIG",
            JpmCoder::Jpeg => "JPM object coded with JPEG",
            JpmCoder::JpegLs => "JPM object coded with JPEG-LS",
            JpmCoder::Jbig2 => "JPM object coded with JBIG2",
            _ => "JPM object with an unknown coder",
        }
    }
}

/// An object coded with something other than JPEG 2000 or no compression, for
/// [`JpmOptions::foreign`].
#[derive(Clone, Copy, Debug)]
pub struct ForeignObject<'a> {
    pub coder: JpmCoder,
    /// The object's codestream.
    pub data: &'a [u8],
    /// From the object's `ihdr`.
    pub width: u32,
    pub height: u32,
    pub components: u16,
    pub bit_depth: u8,
}

/// Decodes a [`ForeignObject`] to a [`PixelFormat::Gray8`] or [`PixelFormat::Rgba8`] image of
/// its size (1-bit data: 0 for white, 255 for black, as it is coded), or `None` when it
/// cannot.
pub type ForeignDecoder<'a> = &'a (dyn Fn(&ForeignObject<'_>) -> Option<Image> + Sync);

#[derive(Clone, Copy, Default)]
pub struct JpmOptions<'a> {
    /// Draw the page at `1 / 2^reduce` of its size (`ceil(size / 2^reduce)`).
    pub reduce: u8,
    pub parallelism: Parallelism,
    /// Checked between layout objects (and inside JPEG 2000 decodes).
    pub cancel: Option<&'a AtomicBool>,
    /// Decoder for objects of the other coders; without it (or when it returns `None`) such a
    /// page is refused with the coder's name.
    pub foreign: Option<ForeignDecoder<'a>>,
}

/// Whether `data` is a JPM file: a JP2-family file whose file type box names `jpm ` as its
/// brand or among its compatible brands.
pub fn is_jpm(data: &[u8]) -> bool {
    if !boxes::is_jp2(data) {
        return false;
    }
    boxes::boxes(&data[12..])
        .find(|(t, _)| t == b"ftyp")
        .is_some_and(|(_, c)| {
            c.get(0..4) == Some(b"jpm ")
                || c.get(8..)
                    .is_some_and(|cl| cl.chunks_exact(4).any(|b| b == b"jpm "))
        })
}

/// The pages of a JPM file (reads box headers and page headers only).
pub fn read_jpm_info(data: &[u8]) -> Result<JpmInfo> {
    if !is_jpm(data) {
        return Err(Jp2Error::NotJp2);
    }
    let mut info = JpmInfo {
        pages: Vec::new(),
        profile: 0,
    };
    for (t, c) in boxes::boxes(&data[12..]) {
        match &t {
            b"mhdr" => info.profile = c.get(4).copied().unwrap_or(0),
            b"page" => {
                let (_, ph) = boxes::boxes(c)
                    .find(|(t, _)| t == b"phdr")
                    .ok_or(Jp2Error::Malformed("page without a page header"))?;
                info.pages.push(parse_phdr(ph)?.page);
            }
            _ => {}
        }
    }
    if info.pages.is_empty() {
        return Err(Jp2Error::Malformed("JPM file without pages"));
    }
    Ok(info)
}

struct Phdr {
    page: JpmPage,
    colour: u16,
}

fn parse_phdr(c: &[u8]) -> Result<Phdr> {
    let short = Jp2Error::Malformed("short page header");
    let page = JpmPage {
        layout_objects: be16(c, 0).ok_or(short.clone())?,
        height: be32(c, 2).ok_or(short.clone())?,
        width: be32(c, 6).ok_or(short.clone())?,
        orientation: be16(c, 10).ok_or(short.clone())?,
    };
    let colour = be16(c, 12).ok_or(short)?;
    if page.width == 0 || page.height == 0 {
        return Err(Jp2Error::Malformed("empty JPM page"));
    }
    Ok(Phdr { page, colour })
}

/// The box lists of a superbox with shared-data references (`sref`) replaced by the boxes
/// they name.
fn children<'a>(c: &'a [u8], shared: &[(u16, &'a [u8])]) -> Vec<([u8; 4], &'a [u8])> {
    let mut v = Vec::new();
    for (t, b) in boxes::boxes(c) {
        if &t == b"sref" {
            if let Some(id) = be16(b, 0)
                && let Some((_, sb)) = shared.iter().find(|(i, _)| *i == id)
            {
                v.extend(boxes::boxes(sb).take(1));
            }
        } else {
            v.push((t, b));
        }
    }
    v
}

/// A colour from a base colour box (`bclr`), as straight 8-bit RGB.
fn base_colour(c: &[u8]) -> Option<[u8; 3]> {
    let mut vals: Option<Vec<i32>> = None;
    let mut depth = 8u32;
    let mut sycc = false;
    for (t, b) in boxes::boxes(c) {
        match &t {
            b"bcvl" => {
                let nc = be16(b, 0)? as usize;
                let bpc = *b.get(2)?;
                if bpc != 255 {
                    depth = (bpc as u32 & 0x7F) + 1;
                }
                vals = Some(
                    (0..nc.min(4))
                        .map(|i| be16(b, 3 + 2 * i).map(|v| v as i16 as i32))
                        .collect::<Option<_>>()?,
                );
            }
            b"colr" => sycc = b.first() == Some(&1) && be32(b, 3) == Some(18),
            _ => {}
        }
    }
    let v = vals?;
    let max = ((1u64 << depth.min(16)) - 1) as f32;
    let s = |x: i32| (x as f32 / max * 255.0).clamp(0.0, 255.0);
    let rgb = match v.len() {
        0 => return None,
        1 | 2 => [s(v[0]); 3],
        _ if sycc => {
            let (y, cb, cr) = (s(v[0]), s(v[1]) - 128.0, s(v[2]) - 128.0);
            [
                y + 1.402 * cr,
                y - 0.344_136 * cb - 0.714_136 * cr,
                y + 1.772 * cb,
            ]
        }
        _ => [s(v[0]), s(v[1]), s(v[2])],
    };
    Some(rgb.map(|x| x.clamp(0.0, 255.0).round() as u8))
}

/// A decoded object: `rgb` (3 bytes per pixel) and/or `mask` (opacity, 255 opaque).
struct Raster {
    w: usize,
    h: usize,
    /// Size at full resolution (`w x h` unless resolution levels were dropped).
    fw: u32,
    fh: u32,
    rgb: Option<Vec<u8>>,
    mask: Option<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Mask,
    Image,
    Both,
}

/// One object box, parsed.
struct Object<'a> {
    role: Role,
    no_codestream: bool,
    /// OVoff, OHoff.
    off: (u32, u32),
    /// VRN, VRD, HRN, HRD.
    scale: [u32; 4],
    base: Option<[u8; 3]>,
    /// `ihdr` and the rest of the JP2 header.
    jp2h: Option<&'a [u8]>,
    cs_off: u64,
    cs_len: u32,
    dr: u16,
}

fn parse_object<'a>(c: &'a [u8], shared: &[(u16, &'a [u8])]) -> Result<Object<'a>> {
    let mut o = Object {
        role: Role::Image,
        no_codestream: true,
        off: (0, 0),
        scale: [1; 4],
        base: None,
        jp2h: None,
        cs_off: 0,
        cs_len: 0,
        dr: 0,
    };
    let mut seen = false;
    for (t, b) in children(c, shared) {
        match &t {
            b"ohdr" => {
                let e = Jp2Error::Malformed("short object header");
                if b.len() < 24 {
                    return Err(e);
                }
                o.role = match b[0] {
                    0 => Role::Mask,
                    1 => Role::Image,
                    2 => Role::Both,
                    _ => return Err(Jp2Error::Unsupported("JPM object of a reserved type")),
                };
                o.no_codestream = b[1] == 1;
                o.off = (be32(b, 2).ok_or(e.clone())?, be32(b, 6).ok_or(e.clone())?);
                o.cs_off = u64::from_be_bytes(b[10..18].try_into().unwrap_or_default());
                o.cs_len = be32(b, 18).ok_or(e.clone())?;
                o.dr = be16(b, 22).ok_or(e)?;
                seen = true;
            }
            b"scal" => {
                for (i, s) in o.scale.iter_mut().enumerate() {
                    *s = be16(b, 2 * i).unwrap_or(1).max(1) as u32;
                }
            }
            b"bclr" => o.base = base_colour(b),
            b"jp2h" => o.jp2h = Some(b),
            _ => {}
        }
    }
    if !seen {
        return Err(Jp2Error::Malformed("object without an object header"));
    }
    Ok(o)
}

/// An object's codestream bytes: a contiguous codestream box or fragment table box at
/// `cs_off` (or the codestream itself there, which some writers point at).
fn codestream<'a>(data: &'a [u8], o: &Object) -> Result<std::borrow::Cow<'a, [u8]>> {
    if o.dr != 0 {
        return Err(Jp2Error::Unsupported("JPM object stored in another file"));
    }
    let off = usize::try_from(o.cs_off)
        .ok()
        .filter(|&p| p < data.len())
        .ok_or(Jp2Error::Truncated("JPM object codestream past the end"))?;
    let at = &data[off..];
    if let Some((t, c)) = boxes::boxes(at).next()
        && (&t == b"jp2c" || &t == b"ftbl")
    {
        return if &t == b"jp2c" {
            Ok(c.into())
        } else {
            boxes::join(data, c).map(Into::into)
        };
    }
    let end = if o.cs_len == 0 {
        at.len()
    } else {
        (o.cs_len as usize).min(at.len())
    };
    Ok(at[..end].into())
}

/// `ihdr` fields: (width, height, components, bits, coder).
fn ihdr(jp2h: &[u8]) -> Result<(u32, u32, u16, u8, JpmCoder)> {
    let (_, c) = boxes::boxes(jp2h)
        .find(|(t, _)| t == b"ihdr")
        .ok_or(Jp2Error::Malformed("JPM object without an image header"))?;
    if c.len() < 14 {
        return Err(Jp2Error::Malformed("short ihdr box"));
    }
    Ok((
        be32(c, 4).unwrap_or(0),
        be32(c, 0).unwrap_or(0),
        be16(c, 8).unwrap_or(0),
        (c[10] & 0x7F) + 1,
        JpmCoder::from_u8(c[11]),
    ))
}

/// How many resolution levels a JPEG 2000 object may drop: objects drawn at `scale`
/// page-grid units per object pixel on a page reduced by `2^reduce`.
fn object_reduce(reduce: u8, scale: [u32; 4]) -> u8 {
    let fx = (1u64 << reduce) as f64 * scale[3] as f64 / scale[2] as f64;
    let fy = (1u64 << reduce) as f64 * scale[1] as f64 / scale[0] as f64;
    let f = fx.min(fy);
    if f < 2.0 {
        0
    } else {
        f.log2().floor().min(32.0) as u8
    }
}

/// Splits a decoded image into colour and opacity for `role`. `bits` is the coded depth (the
/// T.805 mask rule: 1-bit 1 is black = opaque, deeper 0 is black = opaque).
fn to_raster(img: Image, role: Role, bits: u8) -> Result<Raster> {
    let (w, h) = (img.width as usize, img.height as usize);
    let Pixels::U8(px) = img.pixels else {
        return Err(Jp2Error::Malformed("unexpected sample depth"));
    };
    let mask_of = |v: u8| if bits == 1 { v } else { 255 - v };
    let mut r = Raster {
        w,
        h,
        fw: w as u32,
        fh: h as u32,
        rgb: None,
        mask: None,
    };
    match (img.format, role) {
        (PixelFormat::Gray8, Role::Mask) => r.mask = Some(px.iter().map(|&v| mask_of(v)).collect()),
        (PixelFormat::Rgba8, Role::Mask) => {
            r.mask = Some(px.chunks_exact(4).map(|p| mask_of(p[0])).collect())
        }
        (PixelFormat::Gray8, Role::Image) => {
            r.rgb = Some(px.iter().flat_map(|&v| [v; 3]).collect())
        }
        (PixelFormat::Rgba8, Role::Image) => {
            r.rgb = Some(
                px.chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect(),
            )
        }
        (PixelFormat::Rgba8, Role::Both) => {
            r.rgb = Some(
                px.chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect(),
            );
            r.mask = Some(px.chunks_exact(4).map(|p| p[3]).collect());
        }
        _ => {
            return Err(Jp2Error::Unsupported(
                "JPM image-and-mask object without an opacity channel",
            ));
        }
    }
    Ok(r)
}

/// Uncompressed object data (T.805 Table B.24, value 0): `nc` interleaved components of
/// 1 or 8 bits, rows padded to whole bytes.
fn uncompressed(cs: &[u8], w: u32, h: u32, nc: u16, bits: u8, role: Role) -> Result<Raster> {
    let (w, h, nc) = (w as usize, h as usize, nc as usize);
    if !(1..=4).contains(&nc) || !matches!(bits, 1 | 8) {
        return Err(Jp2Error::Unsupported(
            "JPM uncompressed object other than 1 or 8 bits, 1 to 4 components",
        ));
    }
    let n = w.checked_mul(h).filter(|&n| n as u64 <= MAX_OUTPUT_PIXELS);
    let n = n.ok_or(Jp2Error::TooLarge((w * h) as u64))?;
    let row = (w * nc * bits as usize).div_ceil(8);
    if cs.len() < row * h {
        return Err(Jp2Error::Truncated("JPM uncompressed object"));
    }
    let sample = |x: usize, y: usize, c: usize| -> u8 {
        let i = x * nc + c;
        if bits == 8 {
            cs[y * row + i]
        } else if cs[y * row + i / 8] >> (7 - i % 8) & 1 != 0 {
            255
        } else {
            0
        }
    };
    let mut gray: Vec<u8> = try_vec(n * nc)?;
    for y in 0..h {
        for x in 0..w {
            for c in 0..nc {
                gray[(y * w + x) * nc + c] = sample(x, y, c);
            }
        }
    }
    let (format, pixels) = match (nc, role) {
        (1, _) => (PixelFormat::Gray8, gray),
        (2, Role::Both) => (
            PixelFormat::Rgba8,
            gray.chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
        ),
        (3, _) => (
            PixelFormat::Rgba8,
            gray.chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
        ),
        (4, Role::Both) => (PixelFormat::Rgba8, gray),
        _ => (
            PixelFormat::Rgba8,
            gray.chunks_exact(nc)
                .flat_map(|p| [p[0], p[0], p[0], 255])
                .collect(),
        ),
    };
    let img = Image {
        width: w as u32,
        height: h as u32,
        origin: (0, 0),
        format,
        pixels: Pixels::U8(pixels),
        truncated: false,
        reduce: 0,
    };
    // The opacity component of an image-and-mask object follows the T.805 mask rule too.
    let mut r = to_raster(img, role, bits)?;
    if role == Role::Both
        && bits > 1
        && let Some(m) = &mut r.mask
    {
        m.iter_mut().for_each(|v| *v = 255 - *v);
    }
    Ok(r)
}

/// Decodes an object at `reduce` discarded JPEG 2000 levels (other coders ignore it). Returns
/// the raster, the levels actually discarded and whether data was missing.
fn decode_object(
    data: &[u8],
    o: &Object,
    reduce: u8,
    opts: &JpmOptions,
) -> Result<(Raster, u8, bool)> {
    let jp2h = o
        .jp2h
        .ok_or(Jp2Error::Malformed("JPM object without a JP2 header"))?;
    let (w, h, nc, bits, coder) = ihdr(jp2h)?;
    let cs = codestream(data, o)?;
    match coder {
        JpmCoder::Jpeg2000 => {
            let mut bx = boxes::empty(Container::Jp2);
            boxes::parse_jp2h(jp2h, &mut bx)?;
            bx.codestream = &cs;
            let dec = Decoder::from_boxes(bx)?;
            let r = reduce.min(dec.info().levels);
            let img = dec.decode(&DecodeOptions {
                reduce: r,
                region: None,
                max_layers: None,
                depth: Depth::Eight,
                parallelism: opts.parallelism,
                cancel: opts.cancel,
            })?;
            let truncated = img.truncated;
            let bits = dec.info().components.first().map_or(bits, |c| c.precision);
            let mut ras = to_raster(img, o.role, bits)?;
            (ras.fw, ras.fh) = (dec.info().width, dec.info().height);
            Ok((ras, r, truncated))
        }
        JpmCoder::Uncompressed => Ok((uncompressed(&cs, w, h, nc, bits, o.role)?, 0, false)),
        c => {
            let obj = ForeignObject {
                coder: c,
                data: &cs,
                width: w,
                height: h,
                components: nc,
                bit_depth: bits,
            };
            let img = opts
                .foreign
                .and_then(|f| f(&obj))
                .filter(|i| i.width == w && i.height == h)
                .ok_or(Jp2Error::Unsupported(c.refusal()))?;
            Ok((to_raster(img, o.role, bits)?, 0, false))
        }
    }
}

/// Where each output column (or row) of the page samples a placed object, along one axis.
struct Axis {
    /// Per output index: `None` outside the layout window, else `Some(None)` inside the window
    /// but outside the object, or `Some(Some((i0, i1, frac)))` bilinear taps.
    taps: Vec<Option<Option<(u32, u32, f32)>>>,
}

/// `out`: output pixels; `page`: page size in grid units; `r`: page reduce; `win`: layout window (offset, length) in page grid
/// units; `obj_off`: the object's offset into its scaled self; `num/den`: page-grid units per
/// object pixel; `full`: object pixels at full resolution; `n`: pixels decoded, with `k`
/// resolution levels dropped.
#[allow(clippy::too_many_arguments)]
fn axis(
    out: usize,
    page: f64,
    r: u8,
    win: (u32, u32),
    obj_off: u32,
    num: u32,
    den: u32,
    full: u32,
    n: usize,
    k: u8,
) -> Axis {
    let step = (1u64 << r) as f64;
    let (wo, wl) = (win.0 as f64, win.1 as f64);
    // Size of the scaled object in page grid units.
    let scaled = full as f64 * num as f64 / den as f64;
    let taps = (0..out)
        .map(|x| {
            // Inside tests use the centre of the part of the page the output pixel covers
            // (the last one may hang over the page edge); sampling uses its nominal centre,
            // which lands on whole pixels of an object reduced alike.
            let p = (x as f64 + 0.5) * step;
            let pc = (x as f64 * step + ((x + 1) as f64 * step).min(page)) / 2.0;
            if pc < wo || pc >= wo + wl {
                return None;
            }
            if pc - wo + obj_off as f64 >= scaled || n == 0 {
                return Some(None);
            }
            let u = p - wo + obj_off as f64;
            // Object pixel centres sit at (i + 0.5) in object units.
            let s = (u * den as f64 / num as f64 / (1u64 << k) as f64 - 0.5).max(0.0);
            let i0 = (s.floor() as usize).min(n - 1);
            let i1 = (i0 + 1).min(n - 1);
            Some(Some((
                i0 as u32,
                i1 as u32,
                (s - i0 as f64).min(1.0) as f32,
            )))
        })
        .collect();
    Axis { taps }
}

/// A layout object ready to draw.
struct Placed {
    xs: Axis,
    ys: Axis,
    /// Mask placement (when the mask is a separate object with its own scale and offset).
    mask: Option<(Raster, Axis, Axis)>,
    image: Option<Raster>,
    base: [u8; 3],
}

#[inline]
fn bilerp(
    v: &[u8],
    stride: usize,
    ch: usize,
    c: usize,
    x: (u32, u32, f32),
    y: (u32, u32, f32),
) -> f32 {
    let at = |xx: u32, yy: u32| v[(yy as usize * stride + xx as usize) * ch + c] as f32;
    let top = at(x.0, y.0) + (at(x.1, y.0) - at(x.0, y.0)) * x.2;
    let bot = at(x.0, y.1) + (at(x.1, y.1) - at(x.0, y.1)) * x.2;
    top + (bot - top) * y.2
}

/// Draws one layout object over the row `y` of the page (`row`: RGBA).
fn draw_row(p: &Placed, y: usize, row: &mut [u8]) {
    let Some(ty) = p.ys.taps[y] else { return };
    let mty = p.mask.as_ref().map(|(_, _, my)| my.taps[y]);
    for (x, px) in row.chunks_exact_mut(4).enumerate() {
        let Some(tx) = p.xs.taps[x] else { continue };
        // Opacity: the mask object's sample, opaque outside it or without one.
        let m = match (&p.mask, mty) {
            (Some((mr, mx, _)), Some(my)) => match (mx.taps[x], my) {
                (Some(Some(sx)), Some(Some(sy))) => {
                    let v = mr.mask.as_deref().unwrap_or(&[]);
                    bilerp(v, mr.w, 1, 0, sx, sy)
                }
                (Some(_), Some(_)) => 255.0,
                _ => 0.0,
            },
            _ => 255.0,
        };
        if m <= 0.0 {
            continue;
        }
        let colour = |c: usize| -> f32 {
            match (&p.image, tx, ty) {
                (Some(im), Some(sx), Some(sy)) => match &im.rgb {
                    Some(rgb) => bilerp(rgb, im.w, 3, c, sx, sy),
                    None => p.base[c] as f32,
                },
                _ => p.base[c] as f32,
            }
        };
        let m = m.min(255.0) / 255.0;
        let a = px[3] as f32 / 255.0;
        let oa = m + (1.0 - m) * a;
        for (c, v) in px[..3].iter_mut().enumerate() {
            let x = (m * colour(c) + (1.0 - m) * a * *v as f32) / oa;
            *v = x.round().clamp(0.0, 255.0) as u8;
        }
        px[3] = (oa * 255.0).round() as u8;
    }
}

/// Draw page `page` (0-based, in file order) of a JPM file as 8-bit RGBA.
pub fn decode_jpm_page(data: &[u8], page: usize, opts: &JpmOptions) -> Result<Image> {
    if !is_jpm(data) {
        return Err(Jp2Error::NotJp2);
    }
    let top: Vec<_> = boxes::boxes(&data[12..]).collect();
    let shared: Vec<(u16, &[u8])> = top
        .iter()
        .filter(|(t, _)| t == b"sdat")
        .filter_map(|(_, c)| Some((be16(c, 0)?, c.get(2..)?)))
        .collect();
    let (_, pbox) = top
        .iter()
        .filter(|(t, _)| t == b"page")
        .nth(page)
        .ok_or(Jp2Error::InvalidRequest("no such JPM page"))?;
    let kids = children(pbox, &shared);
    let ph = kids
        .iter()
        .find(|(t, _)| t == b"phdr")
        .ok_or(Jp2Error::Malformed("page without a page header"))?;
    let ph = parse_phdr(ph.1)?;
    let r = opts.reduce.min(31);
    let (pw, phh) = (ph.page.width as f64, ph.page.height as f64);
    let ow = (ph.page.width as u64).div_ceil(1 << r) as usize;
    let oh = (ph.page.height as u64).div_ceil(1 << r) as usize;
    if ow as u64 * oh as u64 > MAX_OUTPUT_PIXELS {
        return Err(Jp2Error::TooLarge(ow as u64 * oh as u64 * 4));
    }
    let _res = specular_core::budget::reserve_n(ow as u64 * oh as u64, 4)?;
    let mut out: Vec<u8> = try_vec(ow * oh * 4)?;
    let bg: Option<[u8; 3]> = match ph.colour {
        0 => None,
        1 => Some([255; 3]),
        2 => Some([0; 3]),
        255 => Some(
            kids.iter()
                .find(|(t, _)| t == b"bclr")
                .and_then(|(_, c)| base_colour(c))
                .ok_or(Jp2Error::Malformed("page colour without a base colour box"))?,
        ),
        _ => return Err(Jp2Error::Unsupported("JPM page colour of a reserved kind")),
    };
    if let Some(c) = bg {
        for px in out.chunks_exact_mut(4) {
            px.copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }

    // Layout objects in drawing order (increasing LObjID; 0 is the page thumbnail).
    let mut lobjs: Vec<(u16, &[u8], &[u8])> = Vec::new();
    for (t, c) in &kids {
        if t != b"lobj" {
            continue;
        }
        let lk = children(c, &shared);
        let Some((_, lh)) = lk.iter().find(|(t, _)| t == b"lhdr") else {
            return Err(Jp2Error::Malformed("layout object without a header"));
        };
        let id = be16(lh, 0).ok_or(Jp2Error::Malformed("short layout object header"))?;
        if id != 0 && lobjs.len() < MAX_OBJECTS {
            lobjs.push((id, lh, c));
        }
    }
    lobjs.sort_by_key(|l| l.0);

    let mut truncated = false;
    let run = |out: &mut Vec<u8>, truncated: &mut bool| -> Result<()> {
        for &(_, lh, lc) in &lobjs {
            if opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(Jp2Error::Cancelled);
            }
            let e = Jp2Error::Malformed("short layout object header");
            let lheight = be32(lh, 2).ok_or(e.clone())?;
            let lwidth = be32(lh, 6).ok_or(e.clone())?;
            let lvoff = be32(lh, 10).ok_or(e.clone())?;
            let lhoff = be32(lh, 14).ok_or(e)?;
            let objs: Vec<Object> = children(lc, &shared)
                .iter()
                .filter(|(t, _)| t == b"objc")
                .map(|(_, c)| parse_object(c, &shared))
                .collect::<Result<_>>()?;
            if lwidth == 0 || lheight == 0 {
                continue;
            }
            let mut placed = Placed {
                xs: Axis { taps: Vec::new() },
                ys: Axis { taps: Vec::new() },
                mask: None,
                image: None,
                base: [0; 3],
            };
            let mut have_axes = false;
            for o in &objs {
                if o.role != Role::Mask {
                    placed.base = o.base.unwrap_or([0; 3]);
                }
                if o.no_codestream {
                    if o.role != Role::Image {
                        return Err(Jp2Error::Malformed("JPM mask object without a codestream"));
                    }
                    continue;
                }
                let k = object_reduce(r, o.scale);
                let (ras, k, tr) = decode_object(data, o, k, opts)?;
                *truncated |= tr;
                let [vrn, vrd, hrn, hrd] = o.scale;
                let xs = axis(
                    ow,
                    pw,
                    r,
                    (lhoff, lwidth),
                    o.off.1,
                    hrn,
                    hrd,
                    ras.fw,
                    ras.w,
                    k,
                );
                let ys = axis(
                    oh,
                    phh,
                    r,
                    (lvoff, lheight),
                    o.off.0,
                    vrn,
                    vrd,
                    ras.fh,
                    ras.h,
                    k,
                );
                match o.role {
                    Role::Mask => {
                        let (mx, my) = (xs, ys);
                        placed.mask = Some((ras, mx, my));
                    }
                    Role::Image => {
                        placed.xs = xs;
                        placed.ys = ys;
                        placed.image = Some(ras);
                        have_axes = true;
                    }
                    Role::Both => {
                        let mx = Axis {
                            taps: xs.taps.clone(),
                        };
                        let my = Axis {
                            taps: ys.taps.clone(),
                        };
                        let mask = Raster {
                            w: ras.w,
                            h: ras.h,
                            fw: ras.fw,
                            fh: ras.fh,
                            rgb: None,
                            mask: ras.mask.clone(),
                        };
                        placed.mask = Some((mask, mx, my));
                        placed.xs = xs;
                        placed.ys = ys;
                        placed.image = Some(ras);
                        have_axes = true;
                    }
                }
            }
            if !have_axes {
                // No image: the base colour fills the layout window.
                placed.xs = axis(ow, pw, r, (lhoff, lwidth), 0, 1, 1, 0, 0, 0);
                placed.ys = axis(oh, phh, r, (lvoff, lheight), 0, 1, 1, 0, 0, 0);
            }
            out.par_chunks_mut(ow * 4)
                .enumerate()
                .for_each(|(y, row)| draw_row(&placed, y, row));
        }
        Ok(())
    };
    crate::with_threads(opts.parallelism, (ow * oh) as u64, || {
        run(&mut out, &mut truncated)
    })?;

    let (mut w, mut h) = (ow, oh);
    if matches!(ph.page.orientation, 2..=4) {
        out = rotate(&out, w, h, ph.page.orientation)?;
        if ph.page.orientation != 3 {
            std::mem::swap(&mut w, &mut h);
        }
    }
    Ok(Image {
        width: w as u32,
        height: h as u32,
        origin: (0, 0),
        format: PixelFormat::Rgba8,
        pixels: Pixels::U8(out),
        truncated,
        reduce: r,
    })
}

/// `src` (`w x h` RGBA) turned clockwise by 90 (2), 180 (3) or 270 (4) degrees.
fn rotate(src: &[u8], w: usize, h: usize, or: u16) -> Result<Vec<u8>> {
    let mut dst: Vec<u8> = try_vec(src.len())?;
    let dw = if or == 3 { w } else { h };
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match or {
                2 => (h - 1 - y, x),
                3 => (w - 1 - x, h - 1 - y),
                _ => (y, w - 1 - x),
            };
            let s = (y * w + x) * 4;
            let d = (dy * dw + dx) * 4;
            dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    Ok(dst)
}

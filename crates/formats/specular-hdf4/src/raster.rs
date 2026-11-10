//! Raster images: GR (`RI0.0` Vgroups), raster image groups (RIG, 8- and 24-bit) and the old
//! 8-bit RI8 tags, with their palettes.

use std::collections::HashSet;

use crate::codec;
use crate::dd::{
    TAG_CI, TAG_CI8, TAG_ID, TAG_ID8, TAG_II8, TAG_IP8, TAG_LUT, TAG_RI, TAG_RI8, TAG_RIG,
};
use crate::elem;
use crate::nt::Nt;
use crate::vobj::attr_of;
use crate::{Attr, Ctx, Error, File, Result, bad, be16, bei32, limits};

/// Compression tags of raster image descriptions.
const COMP_RLE: u16 = 11;
const COMP_IMCOMP: u16 = 12;
const COMP_JPEG_OLD: u16 = 13;
const COMP_GREYJPEG_OLD: u16 = 14;
const COMP_JPEG: u16 = 15;
const COMP_GREYJPEG: u16 = 16;

/// A palette: up to 256 RGB entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub entries: Vec<[u8; 3]>,
}

/// A raster image.
#[derive(Clone, Debug)]
pub struct Image {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Components per pixel.
    pub ncomp: u32,
    pub nt: Nt,
    /// 0 pixel, 1 line, 2 plane interlace.
    pub interlace: u16,
    pub attrs: Vec<Attr>,
    pub palette: Option<Palette>,
    /// Read through the GR interface (an `RI0.0` Vgroup); otherwise a RIG or RI8 image.
    pub gr: bool,
    /// The compression tag of the description (0 none, 11 RLE, 15/16 JPEG, …).
    pub comp: u16,
    /// The data element (tag, ref); `None` when never written.
    pub(crate) data: Option<(u16, u16)>,
}

impl Image {
    /// "RLE", "JPEG", "deflate", … when compressed.
    pub fn compression(&self, f: &File, ctx: Ctx) -> Option<String> {
        match self.comp {
            0 => {
                let (t, r) = self.data?;
                elem::store(f, ctx, t, r)
                    .ok()?
                    .coder()
                    .map(|c| c.name().to_owned())
            }
            COMP_RLE => Some("RLE".into()),
            COMP_JPEG | COMP_GREYJPEG | COMP_JPEG_OLD | COMP_GREYJPEG_OLD => Some("JPEG".into()),
            COMP_IMCOMP => Some("IMCOMP".into()),
            c => Some(format!("compression {c}")),
        }
    }

    /// Whether the samples come from a lossy JPEG stream.
    pub fn is_jpeg(&self) -> bool {
        matches!(self.comp, COMP_JPEG | COMP_GREYJPEG)
    }
}

/// An image's samples.
#[derive(Clone, Debug)]
pub struct ImageData {
    pub width: usize,
    pub height: usize,
    pub ncomp: usize,
    /// Stored numbers in [y][x][component] order.
    pub values: Vec<f64>,
}

/// An image description (ID element): size, number type, components, interlace, compression.
struct Desc {
    w: i32,
    h: i32,
    nt_ref: u16,
    ncomp: i16,
    il: i16,
    comp: u16,
}

fn desc(b: &[u8]) -> Option<Desc> {
    Some(Desc {
        w: bei32(b, 0)?,
        h: bei32(b, 4)?,
        nt_ref: be16(b, 10)?,
        ncomp: be16(b, 12)? as i16,
        il: be16(b, 14)? as i16,
        comp: be16(b, 16)?,
    })
}

fn palette_of(b: &[u8]) -> Option<Palette> {
    if b.len() < 3 {
        return None;
    }
    Some(Palette {
        entries: b
            .chunks_exact(3)
            .take(256)
            .map(|c| [c[0], c[1], c[2]])
            .collect(),
    })
}

fn image_from(
    f: &File,
    ctx: Ctx,
    name: String,
    members: &[(u16, u16)],
    gr: bool,
) -> Result<Option<Image>> {
    let find = |tag: u16| members.iter().find(|m| m.0 == tag).map(|m| m.1);
    let Some(id) = find(TAG_ID) else {
        return Ok(None);
    };
    let d = f
        .dds
        .raw(ctx.data, TAG_ID, id)
        .and_then(desc)
        .ok_or_else(|| bad("image description"))?;
    if d.w <= 0 || d.h <= 0 || !(1..=64).contains(&d.ncomp) {
        return Err(bad("image size"));
    }
    let nt = match f.dds.raw(ctx.data, crate::dd::TAG_NT, d.nt_ref) {
        Some(b) => Nt::from_element(b)?,
        None => Nt::from_code(21)?,
    };
    let data = find(TAG_RI)
        .map(|r| (TAG_RI, r))
        .or_else(|| find(TAG_CI).map(|r| (TAG_CI, r)))
        .filter(|&(t, r)| f.dds.has(t, r));
    let palette = find(TAG_LUT)
        .and_then(|r| f.dds.raw(ctx.data, TAG_LUT, r))
        .and_then(palette_of);
    Ok(Some(Image {
        name,
        width: d.w as u32,
        height: d.h as u32,
        ncomp: d.ncomp as u32,
        nt,
        interlace: d.il.clamp(0, 2) as u16,
        attrs: Vec::new(),
        palette,
        gr,
        comp: d.comp,
        data,
    }))
}

pub(crate) fn scan(f: &mut File, ctx: Ctx) -> Result<()> {
    let mut images = Vec::new();
    let mut used: HashSet<(u16, u16)> = HashSet::new();
    let mut warnings = Vec::new();
    let mut file_attrs = Vec::new();
    // GR numbers the unnamed images ("Raster Image #n") by their place among all the
    // candidates it found, GR images and RIGs duplicating them included.
    let mut candidates = 0usize;
    // The GR interface finds its Vgroup by name (`Vfind`); early versions left the class
    // empty.
    if let Some(top) = f
        .vgroups
        .iter()
        .find(|v| v.class == "RIG0.0" || v.name == "RIG0.0")
    {
        for &(tag, rf) in &top.members {
            if images.len() >= limits::MAX_ITEMS {
                break;
            }
            if tag == crate::dd::TAG_VH {
                if let Some(vd) = f.vdata(rf)
                    && vd.class.starts_with("RIATTR0.0")
                    && let Some(a) = attr_of(f, ctx, vd, true)
                {
                    file_attrs.push(a);
                }
                continue;
            }
            let Some(vg) = f
                .vgroup(rf)
                .filter(|v| tag == crate::dd::TAG_VG && v.class == "RI0.0")
            else {
                continue;
            };
            candidates += 1;
            match image_from(f, ctx, vg.name.clone(), &vg.members, true) {
                Ok(Some(mut im)) => {
                    im.attrs = vg
                        .child_vdatas()
                        .filter_map(|r| f.vdata(r))
                        .filter(|v| v.class.starts_with("RIATTR0.0"))
                        .take(limits::MAX_ATTRS)
                        .filter_map(|v| attr_of(f, ctx, v, true))
                        .collect();
                    if let Some(d) = im.data {
                        used.insert(d);
                    }
                    images.push(im);
                }
                Ok(None) => {}
                Err(e) => warnings.push(format!("image {}: {e}", vg.name)),
            }
        }
    }
    let rigs: Vec<u16> = f.dds.refs(TAG_RIG).collect();
    for rf in &rigs {
        if images.len() >= limits::MAX_ITEMS {
            break;
        }
        let Some(b) = f.dds.raw(ctx.data, TAG_RIG, *rf) else {
            continue;
        };
        let members: Vec<(u16, u16)> = b
            .chunks_exact(4)
            .map(|c| {
                (
                    u16::from_be_bytes([c[0], c[1]]),
                    u16::from_be_bytes([c[2], c[3]]),
                )
            })
            .collect();
        let name = format!("Raster Image #{candidates}");
        candidates += 1;
        match image_from(f, ctx, name, &members, false) {
            Ok(Some(im)) => {
                if im.data.is_some_and(|d| used.contains(&d)) {
                    continue; // the RIG duplicating a GR image
                }
                if let Some(d) = im.data {
                    used.insert(d);
                }
                images.push(im);
            }
            Ok(None) => {}
            Err(e) => warnings.push(format!("raster image group {rf}: {e}")),
        }
    }
    // Old 8-bit images (ID8, RI8 / CI8 / II8, IP8) that no RIG already gives: RIG writers
    // store the data once under both tags, so duplicates are found by offset.
    let mut used_at: HashSet<u32> = used
        .iter()
        .filter_map(|&(tag, rf)| f.dds.get(tag, rf))
        .map(|d| d.offset)
        .collect();
    let old: Vec<(u16, u16, u32)> = f
        .dds
        .list
        .iter()
        .filter(|d| matches!(d.base_tag(), TAG_RI8 | TAG_CI8 | TAG_II8))
        .map(|d| (d.base_tag(), d.rf, d.offset))
        .collect();
    for (tag, rf, offset) in old {
        if images.len() >= limits::MAX_ITEMS {
            break;
        }
        {
            let Some(id) = f.dds.raw(ctx.data, TAG_ID8, rf) else {
                continue;
            };
            let (Some(w), Some(h)) = (be16(id, 0), be16(id, 2)) else {
                continue;
            };
            let comp = match tag {
                TAG_RI8 => 0,
                TAG_CI8 => COMP_RLE,
                _ => COMP_IMCOMP,
            };
            let data = (tag, rf);
            if w == 0 || h == 0 || used.contains(&data) || !used_at.insert(offset) {
                continue;
            }
            used.insert(data);
            let name = format!("Raster Image #{candidates}");
            candidates += 1;
            images.push(Image {
                name,
                width: u32::from(w),
                height: u32::from(h),
                ncomp: 1,
                nt: Nt::from_code(21)?,
                interlace: 0,
                attrs: Vec::new(),
                palette: f.dds.raw(ctx.data, TAG_IP8, rf).and_then(palette_of),
                gr: false,
                comp,
                data: Some(data),
            });
        }
    }
    f.images = images;
    f.image_attrs = file_attrs;
    f.warnings.extend(warnings);
    Ok(())
}

pub(crate) fn read(f: &File, ctx: Ctx, im: &Image) -> Result<ImageData> {
    let (w, h, c) = (im.width as usize, im.height as usize, im.ncomp as usize);
    let n = w
        .checked_mul(h)
        .and_then(|x| x.checked_mul(c))
        .ok_or(Error::TooLarge)?;
    let size = im.nt.size();
    if n.checked_mul(size.max(8))
        .is_none_or(|b| b as u64 > limits::MAX_ELEMENT)
    {
        return Err(Error::TooLarge);
    }
    // The f64 values and the samples they come from.
    let _res = specular_core::budget::reserve_n(n as u64, 8 + size as u64)?;
    let Some((tag, rf)) = im.data else {
        return Ok(ImageData {
            width: w,
            height: h,
            ncomp: c,
            values: vec![0.0; n],
        });
    };
    let st = elem::store(f, ctx, tag, rf)?;
    let raw = elem::read(f, ctx, &st, (tag, rf))?;
    let bytes: Vec<u8> = match im.comp {
        0 => raw.into_owned(),
        COMP_RLE => codec::raster_rle(&raw, n * size),
        COMP_JPEG | COMP_GREYJPEG => return jpeg(&raw, w, h, c),
        COMP_IMCOMP => return Err(Error::Unsupported("IMCOMP raster image".into())),
        COMP_JPEG_OLD | COMP_GREYJPEG_OLD => {
            return Err(Error::Unsupported("old-style JPEG raster image".into()));
        }
        k => return Err(Error::Unsupported(format!("raster compression {k}"))),
    };
    let get = |i: usize| -> f64 {
        let s = i * size;
        bytes.get(s..s + size).map_or(0.0, |b| im.nt.get(b))
    };
    let mut values = vec![0.0; n];
    for y in 0..h {
        for x in 0..w {
            for k in 0..c {
                let i = match im.interlace {
                    1 => (y * c + k) * w + x,
                    2 => (k * h + y) * w + x,
                    _ => (y * w + x) * c + k,
                };
                values[(y * w + x) * c + k] = get(i);
            }
        }
    }
    Ok(ImageData {
        width: w,
        height: h,
        ncomp: c,
        values,
    })
}

fn jpeg(b: &[u8], w: usize, h: usize, c: usize) -> Result<ImageData> {
    let p = specular_jpeg::decode(b, &specular_jpeg::DecodeOptions::default())
        .map_err(|e| bad(&format!("JPEG raster image: {e}")))?;
    if p.width as usize != w || p.height as usize != h {
        return Err(bad("JPEG raster image size"));
    }
    let rgba = specular_jpeg::planes_to_rgba(&p);
    let mut values = Vec::with_capacity(w * h * c);
    for px in rgba.chunks_exact(4) {
        for k in 0..c {
            values.push(f64::from(px[k.min(2)]));
        }
    }
    Ok(ImageData {
        width: w,
        height: h,
        ncomp: c,
        values,
    })
}

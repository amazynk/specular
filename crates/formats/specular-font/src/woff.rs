//! WOFF 1.0 (zlib per table) and WOFF 2.0 (one Brotli stream, transformed `glyf`/`loca`
//! and `hmtx`, collections) unwrapped to standalone SFNT fonts.

use crate::read::{Reader, u16_at};
use crate::sfnt::{self, TTCF};
use crate::{Error, Limits};

/// What a WOFF file unwraps to: one SFNT per font, its metadata XML and private block size.
pub(crate) struct Unwrapped {
    pub fonts: Vec<Vec<u8>>,
    pub metadata: Option<String>,
    pub private_len: u32,
}

fn metadata(data: &[u8], off: u32, len: u32, orig: u32, limits: &Limits) -> Option<String> {
    if len == 0 || orig as usize > limits.max_table_bytes {
        return None;
    }
    let comp = crate::read::slice(data, off as usize, len as usize)?;
    let raw = specular_png::inflate_zlib(comp, orig as usize).ok()?;
    Some(String::from_utf8_lossy(&raw).into_owned())
}

fn metadata2(data: &[u8], off: u32, len: u32, orig: u32, limits: &Limits) -> Option<String> {
    if len == 0 || orig as usize > limits.max_table_bytes {
        return None;
    }
    let comp = crate::read::slice(data, off as usize, len as usize)?;
    let raw = specular_brotli::decompress(comp, orig as usize).ok()?;
    Some(String::from_utf8_lossy(&raw).into_owned())
}

pub(crate) fn woff1(data: &[u8], limits: &Limits) -> Result<Unwrapped, Error> {
    let mut r = Reader::new(data);
    if r.tag() != Some(*b"wOFF") {
        return Err(Error::NotAFont);
    }
    let flavor = r.u32().ok_or(Error::Truncated)?;
    if flavor == TTCF {
        return Err(Error::WoffCollection);
    }
    let _length = r.u32().ok_or(Error::Truncated)?;
    let n = r.u16().ok_or(Error::Truncated)?;
    r.skip(2).ok_or(Error::Truncated)?;
    let total = r.u32().ok_or(Error::Truncated)?;
    r.skip(4).ok_or(Error::Truncated)?;
    let (mo, ml, mol) = (
        r.u32().ok_or(Error::Truncated)?,
        r.u32().ok_or(Error::Truncated)?,
        r.u32().ok_or(Error::Truncated)?,
    );
    let _po = r.u32().ok_or(Error::Truncated)?;
    let pl = r.u32().ok_or(Error::Truncated)?;
    if n == 0 {
        return Err(Error::Damaged("WOFF"));
    }
    if n > sfnt::MAX_TABLES {
        return Err(Error::TooManyTables);
    }
    if total as usize > limits.max_sfnt_bytes {
        return Err(Error::TooLarge);
    }
    let mut tables = Vec::with_capacity(n as usize);
    let mut used = 0usize;
    for _ in 0..n {
        let tag = r.tag().ok_or(Error::Truncated)?;
        let off = r.u32().ok_or(Error::Truncated)? as usize;
        let comp = r.u32().ok_or(Error::Truncated)? as usize;
        let orig = r.u32().ok_or(Error::Truncated)? as usize;
        let _sum = r.u32().ok_or(Error::Truncated)?;
        if orig > limits.max_table_bytes {
            return Err(Error::TooLarge);
        }
        used = used.saturating_add(orig);
        if used > limits.max_sfnt_bytes {
            return Err(Error::TooLarge);
        }
        let src = crate::read::slice(data, off, comp).ok_or(Error::Truncated)?;
        let bytes = if comp < orig {
            let out = specular_png::inflate_zlib(src, orig).map_err(|_| Error::Damaged("WOFF"))?;
            if out.len() != orig {
                return Err(Error::Damaged("WOFF"));
            }
            out
        } else if comp == orig {
            src.to_vec()
        } else {
            return Err(Error::Damaged("WOFF"));
        };
        tables.push((tag, bytes));
    }
    Ok(Unwrapped {
        fonts: vec![sfnt::write(flavor, tables)],
        metadata: metadata(data, mo, ml, mol, limits),
        private_len: pl,
    })
}

const KNOWN_TAGS: [&[u8; 4]; 63] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post", b"cvt ", b"fpgm",
    b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT", b"EBLC", b"gasp", b"hdmx", b"kern",
    b"LTSH", b"PCLT", b"VDMX", b"vhea", b"vmtx", b"BASE", b"GDEF", b"GPOS", b"GSUB", b"EBSC",
    b"JSTF", b"MATH", b"CBDT", b"CBLC", b"COLR", b"CPAL", b"SVG ", b"sbix", b"acnt", b"avar",
    b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc", b"feat", b"fmtx", b"fvar", b"gvar", b"hsty",
    b"just", b"lcar", b"mort", b"morx", b"opbd", b"prop", b"trak", b"Zapf", b"Silf", b"Glat",
    b"Gloc", b"Feat", b"Sill",
];

fn base128(r: &mut Reader<'_>) -> Option<u32> {
    let mut v = 0u32;
    for i in 0..5 {
        let b = r.u8()?;
        if i == 0 && b == 0x80 {
            return None;
        }
        if v & 0xFE00_0000 != 0 {
            return None;
        }
        v = (v << 7) | (b & 0x7F) as u32;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn u255(r: &mut Reader<'_>) -> Option<u16> {
    let c = r.u8()?;
    Some(match c {
        253 => r.u16()?,
        255 => r.u8()? as u16 + 253,
        254 => r.u8()? as u16 + 506,
        c => c as u16,
    })
}

struct Entry {
    tag: [u8; 4],
    orig: u32,
    /// Bytes in the decompressed stream.
    len: u32,
    transformed: bool,
    /// Offset in the decompressed stream.
    at: usize,
}

pub(crate) fn woff2(data: &[u8], limits: &Limits) -> Result<Unwrapped, Error> {
    let bad = Error::Damaged("WOFF2");
    let mut r = Reader::new(data);
    if r.tag() != Some(*b"wOF2") {
        return Err(Error::NotAFont);
    }
    let flavor = r.u32().ok_or(Error::Truncated)?;
    let _length = r.u32().ok_or(Error::Truncated)?;
    let n = r.u16().ok_or(Error::Truncated)?;
    r.skip(2).ok_or(Error::Truncated)?;
    let total = r.u32().ok_or(Error::Truncated)?;
    let comp_size = r.u32().ok_or(Error::Truncated)?;
    r.skip(4).ok_or(Error::Truncated)?;
    let (mo, ml, mol) = (
        r.u32().ok_or(Error::Truncated)?,
        r.u32().ok_or(Error::Truncated)?,
        r.u32().ok_or(Error::Truncated)?,
    );
    let _po = r.u32().ok_or(Error::Truncated)?;
    let pl = r.u32().ok_or(Error::Truncated)?;
    if n == 0 {
        return Err(bad);
    }
    if n > sfnt::MAX_TABLES {
        return Err(Error::TooManyTables);
    }
    if total as usize > limits.max_sfnt_bytes {
        return Err(Error::TooLarge);
    }
    let mut entries = Vec::with_capacity(n as usize);
    let mut at = 0usize;
    for _ in 0..n {
        let flags = r.u8().ok_or(Error::Truncated)?;
        let tag = match flags & 0x3F {
            63 => r.tag().ok_or(Error::Truncated)?,
            i => *KNOWN_TAGS[i as usize],
        };
        let version = flags >> 6;
        let orig = base128(&mut r).ok_or(Error::Truncated)?;
        let glyf_loca = &tag == b"glyf" || &tag == b"loca";
        let transformed = if glyf_loca {
            version == 0
        } else {
            version != 0
        };
        if transformed && !(glyf_loca || &tag == b"hmtx") {
            return Err(Error::UnknownTransform);
        }
        if transformed && &tag == b"hmtx" && version != 1 {
            return Err(Error::UnknownTransform);
        }
        let len = if transformed {
            base128(&mut r).ok_or(Error::Truncated)?
        } else {
            orig
        };
        if orig as usize > limits.max_table_bytes || len as usize > limits.max_table_bytes {
            return Err(Error::TooLarge);
        }
        entries.push(Entry {
            tag,
            orig,
            len,
            transformed,
            at,
        });
        at = at.checked_add(len as usize).ok_or(Error::TooLarge)?;
        if at > limits.max_sfnt_bytes {
            return Err(Error::TooLarge);
        }
    }
    // Fonts: the table indices of each.
    let fonts: Vec<(u32, Vec<usize>)> = if flavor == TTCF {
        let _version = r.u32().ok_or(Error::Truncated)?;
        let nf = u255(&mut r).ok_or(Error::Truncated)?;
        if nf == 0 {
            return Err(Error::NoFaces);
        }
        if u32::from(nf) > crate::sfnt::MAX_FACES {
            return Err(Error::TooManyFaces);
        }
        let mut fonts = Vec::new();
        for _ in 0..nf {
            let nt = u255(&mut r).ok_or(Error::Truncated)?;
            let fl = r.u32().ok_or(Error::Truncated)?;
            let mut idx = Vec::with_capacity(nt as usize);
            for _ in 0..nt {
                let i = u255(&mut r).ok_or(Error::Truncated)? as usize;
                if i >= entries.len() {
                    return Err(bad);
                }
                idx.push(i);
            }
            fonts.push((fl, idx));
        }
        fonts
    } else {
        vec![(flavor, (0..entries.len()).collect())]
    };
    let comp = crate::read::slice(data, r.pos(), comp_size as usize).ok_or(Error::Truncated)?;
    let stream = specular_brotli::decompress(comp, at).map_err(|_| bad.clone())?;
    if stream.len() < at {
        return Err(bad);
    }
    let raw = |e: &Entry| &stream[e.at..e.at + e.len as usize];
    let mut out = Vec::with_capacity(fonts.len());
    // Reconstructed glyf/loca shared between the fonts of a collection.
    let mut glyf_cache: Vec<(usize, Rebuilt)> = Vec::new();
    // Every font of a collection gets its own copy of the tables it lists, so many fonts
    // listing one large table would multiply it: all of them together stay in the limit.
    let mut total = 0usize;
    for (fl, idx) in &fonts {
        let find = |t: &[u8; 4]| idx.iter().copied().find(|&i| &entries[i].tag == t);
        let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(idx.len());
        let mut x_mins: Option<Vec<i16>> = None;
        if let Some(gi) = find(b"glyf")
            && entries[gi].transformed
        {
            let cached = glyf_cache.iter().find(|c| c.0 == gi).map(|c| c.1.clone());
            let (glyf, loca, mins) = match cached {
                Some(c) => c,
                None => {
                    let c = glyf_transform(raw(&entries[gi]), limits)?;
                    glyf_cache.push((gi, c.clone()));
                    c
                }
            };
            if let Some(li) = find(b"loca")
                && loca.len() != entries[li].orig as usize
            {
                return Err(bad);
            }
            tables.push((*b"glyf", glyf));
            tables.push((*b"loca", loca));
            x_mins = Some(mins);
        }
        for &i in idx {
            let e = &entries[i];
            if (&e.tag == b"glyf" || &e.tag == b"loca") && e.transformed {
                if x_mins.is_none() {
                    return Err(bad);
                }
                continue;
            }
            if &e.tag == b"hmtx" && e.transformed {
                let hhea = find(b"hhea").map(|i| raw(&entries[i])).ok_or(bad.clone())?;
                let maxp = find(b"maxp").map(|i| raw(&entries[i])).ok_or(bad.clone())?;
                let mins = x_mins.as_deref().ok_or(bad.clone())?;
                let h = hmtx_transform(raw(e), hhea, maxp, mins).ok_or(bad.clone())?;
                if h.len() != e.orig as usize {
                    return Err(bad);
                }
                tables.push((e.tag, h));
                continue;
            }
            tables.push((e.tag, raw(e).to_vec()));
        }
        total = tables
            .iter()
            .fold(total, |t, (_, b)| t.saturating_add(b.len()));
        if total > limits.max_sfnt_bytes {
            return Err(Error::TooLarge);
        }
        out.push(sfnt::write(*fl, tables));
    }
    Ok(Unwrapped {
        fonts: out,
        metadata: metadata2(data, mo, ml, mol, limits),
        private_len: pl,
    })
}

fn hmtx_transform(src: &[u8], hhea: &[u8], maxp: &[u8], x_mins: &[i16]) -> Option<Vec<u8>> {
    let mut r = Reader::new(src);
    let flags = r.u8()?;
    let nh = u16_at(hhea, 34)? as usize;
    let ng = u16_at(maxp, 4)? as usize;
    if nh == 0 || nh > ng {
        return None;
    }
    let mut adv = Vec::with_capacity(nh);
    for _ in 0..nh {
        adv.push(r.u16()?);
    }
    let lsb_at = |g: usize| x_mins.get(g).copied().unwrap_or(0);
    let mut lsb = Vec::with_capacity(ng);
    for g in 0..nh {
        lsb.push(if flags & 1 == 0 { r.i16()? } else { lsb_at(g) });
    }
    for g in nh..ng {
        lsb.push(if flags & 2 == 0 { r.i16()? } else { lsb_at(g) });
    }
    let mut out = Vec::with_capacity(nh * 4 + (ng - nh) * 2);
    for g in 0..ng {
        if g < nh {
            out.extend_from_slice(&adv[g].to_be_bytes());
        }
        out.extend_from_slice(&lsb[g].to_be_bytes());
    }
    Some(out)
}

/// The decoded coordinates of one point from a WOFF2 triplet.
fn triplet(flag: u8, r: &mut Reader<'_>) -> Option<(i32, i32)> {
    let f = (flag & 0x7F) as i32;
    let sign = |bit: i32, v: i32| if bit & 1 != 0 { v } else { -v };
    Some(if f < 10 {
        let b = r.u8()? as i32;
        (0, sign(f, ((f & 14) << 7) + b))
    } else if f < 20 {
        let b = r.u8()? as i32;
        (sign(f, (((f - 10) & 14) << 7) + b), 0)
    } else if f < 84 {
        let b0 = f - 20;
        let b1 = r.u8()? as i32;
        (
            sign(f, 1 + (b0 & 0x30) + (b1 >> 4)),
            sign(f >> 1, 1 + ((b0 & 0x0C) << 2) + (b1 & 0x0F)),
        )
    } else if f < 120 {
        let b0 = f - 84;
        let (b1, b2) = (r.u8()? as i32, r.u8()? as i32);
        (
            sign(f, 1 + ((b0 / 12) << 8) + b1),
            sign(f >> 1, 1 + (((b0 % 12) >> 2) << 8) + b2),
        )
    } else if f < 124 {
        let (b1, b2, b3) = (r.u8()? as i32, r.u8()? as i32, r.u8()? as i32);
        (
            sign(f, (b1 << 4) + (b2 >> 4)),
            sign(f >> 1, ((b2 & 0x0F) << 8) + b3),
        )
    } else {
        let (b1, b2, b3, b4) = (
            r.u8()? as i32,
            r.u8()? as i32,
            r.u8()? as i32,
            r.u8()? as i32,
        );
        (sign(f, (b1 << 8) + b2), sign(f >> 1, (b3 << 8) + b4))
    })
}

/// Most points in one reconstructed glyph.
const MAX_POINTS: usize = 1 << 16;

/// A rebuilt `glyf`, its `loca`, and each glyph's xMin.
type Rebuilt = (Vec<u8>, Vec<u8>, Vec<i16>);

/// The transformed `glyf` table back to `glyf` and `loca`, and each glyph's xMin (for the
/// `hmtx` transform).
fn glyf_transform(src: &[u8], limits: &Limits) -> Result<Rebuilt, Error> {
    let bad = || Error::Damaged("WOFF2");
    let mut h = Reader::new(src);
    h.skip(2).ok_or_else(bad)?;
    let options = h.u16().ok_or_else(bad)?;
    let ng = h.u16().ok_or_else(bad)? as usize;
    let index_format = h.u16().ok_or_else(bad)?;
    let mut sizes = [0usize; 7];
    for s in &mut sizes {
        *s = h.u32().ok_or_else(bad)? as usize;
    }
    let mut at = h.pos();
    let mut part = |len: usize| -> Result<&[u8], Error> {
        let s = crate::read::slice(src, at, len).ok_or_else(bad)?;
        at += len;
        Ok(s)
    };
    let mut n_contours = Reader::new(part(sizes[0])?);
    let mut n_points = Reader::new(part(sizes[1])?);
    let mut flags = Reader::new(part(sizes[2])?);
    let mut glyphs = Reader::new(part(sizes[3])?);
    let mut composite = Reader::new(part(sizes[4])?);
    let bbox_all = part(sizes[5])?;
    let mut instr = Reader::new(part(sizes[6])?);
    let overlap = if options & 1 != 0 {
        crate::read::slice(src, at, ng.div_ceil(8)).ok_or_else(bad)?
    } else {
        &[]
    };
    let bitmap_len = 4 * ng.div_ceil(32);
    let bitmap = bbox_all.get(..bitmap_len).ok_or_else(bad)?;
    let mut bbox = Reader::new(&bbox_all[bitmap_len..]);
    let has_bbox = |g: usize| bitmap[g >> 3] & (0x80 >> (g & 7)) != 0;

    let mut glyf = Vec::new();
    let mut offsets = Vec::with_capacity(ng + 1);
    let mut x_mins = Vec::with_capacity(ng);
    let mut pts: Vec<(i32, i32, bool)> = Vec::new();
    for g in 0..ng {
        offsets.push(glyf.len());
        let nc = n_contours.i16().ok_or_else(bad)?;
        let explicit = if has_bbox(g) {
            Some([
                bbox.i16().ok_or_else(bad)?,
                bbox.i16().ok_or_else(bad)?,
                bbox.i16().ok_or_else(bad)?,
                bbox.i16().ok_or_else(bad)?,
            ])
        } else {
            None
        };
        if nc == 0 {
            if explicit.is_some() {
                return Err(bad());
            }
            x_mins.push(0);
            continue;
        }
        if nc < 0 {
            let b = explicit.ok_or_else(bad)?;
            // Components, copied as they are.
            let start = composite.pos();
            let mut have_instr = false;
            loop {
                let f = composite.u16().ok_or_else(bad)?;
                composite.skip(2).ok_or_else(bad)?;
                let mut n = if f & 1 != 0 { 4 } else { 2 };
                if f & 0x08 != 0 {
                    n += 2;
                } else if f & 0x40 != 0 {
                    n += 4;
                } else if f & 0x80 != 0 {
                    n += 8;
                }
                composite.skip(n).ok_or_else(bad)?;
                have_instr |= f & 0x100 != 0;
                if f & 0x20 == 0 {
                    break;
                }
            }
            let comp = &composite.data()[start..composite.pos()];
            glyf.extend_from_slice(&(-1i16).to_be_bytes());
            for v in b {
                glyf.extend_from_slice(&v.to_be_bytes());
            }
            glyf.extend_from_slice(comp);
            if have_instr {
                let il = u255(&mut glyphs).ok_or_else(bad)?;
                let ins = instr.bytes(il as usize).ok_or_else(bad)?;
                glyf.extend_from_slice(&il.to_be_bytes());
                glyf.extend_from_slice(ins);
            }
            x_mins.push(b[0]);
        } else {
            let nc = nc as usize;
            let mut ends = Vec::with_capacity(nc);
            let mut total = 0usize;
            for _ in 0..nc {
                let n = u255(&mut n_points).ok_or_else(bad)? as usize;
                total += n;
                if total > MAX_POINTS {
                    return Err(bad());
                }
                ends.push(total);
            }
            if total == 0 {
                return Err(bad());
            }
            pts.clear();
            let (mut x, mut y) = (0i32, 0i32);
            for _ in 0..total {
                let f = flags.u8().ok_or_else(bad)?;
                let (dx, dy) = triplet(f, &mut glyphs).ok_or_else(bad)?;
                x += dx;
                y += dy;
                pts.push((x, y, f & 0x80 == 0));
            }
            let il = u255(&mut glyphs).ok_or_else(bad)?;
            let ins = instr.bytes(il as usize).ok_or_else(bad)?;
            let b = explicit.unwrap_or_else(|| {
                let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
                for &(x, y, _) in &pts {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
                [x0 as i16, y0 as i16, x1 as i16, y1 as i16]
            });
            x_mins.push(b[0]);
            glyf.extend_from_slice(&(nc as i16).to_be_bytes());
            for v in b {
                glyf.extend_from_slice(&v.to_be_bytes());
            }
            for e in &ends {
                glyf.extend_from_slice(&((e - 1) as u16).to_be_bytes());
            }
            glyf.extend_from_slice(&il.to_be_bytes());
            glyf.extend_from_slice(ins);
            let overlap_bit = overlap
                .get(g >> 3)
                .is_some_and(|b| b & (0x80 >> (g & 7)) != 0);
            write_points(&mut glyf, &pts, overlap_bit);
        }
        while glyf.len() % 4 != 0 {
            glyf.push(0);
        }
        if glyf.len() > limits.max_table_bytes {
            return Err(Error::TooLarge);
        }
    }
    offsets.push(glyf.len());
    let mut loca = Vec::with_capacity((ng + 1) * 4);
    for o in offsets {
        if index_format == 0 {
            loca.extend_from_slice(&((o / 2) as u16).to_be_bytes());
        } else {
            loca.extend_from_slice(&(o as u32).to_be_bytes());
        }
    }
    if index_format == 0 && glyf.len() / 2 > u16::MAX as usize {
        return Err(bad());
    }
    Ok((glyf, loca, x_mins))
}

/// Flags and coordinates of a simple glyph in `glyf` encoding.
fn write_points(out: &mut Vec<u8>, pts: &[(i32, i32, bool)], overlap: bool) {
    let mut fl = Vec::with_capacity(pts.len());
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let (mut px, mut py) = (0i32, 0i32);
    for (i, &(x, y, on)) in pts.iter().enumerate() {
        let (dx, dy) = (x - px, y - py);
        px = x;
        py = y;
        let mut f = u8::from(on);
        if i == 0 && overlap {
            f |= 0x40;
        }
        if dx == 0 {
            f |= 0x10;
        } else if dx.abs() < 256 {
            f |= 0x02 | if dx > 0 { 0x10 } else { 0 };
            xs.push(dx.unsigned_abs() as u8);
        } else {
            xs.extend_from_slice(&(dx as i16).to_be_bytes());
        }
        if dy == 0 {
            f |= 0x20;
        } else if dy.abs() < 256 {
            f |= 0x04 | if dy > 0 { 0x20 } else { 0 };
            ys.push(dy.unsigned_abs() as u8);
        } else {
            ys.extend_from_slice(&(dy as i16).to_be_bytes());
        }
        fl.push(f);
    }
    out.extend_from_slice(&fl);
    out.extend_from_slice(&xs);
    out.extend_from_slice(&ys);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WOFF2 collection listing 65535 fonts, each a copy of the same tables: refused from
    /// its header, and copies of all fonts together stay in the size limit.
    #[test]
    fn many_collection_fonts_are_refused() {
        let mut d = b"wOF2ttcf".to_vec();
        d.extend_from_slice(&0u32.to_be_bytes()); // length
        d.extend_from_slice(&1u16.to_be_bytes()); // tables
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(&64u32.to_be_bytes()); // sfnt size
        d.extend_from_slice(&16u32.to_be_bytes()); // compressed size
        d.extend_from_slice(&[0; 4 + 20]);
        d.extend_from_slice(&[0, 16]); // table 0 (cmap), 16 bytes
        d.extend_from_slice(&0x0002_0000u32.to_be_bytes());
        d.extend_from_slice(&[253, 0xFF, 0xFF]); // 65535 fonts
        let r = woff2(&d, &Limits::default()).map(|u| u.fonts.len());
        assert!(matches!(r, Err(Error::TooManyFaces)), "{r:?}");
    }
}

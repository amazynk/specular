//! Uncompressed image items (`unci`, ISO/IEC 23001-17).
//!
//! The `uncC` property describes the sample layout (components, bit
//! depths, sampling, interleaving, padding, tiling) and `cmpd` names the
//! components. Generic compression (23001-17 Amd 2: `cmpC` with `icef`)
//! of the whole item, its tiles, rows or other units with deflate, zlib
//! or Brotli is undone first: the units, decompressed in order, are the
//! uncompressed stream.
//!
//! Decoded: unsigned monochrome, YCbCr (4:4:4, 4:2:2, 4:2:0) and RGB
//! components of 1 to 16 bits, with alpha and padding components;
//! component, pixel, mixed, row and tile-component interleaving; bit
//! packing, component alignment, pixel / row / tile padding, both byte
//! orders; the `rgb3`, `rgba` and `abgr` profiles of `uncC` version 1.
//! Components shallower than 8 bits are scaled to 8 (`v × 255 / max`,
//! rounded), so an image of mixed depths comes out at its deepest one.
//! Refused, by name: block packing (`block_size` > 0), multi-Y
//! interleaving, 4:1:1 sampling, signed / float / complex components,
//! and depth, disparity, palette, colour filter array (raw sensor) and
//! CMYK components.

use heif::{Chroma, HeifFrame, HeifPixelFormat, ImageNode, Plane};

use crate::{Error, Result};

/// What a component is (`cmpd` `component_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Y,
    Cb,
    Cr,
    R,
    G,
    B,
    A,
    /// Padding: read and dropped.
    Pad,
}

#[derive(Clone, Copy, Debug)]
struct Comp {
    role: Role,
    bits: u8,
    /// `component_align_size` in bytes (0: bit-packed).
    align: u8,
}

/// The parsed `uncC` (+ `cmpd`, `cmpC`, `icef`) of an item.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    comps: Vec<Comp>,
    chroma: Chroma,
    rgb: bool,
    interleave: u8,
    little_endian: bool,
    pixel_size: u32,
    row_align: u32,
    tile_align: u32,
    tile_cols: u32,
    tile_rows: u32,
    compression: Option<Compression>,
}

#[derive(Clone, Debug)]
struct Compression {
    kind: [u8; 4],
    /// `compressed_unit_type`: 0 / 1 the whole item, else units listed by `icef`.
    unit_type: u8,
    /// `(offset, size)` of each unit inside the item data.
    units: Vec<(u64, u64)>,
}

/// The output geometry of a [`Layout`]: chroma, depth, alpha, RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Format {
    pub chroma: Chroma,
    pub bit_depth: u8,
    pub alpha: bool,
    pub rgb: bool,
}

struct Be<'a> {
    b: &'a [u8],
    at: usize,
}

impl Be<'_> {
    fn take(&mut self, n: usize, what: &str) -> Result<&[u8]> {
        let s = self
            .b
            .get(self.at..self.at + n)
            .ok_or_else(|| invalid(format!("{what} truncated")))?;
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self, what: &str) -> Result<u8> {
        Ok(self.take(1, what)?[0])
    }
    fn u16(&mut self, what: &str) -> Result<u16> {
        let s = self.take(2, what)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u32(&mut self, what: &str) -> Result<u32> {
        let s = self.take(4, what)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn uint(&mut self, bytes: usize, what: &str) -> Result<u64> {
        Ok(self
            .take(bytes, what)?
            .iter()
            .fold(0u64, |a, &b| (a << 8) | u64::from(b)))
    }
}

fn invalid(m: impl Into<String>) -> Error {
    heif::HeifError::invalid(m.into()).into()
}

fn unsupported(m: impl Into<String>) -> Error {
    Error::Unsupported(m.into())
}

/// The raw body (version and flags included) of the item's `t` property.
pub(crate) fn raw_property<'a>(node: &'a ImageNode, t: &[u8; 4]) -> Option<&'a [u8]> {
    node.properties
        .entries
        .iter()
        .find_map(|e| match &e.property {
            heif::props::Property::Unknown(r) if &r.box_type == t => Some(&r.body[..]),
            _ => None,
        })
}

/// What `component_type` `t` names, for refusals.
fn component_name(t: u16) -> String {
    match t {
        8 => "depth".into(),
        9 => "disparity".into(),
        10 => "palette".into(),
        11 => "colour filter array (raw sensor data)".into(),
        13 => "cyan".into(),
        14 => "magenta".into(),
        15 => "yellow".into(),
        16 => "key (black)".into(),
        t if t >= 0x8000 => "custom".into(),
        t => format!("type {t}"),
    }
}

impl Layout {
    /// Parse the `uncC`, `cmpd`, `cmpC` and `icef` properties of `node`.
    pub(crate) fn of(node: &ImageNode) -> Result<Layout> {
        let id = node.item.id;
        let uncc = raw_property(node, b"uncC")
            .ok_or_else(|| invalid(format!("item {id}: unci item without uncC")))?;
        let mut r = Be { b: uncc, at: 0 };
        let version = r.u8("uncC version")?;
        r.take(3, "uncC flags")?;
        let profile: [u8; 4] = r.take(4, "uncC profile")?.try_into().unwrap_or([0; 4]);
        let mut l = Layout {
            comps: Vec::new(),
            chroma: Chroma::Yuv444,
            rgb: false,
            interleave: 1,
            little_endian: false,
            pixel_size: 0,
            row_align: 0,
            tile_align: 0,
            tile_cols: 1,
            tile_rows: 1,
            compression: None,
        };
        let comp = |role, bits| Comp {
            role,
            bits,
            align: 0,
        };
        match version {
            1 => {
                l.comps = match &profile {
                    b"rgb3" => vec![comp(Role::R, 8), comp(Role::G, 8), comp(Role::B, 8)],
                    b"rgba" => vec![
                        comp(Role::R, 8),
                        comp(Role::G, 8),
                        comp(Role::B, 8),
                        comp(Role::A, 8),
                    ],
                    b"abgr" => vec![
                        comp(Role::A, 8),
                        comp(Role::B, 8),
                        comp(Role::G, 8),
                        comp(Role::R, 8),
                    ],
                    p => {
                        return Err(unsupported(format!(
                            "item {id}: uncompressed profile '{}'",
                            heif::boxes::fourcc_str(p)
                        )));
                    }
                };
            }
            0 => l.parse_v0(node, &mut r)?,
            v => return Err(unsupported(format!("item {id}: uncC version {v}"))),
        }
        l.compression = compression(node)?;
        l.check(id)?;
        Ok(l)
    }

    fn parse_v0(&mut self, node: &ImageNode, r: &mut Be<'_>) -> Result<()> {
        let id = node.item.id;
        let cmpd = raw_property(node, b"cmpd")
            .ok_or_else(|| invalid(format!("item {id}: uncC version 0 without cmpd")))?;
        // `cmpd` is a plain box: the count comes first.
        let mut c = Be { b: cmpd, at: 0 };
        let count = c.u32("cmpd component_count")?;
        if count == 0 || count > 64 {
            return Err(invalid(format!("item {id}: cmpd with {count} components")));
        }
        let mut types = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let t = c.u16("cmpd component_type")?;
            if t >= 0x8000 {
                // component_type_uri, null-terminated.
                while c.u8("cmpd component_type_uri")? != 0 {}
            }
            types.push(t);
        }
        let n = r.u32("uncC component_count")?;
        if n == 0 || n > 64 {
            return Err(invalid(format!("item {id}: uncC with {n} components")));
        }
        for _ in 0..n {
            let index = r.u16("uncC component_index")?;
            let bits = u16::from(r.u8("uncC component_bit_depth_minus_one")?) + 1;
            let format = r.u8("uncC component_format")?;
            let align = r.u8("uncC component_align_size")?;
            let t = *types
                .get(index as usize)
                .ok_or_else(|| invalid(format!("item {id}: uncC component_index {index}")))?;
            match format {
                0 => {}
                1 => return Err(unsupported(format!("item {id}: floating-point samples"))),
                2 => return Err(unsupported(format!("item {id}: complex samples"))),
                3 => return Err(unsupported(format!("item {id}: signed samples"))),
                f => return Err(unsupported(format!("item {id}: component format {f}"))),
            }
            let role = match t {
                0 | 1 => Role::Y,
                2 => Role::Cb,
                3 => Role::Cr,
                4 => Role::R,
                5 => Role::G,
                6 => Role::B,
                7 => Role::A,
                12 => Role::Pad,
                t => {
                    return Err(unsupported(format!(
                        "item {id}: {} components",
                        component_name(t)
                    )));
                }
            };
            if bits > 16 && role != Role::Pad {
                return Err(unsupported(format!("item {id}: {bits}-bit samples")));
            }
            if align != 0 && u16::from(align) * 8 < bits {
                return Err(invalid(format!(
                    "item {id}: {bits}-bit component in {align} aligned bytes"
                )));
            }
            self.comps.push(Comp {
                role,
                bits: bits.min(64) as u8,
                align,
            });
        }
        let sampling = r.u8("uncC sampling_type")?;
        self.interleave = r.u8("uncC interleave_type")?;
        let block_size = r.u8("uncC block_size")?;
        let flags = r.u8("uncC flags")?;
        self.little_endian = flags & 0x80 != 0;
        self.pixel_size = r.u32("uncC pixel_size")?;
        self.row_align = r.u32("uncC row_align_size")?;
        self.tile_align = r.u32("uncC tile_align_size")?;
        self.tile_cols = r.u32("uncC num_tile_cols_minus_one")?.saturating_add(1);
        self.tile_rows = r.u32("uncC num_tile_rows_minus_one")?.saturating_add(1);
        if block_size != 0 {
            return Err(unsupported(format!(
                "item {id}: uncompressed block packing (block_size {block_size})"
            )));
        }
        self.chroma = match sampling {
            0 => Chroma::Yuv444,
            1 => Chroma::Yuv422,
            2 => Chroma::Yuv420,
            3 => return Err(unsupported(format!("item {id}: 4:1:1 sampling"))),
            s => return Err(invalid(format!("item {id}: sampling_type {s}"))),
        };
        Ok(())
    }

    /// The component set, sampling and interleaving make a picture we decode.
    fn check(&mut self, id: u32) -> Result<()> {
        let has = |r: Role| self.comps.iter().filter(|c| c.role == r).count();
        for r in [
            Role::Y,
            Role::Cb,
            Role::Cr,
            Role::R,
            Role::G,
            Role::B,
            Role::A,
        ] {
            if has(r) > 1 {
                return Err(unsupported(format!("item {id}: repeated {r:?} components")));
            }
        }
        let (y, cb, cr) = (has(Role::Y), has(Role::Cb), has(Role::Cr));
        let (rr, g, b) = (has(Role::R), has(Role::G), has(Role::B));
        self.rgb = rr + g + b > 0;
        let ok = match (self.rgb, y, cb + cr) {
            (true, 0, 0) => rr == 1 && g == 1 && b == 1,
            (false, 1, 0) => true,
            (false, 1, 2) => true,
            _ => false,
        };
        if !ok {
            return Err(unsupported(format!(
                "item {id}: uncompressed component set {:?}",
                self.comps.iter().map(|c| c.role).collect::<Vec<_>>()
            )));
        }
        if y == 1 && cb == 0 {
            // Monochrome: the sampling type does not apply.
            self.chroma = Chroma::Mono;
        } else if self.rgb && self.chroma != Chroma::Yuv444 {
            return Err(invalid(format!("item {id}: subsampled RGB")));
        }
        match self.interleave {
            0 | 4 => {}
            1 if self.chroma == Chroma::Yuv444 || self.chroma == Chroma::Mono => {}
            2 if cb == 1 => {}
            3 if self.chroma != Chroma::Yuv420 => {}
            5 => {
                return Err(unsupported(format!(
                    "item {id}: uncompressed multi-Y interleaving"
                )));
            }
            1..=3 => {
                return Err(invalid(format!(
                    "item {id}: interleave_type {} with {:?}",
                    self.interleave, self.chroma
                )));
            }
            i => return Err(invalid(format!("item {id}: interleave_type {i}"))),
        }
        Ok(())
    }

    /// The decoded frame's layout.
    pub(crate) fn format(&self) -> Format {
        let bits = self
            .comps
            .iter()
            .filter(|c| c.role != Role::Pad)
            .map(|c| c.bits)
            .max()
            .unwrap_or(8);
        Format {
            chroma: self.chroma,
            bit_depth: bits.max(8),
            alpha: self.comps.iter().any(|c| c.role == Role::A),
            rgb: self.rgb,
        }
    }
}

/// The `cmpC` / `icef` of an item, when it is compressed.
fn compression(node: &ImageNode) -> Result<Option<Compression>> {
    let id = node.item.id;
    let Some(cmpc) = raw_property(node, b"cmpC") else {
        return Ok(None);
    };
    let mut r = Be { b: cmpc, at: 4 };
    let kind: [u8; 4] = r
        .take(4, "cmpC compression_type")?
        .try_into()
        .unwrap_or([0; 4]);
    let unit_type = r.u8("cmpC compressed_unit_type")?;
    if !matches!(&kind, b"defl" | b"zlib" | b"brot") {
        return Err(unsupported(format!(
            "item {id}: '{}' compression",
            heif::boxes::fourcc_str(&kind)
        )));
    }
    let mut units = Vec::new();
    // Without an `icef` the item is one unit (libheif writes a one-tile image so).
    let icef = raw_property(node, b"icef").filter(|_| unit_type > 1);
    let unit_type = if icef.is_some() { unit_type } else { 0 };
    if let Some(icef) = icef {
        let mut r = Be { b: icef, at: 4 };
        let codes = r.u8("icef codes")?;
        let off_bytes = match codes >> 5 {
            0 => 0,
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 8,
            c => return Err(invalid(format!("item {id}: icef unit_offset_code {c}"))),
        };
        let size_bytes = match (codes >> 2) & 7 {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 8,
            c => return Err(invalid(format!("item {id}: icef unit_size_code {c}"))),
        };
        let n = r.u32("icef num_compressed_units")?;
        // Every entry takes at least a byte: the count is bounded by the box.
        if n as usize > icef.len() {
            return Err(invalid(format!("item {id}: icef claims {n} units")));
        }
        let mut next = 0u64;
        for _ in 0..n {
            let off = if off_bytes == 0 {
                next
            } else {
                r.uint(off_bytes, "icef unit_offset")?
            };
            let size = r.uint(size_bytes, "icef unit_size")?;
            next = off.saturating_add(size);
            units.push((off, size));
        }
    }
    Ok(Some(Compression {
        kind,
        unit_type,
        units,
    }))
}

/// Undo the generic compression of `data` (at most `limit` bytes out).
fn decompress(id: u32, c: &Compression, data: &[u8], limit: usize) -> Result<Vec<u8>> {
    let one = |d: &[u8], left: usize| -> Result<Vec<u8>> {
        let bad = |e: String| invalid(format!("item {id}: {e}"));
        match &c.kind {
            b"defl" => miniz_oxide::inflate::decompress_to_vec_with_limit(d, left)
                .map_err(|e| bad(format!("deflate: {e:?}"))),
            b"zlib" => miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(d, left)
                .map_err(|e| bad(format!("zlib: {e:?}"))),
            _ => specular_brotli::decompress(d, left).map_err(|e| bad(format!("brotli: {e}"))),
        }
    };
    if c.unit_type <= 1 {
        return one(data, limit);
    }
    let mut out = Vec::new();
    for &(off, size) in &c.units {
        let unit = usize::try_from(off)
            .ok()
            .zip(usize::try_from(size).ok())
            .and_then(|(o, s)| data.get(o..o.checked_add(s)?))
            .ok_or_else(|| invalid(format!("item {id}: compressed unit past the item data")))?;
        let left = limit.saturating_sub(out.len());
        out.extend_from_slice(&one(unit, left)?);
    }
    Ok(out)
}

/// MSB-first bit reader with byte-order aware whole-byte reads.
struct Bits<'a> {
    b: &'a [u8],
    /// Position in bits.
    pos: usize,
}

impl Bits<'_> {
    fn bits(&mut self, n: u32) -> Result<u64> {
        let end = self.pos + n as usize;
        if end > self.b.len() * 8 {
            return Err(invalid("uncompressed item data truncated"));
        }
        let mut v = 0u64;
        let mut p = self.pos;
        while p < end {
            let byte = self.b[p / 8];
            let off = p % 8;
            let take = (8 - off).min(end - p);
            let chunk = (byte >> (8 - off - take)) & ((1u16 << take) - 1) as u8;
            v = (v << take) | u64::from(chunk);
            p += take;
        }
        self.pos = end;
        Ok(v)
    }

    fn bytes(&mut self, n: usize, little: bool) -> Result<u64> {
        let s = self.pos / 8;
        let bytes = self
            .b
            .get(s..s + n)
            .ok_or_else(|| invalid("uncompressed item data truncated"))?;
        self.pos += n * 8;
        Ok(if little {
            bytes.iter().rev().fold(0, |a, &b| (a << 8) | u64::from(b))
        } else {
            bytes.iter().fold(0, |a, &b| (a << 8) | u64::from(b))
        })
    }

    fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }

    /// Pads the span that started at byte `start` to a multiple of `align` bytes.
    fn pad(&mut self, start: usize, align: u32) {
        self.align();
        if align > 1 {
            let len = self.pos / 8 - start;
            let a = align as usize;
            self.pos = (start + len.div_ceil(a) * a) * 8;
        }
    }

    fn byte(&self) -> usize {
        self.pos.div_ceil(8)
    }
}

/// The planes being filled: one `u16` per sample, per component.
struct Canvas {
    planes: Vec<Vec<u16>>,
    dims: Vec<(u32, u32)>,
}

impl Layout {
    /// Size of component `c`'s plane in a `w × h` area.
    fn comp_dims(&self, c: &Comp, w: u32, h: u32) -> (u32, u32) {
        match c.role {
            Role::Cb | Role::Cr => self.chroma.chroma_dims(w, h),
            _ => (w, h),
        }
    }

    fn read(&self, r: &mut Bits<'_>, c: &Comp) -> Result<u16> {
        let v = if c.align > 0 {
            if !r.pos.is_multiple_of(8) {
                r.align();
            }
            r.bytes(c.align as usize, self.little_endian)?
        } else if c.bits.is_multiple_of(8) && r.pos.is_multiple_of(8) && c.bits <= 64 {
            r.bytes(c.bits as usize / 8, self.little_endian)?
        } else {
            r.bits(u32::from(c.bits))?
        };
        Ok((v & ((1u64 << c.bits.min(63)) - 1)) as u16)
    }

    /// Decode the item `data` (compressed or not) of a `w × h` image.
    pub(crate) fn decode(&self, id: u32, data: &[u8], w: u32, h: u32) -> Result<HeifFrame> {
        let (tc, tr) = (self.tile_cols, self.tile_rows);
        if tc == 0 || tr == 0 || !w.is_multiple_of(tc) || !h.is_multiple_of(tr) {
            return Err(invalid(format!(
                "item {id}: {tc}×{tr} tiles do not divide the {w}×{h} image"
            )));
        }
        let (tw, th) = (w / tc, h / tr);
        let (sx, sy) = self.chroma.shift();
        if self.chroma != Chroma::Mono
            && (!tw.is_multiple_of(1 << sx) || !th.is_multiple_of(1 << sy))
        {
            return Err(invalid(format!(
                "item {id}: {tw}×{th} tiles of a subsampled image"
            )));
        }
        // Every sample takes at least a bit: a stream shorter than that is truncated, and
        // the planes are charged to the budget before they are allocated.
        let samples: u64 = self
            .comps
            .iter()
            .map(|c| {
                let (cw, ch) = self.comp_dims(c, w, h);
                u64::from(cw) * u64::from(ch)
            })
            .sum();
        let _planes = specular_core::budget::reserve(samples.saturating_mul(4))?;
        let owned;
        let data = match &self.compression {
            Some(c) => {
                let limit = samples
                    .saturating_mul(16)
                    .saturating_add(1 << 20)
                    .min(heif::file::MAX_ITEM_BYTES) as usize;
                owned = decompress(id, c, data, limit)?;
                &owned[..]
            }
            None => data,
        };
        if (data.len() as u64).saturating_mul(8) < samples {
            return Err(invalid(format!(
                "item {id}: uncompressed item data truncated"
            )));
        }
        let mut cv = Canvas {
            planes: Vec::with_capacity(self.comps.len()),
            dims: Vec::with_capacity(self.comps.len()),
        };
        for c in &self.comps {
            let (cw, ch) = if c.role == Role::Pad {
                (0, 0)
            } else {
                self.comp_dims(c, w, h)
            };
            cv.planes.push(vec![0u16; cw as usize * ch as usize]);
            cv.dims.push((cw, ch));
        }
        let mut r = Bits { b: data, pos: 0 };
        if self.interleave == 4 {
            for (k, c) in self.comps.iter().enumerate() {
                for t in 0..tc * tr {
                    let start = r.byte();
                    self.plane(&mut r, &mut cv, k, c, (t % tc, t / tc), (tw, th))?;
                    r.pad(start, self.tile_align);
                }
            }
        } else {
            for t in 0..tc * tr {
                let start = r.byte();
                self.tile(&mut r, &mut cv, (t % tc, t / tc), (tw, th))?;
                r.pad(start, self.tile_align);
            }
        }
        self.frame(cv, w, h)
    }

    /// One tile of every component, in the interleaving's order.
    fn tile(
        &self,
        r: &mut Bits<'_>,
        cv: &mut Canvas,
        (tx, ty): (u32, u32),
        (tw, th): (u32, u32),
    ) -> Result<()> {
        match self.interleave {
            0 => {
                for (k, c) in self.comps.iter().enumerate() {
                    self.plane(r, cv, k, c, (tx, ty), (tw, th))?;
                }
            }
            1 => {
                for y in 0..th {
                    let row = r.byte();
                    for x in 0..tw {
                        let px = r.byte();
                        for (k, c) in self.comps.iter().enumerate() {
                            let v = self.read(r, c)?;
                            put(cv, k, tx * tw + x, ty * th + y, v);
                        }
                        if self.pixel_size > 0 {
                            r.align();
                            r.pos = (px + self.pixel_size as usize) * 8;
                        }
                    }
                    r.pad(row, self.row_align);
                }
            }
            2 => {
                // The other components as planes; the chroma components interleaved as one
                // plane where the first of them is listed.
                let chroma: Vec<usize> = (0..self.comps.len())
                    .filter(|&k| matches!(self.comps[k].role, Role::Cb | Role::Cr))
                    .collect();
                for (k, c) in self.comps.iter().enumerate() {
                    if !chroma.contains(&k) {
                        self.plane(r, cv, k, c, (tx, ty), (tw, th))?;
                    } else if chroma.first() == Some(&k) {
                        let (cw, ch) = self.chroma.chroma_dims(tw, th);
                        for y in 0..ch {
                            let row = r.byte();
                            for x in 0..cw {
                                for &k in &chroma {
                                    let v = self.read(r, &self.comps[k])?;
                                    put(cv, k, tx * cw + x, ty * ch + y, v);
                                }
                            }
                            r.pad(row, self.row_align);
                        }
                    }
                }
            }
            _ => {
                // Row: each component's row in turn (no vertical subsampling).
                for y in 0..th {
                    for (k, c) in self.comps.iter().enumerate() {
                        let (cw, _) = self.comp_dims(c, tw, th);
                        let row = r.byte();
                        for x in 0..cw {
                            let v = self.read(r, c)?;
                            put(cv, k, tx * cw + x, ty * th + y, v);
                        }
                        r.pad(row, self.row_align);
                    }
                }
            }
        }
        Ok(())
    }

    /// One tile of component `k` as a plane.
    fn plane(
        &self,
        r: &mut Bits<'_>,
        cv: &mut Canvas,
        k: usize,
        c: &Comp,
        (tx, ty): (u32, u32),
        (tw, th): (u32, u32),
    ) -> Result<()> {
        let (cw, ch) = self.comp_dims(c, tw, th);
        for y in 0..ch {
            let row = r.byte();
            for x in 0..cw {
                let v = self.read(r, c)?;
                put(cv, k, tx * cw + x, ty * ch + y, v);
            }
            r.pad(row, self.row_align);
        }
        Ok(())
    }

    /// The component planes as a frame: Y / Cb / Cr or G / B / R, then alpha, at one depth.
    fn frame(&self, cv: Canvas, w: u32, h: u32) -> Result<HeifFrame> {
        let f = self.format();
        let fmt = HeifPixelFormat::new(f.chroma, f.bit_depth, f.alpha)?;
        let order: &[Role] = match (f.rgb, f.chroma) {
            (true, _) => &[Role::G, Role::B, Role::R],
            (false, Chroma::Mono) => &[Role::Y],
            _ => &[Role::Y, Role::Cb, Role::Cr],
        };
        let wide = fmt.bytes_per_sample() == 2;
        let out_max = u32::from(fmt.max_value());
        let mut planes = Vec::with_capacity(fmt.plane_count());
        for role in order.iter().chain(f.alpha.then_some(&Role::A)) {
            let k = self
                .comps
                .iter()
                .position(|c| c.role == *role)
                .ok_or_else(|| invalid("uncompressed component missing"))?;
            let bits = self.comps[k].bits;
            let (pw, ph) = cv.dims[k];
            let src = &cv.planes[k];
            // Shallower components widen by bit replication (5-bit 0b10110 → 0b10110101).
            let scale = |v: u16| -> u16 {
                if bits >= f.bit_depth {
                    return v;
                }
                let v = u32::from(v);
                let mut out = 0u32;
                let mut have = 0u8;
                while have < f.bit_depth {
                    let shift = i32::from(f.bit_depth) - i32::from(have) - i32::from(bits);
                    out |= if shift >= 0 { v << shift } else { v >> -shift };
                    have += bits;
                }
                (out & out_max) as u16
            };
            let data: Vec<u8> = if wide {
                src.iter().flat_map(|&v| scale(v).to_le_bytes()).collect()
            } else {
                src.iter().map(|&v| scale(v) as u8).collect()
            };
            planes.push(Plane {
                stride: pw as usize * (1 + wide as usize),
                data,
            });
            debug_assert_eq!(
                planes.last().map(|p| p.data.len()),
                Some(pw as usize * ph as usize * (1 + wide as usize))
            );
        }
        let frame = HeifFrame {
            width: w,
            height: h,
            format: fmt,
            planes,
        };
        frame.validate()?;
        Ok(frame)
    }
}

fn put(cv: &mut Canvas, k: usize, x: u32, y: u32, v: u16) {
    let (w, h) = cv.dims[k];
    if x < w && y < h {
        cv.planes[k][y as usize * w as usize + x as usize] = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_reader_reads_msb_first_and_pads() {
        let data = [0b1010_1100, 0b0101_0000, 0xff, 0x12, 0x34];
        let mut r = Bits { b: &data, pos: 0 };
        assert_eq!(r.bits(3).unwrap(), 0b101);
        assert_eq!(r.bits(6).unwrap(), 0b011000);
        r.pad(0, 0);
        assert_eq!(r.pos, 16);
        r.pad(0, 3);
        assert_eq!(r.pos, 24);
        assert_eq!(r.bytes(2, false).unwrap(), 0x1234);
        assert!(r.bits(1).is_err());
        let mut r = Bits {
            b: &data[3..],
            pos: 0,
        };
        assert_eq!(r.bytes(2, true).unwrap(), 0x3412);
    }
}

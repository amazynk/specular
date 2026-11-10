//! Own JPEG 2000 Part 1 and HTJ2K decoder for Specular (DESIGN.md §5): JP2 files and raw codestreams.
//!
//! * **Containers.** JP2 (`ihdr`, `colr` enumerated or ICC, `pclr`/`cmap`, `cdef`, `res`) and
//!   raw codestreams. JPX files are decoded one codestream at a time
//!   ([`codestream_count`], [`decode_at`]), each with the JP2 header overridden by its own
//!   codestream and compositing layer headers; codestreams split into fragments (`ftbl`) are
//!   joined first ([`defragment`]; fragments in other files are refused). The frames of a JPX
//!   composition box (canvas,
//!   layers registered to their codestreams, cropped, scaled and placed, opacity and chroma
//!   keys, frame times and persistence) are drawn by [`decode_composite`] / [`Compositor`]
//!   ([`composition`] for the first, [`animation`] for all).
//! * **Codestream.** All of Part 1: multiple tiles and tile-parts, the five progression orders
//!   and POC changes, precincts, PPM/PPT, SOP/EPH, all code-block styles (bypass, reset,
//!   termination on every pass, vertically causal contexts, segmentation symbols), ROI (max-shift),
//!   scalar derived / expounded quantisation and the reversible (5/3) and irreversible (9/7)
//!   wavelets with RCT/ICT, component subsampling and precisions up to 38 bits (reduced to 16).
//! * **JPM (Part 6).** Compound pages ([`read_jpm_info`], [`decode_jpm_page`]): the layout
//!   objects of a page drawn in order onto its page colour, each image (or flat base colour)
//!   through its opacity mask, scaled, offset and clipped to its window as in T.805 clause 5,
//!   to 8-bit RGBA, with page orientation applied. JPEG 2000 and uncompressed objects are
//!   decoded here (from contiguous codestreams or fragment tables, with shared JP2 headers);
//!   objects of the other coders go to a caller-supplied [`JpmOptions::foreign`] decoder.
//!   The single-image entry points ([`decode`], [`read_info`]) refuse JPM files.
//! * **HTJ2K (Part 15).** JPH files and raw HT codestreams: own HT block decoder (cleanup pass
//!   with MEL, VLC and MagSgn, plus SigProp/MagRef refinement), HT and mixed (`HTMIXED`,
//!   decided per code-block) tile-components, placeholder passes and multiple HT sets, on the
//!   same tier-2, dequantisation, IDWT, colour and reduce/region paths as Part 1.
//! * **Fast paths.** [`DecodeOptions::reduce`] discards resolution levels (a 1/2^r decode does
//!   tier-1 and the IDWT only for the kept levels); [`DecodeOptions::region`] decodes only the
//!   tiles, precincts and code-blocks touching the region and runs the IDWT on windows;
//!   [`DecodeOptions::max_layers`] stops at a quality layer. Tiles, components, code-blocks and
//!   IDWT rows are decoded in parallel.
//! * **Non-linearity (NLT).** The binary-complement point transform (T.801 NLT type 3, which
//!   OpenEXR's HTJ2K compression puts on its half and float components) is undone on signed
//!   components, and so is OpenJPH's lookup-table non-linearity (type 4, OpenEXR's lossy
//!   LJ2K: the table, then binary complement).
//! * **Robustness.** Never panics on malformed input; allocations are bounded and fallible.
//!   Truncated or corrupt packet data yields a partial image with [`Image::truncated`] set.
//!
//! # Not supported
//! * Other NLT types (gamma, T.801 lookup tables) and Part 2 colour spaces,
//!   wavelet kernels and multi-component transforms: [`Jp2Error::Unsupported`].
//! * More than 30 magnitude bit-planes in a Part 1 (MQ-coded) code-block or an irreversible
//!   component: those do not decode exactly. (Reversible HT components that deep, such as
//!   OpenEXR's 32-bit FLOAT and UINT channels, take a 64-bit path: HT block words, 5/3
//!   wavelet and RCT in `i64`, up to 56 bit-planes; shallower ones keep the 32-bit path.)
//! * JPM: objects stored in other files (data references), JPM page collections (pages are
//!   taken in file order), and coders other than JPEG 2000 and none without
//!   [`JpmOptions::foreign`] (refused with the coder's name).

mod boxes;
mod color;
mod compose;
mod dwt;
mod error;
mod geom;
mod ht;
mod ht_vlc;
mod jpm;
mod markers;
mod mq;
mod t1;
mod t2;
mod tagtree;
mod tile;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

pub use boxes::{ColorSpace, Container, GEOJP2_UUID, GeoBoxes, Resolution, geo_boxes};
pub use compose::{
    Animation, Composition, Compositor, MAX_FRAMES, Placement, animation, composition,
    decode_composite,
};
pub use error::{Jp2Error, Result};
pub use jpm::{
    ForeignDecoder, ForeignObject, JpmCoder, JpmInfo, JpmOptions, JpmPage, decode_jpm_page, is_jpm,
    read_jpm_info,
};
pub use markers::Progression;

use geom::{Rect, ceil_shr, try_vec};

/// Largest output we are willing to allocate, in pixels.
pub const MAX_OUTPUT_PIXELS: u64 = 1 << 31;

/// Per-component header information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    /// Bit depth (1..=38).
    pub precision: u8,
    pub signed: bool,
    /// Subsampling factors.
    pub dx: u8,
    pub dy: u8,
    /// Coded with the reversible (5/3) wavelet (main header COD / COC).
    pub reversible: bool,
}

/// Image header information (cheap to read: no packet data is touched).
#[derive(Clone, Debug, PartialEq)]
pub struct Jp2Info {
    /// Full-resolution image size.
    pub width: u32,
    pub height: u32,
    /// Image origin on the reference grid (usually 0, 0).
    pub x0: u32,
    pub y0: u32,
    pub container: Container,
    pub components: Vec<ComponentInfo>,
    /// Colour space of the output channels before conversion.
    pub color: ColorSpace,
    pub icc_profile: Option<Vec<u8>>,
    /// Number of resolution levels that can be discarded (the smallest number of
    /// decomposition levels of any component); valid [`DecodeOptions::reduce`] values are
    /// `0..=levels`.
    pub levels: u8,
    pub layers: u16,
    pub progression: Progression,
    /// Lossless (5/3) wavelet on the first component.
    pub reversible: bool,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles_x: u32,
    pub tiles_y: u32,
    /// Code-block size of the first component.
    pub code_block: (u32, u32),
    pub resolution: Option<Resolution>,
    /// The output has an alpha channel.
    pub has_alpha: bool,
    /// The alpha is premultiplied (`cdef` type 2); decoded pixels are straight.
    pub premultiplied_alpha: bool,
    /// The image uses a palette (`pclr`).
    pub has_palette: bool,
    /// Bit depth of the deepest output channel: a palette column's for palette-mapped
    /// channels, else the component's ([`Depth::Sixteen`] keeps it when above 8).
    pub output_precision: u8,
    /// The output is a single grey channel.
    pub gray: bool,
    /// The image is grey with an alpha channel (output as RGBA with the grey replicated).
    pub gray_alpha: bool,
    /// First Latin-1/UTF-8 comment (`COM`) of the main header.
    pub comment: Option<String>,
    /// HTJ2K (Part 15): the CAP marker or a main-header COD/COC selects HT code-blocks.
    pub htj2k: bool,
}

impl Jp2Info {
    /// Output size at `reduce` discarded levels.
    pub fn reduced_size(&self, reduce: u8) -> (u32, u32) {
        (
            reduced_extent(self.x0, self.x0 + self.width, reduce),
            reduced_extent(self.y0, self.y0 + self.height, reduce),
        )
    }
}

fn reduced_extent(a: u32, b: u32, r: u8) -> u32 {
    ceil_shr(b, r as u32) - ceil_shr(a, r as u32)
}

/// Rectangle in output pixel coordinates (relative to the top-left of the reduced image).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Parallelism {
    /// Use the global rayon pool for images of at least ~1 MP, a single thread otherwise.
    #[default]
    Auto,
    Single,
    /// Use a dedicated (cached) pool with this many threads.
    Threads(usize),
}

/// Output sample depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Depth {
    #[default]
    Eight,
    Sixteen,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DecodeOptions<'a> {
    /// Discard this many resolution levels: the output is about `1 / 2^reduce` of the full
    /// size (exactly `ceil(x1 / 2^r) - ceil(x0 / 2^r)`), see [`Jp2Info::reduced_size`].
    pub reduce: u8,
    /// Decode only this rectangle of the (reduced) image; the output is exactly this size.
    pub region: Option<Region>,
    /// Decode at most this many quality layers.
    pub max_layers: Option<u16>,
    pub depth: Depth,
    pub parallelism: Parallelism,
    /// Checked between tiles, resolution levels and every 1024 packets.
    pub cancel: Option<&'a AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    Rgba8,
    Gray16,
    Rgba16,
}

impl PixelFormat {
    pub fn channels(self) -> usize {
        match self {
            PixelFormat::Gray8 | PixelFormat::Gray16 => 1,
            _ => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pixels {
    U8(Vec<u8>),
    U16(Vec<u16>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Position of the output in the reduced image (the region origin, or (0, 0)).
    pub origin: (u32, u32),
    pub format: PixelFormat,
    /// Row-major, `width * channels` samples per row, no padding.
    pub pixels: Pixels,
    /// Packet data was missing or corrupt; affected areas are blurred or black.
    pub truncated: bool,
    pub reduce: u8,
}

/// Smallest-work `reduce` whose output still covers `target_w x target_h` (or the full size if
/// the target is larger than the image).
pub fn choose_reduce(info: &Jp2Info, target_w: u32, target_h: u32) -> u8 {
    let mut best = 0;
    for r in 0..=info.levels {
        let (w, h) = info.reduced_size(r);
        if w >= target_w && h >= target_h {
            best = r;
        } else {
            break;
        }
    }
    best
}

/// A parsed file, ready for (repeated) decodes.
pub struct Decoder<'a> {
    boxes: boxes::Boxes<'a>,
    cs: markers::Codestream<'a>,
    info: Jp2Info,
}

/// See [`Jp2Info::output_precision`].
fn output_precision(boxes: &boxes::Boxes, siz: &markers::Siz) -> u8 {
    let comp = |c: usize| siz.comps.get(c).map_or(8, |c| c.prec);
    match (&boxes.palette, &boxes.cmap) {
        (Some(pal), Some(cmap)) => cmap
            .iter()
            .map(|e| match e.palette_col {
                Some(col) if (col as usize) < pal.columns.len() => pal.columns[col as usize].0,
                _ => comp(e.component as usize),
            })
            .max()
            .unwrap_or(8),
        _ => siz.comps.iter().map(|c| c.prec).max().unwrap_or(8),
    }
}

impl<'a> Decoder<'a> {
    pub fn new(data: &'a [u8]) -> Result<Decoder<'a>> {
        Self::new_at(data, 0)
    }

    /// The decoder for codestream `index` of a JPX file (see [`codestream_count`]); index 0 is
    /// what [`Decoder::new`] decodes.
    pub fn new_at(data: &'a [u8], index: usize) -> Result<Decoder<'a>> {
        Self::new_layer(data, index, index)
    }

    /// The decoder for codestream `index` of a JPX file shown as compositing layer `layer`
    /// (that layer's colour and channel headers).
    pub(crate) fn new_layer(data: &'a [u8], index: usize, layer: usize) -> Result<Decoder<'a>> {
        Self::from_boxes(boxes::parse_layer(data, index, layer)?)
    }

    /// The decoder for a codestream and the header boxes that go with it.
    pub(crate) fn from_boxes(boxes: boxes::Boxes<'a>) -> Result<Decoder<'a>> {
        let cs = markers::parse(boxes.codestream)?;
        if let Some(&t) = cs.nlt.iter().find(|&&t| !matches!(t, 0 | 3 | 4)) {
            return Err(Jp2Error::Unsupported(match t {
                1 => "gamma non-linearity (NLT type 1)",
                2 => "lookup-table non-linearity (NLT type 2)",
                _ => "unknown non-linearity (NLT) type",
            }));
        }
        let siz = &cs.siz;
        let cod = cs.main.cod.expect("checked in parse");
        let c0 = cs.main_comp(0);
        let mapping = color::Mapping::new(Some(&boxes), siz, false);
        let color = boxes.color.unwrap_or(if mapping.kind == color::Kind::Gray {
            ColorSpace::Gray
        } else {
            ColorSpace::Srgb
        });
        let (gx, gy) = color::output_grid(siz);
        let (x0, y0) = (siz.x0.div_ceil(gx), siz.y0.div_ceil(gy));
        let info = Jp2Info {
            width: siz.x1.div_ceil(gx) - x0,
            height: siz.y1.div_ceil(gy) - y0,
            x0,
            y0,
            container: boxes.container,
            components: siz
                .comps
                .iter()
                .enumerate()
                .map(|(i, c)| ComponentInfo {
                    precision: c.prec,
                    signed: c.signed,
                    dx: c.dx,
                    dy: c.dy,
                    reversible: cs.main_comp(i).reversible,
                })
                .collect(),
            color,
            icc_profile: boxes.icc.clone(),
            levels: tile::min_levels(&cs),
            layers: cod.layers,
            progression: cod.prog,
            reversible: c0.reversible,
            tile_width: siz.tw,
            tile_height: siz.th,
            tiles_x: siz.tiles_x,
            tiles_y: siz.tiles_y,
            code_block: (1 << c0.cbw, 1 << c0.cbh),
            resolution: boxes.resolution,
            has_alpha: mapping.alpha.is_some(),
            premultiplied_alpha: matches!(mapping.alpha, Some((_, true))),
            has_palette: boxes.palette.is_some() && boxes.cmap.is_some(),
            output_precision: output_precision(&boxes, siz),
            gray: mapping.is_gray(),
            gray_alpha: mapping.kind == color::Kind::Gray && mapping.alpha.is_some(),
            comment: cs.comment.clone(),
            htj2k: cs.cap_ht
                || (0..siz.comps.len()).any(|c| cs.main_comp(c).style & markers::CBS_HT != 0),
        };
        Ok(Decoder { boxes, cs, info })
    }

    pub fn info(&self) -> &Jp2Info {
        &self.info
    }

    pub fn decode(&self, opts: &DecodeOptions) -> Result<Image> {
        let info = &self.info;
        if opts.reduce > info.levels {
            return Err(Jp2Error::InvalidRequest(
                "reduce exceeds the resolution levels",
            ));
        }
        let rd = opts.reduce as u32;
        let siz = &self.cs.siz;
        // Output coordinates are reduced reference-grid coordinates divided by the common
        // subsampling (see `color::output_grid`).
        let (gx, gy) = color::output_grid(siz);
        let to_out = |r: Rect| {
            Rect::new(
                ceil_shr(r.x0, rd).div_ceil(gx),
                ceil_shr(r.y0, rd).div_ceil(gy),
                ceil_shr(r.x1, rd).div_ceil(gx),
                ceil_shr(r.y1, rd).div_ceil(gy),
            )
        };
        let full = to_out(Rect::new(siz.x0, siz.y0, siz.x1, siz.y1));
        let out = match opts.region {
            None => full,
            Some(r) => {
                let x0 = full.x0 as u64 + r.x as u64;
                let y0 = full.y0 as u64 + r.y as u64;
                let x1 = x0 + r.w as u64;
                let y1 = y0 + r.h as u64;
                if r.w == 0 || r.h == 0 || x1 > full.x1 as u64 || y1 > full.y1 as u64 {
                    return Err(Jp2Error::InvalidRequest("region outside the image"));
                }
                Rect::new(x0 as u32, y0 as u32, x1 as u32, y1 as u32)
            }
        };
        if out.area() > MAX_OUTPUT_PIXELS {
            return Err(Jp2Error::TooLarge(out.area() * 4));
        }
        let sixteen = opts.depth == Depth::Sixteen;
        let mapping = color::Mapping::new(Some(&self.boxes), siz, sixteen);
        let gray = mapping.is_gray();
        let nch = if gray { 1 } else { 4 };
        let format = match (gray, sixteen) {
            (true, false) => PixelFormat::Gray8,
            (false, false) => PixelFormat::Rgba8,
            (true, true) => PixelFormat::Gray16,
            (false, true) => PixelFormat::Rgba16,
        };
        let n = out.area() as usize * nch;
        let _res = specular_core::budget::reserve_n(n as u64, if sixteen { 2 } else { 1 })?;
        let mut p8: Vec<u8> = if sixteen { Vec::new() } else { try_vec(n)? };
        let mut p16: Vec<u16> = if sixteen { try_vec(n)? } else { Vec::new() };
        let max_layers = opts.max_layers.unwrap_or(u16::MAX).max(1);
        let ow = out.w() as usize;
        let budget = specular_core::budget::current_budget();
        let truncated = with_threads(opts.parallelism, out.area(), || -> Result<bool> {
            let mut truncated = self.cs.truncated;
            for q in 0..siz.tiles_y as usize {
                if tile::cancelled(opts.cancel) {
                    return Err(Jp2Error::Cancelled);
                }
                // Tiles of this row intersecting the output.
                let mut row_tiles = Vec::new();
                let mut band = Rect::default();
                for p in 0..siz.tiles_x as usize {
                    let t = q * siz.tiles_x as usize + p;
                    let tr = siz.tile_rect(t);
                    let w = to_out(tr).intersect(&out);
                    if !w.is_empty() {
                        band = Rect::new(out.x0, w.y0, out.x1, w.y1);
                        row_tiles.push((t, w));
                    }
                }
                if row_tiles.is_empty() {
                    continue;
                }
                let results: Vec<Result<tile::TileOut>> = row_tiles
                    .par_iter()
                    .map(|&(t, w)| {
                        // Each tile reserves its buffers on a pool worker.
                        budget.run(|| {
                            // The tile's window in reduced reference-grid coordinates.
                            let tr = siz.tile_rect(t);
                            let rr = Rect::new(
                                ceil_shr(tr.x0, rd),
                                ceil_shr(tr.y0, rd),
                                ceil_shr(tr.x1, rd),
                                ceil_shr(tr.y1, rd),
                            );
                            let w = Rect::new(
                                w.x0.saturating_mul(gx),
                                w.y0.saturating_mul(gy),
                                w.x1.saturating_mul(gx),
                                w.y1.saturating_mul(gy),
                            )
                            .intersect(&rr);
                            tile::decode_tile(
                                &self.cs,
                                t,
                                &tile::TileRequest {
                                    reduce: opts.reduce,
                                    window: w,
                                    max_layers,
                                    cancel: opts.cancel,
                                    full: false,
                                },
                            )
                        })
                    })
                    .collect();
                let mut outs = Vec::with_capacity(results.len());
                for (res, &(_, w)) in results.into_iter().zip(&row_tiles) {
                    match res {
                        Ok(o) => {
                            truncated |= o.truncated;
                            outs.push((o, w));
                        }
                        Err(
                            e @ (Jp2Error::Cancelled
                            | Jp2Error::TooLarge(_)
                            | Jp2Error::NeedsMemory(_)
                            | Jp2Error::InvalidRequest(_)),
                        ) => return Err(e),
                        Err(_) => truncated = true,
                    }
                }
                // Assemble the band's rows.
                let r0 = (band.y0 - out.y0) as usize;
                let r1 = (band.y1 - out.y0) as usize;
                let assemble_row = |y: u32, o8: Option<&mut [u8]>, o16: Option<&mut [u16]>| {
                    let mut rows: Vec<Vec<u16>> = vec![Vec::new(); mapping.chans.len()];
                    let mut o8 = o8;
                    let mut o16 = o16;
                    for (to, w) in &outs {
                        let len = w.w() as usize;
                        for (ch, row) in rows.iter_mut().enumerate() {
                            row.resize(len, 0);
                            mapping.fill(ch, &to.comps, siz, w.x0, y, row);
                        }
                        let s = (w.x0 - out.x0) as usize * nch;
                        let e = s + len * nch;
                        mapping.pixels(
                            &rows,
                            len,
                            o8.as_deref_mut().map(|o| &mut o[s..e]),
                            o16.as_deref_mut().map(|o| &mut o[s..e]),
                        );
                    }
                };
                if sixteen {
                    p16[r0 * ow * nch..r1 * ow * nch]
                        .par_chunks_mut(ow * nch)
                        .enumerate()
                        .for_each(|(i, row)| assemble_row(band.y0 + i as u32, None, Some(row)));
                } else {
                    p8[r0 * ow * nch..r1 * ow * nch]
                        .par_chunks_mut(ow * nch)
                        .enumerate()
                        .for_each(|(i, row)| assemble_row(band.y0 + i as u32, Some(row), None));
                }
            }
            Ok(truncated)
        })?;
        Ok(Image {
            width: out.w(),
            height: out.h(),
            origin: (out.x0 - full.x0, out.y0 - full.y0),
            format,
            pixels: if sixteen {
                Pixels::U16(p16)
            } else {
                Pixels::U8(p8)
            },
            truncated,
            reduce: opts.reduce,
        })
    }
}

impl Decoder<'_> {
    /// Decode every component at its own sampling, before any colour mapping: native
    /// unsigned values (signed components offset by `2^(prec-1)`, precisions above 16 reduced
    /// to 16). For conformance testing against per-component references.
    #[doc(hidden)]
    pub fn decode_components(
        &self,
        reduce: u8,
        max_layers: Option<u16>,
    ) -> Result<Vec<RawComponent>> {
        Ok(self
            .components(reduce, max_layers, false, tile::Samples::native)?
            .into_iter()
            .map(|(width, height, samples)| RawComponent {
                width,
                height,
                samples,
            })
            .collect())
    }

    /// Every component at full resolution and precision (up to 31 bits): the DC-shifted
    /// unsigned values, row-major, with each component's width and height. For data packed
    /// into a codestream, such as GRIB2's JPEG 2000 template.
    #[doc(hidden)]
    pub fn decode_components_i32(&self) -> Result<Vec<(u32, u32, Vec<i32>)>> {
        self.components(0, None, true, tile::Samples::native_i32)
    }

    /// Every component at full resolution and precision (up to 32 bits): the DC-shifted
    /// unsigned values (signed components offset by `2^(prec-1)`), row-major, with each
    /// component's width and height. For OpenEXR's HTJ2K chunks.
    #[doc(hidden)]
    pub fn decode_components_u32(&self) -> Result<Vec<(u32, u32, Vec<u32>)>> {
        self.components(0, None, true, tile::Samples::native_u32)
    }

    fn components<T: Clone + Default>(
        &self,
        reduce: u8,
        max_layers: Option<u16>,
        full: bool,
        native: fn(&tile::Samples, usize, &mut [T]),
    ) -> Result<Vec<(u32, u32, Vec<T>)>> {
        if reduce > self.info.levels {
            return Err(Jp2Error::InvalidRequest(
                "reduce exceeds the resolution levels",
            ));
        }
        let rd = reduce as u32;
        let siz = &self.cs.siz;
        let crect = |c: &markers::CompSiz| {
            let (dx, dy) = (c.dx as u32, c.dy as u32);
            Rect::new(
                ceil_shr(siz.x0.div_ceil(dx), rd),
                ceil_shr(siz.y0.div_ceil(dy), rd),
                ceil_shr(siz.x1.div_ceil(dx), rd),
                ceil_shr(siz.y1.div_ceil(dy), rd),
            )
        };
        let mut out = Vec::with_capacity(siz.comps.len());
        let mut _res = specular_core::Reservation::none();
        for c in &siz.comps {
            let r = crect(c);
            if r.area() > MAX_OUTPUT_PIXELS {
                return Err(Jp2Error::TooLarge(r.area() * 2));
            }
            _res.grow(r.area().saturating_mul(std::mem::size_of::<T>() as u64))?;
            out.push((r.w(), r.h(), try_vec(r.area() as usize)?));
        }
        let mut truncated = self.cs.truncated;
        for t in 0..(siz.tiles_x * siz.tiles_y) as usize {
            let tr = siz.tile_rect(t);
            let window = Rect::new(
                ceil_shr(tr.x0, rd),
                ceil_shr(tr.y0, rd),
                ceil_shr(tr.x1, rd),
                ceil_shr(tr.y1, rd),
            );
            let req = tile::TileRequest {
                reduce,
                window,
                max_layers: max_layers.unwrap_or(u16::MAX).max(1),
                cancel: None,
                full,
            };
            let o = match tile::decode_tile(&self.cs, t, &req) {
                Ok(o) => o,
                Err(e @ (Jp2Error::TooLarge(_) | Jp2Error::InvalidRequest(_))) => return Err(e),
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            truncated |= o.truncated;
            for (c, co) in o.comps.iter().enumerate() {
                let full = crect(&siz.comps[c]);
                let r = co.rect.intersect(&full);
                let dst = &mut out[c];
                for y in r.y0..r.y1 {
                    let s = ((y - co.rect.y0) * co.rect.w() + (r.x0 - co.rect.x0)) as usize;
                    let d = ((y - full.y0) * full.w() + (r.x0 - full.x0)) as usize;
                    native(&co.data, s, &mut dst.2[d..d + r.w() as usize]);
                }
            }
        }
        if truncated {
            return Err(Jp2Error::Malformed("truncated or corrupt tile data"));
        }
        Ok(out)
    }
}

/// One component from [`Decoder::decode_components`].
#[doc(hidden)]
pub struct RawComponent {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u16>,
}

/// Whether `head` (the first bytes of a file) starts like a JP2-family file (JP2, JPX, JPM,
/// JPH: the 12-byte signature box) or a raw codestream (SOC followed by SIZ).
pub fn probe(head: &[u8]) -> bool {
    boxes::is_jp2(head) || boxes::is_codestream(head)
}

/// Parse the headers only.
pub fn read_info(data: &[u8]) -> Result<Jp2Info> {
    Ok(Decoder::new(data)?.info)
}

/// Decode `data` with `opts`.
pub fn decode(data: &[u8], opts: &DecodeOptions) -> Result<Image> {
    Decoder::new(data)?.decode(opts)
}

/// How many codestreams `data` holds: the top-level codestreams of a JPX file (fragmented ones
/// included, which fail to decode), 1 for a JP2 file or raw codestream, 0 for anything else.
/// Reads box headers only.
pub fn codestream_count(data: &[u8]) -> usize {
    boxes::codestream_count(data)
}

/// [`read_info`] for codestream `index` of a JPX file (a fragmented one is joined first).
pub fn read_info_at(data: &[u8], index: usize) -> Result<Jp2Info> {
    match boxes::defragment(data) {
        Some(d) => Ok(Decoder::new_at(&d, index)?.info),
        None => Ok(Decoder::new_at(data, index)?.info),
    }
}

/// [`decode`] for codestream `index` of a JPX file (a fragmented one is joined first).
pub fn decode_at(data: &[u8], index: usize, opts: &DecodeOptions) -> Result<Image> {
    match boxes::defragment(data) {
        Some(d) => Decoder::new_at(&d, index)?.decode(opts),
        None => Decoder::new_at(data, index)?.decode(opts),
    }
}

/// A JPX file with its fragmented codestreams (fragment table boxes, `ftbl`) joined: each
/// replaced by a contiguous codestream box in place, so codestream numbering and the other
/// boxes stay as they were. `None` when there is nothing to join. A table whose fragments the
/// file does not hold (kept in other files, past its end) stays a table, and that codestream
/// is refused. Callers that decode a file several times join it once and decode the result.
pub fn defragment(data: &[u8]) -> Option<Vec<u8>> {
    boxes::defragment(data)
}

/// Below this many output pixels `Parallelism::Auto` decodes on the calling thread.
pub const AUTO_PARALLEL_MIN_PIXELS: u64 = 1 << 20;

fn pool(n: usize) -> Option<Arc<rayon::ThreadPool>> {
    static POOLS: Mutex<Option<HashMap<usize, Arc<rayon::ThreadPool>>>> = Mutex::new(None);
    let mut g = POOLS.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    if let Some(p) = map.get(&n) {
        return Some(p.clone());
    }
    let p = Arc::new(
        specular_thread::rayon_pool("dec-jp2")
            .num_threads(n)
            .build()
            .ok()?,
    );
    map.insert(n, p.clone());
    Some(p)
}

/// Runs `f` on a pool of `par` threads. The caller's memory budget and share go with it
/// (rayon workers have their own thread-locals): reservations in `f` and in the parallel
/// work it starts ([`specular_core::BudgetHandle::run`]) count against the caller's budget.
fn with_threads<T: Send>(par: Parallelism, pixels: u64, f: impl FnOnce() -> T + Send) -> T {
    let budget = specular_core::budget::current_budget();
    let n = match par {
        Parallelism::Single => 1,
        Parallelism::Threads(n) => n.max(1),
        Parallelism::Auto => {
            if pixels < AUTO_PARALLEL_MIN_PIXELS {
                1
            } else {
                return f();
            }
        }
    };
    match pool(n) {
        Some(p) => p.install(|| budget.run(f)),
        None => f(),
    }
}

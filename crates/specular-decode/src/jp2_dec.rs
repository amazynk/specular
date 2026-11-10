//! JPEG 2000 Part 1 (JP2, JPX, raw J2K codestreams) through `specular-jp2`, Specular's own
//! decoder.
//!
//! * **Pages.** A JPX file with more than one codestream shows each as a page, in file order,
//!   labelled with its size: the count comes from a walk of the top-level box headers at open,
//!   and a page parses only its own codestream's headers.
//! * **Composition.** A JPX file whose composition box (`comp`) does more than show its first
//!   codestream whole has the first frame of that composition as page 0 ("merged"), the
//!   codestreams following as pages 1‥: layers cropped, scaled (nearest) and placed on the
//!   canvas, with their alpha, opacity and chroma keys, each layer registered to its
//!   codestream (`creg`: another codestream, offsets, scaling). Scales decode the layers at
//!   reduced resolution; regions are cut from the cached whole level. Compositions the crate
//!   does not take (a layer built from several codestreams, damaged instructions) fall back
//!   to the codestreams alone. An animation (instruction sets with frame times) plays in the
//!   animation player, its layers decoded once.
//! * **JPM** (Part 6) files are compound pages, each a page of the file: see `jpm_dec`.
//! * **Fragments.** A JPX file whose codestreams are split into fragments (`ftbl`) is joined
//!   once at open; fragments in other files are refused.
//!
//! * **Scales.** 1/2/4/8 map to discarded resolution levels (`reduce`), so a 1/8 overview does
//!   tier-1 and the inverse wavelet only for the kept levels. Images with fewer decomposition
//!   levels than a scale needs are decoded at the smallest available level and box-filtered.
//! * **Regions.** Decoded directly: only the tiles, precincts and code-blocks touching the
//!   region are decoded (`fast_regions`).
//! * **Colour.** The ICC profile (`colr` method 2) is passed through [`ImageInfo`]; opaque
//!   colour images are delivered as planar RGB so the viewer's ICC transform applies to them.
//!   Sources deeper than 8 bits (components, or the columns of a palette) keep 16 bits at
//!   any size, with or without an ICC profile: grey as `Gray16`, grey + alpha as
//!   `GrayAlpha16`, colour as `Rgba16` (sRGB-coded, or in the profile's space). SDR wide
//!   rasters are tiled like 8-bit ones.
//! * HTJ2K (Part 15: `.jph` files, raw `.jhc`/`.j2c` codestreams, HT code-blocks in a J2K
//!   codestream, and "mixed" tile-components decided per code-block) decodes through the
//!   same path, with the same scales and regions. DICOM's HTJ2K transfer syntaxes reach it
//!   through `specular-dicom`.
//! * **Georeference.** A single-codestream JP2 file's GeoJP2 box (a degenerate GeoTIFF in a
//!   `uuid` box) or, without one, its GMLJP2 `gml:RectifiedGrid` places it on the map, as
//!   GDAL reads them (ERDAS/ECW SDK, Kakadu and GDAL writers). Sidecar files are read only
//!   when the file has neither.

use crate::whole;
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    PageLabel, Rect, Result, Session,
};
use parking_lot::Mutex;
use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, RasterPlane, Source,
};
use specular_jp2::{
    Animation, ColorSpace, Composition, Container, DecodeOptions, Depth, Image, Jp2Error, Jp2Info,
    Parallelism, PixelFormat, Pixels, Region,
};
use std::sync::Arc;
use std::time::Duration;

pub struct Jp2Decoder;

pub(crate) fn map_err(e: Jp2Error) -> DecodeError {
    match e {
        Jp2Error::NotJp2 => DecodeError::UnsupportedFormat,
        Jp2Error::Truncated(s) => DecodeError::Truncated(s.into()),
        Jp2Error::Malformed(s) => DecodeError::Malformed(s.into()),
        Jp2Error::Unsupported(s) => DecodeError::Unsupported(s.into()),
        Jp2Error::TooLarge(_) => DecodeError::TooLarge,
        Jp2Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        Jp2Error::Cancelled => DecodeError::Cancelled,
        Jp2Error::InvalidRequest(s) => DecodeError::Malformed(format!("bad request: {s}")),
        e => DecodeError::Malformed(e.to_string()),
    }
}

fn plane(width: u32, height: u32, bpp: usize, data: Vec<u8>) -> RasterPlane {
    RasterPlane {
        width,
        height,
        stride: width as usize * bpp,
        data,
        h_div: 1,
        v_div: 1,
    }
}

/// Native-endian bytes of samples `keep` of the first `n` groups of `of` `u16` samples (zeros
/// where `d` is short).
fn ne_bytes(d: &[u16], n: usize, keep: &[usize], of: usize) -> Vec<u8> {
    let k = keep.len();
    let mut out = vec![0u8; n * k * 2];
    if k == of {
        for (o, v) in out.chunks_exact_mut(2).zip(d) {
            o.copy_from_slice(&v.to_ne_bytes());
        }
        return out;
    }
    for (o, px) in out.chunks_exact_mut(k * 2).zip(d.chunks_exact(of)) {
        for (o, &c) in o.chunks_exact_mut(2).zip(keep) {
            o.copy_from_slice(&px[c].to_ne_bytes());
        }
    }
    out
}

/// Converts a decoded image to a raster at `scale`. `planar` splits opaque RGBA8 into R, G, B
/// planes; 16-bit output stays compact: grey as `Gray16`, grey + alpha (`gray_alpha`, which
/// the crate delivers as RGBA with the grey replicated) as `GrayAlpha16`, colour as `Rgba16`
/// (sRGB transfer).
fn to_raster(img: Image, scale: u32, planar: bool, gray_alpha: bool) -> Raster {
    let (w, h) = (img.width, img.height);
    let n = w as usize * h as usize;
    let (layout, planes) = match (img.format, img.pixels) {
        (PixelFormat::Gray8, Pixels::U8(d)) => (PixelLayout::Gray, vec![plane(w, h, 1, d)]),
        (PixelFormat::Rgba8, Pixels::U8(d)) if planar => {
            let mut p = [vec![0u8; n], vec![0u8; n], vec![0u8; n]];
            for (i, px) in d.chunks_exact(4).enumerate().take(n) {
                p[0][i] = px[0];
                p[1][i] = px[1];
                p[2][i] = px[2];
            }
            let [r, g, b] = p;
            (
                PixelLayout::Rgb,
                vec![plane(w, h, 1, r), plane(w, h, 1, g), plane(w, h, 1, b)],
            )
        }
        (PixelFormat::Rgba8, Pixels::U8(d)) => (PixelLayout::Rgba, vec![plane(w, h, 4, d)]),
        (PixelFormat::Gray16, Pixels::U16(d)) => (
            PixelLayout::Gray16,
            vec![plane(w, h, 2, ne_bytes(&d, n, &[0], 1))],
        ),
        (PixelFormat::Rgba16, Pixels::U16(d)) if gray_alpha => (
            PixelLayout::GrayAlpha16,
            vec![plane(w, h, 4, ne_bytes(&d, n, &[0, 3], 4))],
        ),
        (PixelFormat::Rgba16, Pixels::U16(d)) => (
            PixelLayout::Rgba16(HdrEncoding::SRGB),
            vec![plane(w, h, 8, ne_bytes(&d, n, &[0, 1, 2, 3], 4))],
        ),
        // The crate pairs 8-bit formats with `U8` and 16-bit ones with `U16`.
        (_, _) => (PixelLayout::Gray, vec![plane(w, h, 1, vec![0; n])]),
    };
    Raster {
        width: w,
        height: h,
        origin: img.origin,
        scale,
        layout,
        planes,
        truncated: img.truncated,
    }
}

fn coding(i: &Jp2Info) -> String {
    let mut s = match i.container {
        Container::Codestream => "J2K codestream".to_string(),
        Container::Jp2 => "JP2".to_string(),
        Container::Jpx => "JPX (first codestream)".to_string(),
    };
    if i.htj2k {
        s += ", HTJ2K";
    }
    s += if i.reversible {
        ", lossless 5/3"
    } else {
        ", lossy 9/7"
    };
    let bits = i.components.iter().map(|c| c.precision).max().unwrap_or(8);
    s += &format!(", {bits}-bit");
    if i.components.iter().any(|c| c.signed) {
        s += " signed";
    }
    s += &format!(
        ", {} levels, {} layers, {:?}",
        i.levels, i.layers, i.progression
    );
    if i.tiles_x * i.tiles_y > 1 {
        s += &format!(
            ", {}×{} tiles of {}×{}",
            i.tiles_x, i.tiles_y, i.tile_width, i.tile_height
        );
    }
    match i.color {
        ColorSpace::Sycc | ColorSpace::Esycc => s += ", sYCC",
        ColorSpace::Cmyk => s += ", CMYK",
        ColorSpace::Icc => s += ", ICC",
        ColorSpace::Other(c) => s += &format!(", colour space {c}"),
        ColorSpace::Srgb | ColorSpace::Gray => {}
    }
    if i.has_palette {
        s += ", palette";
    }
    s
}

impl Decoder for Jp2Decoder {
    fn format(&self) -> Format {
        Format::Jpeg2000
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_jp2::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[
            "jp2", "j2k", "j2c", "jpc", "jpf", "jpx", "jph", "jhc", "jpm", "jpgm",
        ]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // JPM compound pages: drawn from their layout objects (`jpm_dec`), fragment tables
        // and all.
        if specular_jp2::is_jpm(source.bytes()) {
            return crate::jpm_dec::open_page(source, 0);
        }
        // Codestreams split into fragments: joined once, the joined file decoded from then on.
        let source = match specular_jp2::defragment(source.bytes()) {
            Some(joined) => Arc::new(Source::from_entry(
                source.path().to_path_buf(),
                &source,
                joined,
            )),
            None => source,
        };
        let data = source.bytes();
        let streams = (specular_jp2::codestream_count(data) as u32).max(1);
        // A composition that only shows the first codestream whole is no page of its own.
        let anim = specular_jp2::animation(data).filter(|a| a.frames.len() > 1);
        let comp = specular_jp2::composition(data)
            .filter(|c| {
                anim.is_some()
                    || specular_jp2::read_info_at(data, 0)
                        .is_ok_and(|i| !c.plain((i.width, i.height)))
            })
            .map(|c| Arc::new((c, anim.map(Arc::new))));
        let pages = streams + u32::from(comp.is_some());
        let s = open_page(source, 0, pages, comp)?;
        if pages != 1 {
            return Ok(s);
        }
        let i = s.info();
        match embedded_geo(s.source().bytes(), i.width, i.height) {
            Some(g) => Ok(Arc::new(crate::georaster_dec::GeoWrap {
                inner: s,
                geo: Some(Arc::new(g)),
                paged: None,
            })),
            None => Ok(s),
        }
    }
}

/// The georeference a JP2 file carries in its own boxes: GeoJP2 first (GDAL's order), then
/// the first GMLJP2 grid.
pub(crate) fn embedded_geo(data: &[u8], width: u32, height: u32) -> Option<crate::geo::GeoRef> {
    use crate::geo::{self, RasterGeo};
    let b = specular_jp2::geo_boxes(data);
    let levels = vec![(width, height)];
    if let Some(tags) = b.geojp2.and_then(specular_tiff::GeoTags::of_first_ifd)
        && let Some(mut g) = geo::from_geotiff(&tags, width, height, levels.clone())
    {
        g.crs_note = Some("GeoJP2 box".into());
        return Some(g);
    }
    let r = b
        .gml
        .iter()
        .find_map(|x| geo::specular_gis::gml::rectified_grid(x))?;
    let note = match (&r.crs, r.srs_name.is_empty()) {
        (Some(_), _) => "GMLJP2 box".to_string(),
        (None, true) => "GMLJP2 box, no CRS given".to_string(),
        (None, false) => format!("GMLJP2 box, CRS not known: {}", r.srs_name),
    };
    Some(geo::raster(
        r.to_map,
        r.crs,
        Some(note),
        (width, height),
        RasterGeo {
            levels,
            keys: r.keys,
            keys_kind: "header",
            ..Default::default()
        },
    ))
}

/// Page `page` of `pages` (1 for anything but a JPX file with several codestreams or a
/// composition): the composition `comp` as page 0 when there is one, then the codestreams.
fn open_page(
    source: Arc<Source>,
    page: u32,
    pages: u32,
    comp: Option<Arc<Comp>>,
) -> Result<Arc<dyn Session>> {
    let offset = u32::from(comp.is_some());
    if let Some(c) = comp.as_ref().filter(|_| page == 0) {
        return open_composite(source, c.clone(), pages);
    }
    let stream = page - offset;
    {
        let i = specular_jp2::read_info_at(source.bytes(), stream as usize).map_err(map_err)?;
        // Deeper than 8 bits (a palette's columns count, not its indices): kept at 16 bits,
        // with or without an ICC profile (the viewer's transform reads 16-bit input).
        let bits = i.output_precision;
        let wide = bits > 8;
        let info = ImageInfo {
            format: Format::Jpeg2000,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: i.icc_profile.clone().map(Arc::new),
            has_alpha: i.has_alpha,
            bit_depth: bits.min(16),
            progressive: false,
            coding: if pages > 1 {
                let n = pages - offset;
                format!("{}, codestream {} of {n}", coding(&i), stream + 1)
            } else {
                coding(&i)
            },
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(Jp2Session {
            source,
            info,
            jp2: i,
            wide,
            levels: Mutex::new(Vec::new()),
            page,
            pages,
            stream,
            comp,
        }))
    }
}

/// A JPX file's composition: its first frame, and every frame when it is an animation.
type Comp = (Composition, Option<Arc<Animation>>);

/// Page 0 of a JPX file with a composition: its first frame on the canvas (or its animation).
fn open_composite(source: Arc<Source>, both: Arc<Comp>, pages: u32) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let comp = &both.0;
    let anim = both.1.as_ref();
    let mut layers: Vec<usize> = match anim {
        Some(a) => a
            .frames
            .iter()
            .flat_map(|f| f.placements.iter().map(|p| p.layer))
            .collect(),
        None => comp.placements.iter().map(|p| p.layer).collect(),
    };
    layers.sort_unstable();
    layers.dedup();
    let infos: Vec<Jp2Info> = layers
        .iter()
        .filter_map(|&l| specular_jp2::read_info_at(data, comp.codestream(l)).ok())
        .collect();
    let Some(first) = infos.first() else {
        return Err(DecodeError::Malformed(
            "no decodable compositing layer".into(),
        ));
    };
    let wide = infos.iter().any(|i| i.output_precision > 8);
    // The layers are drawn in their own colour space: a profile is kept when they share it.
    let icc = first
        .icc_profile
        .clone()
        .filter(|p| infos.iter().all(|i| i.icc_profile.as_ref() == Some(p)));
    let n = comp.placements.len();
    let mut coding = format!(
        "JPX, composition of {n} layer{} on a {}×{} canvas",
        if n == 1 { "" } else { "s" },
        comp.width,
        comp.height
    );
    if let Some(a) = anim {
        coding += &format!(", animation of {} frames", a.frames.len());
    } else if comp.animated {
        coding += ", first frame of an animation";
    }
    if infos.len() < layers.len() {
        coding += &format!(", {} layer(s) not decodable", layers.len() - infos.len());
    }
    let px = |i: &Jp2Info| u64::from(i.width) * u64::from(i.height);
    // One layer decodes at a time: its components' plane and wavelet buffers, 12 bytes each.
    let layer_work = infos
        .iter()
        .map(|i| px(i) * i.components.len().clamp(1, 4) as u64 * 12)
        .max()
        .unwrap_or(0);
    let layer_bytes = infos.iter().map(|i| px(i) * if wide { 8 } else { 4 }).sum();
    let info = ImageInfo {
        format: Format::Jpeg2000,
        width: comp.width,
        height: comp.height,
        orientation: Orientation::NORMAL,
        icc_profile: icc.map(Arc::new),
        has_alpha: true,
        bit_depth: if wide { 16 } else { 8 },
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    let animation = anim.map(|a| AnimationInfo {
        frame_count: Some(a.frames.len() as u32),
        loop_count: a.loop_count,
    });
    Ok(Arc::new(JpxComposite {
        source,
        info,
        comp: both,
        wide,
        layer_work,
        layer_bytes,
        levels: whole::Levels::new(),
        pages,
        animation,
    }))
}

/// The first frame of a JPX composition (page 0 of the file), and its animation.
struct JpxComposite {
    source: Arc<Source>,
    info: ImageInfo,
    comp: Arc<Comp>,
    /// Decode to 16 bits (a layer is deeper than 8).
    wide: bool,
    /// Peak working memory of one full-size layer decode, and every layer's decoded pixels.
    layer_work: u64,
    layer_bytes: u64,
    levels: whole::Levels,
    pages: u32,
    animation: Option<AnimationInfo>,
}

impl JpxComposite {
    fn options<'a>(
        &self,
        scale: u32,
        parallel: bool,
        cancel: &'a CancelToken,
    ) -> DecodeOptions<'a> {
        DecodeOptions {
            reduce: scale.max(1).trailing_zeros().min(31) as u8,
            region: None,
            max_layers: None,
            depth: if self.wide {
                Depth::Sixteen
            } else {
                Depth::Eight
            },
            parallelism: if parallel {
                Parallelism::Auto
            } else {
                Parallelism::Single
            },
            cancel: Some(cancel.flag()),
        }
    }
}

/// The frames of a JPX animation, drawn one by one with the layers decoded once.
struct JpxFrames {
    source: Arc<Source>,
    anim: Arc<Animation>,
    drawer: specular_jp2::Compositor,
    scale: u32,
    wide: bool,
    index: usize,
}

impl Frames for JpxFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let frame = self.anim.frames.get(self.index)?;
        let i = self.index;
        self.index += 1;
        let opts = DecodeOptions {
            reduce: self.scale.trailing_zeros().min(31) as u8,
            region: None,
            max_layers: None,
            depth: if self.wide {
                Depth::Sixteen
            } else {
                Depth::Eight
            },
            parallelism: Parallelism::Auto,
            cancel: Some(cancel.flag()),
        };
        Some(
            self.drawer
                .draw(self.source.bytes(), frame, &opts)
                .map_err(map_err)
                .map(|img| Frame {
                    index: i as u32,
                    raster: to_raster(img, self.scale, false, false),
                    // A frame shown for ever is the last one: it stays up.
                    delay: Duration::from_millis(frame.delay_ms.unwrap_or(0).into()),
                }),
        )
    }
    fn rewind(&mut self) -> Result<()> {
        self.index = 0;
        Ok(())
    }
    fn delays(&self) -> Option<Vec<Duration>> {
        Some(
            self.anim
                .frames
                .iter()
                .map(|f| Duration::from_millis(f.delay_ms.unwrap_or(0).into()))
                .collect(),
        )
    }
}

impl Session for JpxComposite {
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        let Some(anim) = self.comp.1.clone() else {
            return Err(DecodeError::Unsupported("not animated".into()));
        };
        Ok(Box::new(JpxFrames {
            source: self.source.clone(),
            anim,
            drawer: specular_jp2::Compositor::default(),
            scale: scale.max(1),
            wide: self.wide,
            index: 0,
        }))
    }
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn alpha(&self) -> Option<Alpha> {
        Some(Alpha::Straight)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        0
    }
    fn page_label(&self) -> Option<PageLabel> {
        Some(PageLabel::Merged)
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(
            self.source.clone(),
            page,
            self.pages,
            Some(self.comp.clone()),
        )
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        specular_log::debug!(target: specular_log::DECODE, "jp2: JPX composition at 1/{}", req.scale.max(1));
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |s| s,
            |s| {
                let opts = self.options(s, req.parallel, cancel);
                let img = specular_jp2::decode_composite(self.source.bytes(), &self.comp.0, &opts)
                    .map_err(map_err)?;
                Ok(to_raster(img, s, false, false))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.max(1);
        if self.levels.has(s) {
            return 0;
        }
        let shrink = u64::from(s) * u64::from(s);
        let bpp = if self.wide { 8 } else { 4 };
        // The level and the request's copy of it, the decoded layers, one layer's decode.
        2 * whole::level_bytes(self.info.width, self.info.height, s, bpp)
            + (self.layer_bytes + self.layer_work) / shrink
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
}

struct Jp2Session {
    source: Arc<Source>,
    info: ImageInfo,
    jp2: Jp2Info,
    /// Decode to 16 bits.
    wide: bool,
    /// Whole levels for scales finer than the codestream's coarsest resolution.
    levels: Mutex<Vec<Arc<Raster>>>,
    /// The page shown, and how many the file has.
    page: u32,
    pages: u32,
    /// The codestream shown: `page`, less one when page 0 is the composition `comp`.
    stream: u32,
    comp: Option<Arc<Comp>>,
}

impl Jp2Session {
    fn options<'a>(
        &self,
        reduce: u8,
        region: Option<Region>,
        parallel: bool,
        cancel: &'a CancelToken,
    ) -> DecodeOptions<'a> {
        DecodeOptions {
            reduce,
            region,
            max_layers: None,
            depth: if self.wide {
                Depth::Sixteen
            } else {
                Depth::Eight
            },
            parallelism: if parallel {
                Parallelism::Auto
            } else {
                Parallelism::Single
            },
            cancel: Some(cancel.flag()),
        }
    }

    /// Whether opaque colour output is split into planes (for the ICC transform).
    fn planar(&self) -> bool {
        !self.jp2.gray && !self.jp2.has_alpha
    }

    fn bytes_per_pixel(&self) -> u64 {
        if self.wide {
            if self.jp2.gray {
                2
            } else if self.jp2.gray_alpha {
                4
            } else {
                8
            }
        } else if self.jp2.gray {
            1
        } else if self.jp2.has_alpha {
            4
        } else {
            3
        }
    }

    /// Discarded levels for `scale` and the box-filter factor left over.
    fn reduce_for(&self, scale: u32) -> (u8, u32) {
        let want = scale.max(1).trailing_zeros().min(31) as u8;
        let r = want.min(self.jp2.levels);
        (r, 1 << (want - r))
    }

    fn direct(
        &self,
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let (reduce, _) = self.reduce_for(scale);
        let (rw, rh) = self.jp2.reduced_size(reduce);
        // The level may be a pixel off `ceil(width / scale)` when the image origin is not 0.
        let region = region
            .map(|r| {
                let x = r.x.min(rw.saturating_sub(1));
                let y = r.y.min(rh.saturating_sub(1));
                Region {
                    x,
                    y,
                    w: r.w.min(rw - x).max(1),
                    h: r.h.min(rh - y).max(1),
                }
            })
            .filter(|r| (r.x, r.y, r.w, r.h) != (0, 0, rw, rh));
        let opts = self.options(reduce, region, parallel, cancel);
        let img = specular_jp2::decode_at(self.source.bytes(), self.stream as usize, &opts)
            .map_err(map_err)?;
        Ok(to_raster(img, scale, self.planar(), self.jp2.gray_alpha))
    }

    /// Whole level 1/`scale` when the codestream has too few levels: decoded at its coarsest
    /// resolution and box-filtered, cached.
    fn filtered_level(
        &self,
        scale: u32,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Arc<Raster>> {
        if let Some(l) = self.levels.lock().iter().find(|l| l.scale == scale) {
            return Ok(l.clone());
        }
        let (reduce, _) = self.reduce_for(scale);
        let mut src = self.direct(1 << reduce, None, parallel, cancel)?;
        src.scale = 1 << reduce;
        let lvl = Arc::new(whole::downscale(
            &src,
            self.info.width,
            self.info.height,
            scale,
        ));
        let mut g = self.levels.lock();
        if !g.iter().any(|l| l.scale == scale) {
            g.push(lvl.clone());
        }
        Ok(lvl)
    }
}

impl Session for Jp2Session {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        use specular_jp2::ColorSpace as C;
        if self.jp2.has_palette {
            return Some("Palette");
        }
        Some(match self.jp2.color {
            C::Srgb => "RGB",
            C::Gray => "Greyscale",
            C::Sycc | C::Esycc => "YCbCr",
            C::Cmyk => "CMYK",
            C::Icc => return crate::icc_colour_model(self.jp2.icc_profile.as_deref()?),
            // The other enumerated spaces of ISO/IEC 15444-2 table M.25.
            C::Other(0 | 15 | 17) => "Greyscale",
            C::Other(1 | 3 | 4 | 9 | 22 | 23) => "YCbCr",
            C::Other(13) => "YCCK",
            C::Other(14) => "Lab",
            C::Other(16 | 20 | 21) => "RGB",
            C::Other(_) => return None,
        })
    }
    /// The `cdef` opacity channel (type 2: premultiplied), or a fourth / second component
    /// taken as alpha without one; with a palette, its transparency.
    fn alpha(&self) -> Option<Alpha> {
        self.jp2.has_alpha.then_some(if self.jp2.has_palette {
            Alpha::Transparency
        } else if self.jp2.premultiplied_alpha {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        if self.colour_model() != Some("YCbCr") {
            return None;
        }
        let [y, cb, ..] = self.jp2.components.as_slice() else {
            return None;
        };
        if y.dx == 0 || y.dy == 0 || cb.dx % y.dx != 0 || cb.dy % y.dy != 0 {
            return None;
        }
        crate::subsampling_name(u32::from(cb.dx / y.dx), u32::from(cb.dy / y.dy))
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if self.pages < 2 {
            return Err(DecodeError::Unsupported("no pages".into()));
        }
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(self.source.clone(), page, self.pages, self.comp.clone())
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        match req.region {
            None => {
                specular_log::debug!(target: specular_log::DECODE, "jp2: decode 1/{scale} whole image")
            }
            Some(r) => {
                specular_log::trace!(target: specular_log::DECODE, "jp2: 1/{scale} region {}×{}+{}+{}", r.w, r.h, r.x, r.y)
            }
        }
        let (_, extra) = self.reduce_for(scale);
        if extra > 1 {
            let l = self.filtered_level(scale, req.parallel, cancel)?;
            return Ok(match req.region {
                Some(r) => crate::jpeg::crop(&l, r),
                None => (*l).clone(),
            });
        }
        self.direct(scale, req.region, req.parallel, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        let (reduce, extra) = self.reduce_for(scale);
        let (rw, rh) = self.jp2.reduced_size(reduce);
        let (w, h) = match req.region {
            Some(r) if extra == 1 => (r.w.min(rw), r.h.min(rh)),
            _ => (rw, rh),
        };
        let px = u64::from(w) * u64::from(h);
        // Per decoded component: the reconstructed plane and two wavelet buffers of 4 bytes
        // per sample. Tiles are decoded a tile row at a time.
        let rows = u64::from(self.jp2.tiles_y.max(1));
        let ncomp = self.jp2.components.len().clamp(1, 4) as u64;
        let work = px.div_ceil(rows).max(px.min(1 << 22)) * ncomp * 12;
        // The 8- or 16-bit output, and the crate's own output it is copied from: interleaved
        // RGBA8 split into planes, or 16-bit samples (grey + alpha as RGBA16) made compact.
        let src = match (self.wide, self.planar()) {
            (true, _) if self.jp2.gray => 2,
            (true, _) => 8,
            (false, true) => 4,
            (false, false) => 0,
        };
        let out = px * (self.bytes_per_pixel() + src);
        work + out
    }
    fn memory_bytes(&self) -> usize {
        self.levels.lock().iter().map(|r| r.memory_bytes()).sum()
    }
    /// Drops the cached box-filtered levels.
    fn trim_memory(&self) -> usize {
        whole::drop_levels(&self.levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-jp2/tests/data/"
    );

    fn open(name: &str) -> Arc<dyn Session> {
        let bytes = std::fs::read(format!("{DATA}{name}")).unwrap();
        assert!(Jp2Decoder.probe(&bytes[..bytes.len().min(crate::PROBE_HEAD)]));
        Jp2Decoder
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    #[test]
    fn probe_signatures() {
        let mut jp2 = vec![0, 0, 0, 12, b'j', b'P', b' ', b' ', 0x0D, 0x0A, 0x87, 0x0A];
        assert!(Jp2Decoder.probe(&jp2));
        jp2[4] = b'J';
        assert!(!Jp2Decoder.probe(&jp2));
        assert!(Jp2Decoder.probe(&[0xFF, 0x4F, 0xFF, 0x51, 0, 0x29]));
        assert!(!Jp2Decoder.probe(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!Jp2Decoder.probe(&[0xFF, 0x4F]));
        assert!(!Jp2Decoder.probe(&[]));
    }

    #[test]
    fn scales_match_levels() {
        let c = CancelToken::new();
        for name in [
            "rgb_rev.j2k",
            "gray_rev.jp2",
            "graya_rev.jp2",
            "rgb_nl1.j2k",
            "palette.jp2",
        ] {
            let s = open(name);
            let i = s.info().clone();
            assert_eq!(i.format, Format::Jpeg2000);
            for scale in [1, 2, 4, 8] {
                let r = s.decode(&DecodeRequest::full(scale), &c).unwrap();
                assert_eq!(
                    (r.width, r.height),
                    (i.width.div_ceil(scale), i.height.div_ceil(scale)),
                    "{name} 1/{scale}"
                );
                assert_eq!(r.scale, scale);
                assert!(!r.truncated, "{name}");
                for p in &r.planes {
                    assert_eq!(p.data.len(), p.stride * p.height as usize);
                }
            }
        }
    }

    #[test]
    fn layouts() {
        let c = CancelToken::new();
        let l = |n: &str| open(n).decode(&DecodeRequest::full(1), &c).unwrap().layout;
        assert_eq!(l("rgb_rev.j2k"), PixelLayout::Rgb);
        assert_eq!(l("gray_rev.jp2"), PixelLayout::Gray);
        assert_eq!(l("graya_rev.jp2"), PixelLayout::Rgba);
        assert_eq!(l("palette.jp2"), PixelLayout::Rgb);
        assert_eq!(l("gray12_rev.j2k"), PixelLayout::Gray16);
        assert_eq!(l("gray16_rev.j2k"), PixelLayout::Gray16);
        assert_eq!(l("graya16_rev.jp2"), PixelLayout::GrayAlpha16);
        assert_eq!(open("gray16_rev.j2k").info().bit_depth, 16);
    }

    /// 16-bit grey + alpha keeps the crate's grey and alpha samples (2 of its 4), at every
    /// scale, in regions, and in filtered levels.
    #[test]
    fn gray_alpha16_compact() {
        let bytes = std::fs::read(format!("{DATA}graya16_rev.jp2")).unwrap();
        let s = open("graya16_rev.jp2");
        let c = CancelToken::new();
        for scale in [1u32, 2, 4, 8] {
            let r = s.decode(&DecodeRequest::full(scale), &c).unwrap();
            assert_eq!(r.layout, PixelLayout::GrayAlpha16);
            assert_eq!(r.planes[0].stride, r.width as usize * 4);
            if scale > 2 {
                continue;
            }
            let img = specular_jp2::decode(
                &bytes,
                &DecodeOptions {
                    reduce: scale.trailing_zeros() as u8,
                    depth: Depth::Sixteen,
                    ..Default::default()
                },
            )
            .unwrap();
            let Pixels::U16(d) = img.pixels else { panic!() };
            let want: Vec<u8> = d
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[3]])
                .flat_map(u16::to_ne_bytes)
                .collect();
            assert!(d.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]));
            assert_eq!(r.planes[0].data, want, "1/{scale}");
        }
    }

    #[test]
    fn wide_matches_eight_bit() {
        // The 16-bit raster narrowed to 8 bits matches the crate's own 8-bit output.
        let bytes = std::fs::read(format!("{DATA}gray12_rev.j2k")).unwrap();
        let s = open("gray12_rev.j2k");
        let r = s
            .decode(&DecodeRequest::full(2), &CancelToken::new())
            .unwrap();
        let eight = specular_jp2::decode(
            &bytes,
            &DecodeOptions {
                reduce: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let Pixels::U8(e) = eight.pixels else {
            panic!()
        };
        let p = &r.planes[0].data;
        let mut maxd = 0i32;
        for (i, &v8) in e.iter().enumerate() {
            let v = u16::from_ne_bytes([p[i * 2], p[i * 2 + 1]]);
            maxd = maxd.max(((v as i32 * 255 + 32767) / 65535 - v8 as i32).abs());
        }
        assert!(maxd <= 1, "max diff {maxd}");
    }

    #[test]
    fn regions_match_full_decode() {
        let c = CancelToken::new();
        for name in ["rgb_rev.j2k", "gray_tiles.j2k", "graya_rev.jp2"] {
            let s = open(name);
            for scale in [1, 2, 8] {
                let full = s.decode(&DecodeRequest::full(scale), &c).unwrap();
                let rect = Rect {
                    x: full.width / 3,
                    y: full.height / 4,
                    w: (full.width / 2).max(1),
                    h: (full.height / 2).max(1),
                };
                let req = DecodeRequest {
                    region: Some(rect),
                    ..DecodeRequest::full(scale)
                };
                let r = s.decode(&req, &c).unwrap();
                assert!(r.origin.0 <= rect.x && r.origin.1 <= rect.y, "{name}");
                assert!(r.origin.0 + r.width >= rect.x + rect.w, "{name}");
                assert!(r.origin.1 + r.height >= rect.y + rect.h, "{name}");
                for (pf, pr) in full.planes.iter().zip(&r.planes) {
                    let bpp = pf.bytes_per_pixel();
                    for y in 0..pr.height as usize {
                        let fy = y + r.origin.1 as usize;
                        let fx = r.origin.0 as usize * bpp;
                        let n = pr.width as usize * bpp;
                        assert_eq!(
                            &pr.data[y * pr.stride..y * pr.stride + n],
                            &pf.data[fy * pf.stride + fx..fy * pf.stride + fx + n],
                            "{name} 1/{scale} row {y}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cancelled_request() {
        let s = open("rgb_rev.j2k");
        let c = CancelToken::new();
        c.cancel();
        assert!(matches!(
            s.decode(&DecodeRequest::full(1), &c),
            Err(DecodeError::Cancelled)
        ));
    }

    #[test]
    fn memory_estimate_is_positive() {
        let s = open("rgb_rev.j2k");
        let full = s.decode_memory(&DecodeRequest::full(1));
        let eighth = s.decode_memory(&DecodeRequest::full(8));
        assert!(full > eighth && eighth > 0);
    }

    #[test]
    fn truncated_never_panics() {
        let bytes = std::fs::read(format!("{DATA}rgb_rev.j2k")).unwrap();
        let c = CancelToken::new();
        for n in (0..bytes.len()).step_by(bytes.len() / 50 + 1) {
            let src = Arc::new(Source::from_vec("t.j2k", bytes[..n].to_vec()));
            if let Ok(s) = Jp2Decoder.open(src) {
                let _ = s.decode(&DecodeRequest::full(2), &c);
                let _ = s.decode(
                    &DecodeRequest {
                        region: Some(Rect {
                            x: 10,
                            y: 10,
                            w: 40,
                            h: 30,
                        }),
                        ..DecodeRequest::full(1)
                    },
                    &c,
                );
            }
        }
    }

    fn jp2_box(t: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut b = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(t);
        b.extend_from_slice(payload);
        b
    }

    /// Wrap a raw codestream of `w`×`h`, `nc` components of `bpc` bits, in a JP2 with
    /// `colr` and the extra header boxes.
    fn jp2_wrap(cs: &[u8], nc: u16, bpc: u8, colr: &[u8], extra: &[u8]) -> Vec<u8> {
        let w = u32::from_be_bytes(cs[8..12].try_into().unwrap());
        let h = u32::from_be_bytes(cs[12..16].try_into().unwrap());
        let mut ihdr = h.to_be_bytes().to_vec();
        ihdr.extend(w.to_be_bytes());
        ihdr.extend(nc.to_be_bytes());
        ihdr.extend([bpc - 1, 7, 0, 0]);
        let mut hdr = jp2_box(b"ihdr", &ihdr);
        hdr.extend(jp2_box(b"colr", colr));
        hdr.extend_from_slice(extra);
        let mut f = jp2_box(b"jP  ", b"\r\n\x87\n");
        f.extend(jp2_box(b"ftyp", b"jp2 \0\0\0\0jp2 "));
        f.extend(jp2_box(b"jp2h", &hdr));
        f.extend(jp2_box(b"jp2c", cs));
        f
    }

    fn samples16(r: &Raster) -> Vec<u16> {
        r.planes[0]
            .data
            .chunks_exact(2)
            .map(|b| u16::from_ne_bytes([b[0], b[1]]))
            .collect()
    }

    fn open_bytes(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
        Jp2Decoder
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    /// A palette with 16-bit columns over 8-bit indices decodes at 16 bits: every pixel is its
    /// palette entry exactly.
    #[test]
    fn sixteen_bit_palette_stays_wide() {
        let cs = std::fs::read(format!("{DATA}gray_tiles.j2k")).unwrap();
        let entry = |i: u32| [i * 257, (i * 7919 + 3) & 0xffff, 65535 - i * 251];
        let mut pclr = 256u16.to_be_bytes().to_vec();
        pclr.extend([3, 15, 15, 15]);
        for i in 0..256 {
            pclr.extend(entry(i).iter().flat_map(|&v| (v as u16).to_be_bytes()));
        }
        let mut extra = jp2_box(b"pclr", &pclr);
        extra.extend(jp2_box(b"cmap", &[0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 2]));
        let srgb = [1, 0, 0, 0, 0, 0, 16];
        let file = jp2_wrap(&cs, 1, 8, &srgb, &extra);
        let s = open_bytes("palette16.jp2", file.clone());
        assert_eq!(s.info().bit_depth, 16);
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, PixelLayout::Rgba16(HdrEncoding::SRGB));
        assert_eq!(r.planes[0].stride, r.width as usize * 8);
        // The indices, from the codestream alone.
        let idx = specular_jp2::decode(&cs, &DecodeOptions::default()).unwrap();
        let Pixels::U8(idx) = idx.pixels else {
            panic!()
        };
        let want: Vec<u16> = idx
            .iter()
            .flat_map(|&i| {
                let [r, g, b] = entry(u32::from(i));
                [r as u16, g as u16, b as u16, 65535]
            })
            .collect();
        assert_eq!(samples16(&r), want);
        // Scaled levels are wide too.
        let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!(half.layout, PixelLayout::Rgba16(HdrEncoding::SRGB));
        // An 8-bit palette stays 8-bit.
        assert_eq!(open("palette.jp2").info().bit_depth, 8);
    }

    /// 16-bit samples with an ICC profile (`colr` method 2) keep 16 bits; the profile is
    /// passed on for the viewer's transform and the samples are the file's.
    #[test]
    fn sixteen_bit_with_icc_stays_wide() {
        let cs = std::fs::read(format!("{DATA}gray16_rev.j2k")).unwrap();
        let icc = b"not a real profile: carried, not parsed here".to_vec();
        let mut colr = vec![2, 0, 0];
        colr.extend_from_slice(&icc);
        let s = open_bytes("gray16icc.jp2", jp2_wrap(&cs, 1, 16, &colr, &[]));
        assert_eq!(s.info().icc_profile.as_deref(), Some(&icc));
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, PixelLayout::Gray16);
        let plain = open("gray16_rev.j2k")
            .decode(&DecodeRequest::full(1), &c)
            .unwrap();
        assert_eq!(r.planes[0].data, plain.planes[0].data);
    }

    /// Trimming drops the box-filtered levels (one decomposition level, asked for 1/8); the
    /// next request makes them again.
    #[test]
    fn trims_filtered_levels() {
        crate::trim_check::assert_trims(&*open("rgb_nl1.j2k"), &DecodeRequest::full(8));
    }

    /// A JPX file's codestreams are pages: each with the JP2 header overridden by its own
    /// codestream and compositing layer headers; a fragmented codestream counts but fails.
    #[test]
    fn jpx_codestreams_are_pages() {
        let rgb = std::fs::read(format!("{DATA}rgb_rev.j2k")).unwrap();
        let gray = std::fs::read(format!("{DATA}gray_tiles.j2k")).unwrap();
        let gray16 = std::fs::read(format!("{DATA}gray16_rev.j2k")).unwrap();
        let srgb = [1, 0, 0, 0, 0, 0, 16];
        let icc = b"layer three's profile".to_vec();
        let mut icc_colr = vec![2, 0, 0];
        icc_colr.extend_from_slice(&icc);
        let layer = |colr: &[u8]| jp2_box(b"jplh", &jp2_box(b"cgrp", &jp2_box(b"colr", colr)));
        // The JP2 header describes codestream 0 (sRGB); later codestreams bring their own.
        let jp2 = jp2_wrap(&rgb, 3, 8, &srgb, &[]);
        let mut f = jp2_box(b"jP  ", b"\r\n\x87\n");
        f.extend(jp2_box(b"ftyp", b"jpx \0\0\0\0jpx jp2 "));
        let hdr_end = 12 + 20;
        f.extend_from_slice(&jp2[hdr_end..jp2.len() - rgb.len() - 8]);
        f.extend(jp2_box(b"jpch", &[]));
        f.extend(jp2_box(b"jpch", &[]));
        f.extend(layer(&srgb));
        f.extend(layer(&[1, 0, 0, 0, 0, 0, 17]));
        f.extend(layer(&srgb));
        f.extend(layer(&icc_colr));
        f.extend(jp2_box(b"jp2c", &rgb));
        f.extend(jp2_box(b"jp2c", &gray));
        f.extend(jp2_box(b"ftbl", &[]));
        f.extend(jp2_box(b"jp2c", &gray16));
        assert_eq!(specular_jp2::codestream_count(&f), 4);
        let s = open_bytes("four.jpx", f.clone());
        assert_eq!((s.page_count(), s.page_index()), (4, 0));
        let label = |s: &dyn Session| match s.page_label() {
            Some(PageLabel::Image { width, height, .. }) => (width, height),
            l => panic!("{l:?}"),
        };
        let c = CancelToken::new();
        let full = |s: &dyn Session| s.decode(&DecodeRequest::full(1), &c).unwrap();
        let plain = |name: &str| full(&*open(name));
        let bytes = |r: Raster| {
            r.planes
                .into_iter()
                .map(|p| p.data.to_vec())
                .collect::<Vec<_>>()
        };
        assert_eq!(label(&*s), (s.info().width, s.info().height));
        assert!(
            s.info().coding.ends_with("codestream 1 of 4"),
            "{}",
            s.info().coding
        );
        assert_eq!(bytes(full(&*s)), bytes(plain("rgb_rev.j2k")));
        let p1 = s.open_page(1).unwrap();
        assert_eq!((p1.page_index(), p1.colour_model()), (1, Some("Greyscale")));
        let g = open("gray_tiles.j2k");
        assert_eq!(label(&*p1), (g.info().width, g.info().height));
        assert_eq!(bytes(full(&*p1)), bytes(plain("gray_tiles.j2k")));
        // The fragmented one.
        assert!(matches!(p1.open_page(2), Err(DecodeError::Unsupported(_))));
        let p3 = p1.open_page(3).unwrap();
        assert_eq!(p3.info().icc_profile.as_deref(), Some(&icc));
        assert_eq!(bytes(full(&*p3)), bytes(plain("gray16_rev.j2k")));
        assert!(p3.open_page(4).is_err());
        assert_eq!(p3.open_page(0).unwrap().page_index(), 0);

        // JP2 files and single-codestream JPX files have no pages, even with a stray second
        // codestream box.
        let mut two = jp2.clone();
        two.extend(jp2_box(b"jp2c", &gray));
        let s = open_bytes("two.jp2", two);
        assert_eq!((s.page_count(), s.page_label()), (1, None));
        assert!(s.open_page(1).is_err());

        // Every prefix opens (or fails) and decodes its pages without panicking.
        for n in (0..f.len()).step_by(7) {
            let Ok(s) = Jp2Decoder.open(Arc::new(Source::from_vec("p.jpx", f[..n].to_vec())))
            else {
                continue;
            };
            for page in 0..s.page_count() {
                if let Ok(p) = s.open_page(page) {
                    let _ = p.decode(&DecodeRequest::full(4), &c);
                }
            }
        }
    }

    /// A JPX composition is page 0, its codestreams pages 1‥; one that shows only the first
    /// codestream whole is not a page.
    #[test]
    fn jpx_composition_is_page_zero() {
        let rgb = std::fs::read(format!("{DATA}rgb_rev.j2k")).unwrap();
        let gray = std::fs::read(format!("{DATA}gray_tiles.j2k")).unwrap();
        let (a, b) = (open("rgb_rev.j2k"), open("gray_tiles.j2k"));
        let ((aw, ah), (bw, bh)) = (
            (a.info().width, a.info().height),
            (b.info().width, b.info().height),
        );
        let (w, h) = (aw + bw, ah.max(bh));
        let layer = |colr: &[u8]| jp2_box(b"jplh", &jp2_box(b"cgrp", &jp2_box(b"colr", colr)));
        // Instructions with an offset and LIFE / NEXT-USE (Ityp 0x05): side by side.
        let comp = |ins: &[[u32; 4]]| {
            let mut copt = h.to_be_bytes().to_vec();
            copt.extend(w.to_be_bytes());
            copt.push(0);
            let mut inst = vec![0, 5, 0, 0, 0, 0, 0, 0];
            inst.extend(ins.iter().flatten().flat_map(|v| v.to_be_bytes()));
            let mut c = jp2_box(b"copt", &copt);
            c.extend(jp2_box(b"inst", &inst));
            jp2_box(b"comp", &c)
        };
        let jp2 = jp2_wrap(&rgb, 3, 8, &[1, 0, 0, 0, 0, 0, 16], &[]);
        let file = |comp: &[u8]| {
            let mut f = jp2_box(b"jP  ", b"\r\n\x87\n");
            f.extend(jp2_box(b"ftyp", b"jpx \0\0\0\0jpx jp2 "));
            f.extend_from_slice(&jp2[32..jp2.len() - rgb.len() - 8]);
            f.extend(layer(&[1, 0, 0, 0, 0, 0, 16]));
            f.extend(layer(&[1, 0, 0, 0, 0, 0, 17]));
            f.extend_from_slice(comp);
            f.extend(jp2_box(b"jp2c", &rgb));
            f.extend(jp2_box(b"jp2c", &gray));
            f
        };
        let f = file(&comp(&[[0, 0, 0, 0], [aw, 0, 1, 0]]));
        let s = open_bytes("comp.jpx", f.clone());
        assert_eq!((s.page_count(), s.page_index()), (3, 0));
        assert_eq!(s.page_label(), Some(PageLabel::Merged));
        assert_eq!((s.info().width, s.info().height), (w, h));
        assert!(
            s.info().coding.starts_with("JPX, composition of 2 layers"),
            "{}",
            s.info().coding
        );
        assert_eq!(
            (s.colour_model(), s.alpha()),
            (Some("RGB"), Some(Alpha::Straight))
        );
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height, r.layout), (w, h, PixelLayout::Rgba));
        let ra = a.decode(&DecodeRequest::full(1), &c).unwrap();
        let rb = b.decode(&DecodeRequest::full(1), &c).unwrap();
        let px = |x: u32, y: u32| {
            let o = ((y * w + x) * 4) as usize;
            r.planes[0].data[o..o + 4].to_vec()
        };
        for (x, y) in [(0, 0), (aw / 2, ah / 3), (aw - 1, ah - 1)] {
            let i = (y * aw + x) as usize;
            let want: Vec<u8> = (0..3).map(|p| ra.planes[p].data[i]).chain([255]).collect();
            assert_eq!(px(x, y), want, "({x}, {y})");
        }
        for (x, y) in [(0, 0), (bw / 2, bh / 3), (bw - 1, bh - 1)] {
            let v = rb.planes[0].data[(y * bw + x) as usize];
            assert_eq!(px(aw + x, y), [v, v, v, 255], "({x}, {y})");
        }
        if ah < h {
            assert_eq!(px(0, h - 1), [0, 0, 0, 0]);
        }
        // Scales and regions: a reduced composition, a region of the cached level.
        let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!((half.width, half.height), (w.div_ceil(2), h.div_ceil(2)));
        assert!(s.memory_bytes() > 0);
        assert!(s.decode_memory(&DecodeRequest::full(4)) > 0);
        s.trim_memory();
        crate::trim_check::assert_trims(&*s, &DecodeRequest::full(1));

        // The codestreams follow.
        let p1 = s.open_page(1).unwrap();
        assert!(
            p1.info().coding.ends_with("codestream 1 of 2"),
            "{}",
            p1.info().coding
        );
        assert_eq!(
            p1.page_label(),
            Some(PageLabel::Image {
                width: aw,
                height: ah,
                bits: None
            })
        );
        let p2 = p1.open_page(2).unwrap();
        assert_eq!(
            (p2.info().width, p2.colour_model()),
            (bw, Some("Greyscale"))
        );
        assert!(p2.open_page(3).is_err());
        let p0 = p2.open_page(0).unwrap();
        assert_eq!(p0.page_label(), Some(PageLabel::Merged));

        // The first codestream whole on its own canvas: no composition page.
        let mut copt = ah.to_be_bytes().to_vec();
        copt.extend(aw.to_be_bytes());
        copt.push(0);
        let mut cbox = jp2_box(b"copt", &copt);
        cbox.extend(jp2_box(
            b"inst",
            &[
                0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
            ],
        ));
        let plain = open_bytes("plain.jpx", file(&jp2_box(b"comp", &cbox)));
        assert_eq!(
            (plain.page_count(), plain.page_label().is_some()),
            (2, true)
        );
        assert!(!matches!(plain.page_label(), Some(PageLabel::Merged)));

        // Prefixes never panic.
        for n in (0..f.len()).step_by(11) {
            let Ok(s) = Jp2Decoder.open(Arc::new(Source::from_vec("p.jpx", f[..n].to_vec())))
            else {
                continue;
            };
            for page in 0..s.page_count() {
                if let Ok(p) = s.open_page(page) {
                    let _ = p.decode(&DecodeRequest::full(4), &c);
                }
            }
        }
    }

    /// A JPX animation plays: two frames of 100 ms (LIFE 1 × TICK 100), the first layer kept,
    /// looping for ever; its codestreams split into fragments are joined at open.
    #[test]
    fn jpx_animation_plays() {
        let rgb = std::fs::read(format!("{DATA}rgb_rev.j2k")).unwrap();
        let a = open("rgb_rev.j2k");
        let (w, h) = (a.info().width, a.info().height);
        let layer = |colr: &[u8]| jp2_box(b"jplh", &jp2_box(b"cgrp", &jp2_box(b"colr", colr)));
        let mut copt = h.to_be_bytes().to_vec();
        copt.extend((2 * w).to_be_bytes());
        copt.push(0);
        // Ityp 0x05 (offset, LIFE / NEXT-USE), REPT 65535 (for ever), TICK 100 ms.
        let mut inst = vec![0, 5, 0xff, 0xff, 0, 0, 0, 100];
        for ins in [[0, 0, 0x8000_0001u32, 0], [w, 0, 1, 0]] {
            inst.extend(ins.iter().flat_map(|v| v.to_be_bytes()));
        }
        let mut comp = jp2_box(b"copt", &copt);
        comp.extend(jp2_box(b"inst", &inst));
        let jp2 = jp2_wrap(&rgb, 3, 8, &[1, 0, 0, 0, 0, 0, 16], &[]);
        let mut f = jp2_box(b"jP  ", b"\r\n\x87\n");
        f.extend(jp2_box(b"ftyp", b"jpx \0\0\0\0jpx jp2 "));
        f.extend_from_slice(&jp2[32..jp2.len() - rgb.len() - 8]);
        f.extend(layer(&[1, 0, 0, 0, 0, 0, 16]));
        f.extend(layer(&[1, 0, 0, 0, 0, 0, 16]));
        f.extend(jp2_box(b"comp", &comp));
        f.extend(jp2_box(b"jp2c", &rgb));
        // Codestream 1: two fragments, kept in a free box after the table.
        let half = rgb.len() / 2;
        let at = f.len() + 8 + 8 + 2 + 2 * 14 + 8;
        let mut flst = 2u16.to_be_bytes().to_vec();
        for (off, len) in [(at, half), (at + half, rgb.len() - half)] {
            flst.extend((off as u64).to_be_bytes());
            flst.extend((len as u32).to_be_bytes());
            flst.extend(0u16.to_be_bytes());
        }
        f.extend(jp2_box(b"ftbl", &jp2_box(b"flst", &flst)));
        f.extend(jp2_box(b"free", &rgb));
        let s = open_bytes("anim.jpx", f);
        assert_eq!(s.page_count(), 3);
        assert_eq!(
            s.animation(),
            Some(&AnimationInfo {
                frame_count: Some(2),
                loop_count: 0
            })
        );
        assert!(
            s.info().coding.contains("animation of 2 frames"),
            "{}",
            s.info().coding
        );
        let c = CancelToken::new();
        let mut fr = s.frames(1).unwrap();
        assert_eq!(
            fr.delays(),
            Some(vec![Duration::from_millis(100), Duration::from_millis(100)])
        );
        let f0 = fr.next_frame(&c).unwrap().unwrap();
        let f1 = fr.next_frame(&c).unwrap().unwrap();
        assert!(fr.next_frame(&c).is_none());
        assert_eq!((f0.raster.width, f0.raster.height), (2 * w, h));
        let alpha = |r: &Raster, x: u32| r.planes[0].data[(x * 4 + 3) as usize];
        // Frame 0: the left half; frame 1: the kept left half and the right.
        assert_eq!((alpha(&f0.raster, 0), alpha(&f0.raster, w)), (255, 0));
        assert_eq!((alpha(&f1.raster, 0), alpha(&f1.raster, w)), (255, 255));
        let row = (2 * w * 4) as usize;
        let d1 = &f1.raster.planes[0].data[..row];
        assert_eq!(&d1[..row / 2], &d1[row / 2..]);
        fr.rewind().unwrap();
        assert_eq!(fr.next_frame(&c).unwrap().unwrap().index, 0);
        // The joined codestream is a page of its own.
        let p2 = s.open_page(2).unwrap();
        let plain = a.decode(&DecodeRequest::full(1), &c).unwrap();
        let got = p2.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(got.planes[0].data, plain.planes[0].data);
    }

    /// GeoJP2 and GMLJP2 (v1 and v2) written by GDAL's JP2OpenJPEG driver, against what GDAL
    /// reads back (`tools/corpus/geojp2-fixtures.py`, GDAL 3.12): the CRS's EPSG code and the
    /// geotransform. Covers latitude-first and northing-first URNs and rotated grids.
    #[test]
    fn geojp2_and_gmljp2_match_gdal() {
        let oracle = std::fs::read_to_string(format!("{DATA}geo/oracle.txt")).unwrap();
        let mut n = 0;
        for line in oracle.lines().filter(|l| !l.trim().is_empty()) {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (name, epsg) = (f[0], f[1].parse::<u32>().unwrap());
            let gt: Vec<f64> = f[2..8].iter().map(|v| v.parse().unwrap()).collect();
            let s = open(&format!("geo/{name}.jp2"));
            let g = s.geo().unwrap_or_else(|| panic!("{name}: no georeference"));
            let t = g.to_map;
            let got = [t.c, t.a, t.b, t.f, t.d, t.e];
            for (a, b) in got.iter().zip(&gt) {
                assert!(
                    (a - b).abs() <= 1e-9 * b.abs().max(1.0),
                    "{name}: {got:?} vs {gt:?}"
                );
            }
            assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(epsg), "{name}");
            let note = g.crs_note.as_deref().unwrap_or_default();
            let want = if name.contains("geojp2") || name.contains("both") {
                "GeoJP2"
            } else {
                "GMLJP2"
            };
            assert!(note.starts_with(want), "{name}: {note}");
            // The image itself still decodes.
            let r = s
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            assert_eq!((r.width, r.height), (24, 16));
            n += 1;
        }
        assert_eq!(n, 10);
    }

    #[test]
    fn plain_jp2_has_no_georeference_and_unknown_crs_says_so() {
        assert!(open("gray_rev.jp2").geo().is_none());
        // A GMLJP2 grid whose srsName is not known still places the image, and says why
        // there is no CRS.
        let mut b = std::fs::read(format!("{DATA}geo/utm_gml.jp2")).unwrap();
        while let Some(at) = b.windows(6).position(|w| w == b"EPSG::") {
            b[at..at + 6].copy_from_slice(b"XXXX::");
        }
        let s = Jp2Decoder
            .open(Arc::new(Source::from_vec("x.jp2", b)))
            .unwrap();
        let g = s.geo().unwrap();
        assert!(g.crs.is_none());
        assert!(
            g.crs_note.as_deref().unwrap().contains("not known"),
            "{:?}",
            g.crs_note
        );
        assert_eq!(g.to_map.c, 500000.0);
    }
}

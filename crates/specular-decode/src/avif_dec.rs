//! AVIF (stills, grids, alpha, AVIS sequences) through `specular-avif`: own HEIF/ISOBMFF parser
//! over the in-tree safe rav1d fork. Stills come as their YCbCr planes
//! ([`PixelLayout::Yuv`]: the `nclx` or bitstream matrix and range, the samples' bits, the AV1
//! chroma siting, an alpha plane), converted where they are shown with chroma interpolated at
//! its siting; `specular_core::yuv::to_rgb` is the CPU path. PQ/HLG stills in BT.709, Display
//! P3 or BT.2020 primaries are labelled with their transfer and primaries, and the renderer
//! linearises and tone maps them for the display. Monochrome, pre-multiplied alpha, sequence
//! frames and the few layouts the planes cannot carry (`specular_avif::YuvImage`) come as
//! 8-bit RGB(A), or 16-bit RGBA holding the HDR signal ([`PixelLayout::Rgba16`]); other HDR
//! (animations, unknown primaries) is tone mapped to sRGB by the crate. Scales 1/2/4/8 are
//! box-filtered (4:4:4 planes); regions crop a cached whole-image level, planes with an apron
//! of one chroma sample each side. An ISO 21496-1 gain map (`tmap`) is handed to the renderer like a JPEG's: on an SDR
//! base it lifts toward the HDR alternate, on a (16-bit) HDR base it lowers toward the SDR
//! alternate with a negative weight. The top-level images of a collection are pages, the
//! primary first; thumbnails, alpha and depth images, grid tiles and gain maps are not.

use crate::gainmap;
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    PageLabel, Result, Session,
};
use parking_lot::Mutex;
use specular_avif::{AvifFile, Decoded, Error, GainMapInfo, HdrTransfer, Layout, YuvImage};
use specular_core::{
    CancelToken, GainMap, GainMapParams, HdrEncoding, Orientation, PixelLayout, Primaries, Raster,
    RasterPlane, Source, Transfer, YuvEncoding, YuvMatrix,
};
use std::sync::Arc;
use std::time::Duration;

pub struct AvifDecoder;

fn map_err(e: Error) -> DecodeError {
    match e {
        Error::NotAvif => DecodeError::UnsupportedFormat,
        Error::Truncated => DecodeError::Truncated("AVIF".into()),
        Error::Malformed(s) | Error::Decoder(s) => DecodeError::Malformed(s),
        Error::Unsupported(s) => DecodeError::Unsupported(s),
        e @ Error::OtherCodec(_) => DecodeError::Unsupported(e.to_string()),
        Error::TooLarge => DecodeError::TooLarge,
        Error::Cancelled => DecodeError::Cancelled,
    }
}

fn plane(width: u32, height: u32, stride: usize, data: Vec<u8>) -> RasterPlane {
    RasterPlane {
        width,
        height,
        stride,
        data,
        h_div: 1,
        v_div: 1,
    }
}

/// `hdr`: the encoding of a [`Layout::Rgba16`] picture.
fn to_raster(d: Decoded, hdr: Option<HdrEncoding>) -> Raster {
    let (w, h) = (d.width, d.height);
    let n = w as usize * h as usize;
    let (layout, planes) = match d.layout {
        Layout::Gray => (PixelLayout::Gray, vec![plane(w, h, w as usize, d.data)]),
        Layout::Rgba => (PixelLayout::Rgba, vec![plane(w, h, w as usize * 4, d.data)]),
        Layout::Rgba16 => (
            PixelLayout::Rgba16(hdr.unwrap_or(HdrEncoding::SRGB)),
            vec![plane(w, h, w as usize * 8, d.data)],
        ),
        Layout::Rgb => {
            let mut p = [vec![0u8; n], vec![0u8; n], vec![0u8; n]];
            for (i, px) in d.data.chunks_exact(3).enumerate().take(n) {
                p[0][i] = px[0];
                p[1][i] = px[1];
                p[2][i] = px[2];
            }
            let [r, g, b] = p;
            (
                PixelLayout::Rgb,
                vec![
                    plane(w, h, w as usize, r),
                    plane(w, h, w as usize, g),
                    plane(w, h, w as usize, b),
                ],
            )
        }
    };
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: d.scale,
        layout,
        planes,
        truncated: false,
    }
}

/// The YCbCr matrix of a CICP `matrix_coefficients` code as `specular-avif` converts it:
/// unspecified and other codes are BT.601, as libavif reads them.
fn yuv_matrix(code: u16) -> YuvMatrix {
    match code {
        0 => YuvMatrix::Identity,
        1 => YuvMatrix::Bt709,
        4 => YuvMatrix::Fcc,
        7 => YuvMatrix::Smpte240,
        8 => YuvMatrix::YCgCo,
        9 | 10 => YuvMatrix::Bt2020,
        _ => YuvMatrix::Bt601,
    }
}

/// The planes of a still as a [`PixelLayout::Yuv`] raster whose R'G'B' is `colour`.
fn planes_raster(y: YuvImage, colour: HdrEncoding) -> Raster {
    let e = YuvEncoding {
        matrix: yuv_matrix(y.matrix),
        full_range: y.full_range,
        bits: y.bits,
        cosited_x: y.cosited_x,
        cosited_y: y.cosited_y,
        bottom_y: false,
        right_x: false,
        alpha: y.planes.len() > 3,
        colour,
    };
    Raster {
        width: y.width,
        height: y.height,
        origin: (0, 0),
        scale: y.scale,
        layout: PixelLayout::Yuv(e),
        planes: y
            .planes
            .into_iter()
            .map(|p| RasterPlane {
                width: p.width,
                height: p.height,
                stride: p.stride,
                data: p.data,
                h_div: p.h_div,
                v_div: p.v_div,
            })
            .collect(),
        truncated: false,
    }
}

/// `r` of a whole level: [`crate::jpeg::crop`], YCbCr planes widened by one chroma sample
/// each side so the pixels asked for interpolate chroma across the window's edges like the
/// whole level's.
fn crop_level(level: &Raster, r: crate::Rect) -> Raster {
    if let PixelLayout::Yuv(_) = level.layout {
        let div = level
            .planes
            .iter()
            .map(|p| u32::from(p.h_div.max(p.v_div)))
            .max()
            .unwrap_or(1);
        let a = if div > 1 { 2 * div } else { 0 };
        let (x, y) = (r.x.saturating_sub(a), r.y.saturating_sub(a));
        return crate::jpeg::crop(
            level,
            crate::Rect {
                x,
                y,
                w: (r.x + r.w).saturating_add(a) - x,
                h: (r.y + r.h).saturating_add(a) - y,
            },
        );
    }
    crate::jpeg::crop(level, r)
}

impl Decoder for AvifDecoder {
    fn format(&self) -> Format {
        Format::Avif
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_avif::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["avif", "avifs"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let file = match AvifFile::open(source.bytes()) {
            Ok(f) => f,
            // A grid of HEVC, AVC, JPEG or uncompressed tiles: the HEIF decoder reads those;
            // when it cannot, the AVIF refusal names the codec.
            Err(e @ Error::OtherCodec(_)) => {
                return match crate::Decoder::open(&crate::heic::HeicDecoder, source) {
                    Ok(s) => Ok(s),
                    Err(DecodeError::Cancelled) => Err(DecodeError::Cancelled),
                    Err(_) => Err(map_err(e)),
                };
            }
            Err(e) => return Err(map_err(e)),
        };
        open_file(source, file)
    }
}

/// A session over an opened file (any page of it).
fn open_file(source: Arc<Source>, file: AvifFile) -> Result<Arc<dyn Session>> {
    let i = file.info();
    let animation = (i.animated && i.frame_count > 1).then_some(AnimationInfo {
        frame_count: Some(i.frame_count),
        loop_count: i.loop_count,
    });
    let mut coding = format!("AV1 {}-bit {}", i.bit_depth, i.chroma);
    if let Some((c, r)) = i.grid {
        coding += &format!(", grid {c}×{r}");
    }
    // Stills keep their HDR signal (16-bit) when the renderer knows the primaries.
    let hdr = if i.animated {
        None
    } else {
        i.hdr.and_then(hdr_encoding)
    };
    if let Some(h) = i.hdr {
        coding += match (h.transfer, hdr.is_some()) {
            (HdrTransfer::Pq, true) => ", PQ HDR",
            (HdrTransfer::Hlg, true) => ", HLG HDR",
            (HdrTransfer::Pq, false) => ", PQ HDR (tone mapped)",
            (HdrTransfer::Hlg, false) => ", HLG HDR (tone mapped)",
        };
        if h.max_cll > 0 {
            coding += &format!(", MaxCLL {} nits", h.max_cll);
        }
    }
    // A map needs its base as the renderer sees it: an SDR base, or an HDR base kept
    // wide (a base tone mapped to 8-bit here has lost what the map was made against).
    // Unsupported metadata versions are left out too (cheap: the `tmap` body only).
    let gain_map = file.gain_map().filter(|g| {
        gain_map_params(g, i.icc.as_deref()).is_some_and(|p| {
            if i.hdr.is_none() {
                !p.base_is_hdr
            } else {
                hdr.is_some()
            }
        })
    });
    if let Some(p) = gain_map
        .as_ref()
        .and_then(|g| gain_map_params(g, i.icc.as_deref()))
    {
        coding += if p.base_is_hdr {
            ", gain map (ISO 21496-1, HDR base)"
        } else {
            ", gain map (ISO 21496-1)"
        };
    } else if i.gain_map {
        coding += ", gain map (not applied)";
    }
    if i.animated {
        coding += &format!(", AVIS {} frames", i.frame_count);
    }
    if file.page_count() > 1 {
        coding += &format!(", collection of {}", file.page_count());
    }
    let info = ImageInfo {
        format: Format::Avif,
        width: i.width,
        height: i.height,
        orientation: Orientation::from_exif(i.orientation),
        icc_profile: i.icc.clone().map(Arc::new),
        has_alpha: i.has_alpha,
        bit_depth: i.bit_depth,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(AvifSession {
        source,
        info,
        file,
        animation,
        gain_map,
        hdr,
        levels: Mutex::new(Vec::new()),
    }))
}

/// The 16-bit output encoding of an HDR still: PQ or HLG in primaries the renderer converts.
fn hdr_encoding(h: specular_avif::Hdr) -> Option<HdrEncoding> {
    let primaries = match h.primaries {
        1 => Primaries::Bt709,
        12 => Primaries::DisplayP3,
        9 => Primaries::Bt2020,
        _ => return None,
    };
    let transfer = match h.transfer {
        HdrTransfer::Pq => Transfer::Pq,
        HdrTransfer::Hlg => Transfer::Hlg,
    };
    Some(HdrEncoding {
        transfer,
        primaries,
    })
}

/// The metadata of a gain map of a sane size, in either direction (the caller checks the
/// base matches). The base's primaries come from its ICC profile when it has one (the
/// profile describes its colours), else from its `nclx`.
fn gain_map_params(g: &GainMapInfo, base_icc: Option<&[u8]>) -> Option<GainMapParams> {
    let prim = |p: Option<u16>| p.and_then(gainmap::cicp_primaries);
    let base = base_icc
        .and_then(gainmap::icc_primaries)
        .or_else(|| prim(g.base_primaries));
    let p = gainmap::tmap_params(&g.metadata, base, prim(g.alternate_primaries))?;
    let sane = g.width <= gainmap::MAX_MAP_SIDE && g.height <= gainmap::MAX_MAP_SIDE;
    sane.then_some(p)
}

struct AvifSession {
    source: Arc<Source>,
    info: ImageInfo,
    file: AvifFile,
    animation: Option<AnimationInfo>,
    /// The ISO 21496-1 gain map (of an SDR base, or of a wide HDR base).
    gain_map: Option<GainMapInfo>,
    /// The encoding of a still decoded wide ([`PixelLayout::Rgba16`]); `None` for 8-bit.
    hdr: Option<HdrEncoding>,
    /// Decoded whole-image levels (for region requests and repeated decodes).
    levels: Mutex<Vec<Arc<Raster>>>,
}

impl AvifSession {
    fn level(&self, scale: u32, parallel: bool, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(r) = self.levels.lock().iter().find(|r| r.scale == scale) {
            return Ok(r.clone());
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let cancelled = || cancel.is_cancelled();
        if let Some(r) = self.planes(scale, parallel, &cancelled)? {
            let r = Arc::new(r);
            let mut g = self.levels.lock();
            if !g.iter().any(|l| l.scale == scale) {
                g.push(r.clone());
            }
            return Ok(r);
        }
        let d = match self.hdr {
            Some(_) => self.file.decode_hdr_with(scale, parallel, &cancelled),
            None => self.file.decode_with(scale, parallel, &cancelled),
        }
        .map_err(map_err)?;
        let r = Arc::new(to_raster(d, self.hdr));
        let mut g = self.levels.lock();
        if !g.iter().any(|l| l.scale == scale) {
            g.push(r.clone());
        }
        Ok(r)
    }

    /// The still's planes at 1/`scale` when they show it as the RGB path would: SDR (its
    /// R'G'B' labelled sRGB-like, as for other YCbCr stills), or HDR kept wide. `None` for
    /// HDR the crate tone maps, and what [`AvifFile::decode_planes`] does not carry.
    fn planes(
        &self,
        scale: u32,
        parallel: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Raster>> {
        if self.file.info().hdr.is_some() && self.hdr.is_none() {
            return Ok(None);
        }
        let Some(y) = self
            .file
            .decode_planes(scale, parallel, cancelled)
            .map_err(map_err)?
        else {
            return Ok(None);
        };
        // A PQ / HLG bitstream without an `nclx` saying so: the crate tone maps it.
        if self.hdr.is_none() && matches!(y.transfer, 16 | 18) {
            return Ok(None);
        }
        Ok(Some(planes_raster(
            y,
            self.hdr.unwrap_or(HdrEncoding::SRGB),
        )))
    }

    fn decode_gain_map(&self, g: &GainMapInfo, cancel: &CancelToken) -> Option<GainMap> {
        let params = gain_map_params(g, self.info.icc_profile.as_deref().map(|v| &v[..]))?;
        let t0 = std::time::Instant::now();
        let d = self
            .file
            .decode_gain_map(true, &|| cancel.is_cancelled())
            .ok()?;
        let map = gainmap::to_rgba8(&to_raster(d, None));
        specular_log::debug!(
            target: specular_log::DECODE,
            "avif: gain map {}×{} decoded in {:.1} ms",
            map.width,
            map.height,
            t0.elapsed().as_secs_f64() * 1e3
        );
        Some(GainMap {
            map,
            params,
            source: gainmap::ISO_SOURCE,
        })
    }

    fn out_bytes(&self, scale: u32) -> u64 {
        let s = scale.max(1);
        let px = u64::from(self.info.width.div_ceil(s)) * u64::from(self.info.height.div_ceil(s));
        // RGB(A), or at most four planes of the samples (4:4:4 with alpha).
        let samples =
            (3 + u64::from(self.info.has_alpha)) * if self.info.bit_depth > 8 { 2 } else { 1 };
        px * samples.max(if self.hdr.is_some() {
            8
        } else if self.info.has_alpha {
            4
        } else {
            3
        })
    }
}

impl Session for AvifSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// The `nclx`: PQ / HLG over a profile, its primaries without one.
    fn colour_space(&self) -> Option<&'static str> {
        let i = self.file.info();
        let (p, t) = i.cicp?;
        if i.icc.is_some() && !matches!(t, 16 | 18) {
            return None;
        }
        crate::colour_space::cicp_space(p, t)
    }
    fn colour_model(&self) -> Option<&'static str> {
        let i = self.file.info();
        Some(crate::cicp_colour_model(i.chroma == "4:0:0", i.matrix))
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        matches!(self.colour_model(), Some("YCbCr" | "YCgCo")).then_some(self.file.info().chroma)
    }
    /// The alpha auxiliary image or track; premultiplied with a `prem` reference.
    fn alpha(&self) -> Option<Alpha> {
        let i = self.file.info();
        i.has_alpha.then_some(if i.premultiplied {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        match req.region {
            None => {
                specular_log::debug!(target: specular_log::DECODE, "avif: decode 1/{scale} whole image")
            }
            Some(r) => {
                specular_log::trace!(target: specular_log::DECODE, "avif: 1/{scale} region {}×{}+{}+{}", r.w, r.h, r.x, r.y)
            }
        }
        let level = self.level(scale, req.parallel, cancel)?;
        match req.region {
            Some(r) => Ok(crop_level(&level, r)),
            None => Ok((*level).clone()),
        }
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        // Codec working set plus the cached level and the returned copy.
        let codec = match self.hdr {
            Some(_) => self.file.decode_hdr_memory(scale),
            None => self.file.decode_memory(scale),
        }
        .max(self.file.decode_planes_memory(scale));
        codec + 2 * self.out_bytes(scale)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.lock().iter().map(|r| r.memory_bytes()).sum()
    }
    /// Drops the cached levels (regions are cropped from them).
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn has_gain_map(&self) -> bool {
        self.gain_map.is_some()
    }
    fn page_count(&self) -> u32 {
        self.file.page_count()
    }
    fn page_index(&self) -> u32 {
        self.file.page()
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.file.page_count() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.file.page_count() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        let file = self.file.reopen_page(page).map_err(map_err)?;
        open_file(self.source.clone(), file)
    }
    fn gain_map(&self, cancel: &CancelToken) -> Option<GainMap> {
        self.decode_gain_map(self.gain_map.as_ref()?, cancel)
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.animation.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        let f = self.file.frames(scale.max(1), true).map_err(map_err)?;
        Ok(Box::new(AvifFrames { f, index: 0 }))
    }
}

/// Sequential AVIS frames (each a full canvas).
struct AvifFrames {
    f: specular_avif::Frames,
    index: u32,
}

impl Frames for AvifFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let r = self.f.next_with(&|| cancel.is_cancelled())?;
        let index = self.index;
        self.index += 1;
        Some(r.map_err(map_err).map(|d| Frame {
            index,
            delay: Duration::from_millis(u64::from(d.duration_ms)),
            raster: to_raster(d, None),
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.f.rewind();
        self.index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-avif/tests/data/"
    );

    fn open(name: &str) -> Arc<dyn Session> {
        let bytes = std::fs::read(format!("{DATA}{name}")).unwrap();
        assert!(AvifDecoder.probe(&bytes));
        AvifDecoder
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    /// RGBA8 (or native-endian RGBA16) bytes of `r`: planes through the CPU conversion, RGB
    /// and grey widened.
    fn rgba(r: &Raster) -> Vec<u8> {
        match r.layout {
            PixelLayout::Yuv(_) => specular_core::yuv::to_rgb(r).unwrap().planes[0]
                .data
                .clone(),
            PixelLayout::Rgb => {
                let n = (r.width * r.height) as usize;
                (0..n)
                    .flat_map(|i| {
                        [
                            r.planes[0].data[i],
                            r.planes[1].data[i],
                            r.planes[2].data[i],
                            255,
                        ]
                    })
                    .collect()
            }
            _ => r.planes[0].data.clone(),
        }
    }

    /// The largest difference between two byte rasters of the same size.
    fn worst(a: &[u8], b: &[u8]) -> u8 {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0)
    }

    /// Reads a binary PPM (P6, 8-bit) as RGBA8.
    fn ppm(name: &str) -> (u32, u32, Vec<u8>) {
        let b = std::fs::read(format!("{DATA}{name}")).unwrap();
        let mut fields = Vec::new();
        let mut i = 0;
        while fields.len() < 4 {
            while b[i].is_ascii_whitespace() {
                i += 1;
            }
            let s = i;
            while !b[i].is_ascii_whitespace() {
                i += 1;
            }
            fields.push(String::from_utf8_lossy(&b[s..i]).into_owned());
        }
        assert_eq!((fields[0].as_str(), fields[3].as_str()), ("P6", "255"));
        let (w, h): (u32, u32) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
        let px = &b[i + 1..];
        let data = px
            .chunks_exact(3)
            .take((w * h) as usize)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        (w, h, data)
    }

    /// Stills come as their planes, converted on the CPU like the crate's own RGB within
    /// rounding (both interpolate chroma bilinearly at the AV1 siting), and like libavif's
    /// reference decodes; their windows convert like the whole image.
    #[test]
    fn planes_convert_like_the_rgb_path() {
        let c = CancelToken::new();
        for (name, div) in [
            ("rgb8_420.avif", (2, 2)),
            ("rgb10_420.avif", (2, 2)),
            ("rgb8_444.avif", (1, 1)),
            ("alpha.avif", (2, 2)),
            ("grid.avif", (2, 2)),
            ("grid_odd.avif", (2, 2)),
            ("grid420-colocated.avif", (2, 2)),
            ("grid422.avif", (2, 1)),
            ("siting420-colocated.avif", (2, 2)),
            ("siting420-vertical.avif", (2, 2)),
            ("siting420-odd-unknown.avif", (2, 2)),
            ("siting422-odd.avif", (2, 1)),
            ("siting420-10bit-colocated.avif", (2, 2)),
        ] {
            let s = open(name);
            let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
            let PixelLayout::Yuv(e) = full.layout else {
                panic!("{name}: {:?}", full.layout)
            };
            assert_eq!(
                (e.bits, e.alpha),
                (s.info().bit_depth, s.info().has_alpha),
                "{name}"
            );
            let bits = e.bits;
            assert_eq!((full.planes[1].h_div, full.planes[1].v_div), div, "{name}");
            let file = AvifFile::open(std::fs::read(format!("{DATA}{name}")).unwrap()).unwrap();
            let old = to_raster(file.decode(1, true).unwrap(), None);
            let mut got = rgba(&full);
            if bits > 8 {
                // 16-bit codes to 8 bits, as the crate's own output rounds them.
                got = got
                    .chunks_exact(2)
                    .map(|b| ((u32::from(u16::from_ne_bytes([b[0], b[1]])) + 128) / 257) as u8)
                    .collect();
            }
            let w = worst(&got, &rgba(&old));
            eprintln!("{name}: planes vs RGB path worst {w}");
            assert!(w <= 1, "{name}: off by {w}");
            // A window: the same pixels as the whole image's conversion.
            let (x, y, ww, wh) = (5, 3, full.width.min(19) - 5, full.height.min(13) - 3);
            let req = DecodeRequest {
                region: Some(crate::Rect { x, y, w: ww, h: wh }),
                ..DecodeRequest::full(1)
            };
            let r = s.decode(&req, &c).unwrap();
            let (ox, oy) = r.origin;
            assert!(ox <= x && oy <= y && ox + r.width >= x + ww && oy + r.height >= y + wh);
            let (part, whole) = (rgba(&r), rgba(&full));
            let bpp = if bits > 8 { 8 } else { 4 };
            for row in 0..wh {
                let a = (((y + row - oy) * r.width + x - ox) * bpp) as usize;
                let b = (((y + row) * full.width + x) * bpp) as usize;
                let n = (ww * bpp) as usize;
                assert_eq!(&part[a..a + n], &whole[b..b + n], "{name} row {row}");
            }
            // Reduced levels: 4:4:4 planes at the output size.
            let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
            assert_eq!(
                (half.width, half.height),
                (full.width.div_ceil(2), full.height.div_ceil(2))
            );
            assert!(half.planes.iter().all(|p| p.h_div == 1 && p.v_div == 1));
            let old = to_raster(file.decode(2, true).unwrap(), None);
            let mut got = rgba(&half);
            if bits > 8 {
                got = got
                    .chunks_exact(2)
                    .map(|b| ((u32::from(u16::from_ne_bytes([b[0], b[1]])) + 128) / 257) as u8)
                    .collect();
            }
            let w = worst(&got, &rgba(&old));
            eprintln!("{name}: 1/2 planes vs RGB path worst {w}");
            // Both average the interpolated chroma; the RGB path converts in integers.
            assert!(w <= 3, "{name} at 1/2: off by {w}");
        }
        // libavif's reference decodes (its built-in bilinear chroma upsampling).
        for name in [
            "siting420-unknown",
            "siting420-odd-unknown",
            "siting422",
            "siting422-odd",
        ] {
            let r = open(&format!("{name}.avif"))
                .decode(&DecodeRequest::full(1), &c)
                .unwrap();
            assert!(matches!(r.layout, PixelLayout::Yuv(_)));
            let (w, h, want) = ppm(&format!("{name}.libavif.ppm"));
            assert_eq!((w, h), (r.width, r.height));
            let d = worst(&rgba(&r), &want);
            eprintln!("{name}: planes vs libavif worst {d}");
            assert!(d <= 1, "{name}: off by {d}");
        }
        // Monochrome and pre-multiplied alpha stay on the RGB path; sequence frames too.
        let g = open("gray.avif")
            .decode(&DecodeRequest::full(1), &c)
            .unwrap();
        assert_eq!(g.layout, PixelLayout::Gray);
        let mut fr = open("anim3.avif").frames(1).unwrap();
        let f = fr.next_frame(&c).unwrap().unwrap();
        assert_eq!(f.raster.layout, PixelLayout::Rgb);
    }

    /// A 10-bit PQ still comes as its planes labelled PQ, converting to the 16-bit signal the
    /// crate's wide path gives.
    #[test]
    fn hdr_planes_keep_the_signal() {
        let s = open("pq10.avif");
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        let PixelLayout::Yuv(e) = r.layout else {
            panic!("{:?}", r.layout)
        };
        assert_eq!(e.colour.transfer, Transfer::Pq);
        let file = AvifFile::open(std::fs::read(format!("{DATA}pq10.avif")).unwrap()).unwrap();
        let old = to_raster(
            file.decode_hdr_with(1, true, &|| false).unwrap(),
            Some(e.colour),
        );
        let rgb = specular_core::yuv::to_rgb(&r).unwrap();
        assert_eq!(rgb.layout, old.layout);
        let u16s = |r: &Raster| -> Vec<u16> {
            r.planes[0]
                .data
                .chunks_exact(2)
                .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                .collect()
        };
        let worst = u16s(&rgb)
            .iter()
            .zip(u16s(&old))
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap();
        eprintln!("pq10: planes vs wide RGB path worst {worst} of 65535");
        // A tenth of a 10-bit step (chroma interpolated in float against in integers).
        assert!(worst <= 64 * 2, "off by {worst}");
    }

    #[test]
    fn still_scales_and_regions() {
        let s = open("rgb8_420.avif");
        assert!(s.animation().is_none());
        let c = CancelToken::new();
        let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert!(matches!(full.layout, PixelLayout::Yuv(e) if !e.alpha && e.bits == 8));
        assert_eq!((full.width, full.height), (s.info().width, s.info().height));
        let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!(half.width, full.width.div_ceil(2));
        assert!(s.memory_bytes() > 0);
        let a = open("alpha.avif")
            .decode(&DecodeRequest::full(1), &c)
            .unwrap();
        assert!(matches!(a.layout, PixelLayout::Yuv(e) if e.alpha));
        assert_ne!(
            open("rot90_flip.avif").info().orientation,
            Orientation::NORMAL
        );
    }

    #[test]
    fn collection_pages() {
        let s = open("collection.avif");
        assert_eq!((s.page_count(), s.page_index()), (3, 0));
        let size = |w, h| {
            Some(PageLabel::Image {
                width: w,
                height: h,
                bits: None,
            })
        };
        assert_eq!(s.page_label(), size(32, 24));
        assert!(s.info().coding.ends_with(", collection of 3"));
        let p = s.open_page(2).unwrap();
        assert_eq!((p.page_index(), p.page_count()), (2, 3));
        assert_eq!(p.page_label(), size(40, 20));
        assert!(p.info().has_alpha);
        let r = p
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!((r.width, r.height), (40, 20));
        assert!(
            matches!(r.layout, PixelLayout::Yuv(e) if e.alpha),
            "{:?}",
            r.layout
        );
        assert_eq!(p.open_page(1).unwrap().info().width, 16);
        assert!(s.open_page(3).is_err());
        // One image, or one sequence: one page and no label.
        for one in ["alpha.avif", "anim3.avif", "grid.avif"] {
            let s = open(one);
            assert_eq!((s.page_count(), s.page_label()), (1, None), "{one}");
            assert!(s.open_page(1).is_err());
        }
        // Thumbnails are not pages (conformance corpus, when fetched).
        let tomsk = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tools/corpus/conformance"
                )
                .into()
            })
            .join("av1-avif/testFiles/Microsoft/Tomsk_with_thumbnails.avif");
        if let Ok(bytes) = std::fs::read(&tomsk) {
            let s = AvifDecoder
                .open(Arc::new(Source::from_vec(&tomsk, bytes)))
                .unwrap();
            assert_eq!(s.page_count(), 1);
        }
    }

    #[test]
    fn avis_frames() {
        let s = open("anim3.avif");
        let a = s.animation().unwrap().clone();
        assert_eq!(a.frame_count, Some(3));
        let mut fr = s.frames(1).unwrap();
        let c = CancelToken::new();
        for _ in 0..2 {
            let mut n = 0;
            while let Some(f) = fr.next_frame(&c) {
                let f = f.unwrap();
                assert_eq!(f.index, n);
                assert_eq!(f.raster.width, s.info().width);
                n += 1;
            }
            assert_eq!(n, 3);
            fr.rewind().unwrap();
        }
    }

    #[test]
    fn truncated_never_panics() {
        let bytes = std::fs::read(format!("{DATA}anim_alpha.avif")).unwrap();
        let c = CancelToken::new();
        for n in (0..bytes.len()).step_by(97) {
            let src = Arc::new(Source::from_vec("t.avif", bytes[..n].to_vec()));
            if let Ok(s) = AvifDecoder.open(src) {
                let _ = s.decode(&DecodeRequest::full(2), &c);
            }
        }
    }

    /// Trimming drops the cached levels; the next request decodes them again.
    #[test]
    fn trims_levels() {
        let req = DecodeRequest {
            region: Some(crate::Rect {
                x: 4,
                y: 4,
                w: 16,
                h: 16,
            }),
            ..DecodeRequest::full(1)
        };
        crate::trim_check::assert_trims(&*open("rgb8_420.avif"), &req);
    }
}

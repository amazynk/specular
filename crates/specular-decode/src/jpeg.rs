//! JPEG through the in-tree decoder (or the hardware path when it applies).
//!
//! Pages (Multi-Picture Format, CIPA DC-007, `.mpo` and MPF JPEGs): page 0 is the primary
//! image (the whole file); pages 1‥ the further images of the primary's MPF index
//! in index order (the second view of a stereo pair, multi-angle and panorama frames),
//! without large-thumbnail previews, gain maps and auxiliary images (see
//! `gainmap::mpo_pages`). The index is read once at open and shared by every page; a page is
//! a zero-copy view of its own bytes, opened like any JPEG.

use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_jpeg as sj;
use std::sync::{Arc, OnceLock};

pub struct JpegDecoder;

impl Decoder for JpegDecoder {
    fn format(&self) -> Format {
        Format::Jpeg
    }
    fn probe(&self, head: &[u8]) -> bool {
        head.len() >= 3 && head[0] == 0xFF && head[1] == 0xD8 && head[2] == 0xFF
    }
    fn extensions(&self) -> &'static [&'static str] {
        // GoPro and Canon thumbnails (`.thm`) and Insta360 photos (`.insp`) are JPEG.
        &["jpg", "jpeg", "jpe", "jfif", "jif", "mpo", "thm", "insp"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let ranges = crate::gainmap::mpo_pages(source.bytes());
        let mpo = (!ranges.is_empty()).then(|| {
            Arc::new(Mpo {
                stereo: ranges.len() == 1 && crate::gainmap::mpo_is_stereo(source.bytes()),
                whole: source.clone(),
                ranges,
            })
        });
        Ok(Arc::new(open_session(source, mpo, 0)?))
    }
}

/// The extra images of a Multi-Picture file, shared by its pages.
struct Mpo {
    /// The whole file (page 0).
    whole: Arc<Source>,
    /// Bytes of pages 1‥ within it.
    ranges: Vec<std::ops::Range<usize>>,
    /// A stereo pair (two disparity images): page 0 the left view, page 1 the right.
    stereo: bool,
}

impl Mpo {
    fn pages(&self) -> u32 {
        1 + self.ranges.len() as u32
    }
}

fn open_session(source: Arc<Source>, mpo: Option<Arc<Mpo>>, page: u32) -> Result<JpegSession> {
    let ji = sj::read_info(source.bytes()).map_err(map_err)?;
    let orientation = ji
        .exif
        .clone()
        .and_then(|r| source.bytes().get(r))
        .and_then(crate::exif::exif_orientation)
        .map(Orientation::from_exif)
        .unwrap_or_default();
    let orientation = if orientation.0 == 0 {
        Orientation::NORMAL
    } else {
        orientation
    };
    // SOF10 (arithmetic-coded progressive) is progressive too: several scans, the row
    // index needs a pass over all of them.
    let progressive = matches!(ji.coding, sj::Coding::Progressive) || ji.sof_marker == 0xCA;
    let sub = subsampling_name(&ji);
    let image = mpo.as_ref().map_or(String::new(), |m| {
        if m.stereo {
            format!(
                ", MPO stereo {} view",
                if page == 0 { "left" } else { "right" }
            )
        } else {
            format!(", MPF image {} of {}", page + 1, m.pages())
        }
    });
    let coding = format!(
        "{} {}{}{image}",
        match ji.coding {
            sj::Coding::Baseline => "baseline",
            sj::Coding::ExtendedHuffman => "extended",
            sj::Coding::Progressive => "progressive",
            sj::Coding::Arithmetic if progressive => "arithmetic progressive",
            sj::Coding::Arithmetic => "arithmetic",
            sj::Coding::Lossless => "lossless",
        },
        sub,
        if ji.restart_interval > 0 {
            ", restart markers"
        } else {
            ""
        }
    );
    let coding = if sj::is_hierarchical(source.bytes()) {
        format!("hierarchical {coding}")
    } else {
        coding
    };
    let info = ImageInfo {
        format: Format::Jpeg,
        width: ji.width,
        height: ji.height,
        orientation,
        icc_profile: ji.icc_profile.clone().map(Arc::new),
        has_alpha: false,
        bit_depth: ji.precision,
        progressive,
        coding,
        embedded_preview: ji.exif_thumbnail.clone(),
        fast_regions: true,
        scales: &[1, 2, 4, 8],
    };
    Ok(JpegSession {
        source,
        info,
        jinfo: ji,
        index: OnceLock::new(),
        index_lock: Mutex::new(()),
        estimates: Mutex::new(Vec::new()),
        mpo,
        page,
    })
}

fn subsampling_name(ji: &sj::JpegInfo) -> &'static str {
    if ji.components.len() == 1 {
        return "greyscale";
    }
    let c = &ji.components;
    if c.len() >= 3 && c[1].h_samp == c[2].h_samp && c[1].v_samp == c[2].v_samp {
        match (ji.max_h / c[1].h_samp.max(1), ji.max_v / c[1].v_samp.max(1)) {
            (1, 1) => "4:4:4",
            (2, 1) => "4:2:2",
            (2, 2) => "4:2:0",
            (1, 2) => "4:4:0",
            (4, 1) => "4:1:1",
            _ => "subsampled",
        }
    } else {
        "subsampled"
    }
}

pub(crate) fn map_err(e: sj::JpegError) -> DecodeError {
    match e {
        sj::JpegError::NotJpeg => DecodeError::UnsupportedFormat,
        sj::JpegError::Truncated(s) => DecodeError::Truncated(s.into()),
        sj::JpegError::Malformed(s) => DecodeError::Malformed(s.into()),
        sj::JpegError::Unsupported(s) => DecodeError::Unsupported(s.into()),
        sj::JpegError::TooLarge(_) => DecodeError::TooLarge,
        sj::JpegError::Cancelled => DecodeError::Cancelled,
        sj::JpegError::IndexMismatch => DecodeError::Malformed("row index mismatch".into()),
        sj::JpegError::Fallback(s) => DecodeError::Malformed(s),
        other => DecodeError::Malformed(other.to_string()),
    }
}

pub(crate) fn scale_of(s: u32) -> sj::Scale {
    match s {
        0 | 1 => sj::Scale::Full,
        2 | 3 => sj::Scale::Half,
        4..=7 => sj::Scale::Quarter,
        _ => sj::Scale::Eighth,
    }
}

pub(crate) fn to_raster(d: sj::DecodedPlanes, scale: u32) -> Raster {
    let max_h = d.planes.iter().map(|p| p.h_samp).max().unwrap_or(1).max(1);
    let max_v = d.planes.iter().map(|p| p.v_samp).max().unwrap_or(1).max(1);
    let layout = match d.layout {
        sj::PlanesLayout::Gray => PixelLayout::Gray,
        sj::PlanesLayout::YCbCr => PixelLayout::YCbCr,
        sj::PlanesLayout::Rgb => PixelLayout::Rgb,
        sj::PlanesLayout::Rgba => PixelLayout::Rgba,
    };
    let planes = d
        .planes
        .into_iter()
        .map(|p| RasterPlane {
            width: p.width,
            height: p.height,
            stride: p.stride,
            h_div: if layout == PixelLayout::Rgba {
                1
            } else {
                max_h / p.h_samp.max(1)
            },
            v_div: if layout == PixelLayout::Rgba {
                1
            } else {
                max_v / p.v_samp.max(1)
            },
            data: p.data,
        })
        .collect();
    Raster {
        width: d.width,
        height: d.height,
        origin: d.origin,
        scale,
        layout,
        planes,
        truncated: d.truncated,
    }
}

/// Scale factor, region width, region height.
type EstimateKey = (u32, u32, u32);

/// Working memory of a plain whole-image progressive decode above which levels build the row
/// index first (see `JpegSession::index_first`).
const INDEX_FIRST_WORKING_BYTES: u64 = 64 << 20;

struct JpegSession {
    source: Arc<Source>,
    info: ImageInfo,
    jinfo: sj::JpegInfo,
    /// The row index, kept for the session's life: [`Session::trim_memory`] leaves it (the
    /// default, nothing released). It is small next to what it saves (measured, 16 threads
    /// under load: 24 MP 4:2:0 13 KiB, rebuilt in 20 ms; 108 MP baseline 33 KiB, 20 ms; 50 MP
    /// progressive 277 KiB, 250 ms; 24 MP arithmetic progressive 1 MiB, 660 ms), and without
    /// it each region decode is a pass over the whole file (a 512-px 1:1 tile 25-340 ms instead
    /// of 1-120 ms) until it is rebuilt; dropping it would also make `regions_ready` false
    /// again, which holds back the app's detail tiles while a level is pending.
    index: OnceLock<Arc<sj::RowIndex>>,
    index_lock: Mutex<()>,
    /// Peak-memory estimates from the file's real scan script, by (scale, region size).
    /// Tile requests come in a handful of sizes, so this stays tiny.
    estimates: Mutex<Vec<(EstimateKey, sj::MemoryEstimate)>>,
    /// Multi-Picture files: the pages, and which one this is (0 = the primary).
    mpo: Option<Arc<Mpo>>,
    page: u32,
}

impl JpegSession {
    fn opts<'a>(&self, req: &DecodeRequest, cancel: &'a CancelToken) -> sj::DecodeOptions<'a> {
        sj::DecodeOptions {
            scale: scale_of(req.scale),
            region: req.region.map(|r| sj::Region {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            cancel: Some(cancel.flag()),
            parallelism: if req.parallel {
                sj::Parallelism::Auto
            } else {
                sj::Parallelism::Single
            },
        }
    }

    /// The software decode (see [`crate::hw_jpeg`] for when the hardware takes over).
    fn decode_software(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let data = self.source.bytes();
        let scale = scale_of(req.scale).factor();
        let opts = self.opts(req, cancel);
        let index_first = self.index.get().is_none() && self.index_first(req);
        let how = if self.index.get().is_some() {
            "with row index"
        } else if index_first {
            "building the row index first"
        } else if self.records_index(req) {
            "recording the row index"
        } else {
            "plain"
        };
        match req.region {
            None => {
                specular_log::debug!(target: specular_log::DECODE, "jpeg: decode 1/{scale} whole image, {how}, parallel {}", req.parallel)
            }
            Some(r) => {
                specular_log::trace!(target: specular_log::DECODE, "jpeg: decode 1/{scale} region {}×{}+{}+{}, {how}", r.w, r.h, r.x, r.y)
            }
        }
        if index_first {
            match self.prepare_regions(cancel) {
                Ok(()) => {}
                Err(DecodeError::Cancelled) => return Err(DecodeError::Cancelled),
                // A file the index cannot cover (cut short) still decodes what is there.
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "jpeg: no row index ({e}); plain decode");
                    specular_core::paths::note_scale_fallback(
                        specular_core::paths::ScaleFallback::NoIndex,
                    );
                }
            }
        }
        if let Some(ix) = self.index.get() {
            return sj::decode_with_index(data, ix, &opts)
                .map(|d| to_raster(d, scale))
                .map_err(map_err);
        }
        // Sequential files without restart markers record the row index almost for free
        // while decoding (files with restart markers index in ~8 ms separately; for
        // progressive files the index costs a pass over every scan, see `prepare_regions`).
        if self.records_index(req) {
            let _g = self.index_lock.lock();
            if self.index.get().is_none() {
                match sj::decode_and_index(data, &opts) {
                    Ok((d, ix)) => {
                        let _ = self.index.set(Arc::new(ix));
                        return Ok(to_raster(d, scale));
                    }
                    // A cut-short file has no complete index, but the rows that are there
                    // still show (with the "incomplete" badge).
                    Err(sj::JpegError::Truncated(why)) => {
                        specular_log::debug!(target: specular_log::DECODE, "jpeg: {why}; decoding what is there without an index");
                    }
                    Err(e) => return Err(map_err(e)),
                }
                drop(_g);
                return sj::decode(data, &opts)
                    .map(|d| to_raster(d, scale))
                    .map_err(map_err);
            }
            drop(_g);
            return self.decode_software(req, cancel);
        }
        sj::decode(data, &opts)
            .map(|d| to_raster(d, scale))
            .map_err(map_err)
    }

    /// Whether a whole-image level records the row index while decoding: sequential files
    /// without restart markers (with them the index takes ~8 ms separately), and arithmetic-
    /// coded sequential files (their separate index pass is a whole entropy decode).
    fn records_index(&self, req: &DecodeRequest) -> bool {
        req.prepare_regions
            && req.region.is_none()
            && !self.info.progressive
            && (self.jinfo.restart_interval == 0 || self.jinfo.coding == sj::Coding::Arithmetic)
    }

    /// Whether a whole-image progressive level builds the row index before decoding: with
    /// it the level decodes in horizontal stripes of bounded coefficient memory (a 100 MP
    /// file at 1/4: 99 instead of 370 MB peak, about as fast), and the index is ready for
    /// the first zoom. Not for 1/8 (DC only: small, and the fastest first pixels) or when
    /// the coefficient buffer is small anyway (the index pass would only add time).
    /// Not for arithmetic coding: its indexed decode keeps the coefficients of every row it
    /// needs (no stripes), so building the index first would only add a pass.
    fn index_first(&self, req: &DecodeRequest) -> bool {
        self.info.progressive
            && self.jinfo.coding != sj::Coding::Arithmetic
            && req.prepare_regions
            && req.region.is_none()
            && req.scale < 8
            && sj::memory_estimate(&self.jinfo, scale_of(req.scale), None).working_bytes
                >= INDEX_FIRST_WORKING_BYTES
    }

    /// Peak-memory estimate for this scale and region size. Progressive files use the real
    /// scan script (one marker pass, ~2 ms for 34 MP, cached per size); sequential files have
    /// one scan, so the header-only estimate is the same number for free (the marker pass
    /// would cost 15-23 ms on a 100 MB file, on the UI thread).
    fn estimate(&self, scale: sj::Scale, region: Option<sj::Region>) -> sj::MemoryEstimate {
        if !self.info.progressive {
            return sj::memory_estimate(&self.jinfo, scale, region);
        }
        let key = (
            scale.factor(),
            region.map_or(0, |r| r.w),
            region.map_or(0, |r| r.h),
        );
        if let Some((_, e)) = self.estimates.lock().iter().find(|(k, _)| *k == key) {
            return *e;
        }
        let e = sj::memory_estimate_for(self.source.bytes(), scale, region)
            .unwrap_or_else(|_| sj::memory_estimate(&self.jinfo, scale, region));
        let mut c = self.estimates.lock();
        if c.len() >= 32 {
            c.remove(0);
        }
        c.push((key, e));
        e
    }
}

impl Session for JpegSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// Without a profile: one component is greyscale, four CMYK (or YCCK).
    fn colour_space(&self) -> Option<&'static str> {
        if self.info.icc_profile.is_some() {
            return None;
        }
        match self.jinfo.components.len() {
            1 => Some("grey"),
            4 => Some("cmyk"),
            _ => None,
        }
    }

    fn source(&self) -> &Arc<Source> {
        &self.source
    }

    fn decode_preview(&self) -> Option<Result<Raster>> {
        let r = self.info.embedded_preview.clone()?;
        let data = self.source.bytes().get(r)?;
        let opts = sj::DecodeOptions {
            parallelism: sj::Parallelism::Single,
            ..Default::default()
        };
        Some(
            sj::decode(data, &opts)
                .map(|d| to_raster(d, 0))
                .map_err(map_err),
        )
    }

    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        crate::hw_jpeg::decode(self.source.bytes(), &self.jinfo, req, cancel, || {
            self.decode_software(req, cancel)
        })
    }

    fn has_gain_map(&self) -> bool {
        // A further MPF image is not searched: its trailing bytes are the next page.
        self.page == 0 && crate::gainmap::jpeg_has_gain_map(self.source.bytes())
    }

    fn gain_map(&self, cancel: &CancelToken) -> Option<specular_core::GainMap> {
        if self.page > 0 {
            return None;
        }
        crate::gainmap::jpeg_gain_map(self.source.bytes(), cancel)
    }

    fn page_count(&self) -> u32 {
        self.mpo.as_ref().map_or(1, |m| m.pages())
    }

    fn page_index(&self) -> u32 {
        self.page
    }

    fn page_label(&self) -> Option<PageLabel> {
        if self.mpo.as_ref()?.stereo {
            return Some(PageLabel::Stereo {
                right: self.page == 1,
            });
        }
        Some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }

    /// Page 0 is the whole file, pages 1‥ views of the further MPF images found at open.
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        let Some(m) = self.mpo.as_ref().filter(|m| page < m.pages()) else {
            return Err(DecodeError::Malformed("no such page".into()));
        };
        let source = if page == 0 {
            m.whole.clone()
        } else {
            let r = m.ranges[page as usize - 1].clone();
            let bytes = m
                .whole
                .bytes()
                .get(r)
                .ok_or(DecodeError::Truncated("MPF image outside the file".into()))?;
            Arc::new(
                Source::from_slice(m.whole.path(), m.whole.clone(), bytes)
                    .ok_or_else(|| DecodeError::Malformed("MPF image range".into()))?,
            )
        };
        Ok(Arc::new(open_session(source, Some(m.clone()), page)?))
    }

    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        if self.index.get().is_some() {
            return Ok(());
        }
        let _g = self.index_lock.lock();
        if self.index.get().is_some() {
            return Ok(());
        }
        let opts = sj::DecodeOptions {
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let t0 = std::time::Instant::now();
        let ix = sj::build_index_with(self.source.bytes(), &opts).map_err(map_err)?;
        specular_log::debug!(target: specular_log::DECODE, "jpeg: row index built in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
        let _ = self.index.set(Arc::new(ix));
        Ok(())
    }

    fn regions_ready(&self) -> bool {
        self.index.get().is_some()
    }

    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let region = req.region.map(|r| sj::Region {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        });
        let e = self.estimate(scale_of(req.scale), region);
        if self.index.get().is_none() && self.index_first(req) {
            // The index build, then the decode with the index kept.
            e.index_build_bytes.max(e.peak_with_index() + e.index_bytes)
        } else if self.index.get().is_some() || req.region.is_some() {
            e.peak_with_index()
        } else {
            e.peak()
        }
    }

    fn open_memory(&self, levels: &[u32]) -> u64 {
        let est: Vec<_> = levels
            .iter()
            // From the headers alone: the scan pass of `estimate` costs ~10 ms on a 50 MP
            // progressive file, on the open path, and the dominant term does not need it.
            .map(|&s| sj::memory_estimate(&self.jinfo, scale_of(s), None))
            .collect();
        // Sequential files build the row index during the first level. Progressive ones
        // build it before a big level (`index_first`), which then decodes in stripes, or in
        // a separate pass on the first zoom, after the levels have finished (outputs kept).
        let index = est.first().map_or(0, |e| e.index_bytes);
        if self.info.progressive {
            // (sum over levels, sum over the levels not waiting for the index)
            let (mut total, mut small, mut striped) = (0u64, 0u64, false);
            for (&s, e) in levels.iter().zip(&est) {
                if s < 8
                    && e.working_bytes >= INDEX_FIRST_WORKING_BYTES
                    && self.jinfo.coding != sj::Coding::Arithmetic
                {
                    striped = true;
                    total += e.peak_with_index();
                } else {
                    total += e.peak();
                    small += e.peak();
                }
            }
            let build = est.first().map_or(0, |e| e.index_build_bytes);
            if striped {
                // The index pass runs alongside the small levels, before the striped ones.
                (total + index).max(build + small)
            } else {
                let outputs: u64 = est.iter().map(|e| e.output_bytes).sum();
                total.max(outputs + build)
            }
        } else {
            let levels: u64 = est.iter().map(|e| e.peak()).sum::<u64>();
            levels + index
        }
    }

    fn memory_bytes(&self) -> usize {
        self.index.get().map(|i| i.memory_bytes()).unwrap_or(0)
    }
}

/// Crop `src` to the region (scaled coordinates), expanding to 16-pixel alignment so that
/// subsampled planes crop exactly.
pub(crate) fn crop(src: &Raster, r: crate::Rect) -> Raster {
    let x0 = (r.x.max(src.origin.0) - src.origin.0) & !15;
    let y0 = (r.y.max(src.origin.1) - src.origin.1) & !15;
    let x1 = (r.x + r.w).saturating_sub(src.origin.0).min(src.width);
    let y1 = (r.y + r.h).saturating_sub(src.origin.1).min(src.height);
    let (w, h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let planes = src
        .planes
        .iter()
        .map(|p| {
            let bpp = p.bytes_per_pixel();
            let px0 = (x0 / p.h_div as u32) as usize;
            let py0 = (y0 / p.v_div as u32) as usize;
            let pw = w.div_ceil(p.h_div as u32).min(p.width - px0 as u32) as usize;
            let ph = h.div_ceil(p.v_div as u32).min(p.height - py0 as u32) as usize;
            let stride = pw * bpp;
            let mut data = vec![0u8; stride * ph];
            for y in 0..ph {
                let s = (py0 + y) * p.stride + px0 * bpp;
                data[y * stride..(y + 1) * stride].copy_from_slice(&p.data[s..s + stride]);
            }
            RasterPlane {
                width: pw as u32,
                height: ph as u32,
                stride,
                data,
                h_div: p.h_div,
                v_div: p.v_div,
            }
        })
        .collect();
    Raster {
        width: w,
        height: h,
        origin: (src.origin.0 + x0, src.origin.1 + y0),
        scale: src.scale,
        layout: src.layout,
        planes,
        truncated: src.truncated,
    }
}

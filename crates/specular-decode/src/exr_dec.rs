//! OpenEXR through `specular-exr`: each part, as linear half-float RGBA (`Rgba16F`, 1.0 =
//! reference white) for the HDR path (tone-mapped or shown on an HDR surface by the renderer).
//!
//! * **Pages.** A multi-part file has a page per part with a data window, in file order (page 0
//!   is the first, the image a single-part reader shows), labelled with the part's name. Deep
//!   parts are shown flattened (samples composited front to back by `Z`; "deep, flattened" in
//!   the coding). The headers are read once at open and shared by every page. The layers
//!   inside one part (`left.R`, `diffuse.R`, …) are not pages.
//! * **Levels.** Nothing is decoded whole: 1/2, 1/4 and 1/8 are box-filtered in linear light
//!   as the scanline blocks (or rows of tiles) stream, from the smallest mip (or ripmap
//!   diagonal) level that divides the scale, else from full size, so peak memory is the output
//!   and a few blocks per thread. Regions decode only the chunks they meet. The 1/8 level (or
//!   the smallest mip at least 1/4) is the quick preview of large mipmapped files.
//! * **Colour.** BT.709 / Display P3 / BT.2020 chromaticities are passed on as the raster's
//!   primaries (converted on the GPU); any other set is converted to BT.709 on the CPU
//!   (Bradford white adaptation).
//! * **Windows.** The data window is shown; a different display window is only reported.
//! * **Alpha.** OpenEXR colour is premultiplied; it is divided out to the straight convention.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Primaries, Raster, Source, hdr};
use specular_exr as exr;

use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use crate::{linear, radiance_dec};

pub struct ExrDecoder;

fn map_err(e: exr::ExrError) -> DecodeError {
    match e {
        exr::ExrError::NotExr => DecodeError::UnsupportedFormat,
        exr::ExrError::Truncated => DecodeError::Truncated("header".into()),
        exr::ExrError::Unsupported(s) => DecodeError::Unsupported(s.into()),
        exr::ExrError::UnsupportedCompression(s) => {
            DecodeError::Unsupported(format!("{s} compression"))
        }
        exr::ExrError::TooLarge { .. } | exr::ExrError::OutOfMemory(_) => DecodeError::TooLarge,
        exr::ExrError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        other => DecodeError::Malformed(other.to_string()),
    }
}

/// How pixels reach BT.709-or-tagged primaries.
#[derive(Clone, Copy, Debug)]
enum Colour {
    /// Known primaries: decode straight to half floats.
    Tagged(Primaries),
    /// Other chromaticities: decode to f32 and convert to BT.709.
    Matrix([[f32; 3]; 3]),
}

impl Decoder for ExrDecoder {
    fn format(&self) -> Format {
        Format::Exr
    }
    fn probe(&self, head: &[u8]) -> bool {
        exr::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["exr"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let file = exr::read_info(source.bytes()).map_err(map_err)?;
        let pages: Arc<[usize]> = (0..file.parts.len())
            .filter(|&i| {
                let p = &file.parts[i];
                let (w, h) = (p.width(), p.height());
                w > 0 && h > 0 && w <= u64::from(u32::MAX) && h <= u64::from(u32::MAX)
            })
            .collect();
        if pages.is_empty() {
            return Err(DecodeError::Malformed("empty data window".into()));
        }
        Ok(Arc::new(open_part(source, Arc::new(file), pages, 0)))
    }
}

/// Page `page` (part `pages[page]`) of a file whose headers are read.
fn open_part(
    source: Arc<Source>,
    file: Arc<exr::ExrInfo>,
    pages: Arc<[usize]>,
    page: u32,
) -> ExrSession {
    let part_index = pages[page as usize];
    let part = &file.parts[part_index];
    let (width, height) = (part.width(), part.height());
    let colour = match part.chromaticities {
        None => Colour::Tagged(Primaries::Bt709),
        Some(c) => {
            let c = [c.red, c.green, c.blue, c.white];
            match hdr::primaries_matching(c, 2e-3) {
                Some(p) => Colour::Tagged(p),
                None => hdr::chromaticities_to_bt709(c)
                    .map_or(Colour::Tagged(Primaries::Bt709), Colour::Matrix),
            }
        }
    };
    // Mip levels k = 1..=3 present along the diagonal.
    let mips: Vec<u32> = (1..=3)
        .filter(|&k| part.level_index(k, k).is_some())
        .map(|k| 1 << k)
        .collect();
    let half = part
        .channels
        .iter()
        .all(|c| c.pixel_type == exr::PixelType::Half);
    let has_alpha = part
        .channels
        .iter()
        .any(|c| c.name == "A" || c.name.ends_with(".A"));
    let info = ImageInfo {
        format: Format::Exr,
        width: width as u32,
        height: height as u32,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha,
        bit_depth: if half { 16 } else { 32 },
        progressive: false,
        coding: coding(&file, part_index, colour, &mips),
        embedded_preview: None,
        fast_regions: true,
        scales: &[1, 2, 4, 8],
    };
    let model = exr::auto_color(part).map(|c| match c {
        exr::ColorKind::Rgb => "RGB",
        exr::ColorKind::Luminance => "Greyscale",
        exr::ColorKind::LuminanceChroma => "YC",
    });
    ExrSession {
        source,
        info,
        part: part_index,
        colour,
        model,
        mips,
        file,
        pages,
        page,
    }
}

fn coding(file: &exr::ExrInfo, part: usize, colour: Colour, mips: &[u32]) -> String {
    let p = &file.parts[part];
    let half = p
        .channels
        .iter()
        .all(|c| c.pixel_type == exr::PixelType::Half);
    let mut s = format!(
        "{} {}, {}",
        if half { "half" } else { "float" },
        if p.tile_desc().is_some() {
            "tiled"
        } else {
            "scanline"
        },
        p.compression.name()
    );
    if p.is_deep() {
        s += ", deep, flattened";
    }
    if p.levels.len() > 1 {
        s += &format!(", {} levels", p.levels.len());
        if !mips.is_empty() {
            s += " (mipmap)";
        }
    }
    if file.parts.len() > 1 {
        s += &format!(", part {} of {}", part + 1, file.parts.len());
    }
    let layers = p.layers();
    if layers.len() > 1 {
        s += &format!(", {} layers", layers.len());
    }
    match colour {
        Colour::Tagged(Primaries::Bt709) => {}
        Colour::Tagged(pr) => s += &format!(", {pr:?} primaries"),
        Colour::Matrix(_) => s += ", custom chromaticities",
    }
    if p.display_window != p.data_window {
        s += &format!(
            ", data window {}×{} in display window {}×{}",
            p.data_window.width(),
            p.data_window.height(),
            p.display_window.width(),
            p.display_window.height()
        );
    }
    s
}

struct ExrSession {
    source: Arc<Source>,
    info: ImageInfo,
    /// The headers, shared by every page.
    file: Arc<exr::ExrInfo>,
    part: usize,
    /// The part of each page.
    pages: Arc<[usize]>,
    page: u32,
    colour: Colour,
    /// The shown channels' colour model; `None` for parts without colour channels.
    model: Option<&'static str>,
    /// Scales (2, 4, 8) that have a mip level of their own.
    mips: Vec<u32>,
}

impl ExrSession {
    /// The level 1/`scale` is box-filtered from: the smallest mip level it divides (by a
    /// power of two), else full size.
    fn base_of(&self, scale: u32) -> u32 {
        self.mips
            .iter()
            .copied()
            .filter(|&m| m <= scale && scale.is_multiple_of(m))
            .max()
            .unwrap_or(1)
    }

    /// A reader of level 1/`base` (1 or a mip scale).
    fn reader(&self, base: u32) -> Result<exr::RowReader<'_>> {
        let level = if base == 1 {
            exr::LevelSelect::Full
        } else {
            let k = base.trailing_zeros();
            let i = self.file.parts[self.part]
                .level_index(k, k)
                .ok_or_else(|| DecodeError::Malformed("missing mip level".into()))?;
            exr::LevelSelect::Index(i)
        };
        let opts = exr::DecodeOptions {
            part: self.part,
            level,
            ..Default::default()
        };
        exr::RowReader::new(self.source.bytes(), &self.file, &opts).map_err(map_err)
    }

    fn grid(&self, scale: u32, base: u32, reader: &exr::RowReader) -> linear::Grid {
        let l = reader.level();
        let mut g = linear::Grid {
            // Mip levels round down (or up); the viewer expects ceil(size / scale).
            size: (
                self.info.width.div_ceil(base),
                self.info.height.div_ceil(base),
            ),
            real: (l.width, l.height),
            f: scale / base,
            band: reader.band_rows(),
            scratch: 0,
        };
        g.scratch = reader.scratch_bytes(g.group_rows());
        g
    }

    fn prep(&self) -> (linear::Prep, Primaries) {
        // OpenEXR colour is premultiplied; rasters (and the reduction) are straight.
        let unpremultiply = self.info.has_alpha;
        match self.colour {
            Colour::Tagged(p) => (
                linear::Prep {
                    matrix: None,
                    clamp_alpha: false,
                    unpremultiply,
                },
                p,
            ),
            Colour::Matrix(m) => (
                linear::Prep {
                    matrix: Some(m),
                    clamp_alpha: true,
                    unpremultiply,
                },
                Primaries::Bt709,
            ),
        }
    }
}

impl Session for ExrSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.model
    }
    /// OpenEXR colour is premultiplied by its alpha (A) by definition.
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(Alpha::Premultiplied)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    /// The 1/8 (or smallest ≥ 1/8) mip level of large mipmapped files.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        let scale = *self.mips.iter().filter(|&&s| s >= 4).max()?;
        if px < 2_000_000 {
            return None;
        }
        let req = DecodeRequest {
            parallel: false,
            ..DecodeRequest::full(scale)
        };
        Some(self.decode(&req, &CancelToken::new()))
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        let base = self.base_of(scale);
        let reader = self.reader(base)?;
        let (prep, primaries) = self.prep();
        let out = radiance_dec::out_rect(self.info.width, self.info.height, scale, req.region);
        let mut r = linear::reduce(
            self.grid(scale, base, &reader),
            out,
            prep,
            primaries,
            false,
            req.parallel,
            cancel,
            |rows, cols, buf, s: &mut exr::RowScratch| reader.read_rows(rows, cols, buf, s) > 0,
        )?;
        r.scale = scale;
        Ok(r)
    }
    /// Mip levels make reduced scales cheap; otherwise every chunk is decoded at any scale.
    fn scaled_decode_cheap(&self) -> bool {
        !self.mips.is_empty()
    }
    fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages.len() > 1).then(|| PageLabel::DocumentLayer {
            name: self.file.parts[self.part]
                .name
                .as_deref()
                .unwrap_or_default()
                .into(),
            index: self.page + 1,
            hidden: false,
        })
    }
    /// Another part, from the headers read at open.
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.pages.len() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        Ok(Arc::new(open_part(
            self.source.clone(),
            self.file.clone(),
            self.pages.clone(),
            page,
        )))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        let base = self.base_of(scale);
        let out = radiance_dec::out_rect(self.info.width, self.info.height, scale, req.region);
        match self.reader(base) {
            Ok(r) => self.grid(scale, base, &r).memory(out, req.parallel),
            // The decode fails early.
            Err(_) => u64::from(out.w) * u64::from(out.h) * 8,
        }
    }
}

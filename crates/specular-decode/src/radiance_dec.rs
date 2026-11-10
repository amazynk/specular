//! Radiance RGBE (`.hdr`, `.pic`) and Portable Float Map (`.pfm`) through `specular-radiance`,
//! as linear half-float RGBA (`Rgba16F`, 1.0 = reference white) for the HDR path.
//!
//! Values are shown as stored (Radiance `EXPOSURE` is reported, not undone — like other
//! viewers). XYZE files are converted from XYZ, and a `PRIMARIES` line other than BT.709 is
//! converted to BT.709 (Bradford white adaptation); files without one are taken as BT.709.
//!
//! Files stored row by row (PFM, and Radiance in any orientation but the x-major ones) never
//! hold the whole `f32` image: the scanlines are located once (a skip pass over the run
//! headers) and levels 1/2/4/8 are box-filtered in linear light as the rows stream, regions
//! only reading their rows — so an 8k panorama opens at whatever level fits the budget, and
//! zooms in by regions. x-major files decode whole and cache the level.

use std::sync::Arc;

use parking_lot::Mutex;

use specular_core::{CancelToken, Orientation, Primaries, Raster, Source, hdr};
use specular_radiance as rad;

use crate::linear;
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Rect, Result, Session};

/// Radiance (`Format::Hdr`) or PFM (`Format::Pfm`): one reader, two registrations, so each
/// claims its own extensions and MIME type.
pub struct RadianceDecoder(pub Format);

fn map_err(e: rad::Error) -> DecodeError {
    match e {
        rad::Error::NotRecognized => DecodeError::UnsupportedFormat,
        rad::Error::Unsupported(s) => DecodeError::Unsupported(s.into()),
        rad::Error::TooLarge { .. } | rad::Error::OutOfMemory(_) => DecodeError::TooLarge,
        rad::Error::InvalidHeader(s) => DecodeError::Malformed(s.into()),
    }
}

/// Matrix to BT.709, if the file needs one.
fn matrix(i: &rad::Info) -> Option<[[f32; 3]; 3]> {
    if i.color_space == rad::ColorSpace::Xyz {
        return Some(hdr::xyz_to_bt709());
    }
    let p = i.primaries?;
    let c = [[p[0], p[1]], [p[2], p[3]], [p[4], p[5]], [p[6], p[7]]];
    if hdr::primaries_matching(c, 2e-3) == Some(Primaries::Bt709) {
        return None;
    }
    hdr::chromaticities_to_bt709(c)
}

impl Decoder for RadianceDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        let want = if self.0 == Format::Pfm {
            rad::Format::Pfm
        } else {
            rad::Format::Radiance
        };
        rad::detect(head) == Some(want)
    }
    fn extensions(&self) -> &'static [&'static str] {
        if self.0 == Format::Pfm {
            &["pfm"]
        } else {
            &["hdr", "pic", "rgbe"]
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let i = rad::read_info(source.bytes()).map_err(map_err)?;
        if i.width == 0 || i.height == 0 {
            return Err(DecodeError::Malformed("empty image".into()));
        }
        let m = matrix(&i);
        let info = ImageInfo {
            format: match i.format {
                rad::Format::Pfm => Format::Pfm,
                rad::Format::Radiance => Format::Hdr,
            },
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: i.channels == 4,
            bit_depth: 32,
            progressive: false,
            coding: coding(&i, m.is_some()),
            embedded_preview: None,
            // Row by row: regions and reduced levels stream from the scanlines.
            fast_regions: !i.orientation.x_major,
            scales: &[1, 2, 4, 8],
        };
        let model = match (i.format, i.color_space) {
            (rad::Format::Radiance, rad::ColorSpace::Xyz) => "XYZE",
            (rad::Format::Radiance, _) => "RGBE",
            (rad::Format::Pfm, _) if i.channels == 1 => "Greyscale",
            (rad::Format::Pfm, _) => "RGB",
        };
        Ok(Arc::new(RadianceSession {
            source,
            info,
            rinfo: i,
            model,
            matrix: m,
            index: Mutex::new(None),
            levels: Levels::new(),
        }))
    }
}

fn coding(i: &rad::Info, converted: bool) -> String {
    let mut s = match i.format {
        rad::Format::Radiance => match i.color_space {
            rad::ColorSpace::Rgb => "Radiance RGBE".to_string(),
            rad::ColorSpace::Xyz => "Radiance XYZE".to_string(),
        },
        rad::Format::Pfm => format!(
            "PFM {}, {}",
            match i.channels {
                1 => "grey",
                4 => "RGBA",
                _ => "RGB",
            },
            if i.encoding == rad::Encoding::F32Le {
                "little-endian"
            } else {
                "big-endian"
            }
        ),
    };
    if i.exposure != 1.0 {
        s += &format!(", exposure {}", i.exposure);
    }
    if i.primaries.is_some() && converted && i.color_space == rad::ColorSpace::Rgb {
        s += ", custom primaries";
    }
    if i.orientation != rad::Orientation::STANDARD {
        s += ", rotated scan order";
    }
    s
}

struct RadianceSession {
    source: Arc<Source>,
    info: ImageInfo,
    rinfo: rad::Info,
    model: &'static str,
    matrix: Option<[[f32; 3]; 3]>,
    /// Row access to the file, built on first use (`None` inside: x-major Radiance, which is
    /// decoded whole through `levels`).
    index: Mutex<Option<Arc<Option<rad::RowIndex>>>>,
    /// Whole levels of x-major files.
    levels: Levels,
}

impl RadianceSession {
    /// The file's row index, located on first use (a skip pass over the run headers).
    fn index(&self) -> Result<Arc<Option<rad::RowIndex>>> {
        let mut g = self.index.lock();
        if let Some(i) = &*g {
            return Ok(i.clone());
        }
        let i = Arc::new(
            rad::RowIndex::new(self.source.bytes(), &self.rinfo, &rad::Limits::default())
                .map_err(map_err)?,
        );
        *g = Some(i.clone());
        Ok(i)
    }

    /// Files stored row by row (all but x-major Radiance) stream.
    fn rows(&self) -> bool {
        self.info.fast_regions
    }

    fn prep(&self) -> linear::Prep {
        linear::Prep {
            matrix: self.matrix,
            clamp_alpha: true,
            unpremultiply: false,
        }
    }

    fn grid(&self, scale: u32) -> linear::Grid {
        let size = (self.info.width, self.info.height);
        linear::Grid {
            size,
            real: size,
            f: scale,
            band: 1,
            // One scanline's planes.
            scratch: u64::from(self.info.width) * 4,
        }
    }

    /// The pixels of level 1/`scale` that `req` asks for (the whole level, or the region
    /// clipped to it).
    fn out_rect(&self, scale: u32, region: Option<Rect>) -> Rect {
        out_rect(self.info.width, self.info.height, scale, region)
    }

    fn decode_full(&self, parallel: bool) -> Result<Raster> {
        let opts = rad::DecodeOptions {
            channels: rad::OutputChannels::Rgba,
            parallel,
            ..Default::default()
        };
        let img = rad::decode(self.source.bytes(), &opts).map_err(map_err)?;
        Ok(linear::f32_to_raster(
            img.width,
            img.height,
            4,
            &img.data,
            self.matrix,
            Primaries::Bt709,
            img.truncated,
            parallel,
        ))
    }
}

/// Level 1/`scale` of a `w`×`h` image, or `region` clipped to it.
pub(crate) fn out_rect(w: u32, h: u32, scale: u32, region: Option<Rect>) -> Rect {
    let (lw, lh) = (w.div_ceil(scale.max(1)), h.div_ceil(scale.max(1)));
    match region {
        None => Rect {
            x: 0,
            y: 0,
            w: lw,
            h: lh,
        },
        Some(r) => {
            let (x, y) = (r.x.min(lw), r.y.min(lh));
            Rect {
                x,
                y,
                w: r.x.saturating_add(r.w).min(lw) - x,
                h: r.y.saturating_add(r.h).min(lh) - y,
            }
        }
    }
}

impl Session for RadianceSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if !self.rows() {
            return self.levels.decode(
                req,
                (self.info.width, self.info.height),
                |_| 1,
                |_| self.decode_full(req.parallel),
                cancel,
            );
        }
        let scale = req.scale.max(1);
        let index = self.index()?;
        let Some(index) = index.as_ref() else {
            return Err(DecodeError::Unsupported("row access".into()));
        };
        let data = self.source.bytes();
        let mut r = linear::reduce(
            self.grid(scale),
            self.out_rect(scale, req.region),
            self.prep(),
            Primaries::Bt709,
            index.truncated(),
            req.parallel,
            cancel,
            |rows, cols, buf, line: &mut Vec<u8>| {
                index.read_rows(data, &self.rinfo, rows, cols, buf, line);
                false
            },
        )?;
        r.scale = scale;
        Ok(r)
    }
    /// Locates the scanlines (Radiance), so that regions decode only their rows.
    fn prepare_regions(&self, _cancel: &CancelToken) -> Result<()> {
        if self.rows() {
            self.index()?;
        }
        Ok(())
    }
    fn regions_ready(&self) -> bool {
        !self.rows() || self.index.lock().is_some()
    }
    /// Run-length scanlines decode whole at any scale.
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        if !self.rows() {
            let mut m = whole::level_bytes(w, h, s, 8);
            if !self.levels.has(s) && !self.levels.has(1) {
                // f32 RGBA from the decoder, then the half-float raster.
                m += whole::level_bytes(w, h, 1, 16 + 8);
            }
            return m;
        }
        let index = if self.index.lock().is_some() {
            0
        } else {
            u64::from(h) * 8
        };
        let out = self.out_rect(s, req.region);
        index + self.grid(s).memory(out, req.parallel)
    }
    fn memory_bytes(&self) -> usize {
        let index = self
            .index
            .lock()
            .as_ref()
            .and_then(|i| i.as_ref().as_ref().map(|i| i.memory_bytes()))
            .unwrap_or(0);
        index + self.levels.memory_bytes()
    }
    /// Drops the row index (and the cached levels of x-major files).
    fn trim_memory(&self) -> usize {
        let index = self
            .index
            .lock()
            .take()
            .and_then(|i| i.as_ref().as_ref().map(|i| i.memory_bytes()))
            .unwrap_or(0);
        index + self.levels.clear()
    }
}

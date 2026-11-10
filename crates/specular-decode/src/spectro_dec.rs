//! Spectroscopy camera files through the science view (`specular-micro`): Princeton
//! Instruments SPE (2.x and LightField 3.0) and Andor SIF.
//!
//! Each frame (and, in SPE 3.0, each region of interest of a frame) is a page. A page is a
//! real-valued plane whose rows are spectra: the spectrum under the cursor is its row,
//! plotted against the wavelength calibration of its columns (else column numbers). The
//! exposure, detector temperature and gain are facts; the default look is the auto-stretched
//! plane.

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_micro::spectral::{Page, Spectral};
use specular_micro::{sif, spe};

use crate::science::{
    Linear, NdPlane, PixelSpectra, PlaneCache, ScienceKind, ScienceSummary, Spectrum,
    SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

fn err(e: specular_micro::Error) -> DecodeError {
    use specular_micro::Error as E;
    match e {
        E::NotLif | E::NotNd2 => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s.into()),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        E::NoImage => DecodeError::Unsupported("no frames in the file".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Spe,
    Sif,
}

pub struct SpectroDecoder(pub Kind);

impl Decoder for SpectroDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Spe => Format::Spe,
            Kind::Sif => Format::Sif,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Sif => sif::sniff(head),
            // The header's fields against the file's size: see `probe_file`.
            Kind::Spe => false,
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Sif => sif::sniff(data),
            Kind::Spe => spe::sniff(data),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Spe => &["spe"],
            Kind::Sif => &["sif"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let s = match self.0 {
            Kind::Spe => spe::parse(source.bytes()),
            Kind::Sif => sif::parse(source.bytes()),
        }
        .map_err(err)?;
        let regions = s.pages.iter().map(|p| p.region).max().unwrap_or(0) + 1;
        let file = Arc::new(File {
            spectral: s,
            format: self.format(),
            regions,
        });
        Ok(Arc::new(SpectroSession::new(source, file, 0)?))
    }
}

struct File {
    spectral: Spectral,
    format: Format,
    regions: usize,
}

/// A page's rows as spectra.
#[derive(Debug)]
struct Rows {
    source: Arc<Source>,
    page: Page,
    values: Mutex<Option<Arc<Vec<f32>>>>,
    /// The memory budget's share of `values`.
    held: Mutex<Option<specular_core::Reservation>>,
}

impl Rows {
    fn values(&self) -> Option<Arc<Vec<f32>>> {
        let mut g = self.values.lock();
        if g.is_none() {
            *g = self
                .page
                .read_reserved(self.source.bytes())
                .ok()
                .map(|(v, _, held)| {
                    *self.held.lock() = Some(held);
                    Arc::new(v)
                });
        }
        g.clone()
    }
}

impl SpectrumSource for Rows {
    fn grid(&self) -> (usize, usize) {
        (1, self.page.height)
    }
    fn channels(&self) -> usize {
        self.page.width
    }
    fn x_axis(&self) -> (String, String) {
        if self.page.wavelengths.is_some() {
            ("Wavelength".into(), "nm".into())
        } else {
            ("Column".into(), String::new())
        }
    }
    fn at(&self, _x: usize, y: usize, _index: &[u32]) -> Option<Spectrum> {
        let v = self.values()?;
        let w = self.page.width;
        let row = v.get(y * w..(y + 1) * w)?.to_vec();
        Some(Spectrum {
            y: row,
            x: match &self.page.wavelengths {
                Some(wl) => SpectrumX::Values(wl.clone()),
                None => SpectrumX::Linear(Linear {
                    offset: 0.0,
                    step: 1.0,
                }),
            },
            sticks: false,
        })
    }
}

struct SpectroSession {
    source: Arc<Source>,
    file: Arc<File>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

fn kw(k: impl Into<String>, v: impl ToString) -> (String, String, String) {
    (k.into(), v.to_string(), String::new())
}

impl SpectroSession {
    fn new(source: Arc<Source>, file: Arc<File>, page: u32) -> Result<SpectroSession> {
        let s = &file.spectral;
        let p = s
            .pages
            .get(page as usize)
            .ok_or_else(|| DecodeError::Malformed("no such frame".into()))?
            .clone();
        let (w, h) = (
            u32::try_from(p.width).map_err(|_| DecodeError::TooLarge)?,
            u32::try_from(p.height).map_err(|_| DecodeError::TooLarge)?,
        );
        if w == 0 || h == 0 {
            return Err(DecodeError::Malformed("empty frame".into()));
        }
        if u64::from(w) * u64::from(h) > specular_micro::MAX_PLANE {
            return Err(DecodeError::TooLarge);
        }
        let mut keywords = vec![kw("Format", &s.version), kw("Frames", s.frames)];
        if file.regions > 1 {
            keywords.push(kw("Regions", file.regions));
            keywords.push(kw("Region", p.region + 1));
        }
        keywords.push(kw("Frame", p.frame + 1));
        if let Some([x, y, bx, by]) = p.sensor {
            keywords.push(kw("Sensor position", format!("{x}, {y}")));
            if bx > 1 || by > 1 {
                keywords.push(kw("Binning", format!("{bx}×{by}")));
            }
        }
        if let Some(t) = s.temperature {
            keywords.push(kw("Detector temperature (°C)", t));
        }
        if let Some(g) = &s.gain {
            keywords.push(kw("Gain", g));
        }
        if let Some(sp) = &s.spectrograph {
            keywords.push(kw("Spectrograph", sp));
        }
        if let Some(wl) = &p.wavelengths
            && let (Some(a), Some(b)) = (wl.first(), wl.last())
        {
            keywords.push(kw("Wavelengths (nm)", format!("{a:.3}–{b:.3}")));
        }
        keywords.extend(s.facts.iter().map(|(k, v)| kw(k.clone(), v)));
        let rows = Arc::new(Rows {
            source: source.clone(),
            page: p.clone(),
            values: Mutex::new(None),
            held: Mutex::new(None),
        });
        let summary = ScienceSummary {
            width: w,
            height: h,
            sample: p.sample.name().to_owned(),
            instrument: s.detector.clone(),
            detector: s.detector.clone(),
            date_obs: s.date.clone(),
            exposure: s.exposure,
            unit: Some("counts".into()),
            keywords: Arc::new(keywords),
            kind: ScienceKind::Spectroscopy,
            spectra: Some(PixelSpectra::new(rows, w, h)),
            vendor: Some(
                match file.format {
                    Format::Spe => "Princeton Instruments",
                    _ => "Andor",
                }
                .into(),
            ),
            ..Default::default()
        };
        let info = ImageInfo {
            format: file.format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: (p.sample.bytes() * 8).min(64) as u8,
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(SpectroSession {
            source,
            file,
            page,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn page(&self) -> &Page {
        &self.file.spectral.pages[self.page as usize]
    }

    fn plane(&self, cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(&[], cancel, || {
            let (values, truncated) = self.page().read(self.source.bytes()).map_err(err)?;
            Ok(NdPlane {
                width: self.info.width,
                height: self.info.height,
                values,
                truncated,
            })
        })
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let p = self.plane(cancel)?;
        let (lo, hi) = specular_core::nd::Limits::ZScale
            .compute(&p.values, p.width as usize)
            .filter(|(lo, hi)| hi > lo)
            .unwrap_or((0.0, 1.0));
        let grey: Vec<u8> = p
            .values
            .iter()
            .map(|&x| {
                if !x.is_finite() {
                    0
                } else {
                    (((x - lo) / (hi - lo)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                }
            })
            .collect();
        Ok(whole::interleaved(
            p.width,
            p.height,
            PixelLayout::Gray,
            grey,
            p.truncated,
        ))
    }
}

impl Session for SpectroSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * 8 + whole::level_bytes(self.info.width, self.info.height, req.scale, 1)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.file.spectral.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        // Regions of one frame differ in size: the page shows it.
        (self.file.regions > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.page_count() {
            return Err(DecodeError::Unsupported("no such frame".into()));
        }
        Ok(Arc::new(SpectroSession::new(
            self.source.clone(),
            self.file.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        if !index.is_empty() {
            return Some(Err(DecodeError::Malformed("no such plane".into())));
        }
        Some(self.plane(cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WinSpec 2.x file of two 4×2 uint16 frames with a linear calibration.
    fn spe2() -> Vec<u8> {
        let mut b = vec![0u8; spe::DATA_START];
        b[42..44].copy_from_slice(&4u16.to_le_bytes());
        b[656..658].copy_from_slice(&2u16.to_le_bytes());
        b[108..110].copy_from_slice(&3i16.to_le_bytes());
        b[1446..1450].copy_from_slice(&2i32.to_le_bytes());
        b[36..40].copy_from_slice(&(-70f32).to_le_bytes());
        b[3098] = 1;
        b[3101] = 1;
        b[3263..3271].copy_from_slice(&400f64.to_le_bytes());
        b[3271..3279].copy_from_slice(&1.5f64.to_le_bytes());
        for i in 0..16u16 {
            b.extend_from_slice(&i.to_le_bytes());
        }
        b
    }

    #[test]
    fn frames_are_pages_with_row_spectra() {
        let s = crate::Registry::new()
            .open(Arc::new(Source::from_vec("a.spe", spe2())))
            .unwrap();
        assert_eq!(s.info().format, Format::Spe);
        assert_eq!(s.page_count(), 2);
        let sci = s.science().unwrap();
        assert_eq!(sci.kind, ScienceKind::Spectroscopy);
        assert!(
            sci.keywords
                .iter()
                .any(|k| k.0 == "Detector temperature (°C)" && k.1 == "-70")
        );
        let sp = sci.spectra.as_ref().unwrap().at(0, 1, &[]).unwrap();
        assert_eq!(sp.y, [4.0, 5.0, 6.0, 7.0]);
        assert_eq!(sp.x_at(0), 401.5);
        let p2 = s.open_page(1).unwrap();
        let pl = p2.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
        assert_eq!(pl.values[0], 8.0);
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
    }
}

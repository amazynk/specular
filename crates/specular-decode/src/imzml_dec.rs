//! Imaging mass spectrometry through the N-D science path: imzML 1.1 data sets (the
//! `.imzML` XML opened, its `.ibd` binary found beside it by name), read by
//! `specular-imzml`.
//!
//! The image is the total ion current (each pixel's summed intensities) through viridis;
//! [`MassSpec::ion_plane`] gives the ion image of an m/z window instead, which the viewer
//! asks for when an m/z is typed. Every pixel's mass spectrum is plotted under the cursor
//! ([`crate::science::PixelSpectra`]). A data set with several z positions gets a "z"
//! axis. The `.ibd` is memory-mapped: spectra are read only when an image or a pixel asks.

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::nd::Limits;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_imzml::{ImzMl, Mode, Polarity};

pub use specular_imzml::{Tolerance as IonTolerance, Window as IonWindow};

use crate::science::{
    AxisKind, ColourTable, Linear, NdAxis, NdPlane, PixelCalibration, PixelSpectra, PlaneCache,
    ScienceKind, ScienceSummary, Spectrum, SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

/// Bytes of the XML looked at for the mzML root and the imaging vocabulary.
const SNIFF: usize = 16 << 10;

pub struct ImzmlDecoder;

/// Whether the start of a file is an imzML document: an mzML root and the imaging
/// ("IMS") controlled vocabulary.
fn sniff(head: &[u8]) -> bool {
    let has = |n: &[u8]| head.windows(n.len()).any(|w| w == n);
    (has(b"<mzML") || has(b"<indexedmzML")) && (has(b"\"IMS\"") || has(b"IMS:1000"))
}

impl Decoder for ImzmlDecoder {
    fn format(&self) -> Format {
        Format::Imzml
    }
    fn probe(&self, _head: &[u8]) -> bool {
        // The root element comes after the XML declaration and comments: see `probe_file`.
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        sniff(&data[..data.len().min(SNIFF)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["imzml"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(ImzmlSession::open(source)?))
    }
}

pub(crate) fn imzml_err(e: specular_imzml::Error) -> DecodeError {
    use specular_imzml::Error as E;
    match e {
        E::NotImzml => DecodeError::UnsupportedFormat,
        E::Malformed(s) => DecodeError::Malformed(s),
        E::TooLarge(_) => DecodeError::TooLarge,
        e => DecodeError::Unsupported(e.to_string()),
    }
}

/// A mass spectrometry image's spectra and images: the parsed XML and the mapped `.ibd`.
pub struct MassSpec {
    imzml: ImzMl,
    ibd: Arc<Source>,
    /// The m/z range of all spectra (declared, else from the arrays' ends).
    range: Option<(f64, f64)>,
    /// Each spectrum's total ion current, once computed.
    tic: Mutex<Option<Arc<Vec<f32>>>>,
    planes: PlaneCache,
}

impl std::fmt::Debug for MassSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MassSpec")
            .field("spectra", &self.imzml.entries.len())
            .field("width", &self.imzml.width)
            .field("height", &self.imzml.height)
            .field("range", &self.range)
            .finish()
    }
}

/// Plane cache keys: the TIC of plane `z`, or the ion image of a window.
fn tic_key(zi: usize) -> Vec<u32> {
    vec![0, zi as u32]
}

fn ion_key(w: IonWindow, zi: usize) -> Vec<u32> {
    let (kind, tol) = match w.tolerance {
        IonTolerance::Ppm(p) => (1, p),
        IonTolerance::Da(d) => (2, d),
    };
    let split = |v: f64| {
        let b = v.to_bits();
        [(b >> 32) as u32, b as u32]
    };
    let mut k = vec![kind, zi as u32];
    k.extend(split(w.mz));
    k.extend(split(tol));
    k
}

impl MassSpec {
    /// The m/z range of all spectra.
    pub fn mz_range(&self) -> Option<(f64, f64)> {
        self.range
    }

    /// The plane index's z position (0 without a z axis).
    fn zi(&self, index: &[u32]) -> usize {
        if self.imzml.depth() > 1 {
            index.first().copied().unwrap_or(0) as usize
        } else {
            0
        }
    }

    /// The pixel's position as the file numbers it (1-based x and y, and z).
    pub fn position(&self, x: u32, y: u32, index: &[u32]) -> (u32, u32, u32) {
        let z = self.imzml.zs.get(self.zi(index)).copied().unwrap_or(1);
        (self.imzml.origin.0 + x, self.imzml.origin.1 + y, z)
    }

    /// Whether the pixel has a spectrum.
    pub fn has_spectrum(&self, x: u32, y: u32, index: &[u32]) -> bool {
        self.imzml.index_at(x, y, self.zi(index)).is_some()
    }

    /// The m/z of the most intense value of the pixel's spectrum (its base peak).
    pub fn base_peak(&self, x: u32, y: u32, index: &[u32]) -> Option<f64> {
        let i = self.imzml.index_at(x, y, self.zi(index))?;
        let s = self.imzml.spectrum(self.ibd.bytes(), i);
        let k = s
            .intensity
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_finite())
            .max_by(|a, b| a.1.total_cmp(b.1))?
            .0;
        Some(s.mz[k])
    }

    fn plane(&self, values: Vec<f32>) -> NdPlane {
        NdPlane {
            width: self.imzml.width,
            height: self.imzml.height,
            values,
            truncated: false,
        }
    }

    /// The total ion current image of the plane at `index`.
    pub fn tic_plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let zi = self.zi(index);
        self.planes.get_or(&tic_key(zi), cancel, || {
            let tic = self.tic.lock().clone();
            let tic = match tic {
                Some(t) => t,
                None => {
                    let t = self
                        .imzml
                        .tic(self.ibd.bytes(), &|| cancel.is_cancelled())
                        .ok_or(DecodeError::Cancelled)?;
                    let t = Arc::new(t);
                    *self.tic.lock() = Some(t.clone());
                    t
                }
            };
            Ok(self.plane(self.imzml.image(&tic, zi)))
        })
    }

    /// The ion image of `window` (each pixel's intensities summed over the m/z window, both
    /// ends included) for the plane at `index`; NaN where a pixel has no spectrum.
    pub fn ion_plane(
        &self,
        window: IonWindow,
        index: &[u32],
        cancel: &CancelToken,
    ) -> Result<Arc<NdPlane>> {
        let zi = self.zi(index);
        if !(window.mz.is_finite() && window.half_width().is_finite()) {
            return Err(DecodeError::Malformed("m/z window".into()));
        }
        self.planes.get_or(&ion_key(window, zi), cancel, || {
            let per = self
                .imzml
                .ion(self.ibd.bytes(), window, &|| cancel.is_cancelled())
                .ok_or(DecodeError::Cancelled)?;
            Ok(self.plane(self.imzml.image(&per, zi)))
        })
    }
}

impl SpectrumSource for MassSpec {
    fn grid(&self) -> (usize, usize) {
        (self.imzml.width as usize, self.imzml.height as usize)
    }
    fn channels(&self) -> usize {
        if self.imzml.shared_mz {
            self.imzml.entries[0].mz.len as usize
        } else {
            0
        }
    }
    fn x_axis(&self) -> (String, String) {
        ("m/z".into(), String::new())
    }
    fn at(&self, x: usize, y: usize, index: &[u32]) -> Option<Spectrum> {
        let i = self.imzml.index_at(x as u32, y as u32, self.zi(index))?;
        let s = self.imzml.spectrum(self.ibd.bytes(), i);
        Some(Spectrum {
            y: s.intensity,
            x: SpectrumX::Values(s.mz.into()),
            sticks: self.imzml.centroid == Some(true),
        })
    }
}

/// Viridis as a colour table, the usual look of mass spectrometry images.
pub(crate) fn viridis_lut() -> Arc<ColourTable> {
    let t = specular_inspect::ColourMap::Viridis.table();
    let mut out = [[0u8; 3]; 256];
    for (o, c) in out.iter_mut().zip(t.iter()) {
        *o = c.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    Arc::new(out)
}

struct ImzmlSession {
    source: Arc<Source>,
    ms: Arc<MassSpec>,
    info: ImageInfo,
    summary: ScienceSummary,
    levels: Levels,
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

/// A number with at most `d` decimals, without trailing zeros.
fn num(v: f64, d: usize) -> String {
    let s = format!("{v:.d$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    }
}

impl ImzmlSession {
    fn open(source: Arc<Source>) -> Result<ImzmlSession> {
        let imzml = ImzMl::parse(source.bytes()).map_err(imzml_err)?;
        let stem = source
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| DecodeError::Unsupported("no .ibd file beside the .imzML".into()))?
            .to_owned();
        let ibd_path = specular_gis::sidecar::find_name(source.path(), &format!("{stem}.ibd"))
            .ok_or_else(|| {
                DecodeError::Unsupported(format!("the binary file {stem}.ibd is missing"))
            })?;
        let ibd = Arc::new(Source::open(&ibd_path)?);
        imzml.check_ibd(ibd.bytes()).map_err(imzml_err)?;
        ibd.advise_random();
        let range = imzml.mz_range(ibd.bytes());
        let (w, h) = (imzml.width, imzml.height);
        let mut axes = Vec::new();
        if imzml.depth() > 1 {
            axes.push(NdAxis {
                name: "z".into(),
                len: imzml.depth() as u32,
                unit: String::new(),
                labels: imzml.zs.iter().map(|z| z.to_string()).collect(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Cube,
            });
        }
        let first = &imzml.entries[0];
        let sample = format!(
            "{}, m/z {}, intensity {}",
            imzml.mode.name(),
            first.mz.dtype.name(),
            first.intensity.dtype.name()
        );
        let pixel = imzml.pixel_size.map(|(sx, sy)| PixelCalibration {
            x: Linear {
                offset: f64::from(imzml.origin.0.saturating_sub(1)) * sx,
                step: sx,
            },
            y: Linear {
                offset: f64::from(imzml.origin.1.saturating_sub(1)) * sy,
                step: sy,
            },
            unit_x: "µm".into(),
            unit_y: "µm".into(),
        });
        let keywords = keywords(&imzml, range);
        let ms = Arc::new(MassSpec {
            imzml,
            ibd,
            range,
            tic: Mutex::new(None),
            planes: PlaneCache::new(),
        });
        let summary = ScienceSummary {
            width: w,
            height: h,
            start: vec![0; axes.len()],
            axes,
            sample: sample.clone(),
            title: Some(stem),
            kind: ScienceKind::MassSpec,
            luts: vec![viridis_lut()],
            pixel,
            instrument: ms.imzml.instrument.clone(),
            spectra: Some(PixelSpectra::new(ms.clone(), w, h)),
            mass_spec: Some(ms.clone()),
            keywords: Arc::new(keywords),
            ..Default::default()
        };
        let info = ImageInfo {
            format: Format::Imzml,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 32,
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(ImzmlSession {
            source,
            ms,
            info,
            summary,
            levels: Levels::new(),
        })
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let p = self.ms.tic_plane(&self.summary.start, cancel)?;
        Ok(lut_raster(&p, &viridis_lut()))
    }
}

fn keywords(m: &ImzMl, range: Option<(f64, f64)>) -> Vec<(String, String, String)> {
    let mut k = Vec::new();
    k.push(kw(
        "Mode",
        if m.mode_declared {
            m.mode.name().to_owned()
        } else {
            format!("{} (not declared)", m.mode.name())
        },
    ));
    k.push(kw("Spectra", m.entries.len().to_string()));
    let mut grid = format!("{} × {}", m.width, m.height);
    if m.depth() > 1 {
        grid += &format!(" × {} z", m.depth());
    }
    k.push(kw("Pixels", grid));
    if let Some((lo, hi)) = range {
        k.push(kw("m/z range", format!("{} … {}", num(lo, 4), num(hi, 4))));
    }
    if m.mode == Mode::Continuous && !m.entries.is_empty() {
        k.push(kw("m/z values", m.entries[0].mz.len.to_string()));
    }
    if let Some((x, y)) = m.pixel_size {
        k.push(kw(
            "Pixel size",
            if x == y {
                format!("{} µm", num(x, 3))
            } else {
                format!("{} × {} µm", num(x, 3), num(y, 3))
            },
        ));
    }
    if let Some(i) = &m.instrument {
        k.push(kw("Instrument", i.clone()));
    }
    if let Some(p) = m.polarity {
        k.push(kw(
            "Polarity",
            match p {
                Polarity::Positive => "positive",
                Polarity::Negative => "negative",
                Polarity::Mixed => "mixed",
            }
            .into(),
        ));
    }
    if let Some(c) = m.centroid {
        k.push(kw(
            "Spectrum type",
            if c { "centroid" } else { "profile" }.into(),
        ));
    }
    if let Some(e) = m.entries.first() {
        k.push(kw("m/z array", e.mz.dtype.name().into()));
        k.push(kw("Intensity array", e.intensity.dtype.name().into()));
    }
    if let Some(s) = &m.software {
        k.push(kw("Software", s.clone()));
    }
    if let Some(u) = &m.uuid_text {
        k.push(kw("UUID", u.clone()));
    }
    k
}

/// A plane as RGBA through `lut` at zscale limits; pixels without a spectrum transparent.
pub(crate) fn lut_raster(p: &NdPlane, lut: &ColourTable) -> Raster {
    let (lo, hi) = Limits::ZScale
        .compute(&p.values, p.width as usize)
        .filter(|(l, h)| h > l)
        .unwrap_or((0.0, 1.0));
    let scale = 255.0 / (hi - lo);
    let mut rgba = Vec::with_capacity(p.values.len() * 4);
    for v in &p.values {
        if v.is_finite() {
            let i = ((v - lo) * scale).round().clamp(0.0, 255.0) as usize;
            let c = lut[i];
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        } else {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

impl Session for ImzmlSession {
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
        let spectra = self.ms.imzml.entries.len() as u64;
        px * 8 + spectra * 4 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.ms.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.ms.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.ms.tic_plane(index, cancel))
    }
}

#[cfg(test)]
mod tests;

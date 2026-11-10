//! CASA images (the `.image`, `.residual`, `.model`, `.pb`, `.mask` directories tclean
//! writes) through `specular-casa`: the image plane is the first two axes, every further axis
//! (Stokes, frequency) a cube slider, and a complex image's parts one more. Rows are shown
//! bottom-up as in FITS. Pixels outside the default mask are NaN. The coordinates become
//! FITS cards for the exact WCS; frequency positions are labelled with their velocity, Stokes
//! positions with their names. The image opens from its directory or from `table.dat` /
//! `table.f0` inside it.

use std::sync::Arc;

use specular_casa::{self as casa, AxisKind as CasaAxis, CasaImage, PlaneData};
use specular_core::{CancelToken, Orientation, Raster, Source};

use crate::science::{
    self, AxisKind, CasaFacts, NdAxis, NdPlane, PixelSpectra, PlaneCache, ScienceSummary, Spectrum,
    SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

pub struct CasaDecoder;

/// Table keywords listed at most.
const MAX_KEYWORDS: usize = 4000;

/// The parts of a complex pixel, in the order of the extra axis.
const PARTS: [&str; 4] = ["Amplitude", "Phase", "Real", "Imaginary"];

fn map_err(e: casa::Error) -> DecodeError {
    match e {
        casa::Error::NotCasa => DecodeError::UnsupportedFormat,
        casa::Error::Malformed(s) => DecodeError::Malformed(s),
        casa::Error::Unsupported(s) => DecodeError::Unsupported(s),
        casa::Error::Truncated => DecodeError::Truncated("CASA tile data".into()),
        casa::Error::TooLarge => DecodeError::TooLarge,
        casa::Error::Io(s) => DecodeError::Io(std::io::Error::other(s)),
        casa::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// The type string of a top-level AipsIO object: what a `table.f0` holds.
fn aipsio_type(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 12 || data[..4] != [0xbe; 4] {
        return None;
    }
    let n = u32::from_be_bytes(data[8..12].try_into().ok()?) as usize;
    data.get(12..12 + n)
}

impl Decoder for CasaDecoder {
    fn format(&self) -> Format {
        Format::Casa
    }
    fn probe(&self, _head: &[u8]) -> bool {
        // The `coords` keyword lies past the first bytes.
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        // An image's `table.dat`, or the header of its tiled storage manager (`table.f0`; the
        // folder decides when it opens).
        casa::sniff(&data[..data.len().min(1 << 20)])
            || matches!(
                aipsio_type(data),
                Some(b"TiledCellStMan" | b"TiledShapeStMan")
            )
    }
    fn extensions(&self) -> &'static [&'static str] {
        // A folder, or the files in it (recognised by content).
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let image = CasaImage::open(source.path()).map_err(map_err)?;
        let mut keywords: Vec<(String, String, String)> = image
            .fits_cards()
            .into_iter()
            .map(|(k, v)| (k, v, String::new()))
            .collect();
        keywords.extend(
            image
                .keywords
                .flatten(MAX_KEYWORDS)
                .into_iter()
                .map(|(k, v)| (k, v, "CASA".to_owned())),
        );
        let shared = Arc::new(Shared {
            keywords: Arc::new(keywords),
            image,
        });
        Ok(Arc::new(CasaSession::new(source, shared)?))
    }
}

struct Shared {
    image: CasaImage,
    keywords: Arc<Vec<(String, String, String)>>,
}

struct CasaSession {
    source: Arc<Source>,
    shared: Arc<Shared>,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

/// `x` with enough decimals to tell values `step` apart (at least `min`, at most 9).
fn decimals(step: f64, min: usize) -> usize {
    if !(step.is_finite() && step > 0.0) {
        return min.max(6);
    }
    let d = (-step.log10()).ceil() + 1.0;
    (d.max(min as f64).min(9.0)) as usize
}

/// A frequency in Hz in the unit its size suits, with the decimals `step` (Hz) needs.
fn frequency_text(hz: f64, step: f64) -> String {
    let (div, unit) = match hz.abs() {
        a if a >= 1e9 => (1e9, "GHz"),
        a if a >= 1e6 => (1e6, "MHz"),
        a if a >= 1e3 => (1e3, "kHz"),
        _ => (1.0, "Hz"),
    };
    format!("{:.*} {unit}", decimals(step / div, 3), hz / div)
}

/// Frequencies (Hz) of the positions of spectral axis `k`.
fn frequencies(image: &CasaImage, wcs: Option<&specular_fits::wcs::Wcs>, k: usize) -> Vec<f64> {
    let axis = &image.coords.axes[k];
    (0..image.shape[k])
        .map(|i| {
            axis.table_value(i as f64)
                .or_else(|| wcs?.axis_value(k, i))
                .unwrap_or(f64::NAN)
        })
        .collect()
}

/// Labels of a spectral axis: frequency and, with a rest frequency, velocity.
fn spectral_labels(image: &CasaImage, hz: &[f64]) -> Vec<String> {
    let step = |v: &[f64]| {
        v.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .filter(|d| *d > 0.0)
            .fold(f64::INFINITY, f64::min)
    };
    let sp = image.coords.spectral.as_ref();
    let vel: Option<Vec<f64>> = sp.and_then(|s| hz.iter().map(|&f| s.velocity(f)).collect());
    let fstep = step(hz);
    let vstep = vel.as_deref().map(step).unwrap_or(f64::INFINITY);
    let unit = sp.map_or("km/s", |s| {
        if s.velocity_unit.trim().is_empty() {
            "km/s"
        } else {
            s.velocity_unit.trim()
        }
    });
    hz.iter()
        .enumerate()
        .map(|(i, &f)| {
            if !f.is_finite() {
                return String::new();
            }
            let mut l = frequency_text(f, fstep);
            if let Some(v) = vel.as_ref().map(|v| v[i]) {
                l += &format!(" · {:.*} {unit}", decimals(vstep, 1).min(6), v);
            }
            l
        })
        .collect()
}

impl CasaSession {
    fn new(source: Arc<Source>, shared: Arc<Shared>) -> Result<CasaSession> {
        let im = &shared.image;
        let width = u32::try_from(im.width()).map_err(|_| DecodeError::TooLarge)?;
        let height = u32::try_from(im.height()).map_err(|_| DecodeError::TooLarge)?;
        let cards = im.fits_cards();
        let header = specular_fits::Header::from_pairs(
            cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")),
        );
        let wcs = specular_fits::wcs::Wcs::from_header(&header, &im.shape).map(Arc::new);
        let mut axes = Vec::new();
        let mut spectra = None;
        for k in 2..im.shape.len() {
            let a = &im.coords.axes[k];
            let len = u32::try_from(im.shape[k]).map_err(|_| DecodeError::TooLarge)?;
            let mut axis = NdAxis {
                name: a.name.clone(),
                len,
                unit: a.unit.clone(),
                labels: Vec::new(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Cube,
            };
            match a.kind {
                CasaAxis::Stokes => {
                    axis.labels = a.stokes.clone();
                    axis.labels.resize(im.shape[k], String::new());
                }
                CasaAxis::Spectral => {
                    let hz = frequencies(im, wcs.as_deref(), k);
                    axis.labels = spectral_labels(im, &hz);
                    axis.unit = "Hz".into();
                    if im.shape[k] > 1 && hz.iter().all(|f| f.is_finite()) && spectra.is_none() {
                        spectra = Some(PixelSpectra::new(
                            Arc::new(CasaSpectra {
                                shared: shared.clone(),
                                axis: k,
                                x: hz.iter().map(|f| f / 1e9).collect(),
                            }),
                            width,
                            height,
                        ));
                    }
                }
                CasaAxis::Tabular => {
                    if a.table.is_some() {
                        axis.labels = (0..im.shape[k])
                            .map(|i| {
                                let v = a.table_value(i as f64).unwrap_or(f64::NAN);
                                format!("{v} {}", a.unit).trim().to_owned()
                            })
                            .collect();
                    }
                }
                _ => {
                    axis.wcs_axis = wcs
                        .as_ref()
                        .is_some_and(|w| w.axis_value(k, 0).is_some())
                        .then_some(k);
                }
            }
            axes.push(axis);
        }
        if im.pixel.is_complex() {
            axes.push(NdAxis {
                name: "Complex".into(),
                len: PARTS.len() as u32,
                unit: String::new(),
                labels: PARTS.map(String::from).to_vec(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
        }
        let mut sample = match im.pixel {
            casa::Pixel::Float => "32-bit float".to_owned(),
            casa::Pixel::Double => "64-bit float".to_owned(),
            casa::Pixel::Complex => "complex, 2 × 32-bit float".to_owned(),
            casa::Pixel::DComplex => "complex, 2 × 64-bit float".to_owned(),
        };
        if im.big_endian {
            sample += ", big-endian";
        }
        let mut summary = ScienceSummary {
            width,
            height,
            axes,
            wcs,
            sample,
            spectra,
            ..Default::default()
        };
        summary.describe(&header, None);
        summary.keywords = shared.keywords.clone();
        summary.observer = im.coords.observer.clone();
        summary.casa = Some(facts(im));
        let info = ImageInfo {
            format: Format::Casa,
            width,
            height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: im.pixel.bits().min(64),
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(CasaSession {
            source,
            shared,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let im = &self.shared.image;
        if index.len() != self.summary.axes.len()
            || index
                .iter()
                .zip(&self.summary.axes)
                .any(|(&i, ax)| i >= ax.len)
        {
            return Err(DecodeError::Malformed("no such plane".into()));
        }
        let cube = im.shape.len().saturating_sub(2);
        let at: Vec<usize> = index[..cube].iter().map(|&i| i as usize).collect();
        let part = index.get(cube).copied().unwrap_or(0);
        let values: Vec<f32> = match im.plane(&at).map_err(map_err)? {
            PlaneData::Real(v) => v,
            PlaneData::Complex(v) => v
                .iter()
                .map(|&[re, im]| match part {
                    0 => re.hypot(im),
                    1 => im.atan2(re).to_degrees(),
                    2 => re,
                    _ => im,
                })
                .collect(),
        };
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        if values.len() != w * h {
            return Err(DecodeError::Malformed("plane size".into()));
        }
        // Bottom-up as in FITS: the first row is shown at the bottom.
        let mut flipped = Vec::with_capacity(values.len());
        for row in values.chunks_exact(w.max(1)).rev() {
            flipped.extend_from_slice(row);
        }
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values: flipped,
            truncated: false,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let first = self.plane(&vec![0; self.summary.axes.len()], cancel)?;
        Ok(science::auto_raster(&[first], None))
    }
}

/// The CASA facts of an image, for its info group.
fn facts(im: &CasaImage) -> CasaFacts {
    let beams = im.plane_beams.as_ref().map(|pb| {
        let range = |f: fn(&casa::Beam) -> f64| {
            pb.beams
                .iter()
                .map(f)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
                    (lo.min(x), hi.max(x))
                })
        };
        science::PlaneBeams {
            count: pb.beams.len(),
            major: range(|b| b.major),
            minor: range(|b| b.minor),
        }
    });
    let sp = im.coords.spectral.as_ref();
    CasaFacts {
        image_type: im.image_type.name(),
        info_type: im.info_type.clone(),
        beam: im.beam.map(|b| [b.major, b.minor, b.pa]),
        plane_beams: beams,
        rest_frequency: sp.map(|s| s.rest).filter(|r| *r > 0.0),
        spectral_frame: sp
            .map(|s| s.system.trim().to_owned())
            .filter(|s| !s.is_empty()),
        velocity: sp.filter(|s| s.rest > 0.0).map(|s| s.velocity_name()),
        mask: im.mask_name.clone(),
        storage: im.storage,
        big_endian: im.big_endian,
    }
}

/// The spectrum along a CASA cube's spectral axis at a pixel.
struct CasaSpectra {
    shared: Arc<Shared>,
    axis: usize,
    /// GHz per channel.
    x: Arc<[f64]>,
}

impl std::fmt::Debug for CasaSpectra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CasaSpectra")
            .field("axis", &self.axis)
            .field("channels", &self.x.len())
            .finish()
    }
}

impl SpectrumSource for CasaSpectra {
    fn grid(&self) -> (usize, usize) {
        let im = &self.shared.image;
        (im.width(), im.height())
    }
    fn channels(&self) -> usize {
        self.x.len()
    }
    fn x_axis(&self) -> (String, String) {
        ("Frequency".into(), "GHz".into())
    }
    fn at(&self, x: usize, y: usize, index: &[u32]) -> Option<Spectrum> {
        let im = &self.shared.image;
        let nd = im.shape.len();
        let mut at = vec![0usize; nd];
        at[0] = x;
        if nd > 1 {
            // Displayed rows are bottom-up.
            at[1] = im.height().checked_sub(1 + y)?;
        }
        for (k, slot) in at.iter_mut().enumerate().skip(2) {
            *slot = index.get(k - 2).map_or(0, |&i| i as usize);
        }
        let y = im.line(&at, self.axis).ok()?;
        Some(Spectrum {
            y,
            x: SpectrumX::Values(self.x.clone()),
            sticks: false,
        })
    }
}

impl Session for CasaSession {
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
        px * 16 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
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
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-casa/tests/data")
            .join(name)
    }

    fn registry() -> crate::Registry {
        crate::Registry::default()
    }

    #[test]
    fn folders_and_their_files_open() {
        let reg = registry();
        let dir = fixture("cube_le.image");
        assert!(specular_core::is_image_folder(&dir));
        assert!(reg.supports_file(&dir));
        let s = reg.open(Arc::new(Source::open(&dir).unwrap())).unwrap();
        assert_eq!(s.info().format, Format::Casa);
        assert_eq!((s.info().width, s.info().height), (13, 11));
        for f in ["table.dat", "table.f0"] {
            let p = dir.join(f);
            let s2 = reg.open(Arc::new(Source::open(&p).unwrap())).unwrap();
            assert_eq!(s2.info().format, Format::Casa, "{f}");
        }
        assert!(reg.supports_file(&dir.join("table.dat")));
        // The mask subtable is a table, not an image.
        assert!(!specular_core::is_image_folder(&dir.join("mask0")));
        assert!(!reg.supports_file(&dir.join("mask0")));
        assert!(!reg.supports_file(&dir.join("table.f0")));
    }

    #[test]
    fn cube_axes_wcs_and_facts() {
        let s = CasaDecoder
            .open(Arc::new(Source::open(fixture("cube_le.image")).unwrap()))
            .unwrap();
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 2);
        assert_eq!(sc.axes[0].labels, ["I", "V"]);
        let f = &sc.axes[1].labels;
        assert_eq!(f.len(), 5);
        assert_eq!(f[0], "230.5010 GHz · 48.1 km/s");
        assert!(sc.wcs.is_some() && !sc.wcs_approximate);
        assert_eq!(sc.unit.as_deref(), Some("Jy/beam"));
        assert_eq!(sc.object.as_deref(), Some("Orion KL"));
        assert_eq!(sc.telescope.as_deref(), Some("ALMA"));
        assert_eq!(sc.observer.as_deref(), Some("Ada Lovelace"));
        let c = sc.casa.as_ref().unwrap();
        assert_eq!(c.image_type, "image");
        assert_eq!(c.beam, Some([1.25, 0.75, -35.0]));
        assert_eq!(c.mask.as_deref(), Some("mask0"));
        assert_eq!(c.spectral_frame.as_deref(), Some("LSRK"));
        // A spectrum per pixel along the frequency axis.
        let sp = sc.spectra.as_ref().unwrap();
        assert_eq!(sp.channels(), 5);
        let v = sp.at(3, 4, &[1, 0]).unwrap();
        assert_eq!(v.y.len(), 5);
        // Masked pixels are NaN in planes, the plane is bottom-up.
        let im = CasaImage::open(&fixture("cube_le.image")).unwrap();
        let PlaneData::Real(want) = im.plane(&[1, 3]).unwrap() else {
            panic!("real")
        };
        let got = s.nd_plane(&[1, 3], &CancelToken::new()).unwrap().unwrap();
        let (w, h) = (13, 11);
        for y in 0..h {
            for x in 0..w {
                let a = got.values[y * w + x];
                let b = want[(h - 1 - y) * w + x];
                assert!(a == b || a.is_nan() && b.is_nan());
            }
        }
        assert!(got.values.iter().any(|v| v.is_nan()));
        assert!(
            s.decode(&DecodeRequest::full(1), &CancelToken::new())
                .is_ok()
        );
    }

    #[test]
    fn complex_parts_and_plane_beams() {
        let s = CasaDecoder
            .open(Arc::new(Source::open(fixture("cplx.image")).unwrap()))
            .unwrap();
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.last().unwrap().labels, PARTS);
        assert_eq!(sc.axes[0].labels, ["XX", "YY", "XY"]);
        let amp = s
            .nd_plane(&[0, 0, 0], &CancelToken::new())
            .unwrap()
            .unwrap();
        let re = s
            .nd_plane(&[0, 0, 2], &CancelToken::new())
            .unwrap()
            .unwrap();
        let imag = s
            .nd_plane(&[0, 0, 3], &CancelToken::new())
            .unwrap()
            .unwrap();
        for k in 0..amp.values.len() {
            let a = re.values[k].hypot(imag.values[k]);
            assert!(a == amp.values[k] || a.is_nan() && amp.values[k].is_nan());
        }
        let be = CasaDecoder
            .open(Arc::new(Source::open(fixture("cube_be.image")).unwrap()))
            .unwrap();
        let c = be.science().unwrap().casa.clone().unwrap();
        let pb = c.plane_beams.unwrap();
        assert_eq!(pb.count, 6);
        assert_eq!(pb.major, (1.0, 3.5));
        assert!(c.big_endian);
        // A 2-D residual has no cube axes.
        let flat = CasaDecoder
            .open(Arc::new(Source::open(fixture("flat.residual")).unwrap()))
            .unwrap();
        let sc = flat.science().unwrap();
        assert!(sc.axes.is_empty());
        assert_eq!(sc.casa.as_ref().unwrap().image_type, "residual");
    }

    #[test]
    fn not_an_image() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            CasaDecoder
                .open(Arc::new(Source::from_vec(dir.join("x"), vec![0; 64])))
                .is_err()
        );
    }
}

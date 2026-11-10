//! Materials-science images through the N-D science path, from readers that describe a file
//! as a [`Dataset`] (`specular_em::dataset`): FEI / Thermo TIA series and Bruker BCF hypermaps
//! (`specular-em`), EBSD maps (`specular-ebsd`), AFM files (`specular-spm`) and areal surface
//! topography (`specular-surf`).
//!
//! * Each page of the dataset is a page of the session; its planes lie along one axis: a
//!   channel axis named by the planes' labels (each with its own value unit), or a series
//!   axis with world values.
//! * A colour page (an EBSD inverse pole figure map) is a red / green / blue channel axis shown
//!   at the fixed 0..1 range.
//! * Per-pixel spectra (BCF, TIA spectrum images) and crystal orientations (EBSD) go to the
//!   readout through [`ScienceSummary::spectra`] and [`ScienceSummary::orientations`].
//! * Height maps (AFM, surfaces) offer plane levelling.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_em::dataset::{self as ds, Dataset, Field};

use crate::science::{
    self, AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PixelSpectra, PlaneCache,
    ScienceKind, ScienceSummary, Spectrum, SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// An EDS hypermap's or spectrum image's spectra, read from the file's bytes.
#[derive(Debug)]
struct EmSpectra {
    source: Arc<Source>,
    spectra: Arc<dyn ds::Spectra>,
}

impl EmSpectra {
    fn spectrum(&self, y: Vec<f32>) -> Spectrum {
        let a = self.spectra.axis();
        Spectrum {
            y,
            x: SpectrumX::Linear(Linear {
                offset: a.offset,
                step: a.step,
            }),
            sticks: false,
        }
    }
}

impl SpectrumSource for EmSpectra {
    fn grid(&self) -> (usize, usize) {
        self.spectra.grid()
    }
    fn channels(&self) -> usize {
        self.spectra.channels()
    }
    fn x_axis(&self) -> (String, String) {
        ("Energy".into(), self.spectra.axis().unit.trim().to_owned())
    }
    fn at(&self, x: usize, y: usize, _index: &[u32]) -> Option<Spectrum> {
        let v = self.spectra.at(self.source.bytes(), x, y)?;
        Some(self.spectrum(v))
    }
    fn sum(&self) -> Option<Spectrum> {
        self.spectra.sum().map(|v| self.spectrum(v))
    }
}

fn em_spectra(
    source: &Arc<Source>,
    spectra: Arc<dyn ds::Spectra>,
    width: u32,
    height: u32,
) -> PixelSpectra {
    PixelSpectra::new(
        Arc::new(EmSpectra {
            source: source.clone(),
            spectra,
        }),
        width,
        height,
    )
}

/// Largest `.emi` sidecar read next to a TIA `.ser`.
const EMI_MAX: u64 = 64 << 20;

/// The formats read through a [`Dataset`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tia,
    Bcf,
    Ang,
    Ctf,
    Nanoscope,
    Nanonis,
    Jpk,
    NtMdt,
    X3p,
    MetroPro,
    Datx,
    Vk4,
    Plu,
    Al3d,
    Sur,
}

pub struct MatsciDecoder(pub Kind);

fn surf_err(e: specular_surf::Error) -> DecodeError {
    use specular_surf::Error as E;
    match e {
        E::NotFormat(_) => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s.into()),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NoImage => DecodeError::Unsupported("no image in the file".into()),
    }
}

fn ebsd_err(e: specular_ebsd::Error) -> DecodeError {
    use specular_ebsd::Error as E;
    match e {
        E::NotAng | E::NotCtf => DecodeError::UnsupportedFormat,
        E::TooLarge(..) => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        E::Malformed(_) | E::NoData => DecodeError::Malformed(e.to_string()),
    }
}

/// A TIA `.ser`'s `.emi` and the index of the series' object in it: `name_2.ser` belongs to
/// `name.emi`, object 1 ([`specular_em::tia::sidecar`]); a lone `name.emi` (object 0) also
/// counts. Missing or unreadable sidecars are no error.
fn emi_sidecar(path: &std::path::Path) -> Option<(Vec<u8>, usize)> {
    let read = |p: &std::path::Path| {
        let meta = std::fs::metadata(p).ok()?;
        if !meta.is_file() || meta.len() > EMI_MAX {
            return None;
        }
        std::fs::read(p).ok()
    };
    let upper = |p: &std::path::Path| p.with_extension("EMI");
    if let Some((emi, k)) = specular_em::tia::sidecar(path)
        && let Some(b) = read(&emi).or_else(|| read(&upper(&emi)))
    {
        return Some((b, k));
    }
    let emi = path.with_extension("emi");
    read(&emi).or_else(|| read(&upper(&emi))).map(|b| (b, 0))
}

impl Kind {
    fn read(self, source: &Source) -> Result<Dataset> {
        let data = source.bytes();
        match self {
            Kind::Tia => {
                let emi = emi_sidecar(source.path());
                let (emi, k) = emi
                    .as_ref()
                    .map_or((None, 0), |(b, k)| (Some(b.as_slice()), *k));
                specular_em::tia::read_indexed(data, emi, k).map_err(crate::em_dec::em_err)
            }
            Kind::Bcf => specular_em::bcf::read(data).map_err(crate::em_dec::em_err),
            Kind::Ang => specular_ebsd::read_ang(data).map_err(ebsd_err),
            Kind::Ctf => specular_ebsd::read_ctf(data).map_err(ebsd_err),
            Kind::Nanoscope => specular_spm::nanoscope::read(data).map_err(crate::em_dec::spm_err),
            Kind::Nanonis => specular_spm::sxm::read(data).map_err(crate::em_dec::spm_err),
            Kind::Jpk => specular_spm::jpk::read(data).map_err(crate::em_dec::spm_err),
            Kind::NtMdt => specular_spm::mdt::read(data).map_err(crate::em_dec::spm_err),
            Kind::X3p => specular_surf::x3p::read(data).map_err(surf_err),
            Kind::MetroPro => specular_surf::zygo::read(data).map_err(surf_err),
            Kind::Datx => specular_surf::zygo::read_datx(data).map_err(surf_err),
            Kind::Vk4 => specular_surf::vk4::read(data).map_err(surf_err),
            Kind::Plu => specular_surf::plu::read(data).map_err(surf_err),
            Kind::Al3d => specular_surf::al3d::read(data).map_err(surf_err),
            Kind::Sur => specular_surf::sur::read(data).map_err(surf_err),
        }
    }
}

impl Decoder for MatsciDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Tia => Format::Tia,
            Kind::Bcf => Format::Bcf,
            Kind::Ang => Format::Ang,
            Kind::Ctf => Format::Ctf,
            Kind::Nanoscope => Format::Nanoscope,
            Kind::Nanonis => Format::Nanonis,
            Kind::Jpk => Format::Jpk,
            Kind::NtMdt => Format::NtMdt,
            Kind::X3p => Format::X3p,
            Kind::MetroPro => Format::MetroPro,
            Kind::Datx => Format::Datx,
            Kind::Vk4 => Format::Vk4,
            Kind::Plu => Format::Plu,
            Kind::Al3d => Format::Al3d,
            Kind::Sur => Format::Sur,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            // A TIFF, a zip and an HDF5 file: the first bytes cannot tell these from others of
            // their container, so only the whole file settles them (`probe_file`).
            Kind::Jpk | Kind::X3p | Kind::Datx => false,
            Kind::Tia => specular_em::tia::probe(head),
            Kind::Bcf => specular_em::bcf::probe(head),
            Kind::Ang => specular_ebsd::probe_ang(head),
            Kind::Ctf => specular_ebsd::probe_ctf(head),
            Kind::Nanoscope => specular_spm::nanoscope::probe(head),
            Kind::Nanonis => specular_spm::sxm::probe(head),
            Kind::NtMdt => specular_spm::mdt::probe(head),
            Kind::MetroPro => specular_surf::zygo::probe(head),
            Kind::Vk4 => specular_surf::vk4::probe(head),
            Kind::Plu => specular_surf::plu::probe(head),
            Kind::Al3d => specular_surf::al3d::probe(head),
            Kind::Sur => specular_surf::sur::probe(head),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Jpk => specular_spm::jpk::probe_file(data),
            Kind::X3p => specular_surf::x3p::probe_file(data),
            // The header check reads past the first bytes.
            Kind::Plu => specular_surf::plu::probe(data),
            Kind::Datx => specular_surf::zygo::probe_datx_file(data),
            _ => self.probe(&data[..data.len().min(crate::PROBE_HEAD)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            // `.ser` is shared with SER videos: TIA files are told apart by content.
            Kind::Tia => &["ser", "emi"],
            Kind::Bcf => &["bcf"],
            Kind::Ang => &["ang"],
            Kind::Ctf => &["ctf"],
            // Nanoscope files are numbered (`.001`, `.002`…): by content only.
            Kind::Nanoscope => &["spm"],
            Kind::Nanonis => &["sxm"],
            Kind::Jpk => &["jpk"],
            Kind::NtMdt => &["mdt"],
            Kind::X3p => &["x3p"],
            // `.dat` is everyone's: MetroPro files are told apart by content.
            Kind::MetroPro => &[],
            Kind::Datx => &["datx"],
            Kind::Vk4 => &["vk4"],
            Kind::Plu => &["plu"],
            Kind::Al3d => &["al3d"],
            Kind::Sur => &["sur"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // An `.emi` opened alone shows its first series.
        let source = if self.0 == Kind::Tia
            && !specular_em::tia::probe(source.bytes())
            && let Some(ser) = first_ser(source.path())
        {
            Arc::new(Source::open(&ser)?)
        } else {
            source
        };
        let set = self.0.read(&source)?;
        let pages: Vec<ds::Page> = set
            .pages
            .into_iter()
            .filter(|p| p.width > 0 && p.height > 0 && !p.planes.is_empty())
            .collect();
        if pages.is_empty() {
            return Err(DecodeError::Unsupported("no image in the file".into()));
        }
        let file = Arc::new(pages);
        Ok(Arc::new(MatsciSession::new(
            source,
            self.format(),
            file,
            0,
        )?))
    }
}

/// The first `.ser` of a TIA `.emi` (`name_1.ser`).
fn first_ser(emi: &std::path::Path) -> Option<std::path::PathBuf> {
    if !emi
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("emi"))
    {
        return None;
    }
    let stem = emi.file_stem()?.to_str()?;
    ["ser", "SER"]
        .iter()
        .map(|e| emi.with_file_name(format!("{stem}_1.{e}")))
        .find(|p| p.is_file())
}

struct MatsciSession {
    source: Arc<Source>,
    pages: Arc<Vec<ds::Page>>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

fn nonempty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

fn linear(l: &ds::Lateral) -> Option<Linear> {
    l.is_usable().then_some(Linear {
        offset: l.offset,
        step: l.step,
    })
}

fn kind_of(field: Field) -> ScienceKind {
    match field {
        Field::ElectronMicroscopy => ScienceKind::ElectronMicroscopy,
        Field::ScanningProbe => ScienceKind::ScanningProbe,
        Field::Surface => ScienceKind::Surface,
        Field::Ebsd => ScienceKind::Ebsd,
    }
}

fn summary(source: &Arc<Source>, p: &ds::Page) -> Result<ScienceSummary> {
    let width = u32::try_from(p.width).map_err(|_| DecodeError::TooLarge)?;
    let height = u32::try_from(p.height).map_err(|_| DecodeError::TooLarge)?;
    let n = u32::try_from(p.planes.len()).map_err(|_| DecodeError::TooLarge)?;
    let mut axes = Vec::new();
    let mut channel_axis = None;
    let mut units_by = None;
    let mut display_ranges = Vec::new();
    let units: Vec<String> = p.planes.iter().map(|q| q.unit.trim().to_owned()).collect();
    let labels: Vec<String> = p.planes.iter().map(|q| q.label.trim().to_owned()).collect();
    if p.rgb && n >= 3 {
        axes.push(NdAxis {
            name: String::new(),
            len: 3,
            unit: String::new(),
            labels: labels[..3].to_vec(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Channel,
        });
        channel_axis = Some(0);
        display_ranges = vec![(0.0, 1.0); 3];
    } else if n > 1 {
        let series = p.axis.kind == ds::AxisKind::Series;
        axes.push(NdAxis {
            name: p.axis.name.trim().to_owned(),
            len: n,
            unit: p
                .axis
                .linear
                .as_ref()
                .map(|l| l.unit.clone())
                .unwrap_or_default(),
            labels: if labels.iter().any(|l| !l.is_empty()) {
                labels
            } else {
                Vec::new()
            },
            wcs_axis: None,
            linear: p.axis.linear.as_ref().and_then(linear),
            kind: if series {
                AxisKind::Cube
            } else {
                AxisKind::Channel
            },
        });
        if units.iter().any(|u| *u != units[0]) {
            units_by = Some((0, units.clone()));
        }
    }
    let pixel = match (p.x.as_ref(), p.y.as_ref()) {
        (Some(x), y) => {
            let y = y.cloned().unwrap_or_else(|| x.clone());
            match (linear(x), linear(&y)) {
                (Some(lx), Some(ly)) => Some(PixelCalibration {
                    x: lx,
                    y: ly,
                    unit_x: x.unit.clone(),
                    unit_y: y.unit.clone(),
                }),
                _ => None,
            }
        }
        _ => None,
    };
    let keywords = p
        .meta
        .iter()
        .map(|(k, v)| (k.clone(), v.clone(), String::new()))
        .collect();
    let sample = match (p.sample, p.rgb) {
        (_, true) => "RGB".to_owned(),
        (Some(s), _) => s.describe().to_owned(),
        (None, _) => "32-bit float".to_owned(),
    };
    Ok(ScienceSummary {
        width,
        height,
        axes,
        channel_axis,
        sample,
        instrument: p.instrument.as_deref().and_then(nonempty),
        date_obs: p.date.as_deref().and_then(nonempty),
        exposure: p.exposure,
        unit: units.first().and_then(|u| nonempty(u)),
        keywords: Arc::new(keywords),
        kind: kind_of(p.field),
        pixel,
        units_by,
        title: p.title.as_deref().and_then(nonempty).or_else(|| {
            (p.planes.len() == 1)
                .then(|| nonempty(&p.planes[0].label))
                .flatten()
        }),
        voltage: p.voltage,
        magnification: p.magnification,
        mode: p.mode.as_deref().and_then(nonempty),
        detector: p.detector.as_deref().and_then(nonempty),
        levelling: p.levelling,
        display_ranges,
        spectra: p
            .spectra
            .clone()
            .map(|s| em_spectra(source, s, width, height)),
        orientations: p.orientations.clone(),
        ..Default::default()
    })
}

impl MatsciSession {
    fn new(
        source: Arc<Source>,
        format: Format,
        pages: Arc<Vec<ds::Page>>,
        page: u32,
    ) -> Result<MatsciSession> {
        let p = &pages[page as usize];
        let mut summary = summary(&source, p)?;
        // A BCF's electron images cover the hypermap's field: their pixels show its spectra.
        if summary.spectra.is_none()
            && format == Format::Bcf
            && let Some(s) = pages.iter().find_map(|q| q.spectra.clone())
        {
            summary.spectra = Some(em_spectra(&source, s, summary.width, summary.height));
        }
        let bits = p.sample.map_or(32, |s| s.bits());
        let info = ImageInfo {
            format,
            width: summary.width,
            height: summary.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: if p.rgb { 8 } else { bits.min(64) as u8 },
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(MatsciSession {
            source,
            pages,
            page,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn page(&self) -> &ds::Page {
        &self.pages[self.page as usize]
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let k = index.first().copied().unwrap_or(0);
        let len = self.summary.axes.first().map_or(1, |a| a.len);
        if k >= len || index.iter().skip(1).any(|&i| i != 0) {
            return Err(DecodeError::Malformed("no such plane".into()));
        }
        let (values, truncated) = self
            .page()
            .read_plane(self.source.bytes(), k as usize)
            .ok_or(DecodeError::TooLarge)?;
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values,
            truncated,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        if self.summary.channel_axis.is_some() {
            let planes = (0..3)
                .map(|k| self.plane(&[k], cancel))
                .collect::<Result<Vec<_>>>()?;
            return Ok(rgb_raster(&planes));
        }
        let idx = vec![0; self.summary.axes.len()];
        Ok(science::auto_raster(&[self.plane(&idx, cancel)?], None))
    }
}

/// Red, green and blue planes in 0..=1 as an RGBA8 raster (NaN shows black).
fn rgb_raster(planes: &[Arc<NdPlane>]) -> Raster {
    let (w, h) = (planes[0].width, planes[0].height);
    let n = w as usize * h as usize;
    let mut rgba = vec![255u8; n * 4];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        for (c, out) in px[..3].iter_mut().enumerate() {
            let v = planes[c].values[i];
            *out = if v.is_finite() {
                (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            } else {
                0
            };
        }
    }
    whole::interleaved(
        w,
        h,
        PixelLayout::Rgba,
        rgba,
        planes.iter().any(|p| p.truncated),
    )
}

impl Session for MatsciSession {
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
        let planes = if self.summary.channel_axis.is_some() {
            3
        } else {
            1
        };
        px * 4 * planes + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
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
        self.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.pages.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(MatsciSession::new(
            self.source.clone(),
            self.info.format,
            self.pages.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else if self
            .summary
            .axes
            .iter()
            .any(|a| a.kind == AxisKind::Channel)
        {
            "Multichannel"
        } else {
            "Greyscale"
        })
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
    use super::*;

    #[test]
    fn emi_names() {
        assert_eq!(first_ser(std::path::Path::new("/nonexistent/a.emi")), None);
        assert!(emi_sidecar(std::path::Path::new("/nonexistent/a_1.ser")).is_none());
    }
}

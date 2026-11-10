//! X-ray diffraction detector frames through the N-D science path (`specular-xtal`): CBF
//! and miniCBF, ESRF EDF (frames and the slices of volume blocks as a slider), ADSC/d*TREK
//! SMV, Bruker, MarCCD and Rigaku R-AXIS, also whole-file gzip, bzip2 or Unix compress
//! compressed (`.cbf.gz`, `.edf.bz2`, `.img.Z`).
//!
//! Frames keep the file's pixel order (row 0 the file's first row), so the geometry's beam
//! centre and pixel coordinates apply to the planes as they are. Masked pixels (negative
//! counts in Pilatus/Eiger frames, EDF `Dummy` values) are NaN.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_xtal::{Image, Kind};

use crate::science::{
    self, AxisKind, NdAxis, NdPlane, PixelCalibration, PlaneCache, ScienceKind, ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

/// Header lines listed at most.
const MAX_KEYWORDS: usize = 4096;

pub(crate) fn format_of(kind: Kind) -> Format {
    match kind {
        Kind::Cbf => Format::Cbf,
        Kind::Edf => Format::Edf,
        Kind::Smv => Format::Smv,
        Kind::Bruker => Format::Bruker,
        Kind::MarCcd => Format::MarCcd,
        Kind::Raxis => Format::Raxis,
    }
}

/// One detector format.
pub struct XtalDecoder(pub Kind);

impl Decoder for XtalDecoder {
    fn format(&self) -> Format {
        format_of(self.0)
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_xtal::sniff(head) == Some(self.0)
    }
    /// The first 16 KiB, also inside a gzip stream.
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(specular_xtal::SNIFF_HEAD)])
            || specular_xtal::sniff_gzip(data) == Some(self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        // `.img` (SMV, R-AXIS) is shared with disk images and `.edf` with EEG recordings:
        // those are claimed by content (`crate::sniff`), as are Bruker's numbered frames.
        match self.0 {
            Kind::Cbf => &["cbf", "cbf.gz", "cbf.bz2"],
            Kind::Edf => &["edf.gz", "edf.bz2"],
            Kind::Bruker => &["sfrm"],
            Kind::MarCcd => &["mccd"],
            Kind::Raxis => &["osc"],
            Kind::Smv => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(XtalSession::new(source)?))
    }
}

fn err(e: specular_xtal::Error) -> DecodeError {
    match e {
        specular_xtal::Error::Unsupported(s) => DecodeError::Unsupported(s),
        specular_xtal::Error::Truncated => DecodeError::Malformed("file is truncated".into()),
        specular_xtal::Error::Malformed(s) => DecodeError::Malformed(s),
        specular_xtal::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

struct XtalSession {
    source: Arc<Source>,
    /// The unpacked file when the whole file is compressed, reserved from the memory budget
    /// while the session holds it.
    unpacked: Option<(Vec<u8>, specular_core::Reservation)>,
    image: Image,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

impl XtalSession {
    fn new(source: Arc<Source>) -> Result<XtalSession> {
        // The unpacked file stays reserved while the session holds it.
        let unpacked = specular_xtal::uncompress(source.bytes()).map_err(err)?;
        let data = unpacked
            .as_ref()
            .map_or(source.bytes(), |(v, _)| v.as_slice());
        let image = Image::parse(data).map_err(err)?;
        let (w, h) = (image.width, image.height);
        let n = image.frames.len();
        let mut axes = Vec::new();
        if n > 1 {
            axes.push(NdAxis {
                name: "Frame".into(),
                len: n as u32,
                unit: String::new(),
                labels: Vec::new(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Cube,
            });
        }
        let mut keywords: Vec<(String, String, String)> = image
            .header
            .iter()
            .map(|(k, v)| (k.clone(), v.clone(), String::new()))
            .collect();
        if unpacked.is_some() {
            keywords.insert(0, kw("File compression", "whole file".into()));
        }
        keywords.truncate(MAX_KEYWORDS);
        let g = &image.geometry;
        // Millimetres on the detector face, for the scale bar and measuring.
        let pixel = g.pixel.map(|(px, py)| PixelCalibration {
            x: science::Linear {
                offset: 0.0,
                step: px * 1e-3,
            },
            y: science::Linear {
                offset: 0.0,
                step: py * 1e-3,
            },
            unit_x: "m".into(),
            unit_y: "m".into(),
        });
        let sample = format!("{}, {}", image.sample.name(), image.compression());
        let summary = ScienceSummary {
            width: w,
            height: h,
            axes,
            sample: sample.clone(),
            unit: Some("counts".into()),
            keywords: Arc::new(keywords),
            kind: ScienceKind::Diffraction,
            pixel,
            title: image.title.clone(),
            detector: image.detector.clone(),
            date_obs: image.date.clone(),
            exposure: g.exposure,
            diffraction: Some(Arc::new(g.clone())),
            ..Default::default()
        };
        let bits = (image.sample.bytes() * 8).min(255) as u8;
        let info = ImageInfo {
            format: format_of(image.kind),
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: bits,
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(XtalSession {
            source,
            unpacked,
            image,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn data(&self) -> &[u8] {
        self.unpacked
            .as_ref()
            .map_or(self.source.bytes(), |(v, _)| v.as_slice())
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let k = index.first().copied().unwrap_or(0) as usize;
        if k >= self.image.frames.len() {
            return Err(DecodeError::Malformed("no such frame".into()));
        }
        self.planes.get_or(&[k as u32], cancel, || {
            let values = self.image.frame(self.data(), k).map_err(err)?;
            Ok(NdPlane {
                width: self.image.width,
                height: self.image.height,
                values,
                truncated: false,
            })
        })
    }
}

impl Session for XtalSession {
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
            |_| Ok(science::auto_raster(&[self.plane(&[0], cancel)?], None)),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * 4 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.unpacked.as_ref().map_or(0, |(v, _)| v.len())
            + self.levels.memory_bytes()
            + self.planes.memory_bytes()
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
    use super::*;

    fn fixture(name: &str) -> Arc<Source> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-xtal/tests/data");
        let bytes = std::fs::read(dir.join(name)).unwrap();
        Arc::new(Source::from_vec(name, bytes))
    }

    #[test]
    fn every_fixture_opens_by_content() {
        let reg = crate::Registry::new();
        let cancel = CancelToken::new();
        for (name, format, frames) in [
            ("pilatus.cbf", Format::Cbf, 1),
            ("small.cbf.gz", Format::Cbf, 1),
            ("saxs.edf", Format::Edf, 3),
            ("saxs.edf.gz", Format::Edf, 3),
            ("bzip2.edf", Format::Edf, 1),
            ("adsc.img", Format::Smv, 1),
            ("dtrek.img", Format::Smv, 1),
            ("frame86.0001", Format::Bruker, 1),
            ("frame100.sfrm", Format::Bruker, 1),
            ("frame.mccd", Format::MarCcd, 1),
            ("plate.img", Format::Raxis, 1),
            ("volume.edf", Format::Edf, 3),
            ("volume_gz.edf", Format::Edf, 3),
            ("types.edf", Format::Edf, 10),
            ("offset.edf", Format::Edf, 1),
            ("adsc32.img", Format::Smv, 1),
            ("adsc32.img.Z", Format::Smv, 1),
            ("noise.img.Z", Format::Smv, 1),
        ] {
            let s = reg
                .open(fixture(name))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, format, "{name}");
            let sc = s.science().unwrap();
            assert_eq!(sc.kind, ScienceKind::Diffraction);
            assert_eq!(sc.planes(), frames, "{name}");
            assert!(sc.diffraction.is_some());
            let p = s.nd_plane(&[frames as u32 - 1], &cancel).unwrap().unwrap();
            assert_eq!(p.values.len(), (sc.width * sc.height) as usize);
            s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        }
    }

    #[test]
    fn pilatus_geometry_and_masks() {
        let s = crate::Registry::new().open(fixture("pilatus.cbf")).unwrap();
        let sc = s.science().unwrap();
        let g = sc.diffraction.as_ref().unwrap();
        assert!(g.complete());
        assert_eq!(g.beam, Some((48.3, 29.7)));
        assert!(sc.detector.as_deref().unwrap().contains("PILATUS"));
        let p = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
        assert!(p.values.iter().any(|v| v.is_nan()));
        let px = sc.pixel.as_ref().unwrap();
        assert!((px.x.step - 172e-6).abs() < 1e-12);
    }

    #[test]
    fn edf_volume_binning() {
        let s = crate::Registry::new().open(fixture("volume.edf")).unwrap();
        let sc = s.science().unwrap();
        assert_eq!(sc.diffraction.as_ref().unwrap().binning, Some((2, 2)));
        assert_eq!(sc.axes[0].len, 3);
        let s = crate::Registry::new().open(fixture("adsc.img")).unwrap();
        let g = s.science().unwrap().diffraction.clone().unwrap();
        assert_eq!(g.binning, Some((2, 2)));
        assert!(g.beam.is_some() && g.distance.is_some() && g.wavelength.is_some());
    }

    #[test]
    fn edf_frames_slide() {
        let s = crate::Registry::new().open(fixture("saxs.edf")).unwrap();
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].len, 3);
        let cancel = CancelToken::new();
        let a = s.nd_plane(&[0], &cancel).unwrap().unwrap();
        let b = s.nd_plane(&[2], &cancel).unwrap().unwrap();
        assert_ne!(a.values, b.values);
        assert!(s.nd_plane(&[3], &cancel).unwrap().is_err());
    }
}

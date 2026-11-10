//! Complex (SAR) samples as planes: the shared part of ENVI, NITF and TIFF complex images.
//!
//! A complex band shows as one of five parts (the [`AxisKind::Part`] axis: magnitude, real,
//! imaginary, phase and decibels, dB first) and through a multilook factor (the looks axis):
//! the samples of each `fx × fy` block are averaged and the block shows the average.
//! Intensity (`re² + im²`) is averaged for magnitude (its square root) and dB
//! (`10·log10`); real, imaginary and phase come from the block's complex sum (coherent
//! looks). Blocks keep the plane's size, so map coordinates and the readout stay the same.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};

use crate::geo::GeoRef;
use crate::science::{AxisKind, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary};
use crate::whole::Levels;
use crate::{DecodeError, DecodeRequest, Format, ImageInfo, Result, Session};

/// The part axis' positions in order; the app names them (index 4 is dB).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Magnitude = 0,
    Real = 1,
    Imaginary = 2,
    Phase = 3,
    Decibel = 4,
}

impl Part {
    const ALL: [Part; 5] = [
        Part::Magnitude,
        Part::Real,
        Part::Imaginary,
        Part::Phase,
        Part::Decibel,
    ];
}

/// Multilook factors offered (x × y, i.e. range × azimuth for most products).
pub(crate) const LOOKS: [(u32, u32); 9] = [
    (1, 1),
    (2, 2),
    (3, 3),
    (4, 4),
    (5, 5),
    (1, 2),
    (2, 1),
    (1, 4),
    (4, 1),
];

/// Where a complex image reads one band's samples from.
pub(crate) trait ComplexSource: Send + Sync {
    /// Band `b`'s (real, imaginary) pairs, row-major, and whether some were missing (NaN).
    fn band(&self, b: u32, cancel: &CancelToken) -> Result<(Vec<[f32; 2]>, bool)>;
}

/// The axes past the image plane: the band axis (when there is more than one band, given
/// by the caller), then the part and looks axes.
pub(crate) fn axes() -> [NdAxis; 2] {
    axes_()
}

fn axes_() -> [NdAxis; 2] {
    [
        NdAxis {
            name: String::new(),
            len: Part::ALL.len() as u32,
            unit: String::new(),
            labels: Vec::new(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Part,
        },
        NdAxis {
            name: "Looks".into(),
            len: LOOKS.len() as u32,
            unit: String::new(),
            labels: LOOKS.iter().map(|(x, y)| format!("{x} × {y}")).collect(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Cube,
        },
    ]
}

/// The first plane shown: dB at one look (on each axis; the band axis, if any, first).
pub(crate) fn start(band_axis: bool) -> Vec<u32> {
    let mut v = if band_axis { vec![0] } else { Vec::new() };
    v.extend([Part::Decibel as u32, 0]);
    v
}

/// The band, part and looks of a plane index (`band_axis`: the index starts with a band).
fn split(index: &[u32], band_axis: bool) -> Result<(u32, Part, (u32, u32))> {
    let (band, rest) = if band_axis {
        (
            index.first().copied().unwrap_or(0),
            index.get(1..).unwrap_or(&[]),
        )
    } else {
        (0, index)
    };
    let part = *Part::ALL
        .get(rest.first().copied().unwrap_or(Part::Decibel as u32) as usize)
        .ok_or_else(|| DecodeError::Malformed("no such part".into()))?;
    let looks = *LOOKS
        .get(rest.get(1).copied().unwrap_or(0) as usize)
        .ok_or_else(|| DecodeError::Malformed("no such look factor".into()))?;
    Ok((band, part, looks))
}

/// The value of a part from a block's complex sum, intensity sum and sample count.
#[inline]
fn value(part: Part, re: f64, im: f64, intensity: f64, n: f64) -> f32 {
    (match part {
        Part::Magnitude => (intensity / n).sqrt(),
        Part::Real => re / n,
        Part::Imaginary => im / n,
        Part::Phase => im.atan2(re),
        Part::Decibel => {
            let i = intensity / n;
            if i > 0.0 { 10.0 * i.log10() } else { f64::NAN }
        }
    }) as f32
}

/// One part of a complex band through `looks`: each block of `fx × fy` samples (the last
/// ones cut at the edges) shows the value of its average; blocks with a missing sample
/// skip it.
pub(crate) fn part_plane(
    samples: &[[f32; 2]],
    w: usize,
    h: usize,
    part: Part,
    (fx, fy): (u32, u32),
) -> Vec<f32> {
    let (fx, fy) = (fx.max(1) as usize, fy.max(1) as usize);
    let mut out = vec![f32::NAN; w * h];
    if fx == 1 && fy == 1 {
        for (o, &[re, im]) in out.iter_mut().zip(samples) {
            if re.is_finite() && im.is_finite() {
                let (re, im) = (f64::from(re), f64::from(im));
                *o = value(part, re, im, re * re + im * im, 1.0);
            }
        }
        return out;
    }
    for by in (0..h).step_by(fy) {
        let y1 = (by + fy).min(h);
        for bx in (0..w).step_by(fx) {
            let x1 = (bx + fx).min(w);
            let (mut re, mut im, mut int, mut n) = (0f64, 0f64, 0f64, 0f64);
            for y in by..y1 {
                for &[r, i] in &samples[y * w + bx..y * w + x1] {
                    if r.is_finite() && i.is_finite() {
                        let (r, i) = (f64::from(r), f64::from(i));
                        re += r;
                        im += i;
                        int += r * r + i * i;
                        n += 1.0;
                    }
                }
            }
            if n == 0.0 {
                continue;
            }
            let v = value(part, re, im, int, n);
            for y in by..y1 {
                out[y * w + bx..y * w + x1].fill(v);
            }
        }
    }
    out
}

/// The plane at `index` (see [`axes`]), through the session's cache.
pub(crate) fn plane(
    src: &dyn ComplexSource,
    cache: &PlaneCache,
    (w, h): (u32, u32),
    index: &[u32],
    band_axis: bool,
    cancel: &CancelToken,
) -> Result<Arc<NdPlane>> {
    let (band, part, looks) = split(index, band_axis)?;
    cache.get_or(index, cancel, || {
        // The pairs and the plane.
        let _res = specular_core::budget::reserve_n(u64::from(w) * u64::from(h), 12)?;
        let (samples, truncated) = src.band(band, cancel)?;
        if samples.len() != w as usize * h as usize {
            return Err(DecodeError::Malformed("band size".into()));
        }
        Ok(NdPlane {
            width: w,
            height: h,
            values: part_plane(&samples, w as usize, h as usize, part, looks),
            truncated,
        })
    })
}

/// The picture for thumbnails and the CPU path: the first band in dB at one look as zscale
/// grey, missing samples transparent.
pub(crate) fn picture(
    src: &dyn ComplexSource,
    cache: &PlaneCache,
    size: (u32, u32),
    band_axis: bool,
    cancel: &CancelToken,
) -> Result<Raster> {
    let p = plane(src, cache, size, &start(band_axis), band_axis, cancel)?;
    Ok(crate::georaster_dec::grey_rgba(&p))
}

/// A complex TIFF page (SampleFormat 5 or 6, e.g. a Sentinel-1 SLC measurement or a
/// GDAL CFloat32 GeoTIFF): each band is a pair of the page's samples.
pub(crate) fn open_tiff(
    source: Arc<Source>,
    tiff: specular_tiff::Tiff,
    page: usize,
) -> Result<Arc<dyn Session>> {
    let p = tiff
        .pages()
        .get(page)
        .ok_or(DecodeError::Malformed("no such page".into()))?;
    let ifd = p.ifd();
    let (w, h) = (p.width(), p.height());
    if u64::from(w) * u64::from(h) > 1 << 28 {
        return Err(DecodeError::TooLarge);
    }
    let bands = u32::from(ifd.samples / 2);
    let geo = ifd
        .geo
        .as_deref()
        .and_then(|g| crate::geo::from_geotiff(g, w, h, p.level_sizes()))
        .map(Arc::new);
    let sample = format!(
        "complex {}-bit {}",
        ifd.bits,
        if ifd.sample_format == specular_tiff::SampleFormat::Float {
            "float"
        } else {
            "signed"
        }
    );
    let mut axes = Vec::new();
    if bands > 1 {
        axes.push(NdAxis {
            name: String::new(),
            len: bands,
            unit: String::new(),
            labels: Vec::new(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Channel,
        });
    }
    axes.extend(axes_());
    let summary = ScienceSummary {
        width: w,
        height: h,
        axes,
        sample: sample.clone(),
        kind: ScienceKind::Gridded,
        geo: geo.clone(),
        start: start(bands > 1),
        title: source
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_owned),
        ..Default::default()
    };
    let format = if ifd
        .geo
        .as_deref()
        .is_some_and(|g| !g.key_directory.is_empty())
    {
        Format::GeoTiff
    } else {
        Format::Tiff
    };
    let info = ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: (ifd.bits * 2).min(255) as u8,
        progressive: false,
        coding: format!("{}, {sample}", ifd.description()),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(TiffSar {
        source,
        tiff,
        page,
        bands,
        info,
        summary,
        geo,
        planes: PlaneCache::new(),
        levels: Levels::new(),
    }))
}

struct TiffSar {
    source: Arc<Source>,
    tiff: specular_tiff::Tiff,
    page: usize,
    bands: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    geo: Option<Arc<GeoRef>>,
    planes: PlaneCache,
    levels: Levels,
}

impl ComplexSource for TiffSar {
    fn band(&self, b: u32, _cancel: &CancelToken) -> Result<(Vec<[f32; 2]>, bool)> {
        if b >= self.bands {
            return Err(DecodeError::Malformed("no such band".into()));
        }
        let opts = specular_tiff::DecodeOptions::default();
        let part = |k: u32| -> Result<(Vec<f32>, bool)> {
            let d = self
                .tiff
                .decode_values(self.source.bytes(), self.page, k as usize, None, &opts)
                .map_err(crate::tiff_dec::map_err)?;
            Ok((
                d.data
                    .chunks_exact(4)
                    .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
                    .collect(),
                d.truncated,
            ))
        };
        let (re, t1) = part(2 * b)?;
        let (im, t2) = part(2 * b + 1)?;
        Ok((
            re.into_iter().zip(im).map(|(r, i)| [r, i]).collect(),
            t1 || t2,
        ))
    }
}

impl Session for TiffSar {
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
        let size = (self.info.width, self.info.height);
        self.levels.decode(
            req,
            size,
            |_| 1,
            |_| picture(self, &self.planes, size, self.bands > 1, cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        u64::from(w) * u64::from(h) * 20 + crate::whole::level_bytes(w, h, req.scale, 4)
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
        self.tiff.pages().len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page as u32
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        crate::tiff_dec::open_page_of(self.source.clone(), self.tiff.clone(), page as usize)
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.geo.clone()
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        let size = (self.info.width, self.info.height);
        Some(plane(
            self,
            &self.planes,
            size,
            index,
            self.bands > 1,
            cancel,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_and_looks() {
        // 3 × 2: (1, 0), (0, 2), (3, 4) / (NaN), (-1, 0), (0, -1).
        let s = [
            [1.0, 0.0],
            [0.0, 2.0],
            [3.0, 4.0],
            [f32::NAN, f32::NAN],
            [-1.0, 0.0],
            [0.0, -1.0],
        ];
        let m = part_plane(&s, 3, 2, Part::Magnitude, (1, 1));
        assert_eq!(&m[..3], &[1.0, 2.0, 5.0]);
        assert!(m[3].is_nan());
        let db = part_plane(&s, 3, 2, Part::Decibel, (1, 1));
        assert!((db[2] - 10.0 * 25f32.log10()).abs() < 1e-5);
        let ph = part_plane(&s, 3, 2, Part::Phase, (1, 1));
        assert!((ph[1] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        // 2 × 2 looks: block (0..2, 0..2) has 3 valid samples, intensity 1 + 4 + 1.
        let m = part_plane(&s, 3, 2, Part::Magnitude, (2, 2));
        let want = (6f32 / 3.0).sqrt();
        assert!(
            [m[0], m[1], m[3], m[4]]
                .iter()
                .all(|v| (v - want).abs() < 1e-6)
        );
        // The edge block (2..3, 0..2): intensity 25 + 1 over 2.
        assert!((m[2] - 13f32.sqrt()).abs() < 1e-6 && m[2] == m[5]);
        let re = part_plane(&s, 3, 2, Part::Real, (2, 2));
        assert_eq!(re[0], 0.0);
        let im = part_plane(&s, 3, 2, Part::Imaginary, (2, 2));
        assert!((im[0] - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn index_split() {
        assert_eq!(split(&[], false).unwrap(), (0, Part::Decibel, (1, 1)));
        assert_eq!(split(&[2, 1, 3], true).unwrap(), (2, Part::Real, (4, 4)));
        assert!(split(&[9], false).is_err());
        assert_eq!(start(true), vec![0, 4, 0]);
        assert_eq!(axes()[1].labels[5], "1 × 2");
    }
}

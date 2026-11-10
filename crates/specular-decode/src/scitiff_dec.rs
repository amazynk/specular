//! TIFF as a scientific format: ImageJ hyperstacks, Zeiss LSM, MetaMorph STK, Molecular
//! Dynamics GEL, and plain TIFFs whose samples are measurements (16 and 32-bit integers,
//! floats, more bands than a colour model uses). These open in the science view: planes of
//! real values over the file's channel, slice and time axes, a calibrated readout and a scale
//! bar, the file's display ranges and colour tables.
//!
//! - **ImageJ**: the `ImageJ=` description gives channels, slices and frames (channels vary
//!   fastest in the page order), spacing, unit, frame interval, the display range and the
//!   calibration function; the IJMetadata tags give per-channel display ranges and LUTs and the
//!   slice labels. Files larger than 4 GiB keep one IFD and their planes back to back after
//!   it.
//! - **Zeiss LSM**: CZ_LSMINFO gives the dimensions and voxel size; each page holds every
//!   channel (planar samples), slices vary fastest, then time points; channel colours and names
//!   come from its colour block.
//! - **MetaMorph STK**: one IFD whose UIC2 tag counts the planes, stored back to back; UIC1
//!   gives the spatial calibration.
//! - **Olympus FluoView**: MM_Header names the dimensions past X and Y (channels, Z, time),
//!   fastest first over the pages, with their steps and units.
//! - **GEL**: square-root coded values (`stored² × scale`).
//! - **SEM** images (Zeiss, FEI / Thermo Fisher, TESCAN tags, Hitachi and JEOL sidecars): the
//!   pixel size and the instrument facts. 8-bit ones stay pictures ([`raster_facts`]), with
//!   the calibration for the readout and the scale bar.
//!
//! Georeferenced TIFFs keep their own path ([`crate::tiff_dec`]).

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_tiff as tf;
use tf::science as ts;

use crate::science::{
    AxisKind, ColourTable, Linear, NdAxis, NdPlane, PixelCalibration, PixelSource, PlaneCache,
    ScienceKind, ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, ImageInfo, Result, Session};

/// Planes the science view will index (larger stacks are cut to this many).
const MAX_PLANES: usize = 1 << 20;
/// Sidecar text files read for SEM facts are at most this large.
const SIDECAR_MAX: u64 = 64 << 10;

/// How one axis indexes the stored planes.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Positions are pages `stride` apart.
    Page(usize),
    /// Positions are samples of each page (planar or chunky bands).
    Band,
}

/// How stored values become physical ones.
#[derive(Clone, Debug)]
enum Transform {
    None,
    Polynomial(ts::Calibration),
    Gel(ts::MdGel),
}

impl Transform {
    fn apply(&self, v: &mut [f32]) {
        match self {
            Transform::None => {}
            Transform::Polynomial(c) => {
                for x in v {
                    *x = c.apply(f64::from(*x)) as f32;
                }
            }
            Transform::Gel(g) => {
                for x in v {
                    *x = g.value(f64::from(*x)) as f32;
                }
            }
        }
    }

    fn value(&self, x: f64) -> f64 {
        match self {
            Transform::None => x,
            Transform::Polynomial(c) => c.apply(x),
            Transform::Gel(g) => g.value(x),
        }
    }
}

/// The scientific reading of a TIFF page or file.
struct Layout {
    /// Stored planes (pages, or one IFD shifted along a contiguous stack).
    pages: Vec<tf::Page>,
    steps: Vec<Step>,
    summary: ScienceSummary,
    transform: Transform,
    /// The session stands for the whole file (a stack), not one page of it.
    whole_file: bool,
    /// One band of measurements on one page, no stack: shown by the picture session with
    /// the summary as its values summary.
    plain: bool,
}

/// What the tags of a file's first IFDs say, read once.
struct Tags {
    imagej: Option<ts::ImageJ>,
    ij_meta: Option<ts::IjMetadata>,
    lsm: Option<ts::LsmInfo>,
    stk: Option<ts::Stk>,
    fluoview: Option<ts::Fluoview>,
    gel: Option<ts::MdGel>,
    instrument: Option<ts::Instrument>,
    resolution: Option<ts::Resolution>,
}

/// The directory (index in `dirs.dirs`) of page `page` as [`tf::Tiff::open`] counts pages.
fn page_dir(dirs: &tf::Directories, page: usize) -> Option<usize> {
    dirs.main_chain()
        .into_iter()
        .filter(|&d| {
            dirs.dirs[d]
                .ifd
                .as_ref()
                .is_some_and(|i| i.subfile_type() & 5 == 0)
        })
        .nth(page)
}

fn read_tags(source: &Source, tiff: &tf::Tiff, page: usize) -> Option<Tags> {
    let data = source.bytes();
    let dirs = tf::Directories::read_limited(data, page.saturating_mul(2) + 2).ok()?;
    let dir = page_dir(&dirs, page)?;
    let first = *dirs.main_chain().first()?;
    let imagej = dirs
        .ascii(data, first, ts::IMAGE_DESCRIPTION)
        .and_then(|d| ts::ImageJ::parse(&d));
    let ij_meta = imagej
        .as_ref()
        .and_then(|_| ts::IjMetadata::read(&dirs, data, first));
    let lsm = ts::LsmInfo::read(&dirs, data, first);
    let stk = ts::Stk::read(&dirs, data, first);
    let fluoview = ts::Fluoview::read(&dirs, data, first);
    let gel = dirs
        .main_chain()
        .into_iter()
        .take(2)
        .find_map(|d| ts::MdGel::read(&dirs, data, d));
    let width = tiff.pages().get(page).map_or(0, |p| p.width());
    let instrument =
        ts::Instrument::read(&dirs, data, dir).or_else(|| sidecar(source.path(), width));
    let resolution = ts::Resolution::read(&dirs, data, dir);
    Some(Tags {
        imagej,
        ij_meta,
        lsm,
        stk,
        fluoview,
        gel,
        instrument,
        resolution,
    })
}

/// A Hitachi or JEOL microscope's `.txt` sidecar next to the image (same name). Missing or
/// unreadable sidecars are no error.
fn sidecar(path: &std::path::Path, width: u32) -> Option<ts::Instrument> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "tif" | "tiff") {
        return None;
    }
    ["txt", "TXT"].iter().find_map(|e| {
        let p = path.with_extension(e);
        let meta = std::fs::metadata(&p).ok()?;
        if !meta.is_file() || meta.len() > SIDECAR_MAX {
            return None;
        }
        let bytes = std::fs::read(&p).ok()?;
        let text = String::from_utf8_lossy(&bytes);
        ts::Instrument::hitachi(&text).or_else(|| ts::Instrument::jeol(&text, width))
    })
}

/// Whether a page's samples are measurements rather than a picture: one band of 16 or more
/// bits or floats with no colour profile, or bands a colour model does not use.
fn measured(ifd: &tf::Ifd) -> bool {
    use tf::Photometric as P;
    if ifd.compression.is_jpeg() || ifd.alpha.is_some() {
        return false;
    }
    match ifd.photometric {
        P::MinIsBlack | P::MinIsWhite if ifd.samples == 1 => {
            ifd.icc.is_none() && (ifd.sample_format == tf::SampleFormat::Float || ifd.bits >= 16)
        }
        P::MinIsBlack => ifd.samples >= 2,
        P::Rgb => ifd.samples > 4 && ifd.bits >= 8,
        _ => false,
    }
}

fn axis(name: &str, len: u32, kind: AxisKind) -> NdAxis {
    NdAxis {
        name: name.to_owned(),
        len,
        unit: String::new(),
        labels: Vec::new(),
        wcs_axis: None,
        linear: None,
        kind,
    }
}

/// Pixel calibration of `step_x` × `step_y` `unit`s per pixel, from `origin` pixels.
fn pixel(step: (f64, f64), unit: &str, origin: (f64, f64)) -> Option<PixelCalibration> {
    let ok = |v: f64| v.is_finite() && v > 0.0;
    (ok(step.0) && ok(step.1) && !unit.is_empty()).then(|| PixelCalibration {
        x: Linear {
            offset: -origin.0 * step.0,
            step: step.0,
        },
        y: Linear {
            offset: -origin.1 * step.1,
            step: step.1,
        },
        unit_x: unit.to_owned(),
        unit_y: unit.to_owned(),
    })
}

/// A length in metres as a step in the most readable unit ("nm", "µm", "mm").
fn readable(m: (f64, f64)) -> ((f64, f64), &'static str) {
    let (k, u) = match m.0 {
        v if v < 1e-6 => (1e9, "nm"),
        v if v < 1e-3 => (1e6, "µm"),
        v if v < 1.0 => (1e3, "mm"),
        _ => (1.0, "m"),
    };
    ((m.0 * k, m.1 * k), u)
}

/// The physical pixel size TIFF resolution gives, when it is plausibly a measurement (finer
/// than 0.1 mm: a microscope or a scanner, not a print density).
fn resolution_pixel(r: &ts::Resolution) -> Option<PixelCalibration> {
    let m = r.pixel_metres()?;
    if m.0 > 1e-4 {
        return None;
    }
    let (s, u) = readable(m);
    pixel(s, u, (0.0, 0.0))
}

/// Facts for the info panel shared by every reading: the instrument and the vendor's fields.
fn describe(s: &mut ScienceSummary, inst: Option<&ts::Instrument>) {
    let Some(i) = inst else { return };
    s.vendor = Some(i.vendor.clone());
    s.instrument = i.model.clone();
    s.voltage = i.voltage;
    s.magnification = i.magnification;
    s.detector = i.detector.clone();
    s.mode = i.mode.clone();
    s.date_obs = i.date.clone();
    s.keywords = Arc::new(
        i.fields
            .iter()
            .map(|(k, v)| (k.clone(), v.clone(), String::new()))
            .collect(),
    );
    if let Some(p) = i.pixel {
        let (st, u) = readable(p);
        if let Some(c) = pixel(st, u, (0.0, 0.0)) {
            s.pixel = Some(c);
            s.pixel_source = Some(PixelSource::Vendor);
        }
    }
    if s.kind == ScienceKind::Microscopy || s.kind == ScienceKind::Astronomy {
        s.kind = ScienceKind::ElectronMicroscopy;
    }
}

/// The colour a LUT gives a channel in a composite: its ramp colour, else its brightest entry.
fn lut_colour(l: &ColourTable) -> [f32; 3] {
    let c = ts::ramp_colour(l).unwrap_or(l[255]);
    c.map(|v| f32::from(v) / 255.0)
}

/// Whether a LUT is the plain grey ramp (shows like no LUT).
fn is_grey(l: &ColourTable) -> bool {
    l.iter()
        .enumerate()
        .all(|(i, e)| e.iter().all(|&v| (i32::from(v) - i as i32).abs() <= 1))
}

/// ImageJ: the hyperstack's axes and planes.
fn imagej_layout(
    tiff: &tf::Tiff,
    page: &tf::Page,
    ij: &ts::ImageJ,
    meta: Option<&ts::IjMetadata>,
    summary: &mut ScienceSummary,
    file_len: u64,
) -> Option<(Vec<tf::Page>, Vec<Step>)> {
    let ifd = page.ifd();
    // Channels as the samples of each page (a 16-bit RGB composite) or as pages.
    let banded = ifd.samples > 1;
    if banded && u32::from(ifd.samples) != ij.channels {
        return None;
    }
    let (c, z, t) = (ij.channels, ij.slices, ij.frames);
    let per_page = if banded { u64::from(c) } else { 1 };
    let images =
        (u64::from(c) * u64::from(z) * u64::from(t) / per_page).min(MAX_PLANES as u64) as usize;
    let pages = if images > 1 {
        stack_pages(tiff, page, images, file_len)?
    } else {
        vec![page.clone()]
    };
    let (c, z, t) = if pages.len() == images {
        (c, z, t)
    } else if banded {
        (c, pages.len() as u32, 1)
    } else {
        // Fewer planes than declared: one axis over the ones there are.
        (1, pages.len() as u32, 1)
    };
    let cs = if banded { 1 } else { c as usize };
    let mut steps = Vec::new();
    let unit = ij.unit.clone().unwrap_or_default();
    let labels: Vec<String> = meta.map(|m| m.labels.clone()).unwrap_or_default();
    if c > 1 {
        let mut a = axis("", c, AxisKind::Channel);
        if labels.len() as u32 == c {
            a.labels = labels.clone();
        }
        summary.axes.push(a);
        steps.push(if banded { Step::Band } else { Step::Page(1) });
        if ij.mode.as_deref() == Some("composite") {
            summary.channel_axis = Some(0);
        }
    }
    if z > 1 {
        let mut a = axis("Z", z, AxisKind::Cube);
        if let Some(sp) = ij.spacing.filter(|s| *s > 0.0) {
            a.unit = ij.zunit.clone().unwrap_or_else(|| unit.clone());
            a.linear = Some(Linear {
                offset: -ij.origin[2].unwrap_or(0.0) * sp,
                step: sp,
            });
        }
        if c == 1 && t == 1 && labels.len() as u32 == z {
            a.labels = labels.clone();
        }
        summary.axes.push(a);
        steps.push(Step::Page(cs));
    }
    if t > 1 {
        let mut a = axis("Time", t, AxisKind::Cube);
        if let Some(fi) = ij.finterval.filter(|s| *s > 0.0) {
            a.unit = ij.tunit.clone().unwrap_or_else(|| "s".into());
            a.linear = Some(Linear {
                offset: 0.0,
                step: fi,
            });
        }
        if c == 1 && z == 1 && labels.len() as u32 == t {
            a.labels = labels.clone();
        }
        summary.axes.push(a);
        steps.push(Step::Page(cs * z as usize));
    }
    Some((pages, steps))
}

/// The `n` planes of a stack starting at `page`: the file's pages from there when it has
/// enough, else the planes stored back to back after an uncompressed first one (ImageJ over
/// 4 GiB, MetaMorph). `None` when neither holds a second plane.
fn stack_pages(tiff: &tf::Tiff, page: &tf::Page, n: usize, file_len: u64) -> Option<Vec<tf::Page>> {
    let all = tiff.pages();
    if all.len() >= n {
        return Some(all[..n].to_vec());
    }
    let ifd = page.ifd();
    if all.len() > 1 || ifd.compression != tf::Compression::None || !ifd.contiguous() {
        return Some(all.to_vec());
    }
    let plane = ifd.stored_bytes();
    let start = *ifd.unit_offsets().first()?;
    if plane == 0 {
        return None;
    }
    // Planes that fit in the file (a cut-short stack shows what is there).
    let fit = (file_len.saturating_sub(start) / plane).max(1);
    let n = (n as u64).min(fit) as usize;
    Some(
        (0..n)
            .filter_map(|k| tf::Page::from_levels(vec![ifd.shifted(k as u64 * plane)]))
            .collect(),
    )
}

/// Builds the scientific reading of page `page`; `None` for a picture.
fn layout(source: &Source, tiff: &tf::Tiff, page: usize, tags: &Tags) -> Option<Layout> {
    let p = tiff.pages().get(page)?;
    let ifd = p.ifd();
    if ifd.geo.is_some() {
        return None;
    }
    let mut summary = ScienceSummary {
        width: p.width(),
        height: p.height(),
        sample: crate::tiff_dec::sample_name(ifd),
        kind: ScienceKind::Microscopy,
        ..Default::default()
    };
    let mut transform = Transform::None;
    let mut whole_file = false;
    let mut plain = false;
    let mut pages = vec![p.clone()];
    let mut steps = Vec::new();
    let file_len = source.bytes().len() as u64;

    if page == 0
        && let Some(lsm) = &tags.lsm
    {
        // Zeiss LSM.
        whole_file = true;
        summary.vendor = Some("Zeiss LSM".into());
        let c = u32::from(ifd.samples);
        let all = tiff.pages();
        let (mut z, mut t) = (lsm.dims[2].max(1), lsm.dims[4].max(1));
        if (z as usize).saturating_mul(t as usize) > all.len() {
            (z, t) = (all.len() as u32, 1);
        }
        pages = all[..(z * t) as usize].to_vec();
        if c > 1 {
            let mut a = axis("", c, AxisKind::Channel);
            if lsm.channel_names.len() as u32 == c {
                a.labels = lsm.channel_names.clone();
            }
            summary.axes.push(a);
            steps.push(Step::Band);
            summary.channel_axis = Some(0);
            if lsm.channel_colours.len() as u32 == c
                && lsm.channel_colours.iter().any(|&c| c != [0, 0, 0])
            {
                summary.channel_colours = lsm
                    .channel_colours
                    .iter()
                    .map(|c| c.map(|v| f32::from(v) / 255.0))
                    .collect();
            }
        }
        if z > 1 {
            let mut a = axis("Z", z, AxisKind::Cube);
            if lsm.voxel[2] > 0.0 {
                a.unit = "µm".into();
                a.linear = Some(Linear {
                    offset: 0.0,
                    step: lsm.voxel[2] * 1e6,
                });
            }
            summary.axes.push(a);
            steps.push(Step::Page(1));
        }
        if t > 1 {
            let mut a = axis("Time", t, AxisKind::Cube);
            let step = if lsm.timestamps.len() >= 2 {
                (lsm.timestamps[lsm.timestamps.len() - 1] - lsm.timestamps[0])
                    / (lsm.timestamps.len() - 1) as f64
            } else {
                lsm.time_interval
            };
            if step > 0.0 {
                a.unit = "s".into();
                a.linear = Some(Linear { offset: 0.0, step });
            }
            summary.axes.push(a);
            steps.push(Step::Page(z as usize));
        }
        if lsm.voxel[0] > 0.0 {
            let y = if lsm.voxel[1] > 0.0 {
                lsm.voxel[1]
            } else {
                lsm.voxel[0]
            };
            summary.pixel = pixel((lsm.voxel[0] * 1e6, y * 1e6), "µm", (0.0, 0.0));
            summary.pixel_source = summary.pixel.as_ref().map(|_| PixelSource::Vendor);
        }
    } else if page == 0
        && let Some(stk) = &tags.stk
    {
        // MetaMorph STK.
        whole_file = true;
        summary.vendor = Some("MetaMorph".into());
        pages = stack_pages(tiff, p, (stk.planes as usize).min(MAX_PLANES), file_len)?;
        if pages.len() > 1 {
            let wl = &stk.wavelengths;
            let by_wavelength = wl.len() == pages.len()
                && wl.iter().all(|w| *w > 0.0)
                && wl.windows(2).any(|w| w[0] != w[1]);
            // ZDistance is the step from the previous plane; a stack with every step set is
            // a Z series, else one whose planes were taken at different times a time series.
            let zd = &stk.z_distance;
            let z = zd.len() == pages.len() && zd.iter().all(|d| *d != 0.0 && d.is_finite());
            let created = &stk.created;
            let timed =
                !z && created.len() == pages.len() && created.windows(2).all(|w| w[0] != w[1]);
            let name = if z {
                "Z"
            } else if timed {
                "Time"
            } else {
                ""
            };
            let mut a = if by_wavelength {
                let mut a = axis("", pages.len() as u32, AxisKind::Channel);
                a.labels = wl.iter().map(|w| format!("{w} nm")).collect();
                a
            } else {
                axis(name, pages.len() as u32, AxisKind::Cube)
            };
            let uniform = |v: &[f64]| {
                let s = v[0];
                v.iter()
                    .all(|d| (d - s).abs() <= 1e-9 * s.abs().max(1.0))
                    .then_some(s)
            };
            if a.labels.is_empty()
                && z
                && let Some(step) = uniform(&zd[..zd.len() - 1])
            {
                a.linear = Some(Linear { offset: 0.0, step });
                a.unit = stk
                    .units
                    .as_deref()
                    .and_then(ts::normalise_unit)
                    .unwrap_or_default();
            } else if timed {
                // Julian day and milliseconds of the day.
                let secs: Vec<f64> = created
                    .iter()
                    .map(|&(d, ms)| f64::from(d) * 86_400.0 + f64::from(ms) / 1000.0)
                    .collect();
                let steps: Vec<f64> = secs.windows(2).map(|w| w[1] - w[0]).collect();
                if let Some(step) = uniform(&steps).filter(|s| *s > 0.0) {
                    a.unit = "s".into();
                    a.linear = Some(Linear { offset: 0.0, step });
                }
            }
            summary.axes.push(a);
            steps.push(Step::Page(1));
        }
        if let (true, Some(x), Some(u)) = (
            stk.spatial || stk.x_calibration.is_some(),
            stk.x_calibration,
            stk.units.as_deref().and_then(ts::normalise_unit),
        ) {
            let y = stk.y_calibration.unwrap_or(x);
            summary.pixel = pixel((x, y), &u, (0.0, 0.0));
            summary.pixel_source = summary.pixel.as_ref().map(|_| PixelSource::Vendor);
        }
        summary.title = stk.name.clone();
    } else if page == 0
        && let Some(fv) = &tags.fluoview
    {
        // Olympus FluoView.
        whole_file = true;
        summary.vendor = Some("Olympus FluoView".into());
        let all = tiff.pages();
        let stack: Vec<&ts::FluoviewDim> = fv
            .dims
            .iter()
            .filter(|d| !matches!(d.axis(), 'X' | 'Y') && d.size > 1)
            .collect();
        let want: usize = stack
            .iter()
            .fold(1usize, |n, d| n.saturating_mul(d.size as usize));
        let fits = want <= all.len();
        pages = all[..if fits { want } else { all.len() }.min(MAX_PLANES)].to_vec();
        let mut stride = 1usize;
        for d in stack.iter().filter(|_| fits) {
            let a = match d.axis() {
                'C' => axis("", d.size, AxisKind::Channel),
                l => {
                    let name = match l {
                        'Z' => "Z",
                        'T' => "Time",
                        _ => d.name.as_str(),
                    };
                    let mut a = axis(name, d.size, AxisKind::Cube);
                    if d.step > 0.0 && !d.unit.is_empty() {
                        if l == 'T' {
                            let k = match d.unit.as_str() {
                                "ms" => 1e-3,
                                "µs" | "us" => 1e-6,
                                "ns" => 1e-9,
                                "min" => 60.0,
                                "h" => 3600.0,
                                _ => 1.0,
                            };
                            a.unit = "s".into();
                            a.linear = Some(Linear {
                                offset: d.origin * k,
                                step: d.step * k,
                            });
                        } else if let Some(m) =
                            ts::normalise_unit(&d.unit).and_then(|u| ts::metres_per(&u))
                        {
                            a.unit = "µm".into();
                            a.linear = Some(Linear {
                                offset: d.origin * m * 1e6,
                                step: d.step * m * 1e6,
                            });
                        } else {
                            a.unit = d.unit.clone();
                            a.linear = Some(Linear {
                                offset: d.origin,
                                step: d.step,
                            });
                        }
                    }
                    a
                }
            };
            summary.axes.push(a);
            steps.push(Step::Page(stride));
            stride *= d.size as usize;
        }
        if !fits && pages.len() > 1 {
            // Fewer pages than the header's dimensions: one axis over the ones there are.
            summary
                .axes
                .push(axis("", pages.len() as u32, AxisKind::Cube));
            steps.push(Step::Page(1));
        }
        if let Some(c) = summary
            .axes
            .iter()
            .position(|a| a.kind == AxisKind::Channel)
        {
            summary.channel_axis = Some(c);
        }
        let dim = |l: char| fv.dims.iter().find(|d| d.axis() == l);
        if let (Some(x), Some(y)) = (dim('X'), dim('Y'))
            && let Some(u) = ts::normalise_unit(&x.unit)
        {
            summary.pixel = pixel((x.step, y.step), &u, (0.0, 0.0));
            summary.pixel_source = summary.pixel.as_ref().map(|_| PixelSource::Vendor);
        }
        summary.title = fv.name.clone();
    } else if page == 0
        && let Some(ij) = &tags.imagej
    {
        // ImageJ.
        let lut_or_cal = tags
            .ij_meta
            .as_ref()
            .is_some_and(|m| m.luts.iter().any(|l| !is_grey(l)))
            || ifd.photometric == tf::Photometric::Palette
            || ij.calibration().is_some_and(|c| !c.is_identity());
        let stack = ij.channels * ij.slices * ij.frames > 1;
        if !(stack || lut_or_cal || measured(ifd)) {
            return None;
        }
        whole_file = true;
        summary.vendor = Some("ImageJ".into());
        if let Some((pg, st)) =
            imagej_layout(tiff, p, ij, tags.ij_meta.as_ref(), &mut summary, file_len)
        {
            pages = pg;
            steps = st;
        }
        if let Some(c) = ij.calibration().filter(|c| !c.is_identity()) {
            transform = Transform::Polynomial(c);
            summary.unit = ij.vunit.clone();
        }
        // The calibrated size: 1 / resolution in the description's unit.
        if let (Some(u), Some(r)) = (ij.unit.as_deref(), tags.resolution) {
            summary.pixel = pixel(
                (1.0 / r.x, 1.0 / r.y),
                u,
                (ij.origin[0].unwrap_or(0.0), ij.origin[1].unwrap_or(0.0)),
            );
            summary.pixel_source = summary.pixel.as_ref().map(|_| PixelSource::Resolution);
        }
        let channels = summary
            .channel_axis
            .map_or(1, |a| summary.axes[a].len as usize)
            .max(
                summary
                    .axes
                    .iter()
                    .find(|a| a.kind == AxisKind::Channel)
                    .map_or(1, |a| a.len as usize),
            );
        if let Some(m) = &tags.ij_meta {
            if !m.ranges.is_empty() {
                summary.display_ranges = m
                    .ranges
                    .iter()
                    .take(channels)
                    .map(|&(lo, hi)| (transform.value(lo), transform.value(hi)))
                    .collect();
            }
            if m.luts.len() >= channels.min(m.luts.len()).max(1) {
                summary.luts = m
                    .luts
                    .iter()
                    .take(channels)
                    .map(|l| Arc::new(**l))
                    .collect();
            }
        }
        if summary.display_ranges.is_empty()
            && let (Some(lo), Some(hi)) = (ij.min, ij.max)
            && hi > lo
        {
            summary.display_ranges = vec![(transform.value(lo), transform.value(hi))];
        }
        if summary.luts.is_empty()
            && ifd.photometric == tf::Photometric::Palette
            && let Some(cm) = ifd.colormap()
            && cm.len() >= 3 * 256
        {
            let n = cm.len() / 3;
            let mut t = [[0u8; 3]; 256];
            for (i, e) in t.iter_mut().enumerate() {
                *e = [0, 1, 2].map(|k| (cm[k * n + i] >> 8) as u8);
            }
            summary.luts = vec![Arc::new(t)];
        }
        if summary.luts.iter().all(|l| is_grey(l)) {
            summary.luts.clear();
        }
        if summary.channel_axis.is_some() && summary.luts.len() > 1 {
            summary.channel_colours = summary.luts.iter().map(|l| lut_colour(l)).collect();
        }
        if let Some(m) = &tags.ij_meta {
            let mut kw: Vec<(String, String, String)> = m
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone(), String::new()))
                .collect();
            kw.extend(
                ij.extra
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone(), String::new())),
            );
            summary.keywords = Arc::new(kw);
        }
    } else if let Some(gel) = &tags.gel {
        // Molecular Dynamics GEL (or a Storm / Typhoon scan).
        if ifd.samples != 1 {
            return None;
        }
        summary.kind = ScienceKind::Gridded;
        summary.vendor = Some("Molecular Dynamics".into());
        summary.unit = gel.units.clone();
        let mut kw = Vec::new();
        for (k, v) in [
            ("MD_FileTag", Some(gel.file_tag.to_string())),
            ("MD_ScalePixel", Some(gel.scale.to_string())),
            ("MD_LabName", gel.lab_name.clone()),
            ("MD_SampleInfo", gel.sample_info.clone()),
            ("MD_PrepDate", gel.prep_date.clone()),
            ("MD_PrepTime", gel.prep_time.clone()),
            ("MD_FileUnits", gel.units.clone()),
        ] {
            if let Some(v) = v {
                kw.push((k.to_owned(), v, String::new()));
            }
        }
        summary.keywords = Arc::new(kw);
        summary.title = gel.sample_info.clone();
        transform = Transform::Gel(gel.clone());
    } else if !measured(ifd) {
        return None;
    } else if ifd.samples == 1 {
        // One band of measurements and nothing more: the picture session shows its values
        // (keeping its pyramid, tiles and stored samples).
        plain = true;
    } else {
        let mut a = axis("", u32::from(ifd.samples), AxisKind::Channel);
        if ifd.photometric == tf::Photometric::Rgb {
            a.labels = ["R", "G", "B"].map(String::from).to_vec();
            a.labels.extend((4..=ifd.samples).map(|i| i.to_string()));
            summary.channel_axis = Some(0);
        }
        summary.axes.push(a);
        steps.push(Step::Band);
    }

    if summary.pixel.is_none()
        && tags.imagej.is_none()
        && let Some(c) = tags.resolution.as_ref().and_then(resolution_pixel)
    {
        summary.pixel = Some(c);
        summary.pixel_source = Some(PixelSource::Resolution);
    }
    describe(&mut summary, tags.instrument.as_ref());
    if pages.is_empty() {
        return None;
    }
    // Every plane must have the first's shape and kind.
    let shape = |q: &tf::Page| {
        let i = q.ifd();
        (q.width(), q.height(), i.samples, i.bits, i.sample_format)
    };
    let s0 = shape(&pages[0]);
    if let Some(bad) = pages.iter().position(|q| shape(q) != s0) {
        pages.truncate(bad);
        summary.axes.clear();
        steps.clear();
        if pages.len() > 1 {
            summary
                .axes
                .push(axis("", pages.len() as u32, AxisKind::Cube));
            steps.push(Step::Page(1));
        }
        summary.channel_axis = None;
        summary.channel_colours.clear();
    }
    Some(Layout {
        pages,
        steps,
        summary,
        transform,
        whole_file,
        plain,
    })
}

/// The picture facts of a TIFF page that stays a picture but is calibrated: an 8-bit SEM image
/// with its vendor's pixel size, a greyscale micrograph with a fine TIFF resolution. The
/// summary is [`ScienceSummary::raster_only`].
fn raster_facts(tiff: &tf::Tiff, page: usize, tags: &Tags) -> Option<ScienceSummary> {
    let p = tiff.pages().get(page)?;
    let ifd = p.ifd();
    if ifd.geo.is_some() {
        return None;
    }
    let grey = matches!(
        ifd.photometric,
        tf::Photometric::MinIsBlack | tf::Photometric::MinIsWhite | tf::Photometric::Palette
    );
    let mut s = ScienceSummary {
        width: p.width(),
        height: p.height(),
        sample: crate::tiff_dec::sample_name(ifd),
        kind: ScienceKind::Microscopy,
        raster_only: true,
        ..Default::default()
    };
    if let Some(ij) = &tags.imagej
        && let (Some(u), Some(r)) = (ij.unit.as_deref(), tags.resolution)
    {
        s.vendor = Some("ImageJ".into());
        s.pixel = pixel(
            (1.0 / r.x, 1.0 / r.y),
            u,
            (ij.origin[0].unwrap_or(0.0), ij.origin[1].unwrap_or(0.0)),
        );
        s.pixel_source = s.pixel.as_ref().map(|_| PixelSource::Resolution);
    } else if grey && let Some(c) = tags.resolution.as_ref().and_then(resolution_pixel) {
        s.pixel = Some(c);
        s.pixel_source = Some(PixelSource::Resolution);
    }
    describe(&mut s, tags.instrument.as_ref());
    (s.pixel.is_some() || tags.instrument.is_some()).then_some(s)
}

/// The scientific session for page `page`, or `Ok(None)` when it is a picture; with the
/// calibrated-picture facts for a picture that has them.
pub(crate) fn open(source: &Arc<Source>, tiff: &tf::Tiff, page: usize) -> Result<Opened> {
    let Some(tags) = read_tags(source, tiff, page) else {
        return Ok(Opened::Picture(None));
    };
    match layout(source, tiff, page, &tags) {
        Some(l) if l.plain => Ok(Opened::Picture(Some(Box::new(l.summary)))),
        Some(l) => Ok(Opened::Science(Arc::new(SciTiffSession::new(
            source.clone(),
            tiff.clone(),
            page,
            l,
            tiff.little_endian,
        )?))),
        None => Ok(Opened::Picture(
            raster_facts(tiff, page, &tags).map(Box::new),
        )),
    }
}

pub(crate) enum Opened {
    Science(Arc<dyn Session>),
    /// A picture, with its calibration and instrument facts when it has any. A summary that
    /// is not raster-only asks for the page's values (one band of measurements).
    Picture(Option<Box<ScienceSummary>>),
}

struct SciTiffSession {
    source: Arc<Source>,
    tiff: tf::Tiff,
    page: usize,
    le: bool,
    pages: Vec<tf::Page>,
    steps: Vec<Step>,
    transform: Transform,
    whole_file: bool,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

impl SciTiffSession {
    fn new(
        source: Arc<Source>,
        tiff: tf::Tiff,
        page: usize,
        l: Layout,
        le: bool,
    ) -> Result<SciTiffSession> {
        let p = &l.pages[0];
        let ifd = p.ifd();
        let mut coding = ifd.description();
        let n = l.summary.planes();
        if n > 1 {
            coding += &format!(", {n} planes");
        }
        if !l.whole_file && tiff.pages().len() > 1 {
            coding += &format!(", page {} of {}", page + 1, tiff.pages().len());
        }
        if tiff.big_tiff {
            coding = format!("BigTIFF {coding}");
        }
        let info = ImageInfo {
            format: crate::Format::Tiff,
            width: p.width(),
            height: p.height(),
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: ifd.bits.min(255) as u8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(SciTiffSession {
            source,
            tiff,
            page,
            le,
            pages: l.pages,
            steps: l.steps,
            transform: l.transform,
            whole_file: l.whole_file,
            info,
            summary: l.summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    /// The stored page and band of the plane at `index`.
    fn locate(&self, index: &[u32]) -> Result<(&tf::Page, usize)> {
        let (mut page, mut band) = (0usize, 0usize);
        for ((a, s), &i) in self.summary.axes.iter().zip(&self.steps).zip(index) {
            if i >= a.len {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
            match s {
                Step::Page(stride) => page += i as usize * stride,
                Step::Band => band = i as usize,
            }
        }
        let p = self
            .pages
            .get(page)
            .ok_or_else(|| DecodeError::Malformed("no such plane".into()))?;
        Ok((p, band))
    }

    fn read(&self, index: &[u32], cancel: &CancelToken) -> Result<NdPlane> {
        let (p, band) = self.locate(index)?;
        let opts = tf::DecodeOptions {
            scale: 1,
            region: None,
            cancel: Some(cancel.flag()),
            parallel: true,
            wide: true,
        };
        let d = p
            .decode_values(self.source.bytes(), self.le, band, None, &opts)
            .map_err(crate::tiff_dec::map_err)?;
        let mut values: Vec<f32> = d
            .data
            .chunks_exact(4)
            .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        self.transform.apply(&mut values);
        Ok(NdPlane {
            width: d.width,
            height: d.height,
            values,
            truncated: d.truncated,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes
            .get_or(index, cancel, || self.read(index, cancel))
    }

    /// The default view: the first plane through its display range (or zscale) and colour
    /// table, or the channels composited in their colours.
    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let s = &self.summary;
        let mut base: Vec<u32> = vec![0; s.axes.len()];
        for (b, &i) in base.iter_mut().zip(&s.start) {
            *b = i;
        }
        let range_at = |c: usize, p: &NdPlane| -> (f32, f32) {
            s.display_ranges
                .get(c)
                .or(s.display_ranges.first())
                .map(|&(lo, hi)| (lo as f32, hi as f32))
                .filter(|(lo, hi)| hi > lo)
                .or_else(|| specular_core::nd::Limits::ZScale.compute(&p.values, p.width as usize))
                .unwrap_or((0.0, 1.0))
        };
        let level = |x: f32, (lo, hi): (f32, f32)| -> f32 {
            if !x.is_finite() || hi <= lo {
                return 0.0;
            }
            ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
        };
        let (w, h) = (self.info.width, self.info.height);
        let n = w as usize * h as usize;
        if let Some(ca) = s.channel_axis {
            let len = s.axes[ca].len as usize;
            let colours: Vec<[f32; 3]> = (0..len)
                .map(|c| {
                    s.channel_colours.get(c).copied().unwrap_or(match (len, c) {
                        (1, _) => [1.0; 3],
                        (_, 0) => [1.0, 0.0, 0.0],
                        (_, 1) => [0.0, 1.0, 0.0],
                        (_, 2) => [0.0, 0.0, 1.0],
                        _ => [1.0; 3],
                    })
                })
                .collect();
            let mut acc = vec![[0f32; 3]; n];
            let mut truncated = false;
            for (c, colour) in colours.iter().enumerate().take(8) {
                let mut idx = base.clone();
                idx[ca] = c as u32;
                let p = self.plane(&idx, cancel)?;
                truncated |= p.truncated;
                let r = range_at(c, &p);
                for (a, &x) in acc.iter_mut().zip(&p.values) {
                    let v = level(x, r);
                    for k in 0..3 {
                        a[k] += v * colour[k];
                    }
                }
            }
            let rgba: Vec<u8> = acc
                .iter()
                .flat_map(|a| {
                    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    [q(a[0]), q(a[1]), q(a[2]), 255]
                })
                .collect();
            return Ok(whole::interleaved(w, h, PixelLayout::Rgba, rgba, truncated));
        }
        let c = s
            .axes
            .iter()
            .position(|a| a.kind == AxisKind::Channel)
            .map_or(0, |a| base[a] as usize);
        let p = self.plane(&base, cancel)?;
        let r = range_at(c, &p);
        let lut = s.luts.get(c).or(s.luts.first());
        match lut {
            Some(t) => {
                let rgba: Vec<u8> = p
                    .values
                    .iter()
                    .flat_map(|&x| {
                        let e = t[(level(x, r) * 255.0 + 0.5) as usize];
                        [e[0], e[1], e[2], 255]
                    })
                    .collect();
                Ok(whole::interleaved(
                    w,
                    h,
                    PixelLayout::Rgba,
                    rgba,
                    p.truncated,
                ))
            }
            None => {
                let grey: Vec<u8> = p
                    .values
                    .iter()
                    .map(|&x| (level(x, r) * 255.0 + 0.5) as u8)
                    .collect();
                Ok(whole::interleaved(
                    w,
                    h,
                    PixelLayout::Gray,
                    grey,
                    p.truncated,
                ))
            }
        }
    }
}

impl Session for SciTiffSession {
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
        let chans = self
            .summary
            .channel_axis
            .map_or(1, |a| u64::from(self.summary.axes[a].len.min(8)));
        px * (4 * chans + 12) + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
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
        Some(
            if self
                .summary
                .axes
                .iter()
                .any(|a| a.kind == AxisKind::Channel)
            {
                "Multichannel"
            } else {
                "Greyscale"
            },
        )
    }
    fn page_count(&self) -> u32 {
        if self.whole_file {
            1
        } else {
            self.tiff.pages().len() as u32
        }
    }
    fn page_index(&self) -> u32 {
        if self.whole_file { 0 } else { self.page as u32 }
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if self.whole_file {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        crate::tiff_dec::open_page_of(self.source.clone(), self.tiff.clone(), page as usize)
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests;

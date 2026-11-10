//! Series assembly (DESIGN §7.2): the headers of the files in a folder (read up to the pixel
//! data, never through it), grouped by Study and Series Instance UID, de-duplicated by SOP
//! Instance UID and ordered by Image Position (Patient) projected on the slice normal from
//! Image Orientation (Patient); without positions by Instance Number, then Acquisition Time.
//! Missing slices are found from the spacing, gantry tilt from the stacking direction. A
//! DICOMDIR's directory records name the files of each series on DICOM media.
//!
//! Scanning reads at most [`MAX_HEADER`] bytes of each of at most [`ScanLimits::files`] files
//! and stops between files when cancelled.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use crate::charset::Charset;
use crate::parse::{self, DataSet, Values};
use crate::ts::{Encoding, TransferSyntax};
use crate::{Error, Result};

/// First tag not read by a header scan: everything from the pixel data on (7FE0,0008 Float
/// Pixel Data, 7FE0,0009 Double Float Pixel Data, 7FE0,0010 Pixel Data, and what follows).
const HEADER_STOP: u32 = 0x7FE0_0008;
/// First read of a file (most headers are a few KiB).
const FIRST_READ: usize = 16 << 10;
/// Most header bytes read from one file (large private groups or overlays before the pixel
/// data); a header longer than this is used as far as it was read.
pub const MAX_HEADER: usize = 16 << 20;
/// Largest deflated file read whole for its header (deflated data sets inflate from the
/// start only).
const MAX_DEFLATED: u64 = 64 << 20;

const STUDY_UID: u32 = 0x0020_000D;
const SERIES_UID: u32 = 0x0020_000E;
const SOP_UID: u32 = 0x0008_0018;
const SOP_CLASS: u32 = 0x0008_0016;
const MODALITY: u32 = 0x0008_0060;
const SERIES_NUMBER: u32 = 0x0020_0011;
const SERIES_DESCRIPTION: u32 = 0x0008_103E;
const INSTANCE_NUMBER: u32 = 0x0020_0013;
const ACQUISITION_TIME: u32 = 0x0008_0032;
const ACQUISITION_DATETIME: u32 = 0x0008_002A;
const POSITION: u32 = 0x0020_0032;
const ORIENTATION: u32 = 0x0020_0037;
const SLICE_LOCATION: u32 = 0x0020_1041;
const NUMBER_OF_FRAMES: u32 = 0x0028_0008;
const PER_FRAME: u32 = 0x5200_9230;
const SHARED: u32 = 0x5200_9229;
const PLANE_POSITION: u32 = 0x0020_9113;
const PLANE_ORIENTATION: u32 = 0x0020_9116;
const FRAME_OF_REFERENCE: u32 = 0x0020_0052;
const TEMPORAL_POSITION: u32 = 0x0020_0100;
const TRIGGER_TIME: u32 = 0x0018_1060;
const FRAME_REFERENCE_TIME: u32 = 0x0054_1300;
const IMAGE_TYPE: u32 = 0x0008_0008;
const CONTAINER_ID: u32 = 0x0040_0512;
const TOTAL_COLUMNS: u32 = 0x0048_0006;
const TOTAL_ROWS: u32 = 0x0048_0007;
const ROWS: u32 = 0x0028_0010;
const COLUMNS: u32 = 0x0028_0011;
const PIXEL_SPACING: u32 = 0x0028_0030;
const SLICE_THICKNESS: u32 = 0x0018_0050;
const PIXEL_MEASURES: u32 = 0x0028_9110;

/// What a header scan keeps of one file. Holds no patient-identifying attributes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instance {
    pub study_uid: String,
    pub series_uid: String,
    pub sop_uid: String,
    pub sop_class: String,
    pub modality: String,
    pub series_number: Option<i64>,
    pub series_description: String,
    pub instance_number: Option<i64>,
    /// Acquisition Time (or the time of Acquisition DateTime), seconds since midnight.
    pub acquisition_time: Option<f64>,
    /// Image Position (Patient) of the first frame, mm.
    pub position: Option<[f64; 3]>,
    /// Image Orientation (Patient): row then column direction cosines.
    pub orientation: Option<[f64; 6]>,
    /// Image Position (Patient) of each frame (enhanced multi-frame files; empty otherwise).
    pub frame_positions: Vec<[f64; 3]>,
    /// Slice Location (0020,1041), mm.
    pub slice_location: Option<f64>,
    /// Frames (1 for single-frame images, 0 without pixel data).
    pub frames: u32,
    /// Images a presentation state applies to (SOP Instance UIDs of its Referenced Series
    /// Sequence); empty for other files.
    pub references: Vec<String>,
    /// Presentation Creation Date and Time of a presentation state (`YYYYMMDDHHMMSS…`).
    pub presentation_created: String,
    /// Content Label of a presentation state.
    pub presentation_label: String,
    /// Frame of Reference UID (0020,0052): images sharing it share patient coordinates.
    pub frame_of_reference: String,
    /// Temporal Position Identifier (0020,0100) of a dynamic series.
    pub temporal_position: Option<i64>,
    /// Trigger Time (0018,1060), else Frame Reference Time (0054,1300), ms.
    pub trigger_time: Option<f64>,
    /// Rows and Columns (0028,0010 and 0028,0011); 0 when not stated.
    pub rows: u32,
    pub columns: u32,
    /// Pixel Spacing (0028,0030; also from the Pixel Measures functional group): between
    /// adjacent rows, then between adjacent columns, mm.
    pub pixel_spacing: Option<[f64; 2]>,
    /// Slice Thickness (0018,0050; also from the Pixel Measures functional group), mm.
    pub slice_thickness: Option<f64>,
    /// Whose anatomy the orientation letters name (0010,2210 and 0018,0015).
    pub anatomy: crate::orient::Anatomy,
}

impl Instance {
    /// Reads the header from the start of a file, `prefix`. `whole`: the prefix is the whole
    /// file. `Ok(None)`: the prefix ends before the pixel data; read more.
    pub fn read(prefix: &[u8], whole: bool) -> Result<Option<Instance>> {
        Instance::read_with(prefix, whole, &Instance::from_dataset)
    }

    /// [`Instance::read`], with the header (everything before the pixel data) given to `f`.
    pub(crate) fn read_with<T>(
        prefix: &[u8],
        whole: bool,
        f: &dyn Fn(&DataSet, &Values, Option<u32>) -> T,
    ) -> Result<Option<T>> {
        let layout = parse::split(prefix)?;
        let meta_v = Values {
            buf: prefix,
            big: false,
        };
        let uid = layout
            .meta
            .get(0x0002_0010)
            .and_then(|e| std::str::from_utf8(meta_v.text_bytes(e)).ok());
        let ts = uid.map_or_else(TransferSyntax::implicit_little, TransferSyntax::from_uid);
        let big = ts.encoding == Encoding::ExplicitBig;
        if ts.encoding == Encoding::DeflatedLittle {
            if !whole {
                return Ok(None);
            }
            let body = prefix.get(layout.body..).unwrap_or(&[]);
            let cap = body.len().saturating_mul(1100).min(1 << 30);
            let inflated = specular_png::inflate_raw(body, cap).map_err(crate::inflate_err)?;
            let h = parse::header(&inflated, 0, true, false, HEADER_STOP)?;
            return Ok(Some(f(
                &h.dataset,
                &Values {
                    buf: &inflated,
                    big: false,
                },
                h.stopped_at,
            )));
        }
        let explicit = ts.encoding != Encoding::ImplicitLittle;
        let h = parse::header(prefix, layout.body, explicit, big, HEADER_STOP)?;
        if h.stopped_at.is_none() && !whole {
            return Ok(None);
        }
        if h.dataset.elements.is_empty() && layout.meta.elements.is_empty() {
            return Err(Error::NotDicom);
        }
        let v = Values { buf: prefix, big };
        Ok(Some(f(&h.dataset, &v, h.stopped_at)))
    }

    fn from_dataset(ds: &DataSet, v: &Values, stopped_at: Option<u32>) -> Instance {
        let cs = Charset::of(ds, v);
        let text = |d: &DataSet, tag| {
            d.get(tag)
                .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
                .unwrap_or_default()
        };
        let nums = |d: &DataSet, tag, n: usize| -> Option<Vec<f64>> {
            let e = d.get(tag)?;
            (0..n).map(|i| v.number(e, i)).collect()
        };
        let has_pixels = matches!(stopped_at, Some(0x7FE0_0008..=0x7FE0_0010));
        let frames = if has_pixels {
            ds.get(NUMBER_OF_FRAMES)
                .and_then(|e| v.number(e, 0))
                .filter(|n| *n >= 1.0 && *n <= u32::MAX as f64)
                .map_or(1, |n| n as u32)
        } else {
            0
        };
        let position3 = |d: &DataSet| nums(d, POSITION, 3).map(|p| [p[0], p[1], p[2]]);
        let frame_positions: Vec<[f64; 3]> = ds
            .get(PER_FRAME)
            .map(|e| {
                e.items
                    .iter()
                    .filter_map(|it| it.item(PLANE_POSITION).and_then(position3))
                    .collect()
            })
            .unwrap_or_default();
        let position = position3(ds).or_else(|| frame_positions.first().copied());
        let orientation: Option<[f64; 6]> = nums(ds, ORIENTATION, 6)
            .or_else(|| {
                let shared = ds.item(SHARED).and_then(|s| s.item(PLANE_ORIENTATION));
                let first = ds
                    .get(PER_FRAME)
                    .and_then(|e| e.items.first())
                    .and_then(|i| i.item(PLANE_ORIENTATION));
                shared.or(first).and_then(|d| nums(d, ORIENTATION, 6))
            })
            .map(|o| [o[0], o[1], o[2], o[3], o[4], o[5]]);
        let frame_positions = if frame_positions.is_empty()
            && let (Some(p), Some(o)) = (position, orientation)
        {
            grid_frame_positions(ds, v, p, o, frames)
        } else {
            frame_positions
        };
        let int = |tag| ds.get(tag).and_then(|e| v.number(e, 0)).map(|n| n as i64);
        // Pixel Measures: the image's own, else the shared or first frame's functional group.
        let measures = |tag, n: usize| {
            nums(ds, tag, n).or_else(|| {
                let shared = ds.item(SHARED).and_then(|s| s.item(PIXEL_MEASURES));
                let first = ds
                    .get(PER_FRAME)
                    .and_then(|e| e.items.first())
                    .and_then(|i| i.item(PIXEL_MEASURES));
                shared
                    .and_then(|d| nums(d, tag, n))
                    .or_else(|| first.and_then(|d| nums(d, tag, n)))
            })
        };
        let pixel_spacing = measures(PIXEL_SPACING, 2)
            .map(|p| [p[0], p[1]])
            .filter(|p| p.iter().all(|x| x.is_finite() && *x > 0.0));
        let slice_thickness = measures(SLICE_THICKNESS, 1)
            .map(|t| t[0])
            .filter(|t| t.is_finite() && *t > 0.0);
        let dim = |tag| {
            ds.get(tag)
                .and_then(|e| v.number(e, 0))
                .filter(|n| *n >= 0.0 && *n <= f64::from(u32::MAX))
                .map_or(0, |n| n as u32)
        };
        let acquisition_time = ds
            .get(ACQUISITION_TIME)
            .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
            .and_then(parse_tm)
            .or_else(|| {
                let dt = ds.get(ACQUISITION_DATETIME)?;
                let s = std::str::from_utf8(v.text_bytes(dt)).ok()?;
                parse_tm(s.get(8..)?)
            });
        let sop_class = text(ds, SOP_CLASS);
        let presentation = sop_class == crate::gsps::GSPS_SOP_CLASS;
        let references = if presentation {
            crate::gsps::referenced_sops(ds, v)
        } else {
            Vec::new()
        };
        let (presentation_created, presentation_label) = if presentation {
            (
                format!("{}{}", text(ds, 0x0070_0082), text(ds, 0x0070_0083)),
                text(ds, 0x0070_0080),
            )
        } else {
            Default::default()
        };
        Instance {
            study_uid: text(ds, STUDY_UID),
            series_uid: text(ds, SERIES_UID),
            sop_uid: text(ds, SOP_UID),
            sop_class,
            modality: text(ds, MODALITY),
            series_number: int(SERIES_NUMBER),
            series_description: text(ds, SERIES_DESCRIPTION),
            instance_number: int(INSTANCE_NUMBER),
            acquisition_time,
            position,
            orientation,
            frame_positions: if frames > 1 {
                frame_positions
            } else {
                Vec::new()
            },
            slice_location: ds.get(SLICE_LOCATION).and_then(|e| v.number(e, 0)),
            frames,
            references,
            presentation_created,
            presentation_label,
            frame_of_reference: text(ds, FRAME_OF_REFERENCE),
            temporal_position: int(TEMPORAL_POSITION),
            trigger_time: ds
                .get(TRIGGER_TIME)
                .or_else(|| ds.get(FRAME_REFERENCE_TIME))
                .and_then(|e| v.number(e, 0)),
            rows: dim(ROWS),
            columns: dim(COLUMNS),
            pixel_spacing,
            slice_thickness,
            anatomy: crate::orient::Anatomy::read(ds, v),
        }
    }

    /// The slice normal (row × column direction), unit length.
    pub fn normal(&self) -> Option<[f64; 3]> {
        let o = self.orientation?;
        normalise(cross([o[0], o[1], o[2]], [o[3], o[4], o[5]]))
    }

    /// Whether this file has an image.
    pub fn has_image(&self) -> bool {
        self.frames > 0
    }

    /// Whether this file is a Grayscale Softcopy Presentation State.
    pub fn is_presentation_state(&self) -> bool {
        self.sop_class == crate::gsps::GSPS_SOP_CLASS
    }
}

/// Reads the header of the file at `path`: the first 16 KiB, then twice as much until the
/// pixel data is reached, the file ends or [`MAX_HEADER`] bytes were read. `Ok(None)`: not
/// DICOM (by its first bytes).
pub fn read_header(path: &Path) -> Result<Option<Instance>> {
    read_header_with(path, &Instance::from_dataset)
}

/// [`read_header`], with the header given to `with`.
pub(crate) fn read_header_with<T>(
    path: &Path,
    with: &dyn Fn(&DataSet, &Values, Option<u32>) -> T,
) -> Result<Option<T>> {
    let mut f = std::fs::File::open(path).map_err(|_| Error::Truncated)?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buf = Vec::new();
    let mut want = FIRST_READ;
    let mut eof = false;
    loop {
        if buf.len() < want && !eof {
            let need = (want - buf.len()) as u64;
            let got = (&mut f)
                .take(need)
                .read_to_end(&mut buf)
                .map_err(|_| Error::Truncated)?;
            eof = (got as u64) < need;
        }
        if want == FIRST_READ && !crate::sniff(&buf) {
            return Ok(None);
        }
        let whole = eof || buf.len() as u64 >= len;
        if let Some(i) = Instance::read_with(&buf, whole, with)? {
            return Ok(Some(i));
        }
        if want >= MAX_HEADER && !is_deflated(&buf) {
            // As far as it was read.
            return Instance::read_with(&buf, true, with);
        }
        want = if is_deflated(&buf) {
            if len > MAX_DEFLATED {
                return Err(Error::TooLarge(len));
            }
            len as usize
        } else {
            (want * 2).min(MAX_HEADER)
        };
    }
}

fn is_deflated(prefix: &[u8]) -> bool {
    let Ok(layout) = parse::split(prefix) else {
        return false;
    };
    let v = Values {
        buf: prefix,
        big: false,
    };
    layout
        .meta
        .get(0x0002_0010)
        .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
        .is_some_and(|u| TransferSyntax::from_uid(u).encoding == Encoding::DeflatedLittle)
}

/// A DICOM time (TM, `HHMMSS.FFFFFF`, also `HH:MM:SS` from old writers) in seconds since
/// midnight.
fn parse_tm(s: &str) -> Option<f64> {
    let s: String = s.trim().chars().filter(|c| *c != ':').collect();
    let (hms, frac) = s.split_once('.').unwrap_or((&s, ""));
    if hms.len() < 2 || !hms.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let field = |r: std::ops::Range<usize>| -> f64 {
        hms.get(r)
            .and_then(|x| x.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let mut t = field(0..2) * 3600.0 + field(2..4) * 60.0 + field(4..6);
    if !frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit()) {
        t += format!("0.{frac}").parse::<f64>().unwrap_or(0.0);
    }
    Some(t)
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalise(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(a, a).sqrt();
    (n > 1e-6 && n.is_finite()).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

// ------------------------------------------------------------------------------- assembly

/// How a series' items are ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesOrder {
    /// Image Position (Patient) along the slice normal.
    Position,
    InstanceNumber,
    AcquisitionTime,
    /// As found (file name order).
    Found,
}

/// One file of a series.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesItem<K> {
    pub key: K,
    pub instance: Instance,
    /// Position along the slice normal, mm (ordering by position only).
    pub along: Option<f64>,
}

/// Slices missing between two items: `missing` slices of the series' spacing would fit
/// between item `after` and the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    pub after: usize,
    pub missing: u32,
    /// Distance between the two items, mm.
    pub distance: f64,
}

/// The images of one series, in display order.
#[derive(Clone, Debug, PartialEq)]
pub struct Series<K> {
    pub study_uid: String,
    pub series_uid: String,
    pub modality: String,
    pub number: Option<i64>,
    pub description: String,
    pub items: Vec<SeriesItem<K>>,
    pub order: SeriesOrder,
    /// Typical distance between slice positions, mm (ordering by position).
    pub spacing: Option<f64>,
    pub gaps: Vec<Gap>,
    /// The spacing varies (beyond gaps) by more than 10 %.
    pub irregular: bool,
    /// Angle between the stacking direction and the slice normal (gantry tilt), degrees,
    /// when above half a degree.
    pub tilt: Option<f64>,
    /// Files left out as copies of another (same SOP Instance UID).
    pub duplicates: usize,
}

impl<K> Series<K> {
    /// Frames over all items.
    pub fn frames(&self) -> u64 {
        self.items
            .iter()
            .map(|i| i.instance.frames.max(1) as u64)
            .sum()
    }

    /// Estimated slices missing over all gaps.
    pub fn missing(&self) -> u32 {
        self.gaps.iter().map(|g| g.missing).sum()
    }
}

/// Groups `found` (in file name order) into series, each ordered for display. Files without
/// an image are left out. Series come in study order, then by Series Number, description and
/// UID.
pub fn assemble<K>(found: Vec<(K, Instance)>) -> Vec<Series<K>> {
    let mut groups: Vec<Vec<(K, Instance)>> = Vec::new();
    let mut index: HashMap<(String, String), usize> = HashMap::new();
    for (k, inst) in found {
        if !inst.has_image() {
            continue;
        }
        let key = (inst.study_uid.clone(), inst.series_uid.clone());
        let g = *index.entry(key).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push((k, inst));
    }
    let mut out: Vec<Series<K>> = groups.into_iter().map(order_series).collect();
    out.sort_by(|a, b| {
        a.study_uid
            .cmp(&b.study_uid)
            .then(
                a.number
                    .unwrap_or(i64::MAX)
                    .cmp(&b.number.unwrap_or(i64::MAX)),
            )
            .then_with(|| a.description.cmp(&b.description))
            .then_with(|| a.series_uid.cmp(&b.series_uid))
    });
    out
}

fn order_series<K>(files: Vec<(K, Instance)>) -> Series<K> {
    // Duplicates: the same SOP instance in several files; the first found stays.
    let mut seen = std::collections::HashSet::new();
    let mut duplicates = 0;
    let mut items: Vec<SeriesItem<K>> = Vec::with_capacity(files.len());
    for (key, instance) in files {
        if !instance.sop_uid.is_empty() && !seen.insert(instance.sop_uid.clone()) {
            duplicates += 1;
            continue;
        }
        items.push(SeriesItem {
            key,
            instance,
            along: None,
        });
    }
    let first = &items[0].instance;
    let mut s = Series {
        study_uid: first.study_uid.clone(),
        series_uid: first.series_uid.clone(),
        modality: first.modality.clone(),
        number: first.series_number,
        description: first.series_description.clone(),
        items: Vec::new(),
        order: SeriesOrder::Found,
        spacing: None,
        gaps: Vec::new(),
        irregular: false,
        tilt: None,
        duplicates,
    };
    // By position: every item has a position and they share one slice orientation.
    let normal = first.normal();
    let parallel = normal.is_some_and(|n| {
        items.iter().all(|i| {
            i.instance.position.is_some()
                && i.instance
                    .normal()
                    .is_some_and(|m| dot(n, m).abs() > 0.9999)
        })
    });
    let by_number = |a: &SeriesItem<K>, b: &SeriesItem<K>| {
        a.instance
            .instance_number
            .cmp(&b.instance.instance_number)
            .then_with(|| cmp_f(a.instance.acquisition_time, b.instance.acquisition_time))
    };
    if let (Some(n), true) = (normal, parallel && items.len() > 1) {
        for i in &mut items {
            i.along = i.instance.position.map(|p| dot(p, n));
        }
        // Positions that are all the same (a time series at one location) order nothing.
        let (lo, hi) = items
            .iter()
            .filter_map(|i| i.along)
            .fold((f64::MAX, f64::MIN), |(lo, hi), a| (lo.min(a), hi.max(a)));
        if hi - lo > 1e-3 {
            items.sort_by(|a, b| cmp_f(a.along, b.along).then_with(|| by_number(a, b)));
            s.order = SeriesOrder::Position;
            spacing_report(&mut s, &items, n);
        } else {
            for i in &mut items {
                i.along = None;
            }
        }
    }
    if s.order != SeriesOrder::Position {
        if items.iter().all(|i| i.instance.instance_number.is_some()) {
            items.sort_by(by_number);
            s.order = SeriesOrder::InstanceNumber;
        } else if items.iter().all(|i| i.instance.acquisition_time.is_some()) {
            items.sort_by(|a, b| cmp_f(a.instance.acquisition_time, b.instance.acquisition_time));
            s.order = SeriesOrder::AcquisitionTime;
        }
    }
    s.items = items;
    s
}

fn cmp_f(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Spacing, gaps, irregular spacing and tilt of items ordered by position.
fn spacing_report<K>(s: &mut Series<K>, items: &[SeriesItem<K>], normal: [f64; 3]) {
    // Distinct positions (several items at one position: echoes, phases).
    let mut at: Vec<(usize, f64)> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let Some(a) = it.along else { continue };
        match at.last_mut() {
            Some(last) if (a - last.1).abs() < 1e-3 => last.0 = i,
            _ => at.push((i, a)),
        }
    }
    if at.len() < 2 {
        return;
    }
    let diffs: Vec<f64> = at.windows(2).map(|w| w[1].1 - w[0].1).collect();
    let mut sorted = diffs.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    if median <= 1e-3 {
        return;
    }
    s.spacing = Some(median);
    for (k, d) in diffs.iter().enumerate() {
        let r = d / median;
        if r > 1.5 {
            s.gaps.push(Gap {
                after: at[k].0,
                missing: ((r.round() as u32).saturating_sub(1)).max(1),
                distance: *d,
            });
        } else if !(0.9..=1.1).contains(&r) {
            s.irregular = true;
        }
    }
    // Tilt: the stacking direction (first to last position) against the normal.
    let (Some(p0), Some(p1)) = (
        items.first().and_then(|i| i.instance.position),
        items.last().and_then(|i| i.instance.position),
    ) else {
        return;
    };
    if let Some(dir) = normalise([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]]) {
        let angle = dot(dir, normal).abs().min(1.0).acos().to_degrees();
        if angle > 0.5 {
            s.tilt = Some(angle);
        }
    }
}

// ------------------------------------------------------------------------------ DICOMDIR

/// An image (or other leaf) record of a DICOMDIR.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirEntry {
    /// Referenced File ID components, relative to the DICOMDIR's folder.
    pub file: Vec<String>,
    pub study_uid: String,
    pub series_uid: String,
    pub sop_uid: String,
    pub instance_number: Option<i64>,
}

impl DirEntry {
    /// The file's path, given the folder of the DICOMDIR.
    pub fn path(&self, root: &Path) -> PathBuf {
        let mut p = root.to_path_buf();
        for c in &self.file {
            p.push(c);
        }
        p
    }

    /// The file as it is on disk (File IDs are upper case, the files may not be), if it
    /// exists.
    pub fn resolve(&self, root: &Path) -> Option<PathBuf> {
        resolve_case(&self.path(root))
    }
}

const DIRECTORY_RECORDS: u32 = 0x0004_1220;
const FIRST_ROOT_RECORD: u32 = 0x0004_1200;
const NEXT_RECORD: u32 = 0x0004_1400;
const LOWER_RECORD: u32 = 0x0004_1420;
const RECORD_TYPE: u32 = 0x0004_1430;
const REFERENCED_FILE: u32 = 0x0004_1500;
const REFERENCED_SOP: u32 = 0x0004_1511;

/// Most directory records followed (cycles in broken offset chains end here too).
const MAX_RECORDS: usize = 1 << 20;

/// Reads the leaf records of a DICOMDIR that reference files, with the study and series
/// they are under: by the records' offsets (next / lower level), else in sequence order.
pub fn read_dicomdir(data: &[u8]) -> Result<Vec<DirEntry>> {
    let d = crate::Dicom::parse(data)?;
    let v = d.values(data);
    let Some(seq) = d.dataset.get(DIRECTORY_RECORDS) else {
        return Ok(Vec::new());
    };
    let records = &seq.items;
    let text = |ds: &DataSet, tag| {
        ds.get(tag)
            .map(|e| String::from_utf8_lossy(v.text_bytes(e)).trim().to_owned())
            .unwrap_or_default()
    };
    // Record offsets: from the start of the item header (8 bytes) before the first element.
    let explicit = d.transfer_syntax.encoding != Encoding::ImplicitLittle;
    let start_of = |ds: &DataSet| -> Option<u64> {
        let e = ds.elements.first()?;
        let header = if explicit && e.vr.long_length() {
            12
        } else {
            8
        };
        (e.value.start as u64).checked_sub(header + 8)
    };
    let by_offset: HashMap<u64, usize> = if d.inflated.is_none() {
        records
            .iter()
            .enumerate()
            .filter_map(|(i, r)| Some((start_of(r)?, i)))
            .collect()
    } else {
        HashMap::new()
    };
    let offset = |ds: &DataSet, tag| -> Option<u64> {
        let e = ds.get(tag)?;
        v.int(e).filter(|o| *o > 0).map(|o| o as u64)
    };
    let root = offset(&d.dataset, FIRST_ROOT_RECORD).and_then(|o| by_offset.get(&o).copied());
    let mut out = Vec::new();
    let mut ctx = (String::new(), String::new());
    let leaf = |r: &DataSet, ctx: &(String, String), out: &mut Vec<DirEntry>| {
        let Some(e) = r.get(REFERENCED_FILE) else {
            return;
        };
        let file: Vec<String> = String::from_utf8_lossy(v.text_bytes(e))
            .split('\\')
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty() && c != ".." && c != ".")
            .collect();
        if file.is_empty() {
            return;
        }
        out.push(DirEntry {
            file,
            study_uid: ctx.0.clone(),
            series_uid: ctx.1.clone(),
            sop_uid: text(r, REFERENCED_SOP),
            instance_number: r
                .get(INSTANCE_NUMBER)
                .and_then(|e| v.number(e, 0))
                .map(|n| n as i64),
        });
    };
    if let Some(root) = root {
        // Depth-first over the offset tree.
        let mut visited = vec![false; records.len()];
        let mut stack: Vec<(usize, (String, String))> = vec![(root, ctx.clone())];
        let mut steps = 0;
        while let Some((mut i, mut c)) = stack.pop() {
            let parent = c.clone();
            loop {
                steps += 1;
                if steps > MAX_RECORDS || visited[i] {
                    break;
                }
                visited[i] = true;
                let r = &records[i];
                c = parent.clone();
                match text(r, RECORD_TYPE).as_str() {
                    "STUDY" => c = (text(r, STUDY_UID), String::new()),
                    "SERIES" => c.1 = text(r, SERIES_UID),
                    "PATIENT" => {}
                    _ => leaf(r, &c, &mut out),
                }
                let next = offset(r, NEXT_RECORD).and_then(|o| by_offset.get(&o).copied());
                if let Some(lower) =
                    offset(r, LOWER_RECORD).and_then(|o| by_offset.get(&o).copied())
                {
                    // Siblings after this record continue with the parent's context once the
                    // lower level is done.
                    if let Some(n) = next {
                        stack.push((n, parent.clone()));
                    }
                    stack.push((lower, c.clone()));
                    break;
                }
                match next {
                    Some(n) => i = n,
                    None => break,
                }
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    for r in records {
        match text(r, RECORD_TYPE).as_str() {
            "PATIENT" => ctx = (String::new(), String::new()),
            "STUDY" => ctx = (text(r, STUDY_UID), String::new()),
            "SERIES" => ctx.1 = text(r, SERIES_UID),
            _ => leaf(r, &ctx, &mut out),
        }
    }
    Ok(out)
}

/// Whether `name` is a DICOMDIR's (`DICOMDIR`, any case, without extension).
pub fn is_dicomdir_name(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|n| n.eq_ignore_ascii_case("DICOMDIR"))
}

/// The DICOMDIR of the medium `dir` is on: in `dir` or up to `levels` folders above it.
pub fn find_dicomdir(dir: &Path, levels: usize) -> Option<PathBuf> {
    let mut d = Some(dir);
    for _ in 0..=levels {
        let here = d?;
        if let Ok(rd) = std::fs::read_dir(here) {
            for e in rd.flatten() {
                if is_dicomdir_name(&e.file_name()) && e.file_type().is_ok_and(|t| t.is_file()) {
                    return Some(e.path());
                }
            }
        }
        d = here.parent();
    }
    None
}

// ---------------------------------------------------------------------------------- scan

/// Bounds of a folder scan.
#[derive(Clone, Copy, Debug)]
pub struct ScanLimits {
    /// Files whose headers are read, at most.
    pub files: usize,
    /// Folders above the scanned one searched for a DICOMDIR.
    pub dicomdir_levels: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        ScanLimits {
            files: 20_000,
            dicomdir_levels: 4,
        }
    }
}

/// What a scan found.
#[derive(Debug, Default)]
pub struct Scan {
    /// The series the files belong to.
    pub series: Vec<Series<PathBuf>>,
    /// Headers read.
    pub headers: usize,
    /// Files skipped as not DICOM (by their first bytes) or unreadable.
    pub skipped: usize,
    /// More candidate files than [`ScanLimits::files`]: the rest were not read.
    pub capped: bool,
    /// The DICOMDIR that named the files, if one was used.
    pub dicomdir: Option<PathBuf>,
    /// Grayscale Softcopy Presentation States found with the images, with their headers
    /// (what they reference, when they were made).
    pub presentations: Vec<(PathBuf, Instance)>,
}

/// Reads the headers of the files around `file` and assembles their series: the files of
/// its series named by a DICOMDIR of its medium when there is one (only those are read),
/// together with the other files in its folder. Stops early (`Err(Cancelled)`) once
/// `cancel` is set.
pub fn scan(file: &Path, limits: ScanLimits, cancel: &AtomicBool) -> Result<Scan> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let mut out = Scan::default();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_type().is_ok_and(|t| t.is_file()) && !is_dicomdir_name(&e.file_name()) {
                candidates.push(e.path());
            }
        }
    }
    candidates.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    // A DICOMDIR names the files of each series, wherever on the medium they are.
    if let Some(dd) = find_dicomdir(dir, limits.dicomdir_levels)
        && let Ok(data) = std::fs::read(&dd)
        && let Ok(entries) = read_dicomdir(&data)
    {
        let root = dd.parent().unwrap_or(Path::new("."));
        let mine = file.canonicalize().ok();
        let resolve = |e: &DirEntry| resolve_case(&e.path(root));
        let series = entries
            .iter()
            .find(|e| mine.is_some() && resolve(e).and_then(|p| p.canonicalize().ok()) == mine)
            .map(|e| (e.study_uid.clone(), e.series_uid.clone()));
        if let Some(key) = series {
            out.dicomdir = Some(dd.clone());
            let have: std::collections::HashSet<PathBuf> = candidates.iter().cloned().collect();
            for e in &entries {
                if (e.study_uid.clone(), e.series_uid.clone()) == key
                    && let Some(p) = resolve(e)
                    && !have.contains(&p)
                {
                    candidates.push(p);
                }
            }
        }
    }
    if candidates.len() > limits.files {
        out.capped = true;
        // The opened file is always read.
        let keep = candidates.iter().position(|p| p == file);
        candidates.truncate(limits.files);
        if let Some(k) = keep
            && k >= limits.files
        {
            candidates.push(file.to_path_buf());
        }
    }
    let mut found = Vec::new();
    for p in candidates {
        if cancel.load(AtomicOrdering::Relaxed) {
            return Err(Error::Cancelled);
        }
        match read_header(&p) {
            Ok(Some(i)) => {
                out.headers += 1;
                if i.is_presentation_state() {
                    out.presentations.push((p, i));
                } else {
                    found.push((p, i));
                }
            }
            _ => out.skipped += 1,
        }
    }
    one_slide_per_series(&mut found, file);
    out.series = assemble(found);
    Ok(out)
}

/// The SOP Class of a VL Whole Slide Microscopy Image.
pub const WSI_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";

/// A whole-slide series is one slide stored as several files (pyramid levels, label,
/// overview, thumbnail), not slices: keeps one file of it, the opened one when it belongs
/// there, otherwise the one with the most frames (the full-resolution level).
fn one_slide_per_series(found: &mut Vec<(PathBuf, Instance)>, opened: &Path) {
    let mut keep: HashMap<(String, String), usize> = HashMap::new();
    for (k, (p, i)) in found.iter().enumerate() {
        if i.sop_class != WSI_SOP_CLASS {
            continue;
        }
        let key = (i.study_uid.clone(), i.series_uid.clone());
        match keep.get(&key) {
            Some(&j) if found[j].0 == opened => {}
            Some(&j) if p != opened && found[j].1.frames >= i.frames => {}
            _ => {
                keep.insert(key, k);
            }
        }
    }
    let mut k = 0;
    found.retain(|(_, i)| {
        let me = k;
        k += 1;
        i.sop_class != WSI_SOP_CLASS
            || keep.get(&(i.study_uid.clone(), i.series_uid.clone())) == Some(&me)
    });
}

/// Bytes read first to tell a whole-slide file: its SOP Class UID is near the start.
const WSI_HEAD: u64 = 4096;

/// What grouping needs of one whole-slide file.
struct SlidePart {
    study: String,
    series: String,
    /// Container Identifier, else Frame of Reference UID: what ties a label in a series of
    /// its own to the slide (as opening the slide does).
    slide: String,
    /// Image Type's third value is VOLUME (a pyramid level).
    volume: bool,
    /// Total pixel matrix size (columns × rows).
    size: u64,
}

fn slide_part(path: &Path) -> Option<SlidePart> {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(WSI_HEAD)
        .read_to_end(&mut head)
        .ok()?;
    let sop = WSI_SOP_CLASS.as_bytes();
    if head.get(128..132) != Some(b"DICM") || !head.windows(sop.len()).any(|w| w == sop) {
        return None;
    }
    let (class, part) = read_header_with(path, &|ds, v, _| {
        let cs = Charset::of(ds, v);
        let text = |tag| {
            ds.get(tag)
                .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
                .unwrap_or_default()
        };
        let num = |tag| ds.get(tag).and_then(|e| v.number(e, 0)).unwrap_or(0.0);
        let container = text(CONTAINER_ID);
        let slide = if container.is_empty() {
            text(FRAME_OF_REFERENCE)
        } else {
            container
        };
        let part = SlidePart {
            study: text(STUDY_UID),
            series: text(SERIES_UID),
            slide,
            volume: text(IMAGE_TYPE)
                .split('\\')
                .nth(2)
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("VOLUME")),
            size: (num(TOTAL_COLUMNS).max(0.0) * num(TOTAL_ROWS).max(0.0)) as u64,
        };
        (text(SOP_CLASS), part)
    })
    .ok()??;
    (class == WSI_SOP_CLASS).then_some(part)
}

/// The whole-slide images among `paths`, each slide's files together (its pyramid levels
/// and thumbnail, and its label and overview, also when those are a series of their own),
/// the full-resolution level first. Only slides of more than one file; files of different
/// folders are never one slide. Reads at most 4 KiB of a file that is not a whole-slide
/// image. Empty once `cancelled` says so.
pub fn whole_slides(paths: &[PathBuf], cancelled: &dyn Fn() -> bool) -> Vec<Vec<PathBuf>> {
    type Key = (PathBuf, String, String);
    type Group = (Key, Vec<(PathBuf, SlidePart)>);
    let mut groups: Vec<Group> = Vec::new();
    let mut at: HashMap<Key, usize> = HashMap::new();
    for p in paths {
        if cancelled() {
            return Vec::new();
        }
        let Some(part) = slide_part(p) else {
            continue;
        };
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        let key = (dir, part.study.clone(), part.series.clone());
        let i = *at.entry(key.clone()).or_insert_with(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        groups[i].1.push((p.clone(), part));
    }
    // A series without a pyramid level (a label or overview of its own) joins the slide it
    // belongs to.
    let has_volume = |g: &[(PathBuf, SlidePart)]| g.iter().any(|(_, p)| p.volume);
    for i in 0..groups.len() {
        if has_volume(&groups[i].1) {
            continue;
        }
        let ((dir, study, _), members) = &groups[i];
        let Some(slide) = members
            .iter()
            .map(|(_, p)| &p.slide)
            .find(|s| !s.is_empty())
        else {
            continue;
        };
        let target = (0..groups.len()).find(|&j| {
            let ((d, st, _), m) = &groups[j];
            j != i
                && d == dir
                && st == study
                && has_volume(m)
                && m.iter().any(|(_, p)| &p.slide == slide)
        });
        if let Some(j) = target {
            let moved = std::mem::take(&mut groups[i].1);
            groups[j].1.extend(moved);
        }
    }
    groups
        .into_iter()
        .filter(|(_, m)| m.len() > 1)
        .map(|(_, mut m)| {
            m.sort_by(|(a, x), (b, y)| {
                y.volume
                    .cmp(&x.volume)
                    .then(y.size.cmp(&x.size))
                    .then_with(|| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()))
            });
            m.into_iter().map(|(p, _)| p).collect()
        })
        .collect()
}

/// `p` as it is on disk: DICOMDIR file IDs are upper case, the files may not be.
fn resolve_case(p: &Path) -> Option<PathBuf> {
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    let mut cur = PathBuf::new();
    for c in p.components() {
        let next = cur.join(c);
        if next.exists() {
            cur = next;
            continue;
        }
        let want = c.as_os_str().to_string_lossy().to_lowercase();
        let found = std::fs::read_dir(if cur.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &cur
        })
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().to_lowercase() == want)?;
        cur = found.path();
    }
    cur.is_file().then_some(cur)
}

/// File name order with digit runs compared as numbers (`IM2` before `IM10`).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.as_bytes(), b.as_bytes());
    loop {
        match (x.first(), y.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let n = x.iter().take_while(|b| b.is_ascii_digit()).count();
                let m = y.iter().take_while(|b| b.is_ascii_digit()).count();
                let (p, q) = (&x[..n], &y[..m]);
                let strip = |s: &[u8]| -> usize { s.iter().take_while(|b| **b == b'0').count() };
                let (p2, q2) = (&p[strip(p)..], &q[strip(q)..]);
                let o = p2
                    .len()
                    .cmp(&q2.len())
                    .then_with(|| p2.cmp(q2))
                    .then(n.cmp(&m));
                if o != Ordering::Equal {
                    return o;
                }
                x = &x[n..];
                y = &y[m..];
            }
            (Some(c), Some(d)) => {
                if c != d {
                    return c.cmp(d);
                }
                x = &x[1..];
                y = &y[1..];
            }
        }
    }
}

/// Frame positions of an RT Dose grid from its Grid Frame Offset Vector (PS3.3 C.8.8.3.2):
/// offsets along the slice normal from the first frame when the first offset is 0, else the
/// frames' z coordinates (for axial grids). Empty without a vector of one offset per frame.
fn grid_frame_positions(
    ds: &DataSet,
    v: &Values,
    p: [f64; 3],
    o: [f64; 6],
    frames: u32,
) -> Vec<[f64; 3]> {
    let Some(e) = ds.get(0x3004_000C) else {
        return Vec::new();
    };
    let offsets: Vec<f64> = (0..v.count(e)).filter_map(|i| v.number(e, i)).collect();
    if frames < 2 || offsets.len() != frames as usize {
        return Vec::new();
    }
    let Some(n) =
        crate::volume::normalise(crate::volume::cross([o[0], o[1], o[2]], [o[3], o[4], o[5]]))
    else {
        return Vec::new();
    };
    if offsets[0] == 0.0 {
        offsets
            .iter()
            .map(|d| [p[0] + d * n[0], p[1] + d * n[1], p[2] + d * n[2]])
            .collect()
    } else {
        offsets.iter().map(|&z| [p[0], p[1], z]).collect()
    }
}

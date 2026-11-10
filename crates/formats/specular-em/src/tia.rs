//! FEI / Thermo Fisher TIA series files (`.ser`) and their `.emi` metadata.
//!
//! A `.ser` file is a little-endian series header (byte order 0x4949, series ID 0x0197,
//! version 0x0210 with 32-bit offsets or 0x0220 with 64-bit ones), the calibrated dimensions
//! the elements were acquired along (none, a line, a scan grid, a frame series), and per
//! element a data block: a 2-D image (data type 0x4122: x / y calibration, rows stored
//! bottom-up) or a 1-D spectrum (0x4120: energy calibration). Elements may carry a tag with the
//! acquisition time and beam position.
//!
//! Images become one page whose planes are the elements (a frame series or a grid of
//! diffraction patterns). Spectra become a page whose single plane is the total counts per
//! position (a spectrum image, a line profile, or a few points along one row) with the
//! spectrum at each position served through [`Spectra`].
//!
//! The `.emi` beside the series (`name.emi` for `name_1.ser`, `name_2.ser`, …) holds an
//! `<ObjectInfo>` XML block per series: microscope, high tension, mode, magnification,
//! camera, dwell time and acquisition date. It is optional.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::dataset::{Axis, AxisKind, Dataset, Field, Lateral, Page, Plane, PlaneData, Spectra};
use crate::samples::{self, Order, Sample};
use crate::xml::{self, Doc};
use crate::{Error, MAX_PLANE, Result};

const SERIES_ID: u16 = 0x0197;
const SPECTRUM: u32 = 0x4120;
const IMAGE: u32 = 0x4122;
const TAG_TIME_POSITION: u16 = 0x4142;

/// Whether `head` starts a TIA series file.
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 10
        && head[0..2] == [0x49, 0x49]
        && u16le(head, 2) == Some(SERIES_ID)
        && matches!(u16le(head, 4), Some(0x0210 | 0x0220))
        && matches!(u32le(head, 6), Some(SPECTRUM | IMAGE))
}

/// The `.emi` that describes `ser` and the index of its object there: `a_2.ser` is described
/// by `a.emi`, object 1.
pub fn sidecar(ser: &Path) -> Option<(PathBuf, usize)> {
    let stem = ser.file_stem()?.to_str()?;
    let (base, n) = stem.rsplit_once('_')?;
    let n: usize = n.parse().ok()?;
    if n == 0 || base.is_empty() {
        return None;
    }
    Some((ser.with_file_name(format!("{base}.emi")), n - 1))
}

/// The `n`-th series (from 1) of an `.emi`: `a.emi` → `a_n.ser`.
pub fn series_path(emi: &Path, n: usize) -> Option<PathBuf> {
    let stem = emi.file_stem()?.to_str()?;
    Some(emi.with_file_name(format!("{stem}_{n}.ser")))
}

/// The `<ObjectInfo>` XML blocks of an `.emi`, in file order (one per series).
pub fn emi_objects(emi: &[u8]) -> Vec<&[u8]> {
    const OPEN: &[u8] = b"<ObjectInfo>";
    const CLOSE: &[u8] = b"</ObjectInfo>";
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(s) = find(emi, i, OPEN) {
        let Some(e) = find(emi, s, CLOSE) else { break };
        out.push(&emi[s..e + CLOSE.len()]);
        i = e + CLOSE.len();
    }
    out
}

/// Reads a series with the first object of its `.emi` (when given).
pub fn read(ser: &[u8], emi: Option<&[u8]>) -> Result<Dataset> {
    read_indexed(ser, emi, 0)
}

/// Reads a series with object `object` of its `.emi` (see [`sidecar`]).
pub fn read_indexed(ser: &[u8], emi: Option<&[u8]>, object: usize) -> Result<Dataset> {
    let h = Header::parse(ser)?;
    let info = emi
        .and_then(|e| emi_objects(e).get(object).copied())
        .map(Info::parse)
        .unwrap_or_default();
    let page = match h.data_type {
        IMAGE => images(ser, &h, &info)?,
        _ => spectra(ser, &h, &info)?,
    };
    Ok(Dataset { pages: vec![page] })
}

#[derive(Clone, Debug)]
struct Dim {
    size: usize,
    offset: f64,
    delta: f64,
    element: f64,
    description: String,
    units: String,
}

#[derive(Debug)]
struct Header {
    data_type: u32,
    tag_type: u32,
    valid: usize,
    dims: Vec<Dim>,
    data: Vec<usize>,
    tags: Vec<usize>,
}

impl Header {
    fn parse(b: &[u8]) -> Result<Header> {
        if !probe(b) {
            return Err(Error::Unsupported("not a TIA series".into()));
        }
        let wide = u16le(b, 4) == Some(0x0220);
        let data_type = u32le(b, 6).ok_or(Error::Truncated("TIA header"))?;
        let tag_type = u32le(b, 10).ok_or(Error::Truncated("TIA header"))?;
        let total = u32le(b, 14).ok_or(Error::Truncated("TIA header"))? as usize;
        let valid = u32le(b, 18).ok_or(Error::Truncated("TIA header"))? as usize;
        let (table, ndim, mut p) = if wide {
            (u64le(b, 22), u32le(b, 30), 34)
        } else {
            (u32le(b, 22).map(u64::from), u32le(b, 26), 30)
        };
        let table = table.ok_or(Error::Truncated("TIA header"))?;
        let ndim = ndim.ok_or(Error::Truncated("TIA header"))? as usize;
        if ndim > 8 || total > b.len() || valid > total {
            return Err(Error::Malformed("TIA series header".into()));
        }
        let mut dims = Vec::with_capacity(ndim);
        for _ in 0..ndim {
            let size = u32le(b, p).ok_or(Error::Truncated("TIA dimension"))? as usize;
            let offset = f64le(b, p + 4).ok_or(Error::Truncated("TIA dimension"))?;
            let delta = f64le(b, p + 12).ok_or(Error::Truncated("TIA dimension"))?;
            let element = u32le(b, p + 20).ok_or(Error::Truncated("TIA dimension"))?;
            let dl = u32le(b, p + 24).ok_or(Error::Truncated("TIA dimension"))? as usize;
            let description = text(b, p + 28, dl)?;
            let q = p + 28 + dl;
            let ul = u32le(b, q).ok_or(Error::Truncated("TIA dimension"))? as usize;
            let units = text(b, q + 4, ul)?;
            p = q + 4 + ul;
            dims.push(Dim {
                size,
                offset,
                delta,
                element: f64::from(element),
                description,
                units,
            });
        }
        let table = usize::try_from(table).map_err(|_| Error::Malformed("offset".into()))?;
        let word = if wide { 8 } else { 4 };
        let read = |k: usize| -> Option<usize> {
            let at = table.checked_add(k.checked_mul(word)?)?;
            let v = if wide {
                u64le(b, at)?
            } else {
                u64::from(u32le(b, at)?)
            };
            usize::try_from(v).ok().filter(|&v| v != 0 && v < b.len())
        };
        let mut data = Vec::with_capacity(valid);
        let mut tags = Vec::with_capacity(valid);
        for k in 0..valid {
            data.push(read(k).unwrap_or(usize::MAX));
            tags.push(read(total + k).unwrap_or(usize::MAX));
        }
        Ok(Header {
            data_type,
            tag_type,
            valid,
            dims,
            data,
            tags,
        })
    }

    /// The beam position stored in element `k`'s tag.
    fn position(&self, b: &[u8], k: usize) -> Option<(f64, f64)> {
        if self.tag_type != u32::from(TAG_TIME_POSITION) {
            return None;
        }
        let t = *self.tags.get(k)?;
        if u16le(b, t)? != TAG_TIME_POSITION {
            return None;
        }
        Some((f64le(b, t + 8)?, f64le(b, t + 16)?))
    }
}

fn sample(code: u16) -> Result<Sample> {
    Ok(match code {
        1 => Sample::U8,
        2 => Sample::U16,
        3 => Sample::U32,
        4 => Sample::I8,
        5 => Sample::I16,
        6 => Sample::I32,
        7 => Sample::F32,
        8 => Sample::F64,
        9 => Sample::ComplexF32,
        10 => Sample::ComplexF64,
        c => return Err(Error::Unsupported(format!("TIA data type {c}"))),
    })
}

/// The `.emi` facts about one series.
#[derive(Clone, Debug, Default)]
struct Info {
    instrument: Option<String>,
    voltage: Option<f64>,
    magnification: Option<f64>,
    mode: Option<String>,
    camera: Option<String>,
    dwell: Option<f64>,
    date: Option<String>,
    meta: Vec<(String, String)>,
}

impl Info {
    fn parse(obj: &[u8]) -> Info {
        let d = Doc::parse(obj);
        let mut info = Info::default();
        let Some(root) = d.child(d.root(), "ObjectInfo") else {
            return info;
        };
        if let Some(desc) = d.path(root, "ExperimentalDescription/Root") {
            for data in d.children(desc, "Data") {
                let label = d.text(data, "Label").unwrap_or_default();
                let value = d.text(data, "Value").unwrap_or_default();
                let unit = d.text(data, "Unit").unwrap_or_default();
                let v = xml::number(value);
                match (label, unit) {
                    ("Microscope", _) => {
                        let m = value.strip_prefix("Microscope ").unwrap_or(value);
                        info.instrument = Some(m.trim().to_string()).filter(|s| !s.is_empty());
                    }
                    ("High tension", "kV") => info.voltage = v.map(|v| v * 1e3),
                    ("Magnification", "x") => info.magnification = v,
                    ("Mode", _) => {
                        info.mode = Some(value.trim().to_string()).filter(|s| !s.is_empty());
                    }
                    _ => {}
                }
                if label.is_empty() {
                    continue;
                }
                let key = if unit.is_empty() {
                    label.to_string()
                } else {
                    format!("{label} ({unit})")
                };
                info.meta.push((key, value.to_string()));
            }
        }
        if info.voltage.is_none() {
            info.voltage = d
                .text(
                    root,
                    "ExperimentalConditions/MicroscopeConditions/AcceleratingVoltage",
                )
                .and_then(xml::number);
        }
        info.date = d
            .text(root, "AcquireDate")
            .map(str::to_string)
            .filter(|s| !s.is_empty());
        if let Some(acq) = d.child(root, "AcquireInfo") {
            info.camera = d
                .text(acq, "CameraNamePath")
                .map(str::to_string)
                .filter(|s| !s.is_empty());
            info.dwell = d.text(acq, "DwellTimePath").and_then(xml::number);
            for &c in &d.el(acq).children {
                let el = d.el(c);
                if el.children.is_empty() && !el.text.trim().is_empty() {
                    info.meta
                        .push((el.name.clone(), el.text.trim().to_string()));
                }
            }
        }
        if let Some(t) = d.text(root, "Manufacturer").filter(|t| !t.is_empty()) {
            info.meta.push(("Manufacturer".into(), t.to_string()));
        }
        info
    }

    fn is_camera(&self) -> bool {
        self.camera.is_some()
    }

    fn apply(&self, page: &mut Page) {
        page.instrument = self.instrument.clone();
        page.voltage = self.voltage;
        page.magnification = self.magnification;
        page.mode = self.mode.clone();
        page.detector = self.camera.clone();
        page.exposure = self.dwell;
        page.date = self.date.clone();
        page.meta = self.meta.clone();
    }
}

/// Unit of an image's pixel calibration: SI metres, or reciprocal metres for diffraction
/// (a TEM diffraction mode, or STEM recorded on a camera or as a scan of patterns).
fn image_unit(h: &Header, info: &Info) -> &'static str {
    let Some(mode) = &info.mode else { return "m" };
    let Some(d) = h.dims.first() else { return "m" };
    let stack = d.units.is_empty();
    let scan = d.size > 1 && !stack;
    if mode.contains("STEM") {
        if info.is_camera() || scan { "1/m" } else { "m" }
    } else if mode.contains("Diffraction") {
        "1/m"
    } else {
        "m"
    }
}

const IMAGE_HEAD: usize = 50;
const SPECTRUM_HEAD: usize = 26;

fn images(b: &[u8], h: &Header, info: &Info) -> Result<Page> {
    let first = *h.data.first().ok_or(Error::NoImage)?;
    let at = |o: usize| f64le(b, first + o).ok_or(Error::Truncated("TIA image element"));
    let (ox, dx, ex) = (at(0)?, at(8)?, f64::from(u32le(b, first + 16).unwrap_or(0)));
    let (oy, dy, ey) = (
        at(20)?,
        at(28)?,
        f64::from(u32le(b, first + 36).unwrap_or(0)),
    );
    let code = u16le(b, first + 40).ok_or(Error::Truncated("TIA image element"))?;
    let s = sample(code)?;
    let w = u32le(b, first + 42).ok_or(Error::Truncated("TIA image element"))? as usize;
    let ht = u32le(b, first + 46).ok_or(Error::Truncated("TIA image element"))? as usize;
    if w == 0 || ht == 0 || (w as u64) * (ht as u64) > MAX_PLANE {
        return Err(Error::TooLarge);
    }
    let unit = image_unit(h, info);
    let mut planes = Vec::with_capacity(h.valid);
    for (k, &off) in h.data.iter().enumerate() {
        let same = off != usize::MAX
            && u16le(b, off + 40) == Some(code)
            && u32le(b, off + 42) == Some(w as u32)
            && u32le(b, off + 46) == Some(ht as u32);
        let data = if same {
            PlaneData::Stored {
                offset: off + IMAGE_HEAD,
                sample: s,
                order: Order::Little,
                bottom_up: true,
                scale: 1.0,
                add: 0.0,
            }
        } else {
            // A missing element: no values (they all read as NaN), not a w × h buffer each.
            PlaneData::Values(Arc::from(Vec::<f32>::new()))
        };
        let label = if h.valid > 1 {
            format!("{}", k + 1)
        } else {
            String::new()
        };
        planes.push(Plane {
            label,
            unit: String::new(),
            data,
        });
    }
    let axis = series_axis(h);
    let mut page = Page {
        field: Field::ElectronMicroscopy,
        width: w,
        height: ht,
        planes,
        axis,
        x: Some(Lateral::new(ox - ex * dx, dx, unit)).filter(Lateral::is_usable),
        y: Some(Lateral::new(oy - ey * dy, dy, unit)).filter(Lateral::is_usable),
        sample: Some(s),
        ..Page::default()
    };
    info.apply(&mut page);
    Ok(page)
}

/// The axis an image series' elements lie along.
fn series_axis(h: &Header) -> Axis {
    let mut axis = Axis {
        kind: AxisKind::Series,
        name: String::new(),
        linear: None,
    };
    if let [d] = h.dims.as_slice() {
        axis.name = d.description.clone();
        if !d.units.is_empty() && d.delta != 0.0 && d.delta.is_finite() {
            let u = if d.units == "meters" { "m" } else { &d.units };
            axis.linear = Some(Lateral::new(d.offset - d.element * d.delta, d.delta, u));
        }
    } else if h.dims.len() > 1 {
        axis.name = "Position".into();
    }
    axis
}

/// The spectrum per position of a spectrum series.
#[derive(Debug)]
struct TiaSpectra {
    width: usize,
    height: usize,
    channels: usize,
    sample: Sample,
    axis: Lateral,
    /// Start of each position's samples (row-major over the grid); `usize::MAX` when not
    /// acquired.
    starts: Vec<usize>,
}

impl Spectra for TiaSpectra {
    fn grid(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn axis(&self) -> Lateral {
        self.axis.clone()
    }

    fn at(&self, file: &[u8], x: usize, y: usize) -> Option<Vec<f32>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let s = *self.starts.get(y * self.width + x)?;
        let bytes = file.get(s..)?;
        let mut out = vec![0f32; self.channels];
        samples::convert(
            bytes,
            self.sample,
            Order::Little,
            self.sample.bytes(),
            &mut out,
        );
        Some(out)
    }
}

fn spectra(b: &[u8], h: &Header, info: &Info) -> Result<Page> {
    let first = *h.data.first().ok_or(Error::NoImage)?;
    let off = f64le(b, first).ok_or(Error::Truncated("TIA spectrum element"))?;
    let delta = f64le(b, first + 8).ok_or(Error::Truncated("TIA spectrum element"))?;
    let element = f64::from(u32le(b, first + 16).unwrap_or(0));
    let code = u16le(b, first + 20).ok_or(Error::Truncated("TIA spectrum element"))?;
    let s = sample(code)?;
    let channels = u32le(b, first + 22).ok_or(Error::Truncated("TIA spectrum element"))? as usize;
    // A spectrum's samples lie in the file: no more channels than its bytes can hold.
    if channels == 0 || channels as u64 > MAX_PLANE || channels > b.len() / s.bytes() {
        return Err(Error::Malformed("TIA spectrum length".into()));
    }
    // The grid: a scan of two dimensions (x fastest when the second element shares the
    // first's y), a line of one, or the positions present.
    let x_fast = h.valid > 1
        && match (h.position(b, 0), h.position(b, 1)) {
            (Some((_, y0)), Some((_, y1))) => y0 == y1,
            _ => false,
        };
    let cal = |d: &Dim| {
        let u = match d.units.as_str() {
            "meters" => "m",
            u => u,
        };
        Some(Lateral::new(d.offset, d.delta, u)).filter(Lateral::is_usable)
    };
    let (width, height, x, y) = match h.dims.as_slice() {
        [d1, d2, ..] if d1.size > 0 && d2.size > 0 => {
            if x_fast {
                (d1.size, d2.size, cal(d1), cal(d2))
            } else {
                (d2.size, d1.size, cal(d2), cal(d1))
            }
        }
        [d] if d.size > 0 => (h.valid.min(d.size).max(1), 1, cal(d), None),
        _ => (h.valid.max(1), 1, None, None),
    };
    let n = width
        .checked_mul(height)
        .filter(|&n| n as u64 <= MAX_PLANE)
        .ok_or(Error::TooLarge)?;
    // Only the elements present have starts (positions past them read as not acquired), so
    // a grid larger than the file holds costs nothing here.
    let mut starts = vec![usize::MAX; n.min(h.data.len())];
    for (k, &o) in h.data.iter().enumerate().take(n) {
        if o != usize::MAX
            && u16le(b, o + 20) == Some(code)
            && u32le(b, o + 22) == Some(channels as u32)
        {
            starts[k] = o + SPECTRUM_HEAD;
        }
    }
    let sp = TiaSpectra {
        width,
        height,
        channels,
        sample: s,
        axis: Lateral::new(off - element * delta, delta, "eV"),
        starts,
    };
    let mut totals: Vec<f32> = {
        use rayon::prelude::*;
        (0..sp.starts.len())
            .into_par_iter()
            .map(|i| match sp.at(b, i % width, i / width) {
                Some(v) if sp.starts[i] != usize::MAX => {
                    v.iter().map(|&c| f64::from(c)).sum::<f64>() as f32
                }
                _ => f32::NAN,
            })
            .collect()
    };
    // Positions not acquired are NaN; a grid far larger than the file is left short (it
    // reads as NaN all the same) rather than padded to gigabytes.
    if n <= b.len() {
        totals.resize(n, f32::NAN);
    }
    let mut page = Page {
        field: Field::ElectronMicroscopy,
        width,
        height,
        planes: vec![Plane::values("Total counts", "", totals)],
        x,
        y,
        sample: Some(s),
        spectra: Some(Arc::new(sp)),
        ..Page::default()
    };
    info.apply(&mut page);
    Ok(page)
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    b.get(from..)?
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|k| from + k)
}

fn text(b: &[u8], at: usize, len: usize) -> Result<String> {
    let s = b
        .get(at..at.checked_add(len).ok_or(Error::Truncated("TIA text"))?)
        .ok_or(Error::Truncated("TIA text"))?;
    Ok(String::from_utf8_lossy(s)
        .trim_end_matches('\0')
        .to_string())
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64le(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        b.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn f64le(b: &[u8], at: usize) -> Option<f64> {
    u64le(b, at).map(f64::from_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version-0x0210 series of `n` elements written by hand.
    fn series(data_type: u32, dims: &[(u32, f64, f64, &str)], elements: &[Vec<u8>]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&[0x49, 0x49]);
        b.extend_from_slice(&SERIES_ID.to_le_bytes());
        b.extend_from_slice(&0x0210u16.to_le_bytes());
        b.extend_from_slice(&data_type.to_le_bytes());
        b.extend_from_slice(&u32::from(TAG_TIME_POSITION).to_le_bytes());
        let n = elements.len() as u32;
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        let table_at = b.len();
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(dims.len() as u32).to_le_bytes());
        for &(size, off, delta, units) in dims {
            b.extend_from_slice(&size.to_le_bytes());
            b.extend_from_slice(&off.to_le_bytes());
            b.extend_from_slice(&delta.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&(units.len() as u32).to_le_bytes());
            b.extend_from_slice(units.as_bytes());
        }
        let mut offs = Vec::new();
        let mut tags = Vec::new();
        for (k, e) in elements.iter().enumerate() {
            offs.push(b.len() as u32);
            b.extend_from_slice(e);
            tags.push(b.len() as u32);
            b.extend_from_slice(&TAG_TIME_POSITION.to_le_bytes());
            b.extend_from_slice(&0u16.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&((k % 2) as f64).to_le_bytes());
            b.extend_from_slice(&((k / 2) as f64).to_le_bytes());
        }
        let table = b.len() as u32;
        b[table_at..table_at + 4].copy_from_slice(&table.to_le_bytes());
        for o in offs.iter().chain(&tags) {
            b.extend_from_slice(&o.to_le_bytes());
        }
        b
    }

    fn image(w: u32, h: u32, base: u16) -> Vec<u8> {
        let mut e = Vec::new();
        for v in [0.0f64, 2e-9] {
            e.extend_from_slice(&v.to_le_bytes());
        }
        e.extend_from_slice(&0u32.to_le_bytes());
        for v in [0.0f64, 2e-9] {
            e.extend_from_slice(&v.to_le_bytes());
        }
        e.extend_from_slice(&0u32.to_le_bytes());
        e.extend_from_slice(&2u16.to_le_bytes());
        e.extend_from_slice(&w.to_le_bytes());
        e.extend_from_slice(&h.to_le_bytes());
        for i in 0..w * h {
            e.extend_from_slice(&(base + i as u16).to_le_bytes());
        }
        e
    }

    fn spectrum(values: &[f32]) -> Vec<u8> {
        let mut e = Vec::new();
        e.extend_from_slice(&100.0f64.to_le_bytes());
        e.extend_from_slice(&10.0f64.to_le_bytes());
        e.extend_from_slice(&0u32.to_le_bytes());
        e.extend_from_slice(&7u16.to_le_bytes());
        e.extend_from_slice(&(values.len() as u32).to_le_bytes());
        for v in values {
            e.extend_from_slice(&v.to_le_bytes());
        }
        e
    }

    #[test]
    fn image_series_rows_flipped() {
        let b = series(
            IMAGE,
            &[(2, 0.0, 1.0, "")],
            &[image(2, 2, 0), image(2, 2, 10)],
        );
        assert!(probe(&b));
        let d = read(&b, None).unwrap();
        let p = &d.pages[0];
        assert_eq!((p.width, p.height, p.planes.len()), (2, 2, 2));
        assert_eq!(p.read_plane(&b, 0).unwrap().0, [2.0, 3.0, 0.0, 1.0]);
        assert_eq!(p.read_plane(&b, 1).unwrap().0, [12.0, 13.0, 10.0, 11.0]);
        assert_eq!(p.x.as_ref().unwrap().unit, "m");
        assert_eq!(p.axis.kind, AxisKind::Series);
    }

    #[test]
    fn spectrum_image_grid_and_totals() {
        let els: Vec<_> = (0..4).map(|k| spectrum(&[k as f32, 1.0, 2.0])).collect();
        let b = series(
            SPECTRUM,
            &[(2, 0.0, 1e-9, "meters"), (2, 0.0, 1e-9, "meters")],
            &els,
        );
        let d = read(&b, None).unwrap();
        let p = &d.pages[0];
        assert_eq!((p.width, p.height), (2, 2));
        assert_eq!(p.read_plane(&b, 0).unwrap().0, [3.0, 4.0, 5.0, 6.0]);
        let s = p.spectra.as_ref().unwrap();
        assert_eq!(s.at(&b, 1, 1).unwrap(), [3.0, 1.0, 2.0]);
        assert_eq!(s.axis(), Lateral::new(100.0, 10.0, "eV"));
        assert!(s.at(&b, 2, 0).is_none());
    }

    #[test]
    fn emi_metadata_and_sidecars() {
        let emi = b"junk<ObjectInfo><ExperimentalDescription><Root><Data><Label>Microscope</Label>\
            <Value>Microscope Tecnai 200</Value><Unit></Unit></Data><Data><Label>High tension</Label>\
            <Value>200</Value><Unit>kV</Unit></Data><Data><Label>Mode</Label><Value> TEM Diffraction</Value>\
            <Unit></Unit></Data></Root></ExperimentalDescription><AcquireDate>Sun Feb 21 2016</AcquireDate>\
            <AcquireInfo><DwellTimePath>0.5</DwellTimePath><CameraNamePath>Orius</CameraNamePath></AcquireInfo>\
            </ObjectInfo>more<ObjectInfo></ObjectInfo>";
        assert_eq!(emi_objects(emi).len(), 2);
        let b = series(IMAGE, &[(1, 0.0, 1.0, "")], &[image(2, 2, 0)]);
        let p = &read(&b, Some(emi)).unwrap().pages[0];
        assert_eq!(p.instrument.as_deref(), Some("Tecnai 200"));
        assert_eq!(p.voltage, Some(200e3));
        assert_eq!(p.exposure, Some(0.5));
        assert_eq!(p.detector.as_deref(), Some("Orius"));
        assert_eq!(p.x.as_ref().unwrap().unit, "1/m");
        assert_eq!(
            sidecar(Path::new("/d/a_b_2.ser")),
            Some((PathBuf::from("/d/a_b.emi"), 1))
        );
        assert_eq!(sidecar(Path::new("/d/a.ser")), None);
        assert_eq!(
            series_path(Path::new("/d/a.emi"), 1),
            Some(PathBuf::from("/d/a_1.ser"))
        );
    }

    #[test]
    fn truncated_never_panics() {
        let b = series(
            IMAGE,
            &[(2, 0.0, 1.0, "")],
            &[image(4, 4, 0), image(4, 4, 1)],
        );
        for n in 0..b.len() {
            let _ = read(&b[..n], None);
        }
        let els: Vec<_> = (0..4).map(|_| spectrum(&[1.0; 8])).collect();
        let b = series(
            SPECTRUM,
            &[(2, 0.0, 1.0, "meters"), (2, 0.0, 1.0, "meters")],
            &els,
        );
        for n in 0..b.len() {
            if let Ok(d) = read(&b[..n], None) {
                let p = &d.pages[0];
                let _ = p.read_plane(&b[..n], 0);
                let _ = p.spectra.as_ref().unwrap().at(&b[..n], 1, 1);
            }
        }
    }

    mod hostile {
        use super::*;

        /// An image element's header claiming `w × h` with no pixels after it.
        fn bare_image(w: u32, h: u32) -> Vec<u8> {
            let mut e = image(1, 1, 0);
            e.truncate(e.len() - 2);
            e[42..46].copy_from_slice(&w.to_le_bytes());
            e[46..50].copy_from_slice(&h.to_le_bytes());
            e
        }

        #[test]
        fn missing_elements_of_a_huge_image_cost_nothing() {
            // 2^30 pixels declared; the second element differs, so it is "missing".
            let b = series(
                IMAGE,
                &[(2, 0.0, 1.0, "")],
                &[bare_image(32768, 32768), image(1, 1, 0)],
            );
            let d = read(&b, None).unwrap();
            match &d.pages[0].planes[1].data {
                PlaneData::Values(v) => assert!(v.is_empty()),
                PlaneData::Stored { .. } => panic!("element 1 is missing"),
            }
        }

        #[test]
        fn spectrum_longer_than_the_file_is_refused() {
            let mut e = spectrum(&[1.0]);
            e[22..26].copy_from_slice(&(1u32 << 29).to_le_bytes());
            let b = series(SPECTRUM, &[(1, 0.0, 1.0, "")], &[e]);
            assert!(read(&b, None).is_err());
        }

        #[test]
        fn huge_spectrum_grid_with_one_element() {
            let b = series(
                SPECTRUM,
                &[(32768, 0.0, 1.0, "meters"), (32768, 0.0, 1.0, "meters")],
                &[spectrum(&[1.0, 2.0])],
            );
            let d = read(&b, None).unwrap();
            let p = &d.pages[0];
            assert_eq!((p.width, p.height), (32768, 32768));
            let s = p.spectra.as_ref().unwrap();
            assert_eq!(s.at(&b, 0, 0).unwrap(), [1.0, 2.0]);
            assert!(s.at(&b, 5, 5).is_none());
        }
    }
}

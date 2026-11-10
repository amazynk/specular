//! Princeton Instruments SPE: a 4100-byte header, then the frames.
//!
//! - **2.x** (WinView/WinSpec): the frame size (x at byte 42, y at 656), sample type (108),
//!   frame count (1446), exposure in seconds (10), detector temperature (36), gain (198) and
//!   the x calibration (from 3000: valid flag, polynomial order and coefficients at 3263,
//!   over pixel numbers from 1).
//! - **3.0** (LightField): the same header (version at 1992) and an XML footer (offset at
//!   678) describing the frame's regions (`DataFormat/DataBlock`), the wavelength of each
//!   sensor column (`Calibrations/WavelengthMapping`), where each region lies on the sensor
//!   and its binning (`SensorMapping`), the exposure, temperature set point and gain.
//!
//! Sample types: 0 float32, 1 int32, 2 int16, 3 uint16, 5 float64, 6 uint8, 8 uint32.

use std::sync::Arc;

use specular_wsi::xml::{self, Element};

use crate::spectral::{Num, Page, Spectral, polynomial};
use crate::{Error, Result};

/// Where the frames start.
pub const DATA_START: usize = 4100;
/// Most pages read.
const MAX_PAGES: usize = 1 << 20;

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn i16_at(b: &[u8], o: usize) -> Option<i16> {
    u16_at(b, o).map(|v| v as i16)
}
fn i32_at(b: &[u8], o: usize) -> Option<i32> {
    crate::le_u32(b, o).map(|v| v as i32)
}
fn f32_at(b: &[u8], o: usize) -> Option<f32> {
    crate::le_u32(b, o).map(f32::from_bits)
}
fn f64_at(b: &[u8], o: usize) -> Option<f64> {
    crate::le_u64(b, o).map(f64::from_bits)
}

/// Whether `data` looks like an SPE file: a known sample type, sizes that fit, and either
/// a 3.0 footer inside the file or 2.x frames that fill it.
pub fn sniff(data: &[u8]) -> bool {
    if data.len() < DATA_START {
        return false;
    }
    let Some(num) = i16_at(data, 108).and_then(num_of) else {
        return false;
    };
    let ver = f32_at(data, 1992).unwrap_or(0.0);
    if (3.0..4.0).contains(&ver) {
        let off = crate::le_u64(data, 678).unwrap_or(0);
        return off as usize > DATA_START
            && (off as usize) < data.len()
            && data[off as usize..].starts_with(b"<");
    }
    let (x, y) = (
        u16_at(data, 42).unwrap_or(0) as usize,
        u16_at(data, 656).unwrap_or(0) as usize,
    );
    let n = i32_at(data, 1446).unwrap_or(0);
    if x == 0 || y == 0 || n <= 0 {
        return false;
    }
    let need = x * y * num.bytes() * n as usize;
    // WinView writes the frames exactly; allow a short tail.
    data.len() >= DATA_START + need && data.len() <= DATA_START + need + 4096
}

fn num_of(code: i16) -> Option<Num> {
    Some(match code {
        0 => Num::F32,
        1 => Num::I32,
        2 => Num::I16,
        3 => Num::U16,
        5 => Num::F64,
        6 => Num::U8,
        8 => Num::U32,
        _ => return None,
    })
}

fn text(e: &Element) -> Option<String> {
    Some(e.text.trim().to_owned()).filter(|t| !t.is_empty())
}

fn descendants<'a>(e: &'a Element, name: &str, out: &mut Vec<&'a Element>) {
    for c in &e.children {
        if c.name == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

fn all<'a>(e: &'a Element, name: &str) -> Vec<&'a Element> {
    let mut v = Vec::new();
    descendants(e, name, &mut v);
    v
}

pub fn parse(data: &[u8]) -> Result<Spectral> {
    if !sniff(data) {
        return Err(Error::Malformed("not an SPE file".into()));
    }
    let num = i16_at(data, 108)
        .and_then(num_of)
        .ok_or_else(|| Error::Unsupported("SPE sample type".into()))?;
    let ver = f32_at(data, 1992).unwrap_or(0.0);
    let mut s = Spectral::default();
    let frames = i32_at(data, 1446).unwrap_or(0).max(0) as usize;
    s.frames = frames;
    if ver >= 3.0 {
        footer(data, num, &mut s)?;
    } else {
        s.version = "SPE 2.x".into();
        let (x, y) = (
            u16_at(data, 42).unwrap_or(0) as usize,
            u16_at(data, 656).unwrap_or(0) as usize,
        );
        let wl = calibration_v2(data, x);
        if frames > MAX_PAGES {
            return Err(Error::TooLarge);
        }
        for f in 0..frames {
            s.pages.push(Page {
                frame: f,
                region: 0,
                width: x,
                height: y,
                offset: DATA_START + f * x * y * num.bytes(),
                sample: num,
                wavelengths: wl.clone(),
                sensor: None,
            });
        }
        let exp = f32_at(data, 10).unwrap_or(0.0);
        if exp > 0.0 && exp.is_finite() {
            s.exposure = Some(f64::from(exp));
        }
        let t = f32_at(data, 36).unwrap_or(0.0);
        if t != 0.0 && t.is_finite() {
            s.temperature = Some(f64::from(t));
        }
        let g = u16_at(data, 198).unwrap_or(0);
        if g > 0 {
            s.gain = Some(g.to_string());
        }
        let date = String::from_utf8_lossy(&data[20..30])
            .trim_matches(['\0', ' '])
            .to_owned();
        if !date.is_empty() {
            s.date = Some(date);
        }
    }
    if s.pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(s)
}

/// The 2.x x calibration over `n` columns.
fn calibration_v2(d: &[u8], n: usize) -> Option<Arc<[f64]>> {
    if d.get(3098).copied().unwrap_or(0) == 0 {
        return None;
    }
    let order = usize::from(d.get(3101).copied().unwrap_or(0)).min(5);
    if order == 0 {
        return None;
    }
    let c: Vec<f64> = (0..=order)
        .filter_map(|k| f64_at(d, 3263 + 8 * k))
        .collect();
    polynomial(&c, n, 1.0)
}

fn footer(data: &[u8], num: Num, s: &mut Spectral) -> Result<()> {
    let off = crate::le_u64(data, 678).unwrap_or(0) as usize;
    let xmltext = String::from_utf8_lossy(data.get(off..).unwrap_or(&[]));
    let root = xml::parse(xmltext.trim_end_matches('\0').trim())
        .map_err(|e| Error::Malformed(e.to_string()))?;
    s.version = format!("SPE {}", root.attr("version").unwrap_or("3.0"));
    let frame = root
        .find("DataFormat")
        .and_then(|d| d.all("DataBlock").find(|b| b.attr("type") == Some("Frame")))
        .ok_or_else(|| Error::Malformed("SPE footer without a frame description".into()))?;
    let count = frame.num("count").unwrap_or(s.frames as f64).max(0.0) as usize;
    let frames = if s.frames == 0 { count } else { s.frames };
    let stride = frame.num("stride").unwrap_or(0.0).max(0.0) as usize;
    let mut regions = Vec::new();
    for r in frame
        .all("DataBlock")
        .filter(|b| b.attr("type") == Some("Region"))
    {
        let w = r.num("width").unwrap_or(0.0).max(0.0) as usize;
        let h = r.num("height").unwrap_or(0.0).max(0.0) as usize;
        let size = r
            .num("size")
            .unwrap_or((w * h * num.bytes()) as f64)
            .max(0.0) as usize;
        let cal: Vec<String> = r
            .attr("calibrations")
            .unwrap_or("")
            .split(',')
            .map(|v| v.trim().to_owned())
            .collect();
        regions.push((w, h, size, cal));
    }
    let frame_bytes: usize = regions.iter().map(|r| r.2).sum();
    let stride = if stride >= frame_bytes {
        stride
    } else {
        frame_bytes
    };
    if regions.is_empty() || frame_bytes == 0 {
        return Err(Error::Malformed("SPE footer without regions".into()));
    }
    // Calibrations by id.
    let cal = root.find("Calibrations");
    let by_id = |id: &str| -> Option<&Element> {
        cal.and_then(|c| c.children.iter().find(|e| e.attr("id") == Some(id)))
    };
    let frame_cal: Vec<String> = frame
        .attr("calibrations")
        .unwrap_or("")
        .split(',')
        .map(|v| v.trim().to_owned())
        .collect();
    let wavelengths: Option<Vec<f64>> = frame_cal
        .iter()
        .chain(regions.iter().flat_map(|r| r.3.iter()))
        .filter_map(|id| by_id(id))
        .find(|e| e.name == "WavelengthMapping")
        .and_then(|m| m.child("Wavelength").or_else(|| m.child("WavelengthError")))
        .and_then(text)
        .map(|t| t.split(',').filter_map(|v| v.trim().parse().ok()).collect());
    let sensor_width = cal
        .and_then(|c| c.child("SensorInformation"))
        .and_then(|e| e.num("width"))
        .unwrap_or(0.0) as usize;
    if frames.saturating_mul(regions.len()) > MAX_PAGES {
        return Err(Error::TooLarge);
    }
    // Per region, once: every frame shares its sensor mapping and wavelengths (a long
    // wavelength list copied per frame and region would take gigabytes).
    let per_region: Vec<_> = regions
        .iter()
        .map(|(w, _, _, ids)| {
            let mapping = ids
                .iter()
                .filter_map(|id| by_id(id))
                .find(|e| e.name == "SensorMapping");
            let sensor = mapping.map(|m| {
                let g = |k: &str| m.num(k).unwrap_or(0.0).max(0.0) as u32;
                [g("x"), g("y"), g("xBinning").max(1), g("yBinning").max(1)]
            });
            let wl = wavelengths
                .as_ref()
                .and_then(|wl| region_wavelengths(wl, *w, sensor, sensor_width));
            (sensor, wl)
        })
        .collect();
    for f in 0..frames {
        let mut o = super::spe::DATA_START + f * stride;
        for (k, (w, h, size, _)) in regions.iter().enumerate() {
            let (sensor, wl) = per_region[k].clone();
            s.pages.push(Page {
                frame: f,
                region: k,
                width: *w,
                height: *h,
                offset: o,
                sample: num,
                wavelengths: wl,
                sensor,
            });
            o += size;
        }
    }
    // Facts.
    if let Some(e) = all(&root, "ExposureTime")
        .into_iter()
        .find_map(|e| text(e)?.parse::<f64>().ok())
    {
        s.exposure = Some(e / 1000.0);
    }
    if let Some(t) = root
        .find("Temperature")
        .and_then(|t| t.child("SetPoint"))
        .and_then(text)
        .and_then(|v| v.parse().ok())
    {
        s.temperature = Some(t);
    }
    if let Some(g) = root.find("AnalogGain").and_then(text) {
        s.gain = Some(g);
    }
    if let Some(g) = root.find("ElectronMultiplierGain").and_then(text) {
        s.facts.push(("EM gain".into(), g));
    }
    if let Some(c) = root.find("Camera") {
        let model = [c.attr("model"), c.attr("serialNumber")]
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if !model.is_empty() {
            s.detector = Some(model);
        }
    }
    if let Some(sp) = root.find("Spectrometer").and_then(|e| e.attr("model")) {
        s.spectrograph = Some(sp.to_owned());
    }
    if let Some(c) = root.find("CenterWavelength").and_then(text) {
        s.facts.push(("Centre wavelength (nm)".into(), c));
    }
    if let Some(g) = root
        .find("Grating")
        .and_then(|g| g.child("Selected"))
        .and_then(text)
    {
        s.facts.push(("Grating".into(), g));
    }
    if let Some(d) = cal
        .and_then(|c| c.child("WavelengthMapping"))
        .and_then(|m| m.attr("date"))
    {
        s.facts.push(("Calibrated".into(), d.to_owned()));
    }
    s.date = root
        .find("Origin")
        .and_then(|o| o.attr("created"))
        .map(str::to_owned);
    Ok(())
}

/// The wavelengths of a region's columns: the mapping as it is when it has one per column,
/// else the mean over each binned group of the sensor columns the region covers.
fn region_wavelengths(
    wl: &[f64],
    w: usize,
    sensor: Option<[u32; 4]>,
    sensor_width: usize,
) -> Option<Arc<[f64]>> {
    if wl.len() == w {
        return Some(wl.into());
    }
    let [x, _, bin, _] = sensor?;
    let (x, bin) = (x as usize, bin.max(1) as usize);
    if wl.len() < sensor_width.max(x + w * bin) {
        return None;
    }
    let v: Vec<f64> = (0..w)
        .map(|i| {
            let s = &wl[x + i * bin..x + (i + 1) * bin];
            s.iter().sum::<f64>() / bin as f64
        })
        .collect();
    Some(v.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2(x: u16, y: u16, frames: i32) -> Vec<u8> {
        let mut b = vec![0u8; DATA_START];
        b[42..44].copy_from_slice(&x.to_le_bytes());
        b[656..658].copy_from_slice(&y.to_le_bytes());
        b[108..110].copy_from_slice(&3i16.to_le_bytes());
        b[1446..1450].copy_from_slice(&frames.to_le_bytes());
        b[10..14].copy_from_slice(&0.5f32.to_le_bytes());
        for i in 0..u32::from(x) * u32::from(y) * frames as u32 {
            b.extend_from_slice(&(i as u16).to_le_bytes());
        }
        b
    }

    #[test]
    fn version_2_frames_and_calibration() {
        let mut b = v2(4, 2, 3);
        assert!(sniff(&b));
        let s = parse(&b).unwrap();
        assert_eq!(s.pages.len(), 3);
        assert_eq!(s.exposure, Some(0.5));
        let (v, t) = s.pages[2].read(&b).unwrap();
        assert!(!t);
        assert_eq!(v[0], 16.0);
        assert!(s.pages[0].wavelengths.is_none());
        // A linear calibration: 500 + 2 nm per pixel (pixels from 1).
        b[3098] = 1;
        b[3101] = 1;
        b[3263..3271].copy_from_slice(&500f64.to_le_bytes());
        b[3271..3279].copy_from_slice(&2f64.to_le_bytes());
        let s = parse(&b).unwrap();
        assert_eq!(
            s.pages[0].wavelengths.as_deref(),
            Some(&[502.0, 504.0, 506.0, 508.0][..])
        );
    }

    /// A page of a 4 KB file declaring 32768 × 32768 pixels (as an SPE 3 footer can; 4 GiB
    /// of values): reserved from the memory budget before the plane is allocated, so a small
    /// budget refuses it.
    #[test]
    fn declared_plane_is_reserved() {
        let b = v2(4, 2, 1);
        let mut page = parse(&b).unwrap().pages[0].clone();
        page.width = 32768;
        page.height = 32768;
        let budget = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(64 << 20));
        let r = specular_core::budget::with_budget(budget.clone(), || page.read(&b));
        assert!(
            matches!(r, Err(Error::NeedsMemory(_))),
            "{:?}",
            r.map(|v| v.1)
        );
        assert_eq!(budget.reserved(), 0);
        // A plane that fits stays reserved while it is kept.
        let b = v2(4, 2, 1);
        let s = parse(&b).unwrap();
        let (v, _, held) =
            specular_core::budget::with_budget(budget.clone(), || s.pages[0].read_reserved(&b))
                .unwrap();
        assert_eq!(budget.reserved(), 4 * v.len() as u64);
        drop(held);
        assert_eq!(budget.reserved(), 0);
    }

    #[test]
    fn not_spe() {
        assert!(!sniff(&[0u8; 100]));
        let mut b = v2(4, 2, 3);
        b.truncate(b.len() - 10);
        assert!(!sniff(&b));
        let mut c = v2(4, 2, 3);
        c[108..110].copy_from_slice(&99i16.to_le_bytes());
        assert!(!sniff(&c));
    }

    #[test]
    fn binned_regions_average_their_columns() {
        let wl: Vec<f64> = (0..8).map(f64::from).collect();
        let r = region_wavelengths(&wl, 2, Some([2, 0, 2, 1]), 8).unwrap();
        assert_eq!(&*r, &[2.5, 4.5]);
        assert!(region_wavelengths(&wl, 5, Some([2, 0, 2, 1]), 8).is_none());
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A million frames sharing a 4096-value wavelength list used to copy it per frame
    /// (32 GB); every page now shares one list.
    #[test]
    fn wavelengths_are_shared_between_frames() {
        let mut b = vec![0u8; DATA_START];
        b[108..110].copy_from_slice(&3i16.to_le_bytes());
        b[1992..1996].copy_from_slice(&3.0f32.to_le_bytes());
        b.push(0); // The footer must start after the header.
        let off = b.len() as u64;
        b[678..686].copy_from_slice(&off.to_le_bytes());
        let wl: Vec<String> = (0..4096).map(|i| format!("{}", 400 + i)).collect();
        let xml = format!(
            "<SpeFormat version=\"3.0\"><DataFormat><DataBlock type=\"Frame\" count=\"1000000\" \
             calibrations=\"1\"><DataBlock type=\"Region\" width=\"4096\" height=\"1\"/>\
             </DataBlock></DataFormat><Calibrations><WavelengthMapping id=\"1\"><Wavelength>{}\
             </Wavelength></WavelengthMapping></Calibrations></SpeFormat>",
            wl.join(",")
        );
        b.extend_from_slice(xml.as_bytes());
        let s = parse(&b).unwrap();
        assert_eq!(s.pages.len(), 1_000_000);
        let first = s.pages[0].wavelengths.clone().unwrap();
        let last = s.pages[999_999].wavelengths.clone().unwrap();
        assert!(Arc::ptr_eq(&first, &last));
    }
}

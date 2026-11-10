//! NT-MDT `.mdt` files.
//!
//! A 33-byte file header (signature `01 B0 93 FF`, the index of the last frame at byte 12)
//! is followed by frames, each with a 22-byte header: size, type, version, date and time
//! (six 16-bit fields) and the size of its variables. Scanned frames (type 0) store three
//! axis scales (offset, step and a unit code), the scan parameters and, after the
//! variables, the frame-mode size and any point list, then little-endian 16-bit samples, a
//! value being `offset + step × sample` in the z scale's unit. MDA frames (type 106) describe
//! an n-dimensional array with a calibration per dimension and measurand; two-dimensional
//! ones with one measurand are read as images. Spectroscopy, curves and text frames are
//! skipped. Each image frame is a page.

use specular_em::dataset::{Axis, AxisKind, Dataset, Field, Lateral, Page, Plane, PlaneData};
use specular_em::samples::{Order, Sample};

use crate::units::{latin1, si};
use crate::{Error, Result};

/// The largest plane accepted (samples).
const MAX_PLANE: usize = 1 << 28;

const SCANNED: u16 = 0;
const MDA: u16 = 106;

/// Whether `head` starts an NT-MDT file.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&[0x01, 0xB0, 0x93, 0xFF]) || head.starts_with(&[0x01, 0xB0, 0x04, 0x06])
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn i16_at(b: &[u8], at: usize) -> Option<i16> {
    u16_at(b, at).map(|v| v as i16)
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

fn f32_at(b: &[u8], at: usize) -> Option<f64> {
    u32_at(b, at).map(|v| f64::from(f32::from_bits(v)))
}

fn f64_at(b: &[u8], at: usize) -> Option<f64> {
    u64_at(b, at).map(f64::from_bits)
}

/// A unit code of the axis scales: the SI unit and the factor to it.
fn unit(code: i16) -> (&'static str, f64) {
    match code {
        -10 => ("1/cm", 1.0),
        -5 => ("m", 1.0),
        -4 => ("m", 1e-2),
        -3 => ("m", 1e-3),
        -2 => ("m", 1e-6),
        -1 => ("m", 1e-9),
        0 => ("m", 1e-10),
        1 => ("A", 1e-9),
        2 | 8 | 25 => ("V", 1.0),
        4 => ("Hz", 1e3),
        5 => ("deg", 1.0),
        6 => ("%", 1.0),
        7 => ("°C", 1.0),
        9 => ("s", 1.0),
        10 => ("s", 1e-3),
        11 => ("s", 1e-6),
        12 => ("s", 1e-9),
        13 => ("counts", 1.0),
        14 => ("px", 1.0),
        20 => ("A", 1.0),
        21 => ("A", 1e-3),
        22 => ("A", 1e-6),
        23 => ("A", 1e-9),
        24 => ("A", 1e-12),
        26 => ("V", 1e-3),
        27 => ("V", 1e-6),
        28 => ("V", 1e-9),
        29 => ("V", 1e-12),
        30 => ("N", 1.0),
        31 => ("N", 1e-3),
        32 => ("N", 1e-6),
        33 => ("N", 1e-9),
        34 => ("N", 1e-12),
        _ => ("", 1.0),
    }
}

/// An axis scale: offset, step and unit.
fn scale(b: &[u8], at: usize) -> Option<(f64, f64, i16)> {
    Some((f32_at(b, at)?, f32_at(b, at + 4)?, i16_at(b, at + 8)?))
}

fn lateral((offset, step, code): (f64, f64, i16)) -> Option<Lateral> {
    let (u, f) = unit(code);
    let l = Lateral::new((offset + step / 2.0) * f, step * f, u);
    l.is_usable().then_some(l)
}

fn page(width: usize, height: usize, x: Option<Lateral>, y: Option<Lateral>) -> Page {
    Page {
        field: Field::ScanningProbe,
        width,
        height,
        axis: Axis {
            kind: AxisKind::Channel,
            name: String::new(),
            linear: None,
        },
        x,
        y,
        instrument: Some("NT-MDT".into()),
        ..Page::default()
    }
}

/// A scanned frame's page; `d` is the frame's data (after its 22-byte header).
fn scanned(file: &[u8], d: usize, end: usize, var_size: usize) -> Option<Page> {
    let xs = scale(file, d)?;
    let ys = scale(file, d + 10)?;
    let zs = scale(file, d + 20)?;
    let mode = *file.get(d + 31)?;
    let bias = f32_at(file, d + 58);
    let mut p = d + var_size;
    let width = usize::from(u16_at(file, p + 2)?);
    let height = usize::from(u16_at(file, p + 4)?);
    let dots = usize::from(u16_at(file, p + 6)?);
    p += 8;
    if dots > 0 {
        let header = u32_at(file, p)? as usize;
        p = p.checked_add(4 + header)?;
        let mut sizes = 0usize;
        for i in 0..dots {
            let fwd = u32_at(file, p + i * 16 + 8)? as usize;
            let bwd = u32_at(file, p + i * 16 + 12)? as usize;
            sizes = sizes.checked_add(fwd.checked_add(bwd)?)?;
        }
        p = p
            .checked_add(dots * 16)?
            .checked_add(sizes.checked_mul(2)?)?;
    }
    let n = width.checked_mul(height)?;
    if n == 0 || n > MAX_PLANE || p >= end {
        return None;
    }
    let (u, f) = unit(zs.2);
    let mut page = page(width, height, lateral(xs), lateral(ys));
    page.sample = Some(Sample::I16);
    page.levelling = u == "m";
    page.mode = Some(match mode {
        0 => "STM".into(),
        1 => "AFM".into(),
        m => format!("mode {m}"),
    });
    if let Some(b) = bias.filter(|b| b.is_finite() && *b != 0.0) {
        page.meta.push(("Bias".into(), format!("{b} V")));
    }
    let title_at = p + n * 2;
    if let Some(len) = u32_at(file, title_at).map(|l| l as usize)
        && let Some(t) = file.get(title_at + 4..title_at + 4 + len)
        && title_at + 4 + len <= end
    {
        // Titles are Windows-1251; the ASCII part is what matters for a label.
        let t = latin1(t).trim().to_string();
        if !t.is_empty() && t.is_ascii() {
            page.title = Some(t);
        }
    }
    page.planes.push(Plane {
        label: page.title.clone().unwrap_or_else(|| "Z".into()),
        unit: u.to_string(),
        data: PlaneData::Stored {
            offset: p,
            sample: Sample::I16,
            order: Order::Little,
            bottom_up: false,
            scale: zs.1 * f,
            add: zs.0 * f,
        },
    });
    Some(page)
}

/// One MDA calibration: name, unit, bias, scale, count and sample type; and its length.
struct Calibration {
    name: String,
    unit: String,
    bias: f64,
    scale: f64,
    count: u64,
    sample: Option<Sample>,
}

fn calibration(b: &[u8], at: usize) -> Option<(Calibration, usize)> {
    let total = u32_at(b, at)? as usize;
    let fixed = u32_at(b, at + 4)? as usize;
    let name_len = u32_at(b, at + 8)? as usize;
    let comment_len = u32_at(b, at + 12)? as usize;
    let unit_len = u32_at(b, at + 16)? as usize;
    let bias = f64_at(b, at + 44)?;
    let scale = f64_at(b, at + 52)?;
    let min = u64_at(b, at + 60)?;
    let max = u64_at(b, at + 68)?;
    let ty = u32_at(b, at + 76)? as i32;
    let author_len = u32_at(b, at + 80)? as usize;
    let text = |from: usize, len: usize| b.get(from..from.checked_add(len)?).map(latin1);
    let name_at = at + fixed.max(84);
    let name = text(name_at, name_len)?;
    let unit = text(name_at + name_len + comment_len, unit_len)?;
    let own = (fixed.max(84) + name_len + comment_len + unit_len).checked_add(author_len)?;
    let sample = match ty {
        -1 => Some(Sample::I8),
        1 => Some(Sample::U8),
        -2 => Some(Sample::I16),
        2 => Some(Sample::U16),
        -4 => Some(Sample::I32),
        4 => Some(Sample::U32),
        -8 => Some(Sample::I64),
        8 => Some(Sample::U64),
        -5892 => Some(Sample::F32),
        -13320 => Some(Sample::F64),
        _ => None,
    };
    Some((
        Calibration {
            name: name.trim().to_string(),
            unit: unit.trim().to_string(),
            bias,
            scale,
            count: max.checked_sub(min)?.checked_add(1)?,
            sample,
        },
        if total >= own { total } else { own },
    ))
}

/// A two-dimensional, single-measurand MDA frame's page.
fn mda(file: &[u8], d: usize, end: usize) -> Option<Page> {
    let name_size = u32_at(file, d + 44)? as usize;
    let comm_size = u32_at(file, d + 48)? as usize;
    let data_offset = u32_at(file, d + 68)? as usize;
    let data_size = u32_at(file, d + 72)? as usize;
    let title = file
        .get(d + 76..(d + 76).checked_add(name_size)?)
        .map(latin1);
    let s = (d + 76).checked_add(name_size)?.checked_add(comm_size)?;
    let dims = u32_at(file, s + 16)? as usize;
    let measurands = u32_at(file, s + 20)? as usize;
    if dims != 2 || measurands != 1 {
        return None;
    }
    let mut at = s + 24;
    let mut cal = Vec::new();
    for _ in 0..3 {
        let (c, len) = calibration(file, at)?;
        at = at.checked_add(len)?;
        cal.push(c);
    }
    let (x, y, z) = (&cal[0], &cal[1], &cal[2]);
    let width = usize::try_from(x.count).ok()?;
    let height = usize::try_from(y.count).ok()?;
    let n = width
        .checked_mul(height)
        .filter(|&n| n > 0 && n <= MAX_PLANE)?;
    let sample = z.sample?;
    let offset = d.checked_add(data_offset)?;
    if offset >= end || data_size < n * sample.bytes() {
        return None;
    }
    let axis = |c: &Calibration| {
        let (u, f) = si(&c.unit);
        let l = Lateral::new((c.bias + c.scale / 2.0) * f, c.scale * f, u);
        l.is_usable().then_some(l)
    };
    let (u, f) = si(&z.unit);
    let mut page = page(width, height, axis(x), axis(y));
    page.title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    page.sample = Some(sample);
    page.levelling = u == "m";
    page.planes.push(Plane {
        label: if z.name.is_empty() {
            "Z".into()
        } else {
            z.name.clone()
        },
        unit: u,
        data: PlaneData::Stored {
            offset,
            sample,
            order: Order::Little,
            bottom_up: false,
            scale: z.scale * f,
            add: z.bias * f,
        },
    });
    Some(page)
}

/// Reads an NT-MDT file: a page per scanned or two-dimensional MDA frame.
pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotMdt);
    }
    let last = u16_at(file, 12).ok_or(Error::Truncated("header"))?;
    let mut pages = Vec::new();
    let mut at = 33usize;
    for _ in 0..=usize::from(last) {
        let Some(size) = u32_at(file, at).map(|s| s as usize) else {
            break;
        };
        if size < 22 {
            break;
        }
        let end = at.saturating_add(size).min(file.len());
        let ty = u16_at(file, at + 4).unwrap_or(u16::MAX);
        let date: Option<Vec<u16>> = (0..6).map(|i| u16_at(file, at + 8 + 2 * i)).collect();
        let var_size = usize::from(u16_at(file, at + 20).unwrap_or(0));
        let page = match ty {
            SCANNED => scanned(file, at + 22, end, var_size),
            MDA => mda(file, at + 22, end),
            _ => None,
        };
        if let Some(mut p) = page {
            if let Some(&[y, mo, d, h, mi, s]) = date.as_deref() {
                p.date = Some(format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}"));
            }
            pages.push(p);
        }
        at = at.saturating_add(size);
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units() {
        assert_eq!(unit(0), ("m", 1e-10));
        assert_eq!(unit(-1), ("m", 1e-9));
        assert_eq!(unit(24), ("A", 1e-12));
        assert_eq!(unit(99), ("", 1.0));
    }

    #[test]
    fn refuses_garbage() {
        assert_eq!(read(b"junk").unwrap_err(), Error::NotMdt);
        let mut f = vec![0x01, 0xB0, 0x93, 0xFF];
        f.resize(33, 0);
        assert_eq!(read(&f).unwrap_err(), Error::NoImage);
        f.extend_from_slice(&[0xFF; 40]);
        assert!(read(&f).is_err());
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A calibration spanning the whole `u64` range: its count is one past the largest.
    #[test]
    fn calibration_of_the_full_range() {
        let mut b = vec![0u8; 84];
        b[4..8].copy_from_slice(&84u32.to_le_bytes());
        b[68..76].copy_from_slice(&u64::MAX.to_le_bytes());
        b[76..80].copy_from_slice(&(-2i32).to_le_bytes());
        assert!(calibration(&b, 0).is_none());
        b[68..76].copy_from_slice(&9u64.to_le_bytes());
        assert_eq!(calibration(&b, 0).unwrap().0.count, 10);
    }
}

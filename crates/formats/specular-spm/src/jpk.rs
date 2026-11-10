//! JPK Instruments images (`.jpk`, `.jpk-qi-image`).
//!
//! A classic TIFF whose first IFD is an 8-bit thumbnail carrying the scan in private tags
//! (0x8000 and up): the grid origin and lengths in metres (0x8040–0x8043), the scan
//! direction (0x804B) and the feedback mode (0x8030). Each further IFD is one channel: its
//! name (0x8050), whether it is the retrace (0x8051), and up to 0x8080 calibration slots of
//! 0x30 tags from 0x8090, each with a name, a unit (+0x12), a scaling type (+0x13) and a
//! linear multiplier and offset (+0x14, +0x15) that map the stored integers straight to the
//! slot's unit; the slot named by 0x8081 is the one shown.

use specular_em::dataset::{Axis, AxisKind, Dataset, Field, Lateral, Page, Plane, PlaneData};
use specular_em::samples::{Order, Sample, convert};

use crate::units::si;
use crate::{Error, Result};

/// The largest plane accepted (samples).
const MAX_PLANE: usize = 1 << 28;

/// One IFD entry: tag, type, count and where its value bytes are.
#[derive(Clone, Copy, Debug)]
struct Entry {
    tag: u16,
    ty: u16,
    count: usize,
    at: usize,
}

/// A minimal classic-TIFF reader: the IFD chain and typed tag values.
struct Tiff<'a> {
    file: &'a [u8],
    order: Order,
    ifds: Vec<Vec<Entry>>,
}

fn type_size(ty: u16) -> usize {
    match ty {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 | 16 | 17 | 18 => 8,
        _ => 0,
    }
}

impl<'a> Tiff<'a> {
    fn parse(file: &'a [u8]) -> Option<Tiff<'a>> {
        let order = match file.get(..4)? {
            b"II*\0" => Order::Little,
            b"MM\0*" => Order::Big,
            _ => return None,
        };
        let mut t = Tiff {
            file,
            order,
            ifds: Vec::new(),
        };
        let mut next = t.u32(4)? as usize;
        let mut seen = Vec::new();
        let mut listed = 0usize;
        while next != 0 && t.ifds.len() < 4096 && !seen.contains(&next) {
            seen.push(next);
            let n = t.u16(next)? as usize;
            // IFDs don't overlap, so their entries fit in the file together; overlapping
            // ones would list the same bytes over and over.
            listed += n * 12;
            if listed > file.len() {
                return None;
            }
            let mut entries = Vec::with_capacity(n);
            for i in 0..n {
                let e = next + 2 + i * 12;
                let ty = t.u16(e + 2)?;
                let count = t.u32(e + 4)? as usize;
                let size = type_size(ty).checked_mul(count)?;
                let at = if size <= 4 {
                    e + 8
                } else {
                    t.u32(e + 8)? as usize
                };
                entries.push(Entry {
                    tag: t.u16(e)?,
                    ty,
                    count,
                    at,
                });
            }
            t.ifds.push(entries);
            next = t.u32(next + 2 + n * 12)? as usize;
        }
        Some(t)
    }

    fn bytes(&self, at: usize, n: usize) -> Option<&'a [u8]> {
        self.file.get(at..at.checked_add(n)?)
    }

    fn u16(&self, at: usize) -> Option<u16> {
        let b: [u8; 2] = self.bytes(at, 2)?.try_into().ok()?;
        Some(match self.order {
            Order::Little => u16::from_le_bytes(b),
            Order::Big => u16::from_be_bytes(b),
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b: [u8; 4] = self.bytes(at, 4)?.try_into().ok()?;
        Some(match self.order {
            Order::Little => u32::from_le_bytes(b),
            Order::Big => u32::from_be_bytes(b),
        })
    }

    fn u64(&self, at: usize) -> Option<u64> {
        let b: [u8; 8] = self.bytes(at, 8)?.try_into().ok()?;
        Some(match self.order {
            Order::Little => u64::from_le_bytes(b),
            Order::Big => u64::from_be_bytes(b),
        })
    }

    fn entry(&self, ifd: usize, tag: u16) -> Option<Entry> {
        self.ifds.get(ifd)?.iter().find(|e| e.tag == tag).copied()
    }

    /// The `i`-th value of an integer or floating-point tag.
    fn number_at(&self, e: Entry, i: usize) -> Option<f64> {
        if i >= e.count {
            return None;
        }
        let at = e.at + i * type_size(e.ty);
        Some(match e.ty {
            1 | 7 => f64::from(*self.file.get(at)?),
            6 => f64::from(*self.file.get(at)? as i8),
            3 => f64::from(self.u16(at)?),
            8 => f64::from(self.u16(at)? as i16),
            4 | 13 => f64::from(self.u32(at)?),
            9 => f64::from(self.u32(at)? as i32),
            11 => f64::from(f32::from_bits(self.u32(at)?)),
            12 => f64::from_bits(self.u64(at)?),
            16 => self.u64(at)? as f64,
            17 => self.u64(at)? as i64 as f64,
            5 | 10 => {
                let (n, d) = (self.u32(at)?, self.u32(at + 4)?);
                let (n, d) = if e.ty == 10 {
                    (f64::from(n as i32), f64::from(d as i32))
                } else {
                    (f64::from(n), f64::from(d))
                };
                n / d
            }
            _ => return None,
        })
    }

    fn number(&self, ifd: usize, tag: u16) -> Option<f64> {
        self.number_at(self.entry(ifd, tag)?, 0)
    }

    fn numbers(&self, ifd: usize, tag: u16) -> Option<Vec<f64>> {
        let e = self.entry(ifd, tag)?;
        (0..e.count).map(|i| self.number_at(e, i)).collect()
    }

    fn text(&self, ifd: usize, tag: u16) -> Option<String> {
        let e = self.entry(ifd, tag)?;
        if e.ty != 2 {
            return None;
        }
        let b = self.bytes(e.at, e.count)?;
        let b = b.split(|&c| c == 0).next().unwrap_or(b);
        Some(String::from_utf8_lossy(b).trim().to_string())
    }
}

/// Whether `head` is a TIFF (the JPK tags may sit past the head; see [`probe_file`]).
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"MM\0*") || head.starts_with(b"II*\0")
}

/// Whether the whole `file` is a JPK image: a TIFF whose first IFD has the JPK grid tags.
pub fn probe_file(file: &[u8]) -> bool {
    Tiff::parse(file).is_some_and(|t| {
        t.entry(0, 0x8000).is_some() && t.entry(0, 0x8042).is_some() && t.entry(0, 0x8043).is_some()
    })
}

/// Where a channel's samples are: one contiguous run, or strips gathered here.
/// `left`: the bytes all planes may still gather from strips (twice the file: strips that
/// repeat one range of the file gave every one of 4096 planes a full-size copy).
fn plane_data(
    t: &Tiff,
    ifd: usize,
    w: usize,
    h: usize,
    bottom_up: bool,
    left: &mut usize,
) -> Option<PlaneData> {
    if t.number(ifd, 259).unwrap_or(1.0) != 1.0 || t.number(ifd, 277).unwrap_or(1.0) != 1.0 {
        return None;
    }
    let bits = t.number(ifd, 258).unwrap_or(1.0) as u32;
    let format = t.number(ifd, 339).unwrap_or(1.0) as u32;
    let sample = match (format, bits) {
        (1, 8) => Sample::U8,
        (2, 8) => Sample::I8,
        (1, 16) => Sample::U16,
        (2, 16) => Sample::I16,
        (1, 32) => Sample::U32,
        (2, 32) => Sample::I32,
        (3, 32) => Sample::F32,
        (3, 64) => Sample::F64,
        _ => return None,
    };
    let offsets = t.numbers(ifd, 273)?;
    let counts = t.numbers(ifd, 279)?;
    let first = *offsets.first()? as usize;
    let contiguous = offsets
        .windows(2)
        .zip(&counts)
        .all(|(o, c)| o[0] + c == o[1]);
    if contiguous {
        return Some(PlaneData::Stored {
            offset: first,
            sample,
            order: t.order,
            bottom_up,
            scale: 1.0,
            add: 0.0,
        });
    }
    // Strips may repeat a range of the file, so gathering stops at the plane's size; a plane
    // short of samples keeps only those (the rest reads as missing), so the buffer is sized
    // by the file rather than by the declared grid.
    let need = (w * h * sample.bytes()).min(*left);
    let mut bytes = Vec::new();
    for (&o, &c) in offsets.iter().zip(&counts) {
        if bytes.len() >= need {
            break;
        }
        let strip = t.bytes(o as usize, c as usize).unwrap_or(&[]);
        bytes.extend_from_slice(&strip[..strip.len().min(need - bytes.len())]);
    }
    *left -= bytes.len();
    if bytes.len() < w * h * sample.bytes() && bottom_up {
        // The rows present are the bottom ones; with the top missing the plane is missing.
        return Some(PlaneData::Values(Vec::new().into()));
    }
    let mut v = vec![0f32; bytes.len() / sample.bytes()];
    convert(&bytes, sample, t.order, sample.bytes(), &mut v);
    if bottom_up {
        let rows: Vec<&[f32]> = v.chunks_exact(w).rev().collect();
        v = rows.concat();
    }
    Some(PlaneData::Values(v.into()))
}

/// The shown slot's unit, multiplier and offset.
fn calibration(t: &Tiff, ifd: usize) -> (String, f64, f64) {
    let slots = t.number(ifd, 0x8080).unwrap_or(0.0).clamp(0.0, 64.0) as u16;
    let shown = t.text(ifd, 0x8081).unwrap_or_default();
    let Some(n) = (0..slots).find(|&n| t.text(ifd, 0x8090 + 0x30 * n).as_deref() == Some(&shown))
    else {
        return (String::new(), 1.0, 0.0);
    };
    let base = 0x80A0 + 0x30 * n;
    let unit = t.text(ifd, base + 2).unwrap_or_default();
    if t.text(ifd, base + 3).as_deref() != Some("LinearScaling") {
        return (unit, 1.0, 0.0);
    }
    let m = t.number(ifd, base + 4).unwrap_or(1.0);
    let o = t.number(ifd, base + 5).unwrap_or(0.0);
    (unit, m, o)
}

/// Reads a JPK image: one page per channel size, a plane per channel IFD.
pub fn read(file: &[u8]) -> Result<Dataset> {
    let t = Tiff::parse(file).ok_or(Error::NotJpk)?;
    if !probe_file(file) {
        return Err(Error::NotJpk);
    }
    let bottom_up = t
        .text(0, 0x804B)
        .is_none_or(|d| !d.eq_ignore_ascii_case("topDown"));
    let lateral = |origin: u16, length: u16, n: usize| {
        let o = t.number(0, origin).unwrap_or(0.0);
        let step = t.number(0, length)? / n as f64;
        let l = Lateral::new(o + step / 2.0, step, "m");
        l.is_usable().then_some(l)
    };
    let mut meta = Vec::new();
    for (tag, key) in [
        (0x8000, "Version"),
        (0x8003, "Start date"),
        (0x8004, "Name"),
        (0x8009, "Account"),
        (0x8030, "Feedback mode"),
        (0x804B, "Scan direction"),
    ] {
        if let Some(v) = t.text(0, tag).filter(|v| !v.is_empty() && v != "<empty>") {
            meta.push((key.to_string(), v));
        }
    }
    let date = t.text(0, 0x8003);
    let mode = t.text(0, 0x8030);
    let mut pages: Vec<Page> = Vec::new();
    let mut left = file.len().saturating_mul(2);
    for ifd in 1..t.ifds.len() {
        let Some(name) = t.text(ifd, 0x8050) else {
            continue;
        };
        let (Some(w), Some(h)) = (t.number(ifd, 256), t.number(ifd, 257)) else {
            continue;
        };
        let (w, h) = (w as usize, h as usize);
        if w == 0 || h == 0 || w.checked_mul(h).is_none_or(|n| n > MAX_PLANE) {
            continue;
        }
        let Some(data) = plane_data(&t, ifd, w, h, bottom_up, &mut left) else {
            continue;
        };
        let (unit, m, o) = calibration(&t, ifd);
        let (unit, f) = si(&unit);
        let data = match data {
            PlaneData::Stored {
                offset,
                sample,
                order,
                bottom_up,
                ..
            } => PlaneData::Stored {
                offset,
                sample,
                order,
                bottom_up,
                scale: m * f,
                add: o * f,
            },
            PlaneData::Values(v) => PlaneData::Values(
                v.iter()
                    .map(|&r| ((f64::from(r) * m + o) * f) as f32)
                    .collect(),
            ),
        };
        let mut label = t
            .text(ifd, 0x8052)
            .filter(|s| !s.is_empty())
            .unwrap_or(name);
        if t.number(ifd, 0x8051).is_some_and(|r| r != 0.0) {
            label.push_str(" (retrace)");
        }
        let plane = Plane { label, unit, data };
        let i = match pages.iter().position(|p| p.width == w && p.height == h) {
            Some(i) => i,
            None => {
                pages.push(Page {
                    field: Field::ScanningProbe,
                    title: t.text(0, 0x8004).filter(|v| !v.is_empty()),
                    width: w,
                    height: h,
                    axis: Axis {
                        kind: AxisKind::Channel,
                        name: String::new(),
                        linear: None,
                    },
                    x: lateral(0x8040, 0x8042, w),
                    y: lateral(0x8041, 0x8043, h),
                    instrument: Some("JPK".into()),
                    date: date.clone(),
                    mode: mode.clone(),
                    meta: meta.clone(),
                    levelling: plane.unit == "m",
                    ..Page::default()
                });
                pages.len() - 1
            }
        };
        if let PlaneData::Stored { sample, .. } = plane.data
            && pages[i].planes.is_empty()
        {
            pages[i].sample = Some(sample);
        }
        pages[i].planes.push(plane);
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
    fn refuses_garbage() {
        assert_eq!(read(b"hello").unwrap_err(), Error::NotJpk);
        assert!(!probe_file(b"MM\0*\0\0\0\x08\0"));
        assert!(probe(b"MM\0*"));
        let mut plain = b"II*\0\x08\0\0\0\x01\0".to_vec();
        plain.extend_from_slice(&[0, 1, 3, 0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(read(&plain).unwrap_err(), Error::NotJpk);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    type Tag = (u16, u16, u32, Vec<u8>);

    /// A big-endian TIFF of the given IFDs, each a list of `(tag, type, count, value)`;
    /// values that don't fit in the entry go after the IFDs.
    fn tiff(ifds: &[Vec<Tag>]) -> Vec<u8> {
        let mut f = b"MM\0*\0\0\0\x08".to_vec();
        let mut extra = 8 + ifds.iter().map(|e| 2 + 12 * e.len() + 4).sum::<usize>();
        let mut tail = Vec::new();
        for (k, entries) in ifds.iter().enumerate() {
            f.extend_from_slice(&(entries.len() as u16).to_be_bytes());
            for (tag, ty, count, value) in entries {
                f.extend_from_slice(&tag.to_be_bytes());
                f.extend_from_slice(&ty.to_be_bytes());
                f.extend_from_slice(&count.to_be_bytes());
                if value.len() <= 4 {
                    let mut v = value.clone();
                    v.resize(4, 0);
                    f.extend_from_slice(&v);
                } else {
                    f.extend_from_slice(&(extra as u32).to_be_bytes());
                    tail.extend_from_slice(value);
                    extra += value.len();
                }
            }
            let next = if k + 1 < ifds.len() { f.len() + 4 } else { 0 };
            f.extend_from_slice(&(next as u32).to_be_bytes());
        }
        f.extend_from_slice(&tail);
        f
    }

    /// A 16384² float channel whose strip list repeats the same 8 bytes: the plane would
    /// have been gathered and converted at full size.
    fn huge_channel(top_down: bool, strips: u32) -> Vec<u8> {
        let mut head: Vec<Tag> = vec![
            (0x8000, 2, 2, b"1\0".to_vec()),
            (0x8042, 12, 1, 1e-6f64.to_be_bytes().to_vec()),
            (0x8043, 12, 1, 1e-6f64.to_be_bytes().to_vec()),
        ];
        if top_down {
            head.push((0x804B, 2, 8, b"topDown\0".to_vec()));
        }
        let repeated: Vec<u8> = (0..strips).flat_map(|_| 8u32.to_be_bytes()).collect();
        let channel: Vec<Tag> = vec![
            (256, 4, 1, 16384u32.to_be_bytes().to_vec()),
            (257, 4, 1, 16384u32.to_be_bytes().to_vec()),
            (258, 3, 1, 32u16.to_be_bytes().to_vec()),
            (273, 4, strips, repeated.clone()),
            (279, 4, strips, repeated),
            (339, 3, 1, 3u16.to_be_bytes().to_vec()),
            (0x8050, 2, 7, b"height\0".to_vec()),
        ];
        tiff(&[head, channel])
    }

    #[test]
    fn repeated_strips_of_a_huge_channel() {
        let f = huge_channel(true, 4);
        let d = read(&f).unwrap();
        let PlaneData::Values(v) = &d.pages[0].planes[0].data else {
            panic!("strips gathered");
        };
        assert_eq!(v.len(), 8);
        let f = huge_channel(false, 4);
        let d = read(&f).unwrap();
        assert!(matches!(&d.pages[0].planes[0].data, PlaneData::Values(v) if v.is_empty()));
    }

    /// 16 channels whose 512 strips all name the file's first 4 KB: each plane gathered 2 MB
    /// (32 MB from a 70 KB file, and 4096 planes of up to 1 GB each are as easy). All planes
    /// together now gather at most twice the file.
    #[test]
    fn planes_gather_at_most_twice_the_file() {
        let head: Vec<Tag> = vec![
            (0x8000, 2, 2, b"1\0".to_vec()),
            (0x8042, 12, 1, 1e-6f64.to_be_bytes().to_vec()),
            (0x8043, 12, 1, 1e-6f64.to_be_bytes().to_vec()),
            (0x804B, 2, 8, b"topDown\0".to_vec()),
        ];
        let strips = 512u32;
        let offs: Vec<u8> = (0..strips).flat_map(|_| 8u32.to_be_bytes()).collect();
        let counts: Vec<u8> = (0..strips).flat_map(|_| 4096u32.to_be_bytes()).collect();
        let channel: Vec<Tag> = vec![
            (256, 4, 1, 16384u32.to_be_bytes().to_vec()),
            (257, 4, 1, 16384u32.to_be_bytes().to_vec()),
            (258, 3, 1, 32u16.to_be_bytes().to_vec()),
            (273, 4, strips, offs),
            (279, 4, strips, counts),
            (339, 3, 1, 3u16.to_be_bytes().to_vec()),
            (0x8050, 2, 7, b"height\0".to_vec()),
        ];
        let mut ifds = vec![head];
        ifds.extend(std::iter::repeat_n(channel, 16));
        let f = tiff(&ifds);
        let d = read(&f).unwrap();
        let gathered: usize = d
            .pages
            .iter()
            .flat_map(|p| &p.planes)
            .map(|p| match &p.data {
                PlaneData::Values(v) => v.len() * 4,
                PlaneData::Stored { .. } => 0,
            })
            .sum();
        assert!(
            gathered > 0 && gathered <= 2 * f.len(),
            "{gathered} of {}",
            f.len()
        );
    }

    /// 101 IFDs of 40 entries each, 4 bytes apart, all over the same ~500 bytes.
    #[test]
    fn overlapping_ifds() {
        let (base, k, n) = (8usize, 100usize, 40usize);
        let ptrs = base + 2 + 12 * n;
        let mut f = vec![0u8; ptrs + 4 * (k + 1)];
        f[..8].copy_from_slice(b"MM\0*\0\0\0\x08");
        for i in 0..=k {
            let o = base + 4 * i;
            f[o..o + 2].copy_from_slice(&(n as u16).to_be_bytes());
            let next = if i < k { (o + 4) as u32 } else { 0 };
            f[ptrs + 4 * i..ptrs + 4 * i + 4].copy_from_slice(&next.to_be_bytes());
        }
        assert!(Tiff::parse(&f).is_none());
    }
}

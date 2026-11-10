//! Gatan DigitalMicrograph DM3 / DM4: [`parse`] reads the tag tree and describes the images;
//! [`DmImage::read_plane`] reads one plane's values.
//!
//! An image's `ImageData` holds `Data` (the pixels, an array tag), `DataType`, `Dimensions`
//! (x first) and `Calibrations` (`Dimension[i]` and `Brightness`, each `Origin`, `Scale`,
//! `Units`); its `ImageTags` hold the acquisition metadata. The root `Thumbnails` group names
//! the `ImageList` entries that are thumbnails (by `ImageIndex`); those are skipped.
//!
//! Pixels are stored row-major, x fastest, top row first; dimensions past the second are
//! planes. Colour images (data types 8 and 23) are BGRA bytes and come out as three planes
//! per section (red, green, blue). Packed complex FFT images are unpacked to their full size
//! by conjugate symmetry; complex values come out as their magnitude.

pub mod tags;

use crate::samples::{self, Order, Sample};
use crate::{Calibration, Error, MAX_PLANE, Result};
use tags::{Node, Tag, Value};

/// The file version and data byte order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub dm4: bool,
    /// Whether tag values and pixels are little-endian.
    pub little: bool,
}

impl Version {
    pub(crate) fn header_len(self) -> usize {
        if self.dm4 { 16 } else { 12 }
    }
}

/// Reads the version and byte order from the first 16 bytes.
pub fn sniff(head: &[u8]) -> Option<Version> {
    let be = |o: usize| {
        head.get(o..o + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let dm4 = match be(0)? {
        3 => false,
        4 => true,
        _ => return None,
    };
    let order = be(if dm4 { 12 } else { 8 })?;
    if order > 1 {
        return None;
    }
    // The root group: sorted and open flags are 0 or 1.
    let g = if dm4 { 16 } else { 12 };
    if head.len() >= g + 2 && (head[g] > 1 || head[g + 1] > 1) {
        return None;
    }
    Some(Version {
        dm4,
        little: order == 1,
    })
}

/// How an image's pixels are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixels {
    /// One sample per pixel (complex samples give their magnitude).
    Plain(Sample),
    /// Blue, green, red and alpha bytes (data types 8 and 23).
    Bgra,
    /// The old packed complex FFT (data type 5): a 2N × 2N transform in 4N² `f32` values.
    PackedOld,
    /// The new packed complex FFT (27 and 28): H rows of W/2 + 1 complex values.
    PackedNew(Sample),
}

/// Acquisition metadata from the image tags.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub microscope: Option<String>,
    /// Accelerating voltage in volts.
    pub voltage: Option<f64>,
    pub magnification: Option<f64>,
    /// "TEM", "STEM", "DIFFRACTION", …
    pub mode: Option<String>,
    /// Exposure in seconds.
    pub exposure: Option<f64>,
    /// The acquisition date and time as written ("6/12/2017 3:26:55 PM").
    pub acquired: Option<String>,
    /// The camera or detector.
    pub device: Option<String>,
    /// The signal ("EELS", "X-ray", "CL").
    pub signal: Option<String>,
}

/// One image of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct DmImage {
    /// The image's index in the `ImageList`.
    pub index: usize,
    pub name: Option<String>,
    /// The DM data type code.
    pub data_type: u32,
    pub pixels: Pixels,
    /// Dimensions, x first.
    pub dims: Vec<usize>,
    /// Calibration of each dimension (identity when the file has none).
    pub calibrations: Vec<Calibration>,
    /// The intensity calibration.
    pub brightness: Calibration,
    pub meta: Meta,
    /// Every `ImageTags` entry as (dotted path, value), for an all-fields listing.
    pub fields: Vec<(String, String)>,
    offset: usize,
    len: usize,
    little: bool,
}

/// A parsed file.
#[derive(Clone, Debug, PartialEq)]
pub struct Dm {
    pub version: Version,
    /// The images, without thumbnails, in `ImageList` order.
    pub images: Vec<DmImage>,
    /// The whole tag tree.
    pub tags: Vec<Tag>,
}

/// Parses the tag tree and the image descriptions of a whole file.
pub fn parse(file: &[u8]) -> Result<Dm> {
    let version = sniff(file).ok_or(Error::NotDm)?;
    let tags = tags::parse(file, version)?;
    let thumbs: Vec<u64> = tags::group(&tags, "Thumbnails")
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| match &t.node {
            Node::Group(g) => tags::value(g, "ImageIndex")?.u64(),
            Node::Data(_) => None,
        })
        .collect();
    let mut images = Vec::new();
    let mut first_err = None;
    for (index, entry) in tags::group(&tags, "ImageList")
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        if thumbs.contains(&(index as u64)) {
            continue;
        }
        let Node::Group(g) = &entry.node else {
            continue;
        };
        match image(file, version, index, g) {
            Ok(Some(i)) => images.push(i),
            Ok(None) => {}
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if images.is_empty() {
        return Err(first_err.unwrap_or(Error::NoImage));
    }
    Ok(Dm {
        version,
        images,
        tags,
    })
}

fn text(tags: &[Tag], path: &str, file: &[u8], little: bool) -> Option<String> {
    let s = tags::value(tags, path)?.text(file, little)?;
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn number(tags: &[Tag], path: &str) -> Option<f64> {
    tags::value(tags, path)?.f64().filter(|v| v.is_finite())
}

fn calibration(g: &[Tag], file: &[u8], little: bool) -> Calibration {
    Calibration {
        origin: number(g, "Origin").unwrap_or(0.0),
        scale: number(g, "Scale").filter(|&s| s != 0.0).unwrap_or(1.0),
        units: text(g, "Units", file, little).unwrap_or_default(),
    }
}

fn image(file: &[u8], v: Version, index: usize, g: &[Tag]) -> Result<Option<DmImage>> {
    let Some(data) = tags::group(g, "ImageData") else {
        return Ok(None);
    };
    let (offset, len, count) = match tags::value(data, "Data") {
        Some(
            Value::Array {
                offset, len, count, ..
            }
            | Value::Structs { offset, len, count },
        ) => (offset, len, count),
        _ => return Ok(None),
    };
    if *count == 0 {
        return Ok(None);
    }
    let data_type = tags::value(data, "DataType")
        .and_then(Value::u64)
        .ok_or_else(|| Error::Malformed("image without a data type".into()))?;
    let pixels = match data_type {
        1 => Pixels::Plain(Sample::I16),
        2 => Pixels::Plain(Sample::F32),
        3 => Pixels::Plain(Sample::ComplexF32),
        5 => Pixels::PackedOld,
        6 => Pixels::Plain(Sample::U8),
        7 => Pixels::Plain(Sample::I32),
        8 | 23 => Pixels::Bgra,
        9 => Pixels::Plain(Sample::I8),
        10 => Pixels::Plain(Sample::U16),
        11 => Pixels::Plain(Sample::U32),
        12 => Pixels::Plain(Sample::F64),
        13 => Pixels::Plain(Sample::ComplexF64),
        14 => Pixels::Plain(Sample::Bool),
        27 => Pixels::PackedNew(Sample::ComplexF32),
        28 => Pixels::PackedNew(Sample::ComplexF64),
        t => {
            return Err(Error::Unsupported(format!(
                "DigitalMicrograph data type {t}"
            )));
        }
    };
    let mut dims: Vec<usize> = tags::group(data, "Dimensions")
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| match &t.node {
            Node::Data(v) => v.u64().and_then(|d| usize::try_from(d).ok()),
            Node::Group(_) => None,
        })
        .collect();
    if dims.is_empty() || dims.contains(&0) {
        return Err(Error::Malformed("image without dimensions".into()));
    }
    let total = dims
        .iter()
        .try_fold(1u64, |a, &d| a.checked_mul(d as u64))
        .ok_or(Error::TooLarge)?;
    let plane = dims[0] as u64 * dims.get(1).copied().unwrap_or(1) as u64;
    if plane > MAX_PLANE {
        return Err(Error::TooLarge);
    }
    match pixels {
        Pixels::PackedOld
            if dims.len() != 2 || dims[0] != dims[1] || !dims[0].is_multiple_of(2) =>
        {
            return Err(Error::Unsupported(
                "packed complex image that is not 2N × 2N".into(),
            ));
        }
        Pixels::PackedNew(_) if dims.len() > 2 => {
            return Err(Error::Unsupported("packed complex stack".into()));
        }
        _ => {}
    }
    let _ = total;
    let cal = |i: usize| {
        tags::group(data, "Calibrations.Dimension")
            .and_then(|d| d.get(i))
            .and_then(|t| match &t.node {
                Node::Group(c) => Some(calibration(c, file, v.little)),
                Node::Data(_) => None,
            })
            .unwrap_or(Calibration {
                origin: 0.0,
                scale: 1.0,
                units: String::new(),
            })
    };
    let calibrations = (0..dims.len()).map(cal).collect();
    let brightness = tags::group(data, "Calibrations.Brightness")
        .map(|b| calibration(b, file, v.little))
        .unwrap_or(Calibration {
            origin: 0.0,
            scale: 1.0,
            units: String::new(),
        });
    // Trailing dimensions of 1 add nothing.
    while dims.len() > 2 && dims.last() == Some(&1) {
        dims.pop();
    }
    let it = tags::group(g, "ImageTags").unwrap_or(&[]);
    let little = v.little;
    let meta = Meta {
        microscope: text(it, "Microscope Info.Microscope", file, little)
            .or_else(|| text(it, "Microscope Info.Name", file, little)),
        voltage: number(it, "Microscope Info.Voltage").filter(|&v| v > 0.0),
        magnification: number(it, "Microscope Info.Indicated Magnification")
            .or_else(|| number(it, "Microscope Info.Actual Magnification"))
            .filter(|&m| m > 0.0),
        mode: text(it, "Microscope Info.Operation Mode", file, little)
            .or_else(|| text(it, "Microscope Info.Imaging Mode", file, little)),
        exposure: number(it, "DataBar.Exposure Time (s)")
            .or_else(|| number(it, "Acquisition.Parameters.High Level.Exposure (s)"))
            .filter(|&e| e > 0.0),
        acquired: match (
            text(it, "DataBar.Acquisition Date", file, little),
            text(it, "DataBar.Acquisition Time", file, little),
        ) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, t) => d.or(t),
        },
        device: text(it, "DataBar.Device Name", file, little)
            .or_else(|| text(it, "Acquisition.Device.Name", file, little)),
        signal: text(it, "Meta Data.Signal", file, little),
    };
    Ok(Some(DmImage {
        index,
        name: text(g, "Name", file, little),
        data_type: data_type as u32,
        pixels,
        dims,
        calibrations,
        brightness,
        meta,
        fields: tags::flatten(it, file, little),
        offset: *offset,
        len: *len,
        little,
    }))
}

impl DmImage {
    pub fn width(&self) -> usize {
        self.dims[0]
    }

    /// The height (1 for a line or spectrum).
    pub fn height(&self) -> usize {
        self.dims.get(1).copied().unwrap_or(1)
    }

    /// Sections: the product of the dimensions past the second.
    pub fn sections(&self) -> usize {
        self.dims.iter().skip(2).product()
    }

    /// Planes: the sections, times three for colour images.
    pub fn planes(&self) -> usize {
        self.sections() * if self.pixels == Pixels::Bgra { 3 } else { 1 }
    }

    pub fn is_rgb(&self) -> bool {
        self.pixels == Pixels::Bgra
    }

    /// Whether values are complex magnitudes.
    pub fn is_complex(&self) -> bool {
        match self.pixels {
            Pixels::Plain(s) => s.is_complex(),
            Pixels::PackedOld | Pixels::PackedNew(_) => true,
            Pixels::Bgra => false,
        }
    }

    /// The stored sample type (of one component for colour images).
    pub fn sample(&self) -> Sample {
        match self.pixels {
            Pixels::Plain(s) | Pixels::PackedNew(s) => s,
            Pixels::PackedOld => Sample::ComplexF32,
            Pixels::Bgra => Sample::U8,
        }
    }

    /// The unit of the values: the intensity calibration's unit when it is not rescaled.
    pub fn value_unit(&self) -> Option<&str> {
        let b = &self.brightness;
        (!b.units.is_empty() && b.scale == 1.0 && b.origin == 0.0).then_some(b.units.as_str())
    }

    /// Reads plane `p` (`width × height` values, top row first) of `file` into `out`.
    /// Returns whether samples were missing (truncated file; they read as NaN).
    pub fn read_plane(&self, file: &[u8], p: usize, out: &mut [f32]) -> Result<bool> {
        let (w, h) = (self.width(), self.height());
        let n = w * h;
        if p >= self.planes() || out.len() != n {
            return Err(Error::Malformed(format!("plane {p} of {}", self.planes())));
        }
        let end = self.offset.saturating_add(self.len).min(file.len());
        let data = file.get(self.offset..end).unwrap_or(&[]);
        let order = if self.little {
            Order::Little
        } else {
            Order::Big
        };
        match self.pixels {
            Pixels::Plain(s) => {
                let size = s.bytes();
                let start = (p * n * size).min(data.len());
                Ok(samples::convert(&data[start..], s, order, size, out))
            }
            Pixels::Bgra => {
                let (z, c) = (p / 3, p % 3);
                // Red is byte 2, green 1, blue 0.
                let start = (z * n * 4 + 2 - c).min(data.len());
                Ok(samples::convert(&data[start..], Sample::U8, order, 4, out))
            }
            Pixels::PackedNew(s) => Ok(packed_new(data, s, order, w, h, out)),
            Pixels::PackedOld => Ok(packed_old(data, order, w / 2, out)),
        }
    }
}

/// The new packed FFT layout: each row holds W/2 + 1 complex values; the full row is that
/// half reversed followed by the conjugates of its inner values (`|z|` is the same).
fn packed_new(data: &[u8], s: Sample, order: Order, w: usize, h: usize, out: &mut [f32]) -> bool {
    let half = w / 2 + 1;
    let mut packed = vec![0f32; half * h];
    let missing = samples::convert(data, s, order, s.bytes(), &mut packed);
    for (row, src) in out.chunks_exact_mut(w).zip(packed.chunks_exact(half)) {
        for (j, o) in row.iter_mut().enumerate() {
            *o = if j < half {
                src[half - 1 - j]
            } else {
                src[1 + (j - half)]
            };
        }
    }
    missing
}

/// The old packed FFT layout of a 2N × 2N transform (4N² `f32`), as magnitudes.
fn packed_old(data: &[u8], order: Order, n: usize, out: &mut [f32]) -> bool {
    let m = 2 * n;
    let mut t = vec![0f32; m * m];
    let missing = samples::convert(data, Sample::F32, order, 4, &mut t);
    let mag = |re: f32, im: f32| re.hypot(im);
    let at = |i: usize, j: usize| i * m + j;
    let mut a = vec![f32::NAN; m * m];
    // Real values at the DC and Nyquist positions.
    a[at(n, 0)] = t[0].abs();
    a[at(0, 0)] = t[1].abs();
    a[at(n, n)] = t[2 * n * n].abs();
    a[at(0, n)] = t[2 * n * n + 1].abs();
    // Rows 0..N, columns N+1..2N.
    for i in 0..n {
        for k in 0..n - 1 {
            let s = 2 * i * n + 2 + 2 * k;
            a[at(i, n + 1 + k)] = mag(t[s], t[s + 1]);
        }
    }
    // Column 0, rows N+1..2N.
    for r in 0..n - 1 {
        let s = 2 * n + 2 * n * r;
        a[at(n + 1 + r, 0)] = mag(t[s], t[s + 1]);
    }
    // Row N, columns N+1..2N.
    for k in 0..n - 1 {
        let s = 2 * n * n + 2 + 2 * k;
        a[at(n, n + 1 + k)] = mag(t[s], t[s + 1]);
    }
    // Rows N+1..2N, columns N..2N.
    let base = 2 * n * n + 2 * n;
    for r in 0..n - 1 {
        for c in 0..n {
            let s = base + 2 * (r * n + c);
            a[at(n + 1 + r, n + c)] = mag(t[s], t[s + 1]);
        }
    }
    // The rest by symmetry: |A(i, j)| = |A(2N - i, 2N - j)|.
    for i in 0..m {
        for j in 0..m {
            if a[at(i, j)].is_nan() {
                a[at(i, j)] = a[at((m - i) % m, (m - j) % m)];
            }
        }
    }
    out.copy_from_slice(&a);
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny DM3 or DM4 file with one 3 × 2 `u16` image, a thumbnail and calibrations.
    pub(crate) fn tiny(dm4: bool, little: bool) -> Vec<u8> {
        let mut f = Vec::new();
        let word = |f: &mut Vec<u8>, v: u64| {
            if dm4 {
                f.extend(v.to_be_bytes());
            } else {
                f.extend((v as u32).to_be_bytes());
            }
        };
        let num = |v: u32| -> Vec<u8> {
            if little {
                v.to_le_bytes().to_vec()
            } else {
                v.to_be_bytes().to_vec()
            }
        };
        let f32b = |v: f32| -> Vec<u8> {
            if little {
                v.to_le_bytes().to_vec()
            } else {
                v.to_be_bytes().to_vec()
            }
        };
        // Builders producing tag bytes.
        fn tag(dm4: bool, kind: u8, name: &str, body: Vec<u8>) -> Vec<u8> {
            let mut t = vec![kind];
            t.extend((name.len() as u16).to_be_bytes());
            t.extend(name.as_bytes());
            if dm4 {
                t.extend((body.len() as u64).to_be_bytes());
            }
            t.extend(body);
            t
        }
        let w = |v: u64| -> Vec<u8> {
            if dm4 {
                v.to_be_bytes().to_vec()
            } else {
                (v as u32).to_be_bytes().to_vec()
            }
        };
        let data = |info: &[u64], value: Vec<u8>| -> Vec<u8> {
            let mut b = b"%%%%".to_vec();
            b.extend(w(info.len() as u64));
            for &i in info {
                b.extend(w(i));
            }
            b.extend(value);
            b
        };
        let group = |tags: Vec<Vec<u8>>| -> Vec<u8> {
            let mut b = vec![0, 1];
            b.extend(w(tags.len() as u64));
            for t in tags {
                b.extend(t);
            }
            b
        };
        let utf16 = |s: &str| -> Vec<u8> {
            s.encode_utf16()
                .flat_map(|u| {
                    if little {
                        u.to_le_bytes()
                    } else {
                        u.to_be_bytes()
                    }
                })
                .collect()
        };
        let cal = |origin: f32, scale: f32, units: &str| {
            group(vec![
                tag(dm4, 21, "Origin", data(&[6], f32b(origin))),
                tag(dm4, 21, "Scale", data(&[6], f32b(scale))),
                tag(
                    dm4,
                    21,
                    "Units",
                    data(&[20, 4, units.len() as u64], utf16(units)),
                ),
            ])
        };
        let pixels: Vec<u8> = (1u16..=6)
            .flat_map(|v| {
                if little {
                    v.to_le_bytes()
                } else {
                    v.to_be_bytes()
                }
            })
            .collect();
        let img = |pixels: Vec<u8>, w_: u32, h_: u32, mag: bool| {
            let mut it = vec![
                tag(dm4, 21, "DataType", data(&[5], num(10))),
                tag(
                    dm4,
                    21,
                    "Data",
                    data(&[20, 4, pixels.len() as u64 / 2], pixels),
                ),
                tag(
                    dm4,
                    20,
                    "Dimensions",
                    group(vec![
                        tag(dm4, 21, "", data(&[5], num(w_))),
                        tag(dm4, 21, "", data(&[5], num(h_))),
                    ]),
                ),
                tag(
                    dm4,
                    20,
                    "Calibrations",
                    group(vec![
                        tag(dm4, 20, "Brightness", cal(0.0, 1.0, "counts")),
                        tag(
                            dm4,
                            20,
                            "Dimension",
                            group(vec![
                                tag(dm4, 20, "", cal(1.0, 0.5, "nm")),
                                tag(dm4, 20, "", cal(0.0, 0.25, "nm")),
                            ]),
                        ),
                    ]),
                ),
            ];
            let tagsg = if mag {
                group(vec![tag(
                    dm4,
                    20,
                    "Microscope Info",
                    group(vec![
                        tag(
                            dm4,
                            21,
                            "Voltage",
                            data(&[7], {
                                if little {
                                    200000f64.to_le_bytes().to_vec()
                                } else {
                                    200000f64.to_be_bytes().to_vec()
                                }
                            }),
                        ),
                        tag(dm4, 21, "Microscope", data(&[18, 5], b"Titan".to_vec())),
                    ]),
                )])
            } else {
                group(vec![])
            };
            it.push(tag(
                dm4,
                21,
                "Quirk",
                data(&[15, 0, 2, 0, 2, 0, 8], {
                    let mut b = num(7)[..2].to_vec();
                    b.push(1);
                    b
                }),
            ));
            group(vec![
                tag(dm4, 20, "ImageData", group(it)),
                tag(dm4, 20, "ImageTags", tagsg),
            ])
        };
        let thumb = img(vec![0; 2], 1, 1, false);
        let main = img(pixels, 3, 2, true);
        let root = group(vec![
            tag(
                dm4,
                20,
                "ImageList",
                group(vec![tag(dm4, 20, "", thumb), tag(dm4, 20, "", main)]),
            ),
            tag(
                dm4,
                20,
                "Thumbnails",
                group(vec![tag(
                    dm4,
                    20,
                    "",
                    group(vec![tag(dm4, 21, "ImageIndex", data(&[5], num(0)))]),
                )]),
            ),
        ]);
        f.extend((if dm4 { 4u32 } else { 3 }).to_be_bytes());
        word(&mut f, 0);
        f.extend(u32::from(little).to_be_bytes());
        f.extend(root);
        f.extend([0u8; 8]);
        f
    }

    #[test]
    fn tiny_files() {
        for dm4 in [false, true] {
            for little in [false, true] {
                let f = tiny(dm4, little);
                let dm = parse(&f).unwrap();
                assert_eq!(dm.version, Version { dm4, little });
                assert_eq!(dm.images.len(), 1, "thumbnail skipped");
                let im = &dm.images[0];
                assert_eq!(im.index, 1);
                assert_eq!((im.width(), im.height(), im.planes()), (3, 2, 1));
                assert_eq!(im.calibrations[0].units, "nm");
                assert_eq!(im.calibrations[0].at(3.0), 1.0);
                assert_eq!(im.calibrations[1].scale, 0.25);
                assert_eq!(im.value_unit(), Some("counts"));
                assert_eq!(im.meta.voltage, Some(200000.0));
                assert_eq!(im.meta.microscope.as_deref(), Some("Titan"));
                let mut out = vec![0f32; 6];
                assert!(!im.read_plane(&f, 0, &mut out).unwrap());
                assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
                // Truncated anywhere: an error or a parse, never a panic.
                for cut in 0..f.len() {
                    if let Ok(d) = parse(&f[..cut]) {
                        let mut out = vec![0f32; 6];
                        let _ = d.images[0].read_plane(&f[..cut], 0, &mut out);
                    }
                }
            }
        }
    }

    #[test]
    fn not_dm() {
        assert_eq!(
            parse(b"\x89PNG\r\n\x1a\n........").unwrap_err(),
            Error::NotDm
        );
        assert!(sniff(&[0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 9]).is_none());
    }

    #[test]
    fn packed_layouts() {
        // New layout, W = 4, H = 1: half = 3 values a, b, c → |c| |b| |a| |b|.
        let mut d = Vec::new();
        for v in [1f32, 0.0, 0.0, 2.0, 3.0, 4.0] {
            d.extend(v.to_le_bytes());
        }
        let mut out = vec![0f32; 4];
        packed_new(&d, Sample::ComplexF32, Order::Little, 4, 1, &mut out);
        assert_eq!(out, [5.0, 2.0, 1.0, 2.0]);
        // Old layout, N = 1 (2 × 2): four reals.
        let mut d = Vec::new();
        for v in [-1f32, 2.0, 3.0, -4.0] {
            d.extend(v.to_le_bytes());
        }
        let mut out = vec![0f32; 4];
        packed_old(&d, Order::Little, 1, &mut out);
        assert_eq!(out, [2.0, 4.0, 1.0, 3.0]);
    }
}

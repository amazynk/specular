//! ImageJ ROI files (`.roi`, big-endian binary) and ROI sets (`.zip` of `.roi` files), read
//! and written from the published layout of the format.
//!
//! A file is a 64-byte header (`Iout`, version, type, integer bounds, point count, four
//! floats, options, stack position, the offset of a second header), the coordinates, a
//! second 64-byte header (channel/slice/frame position, the name's offset and length) and
//! the name in UTF-16BE. ImageJ uses this crate's coordinate convention (pixel corners), so
//! shapes keep their positions. Non-integer coordinates are written with the sub-pixel
//! option (float coordinates after the integer ones; float bounds for rectangles and ovals),
//! and points always are, so everything Specular writes reads back to 32-bit float
//! precision (exactly for integer and binary-fraction coordinates).
//!
//! Limits of the format: integer coordinates are 16-bit (an image up to 32767 pixels across)
//! and a shape has at most 65535 points; composite (shape) ROIs, text and image ROIs are not
//! read.

use std::io::{Cursor, Read as _, Write as _};

use crate::calib::Calibration;
use crate::geom::{P, Shape};
use crate::list::Measurement;

const MAGIC: &[u8; 4] = b"Iout";
/// The version written (ImageJ 1.52+ files carry 228).
pub const VERSION: u16 = 228;
const HEADER: usize = 64;
const HEADER2: usize = 64;
const SUB_PIXEL: u16 = 128;

const POLYGON: u8 = 0;
const RECT: u8 = 1;
const OVAL: u8 = 2;
const LINE: u8 = 3;
const FREELINE: u8 = 4;
const POLYLINE: u8 = 5;
const FREEHAND: u8 = 7;
const TRACED: u8 = 8;
const ANGLE: u8 = 9;
const POINT: u8 = 10;

/// Why a ROI could not be written or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A coordinate outside ImageJ's 16-bit range, or more than 65535 points.
    TooLarge,
    /// Not finite, or too few points.
    Invalid,
    /// Not an ImageJ ROI, or cut short.
    NotRoi,
    /// A ROI type this reader does not take (composite, text, image, no ROI).
    Unsupported(u8),
    Zip(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TooLarge => f.write_str("too large for an ImageJ ROI"),
            Error::Invalid => f.write_str("not a valid shape"),
            Error::NotRoi => f.write_str("not an ImageJ ROI"),
            Error::Unsupported(t) => write!(f, "unsupported ImageJ ROI type {t}"),
            Error::Zip(e) => write!(f, "ROI set: {e}"),
        }
    }
}

impl std::error::Error for Error {}

fn put16(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn put32(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

fn putf(b: &mut [u8], at: usize, v: f32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

fn short(v: f64) -> Result<i16, Error> {
    if (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&v) {
        Ok(v as i16)
    } else {
        Err(Error::TooLarge)
    }
}

/// One measurement as a `.roi` file: its shape, its label as the name, its frame as the
/// stack position.
pub fn encode(m: &Measurement) -> Result<Vec<u8>, Error> {
    let shape = &m.shape;
    if !shape.is_valid() {
        return Err(Error::Invalid);
    }
    let (kind, pts): (u8, Vec<P>) = match shape {
        Shape::Rect { .. } => (RECT, Vec::new()),
        Shape::Ellipse { .. } => (OVAL, Vec::new()),
        Shape::Line(..) => (LINE, Vec::new()),
        Shape::Polygon(v) => (POLYGON, v.clone()),
        Shape::Freehand(v) => (FREEHAND, v.clone()),
        Shape::Polyline(v) => (POLYLINE, v.clone()),
        Shape::Angle { .. } => (ANGLE, shape.points()),
        Shape::Point(p) => (POINT, vec![*p]),
    };
    if pts.len() > usize::from(u16::MAX) {
        return Err(Error::TooLarge);
    }
    let n = pts.len();
    let (bx0, by0, bx1, by1) = shape.bounds();
    // An ellipse's points are its box corners, so this covers its bounds too.
    let integral = shape
        .points()
        .iter()
        .all(|p| p.x.fract() == 0.0 && p.y.fract() == 0.0);
    let sub = !integral || kind == POINT;
    let (left, top) = (bx0.floor(), by0.floor());
    let (right, bottom) = (bx1.ceil(), by1.ceil());
    let coords = if n > 0 {
        n * 4 + if sub { n * 8 } else { 0 }
    } else {
        0
    };
    let h2 = HEADER + coords;
    let name: Vec<u16> = m.label.encode_utf16().collect();
    let mut b = vec![0u8; h2 + HEADER2 + name.len() * 2];
    b[0..4].copy_from_slice(MAGIC);
    put16(&mut b, 4, VERSION as i16);
    b[6] = kind;
    put16(&mut b, 8, short(top)?);
    put16(&mut b, 10, short(left)?);
    put16(&mut b, 12, short(bottom)?);
    put16(&mut b, 14, short(right)?);
    b[16..18].copy_from_slice(&(n as u16).to_be_bytes());
    let mut options = 0u16;
    match *shape {
        Shape::Line(a, c) => {
            for (i, v) in [a.x, a.y, c.x, c.y].into_iter().enumerate() {
                putf(&mut b, 18 + 4 * i, v as f32);
            }
        }
        Shape::Rect { .. } | Shape::Ellipse { .. } if sub => {
            options |= SUB_PIXEL;
            for (i, v) in [bx0, by0, bx1 - bx0, by1 - by0].into_iter().enumerate() {
                putf(&mut b, 18 + 4 * i, v as f32);
            }
        }
        _ => {}
    }
    if n > 0 {
        for (i, p) in pts.iter().enumerate() {
            put16(&mut b, HEADER + 2 * i, short(p.x.round() - left)?);
            put16(&mut b, HEADER + 2 * n + 2 * i, short(p.y.round() - top)?);
        }
        if sub {
            options |= SUB_PIXEL;
            for (i, p) in pts.iter().enumerate() {
                putf(&mut b, HEADER + 4 * n + 4 * i, p.x as f32);
                putf(&mut b, HEADER + 8 * n + 4 * i, p.y as f32);
            }
        }
    }
    b[50..52].copy_from_slice(&options.to_be_bytes());
    let position = m
        .frame
        .map_or(0, |f| f.saturating_add(1).min(i32::MAX as u32) as i32);
    put32(&mut b, 56, position);
    put32(&mut b, 60, h2 as i32);
    // Second header: the name after it.
    put32(&mut b, h2 + 16, (h2 + HEADER2) as i32);
    put32(&mut b, h2 + 20, name.len() as i32);
    for (i, u) in name.iter().enumerate() {
        b[h2 + HEADER2 + 2 * i..h2 + HEADER2 + 2 * i + 2].copy_from_slice(&u.to_be_bytes());
    }
    Ok(b)
}

fn get16(b: &[u8], at: usize) -> Result<i16, Error> {
    Ok(i16::from_be_bytes(
        b.get(at..at + 2)
            .ok_or(Error::NotRoi)?
            .try_into()
            .map_err(|_| Error::NotRoi)?,
    ))
}

fn getu16(b: &[u8], at: usize) -> Result<u16, Error> {
    Ok(get16(b, at)? as u16)
}

fn get32(b: &[u8], at: usize) -> Result<i32, Error> {
    Ok(i32::from_be_bytes(
        b.get(at..at + 4)
            .ok_or(Error::NotRoi)?
            .try_into()
            .map_err(|_| Error::NotRoi)?,
    ))
}

fn getf(b: &[u8], at: usize) -> Result<f64, Error> {
    Ok(f64::from(f32::from_bits(get32(b, at)? as u32)))
}

/// Reads a `.roi` file as a measurement (uncalibrated, id 0).
pub fn decode(b: &[u8]) -> Result<Measurement, Error> {
    if b.len() < HEADER || &b[0..4] != MAGIC {
        return Err(Error::NotRoi);
    }
    let version = getu16(b, 4)?;
    let kind = b[6];
    let top = f64::from(get16(b, 8)?);
    let left = f64::from(get16(b, 10)?);
    let bottom = f64::from(get16(b, 12)?);
    let right = f64::from(get16(b, 14)?);
    let n = usize::from(getu16(b, 16)?);
    let options = getu16(b, 50)?;
    let sub = options & SUB_PIXEL != 0 && version >= 222;
    let floats = |i: usize| getf(b, 18 + 4 * i);
    let box_shape = |oval: bool| -> Result<Shape, Error> {
        let (x, y, w, h) = if sub {
            (floats(0)?, floats(1)?, floats(2)?, floats(3)?)
        } else {
            (left, top, right - left, bottom - top)
        };
        let (a, c) = (P::new(x, y), P::new(x + w, y + h));
        Ok(if oval {
            Shape::ellipse_in(a, c)
        } else {
            Shape::rect(a, c)
        })
    };
    let polygon = || -> Result<Vec<P>, Error> {
        if b.len() < HEADER + 4 * n {
            return Err(Error::NotRoi);
        }
        if sub && b.len() >= HEADER + 12 * n {
            (0..n)
                .map(|i| {
                    Ok(P::new(
                        getf(b, HEADER + 4 * n + 4 * i)?,
                        getf(b, HEADER + 8 * n + 4 * i)?,
                    ))
                })
                .collect()
        } else {
            (0..n)
                .map(|i| {
                    Ok(P::new(
                        left + f64::from(get16(b, HEADER + 2 * i)?),
                        top + f64::from(get16(b, HEADER + 2 * n + 2 * i)?),
                    ))
                })
                .collect()
        }
    };
    let shape = match kind {
        RECT if get32(b, 36)? > 0 => return Err(Error::Unsupported(kind)),
        RECT => box_shape(false)?,
        OVAL => box_shape(true)?,
        LINE => Shape::Line(
            P::new(floats(0)?, floats(1)?),
            P::new(floats(2)?, floats(3)?),
        ),
        POLYGON | TRACED => Shape::Polygon(polygon()?),
        FREEHAND => Shape::Freehand(polygon()?),
        POLYLINE | FREELINE => {
            let v = polygon()?;
            match v.as_slice() {
                [a, c] => Shape::Line(*a, *c),
                _ => Shape::Polyline(v),
            }
        }
        ANGLE => match polygon()?.as_slice() {
            [a, v, c] => Shape::Angle {
                a: *a,
                vertex: *v,
                b: *c,
            },
            _ => return Err(Error::Invalid),
        },
        // A multi-point ROI is read as its first point (the reader returns one shape).
        POINT => Shape::Point(*polygon()?.first().ok_or(Error::Invalid)?),
        k => return Err(Error::Unsupported(k)),
    };
    if !shape.is_valid() {
        return Err(Error::Invalid);
    }
    let mut m = Measurement::new(shape, Calibration::uncalibrated());
    let mut position = get32(b, 56).unwrap_or(0);
    let h2 = usize::try_from(get32(b, 60).unwrap_or(0)).unwrap_or(0);
    if h2 >= HEADER && b.len() >= h2 + HEADER2 {
        if position <= 0 {
            // The second header's slice, else frame, position.
            position = [8, 12, 4]
                .into_iter()
                .filter_map(|o| get32(b, h2 + o).ok())
                .find(|&p| p > 0)
                .unwrap_or(0);
        }
        let off = usize::try_from(get32(b, h2 + 16)?).unwrap_or(0);
        let len = usize::try_from(get32(b, h2 + 20)?).unwrap_or(0);
        if off > 0 && len > 0 && b.len() >= off + 2 * len {
            let units: Vec<u16> = (0..len)
                .map(|i| u16::from_be_bytes([b[off + 2 * i], b[off + 2 * i + 1]]))
                .collect();
            m.label = String::from_utf16_lossy(&units);
        }
    }
    if position > 0 {
        m.frame = Some(position as u32 - 1);
    }
    Ok(m)
}

/// An ImageJ ROI set: a `.zip` of one stored `.roi` per measurement.
pub fn write_zip(items: &[Measurement]) -> Result<Vec<u8>, Error> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for m in items {
        let data = encode(m)?;
        let stem: String = m
            .label
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(40)
            .collect();
        let name = if stem.is_empty() {
            format!("{:04}.roi", m.id)
        } else {
            format!("{:04}-{stem}.roi", m.id)
        };
        z.start_file(name, opts)
            .map_err(|e| Error::Zip(e.to_string()))?;
        z.write_all(&data).map_err(|e| Error::Zip(e.to_string()))?;
    }
    Ok(z.finish()
        .map_err(|e| Error::Zip(e.to_string()))?
        .into_inner())
}

/// Most bytes read from one member of a ROI set.
const MEMBER_LIMIT: u64 = 16 << 20;

/// The measurements of an ImageJ ROI set, and how many members could not be read.
pub fn read_zip(data: &[u8]) -> Result<(Vec<Measurement>, usize), Error> {
    let mut z = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| Error::Zip(e.to_string()))?;
    let (mut out, mut bad) = (Vec::new(), 0);
    for i in 0..z.len() {
        let Ok(f) = z.by_index(i) else {
            bad += 1;
            continue;
        };
        if f.is_dir()
            || !f
                .name()
                .is_ok_and(|n| n.to_ascii_lowercase().ends_with(".roi"))
        {
            continue;
        }
        let mut b = Vec::new();
        if f.take(MEMBER_LIMIT).read_to_end(&mut b).is_err() {
            bad += 1;
            continue;
        }
        match decode(&b) {
            Ok(m) => out.push(m),
            Err(_) => bad += 1,
        }
    }
    Ok((out, bad))
}

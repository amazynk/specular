//! InfiRay `.irg` (the C200/C201 handheld cameras, P200 pocket cameras, and drones that
//! reuse the format).
//!
//! Little-endian. A header of its own length (u16 at 2) starting with one of three magic
//! pairs and, for two of them, ending with `AC CA`; then an 8-bit contrast-stretched image,
//! then the 16-bit absolute image in steps of 1/16 or 1/10 kelvin, then (on some models) a
//! JPEG visible photo. Layout as documented by the MIT-licensed `infiray_irg` reader
//! (jaseg), from which only the field positions are taken.

use crate::{
    Camera, Conditions, Error, Kind, Mapping, Pixels, Result, Thermal, Visible, malformed,
};

const F: &str = "InfiRay IRG";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Model {
    /// C200/C201 (`CA AC`): 1/16 K, or 1/10 K when flag 0 is 1; a JPEG of stated length.
    C201,
    /// `BA AB`: 1/10 K; the rest of the file is a JPEG.
    Other,
    /// P200 (`04 A0`): 1/10 K from 273.2; no photo.
    P200,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

struct Header {
    model: Model,
    len: usize,
    width: usize,
    height: usize,
    flag0: u16,
    jpeg_len: usize,
    emissivity: f64,
    distance: f64,
}

fn header(d: &[u8]) -> Option<Header> {
    let model = match d.get(..2)? {
        [0xca, 0xac] => Model::C201,
        [0xba, 0xab] => Model::Other,
        [0x04, 0xa0] => Model::P200,
        _ => return None,
    };
    let len = usize::from(u16_at(d, 2)?);
    if len < 78 || len > d.len() {
        return None;
    }
    if model != Model::P200 && d[len - 2..len] != [0xac, 0xca] {
        return None;
    }
    let coarse = u32_at(d, 4)? as usize;
    let height = usize::from(u16_at(d, 8)?);
    let width = usize::from(u16_at(d, 10)?);
    if width == 0 || height == 0 || coarse != width * height {
        return None;
    }
    Some(Header {
        model,
        len,
        width,
        height,
        flag0: u16_at(d, 12)?,
        jpeg_len: u32_at(d, 22)? as usize,
        emissivity: f64::from(u32_at(d, 30)?) / 1e4,
        distance: f64::from(u32_at(d, 42)?) / 1e4,
    })
}

/// A header that checks out and the two images it promises (a two-byte magic alone is too
/// weak for content sniffing).
pub(crate) fn is_irg(d: &[u8]) -> bool {
    header(d).is_some_and(|h| d.len() >= h.len + 3 * h.width * h.height)
}

pub(crate) fn read(d: &[u8]) -> Result<Thermal> {
    let h = header(d).ok_or(Error::NotThermal)?;
    let n = h.width * h.height;
    let fine_at = h.len + n;
    let fine = d
        .get(fine_at..fine_at + 2 * n)
        .ok_or_else(|| malformed(F, "image data shorter than the header says"))?;
    let values = fine
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let rest = &d[fine_at + 2 * n..];
    let (divisor, offset, jpeg) = match h.model {
        Model::C201 => {
            let jpeg = (h.jpeg_len > 0).then(|| rest.get(..h.jpeg_len)).flatten();
            let divisor = if h.flag0 == 1 { 10.0 } else { 16.0 };
            (divisor, 273.15, jpeg)
        }
        Model::Other => (10.0, 273.15, Some(rest)),
        Model::P200 => (10.0, 273.2, None),
    };
    let visible = jpeg
        .filter(|j| j.starts_with(&[0xff, 0xd8]))
        .map(|j| Visible {
            bytes: j.to_vec(),
            width: 0,
            height: 0,
        });
    Ok(Thermal {
        kind: Kind::InfirayIrg,
        width: h.width,
        height: h.height,
        pixels: Pixels::Counts {
            values,
            mapping: Mapping::Linear { divisor, offset },
        },
        conditions: Conditions {
            emissivity: Some(h.emissivity),
            distance: Some(h.distance),
            ..Default::default()
        },
        camera: Camera::default(),
        palette: None,
        visible,
        pip: None,
    })
}

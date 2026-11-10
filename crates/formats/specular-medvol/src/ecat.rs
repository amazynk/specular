//! CTI / Siemens ECAT 7 PET images (`.v`), read the way nibabel documents the format.
//!
//! The file is 512-byte blocks: a main header ("MATRIX7…", the calibration factor, isotope,
//! tracer, patient orientation), then a chained matrix list (one row per frame: its id, the
//! block of its subheader, the last block of its data, a status), and per frame a subheader
//! block (data type, sizes, offsets, scale factor, voxel sizes in cm) followed by the voxels.
//! The byte order is the main header's: little-endian when its software version reads as 74
//! that way, else big-endian (as files written on Sun workstations are).
//!
//! Values are stored × scale factor × calibration factor. Frames are ordered by matrix id.
//! As nibabel does, the voxels are flipped by the patient orientation (all three axes for
//! the head-first codes, rows and slices for the feet-first ones) and placed by the first
//! frame's voxel sizes and offsets; that placement is not a stated patient orientation.

use crate::planes::{self, Src};
use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, MedVolume,
    OrientationSource, Param, ParamValue, Result, Skip,
};

const BLOCK: usize = 512;

/// Most matrix-list blocks followed (31 frames each).
const MAX_LIST_BLOCKS: usize = 4096;

/// Whether `data` starts with the ECAT 7 main header's magic.
pub fn sniff(data: &[u8]) -> bool {
    data.len() >= BLOCK && data.starts_with(b"MATRIX7")
}

struct Rd<'a> {
    b: &'a [u8],
    big: bool,
}

impl Rd<'_> {
    fn u16(&self, at: usize) -> u16 {
        let x = [self.b[at], self.b[at + 1]];
        if self.big {
            u16::from_be_bytes(x)
        } else {
            u16::from_le_bytes(x)
        }
    }
    fn i32(&self, at: usize) -> i32 {
        let x = [self.b[at], self.b[at + 1], self.b[at + 2], self.b[at + 3]];
        if self.big {
            i32::from_be_bytes(x)
        } else {
            i32::from_le_bytes(x)
        }
    }
    fn u32(&self, at: usize) -> u32 {
        self.i32(at) as u32
    }
    fn f32(&self, at: usize) -> f64 {
        f64::from(f32::from_bits(self.u32(at)))
    }
    fn text(&self, at: usize, n: usize) -> Option<String> {
        crate::field_text(&self.b[at..at + n])
    }
}

/// One frame's subheader fields this reader uses.
#[derive(Clone, Debug, PartialEq)]
struct Frame {
    data_at: usize,
    dtype: u16,
    size: [usize; 3],
    offset: [f64; 3],
    scale: f64,
    pixel: [f64; 3],
}

fn dtype(code: u16) -> Result<DataType> {
    Ok(match code {
        1 => DataType::U8,
        2 | 6 => DataType::I16,
        3 | 7 => DataType::I32,
        5 => DataType::F32,
        4 => {
            return Err(Error::Unsupported(
                "ECAT data in VAX floating point is not supported".into(),
            ));
        }
        c => {
            return Err(Error::Unsupported(format!("ECAT data type {c}")));
        }
    })
}

/// The file's frames in matrix-id order, with the byte order.
fn frames(b: &[u8]) -> Result<(Vec<Frame>, bool)> {
    if !sniff(b) {
        return Err(Error::NotRecognised);
    }
    let big = u16::from_le_bytes([b[46], b[47]]) != 74;
    let r = Rd { b, big };
    let file_type = r.u16(50);
    if !matches!(file_type, 2 | 6 | 7 | 10) {
        let what = match file_type {
            1 | 11 | 12 | 14 => "a sinogram (scanner data, not yet reconstructed)",
            3 => "an attenuation correction",
            4 | 13 => "a normalisation",
            5 => "a polar map",
            8 | 9 => "projections",
            _ => "not an image",
        };
        return Err(Error::Unsupported(format!(
            "this ECAT 7 file holds {what}; only images are shown"
        )));
    }
    let mut rows = Vec::new();
    let mut blk = 2usize;
    for _ in 0..MAX_LIST_BLOCKS {
        let at = (blk - 1) * BLOCK;
        if at + BLOCK > b.len() {
            return Err(Error::Truncated("ECAT matrix list".into()));
        }
        let n_free = r.i32(at);
        let next = r.i32(at + 4);
        let n_used = r.i32(at + 12);
        if n_free.checked_add(n_used) != Some(31) || !(0..=31).contains(&n_used) {
            break;
        }
        for k in 1..=n_used as usize {
            let row = at + 16 * k;
            rows.push((r.i32(row), r.i32(row + 4)));
        }
        if next <= 2 {
            break;
        }
        blk = next as usize;
    }
    let mut list: Vec<(i32, i32)> = rows.into_iter().take_while(|&(_, s)| s != 0).collect();
    list.retain(|&(id, _)| id > 0);
    if list.is_empty() {
        return Err(Error::Malformed(
            "ECAT: no frames in the matrix list".into(),
        ));
    }
    list.sort_by_key(|&(id, _)| id);
    let mut out = Vec::with_capacity(list.len());
    for (_, sub) in list {
        let at = usize::try_from(sub - 1).map_err(|_| Error::Malformed("ECAT subheader".into()))?
            * BLOCK;
        if at + BLOCK > b.len() {
            return Err(Error::Truncated("ECAT subheader".into()));
        }
        let size = [4, 6, 8].map(|o| usize::from(r.u16(at + o)));
        out.push(Frame {
            data_at: at + BLOCK,
            dtype: r.u16(at),
            size,
            offset: [r.f32(at + 10), r.f32(at + 14), r.f32(at + 18)],
            scale: r.f32(at + 26),
            pixel: [r.f32(at + 34), r.f32(at + 38), r.f32(at + 42)].map(|x| x * 10.0),
        });
    }
    Ok((out, big))
}

/// What the header and the voxel gathering need.
struct Plan {
    header: Header,
    planes: Vec<Src>,
    stored: DataType,
    flip: [bool; 3],
    big: bool,
}

fn plan(b: &[u8]) -> Result<Plan> {
    let (frames, big) = frames(b)?;
    let r = Rd { b, big };
    let f0 = &frames[0];
    for f in &frames[1..] {
        if f.size != f0.size || f.dtype != f0.dtype {
            return Err(Error::Unsupported(
                "ECAT frames of differing sizes or data types".into(),
            ));
        }
    }
    let stored = dtype(f0.dtype)?;
    let [w, h, d] = f0.size;
    if w == 0 || h == 0 || d == 0 {
        return Err(Error::Malformed("ECAT: an empty frame".into()));
    }
    let calib = r.f32(144);
    let orientation = r.u16(330);
    // nibabel's reading: head-first codes flip every axis, feet-first rows and slices.
    let flip = match orientation {
        1 | 3 | 5 | 7 => [true, true, true],
        0 | 2 | 4 | 6 => [false, true, true],
        _ => [false; 3],
    };
    let plane = w * h * stored.size();
    let mut planes = Vec::with_capacity(d * frames.len());
    for f in &frames {
        for k in 0..d {
            let sk = if flip[2] { d - 1 - k } else { k };
            planes.push(Src {
                at: f.data_at + sk * plane,
                slope: calib * f.scale,
                inter: 0.0,
            });
        }
    }
    let (dtype, scale) = planes::output(stored, &planes);
    let nv = frames.len();
    let extra: Vec<usize> = if nv > 1 { vec![nv] } else { Vec::new() };
    let layout = Layout::dense([w, h, d], &extra, 1).ok_or(Error::TooLarge)?;
    let zoom = f0.pixel;
    // nibabel's affine (its RAS), as LPS: x and y negated.
    let t = [0, 1, 2].map(|a| -(f0.size[a] as f64 - 1.0) / 2.0 * zoom[a] + f0.offset[a]);
    let affine = [
        [-zoom[0], 0.0, 0.0, -t[0]],
        [0.0, -zoom[1], 0.0, -t[1]],
        [0.0, 0.0, zoom[2], t[2]],
    ];
    let mut dims = vec![w, h, d];
    if nv > 1 {
        dims.push(nv);
    }
    let mut params = Vec::new();
    let mut text = |key: &'static str, at: usize, n: usize| {
        if let Some(t) = r.text(at, n) {
            params.push(Param {
                key,
                value: ParamValue::Text(t),
            });
        }
    };
    text("isotope", 66, 8);
    text("tracer", 78, 32);
    text("units", 466, 32);
    let start = r.u32(62);
    if start > 0 {
        params.push(Param {
            key: "date",
            value: ParamValue::Text(utc(i64::from(start))),
        });
    }
    Ok(Plan {
        header: Header {
            kind: Kind::Ecat,
            dtype,
            layout,
            affine,
            orientation: OrientationSource::Ecat,
            dims,
            spacing: zoom,
            scale,
            cal: None,
            description: r.text(296, 32),
            intent: None,
            anatomical: None,
            params,
            data: DataRef {
                file: DataFile::Attached,
                skip: Skip::Start { lines: 0, bytes: 0 },
                encoding: Encoding::Container,
                big_endian: big,
            },
        },
        planes,
        stored,
        flip,
        big,
    })
}

/// Seconds since 1970 as "YYYY-MM-DD hh:mm:ss UTC".
fn utc(t: i64) -> String {
    let days = t.div_euclid(86_400);
    let s = t.rem_euclid(86_400);
    // Civil date from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// The volume header of the ECAT 7 file whose bytes are `data` (all of them: the frames'
/// subheaders are spread through the file).
pub fn parse(data: &[u8]) -> Result<Header> {
    Ok(plan(data)?.header)
}

/// Reads the voxels `header` (from [`parse`]) describes from the file's bytes.
pub fn load(header: Header, data: &[u8]) -> Result<MedVolume> {
    let p = plan(data)?;
    if p.header.dtype != header.dtype || p.header.layout != header.layout {
        return Err(Error::Malformed(
            "ECAT header changed since it was read".into(),
        ));
    }
    let [w, h, _] = header.layout.size;
    let buf = planes::gather(
        data,
        p.stored,
        p.big,
        [w, h],
        &p.planes,
        header.dtype,
        [p.flip[0], p.flip[1]],
    )?;
    MedVolume::from_bytes(header, buf, 0, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(1_290_124_615), "2010-11-18 23:56:55 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00 UTC");
    }

    #[test]
    fn sinograms_are_refused_with_the_reason() {
        let mut b = vec![0u8; 2048];
        b[..9].copy_from_slice(b"MATRIX72v");
        b[46..48].copy_from_slice(&74u16.to_le_bytes());
        b[50..52].copy_from_slice(&11u16.to_le_bytes());
        let e = parse(&b).unwrap_err().to_string();
        assert!(e.contains("sinogram"), "{e}");
        b[50..52].copy_from_slice(&7u16.to_le_bytes());
        assert!(parse(&b).is_err());
    }
}

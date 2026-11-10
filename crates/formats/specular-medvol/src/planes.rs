//! Volumes stored as separate planes, each with its own place in the file and its own
//! scaling (Philips PAR/REC images, ECAT 7 frames): gathered into one dense buffer.
//!
//! When every plane has the same slope and intercept the samples keep their type (in little
//! endian) and the scaling stays the header's; otherwise each plane is scaled into `float32`.

use crate::data::{alloc, held, sample_f64};
use crate::{Bytes, DataType, Error, Result};

/// One stored plane: where its first sample is, and value = stored × slope + intercept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Src {
    pub at: usize,
    pub slope: f64,
    pub inter: f64,
}

/// The one scaling all `planes` share, or `None` when they differ.
pub(crate) fn shared_scale(planes: &[Src]) -> Option<(f64, f64)> {
    let first = planes.first()?;
    planes
        .iter()
        .all(|p| p.slope == first.slope && p.inter == first.inter)
        .then_some((first.slope, first.inter))
}

/// The sample type and scaling of the gathered volume: the stored type when the scaling is
/// shared (`None` for none at all), else `float32`, already scaled.
pub(crate) fn output(stored: DataType, planes: &[Src]) -> (DataType, Option<(f64, f64)>) {
    match shared_scale(planes) {
        Some((1.0, 0.0)) => (stored, None),
        Some(s) => (stored, Some(s)),
        None if planes.is_empty() => (stored, None),
        None => (DataType::F32, None),
    }
}

/// Gathers `planes` of `w` × `h` samples of `stored` (in `src`, byte order `big`) into one
/// dense little-endian buffer of `out` samples, columns and rows reversed as `flip` says.
pub(crate) fn gather(
    src: &[u8],
    stored: DataType,
    big: bool,
    [w, h]: [usize; 2],
    planes: &[Src],
    out: DataType,
    flip: [bool; 2],
) -> Result<Bytes> {
    let n = w.checked_mul(h).ok_or(Error::TooLarge)?;
    let plane_bytes = n.checked_mul(stored.size()).ok_or(Error::TooLarge)?;
    for p in planes {
        if p.at.checked_add(plane_bytes).is_none_or(|e| e > src.len()) {
            return Err(Error::Truncated("voxel data".into()));
        }
    }
    let total = n
        .checked_mul(planes.len())
        .and_then(|x| x.checked_mul(out.size()))
        .filter(|&x| x <= crate::data::MAX_DATA)
        .ok_or(Error::TooLarge)?;
    let (mut buf, res) = alloc(total)?;
    let scaled = out != stored;
    let size = stored.size();
    for p in planes {
        for j in 0..h {
            let sj = if flip[1] { h - 1 - j } else { j };
            for i in 0..w {
                let si = if flip[0] { w - 1 - i } else { i };
                let s = sj * w + si;
                if scaled {
                    let v = sample_f64(src, p.at, s, stored, big) * p.slope + p.inter;
                    buf.extend_from_slice(&(v as f32).to_le_bytes());
                } else {
                    let at = p.at + s * size;
                    let b = &src[at..at + size];
                    if big {
                        buf.extend(b.iter().rev());
                    } else {
                        buf.extend_from_slice(b);
                    }
                }
            }
        }
    }
    Ok(held(buf, res))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(at: usize, slope: f64, inter: f64) -> Src {
        Src { at, slope, inter }
    }

    #[test]
    fn shared_scaling_keeps_the_stored_type() {
        let p = [src(0, 2.0, 1.0), src(8, 2.0, 1.0)];
        assert_eq!(output(DataType::U16, &p), (DataType::U16, Some((2.0, 1.0))));
        let q = [src(0, 1.0, 0.0)];
        assert_eq!(output(DataType::I16, &q), (DataType::I16, None));
        let r = [src(0, 2.0, 1.0), src(8, 3.0, 1.0)];
        assert_eq!(output(DataType::U16, &r), (DataType::F32, None));
    }

    #[test]
    fn gathers_flipped_and_scaled() {
        // Two big-endian 2 × 2 int16 planes, the second stored first.
        let data: Vec<u8> = [5i16, 6, 7, 8, 1, 2, 3, 4]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let planes = [src(8, 1.0, 0.0), src(0, 2.0, 0.5)];
        let b = gather(
            &data,
            DataType::I16,
            true,
            [2, 2],
            &planes,
            DataType::F32,
            [true, false],
        )
        .unwrap();
        let v: Vec<f32> = (*b)
            .as_ref()
            .chunks(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(v, [2.0, 1.0, 4.0, 3.0, 12.5, 10.5, 16.5, 14.5]);
        let b = gather(
            &data,
            DataType::I16,
            true,
            [2, 2],
            &planes[..1],
            DataType::I16,
            [false, true],
        )
        .unwrap();
        assert_eq!((*b).as_ref(), &[3, 0, 4, 0, 1, 0, 2, 0]);
        assert!(matches!(
            gather(
                &data,
                DataType::I16,
                true,
                [2, 2],
                &[src(12, 1.0, 0.0)],
                DataType::I16,
                [false; 2]
            ),
            Err(Error::Truncated(_))
        ));
    }
}

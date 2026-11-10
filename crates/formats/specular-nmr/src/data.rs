//! Binary data files: FIDs (`fid`, `ser`) and processed spectra (`1r`, `1i`, `2rr`), 32-bit
//! integers or 64-bit floats in either byte order.

use std::io::Read;
use std::path::Path;

use specular_spectra::Error;

use crate::process::C64;

/// How numbers are stored (`DTYPA`/`DTYPP` and `BYTORDA`/`BYTORDP`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoding {
    pub float64: bool,
    pub big_endian: bool,
}

impl Encoding {
    /// From the type and byte-order parameters (`None` for an unknown type).
    pub fn new(dtype: i64, byte_order: i64) -> Option<Encoding> {
        let float64 = match dtype {
            0 => false,
            2 => true,
            _ => return None,
        };
        Some(Encoding {
            float64,
            big_endian: byte_order == 1,
        })
    }

    pub fn size(self) -> usize {
        if self.float64 { 8 } else { 4 }
    }

    fn decode(self, b: &[u8]) -> f64 {
        if self.float64 {
            let a: [u8; 8] = b.try_into().unwrap_or([0; 8]);
            if self.big_endian {
                f64::from_be_bytes(a)
            } else {
                f64::from_le_bytes(a)
            }
        } else {
            let a: [u8; 4] = b.try_into().unwrap_or([0; 4]);
            f64::from(if self.big_endian {
                i32::from_be_bytes(a)
            } else {
                i32::from_le_bytes(a)
            })
        }
    }
}

/// Values read from one file at most.
pub const MAX_VALUES: usize = 1 << 26;

/// Reads `count` numbers from the start of the regular file `path`.
pub fn read_values(path: &Path, enc: Encoding, count: usize) -> Result<Vec<f64>, Error> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if count > MAX_VALUES {
        return Err(Error::TooLarge);
    }
    let meta = std::fs::metadata(path)
        .map_err(|_| Error::Malformed(format!("the data file {name} is missing")))?;
    if !meta.is_file() {
        return Err(Error::Malformed(format!("{name} is not a regular file")));
    }
    let want = (count * enc.size()) as u64;
    if meta.len() < want {
        return Err(Error::Truncated(format!(
            "{name} holds {} bytes, the parameters ask for {want}",
            meta.len()
        )));
    }
    let mut bytes = Vec::with_capacity(want as usize);
    std::fs::File::open(path)
        .and_then(|f| f.take(want).read_to_end(&mut bytes))
        .map_err(|e| Error::Malformed(format!("cannot read {name}: {e}")))?;
    if (bytes.len() as u64) < want {
        return Err(Error::Truncated(format!("{name} ends early")));
    }
    Ok(bytes
        .chunks_exact(enc.size())
        .map(|b| enc.decode(b))
        .collect())
}

/// Interleaved real and imaginary values as complex points.
pub fn complex(values: &[f64]) -> Vec<C64> {
    values
        .chunks_exact(2)
        .map(|c| C64::new(c[0], c[1]))
        .collect()
}

/// A 2-D processed spectrum stored in submatrices of `tile = (rows, cols)` (`XDIM` of
/// F1 and F2), submatrices row by row, as a plain row-major matrix of `rows × cols`
/// points. `None` when the sizes do not tile.
pub fn untile(values: &[f64], size: (usize, usize), tile: (usize, usize)) -> Option<Vec<f64>> {
    let (rows, cols) = size;
    let (tr, tc) = tile;
    if tr == 0
        || tc == 0
        || !rows.is_multiple_of(tr)
        || !cols.is_multiple_of(tc)
        || values.len() < rows * cols
    {
        return None;
    }
    let mut out = vec![0.0; rows * cols];
    let blocks_across = cols / tc;
    for (b, block) in values
        .chunks_exact(tr * tc)
        .take(rows * cols / (tr * tc))
        .enumerate()
    {
        let (br, bc) = (b / blocks_across, b % blocks_across);
        for r in 0..tr {
            let dst = (br * tr + r) * cols + bc * tc;
            out[dst..dst + tc].copy_from_slice(&block[r * tc..(r + 1) * tc]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submatrices_land_in_place() {
        // 4 × 6 points in 2 × 3 tiles: tile k holds the value 10·k + position in the tile.
        let mut v = Vec::new();
        for k in 0..4 {
            for p in 0..6 {
                v.push(f64::from(10 * k + p));
            }
        }
        let m = untile(&v, (4, 6), (2, 3)).unwrap();
        assert_eq!(&m[0..6], &[0.0, 1.0, 2.0, 10.0, 11.0, 12.0]);
        assert_eq!(&m[6..12], &[3.0, 4.0, 5.0, 13.0, 14.0, 15.0]);
        assert_eq!(&m[12..18], &[20.0, 21.0, 22.0, 30.0, 31.0, 32.0]);
        assert!(untile(&v, (4, 6), (3, 3)).is_none());
    }

    #[test]
    fn encodings() {
        let e = Encoding::new(0, 1).unwrap();
        assert_eq!(e.decode(&(-5i32).to_be_bytes()), -5.0);
        let e = Encoding::new(2, 0).unwrap();
        assert_eq!(e.decode(&1.5f64.to_le_bytes()), 1.5);
        assert!(Encoding::new(1, 0).is_none());
    }
}

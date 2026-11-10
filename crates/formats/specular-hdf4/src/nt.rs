//! Number types: the HDF4 type codes, their byte order and conversion to real values.

use crate::{Error, Num, Result};

/// A number type as stored: what it is and in which byte order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nt {
    /// The HDF4 type code (`DFNT_*` without the byte order and native flags).
    pub code: u8,
    pub num: Num,
    /// Little-endian (`DFNT_LITEND`, or an NT element of the PC class).
    pub little: bool,
}

/// `DFNT_LITEND`: little-endian storage flag of a type code.
pub(crate) const LITEND: u16 = 0x4000;

impl Nt {
    /// From a type code as vdata fields and attributes store it (flags included).
    pub fn from_code(t: u16) -> Result<Nt> {
        let code = (t & 0xff) as u8;
        let num = match code {
            3 | 4 | 21 => Num::U8,
            20 => Num::I8,
            22 => Num::I16,
            23 => Num::U16,
            24 => Num::I32,
            25 => Num::U32,
            26 => Num::I64,
            27 => Num::U64,
            5 => Num::F32,
            6 => Num::F64,
            _ => return Err(Error::Unsupported(format!("number type {t}"))),
        };
        Ok(Nt {
            code,
            num,
            little: t & LITEND != 0,
        })
    }

    /// From an NT element: version, type, width in bits, class (1 big-endian, 4 PC
    /// little-endian; raster images write 0, read as big-endian).
    pub fn from_element(b: &[u8]) -> Result<Nt> {
        if b.len() < 4 {
            return Err(Error::Malformed("number type element".into()));
        }
        let mut nt = Nt::from_code(u16::from(b[1]))?;
        match b[3] {
            0 | 1 => {}
            4 => nt.little = true,
            2 if nt.num.is_float() => {
                return Err(Error::Unsupported("VAX floating point".into()));
            }
            2 => {}
            c => return Err(Error::Unsupported(format!("number class {c}"))),
        }
        if b[2] != 0 && usize::from(b[2]) != nt.size() * 8 {
            return Err(Error::Malformed("number type width".into()));
        }
        Ok(nt)
    }

    pub fn size(self) -> usize {
        self.num.size()
    }

    /// A text type (`char8`, `uchar8`).
    pub fn is_char(self) -> bool {
        matches!(self.code, 3 | 4)
    }

    /// The type's name as the HDF4 tools print it.
    pub fn name(self) -> &'static str {
        match self.code {
            3 => "uchar8",
            4 => "char8",
            5 => "float32",
            6 => "float64",
            20 => "int8",
            21 => "uint8",
            22 => "int16",
            23 => "uint16",
            24 => "int32",
            25 => "uint32",
            26 => "int64",
            _ => "uint64",
        }
    }

    /// E.g. "16-bit signed", "32-bit float, little-endian".
    pub fn describe(self) -> String {
        let mut s = if self.is_char() {
            "8-bit character".to_owned()
        } else {
            self.num.describe()
        };
        if self.little {
            s += ", little-endian";
        }
        s
    }

    /// The value of one element (`b` holds at least [`Nt::size`] bytes).
    #[inline]
    pub fn get(self, b: &[u8]) -> f64 {
        macro_rules! rd {
            ($t:ty, $n:expr) => {{
                let a: [u8; $n] = b[..$n].try_into().unwrap_or([0; $n]);
                if self.little {
                    <$t>::from_le_bytes(a)
                } else {
                    <$t>::from_be_bytes(a)
                }
            }};
        }
        match self.num {
            Num::U8 => f64::from(b[0]),
            Num::I8 => f64::from(b[0] as i8),
            Num::U16 => f64::from(rd!(u16, 2)),
            Num::I16 => f64::from(rd!(i16, 2)),
            Num::U32 => f64::from(rd!(u32, 4)),
            Num::I32 => f64::from(rd!(i32, 4)),
            Num::U64 => rd!(u64, 8) as f64,
            Num::I64 => rd!(i64, 8) as f64,
            Num::F16 => 0.0,
            Num::F32 => f64::from(rd!(f32, 4)),
            Num::F64 => rd!(f64, 8),
        }
    }

    /// Every element of `b` as real values.
    pub fn values(self, b: &[u8]) -> Vec<f64> {
        b.chunks_exact(self.size()).map(|c| self.get(c)).collect()
    }

    /// The fill the SD interface uses when a dataset gives none (netCDF's defaults), as
    /// stored bytes in this type's order.
    pub fn default_fill(self) -> Vec<u8> {
        let be: Vec<u8> = match self.num {
            Num::U8 | Num::I8 if self.is_char() => vec![0],
            Num::U8 | Num::I8 => vec![0x81],
            Num::U16 | Num::I16 => vec![0x80, 0x01],
            Num::U32 | Num::I32 => vec![0x80, 0, 0, 0x01],
            Num::U64 | Num::I64 => vec![0x80, 0, 0, 0, 0, 0, 0, 0x01],
            Num::F32 | Num::F16 => 9.969_21e36_f32.to_be_bytes().to_vec(),
            Num::F64 => 9.969_209_968_386_869e36_f64.to_be_bytes().to_vec(),
        };
        if self.little {
            be.into_iter().rev().collect()
        } else {
            be
        }
    }

    /// The default float fill, which reads as "no data" when a dataset names no fill.
    pub fn default_float_fill(self) -> Option<f64> {
        match self.num {
            Num::F32 => Some(f64::from(9.969_21e36_f32)),
            Num::F64 => Some(9.969_209_968_386_869e36),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_order() {
        let be = Nt::from_element(&[1, 22, 16, 1]).unwrap();
        assert_eq!((be.num, be.little), (Num::I16, false));
        assert_eq!(be.get(&[0xff, 0xfe]), -2.0);
        let le = Nt::from_element(&[1, 5, 32, 4]).unwrap();
        assert!(le.little);
        assert_eq!(le.get(&1.5f32.to_le_bytes()), 1.5);
        let raster = Nt::from_element(&[1, 3, 8, 0]).unwrap();
        assert!(raster.is_char() && !raster.little);
        let lit = Nt::from_code(0x4000 | 24).unwrap();
        assert_eq!(lit.get(&(-7i32).to_le_bytes()), -7.0);
        assert!(Nt::from_code(7).is_err());
        assert!(Nt::from_element(&[1, 5, 32, 2]).is_err());
        assert_eq!(Nt::from_code(23).unwrap().default_fill(), vec![0x80, 1]);
    }
}

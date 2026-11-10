//! Element types and their conversion from stored bytes and ASCII text.

use crate::{Error, Result, bad, limits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    Bit,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Ty {
    /// Bytes per element (bits are packed, see [`Ty::bytes_for`]).
    pub fn size(self) -> usize {
        match self {
            Ty::Bit | Ty::I8 | Ty::U8 => 1,
            Ty::I16 | Ty::U16 => 2,
            Ty::I32 | Ty::U32 | Ty::F32 => 4,
            Ty::I64 | Ty::U64 | Ty::F64 => 8,
        }
    }

    /// Stored bytes of `n` elements.
    pub fn bytes_for(self, n: u64) -> Option<u64> {
        match self {
            Ty::Bit => Some(n.div_ceil(8)),
            t => n.checked_mul(t.size() as u64),
        }
    }

    /// A legacy type name, with its static spelling.
    pub fn legacy(name: &str) -> Result<(Ty, &'static str)> {
        let n = name.to_ascii_lowercase();
        Ok(match n.as_str() {
            "bit" => (Ty::Bit, "bit"),
            "unsigned_char" => (Ty::U8, "unsigned_char"),
            "char" => (Ty::I8, "char"),
            "signed_char" => (Ty::I8, "signed_char"),
            "short" => (Ty::I16, "short"),
            "unsigned_short" => (Ty::U16, "unsigned_short"),
            "int" => (Ty::I32, "int"),
            "unsigned_int" => (Ty::U32, "unsigned_int"),
            // VTK writes its 64-bit `long` arrays (Linux and macOS) as `long`.
            "long" => (Ty::I64, "long"),
            "unsigned_long" => (Ty::U64, "unsigned_long"),
            "vtktypeint64" | "long_long" | "__int64" => (Ty::I64, "vtktypeint64"),
            "vtktypeuint64" | "unsigned_long_long" | "unsigned___int64" => {
                (Ty::U64, "vtktypeuint64")
            }
            "vtkidtype" => (Ty::I32, "vtkIdType"),
            "float" => (Ty::F32, "float"),
            "double" => (Ty::F64, "double"),
            "string" | "utf8_string" | "variant" => {
                return Err(Error::Unsupported(format!(
                    "VTK {name} arrays are not supported"
                )));
            }
            _ => return bad(format!("unknown data type {name}")),
        })
    }

    /// An XML `type` attribute.
    pub fn xml(name: &str) -> Result<(Ty, &'static str)> {
        Ok(match name {
            "Int8" | "Char" => (Ty::I8, "Int8"),
            "UInt8" | "UnsignedChar" => (Ty::U8, "UInt8"),
            "Int16" | "Short" => (Ty::I16, "Int16"),
            "UInt16" | "UnsignedShort" => (Ty::U16, "UInt16"),
            "Int32" | "Int" => (Ty::I32, "Int32"),
            "UInt32" | "UnsignedInt" => (Ty::U32, "UInt32"),
            "Int64" | "Long" | "IdType" => (Ty::I64, "Int64"),
            "UInt64" | "UnsignedLong" => (Ty::U64, "UInt64"),
            "Float32" | "Float" => (Ty::F32, "Float32"),
            "Float64" | "Double" => (Ty::F64, "Float64"),
            "Bit" => (Ty::Bit, "Bit"),
            "String" => {
                return Err(Error::Unsupported(
                    "VTK string arrays are not supported".into(),
                ));
            }
            _ => return bad(format!("unknown data type {name}")),
        })
    }
}

macro_rules! conv {
    ($b:expr, $n:expr, $big:expr, $t:ty, $sz:expr, $out:expr, $f:expr) => {{
        let it = $b.chunks_exact($sz).take($n);
        if $big {
            $out.extend(it.map(|c| $f(<$t>::from_be_bytes(c.try_into().unwrap_or([0; $sz])))));
        } else {
            $out.extend(it.map(|c| $f(<$t>::from_le_bytes(c.try_into().unwrap_or([0; $sz])))));
        }
    }};
}

/// Reserves room for `n` more values, refusing absurd or unallocatable sizes.
pub(crate) fn reserve<T>(v: &mut Vec<T>, n: u64) -> Result<()> {
    if n > limits::MAX_VALUES {
        return Err(Error::TooLarge);
    }
    v.try_reserve_exact(n as usize).map_err(|_| Error::TooLarge)
}

/// `n` stored elements of `b` (exactly as many bytes as [`Ty::bytes_for`] says, or more) as f32.
pub(crate) fn to_f32(ty: Ty, b: &[u8], n: usize, big: bool, out: &mut Vec<f32>) -> Result<()> {
    let need = ty.bytes_for(n as u64).ok_or(Error::TooLarge)?;
    if (b.len() as u64) < need {
        return Err(Error::Truncated("array data".into()));
    }
    reserve(out, n as u64)?;
    match ty {
        Ty::Bit => out.extend((0..n).map(|i| f32::from((b[i / 8] >> (7 - i % 8)) & 1))),
        Ty::U8 => out.extend(b[..n].iter().map(|&v| f32::from(v))),
        Ty::I8 => out.extend(b[..n].iter().map(|&v| f32::from(v as i8))),
        Ty::I16 => conv!(b, n, big, i16, 2, out, f32::from),
        Ty::U16 => conv!(b, n, big, u16, 2, out, f32::from),
        Ty::I32 => conv!(b, n, big, i32, 4, out, |v| v as f32),
        Ty::U32 => conv!(b, n, big, u32, 4, out, |v| v as f32),
        Ty::I64 => conv!(b, n, big, i64, 8, out, |v| v as f32),
        Ty::U64 => conv!(b, n, big, u64, 8, out, |v| v as f32),
        Ty::F32 => conv!(b, n, big, f32, 4, out, |v| v),
        Ty::F64 => conv!(b, n, big, f64, 8, out, |v| v as f32),
    }
    Ok(())
}

/// `n` stored elements as f64 (coordinates).
pub(crate) fn to_f64(ty: Ty, b: &[u8], n: usize, big: bool) -> Result<Vec<f64>> {
    let mut out = Vec::new();
    let need = ty.bytes_for(n as u64).ok_or(Error::TooLarge)?;
    if (b.len() as u64) < need {
        return Err(Error::Truncated("array data".into()));
    }
    reserve(&mut out, n as u64)?;
    match ty {
        Ty::Bit => out.extend((0..n).map(|i| f64::from((b[i / 8] >> (7 - i % 8)) & 1))),
        Ty::U8 => out.extend(b[..n].iter().map(|&v| f64::from(v))),
        Ty::I8 => out.extend(b[..n].iter().map(|&v| f64::from(v as i8))),
        Ty::I16 => conv!(b, n, big, i16, 2, out, f64::from),
        Ty::U16 => conv!(b, n, big, u16, 2, out, f64::from),
        Ty::I32 => conv!(b, n, big, i32, 4, out, f64::from),
        Ty::U32 => conv!(b, n, big, u32, 4, out, f64::from),
        Ty::I64 => conv!(b, n, big, i64, 8, out, |v| v as f64),
        Ty::U64 => conv!(b, n, big, u64, 8, out, |v| v as f64),
        Ty::F32 => conv!(b, n, big, f32, 4, out, f64::from),
        Ty::F64 => conv!(b, n, big, f64, 8, out, |v| v),
    }
    Ok(out)
}

/// `n` stored integers (cell connectivity, offsets, types).
pub(crate) fn to_i64(ty: Ty, b: &[u8], n: usize, big: bool) -> Result<Vec<i64>> {
    let mut out = Vec::new();
    let need = ty.bytes_for(n as u64).ok_or(Error::TooLarge)?;
    if (b.len() as u64) < need {
        return Err(Error::Truncated("cell data".into()));
    }
    reserve(&mut out, n as u64)?;
    match ty {
        Ty::Bit => out.extend((0..n).map(|i| i64::from((b[i / 8] >> (7 - i % 8)) & 1))),
        Ty::U8 => out.extend(b[..n].iter().map(|&v| i64::from(v))),
        Ty::I8 => out.extend(b[..n].iter().map(|&v| i64::from(v as i8))),
        Ty::I16 => conv!(b, n, big, i16, 2, out, i64::from),
        Ty::U16 => conv!(b, n, big, u16, 2, out, i64::from),
        Ty::I32 => conv!(b, n, big, i32, 4, out, i64::from),
        Ty::U32 => conv!(b, n, big, u32, 4, out, i64::from),
        Ty::I64 => conv!(b, n, big, i64, 8, out, |v| v),
        Ty::U64 => conv!(b, n, big, u64, 8, out, |v| i64::try_from(v).unwrap_or(-1)),
        Ty::F32 => conv!(b, n, big, f32, 4, out, |v: f32| v as i64),
        Ty::F64 => conv!(b, n, big, f64, 8, out, |v: f64| v as i64),
    }
    Ok(out)
}

/// One ASCII number of type `ty` as f32: floats rounded once from the text, integers exact
/// before the conversion.
pub(crate) fn parse_f32(ty: Ty, s: &str) -> Option<f32> {
    match ty {
        Ty::F32 => s
            .parse::<f32>()
            .ok()
            .or_else(|| parse_float(s).map(|v| v as f32)),
        Ty::F64 => parse_float(s).map(|v| v as f32),
        _ => parse_int(s).map(|v| v as f32),
    }
}

pub(crate) fn parse_f64(s: &str) -> Option<f64> {
    parse_float(s)
}

/// An integer, also written as a float ("3.0", "1e3").
pub(crate) fn parse_int(s: &str) -> Option<i128> {
    if let Ok(v) = s.parse::<i128>() {
        return Some(v);
    }
    let f = parse_float(s)?;
    f.is_finite().then_some(f as i128)
}

fn parse_float(s: &str) -> Option<f64> {
    if let Ok(v) = s.parse::<f64>() {
        return Some(v);
    }
    // C++ stream spellings: "nan", "-nan", "inf", "1.#INF".
    let l = s.to_ascii_lowercase();
    let neg = l.starts_with('-');
    let body = l.trim_start_matches(['-', '+']);
    if body.starts_with("nan") || body.contains("#ind") || body.contains("#qnan") {
        Some(f64::NAN)
    } else if body.starts_with("inf") || body.contains("#inf") {
        Some(if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    } else {
        None
    }
}

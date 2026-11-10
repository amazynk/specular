//! Datatype and dataspace messages, and the conversion of stored numbers to real values.

use crate::io::{Ctx, Cur};
use crate::{Error, Result};

/// A numeric element type that converts to real values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Num {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F16,
    F32,
    F64,
}

impl Num {
    pub fn size(self) -> usize {
        match self {
            Num::U8 | Num::I8 => 1,
            Num::U16 | Num::I16 | Num::F16 => 2,
            Num::U32 | Num::I32 | Num::F32 => 4,
            Num::U64 | Num::I64 | Num::F64 => 8,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Num::F16 | Num::F32 | Num::F64)
    }

    /// E.g. "16-bit unsigned", "32-bit float".
    pub fn describe(self) -> String {
        let bits = self.size() * 8;
        match self {
            Num::U8 | Num::U16 | Num::U32 | Num::U64 => format!("{bits}-bit unsigned"),
            Num::I8 | Num::I16 | Num::I32 | Num::I64 => format!("{bits}-bit signed"),
            _ => format!("{bits}-bit float"),
        }
    }
}

/// A datatype.
#[derive(Clone, Debug, PartialEq)]
pub enum Dtype {
    /// Integers and floats (also enums and bitfields, read as their base integers).
    Num { num: Num, be: bool },
    /// Fixed-length string: size, padding (0 NUL-terminated, 1 NUL-padded, 2 space-padded).
    Str { size: usize, pad: u8 },
    /// Variable-length string.
    VlenStr,
    /// Variable-length sequence of the base type.
    VlenSeq(Box<Dtype>),
    /// Object (0) or region (1) reference of `size` bytes.
    Ref { size: usize, kind: u8 },
    /// Fixed-size array of the base type.
    Array { dims: Vec<u32>, base: Box<Dtype> },
    /// Anything else (compound, opaque, time, unusual floats): its class and size.
    Other { class: u8, size: usize },
}

impl Dtype {
    /// Bytes per element as stored in a dataset or attribute.
    pub(crate) fn size(&self, c: &Ctx) -> usize {
        match self {
            Dtype::Num { num, .. } => num.size(),
            Dtype::Str { size, .. } => *size,
            // Length (4), global heap collection address, object index (4).
            Dtype::VlenStr | Dtype::VlenSeq(_) => 4 + c.so as usize + 4,
            Dtype::Ref { size, .. } => *size,
            Dtype::Array { dims, base } => dims
                .iter()
                .fold(base.size(c), |a, &d| a.saturating_mul(d as usize)),
            Dtype::Other { size, .. } => *size,
        }
    }

    /// The numeric type, when the elements are numbers.
    pub fn num(&self) -> Option<(Num, bool)> {
        match self {
            Dtype::Num { num, be } => Some((*num, *be)),
            _ => None,
        }
    }

    /// Short description for the info panel.
    pub fn describe(&self) -> String {
        match self {
            Dtype::Num { num, .. } => num.describe(),
            Dtype::Str { .. } | Dtype::VlenStr => "string".into(),
            Dtype::VlenSeq(_) => "variable-length sequence".into(),
            Dtype::Ref { .. } => "reference".into(),
            Dtype::Array { .. } => "array".into(),
            Dtype::Other { class: 6, .. } => "compound".into(),
            Dtype::Other { class, .. } => format!("class {class}"),
        }
    }
}

/// A numeric member of a compound type (members of nested compounds and the elements of
/// small numeric arrays included).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// The member's name; nested members as "outer.inner", array elements as "name[i]".
    pub name: String,
    /// Byte offset in the compound element.
    pub offset: usize,
    pub num: Num,
    pub be: bool,
}

/// Fields kept for one compound type.
const MAX_FIELDS: usize = 256;
/// Elements of one numeric array member shown as fields.
const MAX_ARRAY_FIELDS: u32 = 16;

/// Decodes a datatype message (or an embedded datatype), returning it and its encoded length.
pub(crate) fn parse_dtype(b: &[u8]) -> Result<(Dtype, usize)> {
    parse_dtype_at(b, 0).map(|(t, n, _)| (t, n))
}

/// The numeric fields of a compound datatype message (none for other types).
pub(crate) fn compound_fields(b: &[u8]) -> Result<Vec<Field>> {
    parse_dtype_at(b, 0).map(|(_, _, f)| f)
}

/// The fields one member of type `t` adds at `offset`, named `name`.
fn member_fields(name: &str, offset: usize, t: &Dtype, sub: Vec<Field>, out: &mut Vec<Field>) {
    match t {
        Dtype::Num { num, be } => out.push(Field {
            name: name.to_owned(),
            offset,
            num: *num,
            be: *be,
        }),
        Dtype::Array { dims, base } => {
            if let Dtype::Num { num, be } = **base {
                let n = dims.iter().fold(1u32, |a, &d| a.saturating_mul(d));
                if n <= MAX_ARRAY_FIELDS {
                    for i in 0..n as usize {
                        out.push(Field {
                            name: format!("{name}[{i}]"),
                            offset: offset + i * num.size(),
                            num,
                            be,
                        });
                    }
                }
            }
        }
        Dtype::Other { class: 6, .. } => {
            for f in sub {
                out.push(Field {
                    name: format!("{name}.{}", f.name),
                    offset: offset + f.offset,
                    ..f
                });
            }
        }
        _ => {}
    }
    out.truncate(MAX_FIELDS);
}

fn parse_dtype_at(b: &[u8], depth: u32) -> Result<(Dtype, usize, Vec<Field>)> {
    if depth > 8 {
        return Err(Error::Malformed("datatype nested too deeply".into()));
    }
    let mut r = Cur::new(b, 0);
    let cv = r.u8()?;
    let (class, version) = (cv & 0x0f, cv >> 4);
    let bits = [r.u8()?, r.u8()?, r.u8()?];
    let size = r.u32()? as usize;
    let t = match class {
        // Fixed point and bitfield.
        0 | 4 => {
            let _offset = r.u16()?;
            let prec = r.u16()?;
            let signed = class == 0 && bits[0] & 0x08 != 0;
            let be = bits[0] & 1 != 0;
            let num = match (size, signed) {
                (1, false) => Some(Num::U8),
                (1, true) => Some(Num::I8),
                (2, false) => Some(Num::U16),
                (2, true) => Some(Num::I16),
                (4, false) => Some(Num::U32),
                (4, true) => Some(Num::I32),
                (8, false) => Some(Num::U64),
                (8, true) => Some(Num::I64),
                _ => None,
            };
            match num {
                Some(num) if prec as usize <= size * 8 => Dtype::Num { num, be },
                _ => Dtype::Other { class, size },
            }
        }
        1 => {
            let _offset = r.u16()?;
            let _prec = r.u16()?;
            let _eloc = r.u8()?;
            let esize = r.u8()?;
            let _mloc = r.u8()?;
            let msize = r.u8()?;
            let _bias = r.u32()?;
            // Byte order: bit 0, with bit 6 for VAX order (unsupported).
            let be = bits[0] & 1 != 0;
            let vax = bits[0] & 0x40 != 0;
            let num = match (size, esize, msize) {
                (2, 5, 10) => Some(Num::F16),
                (4, 8, 23) => Some(Num::F32),
                (8, 11, 52) => Some(Num::F64),
                _ => None,
            };
            match num {
                Some(num) if !vax => Dtype::Num { num, be },
                _ => Dtype::Other { class, size },
            }
        }
        3 => Dtype::Str {
            size,
            pad: bits[0] & 0x0f,
        },
        7 => Dtype::Ref {
            size,
            kind: bits[0] & 0x0f,
        },
        8 => {
            // Enumeration: the base type follows; names and values after it are not needed.
            let (base, n, _) = parse_dtype_at(&b[r.pos..], depth + 1)?;
            r.skip(n)?;
            match base {
                Dtype::Num { .. } => base,
                _ => Dtype::Other { class, size },
            }
        }
        9 => {
            let (base, n, _) = parse_dtype_at(&b[r.pos..], depth + 1)?;
            r.skip(n)?;
            if bits[0] & 0x0f == 1 {
                Dtype::VlenStr
            } else {
                Dtype::VlenSeq(Box::new(base))
            }
        }
        10 => {
            let rank = r.u8()? as usize;
            if version < 3 {
                r.skip(3)?;
            }
            let mut dims = Vec::with_capacity(rank);
            for _ in 0..rank {
                dims.push(r.u32()?);
            }
            if version < 3 {
                r.skip(4 * rank)?;
            }
            let (base, n, _) = parse_dtype_at(&b[r.pos..], depth + 1)?;
            r.skip(n)?;
            Dtype::Array {
                dims,
                base: Box::new(base),
            }
        }
        // Compound: its members, each a name, a byte offset and a datatype.
        6 => {
            let n = usize::from(bits[0]) | (usize::from(bits[1]) << 8);
            let mut fields = Vec::new();
            for _ in 0..n {
                let start = r.pos;
                let mut name = Vec::new();
                loop {
                    let c = r.u8()?;
                    if c == 0 {
                        break;
                    }
                    if name.len() < 256 {
                        name.push(c);
                    }
                }
                if version < 3 {
                    // NUL-terminated and padded to a multiple of 8 bytes.
                    let used = r.pos - start;
                    r.skip((8 - used % 8) % 8)?;
                }
                let offset = if version >= 3 {
                    // As many bytes as the compound size needs.
                    let k = (1..8usize)
                        .find(|&k| (size as u64) < 1u64 << (8 * k))
                        .unwrap_or(8);
                    r.uint(k)? as usize
                } else {
                    r.u32()? as usize
                };
                let mut dims: Vec<u32> = Vec::new();
                if version == 1 {
                    let nd = r.u8()? as usize;
                    r.skip(3 + 4 + 4)?;
                    for k in 0..4 {
                        let d = r.u32()?;
                        if k < nd {
                            dims.push(d);
                        }
                    }
                }
                let (t, len, sub) = parse_dtype_at(&b[r.pos..], depth + 1)?;
                r.skip(len)?;
                let t = if dims.is_empty() {
                    t
                } else {
                    Dtype::Array {
                        dims,
                        base: Box::new(t),
                    }
                };
                let name = String::from_utf8_lossy(&name).into_owned();
                member_fields(&name, offset, &t, sub, &mut fields);
            }
            fields.retain(|f| f.offset.saturating_add(f.num.size()) <= size);
            return Ok((Dtype::Other { class, size }, r.pos, fields));
        }
        // Time (2), opaque (5).
        2 | 5 => {
            if class == 5 {
                // An ASCII tag padded to 8 bytes; its length is in the class bits.
                r.skip(bits[0] as usize)?;
            } else {
                r.skip(2)?;
            }
            Dtype::Other { class, size }
        }
        _ => return Err(Error::Malformed(format!("datatype class {class}"))),
    };
    // Validate fixed sizes against what the element really needs.
    if let Dtype::Num { num, .. } = &t
        && num.size() != size
    {
        return Err(Error::Malformed("datatype size".into()));
    }
    Ok((t, r.pos, Vec::new()))
}

/// A dataspace: its current and maximum dimensions (row-major, slowest first).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Dataspace {
    pub dims: Vec<u64>,
    /// `u64::MAX` for an unlimited dimension.
    pub max: Vec<u64>,
    /// A null dataspace holds no elements at all.
    pub null: bool,
}

impl Dataspace {
    /// Number of elements (1 for a scalar).
    pub fn count(&self) -> u64 {
        if self.null {
            return 0;
        }
        self.dims
            .iter()
            .try_fold(1u64, |a, &d| a.checked_mul(d))
            .unwrap_or(u64::MAX)
    }
}

pub(crate) fn parse_dataspace(b: &[u8], c: &Ctx) -> Result<Dataspace> {
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    let rank = r.u8()? as usize;
    let flags = r.u8()?;
    if rank > 32 {
        return Err(Error::Malformed(format!("dataspace of rank {rank}")));
    }
    let mut null = false;
    match version {
        1 => r.skip(5)?,
        2 => null = r.u8()? == 2,
        _ => return Err(Error::Unsupported(format!("dataspace version {version}"))),
    }
    let mut dims = Vec::with_capacity(rank);
    for _ in 0..rank {
        dims.push(r.len(c)?);
    }
    let mut max = dims.clone();
    if flags & 1 != 0 {
        for m in max.iter_mut() {
            let v = r.len(c)?;
            *m = if crate::io::is_undef(v, c.sl as usize) {
                u64::MAX
            } else {
                v
            };
        }
    }
    Ok(Dataspace { dims, max, null })
}

/// Output sample types that stored numbers convert to.
pub trait Sample: Copy + Send + Sync + Default + 'static {
    const NAN: Self;
    fn from_f64(v: f64) -> Self;
    fn from_f32(v: f32) -> Self;
    fn from_i64(v: i64) -> Self;
    fn from_u64(v: u64) -> Self;
}

impl Sample for f32 {
    const NAN: f32 = f32::NAN;
    fn from_f64(v: f64) -> f32 {
        v as f32
    }
    fn from_f32(v: f32) -> f32 {
        v
    }
    fn from_i64(v: i64) -> f32 {
        v as f32
    }
    fn from_u64(v: u64) -> f32 {
        v as f32
    }
}

impl Sample for f64 {
    const NAN: f64 = f64::NAN;
    fn from_f64(v: f64) -> f64 {
        v
    }
    fn from_f32(v: f32) -> f64 {
        f64::from(v)
    }
    fn from_i64(v: i64) -> f64 {
        v as f64
    }
    fn from_u64(v: u64) -> f64 {
        v as f64
    }
}

/// IEEE half to single precision.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = u32::from(h >> 15) << 31;
    let exp = u32::from((h >> 10) & 0x1f);
    let man = u32::from(h & 0x3ff);
    let bits = match exp {
        0 if man == 0 => sign,
        0 => {
            // Subnormal: normalise.
            let mut e = 127 - 15 + 1;
            let mut m = man;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | ((e as u32) << 23) | ((m & 0x3ff) << 13)
        }
        0x1f => sign | 0x7f80_0000 | (man << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (man << 13),
    };
    f32::from_bits(bits)
}

/// Converts `out.len()` elements of `src`, starting at byte 0 with `stride` bytes between
/// elements, to real values.
pub fn convert<T: Sample>(src: &[u8], num: Num, be: bool, stride: usize, out: &mut [T]) {
    macro_rules! go {
        ($n:expr, $f:expr) => {{
            for (i, o) in out.iter_mut().enumerate() {
                let p = i * stride;
                let mut a = [0u8; $n];
                a.copy_from_slice(&src[p..p + $n]);
                if be {
                    a.reverse();
                }
                *o = $f(a);
            }
        }};
    }
    match num {
        Num::U8 => {
            for (i, o) in out.iter_mut().enumerate() {
                *o = T::from_f32(f32::from(src[i * stride]));
            }
        }
        Num::I8 => {
            for (i, o) in out.iter_mut().enumerate() {
                *o = T::from_f32(f32::from(src[i * stride] as i8));
            }
        }
        Num::U16 => go!(2, |a| T::from_f32(f32::from(u16::from_le_bytes(a)))),
        Num::I16 => go!(2, |a| T::from_f32(f32::from(i16::from_le_bytes(a)))),
        Num::U32 => go!(4, |a| T::from_f64(f64::from(u32::from_le_bytes(a)))),
        Num::I32 => go!(4, |a| T::from_f64(f64::from(i32::from_le_bytes(a)))),
        Num::U64 => go!(8, |a| T::from_u64(u64::from_le_bytes(a))),
        Num::I64 => go!(8, |a| T::from_i64(i64::from_le_bytes(a))),
        Num::F16 => go!(2, |a| T::from_f32(f16_to_f32(u16::from_le_bytes(a)))),
        Num::F32 => go!(4, |a| T::from_f32(f32::from_le_bytes(a))),
        Num::F64 => go!(8, |a| T::from_f64(f64::from_le_bytes(a))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x0001), 5.960_464_5e-8);
        assert!(f16_to_f32(0x7e00).is_nan());
        assert_eq!(f16_to_f32(0x7c00), f32::INFINITY);
    }

    #[test]
    fn conversions() {
        let mut out = [0f32; 2];
        convert(&[0x01, 0x02, 0xff, 0xff], Num::I16, true, 2, &mut out);
        assert_eq!(out, [258.0, -1.0]);
        let mut o64 = [0f64; 1];
        convert(&1.5f64.to_le_bytes(), Num::F64, false, 8, &mut o64);
        assert_eq!(o64, [1.5]);
    }
}

//! casacore records (`TableRecord`, `Record`): the keyword sets of tables and columns, and
//! everything CASA keeps in them (coordinates, image info, masks).

use crate::aipsio::Reader;
use crate::{Error, Result};

/// Fields nested deeper than this are refused.
const MAX_DEPTH: usize = 24;
/// The most fields in one record.
const MAX_FIELDS: usize = 1 << 16;

/// casacore's data type numbers (`DataType`).
pub mod dtype {
    pub const BOOL: i32 = 0;
    pub const CHAR: i32 = 1;
    pub const UCHAR: i32 = 2;
    pub const SHORT: i32 = 3;
    pub const USHORT: i32 = 4;
    pub const INT: i32 = 5;
    pub const UINT: i32 = 6;
    pub const FLOAT: i32 = 7;
    pub const DOUBLE: i32 = 8;
    pub const COMPLEX: i32 = 9;
    pub const DCOMPLEX: i32 = 10;
    pub const STRING: i32 = 11;
    pub const TABLE: i32 = 12;
    pub const ARRAY_BOOL: i32 = 13;
    pub const ARRAY_STRING: i32 = 24;
    pub const RECORD: i32 = 25;
    pub const INT64: i32 = 29;
    pub const ARRAY_INT64: i32 = 30;

    /// The element type of an array type, `None` for scalars.
    pub fn element(t: i32) -> Option<i32> {
        match t {
            ARRAY_BOOL..=ARRAY_STRING => Some(t - ARRAY_BOOL),
            ARRAY_INT64 => Some(INT64),
            _ => None,
        }
    }

    /// The name casacore gives a type.
    pub fn name(t: i32) -> &'static str {
        match element(t).unwrap_or(t) {
            BOOL => "Bool",
            CHAR => "Char",
            UCHAR => "UChar",
            SHORT => "Short",
            USHORT => "UShort",
            INT => "Int",
            UINT => "UInt",
            FLOAT => "Float",
            DOUBLE => "Double",
            COMPLEX => "Complex",
            DCOMPLEX => "DComplex",
            STRING => "String",
            TABLE => "Table",
            RECORD => "Record",
            INT64 => "Int64",
            _ => "Other",
        }
    }
}

/// An array value: its shape (first axis fastest) and its elements.
#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub shape: Vec<usize>,
    pub data: ArrayData,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArrayData {
    Bool(Vec<bool>),
    Int(Vec<i64>),
    Float(Vec<f64>),
    Complex(Vec<(f64, f64)>),
    String(Vec<String>),
}

impl Array {
    pub fn len(&self) -> usize {
        match &self.data {
            ArrayData::Bool(v) => v.len(),
            ArrayData::Int(v) => v.len(),
            ArrayData::Float(v) => v.len(),
            ArrayData::Complex(v) => v.len(),
            ArrayData::String(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The elements as numbers (integers and floats; complex as their real parts).
    pub fn numbers(&self) -> Option<Vec<f64>> {
        match &self.data {
            ArrayData::Int(v) => Some(v.iter().map(|&x| x as f64).collect()),
            ArrayData::Float(v) => Some(v.clone()),
            ArrayData::Complex(v) => Some(v.iter().map(|c| c.0).collect()),
            ArrayData::Bool(_) | ArrayData::String(_) => None,
        }
    }

    pub fn strings(&self) -> Option<&[String]> {
        match &self.data {
            ArrayData::String(v) => Some(v),
            _ => None,
        }
    }
}

/// A field's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Complex(f64, f64),
    String(String),
    /// A subtable, by its name relative to the table (`././logtable`).
    Table(String),
    Record(Record),
    Array(Array),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            Value::Array(a) if a.len() == 1 => a.numbers()?.first().copied(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) | Value::Table(s) => Some(s),
            Value::Array(a) if a.len() == 1 => a.strings()?.first().map(String::as_str),
            _ => None,
        }
    }

    pub fn as_record(&self) -> Option<&Record> {
        match self {
            Value::Record(r) => Some(r),
            _ => None,
        }
    }

    /// The value as numbers: a numeric array, or a single number.
    pub fn numbers(&self) -> Option<Vec<f64>> {
        match self {
            Value::Array(a) => a.numbers(),
            v => v.as_f64().map(|x| vec![x]),
        }
    }

    pub fn strings(&self) -> Option<Vec<String>> {
        match self {
            Value::Array(a) => a.strings().map(<[String]>::to_vec),
            Value::String(s) => Some(vec![s.clone()]),
            _ => None,
        }
    }

    /// The value as short text for the metadata list.
    pub fn text(&self) -> String {
        fn num(x: f64) -> String {
            if x.fract() == 0.0 && x.abs() < 1e15 {
                format!("{}", x as i64)
            } else {
                format!("{x}")
            }
        }
        match self {
            Value::Bool(b) => if *b { "T" } else { "F" }.to_owned(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => num(*f),
            Value::Complex(r, i) => format!("({}, {})", num(*r), num(*i)),
            Value::String(s) | Value::Table(s) => s.trim_end_matches('\0').to_owned(),
            Value::Record(r) => format!("{{{} fields}}", r.fields.len()),
            Value::Array(a) => {
                const SHOWN: usize = 16;
                let items: Vec<String> = match &a.data {
                    ArrayData::Bool(v) => v
                        .iter()
                        .take(SHOWN)
                        .map(|b| if *b { "T" } else { "F" }.to_owned())
                        .collect(),
                    ArrayData::Int(v) => v.iter().take(SHOWN).map(i64::to_string).collect(),
                    ArrayData::Float(v) => v.iter().take(SHOWN).map(|&x| num(x)).collect(),
                    ArrayData::Complex(v) => v
                        .iter()
                        .take(SHOWN)
                        .map(|&(r, i)| format!("({}, {})", num(r), num(i)))
                        .collect(),
                    ArrayData::String(v) => v
                        .iter()
                        .take(SHOWN)
                        .map(|s| s.trim_end_matches('\0').to_owned())
                        .collect(),
                };
                let more = if a.len() > SHOWN { ", …" } else { "" };
                format!("[{}{more}]", items.join(", "))
            }
        }
    }
}

/// A record: named fields in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub fields: Vec<(String, Value)>,
}

impl Record {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn record(&self, name: &str) -> Option<&Record> {
        self.get(name)?.as_record()
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.get(name)?.as_str()
    }

    pub fn f64(&self, name: &str) -> Option<f64> {
        self.get(name)?.as_f64()
    }

    pub fn numbers(&self, name: &str) -> Option<Vec<f64>> {
        self.get(name)?.numbers()
    }

    pub fn strings(&self, name: &str) -> Option<Vec<String>> {
        self.get(name)?.strings()
    }

    /// Every field as `(dotted.path, text)`, sub-records flattened, at most `max`.
    pub fn flatten(&self, max: usize) -> Vec<(String, String)> {
        let mut out = Vec::new();
        self.flatten_into("", max, &mut out);
        out
    }

    fn flatten_into(&self, prefix: &str, max: usize, out: &mut Vec<(String, String)>) {
        for (k, v) in &self.fields {
            if out.len() >= max {
                return;
            }
            let key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            match v {
                Value::Record(r) => r.flatten_into(&key, max, out),
                v => out.push((key, v.text())),
            }
        }
    }
}

/// A field of a record description.
#[derive(Clone, Debug)]
struct FieldDesc {
    name: String,
    ty: i32,
}

fn read_desc(r: &mut Reader, depth: usize) -> Result<Vec<FieldDesc>> {
    if depth > MAX_DEPTH {
        return Err(Error::Malformed("records nested too deep".into()));
    }
    let (version, outer) = r.open(false, &["RecordDesc"])?;
    let n = r.i32()?;
    let n = usize::try_from(n)
        .ok()
        .filter(|&n| n <= MAX_FIELDS)
        .ok_or_else(|| Error::Malformed("bad field count".into()))?;
    r.check_count(n, 8)?;
    let mut fields = Vec::with_capacity(n);
    for _ in 0..n {
        let name = r.string()?;
        let ty = r.i32()?;
        if dtype::element(ty).is_some() {
            r.iposition()?;
        } else if ty == dtype::RECORD {
            read_desc(r, depth + 1)?;
        } else if ty == dtype::TABLE {
            r.string()?;
        }
        if version >= 2 {
            r.string()?;
        }
        fields.push(FieldDesc { name, ty });
    }
    r.close(outer);
    Ok(fields)
}

/// Reads a `TableRecord` or `Record` object.
pub fn read_record(r: &mut Reader) -> Result<Record> {
    read_record_at(r, 0)
}

fn read_record_at(r: &mut Reader, depth: usize) -> Result<Record> {
    if depth > MAX_DEPTH {
        return Err(Error::Malformed("records nested too deep".into()));
    }
    let (_, outer) = r.open(false, &["TableRecord", "Record"])?;
    let desc = read_desc(r, depth)?;
    let _record_type = r.i32()?;
    let mut fields = Vec::with_capacity(desc.len());
    for f in desc {
        let v = read_value(r, f.ty, depth)?;
        fields.push((f.name, v));
    }
    r.close(outer);
    Ok(Record { fields })
}

fn read_value(r: &mut Reader, ty: i32, depth: usize) -> Result<Value> {
    use dtype::*;
    Ok(match ty {
        BOOL => Value::Bool(r.bool()?),
        CHAR => Value::Int(i64::from(r.u8()? as i8)),
        UCHAR => Value::Int(i64::from(r.u8()?)),
        SHORT => Value::Int(i64::from(r.i16()?)),
        USHORT => Value::Int(i64::from(r.u16()?)),
        INT => Value::Int(i64::from(r.i32()?)),
        UINT => Value::Int(i64::from(r.u32()?)),
        INT64 => Value::Int(r.i64()?),
        FLOAT => Value::Float(f64::from(r.f32()?)),
        DOUBLE => Value::Float(r.f64()?),
        COMPLEX => Value::Complex(f64::from(r.f32()?), f64::from(r.f32()?)),
        DCOMPLEX => Value::Complex(r.f64()?, r.f64()?),
        STRING => Value::String(r.string()?),
        TABLE => Value::Table(r.string()?),
        RECORD => Value::Record(read_record_at(r, depth + 1)?),
        t => match dtype::element(t) {
            Some(e) => Value::Array(read_array(r, e)?),
            None => {
                return Err(Error::Unsupported(format!(
                    "record field of type {}",
                    dtype::name(t)
                )));
            }
        },
    })
}

/// Reads an `Array<T>` object of element type `e`.
pub fn read_array(r: &mut Reader, e: i32) -> Result<Array> {
    use dtype::*;
    let (_, outer) = r.open(false, &[])?;
    let ndim = r.count(4)?;
    let mut shape = Vec::with_capacity(ndim);
    for _ in 0..ndim {
        let n = r.i32()?;
        shape
            .push(usize::try_from(n).map_err(|_| Error::Malformed("negative array shape".into()))?);
    }
    let n = r.u32()? as usize;
    let product = shape
        .iter()
        .try_fold(1usize, |a, &b| a.checked_mul(b))
        .ok_or_else(|| Error::Malformed("array shape overflows".into()))?;
    if n != product {
        return Err(Error::Malformed(
            "array size disagrees with its shape".into(),
        ));
    }
    let min = match e {
        BOOL => 0,
        CHAR | UCHAR => 1,
        SHORT | USHORT => 2,
        INT | UINT | FLOAT | STRING => 4,
        INT64 | DOUBLE | COMPLEX => 8,
        DCOMPLEX => 16,
        _ => 1,
    };
    r.check_count(n, min)?;
    let data = match e {
        BOOL => ArrayData::Bool(r.bits(n)?),
        CHAR => ArrayData::Int(
            (0..n)
                .map(|_| r.u8().map(|x| i64::from(x as i8)))
                .collect::<Result<_>>()?,
        ),
        UCHAR => ArrayData::Int(
            (0..n)
                .map(|_| r.u8().map(i64::from))
                .collect::<Result<_>>()?,
        ),
        SHORT => ArrayData::Int(
            (0..n)
                .map(|_| r.i16().map(i64::from))
                .collect::<Result<_>>()?,
        ),
        USHORT => ArrayData::Int(
            (0..n)
                .map(|_| r.u16().map(i64::from))
                .collect::<Result<_>>()?,
        ),
        INT => ArrayData::Int(
            (0..n)
                .map(|_| r.i32().map(i64::from))
                .collect::<Result<_>>()?,
        ),
        UINT => ArrayData::Int(
            (0..n)
                .map(|_| r.u32().map(i64::from))
                .collect::<Result<_>>()?,
        ),
        INT64 => ArrayData::Int((0..n).map(|_| r.i64()).collect::<Result<_>>()?),
        FLOAT => ArrayData::Float(
            (0..n)
                .map(|_| r.f32().map(f64::from))
                .collect::<Result<_>>()?,
        ),
        DOUBLE => ArrayData::Float((0..n).map(|_| r.f64()).collect::<Result<_>>()?),
        COMPLEX => ArrayData::Complex(
            (0..n)
                .map(|_| Ok((f64::from(r.f32()?), f64::from(r.f32()?))))
                .collect::<Result<_>>()?,
        ),
        DCOMPLEX => ArrayData::Complex(
            (0..n)
                .map(|_| Ok((r.f64()?, r.f64()?)))
                .collect::<Result<_>>()?,
        ),
        STRING => ArrayData::String((0..n).map(|_| r.string()).collect::<Result<_>>()?),
        t => {
            return Err(Error::Unsupported(format!("array of {}", dtype::name(t))));
        }
    };
    r.close(outer);
    Ok(Array { shape, data })
}

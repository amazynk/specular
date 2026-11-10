//! Attributes, compact (header messages) and dense (fractal heap), and their values.

use crate::fheap::{Heap, objects_by_index};
use crate::io::{Ctx, Cur, le};
use crate::ohdr::{
    Header, MSG_ATTRIBUTE, MSG_ATTRIBUTE_INFO, MSG_DATASPACE, MSG_DATATYPE, SharedTargets,
};
use crate::types::{Dataspace, Dtype, Num, convert, parse_dataspace, parse_dtype};
use crate::{Error, Result, gheap, limits};

/// An attribute's decoded value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(Vec<i64>),
    UInt(Vec<u64>),
    Float(Vec<f64>),
    Str(Vec<String>),
    /// Object references (object header addresses); one list per element (a variable-length
    /// sequence of references gives several, a plain reference one).
    Refs(Vec<Vec<u64>>),
    /// A value that is not decoded: its type, described.
    Other(String),
}

impl Value {
    /// The numbers, when numeric.
    pub fn as_f64s(&self) -> Option<Vec<f64>> {
        match self {
            Value::Int(v) => Some(v.iter().map(|&x| x as f64).collect()),
            Value::UInt(v) => Some(v.iter().map(|&x| x as f64).collect()),
            Value::Float(v) => Some(v.clone()),
            _ => None,
        }
    }

    /// The first number, when numeric (a numeric string parses too).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Str(s) => s.first()?.trim().parse().ok(),
            _ => self.as_f64s()?.first().copied(),
        }
    }

    /// The first string, when a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => s.first().map(String::as_str),
            _ => None,
        }
    }

    /// The strings joined, when a string: Imaris writes text as an array of one-character
    /// strings.
    pub fn text(&self) -> Option<String> {
        match self {
            Value::Str(s) => Some(s.concat()),
            _ => None,
        }
    }

    /// A short display form: at most `max` elements, then an ellipsis.
    pub fn display(&self, max: usize) -> String {
        fn join<T: std::fmt::Display>(v: &[T], max: usize) -> String {
            let mut s: Vec<String> = v.iter().take(max).map(|x| x.to_string()).collect();
            if v.len() > max {
                s.push("…".into());
            }
            s.join(", ")
        }
        match self {
            Value::Int(v) => join(v, max),
            Value::UInt(v) => join(v, max),
            Value::Float(v) => join(v, max),
            Value::Str(v) => join(v, max),
            Value::Refs(v) => format!("{} references", v.iter().map(Vec::len).sum::<usize>()),
            Value::Other(d) => d.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attr {
    pub name: String,
    pub value: Value,
}

impl Attr {
    /// About how many bytes the attribute takes in memory.
    pub(crate) fn heap_bytes(&self) -> usize {
        let v = match &self.value {
            Value::Int(v) => v.len() * 8,
            Value::UInt(v) => v.len() * 8,
            Value::Float(v) => v.len() * 8,
            Value::Str(v) => v.iter().map(|s| s.len() + 24).sum(),
            Value::Refs(v) => v.iter().map(|r| r.len() * 8 + 24).sum(),
            Value::Other(s) => s.len(),
        };
        self.name.len() + 64 + v
    }
}

/// All attributes of an object, compact and dense, while their bytes stay within `budget`
/// (shared by every object of the file: shared and dense attributes can repeat one large
/// value on every object).
pub(crate) fn read_all<'a>(
    data: &'a [u8],
    c: &Ctx,
    h: &Header<'a>,
    shared: &mut SharedTargets<'a>,
    budget: &mut usize,
) -> Result<Vec<Attr>> {
    fn keep(out: &mut Vec<Attr>, a: Attr, budget: &mut usize) {
        let n = a.heap_bytes();
        if n > *budget {
            *budget = 0;
        } else {
            *budget -= n;
            out.push(a);
        }
    }
    let mut out = Vec::new();
    for m in h.all(MSG_ATTRIBUTE) {
        if out.len() >= limits::MAX_ATTRS || *budget == 0 {
            break;
        }
        match parse(data, c, m.data, shared) {
            Ok(a) => {
                keep(&mut out, a, budget);
            }
            Err(Error::Unsupported(_)) => {}
            Err(e) => return Err(e),
        }
    }
    if let Some(m) = h.find(MSG_ATTRIBUTE_INFO) {
        let mut r = Cur::new(m.data, 0);
        r.skip(1)?;
        let flags = r.u8()?;
        if flags & 1 != 0 {
            r.skip(2)?;
        }
        let heap = r.addr(c)?;
        let names = r.addr(c)?;
        if let (Some(heap), Some(names)) = (heap, names) {
            let heap = Heap::open(data, c, heap)?;
            for obj in objects_by_index(data, c, &heap, names, 0)? {
                if out.len() >= limits::MAX_ATTRS || *budget == 0 {
                    break;
                }
                match parse(data, c, obj, shared) {
                    Ok(a) => {
                        keep(&mut out, a, budget);
                    }
                    Err(Error::Unsupported(_)) => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(out)
}

fn pad8(n: usize) -> usize {
    n.div_ceil(8) * 8
}

/// Decodes one attribute message.
pub(crate) fn parse<'a>(
    data: &'a [u8],
    c: &Ctx,
    b: &'a [u8],
    shared: &mut SharedTargets<'a>,
) -> Result<Attr> {
    let mut r = Cur::new(b, 0);
    let version = r.u8()?;
    let flags = r.u8()?;
    let name_len = r.u16()? as usize;
    let dt_len = r.u16()? as usize;
    let ds_len = r.u16()? as usize;
    let (name_len_p, dt_len_p, ds_len_p) = match version {
        1 => (pad8(name_len), pad8(dt_len), pad8(ds_len)),
        2 => (name_len, dt_len, ds_len),
        3 => {
            r.skip(1)?;
            (name_len, dt_len, ds_len)
        }
        _ => return Err(Error::Malformed(format!("attribute version {version}"))),
    };
    let name_raw = r.bytes(name_len_p)?;
    let name_raw = &name_raw[..name_len.min(name_raw.len())];
    let end = name_raw
        .iter()
        .position(|&x| x == 0)
        .unwrap_or(name_raw.len());
    let name = String::from_utf8_lossy(&name_raw[..end]).into_owned();
    let dt_raw = &r.bytes(dt_len_p)?[..dt_len];
    let ds_raw = &r.bytes(ds_len_p)?[..ds_len];
    let rest = &b[r.pos..];
    let dtype = if version >= 2 && flags & 1 != 0 {
        let m = shared
            .message(data, c, dt_raw, 0, MSG_DATATYPE)?
            .ok_or_else(|| Error::Malformed("committed datatype".into()))?;
        parse_dtype(m)?.0
    } else {
        parse_dtype(dt_raw)?.0
    };
    let ds_raw = if version >= 2 && flags & 2 != 0 {
        shared
            .message(data, c, ds_raw, 0, MSG_DATASPACE)?
            .ok_or_else(|| Error::Malformed("shared dataspace".into()))?
    } else {
        ds_raw
    };
    let space = parse_dataspace(ds_raw, c)?;
    let value = decode_value(data, c, &dtype, &space, rest);
    Ok(Attr { name, value })
}

fn decode_value(data: &[u8], c: &Ctx, t: &Dtype, space: &Dataspace, raw: &[u8]) -> Value {
    let n = space.count();
    if n > limits::MAX_ATTR_ELEMS {
        return Value::Other(format!("{} × {}", n, t.describe()));
    }
    let n = n as usize;
    let size = t.size(c);
    let Some(raw) = raw.get(..n.saturating_mul(size)) else {
        return Value::Other("truncated".into());
    };
    values(data, c, t, raw, n).unwrap_or_else(|_| Value::Other(t.describe()))
}

/// Decodes `n` packed elements of type `t`.
pub(crate) fn values(data: &[u8], c: &Ctx, t: &Dtype, raw: &[u8], n: usize) -> Result<Value> {
    let size = t.size(c);
    Ok(match t {
        Dtype::Num { num, be } => {
            if num.is_float() {
                let mut v = vec![0f64; n.min(raw.len() / size)];
                convert(raw, *num, *be, size, &mut v);
                Value::Float(v)
            } else {
                let signed = matches!(num, Num::I8 | Num::I16 | Num::I32 | Num::I64);
                let ints = raw.chunks_exact(size).map(|e| {
                    let mut a = [0u8; 8];
                    a[..size].copy_from_slice(e);
                    if *be {
                        a[..size].reverse();
                    }
                    u64::from_le_bytes(a)
                });
                if signed {
                    let shift = 64 - 8 * size as u32;
                    Value::Int(ints.map(|u| ((u << shift) as i64) >> shift).collect())
                } else {
                    Value::UInt(ints.collect())
                }
            }
        }
        Dtype::Str { .. } => Value::Str(raw.chunks_exact(size.max(1)).map(text).collect()),
        Dtype::VlenStr => {
            let mut budget = Budget(data.len());
            let mut heaps = gheap::Heaps::default();
            Value::Str(
                raw.chunks_exact(size)
                    .map(|e| budget.take(heaps.vlen(data, c, e)?).map(text))
                    .collect::<Result<_>>()?,
            )
        }
        Dtype::VlenSeq(base) => match base.as_ref() {
            Dtype::Ref { size: rs, kind: 0 } => {
                let mut budget = Budget(data.len());
                let mut heaps = gheap::Heaps::default();
                Value::Refs(
                    raw.chunks_exact(size)
                        .map(|e| {
                            budget
                                .take(heaps.vlen(data, c, e)?)
                                .map(|o| o.chunks_exact((*rs).max(1)).map(le).collect())
                        })
                        .collect::<Result<_>>()?,
                )
            }
            _ => Value::Other(t.describe()),
        },
        Dtype::Ref { size, kind: 0 } => Value::Refs(
            raw.chunks_exact((*size).max(1))
                .map(|e| vec![le(&e[..e.len().min(c.so as usize)])])
                .collect(),
        ),
        Dtype::Array { dims, base } => {
            let Some(per) = dims
                .iter()
                .try_fold(1usize, |a, &d| a.checked_mul(d as usize))
            else {
                return Ok(Value::Other(t.describe()));
            };
            match values(data, c, base, raw, n.saturating_mul(per))? {
                v @ (Value::Int(_) | Value::UInt(_) | Value::Float(_) | Value::Str(_)) => v,
                _ => Value::Other(t.describe()),
            }
        }
        _ => Value::Other(t.describe()),
    })
}

/// Bytes left for variable-length values: elements may all point at one heap object, so
/// without it 65536 elements could copy a large object 65536 times. Real values are stored
/// once each, within the file.
struct Budget(usize);

impl Budget {
    fn take<'a>(&mut self, b: &'a [u8]) -> Result<&'a [u8]> {
        self.0 = self
            .0
            .checked_sub(b.len())
            .ok_or_else(|| Error::Malformed("variable-length values exceed the file".into()))?;
        Ok(b)
    }
}

/// A stored string: up to its first NUL, trailing spaces dropped.
fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim_end().to_string()
}

#[cfg(test)]
mod hostile {
    use super::*;

    const CTX: Ctx = Ctx {
        so: 8,
        sl: 8,
        base: 0,
    };

    #[test]
    fn array_with_overflowing_dims() {
        let t = Dtype::Array {
            dims: vec![u32::MAX; 4],
            base: Box::new(Dtype::Num {
                num: Num::F64,
                be: false,
            }),
        };
        assert!(values(&[], &CTX, &t, &[], 0).is_ok());
    }

    #[test]
    fn vlen_strings_sharing_one_large_object() {
        // A global heap with one 100-byte object, and two strings both pointing at it: more
        // text than the file holds.
        let mut d = b"GCOL\x01\0\0\0".to_vec();
        d.extend_from_slice(&(32u64 + 100).to_le_bytes());
        d.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0, 0]);
        d.extend_from_slice(&100u64.to_le_bytes());
        d.extend_from_slice(&[b'a'; 100]);
        let mut elem = 100u32.to_le_bytes().to_vec();
        elem.extend_from_slice(&0u64.to_le_bytes());
        elem.extend_from_slice(&1u32.to_le_bytes());
        assert!(values(&d, &CTX, &Dtype::VlenStr, &elem, 1).is_ok());
        let two = [elem.clone(), elem].concat();
        assert!(values(&d, &CTX, &Dtype::VlenStr, &two, 2).is_err());
    }

    /// 65,536 variable-length strings all naming the last of 65,535 objects in one global
    /// heap collection: each element walked the collection from its start (4·10^9 object
    /// steps); the collection is now indexed once.
    #[test]
    fn many_vlen_strings_in_one_large_collection() {
        let objs = 65_535u32;
        let mut d = b"GCOL\x01\0\0\0".to_vec();
        d.extend_from_slice(&(16u64 + 16 * u64::from(objs) + 16).to_le_bytes());
        for i in 1..=objs {
            d.extend_from_slice(&(i as u16).to_le_bytes());
            d.extend_from_slice(&[0; 6]);
            d.extend_from_slice(&0u64.to_le_bytes());
        }
        d.extend_from_slice(&[0; 16]);
        let mut elem = 1u32.to_le_bytes().to_vec();
        elem.extend_from_slice(&0u64.to_le_bytes());
        elem.extend_from_slice(&objs.to_le_bytes());
        let n = 1 << 16;
        let raw = elem.repeat(n);
        let t = std::time::Instant::now();
        let v = values(&d, &CTX, &Dtype::VlenStr, &raw, n).unwrap();
        assert!(
            t.elapsed() < std::time::Duration::from_secs(5),
            "{:?}",
            t.elapsed()
        );
        assert_eq!(v, Value::Str(vec![String::new(); n]));
        // A missing object still fails, with the same error as the single lookup.
        let mut miss = 1u32.to_le_bytes().to_vec();
        miss.extend_from_slice(&0u64.to_le_bytes());
        miss.extend_from_slice(&(objs + 1).to_le_bytes());
        let e = values(&d, &CTX, &Dtype::VlenStr, &miss, 1).unwrap_err();
        assert_eq!(
            e.to_string(),
            gheap::vlen(&d, &CTX, &miss).unwrap_err().to_string()
        );
    }
}

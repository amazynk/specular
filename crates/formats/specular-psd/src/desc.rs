//! Photoshop action descriptors (the "Descriptor structure" of the file format notes): the
//! settings of fill layers (`SoCo`, `GdFl`, `PtFl`) are stored as one. Only what the composite
//! reads is kept; references, aliases and raw data are skipped, object arrays stop the parse.

use crate::parse::Rd;

/// Nesting deeper than this is refused (a hostile file could recurse without end).
const MAX_DEPTH: usize = 24;
/// Items or list entries per level, at most.
const MAX_ITEMS: u32 = 1 << 16;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Num(f64),
    /// A number with its unit (`#Prc`, `#Ang`, `#Pxl`, ...).
    Unit([u8; 4], f64),
    Int(i64),
    Bool(bool),
    Text(String),
    /// Enumeration type and value.
    Enum(Vec<u8>, Vec<u8>),
    Obj(Desc),
    List(Vec<Value>),
    Other,
}

impl Value {
    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(v) | Value::Unit(_, v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Desc {
    pub class: Vec<u8>,
    pub items: Vec<(Vec<u8>, Value)>,
}

impl Desc {
    pub fn get(&self, key: &[u8]) -> Option<&Value> {
        self.items.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn num(&self, key: &[u8]) -> Option<f64> {
        self.get(key).and_then(Value::num)
    }

    pub fn obj(&self, key: &[u8]) -> Option<&Desc> {
        match self.get(key)? {
            Value::Obj(d) => Some(d),
            _ => None,
        }
    }

    pub fn list(&self, key: &[u8]) -> &[Value] {
        match self.get(key) {
            Some(Value::List(l)) => l,
            _ => &[],
        }
    }

    pub fn enum_value(&self, key: &[u8]) -> Option<&[u8]> {
        match self.get(key)? {
            Value::Enum(_, v) => Some(v),
            _ => None,
        }
    }

    pub fn boolean(&self, key: &[u8]) -> Option<bool> {
        match self.get(key)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn text(&self, key: &[u8]) -> Option<&str> {
        match self.get(key)? {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }
}

fn unicode(r: &mut Rd) -> Option<String> {
    let n = r.u32()? as usize;
    let raw = r.bytes(n.checked_mul(2)?)?;
    let units = raw
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]));
    let s: String = char::decode_utf16(units)
        .map(|c| c.unwrap_or('\u{FFFD}'))
        .collect();
    Some(s.trim_end_matches('\0').to_string())
}

/// A class or key ID: a length, then that many bytes, or four when the length is 0.
fn id(r: &mut Rd) -> Option<Vec<u8>> {
    let n = r.u32()? as usize;
    Some(r.bytes(if n == 0 { 4 } else { n })?.to_vec())
}

/// A descriptor at `r` (its unicode name first).
pub(crate) fn descriptor(r: &mut Rd) -> Option<Desc> {
    body(r, 0)
}

fn body(r: &mut Rd, depth: usize) -> Option<Desc> {
    if depth > MAX_DEPTH {
        return None;
    }
    let _name = unicode(r)?;
    let class = id(r)?;
    let n = r.u32()?.min(MAX_ITEMS);
    let mut items = Vec::new();
    for _ in 0..n {
        let key = id(r)?;
        let ty: [u8; 4] = r.bytes(4)?.try_into().ok()?;
        let v = value(r, &ty, depth)?;
        items.push((key, v));
    }
    Some(Desc { class, items })
}

fn value(r: &mut Rd, ty: &[u8; 4], depth: usize) -> Option<Value> {
    Some(match ty {
        b"doub" => Value::Num(f64::from_bits(r.u64()?)),
        b"UntF" => {
            let unit: [u8; 4] = r.bytes(4)?.try_into().ok()?;
            Value::Unit(unit, f64::from_bits(r.u64()?))
        }
        b"UnFl" => {
            let _unit = r.bytes(4)?;
            let n = r.u32()?;
            r.skip(u64::from(n) * 8)?;
            Value::Other
        }
        b"long" => Value::Int(i64::from(r.i32()?)),
        b"comp" => Value::Int(r.u64()? as i64),
        b"bool" => Value::Bool(r.u8()? != 0),
        b"TEXT" => Value::Text(unicode(r)?),
        b"enum" => Value::Enum(id(r)?, id(r)?),
        b"Objc" | b"GlbO" => Value::Obj(body(r, depth + 1)?),
        b"VlLs" => {
            let n = r.u32()?.min(MAX_ITEMS);
            let mut l = Vec::new();
            for _ in 0..n {
                let t: [u8; 4] = r.bytes(4)?.try_into().ok()?;
                l.push(value(r, &t, depth + 1)?);
            }
            Value::List(l)
        }
        b"type" | b"GlbC" => {
            unicode(r)?;
            id(r)?;
            Value::Other
        }
        b"alis" | b"tdta" | b"Pth " => {
            let n = r.u32()?;
            r.skip(u64::from(n))?;
            Value::Other
        }
        b"obj " => {
            let n = r.u32()?.min(MAX_ITEMS);
            for _ in 0..n {
                let t: [u8; 4] = r.bytes(4)?.try_into().ok()?;
                match &t {
                    b"prop" => {
                        unicode(r)?;
                        id(r)?;
                        id(r)?;
                    }
                    b"Clss" => {
                        unicode(r)?;
                        id(r)?;
                    }
                    b"Enmr" => {
                        unicode(r)?;
                        id(r)?;
                        id(r)?;
                        id(r)?;
                    }
                    b"rele" => {
                        unicode(r)?;
                        id(r)?;
                        r.u32()?;
                    }
                    b"Idnt" | b"indx" => {
                        r.u32()?;
                    }
                    b"name" => {
                        unicode(r)?;
                        id(r)?;
                        unicode(r)?;
                    }
                    _ => return None,
                }
            }
            Value::Other
        }
        _ => return None,
    })
}

/// A block holding a version word (16) and a descriptor, as `SoCo`, `GdFl` and `PtFl` do.
pub(crate) fn versioned(b: &[u8]) -> Option<Desc> {
    let mut r = Rd::new(b, 0);
    if r.u32()? != 16 {
        return None;
    }
    descriptor(&mut r)
}

/// A block holding two version words (0, then 16) and a descriptor, as the layer effects
/// (`lfx2`, `lmfx`) do.
pub(crate) fn versioned2(b: &[u8]) -> Option<Desc> {
    let mut r = Rd::new(b, 0);
    r.u32()?;
    if r.u32()? != 16 {
        return None;
    }
    descriptor(&mut r)
}

impl Desc {
    /// A number with its unit, `default` when absent.
    pub fn num_or(&self, key: &[u8], default: f64) -> f64 {
        self.num(key).unwrap_or(default)
    }
}

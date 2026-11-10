//! The .NET Remoting Binary Format (MS-NRBF) record grammar, read as data.
//!
//! Paint.NET saves its document with .NET's `BinaryFormatter`. This module reads the records
//! of such a stream into an object table: classes with their member values, strings and
//! arrays, keyed by object id. Nothing is instantiated or executed; type names are only
//! strings. Forward references (a member naming an object that comes later) are kept as ids
//! and resolved by the caller.
//!
//! Bounds: every read is checked against the stream; counts are checked against the bytes
//! left before anything is allocated for them, nesting is limited to [`MAX_DEPTH`], and the
//! total number of values to [`MAX_VALUES`].

use crate::Error;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

/// Inline objects nested in member lists, at most.
const MAX_DEPTH: usize = 64;
/// Member values and array elements in one stream, at most (a document has a few thousand).
const MAX_VALUES: usize = 1 << 22;

/// A member value or array element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Val {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    /// An object of the table (also the inline objects, strings and arrays).
    Ref(i32),
    /// A primitive string or char member, by index into [`Stream::strings`].
    Str(u32),
}

/// A class's layout, shared by every object that uses it (`ClassWithId`).
#[derive(Debug)]
pub(crate) struct Class {
    pub(crate) name: String,
    pub(crate) members: Vec<String>,
    /// Per member: the binary type and, for primitives, the primitive type.
    types: Vec<(u8, u8)>,
}

#[derive(Debug)]
pub(crate) enum Obj {
    Class {
        class: Arc<Class>,
        values: Vec<Val>,
    },
    Str(String),
    Array(Vec<Val>),
    /// A byte array: its range in the stream.
    Bytes(Range<usize>),
}

impl Obj {
    /// The member named `name`, or failing that the first one ending in `+name` (base class
    /// members are named `Base+member`).
    pub(crate) fn member(&self, name: &str) -> Option<Val> {
        let Obj::Class { class, values } = self else {
            return None;
        };
        let i = class.members.iter().position(|m| m == name).or_else(|| {
            class
                .members
                .iter()
                .position(|m| m.rsplit('+').next() == Some(name))
        })?;
        values.get(i).copied()
    }

    pub(crate) fn class_name(&self) -> Option<&str> {
        match self {
            Obj::Class { class, .. } => Some(&class.name),
            _ => None,
        }
    }
}

/// A parsed stream.
pub(crate) struct Stream {
    pub(crate) root: i32,
    pub(crate) objects: HashMap<i32, Obj>,
    pub(crate) strings: Vec<String>,
    /// Object ids of classes in the order their records appear (MemoryBlocks' deferred data
    /// follows the stream in this order).
    pub(crate) order: Vec<i32>,
    /// Where the stream's MessageEnd record ends.
    pub(crate) end: usize,
}

impl Stream {
    pub(crate) fn get(&self, v: Val) -> Option<&Obj> {
        match v {
            Val::Ref(id) => self.objects.get(&id),
            _ => None,
        }
    }

    /// A string value: a string object or a primitive string member.
    pub(crate) fn string(&self, v: Val) -> Option<&str> {
        match v {
            Val::Str(i) => self.strings.get(i as usize).map(String::as_str),
            Val::Ref(_) => match self.get(v)? {
                Obj::Str(s) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }
}

pub(crate) fn int(v: Option<Val>) -> Option<i64> {
    match v? {
        Val::Int(i) => Some(i),
        Val::UInt(u) => i64::try_from(u).ok(),
        _ => None,
    }
}

pub(crate) fn boolean(v: Option<Val>) -> Option<bool> {
    match v? {
        Val::Bool(b) => Some(b),
        Val::Int(i) => Some(i != 0),
        _ => None,
    }
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

fn cut(what: &str) -> Error {
    Error::Truncated(format!("the document's data ends inside {what}"))
}

fn bad(what: &str) -> Error {
    Error::Malformed(format!("the document's data is damaged ({what})"))
}

impl Reader<'_> {
    fn left(&self) -> usize {
        self.d.len() - self.p
    }
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        if n > self.left() {
            return Err(cut("a record"));
        }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }
    /// A count, at most what `each` bytes per item leave room for.
    fn count(&mut self, each: usize) -> Result<usize, Error> {
        let n = self.i32()?;
        let n = usize::try_from(n).map_err(|_| bad("negative count"))?;
        if n.saturating_mul(each.max(1)) > self.left() {
            return Err(cut("a list"));
        }
        Ok(n)
    }
    /// A length-prefixed UTF-8 string (7-bit encoded length).
    fn string(&mut self) -> Result<String, Error> {
        let mut n = 0usize;
        for i in 0..5 {
            let c = self.u8()?;
            n |= usize::from(c & 0x7f) << (7 * i);
            if c & 0x80 == 0 {
                let b = self.take(n)?;
                return Ok(String::from_utf8_lossy(b).into_owned());
            }
        }
        Err(bad("string length"))
    }
}

/// Bytes of a primitive of type `t` (0 for variable-length ones).
fn prim_size(t: u8) -> Option<usize> {
    Some(match t {
        1 | 2 | 10 => 1,
        7 | 14 => 2,
        8 | 11 | 15 => 4,
        6 | 9 | 12 | 13 | 16 => 8,
        3 | 5 | 18 => 0,
        _ => return None,
    })
}

struct Parser<'a> {
    r: Reader<'a>,
    classes: HashMap<i32, Arc<Class>>,
    objects: HashMap<i32, Obj>,
    strings: Vec<String>,
    order: Vec<i32>,
    values: usize,
}

/// What a record in a value position gave.
enum Item {
    Val(Val),
    /// `n` nulls (ObjectNullMultiple), in arrays.
    Nulls(usize),
    End,
}

impl Parser<'_> {
    fn count_values(&mut self, n: usize) -> Result<(), Error> {
        self.values = self.values.saturating_add(n);
        if self.values > MAX_VALUES {
            return Err(Error::Unsupported(
                "a Paint.NET document with an unusually large object graph".into(),
            ));
        }
        Ok(())
    }

    fn prim(&mut self, t: u8) -> Result<Val, Error> {
        let b = |r: &mut Reader, n: usize| -> Result<[u8; 8], Error> {
            let mut a = [0u8; 8];
            a[..n].copy_from_slice(r.take(n)?);
            Ok(a)
        };
        let r = &mut self.r;
        Ok(match t {
            1 => Val::Bool(r.u8()? != 0),
            2 => Val::Int(i64::from(r.u8()?)),
            10 => Val::Int(i64::from(r.u8()? as i8)),
            7 => Val::Int(i64::from(i16::from_le_bytes(
                b(r, 2)?[..2].try_into().unwrap_or_default(),
            ))),
            14 => Val::Int(i64::from(u16::from_le_bytes(
                b(r, 2)?[..2].try_into().unwrap_or_default(),
            ))),
            8 => Val::Int(i64::from(r.i32()?)),
            15 => Val::Int(i64::from(u32::from_le_bytes(
                b(r, 4)?[..4].try_into().unwrap_or_default(),
            ))),
            9 | 12 | 13 => Val::Int(i64::from_le_bytes(b(r, 8)?)),
            16 => Val::UInt(u64::from_le_bytes(b(r, 8)?)),
            11 => Val::Float(f64::from(f32::from_le_bytes(
                b(r, 4)?[..4].try_into().unwrap_or_default(),
            ))),
            6 => Val::Float(f64::from_le_bytes(b(r, 8)?)),
            3 => {
                let c = r.u8()?;
                let n = match c {
                    0..0x80 => 1,
                    0xc0..0xe0 => 2,
                    0xe0..0xf0 => 3,
                    _ => 4,
                };
                let mut s = vec![c];
                s.extend_from_slice(r.take(n - 1)?);
                self.strings.push(String::from_utf8_lossy(&s).into_owned());
                Val::Str((self.strings.len() - 1) as u32)
            }
            5 | 18 => {
                let s = r.string()?;
                self.strings.push(s);
                Val::Str((self.strings.len() - 1) as u32)
            }
            17 => Val::Null,
            _ => return Err(bad("primitive type")),
        })
    }

    fn class_info(&mut self) -> Result<(i32, String, Vec<String>), Error> {
        let id = self.r.i32()?;
        let name = self.r.string()?;
        let n = self.r.count(1)?;
        let mut members = Vec::with_capacity(n);
        for _ in 0..n {
            members.push(self.r.string()?);
        }
        Ok((id, name, members))
    }

    /// MemberTypeInfo for `n` members.
    fn member_types(&mut self, n: usize) -> Result<Vec<(u8, u8)>, Error> {
        let kinds = self.r.take(n)?.to_vec();
        let mut types = Vec::with_capacity(n);
        for k in kinds {
            let extra = match k {
                0 | 7 => self.r.u8()?,
                3 => {
                    self.r.string()?;
                    0
                }
                4 => {
                    self.r.string()?;
                    self.r.i32()?;
                    0
                }
                1 | 2 | 5 | 6 => 0,
                _ => return Err(bad("member type")),
            };
            types.push((k, extra));
        }
        Ok(types)
    }

    fn members(&mut self, class: &Arc<Class>, depth: usize) -> Result<Vec<Val>, Error> {
        self.count_values(class.types.len())?;
        let mut values = Vec::with_capacity(class.types.len());
        for &(kind, prim) in &class.types {
            values.push(if kind == 0 {
                self.prim(prim)?
            } else {
                match self.item(depth + 1)? {
                    Item::Val(v) => v,
                    // A null run in a member list covers this member only.
                    Item::Nulls(_) => Val::Null,
                    Item::End => return Err(bad("stream end inside an object")),
                }
            });
        }
        Ok(values)
    }

    fn insert(&mut self, id: i32, o: Obj) -> Val {
        if matches!(o, Obj::Class { .. }) {
            self.order.push(id);
        }
        self.objects.insert(id, o);
        Val::Ref(id)
    }

    fn elements(&mut self, n: usize, depth: usize) -> Result<Vec<Val>, Error> {
        self.count_values(n)?;
        let mut out = Vec::with_capacity(n.min(self.r.left()));
        while out.len() < n {
            match self.item(depth + 1)? {
                Item::Val(v) => out.push(v),
                Item::Nulls(k) => {
                    let k = k.min(n - out.len());
                    out.extend(std::iter::repeat_n(Val::Null, k));
                }
                Item::End => return Err(bad("stream end inside an array")),
            }
        }
        Ok(out)
    }

    fn prim_array(&mut self, id: i32, n: usize, t: u8) -> Result<Val, Error> {
        let size = prim_size(t).ok_or_else(|| bad("array element type"))?;
        if t == 2 {
            let start = self.r.p;
            self.r.take(n)?;
            return Ok(self.insert(id, Obj::Bytes(start..start + n)));
        }
        if n.saturating_mul(size.max(1)) > self.r.left() {
            return Err(cut("an array"));
        }
        self.count_values(n)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.prim(t)?);
        }
        Ok(self.insert(id, Obj::Array(v)))
    }

    /// One record in a value position (or at the top level).
    fn item(&mut self, depth: usize) -> Result<Item, Error> {
        if depth > MAX_DEPTH {
            return Err(Error::Unsupported(
                "a Paint.NET document nested too deeply".into(),
            ));
        }
        // BinaryLibrary records (12) only name an assembly before the record they precede:
        // skipped in a loop, as a hostile file may chain any number of them.
        let t = loop {
            match self.r.u8()? {
                12 => {
                    self.r.i32()?;
                    self.r.string()?;
                }
                t => break t,
            }
        };
        Ok(Item::Val(match t {
            0 => {
                // SerializedStreamHeader in a value position: only valid first.
                return Err(bad("second stream header"));
            }
            1 => {
                let id = self.r.i32()?;
                let meta = self.r.i32()?;
                let class = self
                    .classes
                    .get(&meta)
                    .cloned()
                    .ok_or_else(|| bad("reference to an unknown class"))?;
                let values = self.members(&class, depth)?;
                self.classes.insert(id, class.clone());
                self.insert(id, Obj::Class { class, values })
            }
            2 | 3 => {
                return Err(Error::Unsupported(
                    "a Paint.NET document saved without member types".into(),
                ));
            }
            4 | 5 => {
                let (id, name, members) = self.class_info()?;
                let types = self.member_types(members.len())?;
                if t == 5 {
                    self.r.i32()?; // library id
                }
                let class = Arc::new(Class {
                    name,
                    members,
                    types,
                });
                self.classes.insert(id, class.clone());
                let values = self.members(&class, depth)?;
                self.insert(id, Obj::Class { class, values })
            }
            6 => {
                let id = self.r.i32()?;
                let s = self.r.string()?;
                self.insert(id, Obj::Str(s))
            }
            7 => {
                let id = self.r.i32()?;
                let shape = self.r.u8()?;
                let rank = self.r.count(4)?;
                let mut n = 1usize;
                for _ in 0..rank {
                    let l = usize::try_from(self.r.i32()?).map_err(|_| bad("array length"))?;
                    n = n.checked_mul(l).ok_or_else(|| bad("array size"))?;
                }
                if matches!(shape, 3..=5) {
                    self.r.take(4 * rank)?; // lower bounds
                }
                let et = self.r.u8()?;
                let extra = match et {
                    0 | 7 => self.r.u8()?,
                    3 => {
                        self.r.string()?;
                        0
                    }
                    4 => {
                        self.r.string()?;
                        self.r.i32()?;
                        0
                    }
                    1 | 2 | 5 | 6 => 0,
                    _ => return Err(bad("array element type")),
                };
                if et == 0 {
                    return Ok(Item::Val(self.prim_array(id, n, extra)?));
                }
                if n > MAX_VALUES {
                    return Err(bad("array size"));
                }
                let v = self.elements(n, depth)?;
                self.insert(id, Obj::Array(v))
            }
            8 => {
                let pt = self.r.u8()?;
                self.prim(pt)?
            }
            9 => Val::Ref(self.r.i32()?),
            10 => Val::Null,
            11 => return Ok(Item::End),
            13 => return Ok(Item::Nulls(usize::from(self.r.u8()?))),
            14 => {
                let n = usize::try_from(self.r.i32()?).map_err(|_| bad("null count"))?;
                return Ok(Item::Nulls(n));
            }
            15 => {
                let id = self.r.i32()?;
                let n = usize::try_from(self.r.i32()?).map_err(|_| bad("array length"))?;
                let pt = self.r.u8()?;
                self.prim_array(id, n, pt)?
            }
            16 | 17 => {
                let id = self.r.i32()?;
                // Not bounded by the bytes left: null runs cover many elements in a few bytes.
                let n = usize::try_from(self.r.i32()?).map_err(|_| bad("array length"))?;
                if n > MAX_VALUES {
                    return Err(bad("array size"));
                }
                let v = self.elements(n, depth)?;
                self.insert(id, Obj::Array(v))
            }
            21 | 22 => {
                return Err(Error::Unsupported(
                    "a remoting message, not a Paint.NET document".into(),
                ));
            }
            _ => return Err(bad("unknown record type")),
        }))
    }
}

/// Parses the stream starting at `data[0]` up to its MessageEnd record.
pub(crate) fn parse(data: &[u8]) -> Result<Stream, Error> {
    let mut p = Parser {
        r: Reader { d: data, p: 0 },
        classes: HashMap::new(),
        objects: HashMap::new(),
        strings: Vec::new(),
        order: Vec::new(),
        values: 0,
    };
    if p.r.u8()? != 0 {
        return Err(bad("no stream header"));
    }
    let root = p.r.i32()?;
    p.r.take(12)?; // header id, major and minor version
    loop {
        match p.item(0)? {
            Item::End => break,
            Item::Val(_) | Item::Nulls(_) => {}
        }
    }
    Ok(Stream {
        root,
        objects: p.objects,
        strings: p.strings,
        order: p.order,
        end: p.r.p,
    })
}

#[cfg(test)]
pub(crate) mod write {
    //! A small NRBF writer for synthetic test documents.

    pub(crate) fn string(out: &mut Vec<u8>, s: &str) {
        let mut n = s.len();
        loop {
            let b = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        }
        out.extend_from_slice(s.as_bytes());
    }

    /// A member: its name and how its value is written.
    pub(crate) enum M<'a> {
        Int(&'a str, i32),
        Byte(&'a str, u8),
        Bool(&'a str, bool),
        Long(&'a str, i64),
        /// A reference to object `id`.
        Ref(&'a str, i32),
        /// A string object `id` with this text, inline.
        Str(&'a str, i32, &'a str),
        Null(&'a str),
    }

    /// ClassWithMembersAndTypes (record 5) with library 2.
    pub(crate) fn class_record(out: &mut Vec<u8>, id: i32, name: &str, members: &[M]) {
        out.push(5);
        out.extend_from_slice(&id.to_le_bytes());
        string(out, name);
        out.extend_from_slice(&(members.len() as i32).to_le_bytes());
        let mname = |m: &M| match *m {
            M::Int(n, _)
            | M::Byte(n, _)
            | M::Bool(n, _)
            | M::Long(n, _)
            | M::Ref(n, _)
            | M::Str(n, _, _)
            | M::Null(n) => n.to_string(),
        };
        for m in members {
            string(out, &mname(m));
        }
        for m in members {
            out.push(match m {
                M::Int(..) | M::Byte(..) | M::Bool(..) | M::Long(..) => 0,
                M::Str(..) => 1,
                _ => 2,
            });
        }
        for m in members {
            match m {
                M::Int(..) => out.push(8),
                M::Byte(..) => out.push(2),
                M::Bool(..) => out.push(1),
                M::Long(..) => out.push(9),
                _ => {}
            }
        }
        out.extend_from_slice(&2i32.to_le_bytes());
        for m in members {
            match *m {
                M::Int(_, v) => out.extend_from_slice(&v.to_le_bytes()),
                M::Byte(_, v) => out.push(v),
                M::Bool(_, v) => out.push(v as u8),
                M::Long(_, v) => out.extend_from_slice(&v.to_le_bytes()),
                M::Ref(_, r) => {
                    out.push(9);
                    out.extend_from_slice(&r.to_le_bytes());
                }
                M::Str(_, id, s) => {
                    out.push(6);
                    out.extend_from_slice(&id.to_le_bytes());
                    string(out, s);
                }
                M::Null(_) => out.push(10),
            }
        }
    }

    pub(crate) fn header(out: &mut Vec<u8>, root: i32) {
        out.push(0);
        out.extend_from_slice(&root.to_le_bytes());
        out.extend_from_slice(&(-1i32).to_le_bytes());
        out.extend_from_slice(&1i32.to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes());
        out.push(12);
        out.extend_from_slice(&2i32.to_le_bytes());
        string(out, "PaintDotNet.Data");
    }

    /// ArraySingleObject (record 16) of references (`None` = null).
    pub(crate) fn object_array(out: &mut Vec<u8>, id: i32, refs: &[Option<i32>]) {
        out.push(16);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&(refs.len() as i32).to_le_bytes());
        for r in refs {
            match r {
                Some(r) => {
                    out.push(9);
                    out.extend_from_slice(&r.to_le_bytes());
                }
                None => out.push(10),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::write::*;
    use super::*;

    #[test]
    fn forward_references_and_members() {
        let mut d = Vec::new();
        header(&mut d, 1);
        class_record(
            &mut d,
            1,
            "A",
            &[
                M::Int("Base+width", 7),
                M::Ref("child", 3),
                M::Str("name", 2, "hello"),
            ],
        );
        class_record(&mut d, 3, "B", &[M::Bool("visible", true), M::Null("x")]);
        d.push(11);
        d.extend_from_slice(b"tail");
        let s = parse(&d).unwrap();
        assert_eq!(s.root, 1);
        let a = &s.objects[&1];
        assert_eq!(int(a.member("width")), Some(7));
        assert_eq!(s.string(a.member("name").unwrap()), Some("hello"));
        let b = s.get(a.member("child").unwrap()).unwrap();
        assert_eq!(b.class_name(), Some("B"));
        assert_eq!(boolean(b.member("visible")), Some(true));
        assert_eq!(s.order, vec![1, 3]);
        assert_eq!(&d[s.end..], b"tail");
    }

    /// BinaryLibrary records chained without end used to recurse once each.
    #[test]
    fn chained_libraries_do_not_recurse() {
        let mut d = Vec::new();
        header(&mut d, 1);
        for _ in 0..200_000 {
            d.push(12);
            d.extend_from_slice(&2i32.to_le_bytes());
            string(&mut d, "x");
        }
        d.push(11);
        let s = specular_thread::on_stack(256 * 1024, || parse(&d).map(|s| s.end));
        assert_eq!(s.unwrap(), d.len());
    }

    #[test]
    fn hostile_streams_fail_cleanly() {
        let mut d = Vec::new();
        header(&mut d, 1);
        class_record(&mut d, 1, "A", &[M::Int("w", 1), M::Ref("c", 3)]);
        object_array(&mut d, 3, &[Some(1), None, Some(4)]);
        d.push(11);
        assert!(parse(&d).is_ok());
        for n in 0..d.len() {
            assert!(parse(&d[..n]).is_err());
        }
        let mut seed = 99u32;
        for _ in 0..2000 {
            let mut b = d.clone();
            for _ in 0..3 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let at = seed as usize % b.len();
                b[at] = (seed >> 8) as u8;
            }
            let _ = parse(&b);
        }
        // A huge declared array of nulls is bounded.
        let mut h = Vec::new();
        header(&mut h, 1);
        h.push(16);
        h.extend_from_slice(&1i32.to_le_bytes());
        h.extend_from_slice(&i32::MAX.to_le_bytes());
        h.push(14);
        h.extend_from_slice(&i32::MAX.to_le_bytes());
        h.push(11);
        assert!(parse(&h).is_err());
        // Deep nesting is bounded.
        let mut deep = Vec::new();
        header(&mut deep, 1);
        for i in 0..200 {
            deep.push(5);
            deep.extend_from_slice(&(i + 1i32).to_le_bytes());
            string(&mut deep, "N");
            deep.extend_from_slice(&1i32.to_le_bytes());
            string(&mut deep, "c");
            deep.push(2);
            deep.extend_from_slice(&2i32.to_le_bytes());
        }
        assert!(parse(&deep).is_err());
    }
}

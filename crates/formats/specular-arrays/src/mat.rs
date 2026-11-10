//! MATLAB `.mat` files, levels 4 and 5 (the v6 and v7 formats: v7 compresses variables).
//! Version 7.3 files are HDF5 with a 512-byte header; [`version`] recognises them so the
//! caller can read them through `specular-hdf5`.
//!
//! * **Level 5:** a 128-byte header (text, subsystem offset, version, `IM`/`MI` byte order),
//!   then data elements: `miMATRIX` arrays and `miCOMPRESSED` zlib streams holding one.
//!   Numeric classes (stored in any `mi` type, as MATLAB does to save space), logical, char,
//!   sparse (densified by the caller), struct and cell arrays (walked: every element becomes
//!   a variable named by its path, `s.field`, `s(2).field`, `c{1,3}`) and objects (their
//!   fields). Function handles and MATLAB class objects (`string`, `table`, `datetime`…) are
//!   listed with a reason.
//! * **Level 4:** a sequence of matrices, each a 20-byte header (`MOPT` type, rows, columns,
//!   imaginary flag, name length), name and data; numeric, text and sparse, either byte order.
//!
//! Arrays are column-major: a variable's shape is MATLAB's (rows, columns, pages…), stored in
//! Fortran order.

use std::ops::Range;

use crate::{Dtype, Elem, Error, MAX_DIMS, Part, PlaneSpec, Result, bad};

/// Variables listed at most (struct and cell elements included).
const MAX_VARS: usize = 100_000;
/// Nesting of structs and cells walked at most.
const MAX_DEPTH: usize = 32;
/// Bytes of a struct field name (MATLAB allows 63).
const MAX_FIELD_NAME: usize = 64;
/// Inflated bytes looked at to read a compressed variable's header.
const PREFIX: usize = 4096;
/// A compressed variable inflated at open to walk it (cells, structs) at most.
pub const MAX_WALK: usize = 1 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    /// Level 4 (no header).
    V4,
    /// Level 5 (the v6 and v7 formats).
    V5,
    /// HDF5-based (v7.3).
    V73,
}

/// Which level `data` is, from its header (level 4 from a plausible first matrix).
pub fn version(data: &[u8]) -> Option<Version> {
    if data.len() >= 128 && data.starts_with(b"MATLAB ") {
        // v7.3 files are HDF5 after the 512-byte header (early ones still say "7.0").
        if data.starts_with(b"MATLAB 7.3 MAT-file")
            || data.get(512..520) == Some(b"\x89HDF\r\n\x1a\n".as_slice())
        {
            return Some(Version::V73);
        }
        if matches!(&data[126..128], b"IM" | b"MI") {
            return Some(Version::V5);
        }
    }
    v4_header(data, 0).map(|_| Version::V4)
}

/// Whether the first bytes are a level 5 or 7.3 header (level 4 has no signature): the
/// header text and, for level 5, a byte-order mark; v7.3 files carry one too.
pub fn sniff(head: &[u8]) -> bool {
    sniff_text(head) && head.get(126..128).is_some_and(|m| m == b"IM" || m == b"MI")
}

/// Whether the first bytes start with the header text ("MATLAB 5.0 MAT-file"); [`sniff`]
/// also checks the byte-order mark at byte 126.
pub fn sniff_text(head: &[u8]) -> bool {
    head.starts_with(b"MATLAB ") && head.get(7..19).is_some_and(|v| v.ends_with(b" MAT-file"))
}

/// A MATLAB array class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Cell,
    Struct,
    Object,
    Char,
    Sparse,
    Double,
    Single,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Function,
    Opaque,
}

impl Class {
    fn from_code(c: u8) -> Option<Class> {
        Some(match c {
            1 => Class::Cell,
            2 => Class::Struct,
            3 => Class::Object,
            4 => Class::Char,
            5 => Class::Sparse,
            6 => Class::Double,
            7 => Class::Single,
            8 => Class::Int8,
            9 => Class::UInt8,
            10 => Class::Int16,
            11 => Class::UInt16,
            12 => Class::Int32,
            13 => Class::UInt32,
            14 => Class::Int64,
            15 => Class::UInt64,
            16 => Class::Function,
            17 => Class::Opaque,
            _ => return None,
        })
    }

    /// MATLAB's name for the class.
    pub fn name(self) -> &'static str {
        match self {
            Class::Cell => "cell",
            Class::Struct => "struct",
            Class::Object => "object",
            Class::Char => "char",
            Class::Sparse => "sparse",
            Class::Double => "double",
            Class::Single => "single",
            Class::Int8 => "int8",
            Class::UInt8 => "uint8",
            Class::Int16 => "int16",
            Class::UInt16 => "uint16",
            Class::Int32 => "int32",
            Class::UInt32 => "uint32",
            Class::Int64 => "int64",
            Class::UInt64 => "uint64",
            Class::Function => "function_handle",
            Class::Opaque => "opaque",
        }
    }

    /// The class of a v7.3 `MATLAB_class` attribute.
    pub fn from_name(s: &str) -> Option<Class> {
        Some(match s {
            "cell" => Class::Cell,
            "struct" => Class::Struct,
            "char" => Class::Char,
            "double" => Class::Double,
            "single" => Class::Single,
            "int8" => Class::Int8,
            "uint8" => Class::UInt8,
            "int16" => Class::Int16,
            "uint16" => Class::UInt16,
            "int32" => Class::Int32,
            "uint32" => Class::UInt32,
            "int64" => Class::Int64,
            "uint64" => Class::UInt64,
            "logical" => Class::UInt8,
            "function_handle" => Class::Function,
            _ => return None,
        })
    }

    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Class::Double
                | Class::Single
                | Class::Int8
                | Class::UInt8
                | Class::Int16
                | Class::UInt16
                | Class::Int32
                | Class::UInt32
                | Class::Int64
                | Class::UInt64
        )
    }

    /// Bits per value of the class.
    pub fn bits(self) -> u32 {
        match self {
            Class::Double | Class::Int64 | Class::UInt64 => 64,
            Class::Single | Class::Int32 | Class::UInt32 => 32,
            Class::Int16 | Class::UInt16 | Class::Char => 16,
            _ => 8,
        }
    }
}

/// Bytes of one variable's data: in the file (`buf` None) or in compressed variable `buf`
/// once inflated ([`Mat::inflate`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub buf: Option<usize>,
    pub range: Range<usize>,
    /// The stored type (which may be narrower than the class).
    pub dtype: Dtype,
}

/// What a variable holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// Numeric or logical values, column-major.
    Numeric { re: Span, im: Option<Span> },
    /// Compressed sparse columns: row indices (`ir`), column starts (`jc`), values.
    Sparse {
        ir: Span,
        jc: Span,
        re: Span,
        im: Option<Span>,
    },
    /// A level 4 sparse matrix: `rows` triplets (row, column, real, imaginary), the last
    /// giving the size, stored as a `rows` × `cols` column-major matrix.
    Triplets { data: Span, rows: u64, cols: u64 },
    /// Characters, one line per row.
    Text(String),
    /// A struct, object or cell array: its elements follow as further variables.
    Container,
    /// An empty array.
    Empty,
    /// A variable that cannot be shown, with the reason.
    Refused(String),
}

/// One variable, or one element of a struct or cell array.
#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    /// The variable's name, or the element's path (`s.a`, `s(2).a`, `c{1,3}`).
    pub path: String,
    pub class: Class,
    /// MATLAB's shape (rows, columns, …); at least two dimensions.
    pub dims: Vec<u64>,
    pub complex: bool,
    pub logical: bool,
    pub global: bool,
    /// The class name of an object.
    pub class_name: Option<String>,
    pub kind: Kind,
    /// Nesting depth (0 for a top-level variable).
    pub depth: usize,
}

impl Var {
    /// "double", "complex single", "logical", "sparse double"…
    pub fn describe(&self) -> String {
        let base = if self.logical {
            "logical".to_owned()
        } else if let Some(n) = &self.class_name {
            n.clone()
        } else {
            self.class.name().to_owned()
        };
        match (&self.kind, self.complex) {
            (Kind::Sparse { .. }, true) => format!(
                "sparse complex {}",
                if self.logical { "logical" } else { "double" }
            ),
            (Kind::Sparse { .. }, false) => {
                format!("sparse {}", if self.logical { "logical" } else { "double" })
            }
            (_, true) => format!("complex {base}"),
            _ => base,
        }
    }

    /// Elements in the array.
    pub fn count(&self) -> u64 {
        crate::count(&self.dims).unwrap_or(u64::MAX)
    }
}

/// A compressed variable: its zlib stream in the file and its inflated size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compressed {
    pub range: Range<usize>,
    pub len: usize,
}

/// A parsed `.mat` file (level 4 or 5).
#[derive(Clone, Debug)]
pub struct Mat {
    pub version: Version,
    /// The header text (level 5), trimmed.
    pub header: String,
    pub big: bool,
    /// Variables and their elements, in file order (containers before their elements).
    pub vars: Vec<Var>,
    pub compressed: Vec<Compressed>,
    /// Things skipped while reading (damaged elements), for the info panel.
    pub notes: Vec<String>,
}

impl Mat {
    /// The bytes a span lies in: the file, or `inflated` for a compressed variable.
    pub fn bytes<'a>(&self, data: &'a [u8], span: &Span, inflated: Option<&'a [u8]>) -> &'a [u8] {
        let b = match span.buf {
            None => data,
            Some(_) => inflated.unwrap_or(&[]),
        };
        b.get(span.range.clone()).unwrap_or(&[])
    }

    /// Inflates compressed variable `i` (at most `max` bytes).
    pub fn inflate(&self, data: &[u8], i: usize, max: usize) -> Result<Vec<u8>> {
        let c = self
            .compressed
            .get(i)
            .ok_or_else(|| Error::Malformed("no such compressed variable".into()))?;
        if c.len > max {
            return Err(Error::TooLarge);
        }
        let src = data
            .get(c.range.clone())
            .ok_or_else(|| Error::Truncated("MATLAB compressed variable".into()))?;
        specular_png::inflate_zlib(src, c.len)
            .map_err(|e| Error::Malformed(format!("MATLAB compressed variable: {e}")))
    }

    /// Reads one plane of numeric variable `v` (its spans' bytes from `data`, or `inflated`
    /// when it lies in a compressed variable).
    pub fn read_plane(
        &self,
        data: &[u8],
        inflated: Option<&[u8]>,
        v: &Var,
        spec: &PlaneSpec,
        part: Part,
        out: &mut Vec<f32>,
    ) -> Result<bool> {
        let Kind::Numeric { re, im } = &v.kind else {
            return bad("not a numeric variable");
        };
        let strides = crate::strides(&v.dims, true);
        let part_re = if im.is_some() { Part::Real } else { part };
        let mut t = crate::read_plane(
            self.bytes(data, re, inflated),
            re.dtype,
            &v.dims,
            &strides,
            spec,
            part_re,
            out,
        )?;
        if let Some(im) = im {
            let mut iv = Vec::new();
            t |= crate::read_plane(
                self.bytes(data, im, inflated),
                im.dtype,
                &v.dims,
                &strides,
                spec,
                Part::Real,
                &mut iv,
            )?;
            crate::combine(out, &iv, part);
        }
        Ok(t)
    }

    /// A 2-D sparse variable as a dense column-major array (rows × columns values).
    pub fn sparse_dense(
        &self,
        data: &[u8],
        inflated: Option<&[u8]>,
        v: &Var,
        part: Part,
    ) -> Result<Vec<f32>> {
        let (m, n) = (v.dims[0], v.dims.get(1).copied().unwrap_or(1));
        let total = m
            .checked_mul(n)
            .filter(|&t| t <= crate::MAX_PLANE)
            .ok_or(Error::TooLarge)?;
        let mut out = Vec::new();
        crate::alloc(&mut out, total as usize)?;
        if let Kind::Triplets {
            data: span,
            rows,
            cols,
        } = &v.kind
        {
            let col = |c: u64| triplet_column(data, span, *rows, *cols, c);
            let (r, c, mut re) = (col(0), col(1), col(2));
            if *cols >= 4 {
                crate::combine(&mut re, &col(3), part);
            }
            for k in 0..(*rows as usize).saturating_sub(1) {
                let (i, j) = (r[k], c[k]);
                if !(i >= 1.0 && j >= 1.0) || i as u64 > m || j as u64 > n {
                    continue;
                }
                out[(j as usize - 1) * m as usize + i as usize - 1] = re[k];
            }
            return Ok(out);
        }
        let Kind::Sparse { ir, jc, re, im } = &v.kind else {
            return bad("not a sparse variable");
        };
        let ints = |s: &Span| -> Vec<i64> {
            let b = self.bytes(data, s, inflated);
            let size = s.dtype.size().max(1);
            let mut tmp = Vec::new();
            let k = b.len() / size;
            let _ = crate::read_plane(
                b,
                s.dtype,
                &[k as u64],
                &[1],
                &PlaneSpec {
                    x_dim: 0,
                    y_dim: None,
                    index: vec![0],
                },
                Part::Real,
                &mut tmp,
            );
            tmp.iter().map(|&x| x as i64).collect()
        };
        let rows = ints(ir);
        let cols = ints(jc);
        let vals = |s: &Span| -> Vec<f32> {
            let b = self.bytes(data, s, inflated);
            let k = b.len() / s.dtype.size().max(1);
            let mut tmp = Vec::new();
            let _ = crate::read_plane(
                b,
                s.dtype,
                &[k as u64],
                &[1],
                &PlaneSpec {
                    x_dim: 0,
                    y_dim: None,
                    index: vec![0],
                },
                Part::Real,
                &mut tmp,
            );
            tmp
        };
        let mut re_v = vals(re);
        if let Some(im) = im {
            let iv = vals(im);
            crate::combine(&mut re_v, &iv, part);
        }
        if cols.len() < n as usize + 1 {
            return bad("MATLAB sparse column index");
        }
        for c in 0..n as usize {
            let (a, b) = (cols[c].max(0) as usize, cols[c + 1].max(0) as usize);
            for (k, &r) in rows.iter().enumerate().take(b).skip(a) {
                if r < 0 || r as u64 >= m {
                    continue;
                }
                // Logical sparse arrays may omit the values: every stored entry is true.
                let val = re_v.get(k).copied().unwrap_or(1.0);
                out[c * m as usize + r as usize] = val;
            }
        }
        Ok(out)
    }
}

/// Parses a level 4 or 5 file.
pub fn parse(data: &[u8]) -> Result<Mat> {
    match version(data) {
        Some(Version::V5) => parse_v5(data),
        Some(Version::V4) => parse_v4(data),
        Some(Version::V73) => Err(Error::Unsupported(
            "MATLAB v7.3 files are HDF5; read them through the HDF5 reader".into(),
        )),
        None => Err(Error::NotArray),
    }
}

// ----- Level 5 --------------------------------------------------------------------------

const MI_INT8: u32 = 1;
const MI_UINT8: u32 = 2;
const MI_INT16: u32 = 3;
const MI_UINT16: u32 = 4;
const MI_INT32: u32 = 5;
const MI_UINT32: u32 = 6;
const MI_SINGLE: u32 = 7;
const MI_DOUBLE: u32 = 9;
const MI_INT64: u32 = 12;
const MI_UINT64: u32 = 13;
const MI_MATRIX: u32 = 14;
const MI_COMPRESSED: u32 = 15;
const MI_UTF8: u32 = 16;
const MI_UTF16: u32 = 17;
const MI_UTF32: u32 = 18;

fn mi_elem(t: u32) -> Option<Elem> {
    Some(match t {
        MI_INT8 => Elem::I8,
        MI_UINT8 | MI_UTF8 => Elem::U8,
        MI_INT16 => Elem::I16,
        MI_UINT16 | MI_UTF16 => Elem::U16,
        MI_INT32 => Elem::I32,
        MI_UINT32 | MI_UTF32 => Elem::U32,
        MI_SINGLE => Elem::F32,
        MI_DOUBLE => Elem::F64,
        MI_INT64 => Elem::I64,
        MI_UINT64 => Elem::U64,
        _ => return None,
    })
}

/// A data element: its type, where its data lies and where the next element starts.
#[derive(Clone, Debug)]
struct Tag {
    ty: u32,
    data: Range<usize>,
    next: usize,
}

#[derive(Clone, Copy)]
struct Rd<'a> {
    d: &'a [u8],
    big: bool,
    /// The buffer the bytes are (None: the file).
    buf: Option<usize>,
}

impl Rd<'_> {
    fn u32(&self, p: usize) -> Option<u32> {
        let b: [u8; 4] = self.d.get(p..p + 4)?.try_into().ok()?;
        Some(if self.big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    fn i32(&self, p: usize) -> Option<i32> {
        self.u32(p).map(|v| v as i32)
    }

    fn tag(&self, p: usize) -> Result<Tag> {
        let w = self
            .u32(p)
            .ok_or_else(|| Error::Truncated("MATLAB data element".into()))?;
        if w >> 16 != 0 {
            // Small data element: type and size share the first word, data in the second.
            let n = (w >> 16) as usize;
            if n > 4 {
                return bad("MATLAB small data element");
            }
            return Ok(Tag {
                ty: w & 0xffff,
                data: p + 4..p + 4 + n,
                next: p + 8,
            });
        }
        let n = self
            .u32(p + 4)
            .ok_or_else(|| Error::Truncated("MATLAB data element".into()))?
            as usize;
        let start = p + 8;
        let end = start
            .checked_add(n)
            .ok_or_else(|| Error::Malformed("MATLAB element size".into()))?;
        let next = if w == MI_COMPRESSED {
            end
        } else {
            end.checked_add(7).map(|e| e & !7).unwrap_or(usize::MAX)
        };
        Ok(Tag {
            ty: w,
            data: start..end,
            next,
        })
    }

    fn span(&self, t: &Tag) -> Result<Span> {
        let elem =
            mi_elem(t.ty).ok_or_else(|| Error::Malformed(format!("MATLAB data type {}", t.ty)))?;
        Ok(Span {
            buf: self.buf,
            range: t.data.clone(),
            dtype: Dtype {
                elem,
                big: self.big,
            },
        })
    }

    fn bytes(&self, t: &Tag) -> &[u8] {
        self.d.get(t.data.clone()).unwrap_or(&[])
    }

    /// The values of an integer element.
    fn ints(&self, t: &Tag) -> Result<Vec<i64>> {
        let s = self.span(t)?;
        let b = self.bytes(t);
        let size = s.dtype.size();
        if b.len() < t.data.len() {
            return Err(Error::Truncated("MATLAB data element".into()));
        }
        Ok(b.chunks_exact(size)
            .map(|c| {
                let mut a = [0u8; 8];
                a[..size].copy_from_slice(c);
                if self.big {
                    a[..size].reverse();
                }
                let u = u64::from_le_bytes(a);
                match s.dtype.elem {
                    Elem::I8 => i64::from(u as u8 as i8),
                    Elem::I16 => i64::from(u as u16 as i16),
                    Elem::I32 => i64::from(u as u32 as i32),
                    _ => u as i64,
                }
            })
            .collect())
    }
}

fn header_text(data: &[u8]) -> String {
    let t = &data[..116.min(data.len())];
    let end = t.iter().position(|&b| b == 0).unwrap_or(t.len());
    String::from_utf8_lossy(&t[..end]).trim().to_owned()
}

fn parse_v5(data: &[u8]) -> Result<Mat> {
    let big = &data[126..128] == b"MI";
    let mut mat = Mat {
        version: Version::V5,
        header: header_text(data),
        big,
        vars: Vec::new(),
        compressed: Vec::new(),
        notes: Vec::new(),
    };
    let rd = Rd {
        d: data,
        big,
        buf: None,
    };
    let mut p = 128;
    while p + 8 <= data.len() {
        let t = match rd.tag(p) {
            Ok(t) => t,
            Err(e) => {
                mat.notes.push(format!("stopped at byte {p}: {e}"));
                break;
            }
        };
        if t.data.end > data.len() {
            mat.notes
                .push(format!("the variable at byte {p} is cut short"));
            if t.ty != MI_MATRIX {
                break;
            }
        }
        match t.ty {
            MI_MATRIX => {
                // A damaged variable is skipped (said so); the others still read.
                let before = mat.vars.len();
                match walk_matrix(&rd, &t, String::new(), 0, &mut mat) {
                    Ok(()) => {}
                    Err(Error::TooLarge) => return Err(Error::TooLarge),
                    Err(e) => {
                        mat.vars.truncate(before);
                        mat.notes
                            .push(format!("the variable at byte {p} was skipped: {e}"));
                    }
                }
            }
            MI_COMPRESSED => {
                if let Err(e) = compressed_var(data, &t, big, &mut mat) {
                    match e {
                        Error::TooLarge => return Err(e),
                        e => mat.notes.push(format!(
                            "a compressed variable at byte {p} was skipped: {e}"
                        )),
                    }
                }
            }
            _ => {}
        }
        if mat.vars.len() > MAX_VARS {
            return Err(Error::TooLarge);
        }
        if t.next <= p {
            break;
        }
        p = t.next;
    }
    Ok(mat)
}

fn compressed_var(data: &[u8], t: &Tag, big: bool, mat: &mut Mat) -> Result<()> {
    let src = data
        .get(t.data.start..t.data.end.min(data.len()))
        .unwrap_or(&[]);
    // The inner element's tag gives the inflated size; zlib's two header bytes precede the
    // raw DEFLATE stream.
    let head = specular_png::inflate_raw_prefix(src.get(2..).unwrap_or(&[]), PREFIX)
        .map_err(|e| Error::Malformed(format!("MATLAB compressed variable: {e}")))?;
    let hr = Rd {
        d: &head,
        big,
        buf: None,
    };
    let inner = hr.tag(0)?;
    if inner.ty != MI_MATRIX {
        return Ok(());
    }
    let len = inner.data.end;
    let id = mat.compressed.len();
    mat.compressed.push(Compressed {
        range: t.data.start..t.data.end.min(data.len()),
        len,
    });
    // A plain numeric or char array is described by its first elements; containers, sparse,
    // complex and char arrays are inflated to walk them.
    let flags = hr.tag(inner.data.start)?;
    let class = hr
        .u32(flags.data.start)
        .and_then(|f| Class::from_code((f & 0xff) as u8));
    let complex = hr.u32(flags.data.start).is_some_and(|f| f & 0x800 != 0);
    let simple = class.is_some_and(Class::is_numeric) && !complex;
    if simple {
        let r = Rd {
            d: &head,
            big,
            buf: Some(id),
        };
        // A numeric variable is pushed only once its data element's tag is read, so a prefix
        // too short to reach it leaves nothing behind: then the whole variable is walked.
        match walk_matrix(&r, &inner, String::new(), 0, mat) {
            Ok(()) => return Ok(()),
            Err(Error::Truncated(_)) => {}
            Err(e) => return Err(e),
        }
    }
    if len > MAX_WALK {
        return Err(Error::TooLarge);
    }
    let full = specular_png::inflate_zlib(src, len)
        .map_err(|e| Error::Malformed(format!("MATLAB compressed variable: {e}")))?;
    let r = Rd {
        d: &full,
        big,
        buf: Some(id),
    };
    walk_matrix(&r, &inner, String::new(), 0, mat)
}

/// Reads the `miMATRIX` element `t` (its tag already read) into `mat.vars` under `path`
/// (empty: the element's own name).
fn walk_matrix(rd: &Rd, t: &Tag, path: String, depth: usize, mat: &mut Mat) -> Result<()> {
    if depth > MAX_DEPTH {
        return bad("MATLAB structs nested too deeply");
    }
    if mat.vars.len() > MAX_VARS {
        return Err(Error::TooLarge);
    }
    let mut v = Var {
        path,
        class: Class::Double,
        dims: vec![0, 0],
        complex: false,
        logical: false,
        global: false,
        class_name: None,
        kind: Kind::Empty,
        depth,
    };
    if t.data.is_empty() {
        // An empty element (an empty cell's contents).
        mat.vars.push(v);
        return Ok(());
    }
    let flags_t = rd.tag(t.data.start)?;
    let flags = rd
        .u32(flags_t.data.start)
        .ok_or_else(|| Error::Truncated("MATLAB array flags".into()))?;
    let code = (flags & 0xff) as u8;
    v.complex = flags & 0x800 != 0;
    v.global = flags & 0x400 != 0;
    v.logical = flags & 0x200 != 0;
    let Some(class) = Class::from_code(code) else {
        v.kind = Kind::Refused(format!("MATLAB array class {code} is not supported"));
        mat.vars.push(v);
        return Ok(());
    };
    v.class = class;
    let mut p = flags_t.next;
    if class == Class::Opaque {
        v.kind = Kind::Refused(
            "MATLAB class objects (string, table, datetime and the like) are not supported".into(),
        );
        if v.path.is_empty() {
            let name_t = rd.tag(p)?;
            v.path = String::from_utf8_lossy(rd.bytes(&name_t)).into_owned();
        }
        mat.vars.push(v);
        return Ok(());
    }
    let dims_t = rd.tag(p)?;
    let dims = rd.ints(&dims_t)?;
    if dims.len() > MAX_DIMS || dims.iter().any(|&d| d < 0) {
        return bad("MATLAB dimensions");
    }
    v.dims = dims.iter().map(|&d| d as u64).collect();
    while v.dims.len() < 2 {
        v.dims.push(1);
    }
    p = dims_t.next;
    let name_t = rd.tag(p)?;
    let name = String::from_utf8_lossy(rd.bytes(&name_t)).into_owned();
    if v.path.is_empty() {
        v.path = name;
    }
    p = name_t.next;
    let n = crate::count(&v.dims).ok_or(Error::TooLarge)?;
    match class {
        Class::Cell => {
            v.kind = Kind::Container;
            let base = v.path.clone();
            let dims = v.dims.clone();
            mat.vars.push(v);
            for i in 0..n {
                if p >= t.data.end {
                    break;
                }
                let ct = rd.tag(p)?;
                walk_matrix(
                    rd,
                    &ct,
                    format!("{base}{{{}}}", subscript(&dims, i)),
                    depth + 1,
                    mat,
                )?;
                p = ct.next;
            }
        }
        Class::Struct | Class::Object => {
            if class == Class::Object {
                let cn = rd.tag(p)?;
                v.class_name = Some(String::from_utf8_lossy(rd.bytes(&cn)).into_owned());
                p = cn.next;
            }
            let len_t = rd.tag(p)?;
            let flen = rd.i32(len_t.data.start).unwrap_or(0).max(0) as usize;
            p = len_t.next;
            let names_t = rd.tag(p)?;
            let raw = rd.bytes(&names_t);
            let names: Vec<String> = if flen == 0 {
                Vec::new()
            } else {
                // MATLAB names are at most 63 bytes; a longer field length would put one
                // huge name into every element's path.
                raw.chunks(flen)
                    .map(|c| {
                        let c = &c[..c.len().min(MAX_FIELD_NAME)];
                        let e = c.iter().position(|&b| b == 0).unwrap_or(c.len());
                        String::from_utf8_lossy(&c[..e]).into_owned()
                    })
                    .collect()
            };
            p = names_t.next;
            v.kind = Kind::Container;
            let base = v.path.clone();
            let one = n == 1;
            let dims = v.dims.clone();
            mat.vars.push(v);
            'elems: for i in 0..n {
                for f in &names {
                    if p >= t.data.end {
                        break 'elems;
                    }
                    let ft = rd.tag(p)?;
                    let path = if one {
                        format!("{base}.{f}")
                    } else {
                        format!("{base}({}).{f}", subscript(&dims, i))
                    };
                    walk_matrix(rd, &ft, path, depth + 1, mat)?;
                    p = ft.next;
                }
            }
        }
        Class::Char => {
            v.kind = if n == 0 {
                Kind::Empty
            } else {
                let ct = rd.tag(p)?;
                Kind::Text(char_text(rd, &ct, &v.dims)?)
            };
            mat.vars.push(v);
        }
        Class::Sparse => {
            let ir = rd.tag(p)?;
            let jc = rd.tag(ir.next)?;
            let pr = rd.tag(jc.next)?;
            let pi = if v.complex {
                Some(rd.span(&rd.tag(pr.next)?)?)
            } else {
                None
            };
            let ir_s = rd.span(&ir)?;
            if v.dims.len() != 2 {
                v.kind = Kind::Refused("sparse arrays of more than two dimensions".into());
            } else {
                v.kind = Kind::Sparse {
                    ir: ir_s,
                    jc: rd.span(&jc)?,
                    re: rd.span(&pr)?,
                    im: pi,
                };
            }
            mat.vars.push(v);
        }
        Class::Function => {
            v.kind = Kind::Refused("MATLAB function handles hold no data to show".into());
            mat.vars.push(v);
        }
        _ => {
            if n == 0 {
                mat.vars.push(v);
                return Ok(());
            }
            let pr = rd.tag(p)?;
            if pr.data.start > rd.d.len() {
                return Err(Error::Truncated("MATLAB array data".into()));
            }
            let re = rd.span(&pr)?;
            let need = n.saturating_mul(re.dtype.size() as u64);
            if (pr.data.len() as u64) < need {
                return bad("MATLAB array data shorter than its dimensions");
            }
            let im = if v.complex {
                let it = rd.tag(pr.next)?;
                Some(rd.span(&it)?)
            } else {
                None
            };
            v.kind = Kind::Numeric { re, im };
            mat.vars.push(v);
        }
    }
    Ok(())
}

/// MATLAB's 1-based subscript of linear index `i` (column-major): "3" for vectors, "2,1"
/// otherwise.
fn subscript(dims: &[u64], i: u64) -> String {
    let vector = dims.iter().filter(|&&d| d != 1).count() <= 1;
    if vector {
        return (i + 1).to_string();
    }
    let mut r = i;
    let mut parts = Vec::new();
    for &d in dims {
        let d = d.max(1);
        parts.push((r % d + 1).to_string());
        r /= d;
    }
    parts.join(",")
}

/// A char array's text: each row a line, column-major storage.
fn char_text(rd: &Rd, t: &Tag, dims: &[u64]) -> Result<String> {
    let b = rd.bytes(t);
    let units: Vec<u32> = match t.ty {
        MI_UTF8 => {
            return Ok(rows_of(
                &String::from_utf8_lossy(b)
                    .chars()
                    .map(u32::from)
                    .collect::<Vec<_>>(),
                dims,
            ));
        }
        MI_UINT8 | MI_INT8 => b.iter().map(|&c| u32::from(c)).collect(),
        MI_UINT16 | MI_UTF16 | MI_INT16 => b
            .chunks_exact(2)
            .map(|c| {
                u32::from(if rd.big {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                })
            })
            .collect(),
        MI_UINT32 | MI_UTF32 | MI_INT32 => b
            .chunks_exact(4)
            .map(|c| {
                let a = [c[0], c[1], c[2], c[3]];
                if rd.big {
                    u32::from_be_bytes(a)
                } else {
                    u32::from_le_bytes(a)
                }
            })
            .collect(),
        MI_DOUBLE => b
            .chunks_exact(8)
            .map(|c| {
                let a: [u8; 8] = c.try_into().unwrap_or_default();
                (if rd.big {
                    f64::from_be_bytes(a)
                } else {
                    f64::from_le_bytes(a)
                }) as u32
            })
            .collect(),
        _ => return bad("MATLAB char data type"),
    };
    Ok(rows_of(&units, dims))
}

/// Characters stored column-major as lines (rows), trailing spaces trimmed.
fn rows_of(units: &[u32], dims: &[u64]) -> String {
    let m = dims.first().copied().unwrap_or(1).max(1) as usize;
    let cols = units.len() / m;
    let ch = |u: u32| char::from_u32(u).unwrap_or('\u{fffd}');
    (0..m)
        .map(|r| {
            let s: String = (0..cols).map(|c| ch(units[c * m + r])).collect();
            s.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ----- Level 4 --------------------------------------------------------------------------

/// A level 4 matrix header at `p`: (big-endian, precision P, type T, rows, cols, imaginary,
/// name, data start).
struct V4 {
    big: bool,
    prec: u32,
    ty: u32,
    rows: u64,
    cols: u64,
    imag: bool,
    name: String,
    data: usize,
}

fn v4_header(d: &[u8], p: usize) -> Option<V4> {
    let h = d.get(p..p + 20)?;
    for big in [false, true] {
        let w = |i: usize| {
            let a: [u8; 4] = h[i * 4..i * 4 + 4].try_into().unwrap_or_default();
            if big {
                i32::from_be_bytes(a)
            } else {
                i32::from_le_bytes(a)
            }
        };
        let (ty, rows, cols, imag, nlen) = (w(0), w(1), w(2), w(3), w(4));
        if !(0..5000).contains(&ty) {
            continue;
        }
        let (m, o, pr, t) = (ty / 1000, (ty / 100) % 10, (ty / 10) % 10, ty % 10);
        // M: 0 little-endian IEEE, 1 big-endian IEEE (VAX and Cray are not read).
        if m != i32::from(big) || o != 0 || pr > 5 || t > 2 {
            continue;
        }
        if rows < 0 || cols < 0 || !(0..=1).contains(&imag) || !(1..=4096).contains(&nlen) {
            continue;
        }
        let name = d.get(p + 20..p + 20 + nlen as usize)?;
        // The name is NUL-terminated printable text.
        if name.last() != Some(&0) || !name[..name.len() - 1].iter().all(|&c| c.is_ascii_graphic())
        {
            continue;
        }
        let size = [8u64, 4, 4, 2, 2, 1][pr as usize];
        let n = (rows as u64).checked_mul(cols as u64)?;
        let data = p + 20 + nlen as usize;
        let len = n.checked_mul(size)?.checked_mul(1 + imag as u64)?;
        if (data as u64).checked_add(len)? > d.len() as u64 {
            continue;
        }
        return Some(V4 {
            big,
            prec: pr as u32,
            ty: t as u32,
            rows: rows as u64,
            cols: cols as u64,
            imag: imag == 1,
            name: String::from_utf8_lossy(&name[..name.len() - 1]).into_owned(),
            data,
        });
    }
    None
}

fn parse_v4(data: &[u8]) -> Result<Mat> {
    let mut mat = Mat {
        version: Version::V4,
        header: String::new(),
        big: false,
        vars: Vec::new(),
        compressed: Vec::new(),
        notes: Vec::new(),
    };
    let mut p = 0;
    while p < data.len() {
        let Some(h) = v4_header(data, p) else {
            mat.notes.push(format!(
                "{} bytes after the last matrix were not read",
                data.len() - p
            ));
            break;
        };
        mat.big = h.big;
        let elem = [
            Elem::F64,
            Elem::F32,
            Elem::I32,
            Elem::I16,
            Elem::U16,
            Elem::U8,
        ][h.prec as usize];
        let dtype = Dtype { elem, big: h.big };
        let n = h.rows * h.cols;
        let len = (n * dtype.size() as u64) as usize;
        let re = Span {
            buf: None,
            range: h.data..h.data + len,
            dtype,
        };
        let im = h.imag.then(|| Span {
            buf: None,
            range: h.data + len..h.data + 2 * len,
            dtype,
        });
        let class = match elem {
            Elem::F32 => Class::Single,
            Elem::I32 => Class::Int32,
            Elem::I16 => Class::Int16,
            Elem::U16 => Class::UInt16,
            Elem::U8 => Class::UInt8,
            _ => Class::Double,
        };
        let mut v = Var {
            path: h.name.clone(),
            class,
            dims: vec![h.rows, h.cols],
            complex: h.imag,
            logical: false,
            global: false,
            class_name: None,
            kind: Kind::Empty,
            depth: 0,
        };
        v.kind = match h.ty {
            _ if n == 0 => Kind::Empty,
            1 => {
                v.class = Class::Char;
                let mut vals = Vec::new();
                let _ = crate::read_plane(
                    &data[re.range.clone()],
                    dtype,
                    &[n],
                    &[1],
                    &PlaneSpec {
                        x_dim: 0,
                        y_dim: None,
                        index: vec![0],
                    },
                    Part::Real,
                    &mut vals,
                );
                let units: Vec<u32> = vals.iter().map(|&c| c.max(0.0) as u32).collect();
                Kind::Text(rows_of(&units, &v.dims))
            }
            2 => v4_sparse(data, &h, &mut v, dtype)?,
            _ => Kind::Numeric { re, im },
        };
        mat.vars.push(v);
        if mat.vars.len() > MAX_VARS {
            return Err(Error::TooLarge);
        }
        p = h.data + len * (1 + usize::from(h.imag));
    }
    if mat.vars.is_empty() {
        return Err(Error::NotArray);
    }
    Ok(mat)
}

/// A level 4 sparse matrix: an (nnz + 1) × 3 (or 4, complex) matrix of 1-based row, column
/// and value(s); its last row gives the size.
fn v4_sparse(data: &[u8], h: &V4, v: &mut Var, dtype: Dtype) -> Result<Kind> {
    if h.cols < 3 || h.rows == 0 {
        return bad("MATLAB level 4 sparse matrix");
    }
    let n = h.rows * h.cols;
    let span = Span {
        buf: None,
        range: h.data..h.data + (n * dtype.size() as u64) as usize,
        dtype,
    };
    let col = |c: u64| -> Vec<f32> { triplet_column(data, &span, h.rows, h.cols, c) };
    let last = h.rows as usize - 1;
    let m = col(0)[last].max(0.0) as u64;
    let k = col(1)[last].max(0.0) as u64;
    v.dims = vec![m, k];
    v.complex = h.cols >= 4;
    v.class = Class::Double;
    Ok(Kind::Triplets {
        data: span,
        rows: h.rows,
        cols: h.cols,
    })
}

/// Column `c` of a level 4 sparse matrix's stored triplets.
fn triplet_column(data: &[u8], span: &Span, rows: u64, cols: u64, c: u64) -> Vec<f32> {
    let mut out = Vec::new();
    let _ = crate::read_plane(
        data.get(span.range.clone()).unwrap_or(&[]),
        span.dtype,
        &[rows, cols],
        &crate::strides(&[rows, cols], true),
        &PlaneSpec {
            x_dim: 0,
            y_dim: None,
            index: vec![0, c],
        },
        Part::Real,
        &mut out,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(ty: u32, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        while out.len() % 8 != 0 {
            out.push(0);
        }
        out
    }

    fn matrix(class: u8, flags: u32, dims: &[i32], name: &str, rest: &[u8]) -> Vec<u8> {
        let mut body = el(
            MI_UINT32,
            &[(flags | u32::from(class)).to_le_bytes(), [0; 4]].concat(),
        );
        body.extend(el(
            MI_INT32,
            &dims
                .iter()
                .flat_map(|d| d.to_le_bytes())
                .collect::<Vec<_>>(),
        ));
        body.extend(el(MI_INT8, name.as_bytes()));
        body.extend_from_slice(rest);
        el(MI_MATRIX, &body)
    }

    fn file(vars: &[Vec<u8>]) -> Vec<u8> {
        let mut h = b"MATLAB 5.0 MAT-file, Platform: test".to_vec();
        h.resize(116, b' ');
        h.extend_from_slice(&[0; 8]);
        h.extend_from_slice(&0x0100u16.to_le_bytes());
        h.extend_from_slice(b"IM");
        for v in vars {
            h.extend_from_slice(v);
        }
        h
    }

    /// A struct whose field length is its whole 64 KiB name block: that one name went into
    /// the path of every element (and of everything nested below them). Names are cut to
    /// MATLAB's length now.
    #[test]
    fn long_field_names_are_cut() {
        let a = matrix(6, 0, &[1, 1], "", &el(MI_INT8, &[1]));
        let mut srest = el(MI_INT32, &65536i32.to_le_bytes());
        srest.extend(el(MI_INT8, &vec![b'n'; 65536]));
        srest.extend(a);
        let s = matrix(2, 0, &[1, 1], "s", &srest);
        let m = parse(&file(&[s])).unwrap();
        assert_eq!(m.vars.len(), 2);
        assert_eq!(m.vars[1].path.len(), "s.".len() + MAX_FIELD_NAME);
    }

    #[test]
    fn reads_level5_numeric_struct_cell() {
        // x = uint8 2×3 [1 2 3; 4 5 6] (column-major 1 4 2 5 3 6), stored as miUINT8.
        let x = matrix(9, 0, &[2, 3], "x", &el(MI_UINT8, &[1, 4, 2, 5, 3, 6]));
        // s.a = double 1×1 stored as miINT8 (-2); s.t = 'hi'.
        let a = matrix(6, 0, &[1, 1], "", &el(MI_INT8, &[0xfe]));
        let t = matrix(4, 0, &[1, 2], "", &el(MI_UINT16, &[b'h', 0, b'i', 0]));
        let mut srest = el(MI_INT32, &4i32.to_le_bytes());
        srest.extend(el(MI_INT8, b"a\0\0\0t\0\0\0"));
        srest.extend(a);
        srest.extend(t);
        let s = matrix(2, 0, &[1, 1], "s", &srest);
        // c = {single 1×2 complex}.
        let mut z = el(
            MI_SINGLE,
            &[3f32.to_le_bytes(), 0f32.to_le_bytes()].concat(),
        );
        z.extend(el(
            MI_SINGLE,
            &[4f32.to_le_bytes(), 1f32.to_le_bytes()].concat(),
        ));
        let inner = matrix(7, 0x800, &[1, 2], "", &z);
        let c = matrix(1, 0, &[1, 1], "c", &inner);
        let f = file(&[x, s, c]);
        assert_eq!(version(&f), Some(Version::V5));
        let m = parse(&f).unwrap();
        let paths: Vec<&str> = m.vars.iter().map(|v| v.path.as_str()).collect();
        assert_eq!(paths, ["x", "s", "s.a", "s.t", "c", "c{1}"]);
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: Some(0),
            index: vec![0, 0],
        };
        let mut out = Vec::new();
        m.read_plane(&f, None, &m.vars[0], &spec, Part::Real, &mut out)
            .unwrap();
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        m.read_plane(&f, None, &m.vars[2], &spec, Part::Real, &mut out)
            .unwrap();
        assert_eq!(out, [-2.0]);
        assert_eq!(m.vars[3].kind, Kind::Text("hi".into()));
        m.read_plane(&f, None, &m.vars[5], &spec, Part::Magnitude, &mut out)
            .unwrap();
        assert_eq!(out, [5.0, 1.0]);
        assert_eq!(m.vars[5].describe(), "complex single");
    }

    #[test]
    fn reads_level5_sparse() {
        // 3×2 sparse: (1,1)=5, (3,2)=7.
        let mut rest = el(
            MI_INT32,
            &[0i32, 2]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        rest.extend(el(
            MI_INT32,
            &[0i32, 1, 2]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        ));
        rest.extend(el(
            MI_DOUBLE,
            &[5f64, 7.0]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        ));
        let sp = matrix(5, 0, &[3, 2], "sp", &rest);
        let f = file(&[sp]);
        let m = parse(&f).unwrap();
        let d = m.sparse_dense(&f, None, &m.vars[0], Part::Real).unwrap();
        assert_eq!(d, [5.0, 0.0, 0.0, 0.0, 0.0, 7.0]);
    }

    #[test]
    fn reads_level4() {
        // A 2×2 double "A" little-endian, then a 1×3 text "t".
        let mut f = Vec::new();
        for (ty, r, c, name, vals) in [
            (0i32, 2i32, 2i32, "A", vec![1f64, 3.0, 2.0, 4.0]),
            (1, 1, 3, "t", vec![b'a' as f64, b'b' as f64, b'c' as f64]),
        ] {
            for w in [ty, r, c, 0, name.len() as i32 + 1] {
                f.extend_from_slice(&w.to_le_bytes());
            }
            f.extend_from_slice(name.as_bytes());
            f.push(0);
            for v in vals {
                f.extend_from_slice(&v.to_le_bytes());
            }
        }
        assert_eq!(version(&f), Some(Version::V4));
        let m = parse(&f).unwrap();
        assert_eq!(m.vars.len(), 2);
        let mut out = Vec::new();
        m.read_plane(
            &f,
            None,
            &m.vars[0],
            &PlaneSpec {
                x_dim: 1,
                y_dim: Some(0),
                index: vec![0, 0],
            },
            Part::Real,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(m.vars[1].kind, Kind::Text("abc".into()));
        // Random bytes are not level 4.
        assert_eq!(version(&[7u8; 64]), None);
    }
}

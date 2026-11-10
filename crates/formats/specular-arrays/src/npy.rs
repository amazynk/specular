//! NumPy `.npy` files (format versions 1.0, 2.0 and 3.0): the magic, a little-endian header
//! length, a Python dict literal giving `descr`, `fortran_order` and `shape`, then the
//! elements. Booleans, integers, half/single/double floats, complex64/128 and datetime64 /
//! timedelta64 (as their 64-bit counts) read in either byte order and storage order; object,
//! string, structured and extended-precision arrays are refused with a reason.

use crate::{Dtype, Elem, Error, MAX_DIMS, Part, PlaneSpec, Result, bad, le_u16, le_u32};

pub const MAGIC: &[u8; 6] = b"\x93NUMPY";
/// Header bytes read at most (NumPy itself caps at 10 000 by default).
const MAX_HEADER: usize = 1 << 20;

/// Whether `head` starts with the `.npy` magic and a known version.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 8 && head.starts_with(MAGIC) && (1..=3).contains(&head[6])
}

/// A parsed `.npy` header.
#[derive(Clone, Debug, PartialEq)]
pub struct Npy {
    /// (major, minor).
    pub version: (u8, u8),
    /// The `descr` string as stored ("<f8").
    pub descr: String,
    pub dtype: Dtype,
    pub fortran: bool,
    /// Logical shape, slowest first; empty for a scalar.
    pub shape: Vec<u64>,
    /// Where the elements start.
    pub data_offset: usize,
    /// The datetime64 / timedelta64 unit ("ns", "D"), when the type is one.
    pub time_unit: Option<String>,
}

impl Npy {
    /// Elements in the array.
    pub fn count(&self) -> u64 {
        crate::count(&self.shape).unwrap_or(u64::MAX)
    }

    /// Bytes of element data the header promises.
    pub fn data_len(&self) -> u64 {
        self.count().saturating_mul(self.dtype.size() as u64)
    }

    pub fn strides(&self) -> Vec<u64> {
        crate::strides(&self.shape, self.fortran)
    }

    /// The type for the info panel: "64-bit float, little-endian, Fortran order".
    pub fn describe(&self) -> String {
        let mut s = match &self.time_unit {
            Some(u) => format!("datetime / timedelta [{u}] as 64-bit counts"),
            None => self.dtype.elem.describe().to_owned(),
        };
        if self.dtype.size() > 1 {
            s += if self.dtype.big {
                ", big-endian"
            } else {
                ", little-endian"
            };
        }
        if self.fortran && self.shape.len() > 1 {
            s += ", Fortran order";
        }
        s
    }

    /// Reads one plane from the whole file `data`.
    pub fn read_plane(
        &self,
        data: &[u8],
        spec: &PlaneSpec,
        part: Part,
        out: &mut Vec<f32>,
    ) -> Result<bool> {
        let body = data.get(self.data_offset..).unwrap_or(&[]);
        crate::read_plane(
            body,
            self.dtype,
            &self.shape,
            &self.strides(),
            spec,
            part,
            out,
        )
    }
}

/// Parses the header of a `.npy` file (or of its first bytes: the data is not needed).
pub fn parse(data: &[u8]) -> Result<Npy> {
    if !data.starts_with(MAGIC) {
        return Err(Error::NotArray);
    }
    let (major, minor) = match (data.get(6), data.get(7)) {
        (Some(&a), Some(&b)) => (a, b),
        _ => return Err(Error::Truncated("NumPy header".into())),
    };
    let (len, start) = match major {
        1 => (
            usize::from(le_u16(data, 8).ok_or_else(|| Error::Truncated("NumPy header".into()))?),
            10,
        ),
        2 | 3 => (
            le_u32(data, 8).ok_or_else(|| Error::Truncated("NumPy header".into()))? as usize,
            12,
        ),
        v => {
            return Err(Error::Unsupported(format!(
                "NumPy file format version {v} is not supported"
            )));
        }
    };
    if len > MAX_HEADER {
        return Err(Error::TooLarge);
    }
    let text = data
        .get(start..start + len)
        .ok_or_else(|| Error::Truncated("NumPy header".into()))?;
    // Version 1 and 2 headers are Latin-1, version 3 UTF-8; the keys and values that matter
    // are ASCII either way.
    let text: String = if major == 3 {
        String::from_utf8_lossy(text).into_owned()
    } else {
        text.iter().map(|&b| char::from(b)).collect()
    };
    let dict = match Literal::parse(&text)? {
        Literal::Dict(d) => d,
        _ => return bad("NumPy header is not a dict"),
    };
    let get = |k: &str| dict.iter().find(|(n, _)| n == k).map(|(_, v)| v);
    let descr = match get("descr") {
        Some(Literal::Str(s)) => s.clone(),
        Some(Literal::List(_)) => {
            return Err(Error::Unsupported(
                "NumPy structured (record) arrays are not supported".into(),
            ));
        }
        _ => return bad("NumPy header without descr"),
    };
    let fortran = match get("fortran_order") {
        Some(Literal::Bool(b)) => *b,
        None => false,
        _ => return bad("NumPy fortran_order"),
    };
    let shape = match get("shape") {
        Some(Literal::Tuple(t)) => t
            .iter()
            .map(|v| match v {
                Literal::Int(n) if *n >= 0 => Ok(*n as u64),
                _ => bad("NumPy shape"),
            })
            .collect::<Result<Vec<u64>>>()?,
        _ => return bad("NumPy header without shape"),
    };
    if shape.len() > MAX_DIMS {
        return Err(Error::Unsupported(format!(
            "arrays of {} dimensions are not supported (at most {MAX_DIMS})",
            shape.len()
        )));
    }
    let (dtype, time_unit) = parse_descr(&descr)?;
    if crate::count(&shape)
        .and_then(|n| n.checked_mul(dtype.size() as u64))
        .is_none()
    {
        return Err(Error::TooLarge);
    }
    Ok(Npy {
        version: (major, minor),
        descr,
        dtype,
        fortran,
        shape,
        data_offset: start + len,
        time_unit,
    })
}

/// A NumPy type string ("<f8", "|u1", ">c16", "<M8[ns]") as an element type and byte order,
/// with the time unit of datetime64 / timedelta64.
pub fn parse_descr(descr: &str) -> Result<(Dtype, Option<String>)> {
    let d = descr.trim();
    let (order, rest) = match d.chars().next() {
        Some(c @ ('<' | '>' | '|' | '=')) => (c, &d[1..]),
        _ => ('=', d),
    };
    let big = order == '>';
    let kind = rest.chars().next().unwrap_or(' ');
    let digits: String = rest[kind.len_utf8().min(rest.len())..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let size: usize = digits.parse().unwrap_or(0);
    let unit = rest.find('[').and_then(|a| {
        rest[a + 1..]
            .find(']')
            .map(|b| rest[a + 1..a + 1 + b].to_owned())
    });
    let refuse = |what: &str| Err(Error::Unsupported(what.to_owned()));
    let elem = match (kind, size) {
        ('b', 1) | ('?', _) => Elem::Bool,
        ('i', 1) => Elem::I8,
        ('i', 2) => Elem::I16,
        ('i', 4) => Elem::I32,
        ('i', 8) => Elem::I64,
        ('u', 1) => Elem::U8,
        ('u', 2) => Elem::U16,
        ('u', 4) => Elem::U32,
        ('u', 8) => Elem::U64,
        ('f', 2) => Elem::F16,
        ('f', 4) => Elem::F32,
        ('f', 8) => Elem::F64,
        ('c', 8) => Elem::C64,
        ('c', 16) => Elem::C128,
        ('M' | 'm', 8) => {
            return Ok((
                Dtype {
                    elem: Elem::I64,
                    big,
                },
                Some(unit.unwrap_or_default()),
            ));
        }
        ('f', _) | ('c', _) | ('g', _) => {
            return refuse("NumPy extended-precision (long double) arrays are not supported");
        }
        ('O', _) => {
            return refuse(
                "This array holds Python objects (pickled data), which cannot be read without running Python code",
            );
        }
        ('S' | 'a' | 'U', _) => return refuse("NumPy text (string) arrays are not images"),
        ('V', _) => return refuse("NumPy raw void (record) arrays are not supported"),
        _ => {
            return Err(Error::Unsupported(format!(
                "NumPy type {descr} is not supported"
            )));
        }
    };
    Ok((Dtype { elem, big }, None))
}

/// The subset of Python literals a `.npy` header uses.
#[derive(Clone, Debug, PartialEq)]
enum Literal {
    Str(String),
    Int(i64),
    Bool(bool),
    None,
    Tuple(Vec<Literal>),
    List(Vec<Literal>),
    Dict(Vec<(String, Literal)>),
}

impl Literal {
    fn parse(s: &str) -> Result<Literal> {
        let b = s.as_bytes();
        let mut p = 0;
        let v = value(b, &mut p, 0)?;
        Ok(v)
    }
}

fn skip_ws(b: &[u8], p: &mut usize) {
    while *p < b.len() && (b[*p].is_ascii_whitespace()) {
        *p += 1;
    }
}

fn value(b: &[u8], p: &mut usize, depth: usize) -> Result<Literal> {
    if depth > 16 {
        return bad("NumPy header nested too deeply");
    }
    skip_ws(b, p);
    let Some(&c) = b.get(*p) else {
        return bad("NumPy header ends early");
    };
    match c {
        b'\'' | b'"' => {
            let q = c;
            *p += 1;
            let mut out = Vec::new();
            while let Some(&ch) = b.get(*p) {
                *p += 1;
                if ch == b'\\' {
                    if let Some(&n) = b.get(*p) {
                        out.push(n);
                        *p += 1;
                    }
                } else if ch == q {
                    return Ok(Literal::Str(String::from_utf8_lossy(&out).into_owned()));
                } else {
                    out.push(ch);
                }
            }
            bad("NumPy header string not closed")
        }
        b'(' | b'[' | b'{' => {
            let close = match c {
                b'(' => b')',
                b'[' => b']',
                _ => b'}',
            };
            *p += 1;
            let mut items = Vec::new();
            let mut pairs = Vec::new();
            loop {
                skip_ws(b, p);
                match b.get(*p) {
                    Some(&x) if x == close => {
                        *p += 1;
                        break;
                    }
                    None => return bad("NumPy header bracket not closed"),
                    _ => {}
                }
                let v = value(b, p, depth + 1)?;
                skip_ws(b, p);
                if c == b'{' {
                    if b.get(*p) != Some(&b':') {
                        return bad("NumPy header dict");
                    }
                    *p += 1;
                    let val = value(b, p, depth + 1)?;
                    let key = match v {
                        Literal::Str(k) => k,
                        _ => return bad("NumPy header key"),
                    };
                    pairs.push((key, val));
                } else {
                    items.push(v);
                }
                skip_ws(b, p);
                if b.get(*p) == Some(&b',') {
                    *p += 1;
                }
            }
            Ok(match c {
                b'(' => Literal::Tuple(items),
                b'[' => Literal::List(items),
                _ => Literal::Dict(pairs),
            })
        }
        _ => {
            let s = *p;
            while *p < b.len() && (b[*p].is_ascii_alphanumeric() || b[*p] == b'-' || b[*p] == b'_')
            {
                *p += 1;
            }
            let word = std::str::from_utf8(&b[s..*p]).unwrap_or("");
            match word {
                "True" => Ok(Literal::Bool(true)),
                "False" => Ok(Literal::Bool(false)),
                "None" => Ok(Literal::None),
                // Python 2 wrote long integers with an L suffix.
                w => w
                    .trim_end_matches(['L', 'l'])
                    .parse::<i64>()
                    .map(Literal::Int)
                    .or_else(|_| bad(format!("NumPy header value {w:?}"))),
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `.npy` file with `header` (padded as NumPy pads it) and `body`.
    pub fn file(major: u8, header: &str, body: &[u8]) -> Vec<u8> {
        let pre = if major == 1 { 10 } else { 12 };
        let mut h = header.to_owned();
        while !(pre + h.len() + 1).is_multiple_of(64) {
            h.push(' ');
        }
        h.push('\n');
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&[major, 0]);
        if major == 1 {
            out.extend_from_slice(&(h.len() as u16).to_le_bytes());
        } else {
            out.extend_from_slice(&(h.len() as u32).to_le_bytes());
        }
        out.extend_from_slice(h.as_bytes());
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn parses_headers() {
        let f = file(
            1,
            "{'descr': '<f8', 'fortran_order': False, 'shape': (3, 4), }",
            &[],
        );
        let n = parse(&f).unwrap();
        assert_eq!(n.shape, [3, 4]);
        assert_eq!(n.dtype, Dtype::le(Elem::F64));
        assert!(!n.fortran);
        assert_eq!(n.data_offset % 64, 0);
        let f = file(
            3,
            "{'descr': '>M8[ns]', 'fortran_order': True, 'shape': (5,), }",
            &[],
        );
        let n = parse(&f).unwrap();
        assert_eq!(n.version, (3, 0));
        assert_eq!(n.time_unit.as_deref(), Some("ns"));
        assert!(n.dtype.big && n.fortran);
        let f = file(
            2,
            "{'descr': '|b1', 'fortran_order': False, 'shape': (), }",
            &[],
        );
        assert_eq!(parse(&f).unwrap().shape, Vec::<u64>::new());
    }

    #[test]
    fn refuses_with_reasons() {
        for (d, word) in [
            ("|O", "Python objects"),
            ("<U8", "text"),
            ("<f16", "extended"),
            ("[('a', '<i4')]", "structured"),
        ] {
            let q = if d.starts_with('[') {
                d.to_owned()
            } else {
                format!("'{d}'")
            };
            let f = file(
                1,
                &format!("{{'descr': {q}, 'fortran_order': False, 'shape': (2,), }}"),
                &[],
            );
            match parse(&f) {
                Err(Error::Unsupported(m)) => assert!(m.contains(word), "{d}: {m}"),
                r => panic!("{d}: {r:?}"),
            }
        }
    }

    #[test]
    fn reads_a_fortran_plane() {
        // 2 × 3 uint16 in Fortran order: rows [1, 2, 3], [4, 5, 6].
        let body: Vec<u8> = [1u16, 4, 2, 5, 3, 6]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let f = file(
            1,
            "{'descr': '<u2', 'fortran_order': True, 'shape': (2, 3), }",
            &body,
        );
        let n = parse(&f).unwrap();
        let mut out = Vec::new();
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: Some(0),
            index: vec![0, 0],
        };
        assert!(!n.read_plane(&f, &spec, Part::Real, &mut out).unwrap());
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }
}

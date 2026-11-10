//! Amira / Avizo `.am` lattices (AmiraMesh): `define Lattice` sizes, a `Parameters` block
//! (`BoundingBox`, `CoordType`, `Units`), and data sections `@1`, `@2`… after the header,
//! binary (big-endian `BINARY`, `BINARY-LITTLE-ENDIAN`) or `ASCII`, raw or compressed
//! (`HxByteRLE`, `HxZip`).
//!
//! The first lattice section of a supported type is read: `byte`, `sbyte`, `short`,
//! `ushort`, `int`, `uint`, `long`, `float`, `double`; `byte[3]` / `byte[4]` as colour, other
//! `T[n]` as n interleaved components shown as volumes. Voxel spacing comes from the
//! bounding box (its corners are the first and last voxel centres) in the stated
//! coordinate unit (millimetres when none is stated). Amira shows y upward: the stored
//! rows run upward here (y negated), so slices look as they do there.

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Param,
    ParamValue, Result, Skip,
};

/// Whether `data` starts like an AmiraMesh file.
pub fn sniff(data: &[u8]) -> bool {
    let first = first_line(data).to_ascii_lowercase();
    (first.starts_with("# amiramesh") || first.starts_with("# avizo"))
        && !first.contains("hypersurface")
}

fn first_line(data: &[u8]) -> String {
    let end = data
        .iter()
        .take(256)
        .position(|&c| c == b'\n')
        .unwrap_or(data.len().min(256));
    String::from_utf8_lossy(&data[..end]).into_owned()
}

/// A data section declared in the header.
#[derive(Clone, Debug)]
struct Section {
    /// `@n`.
    id: u32,
    dtype: Option<DataType>,
    /// Components per voxel (`T[n]`).
    comps: usize,
    /// `HxByteRLE` / `HxZip` with the encoded length.
    coding: Option<(Encoding, usize)>,
    elem: usize,
}

struct Parsed {
    binary: bool,
    big_endian: bool,
    size: [usize; 3],
    bbox: Option<[f64; 6]>,
    unit: f64,
    content: Option<String>,
    rectilinear: bool,
    sections: Vec<Section>,
    /// Where the first `@n` marker line starts.
    body: usize,
}

fn dtype(s: &str) -> Option<(DataType, usize)> {
    Some(match s {
        "byte" | "uchar" => (DataType::U8, 1),
        "sbyte" | "char" => (DataType::I8, 1),
        "short" => (DataType::I16, 2),
        "ushort" => (DataType::U16, 2),
        "int" => (DataType::I32, 4),
        "uint" => (DataType::U32, 4),
        "long" | "int64" => (DataType::I64, 8),
        "ulong" | "uint64" => (DataType::U64, 8),
        "float" => (DataType::F32, 4),
        "double" => (DataType::F64, 8),
        _ => return None,
    })
}

/// Millimetres per coordinate unit.
fn unit_mm(u: &str) -> Option<f64> {
    Some(
        match u.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => 1.0,
            "µm" | "μm" | "um" | "micron" | "microns" | "micrometer" | "micrometers"
            | "micrometre" | "micrometres" => 1e-3,
            "nm" | "nanometer" | "nanometers" => 1e-6,
            "cm" => 10.0,
            "m" | "meter" | "meters" => 1000.0,
            "pm" => 1e-9,
            "å" | "angstrom" | "angstroms" => 1e-7,
            _ => return None,
        },
    )
}

/// The marker line `@n` starting at `at`, if one does: (n, the line's end).
fn marker(data: &[u8], at: usize) -> Option<(u32, usize)> {
    let rest = data.get(at..)?;
    if rest.first() != Some(&b'@') {
        return None;
    }
    let digits = rest[1..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let n: u32 = std::str::from_utf8(&rest[1..1 + digits])
        .ok()?
        .parse()
        .ok()?;
    let mut end = 1 + digits;
    while rest
        .get(end)
        .is_some_and(|&c| c == b' ' || c == b'\t' || c == b'\r')
    {
        end += 1;
    }
    match rest.get(end) {
        Some(b'\n') => Some((n, at + end + 1)),
        None => Some((n, at + end)),
        _ => None,
    }
}

fn parse_text(data: &[u8]) -> Result<Parsed> {
    if !sniff(data) {
        return Err(Error::NotRecognised);
    }
    let first = first_line(data);
    let up = first.to_ascii_uppercase();
    let binary = up.contains("BINARY");
    let big_endian = binary && !up.contains("LITTLE-ENDIAN");
    if !binary && !up.contains("ASCII") {
        return Err(Error::Unsupported(format!("AmiraMesh coding {first:?}")));
    }
    // The header: lines up to the first `@n` line.
    let mut at = 0;
    let mut body = None;
    while at < data.len() {
        if marker(data, at).is_some() {
            body = Some(at);
            break;
        }
        match data[at..].iter().position(|&c| c == b'\n') {
            Some(p) => at += p + 1,
            None => break,
        }
    }
    let body = body.ok_or_else(|| Error::Truncated("AmiraMesh: no data section".into()))?;
    let text = String::from_utf8_lossy(&data[..body]);
    let mut size = None;
    let mut bbox = None;
    let mut unit = None;
    let mut content = None;
    let mut rectilinear = false;
    let mut sections = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("define lattice") {
            let n: Vec<usize> = rest
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if n.is_empty() || n.len() > 3 || n.contains(&0) {
                return Err(Error::Malformed(format!("AmiraMesh lattice {rest:?}")));
            }
            let mut s = [1; 3];
            s[..n.len()].copy_from_slice(&n);
            size = Some(s);
        } else if lower.starts_with("define ") && size.is_none() && !lower.contains("lattice") {
            // `define Nodes`, `define Vertices`… : not an image.
            continue;
        } else if let Some(rest) = lower.strip_prefix("boundingbox") {
            let v = crate::numbers(rest.trim_end_matches(','));
            if let Some(v) = v.filter(|v| v.len() == 6) {
                bbox = Some([v[0], v[1], v[2], v[3], v[4], v[5]]);
            }
        } else if lower.starts_with("coordtype") {
            rectilinear = lower.contains("rectilinear") || lower.contains("stacked");
        } else if lower.starts_with("coordinates") && unit.is_none() {
            // Inside `Units { Coordinates "µm" }`.
            let q = t.split('"').nth(1).unwrap_or("");
            unit = unit_mm(q);
        } else if lower.starts_with("content") && content.is_none() {
            content = t.split('"').nth(1).map(str::to_owned);
        } else if lower.starts_with("lattice") && lower.contains('{') {
            sections.push(section(t)?);
        }
    }
    let size = size.ok_or_else(|| Error::Unsupported("AmiraMesh file without a lattice".into()))?;
    Ok(Parsed {
        binary,
        big_endian,
        size,
        bbox,
        unit: unit.unwrap_or(1.0),
        content,
        rectilinear,
        sections,
        body,
    })
}

/// `Lattice { ushort Data } @1(HxByteRLE,1234)`.
fn section(t: &str) -> Result<Section> {
    let bad = || Error::Malformed(format!("AmiraMesh section {t:?}"));
    let open = t.find('{').ok_or_else(bad)?;
    let close = t.find('}').ok_or_else(bad)?;
    let decl = t.get(open + 1..close).ok_or_else(bad)?.trim();
    let ty = decl.split_whitespace().next().unwrap_or("");
    let (base, comps) = match ty.split_once('[') {
        Some((b, n)) => (
            b,
            n.trim_end_matches(']')
                .parse::<usize>()
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(bad)?,
        ),
        None => (ty, 1),
    };
    let rest = &t[close + 1..];
    let at = rest.find('@').ok_or_else(bad)?;
    let digits: String = rest[at + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let id: u32 = digits.parse().map_err(|_| bad())?;
    let after = &rest[at + 1 + digits.len()..];
    let coding = match after.trim().strip_prefix('(') {
        Some(c) => {
            let c = c.trim_end_matches(')');
            let (name, len) = c.split_once(',').ok_or_else(bad)?;
            let len: usize = len.trim().parse().map_err(|_| bad())?;
            let enc = match name.trim().to_ascii_lowercase().as_str() {
                "hxbyterle" => Encoding::ByteRle,
                "hxzip" => Encoding::Zlib,
                n => {
                    return Err(Error::Unsupported(format!("AmiraMesh compression {n}")));
                }
            };
            Some((enc, len))
        }
        None => None,
    };
    let d = dtype(&base.to_ascii_lowercase());
    Ok(Section {
        id,
        dtype: d.map(|x| x.0),
        comps,
        coding,
        elem: d.map_or(0, |x| x.1 * comps),
    })
}

/// The section read: the first of a supported type.
fn chosen(p: &Parsed) -> Result<&Section> {
    p.sections
        .iter()
        .find(|s| s.dtype.is_some())
        .ok_or_else(|| match p.sections.first() {
            Some(_) => Error::Unsupported("AmiraMesh data of an unsupported type".into()),
            None => Error::Malformed("AmiraMesh: no lattice data".into()),
        })
}

/// Where the chosen section's data starts in `file`.
pub(crate) fn data_start(file: &[u8]) -> Result<usize> {
    let p = parse_text(file)?;
    let want = chosen(&p)?.id;
    let voxels: usize = p.size.iter().product();
    let mut at = p.body;
    loop {
        let (n, start) =
            marker(file, at).ok_or_else(|| Error::Truncated(format!("AmiraMesh @{want}")))?;
        if n == want {
            return Ok(start);
        }
        // Skip this section's data to the next marker.
        let s = p.sections.iter().find(|s| s.id == n);
        let next = match (p.binary, s) {
            (true, Some(s)) if s.elem > 0 || s.coding.is_some() => {
                let len = match s.coding {
                    Some((_, len)) => len,
                    None => voxels.checked_mul(s.elem).ok_or(Error::TooLarge)?,
                };
                start.checked_add(len).ok_or(Error::TooLarge)?
            }
            _ => {
                // ASCII (or an unknown binary type): look for the next marker line.
                let mut q = start;
                loop {
                    let nl = file[q..]
                        .iter()
                        .position(|&c| c == b'\n')
                        .ok_or_else(|| Error::Truncated(format!("AmiraMesh @{want}")))?;
                    q += nl + 1;
                    if marker(file, q).is_some() {
                        break q;
                    }
                }
            }
        };
        // Blank lines between sections.
        at = next;
        while file.get(at).is_some_and(|c| c.is_ascii_whitespace()) {
            at += 1;
        }
    }
}

pub fn parse(data: &[u8]) -> Result<Header> {
    let p = parse_text(data)?;
    let s = chosen(&p)?.clone();
    let dtype = s.dtype.unwrap_or(DataType::U8);
    let colour = dtype == DataType::U8 && matches!(s.comps, 3 | 4);
    let mut layout = if colour {
        let mut l = Layout::dense(p.size, &[], s.comps).ok_or(Error::TooLarge)?;
        l.colour = Some((s.comps, 1));
        l
    } else if s.comps > 1 {
        // Interleaved components, each shown as a volume.
        let mut l = Layout::dense(p.size, &[], s.comps).ok_or(Error::TooLarge)?;
        l.extra = vec![(s.comps, 1)];
        l
    } else {
        Layout::dense(p.size, &[], 1).ok_or(Error::TooLarge)?
    };
    layout.samples = p
        .size
        .iter()
        .product::<usize>()
        .checked_mul(s.comps)
        .ok_or(Error::TooLarge)?;
    let mut spacing = [1.0; 3];
    let mut origin = [0.0; 3];
    if let Some(b) = p.bbox {
        for a in 0..3 {
            let (lo, hi) = (b[2 * a], b[2 * a + 1]);
            if lo.is_finite() && hi.is_finite() {
                origin[a] = lo * p.unit;
                if p.size[a] > 1 && hi > lo {
                    spacing[a] = (hi - lo) / (p.size[a] - 1) as f64 * p.unit;
                }
            }
        }
        // A single slice: its thickness is unknown; keep it like its neighbours'.
        if p.size[2] == 1 {
            spacing[2] = spacing[0];
        }
    }
    let affine = [
        [spacing[0], 0.0, 0.0, origin[0]],
        [0.0, -spacing[1], 0.0, -origin[1]],
        [0.0, 0.0, spacing[2], origin[2]],
    ];
    let encoding = match (p.binary, s.coding) {
        (false, _) => Encoding::Ascii,
        (true, Some((e, _))) => e,
        (true, None) => Encoding::Raw,
    };
    let mut params = Vec::new();
    if p.rectilinear {
        params.push(Param {
            key: "coords",
            value: ParamValue::Text("rectilinear".into()),
        });
    }
    let mut dims = p.size.to_vec();
    if s.comps > 1 {
        dims.push(s.comps);
    }
    Ok(Header {
        kind: Kind::Amira,
        dtype,
        layout,
        affine,
        orientation: OrientationSource::Object,
        dims,
        spacing,
        scale: None,
        cal: None,
        description: p.content,
        intent: None,
        anatomical: None,
        params,
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start { lines: 0, bytes: 0 },
            encoding,
            big_endian: p.big_endian,
        },
    })
}

/// Decodes Amira's byte run-length coding to `need` bytes: a control byte with the high
/// bit set is followed by that many (low 7 bits) literal bytes; otherwise the next byte is
/// repeated that many times. Comes with its reservation from the memory budget.
pub(crate) fn byte_rle(src: &[u8], need: usize) -> Result<(Vec<u8>, specular_core::Reservation)> {
    // Two bytes give at most 127: a larger volume is not in the data, and nothing is
    // reserved for it.
    if need > src.len().div_ceil(2).saturating_mul(127) {
        return Err(Error::Truncated("HxByteRLE data".into()));
    }
    let (mut out, res) = crate::data::alloc(need)?;
    let mut i = 0;
    while out.len() < need {
        let &c = src
            .get(i)
            .ok_or_else(|| Error::Truncated("HxByteRLE data".into()))?;
        i += 1;
        let n = (c & 0x7f) as usize;
        if c & 0x80 != 0 {
            let lit = src
                .get(i..i + n)
                .ok_or_else(|| Error::Truncated("HxByteRLE data".into()))?;
            out.extend_from_slice(&lit[..n.min(need - out.len())]);
            i += n;
        } else {
            let &b = src
                .get(i)
                .ok_or_else(|| Error::Truncated("HxByteRLE data".into()))?;
            i += 1;
            out.resize((out.len() + n).min(need), b);
        }
        if n == 0 && c & 0x80 == 0 {
            return Err(Error::Malformed("HxByteRLE: empty run".into()));
        }
    }
    Ok((out, res))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_length() {
        let src = [3, 7, 0x82, 1, 2, 2, 9];
        assert_eq!(byte_rle(&src, 7).unwrap().0, [7, 7, 7, 1, 2, 9, 9]);
        assert!(byte_rle(&src, 9).is_err());
        assert!(byte_rle(&[0, 1], 1).is_err());
        // More than two bytes can expand to: refused before anything is reserved.
        assert!(matches!(byte_rle(&[127, 7], 128), Err(Error::Truncated(_))));
        assert_eq!(byte_rle(&[127, 7], 127).unwrap().0.len(), 127);
    }

    #[test]
    fn sections() {
        let s = section("Lattice { byte[3] Data } @2(HxByteRLE,100)").unwrap();
        assert_eq!((s.id, s.comps, s.elem), (2, 3, 3));
        assert_eq!(s.coding, Some((Encoding::ByteRle, 100)));
        let s = section("Lattice { float Data } @1").unwrap();
        assert_eq!((s.dtype, s.coding), (Some(DataType::F32), None));
        assert!(section("Lattice { short Data } @1(Bzip,3)").is_err());
    }
}

//! PCD (the Point Cloud Library's format, v0.5–0.7): a text header naming the fields, then
//! the points as ASCII lines, packed binary records, or LZF-compressed field-by-field
//! columns (`binary_compressed`). x, y, z, rgb/rgba (packed in a float or integer),
//! intensity and label (as the class) are read; other fields are skipped.

use crate::build::Collector;
use crate::{Channels, CloudInfo, Error, Limits, PointCloud, PointFormat, Result, malformed};

/// Header lines read at most before `DATA`.
const MAX_HEADER: usize = 64 * 1024;

pub(crate) fn sniff(head: &[u8]) -> bool {
    let h = &head[..head.len().min(2048)];
    let has = |k: &[u8]| h.windows(k.len()).any(|w| w == k);
    (h.starts_with(b"# .PCD") || h.starts_with(b"VERSION") || h.starts_with(b"FIELDS"))
        && has(b"FIELDS")
        && has(b"DATA")
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ty {
    I,
    U,
    F,
}

#[derive(Clone, Debug)]
struct Field {
    name: String,
    size: usize,
    ty: Ty,
    count: usize,
    /// Byte offset in a packed record.
    offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Data {
    Ascii,
    Binary,
    Compressed,
}

struct Header {
    fields: Vec<Field>,
    points: u64,
    data: Data,
    version: String,
    /// Where the data begins.
    body: usize,
    record: usize,
}

fn header(b: &[u8]) -> Result<Header> {
    let mut names: Vec<String> = Vec::new();
    let mut sizes: Vec<usize> = Vec::new();
    let mut types: Vec<Ty> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    let (mut width, mut height, mut points) = (None::<u64>, None::<u64>, None::<u64>);
    let mut version = String::from("0.7");
    let mut pos = 0;
    loop {
        if pos >= b.len() || pos > MAX_HEADER {
            return malformed("PCD header without DATA");
        }
        let end = b[pos..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(b.len(), |e| pos + e);
        let line = String::from_utf8_lossy(&b[pos..end]);
        pos = (end + 1).min(b.len());
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let key = it.next().unwrap_or("").to_ascii_uppercase();
        let rest: Vec<&str> = it.collect();
        let num = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| Error::Malformed(format!("PCD {key} {s}")))
        };
        match key.as_str() {
            "VERSION" => {
                version = rest
                    .first()
                    .unwrap_or(&"0.7")
                    .trim_start_matches('.')
                    .to_string()
            }
            "FIELDS" | "COLUMNS" => names = rest.iter().map(|s| s.to_ascii_lowercase()).collect(),
            "SIZE" => {
                sizes = rest
                    .iter()
                    .map(|s| num(s).map(|v| v as usize))
                    .collect::<Result<_>>()?
            }
            "TYPE" => {
                types = rest
                    .iter()
                    .map(|s| match *s {
                        "I" | "i" => Ok(Ty::I),
                        "U" | "u" => Ok(Ty::U),
                        "F" | "f" => Ok(Ty::F),
                        t => malformed(format!("PCD type {t}")),
                    })
                    .collect::<Result<_>>()?
            }
            "COUNT" => {
                counts = rest
                    .iter()
                    .map(|s| num(s).map(|v| v as usize))
                    .collect::<Result<_>>()?
            }
            "WIDTH" => width = rest.first().map(|s| num(s)).transpose()?,
            "HEIGHT" => height = rest.first().map(|s| num(s)).transpose()?,
            "POINTS" => points = rest.first().map(|s| num(s)).transpose()?,
            "VIEWPOINT" => {}
            "DATA" => {
                let data = match rest.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                    Some("ascii") => Data::Ascii,
                    Some("binary") => Data::Binary,
                    Some("binary_compressed") => Data::Compressed,
                    d => return Err(Error::Unsupported(format!("PCD data {d:?}"))),
                };
                if names.is_empty() {
                    return malformed("PCD without FIELDS");
                }
                let n = names.len();
                if counts.is_empty() {
                    counts = vec![1; n];
                }
                if sizes.len() != n || types.len() != n || counts.len() != n {
                    return malformed("PCD FIELDS, SIZE, TYPE and COUNT differ in length");
                }
                let mut fields = Vec::with_capacity(n);
                let mut offset = 0usize;
                for i in 0..n {
                    let (size, ty, count) = (sizes[i], types[i], counts[i]);
                    let ok = match ty {
                        Ty::F => matches!(size, 4 | 8),
                        _ => matches!(size, 1 | 2 | 4 | 8),
                    };
                    // A field of no values would be read past its record's end.
                    if !ok || count == 0 || count > 1 << 20 {
                        return malformed(format!("PCD field of {size} bytes ×{count}"));
                    }
                    fields.push(Field {
                        name: names[i].clone(),
                        size,
                        ty,
                        count,
                        offset,
                    });
                    offset += size * count;
                }
                let points = points
                    .or_else(|| width?.checked_mul(height.unwrap_or(1)))
                    .ok_or_else(|| Error::Malformed("PCD without POINTS".into()))?;
                return Ok(Header {
                    fields,
                    points,
                    data,
                    version,
                    body: pos,
                    record: offset.max(1),
                });
            }
            _ => {}
        }
    }
}

#[inline]
fn value(b: &[u8], f: &Field) -> f64 {
    match (f.ty, f.size) {
        (Ty::F, 4) => f64::from(f32::from_le_bytes(b[..4].try_into().expect("4"))),
        (Ty::F, _) => f64::from_le_bytes(b[..8].try_into().expect("8")),
        (Ty::U, 1) => f64::from(b[0]),
        (Ty::U, 2) => f64::from(u16::from_le_bytes([b[0], b[1]])),
        (Ty::U, 4) => f64::from(u32::from_le_bytes(b[..4].try_into().expect("4"))),
        (Ty::U, _) => u64::from_le_bytes(b[..8].try_into().expect("8")) as f64,
        (Ty::I, 1) => f64::from(b[0] as i8),
        (Ty::I, 2) => f64::from(i16::from_le_bytes([b[0], b[1]])),
        (Ty::I, 4) => f64::from(i32::from_le_bytes(b[..4].try_into().expect("4"))),
        (Ty::I, _) => i64::from_le_bytes(b[..8].try_into().expect("8")) as f64,
    }
}

/// A packed colour's bits (0x00RRGGBB) from a field's bytes.
#[inline]
fn colour_bits(b: &[u8], f: &Field) -> u32 {
    if f.size >= 4 {
        u32::from_le_bytes(b[..4].try_into().expect("4"))
    } else {
        value(b, f) as u32
    }
}

/// What each point needs: the fields' indices.
struct Pick {
    xyz: [usize; 3],
    rgb: Option<usize>,
    intensity: Option<usize>,
    label: Option<usize>,
}

fn pick(fields: &[Field]) -> Result<Pick> {
    let find = |names: &[&str]| fields.iter().position(|f| names.contains(&f.name.as_str()));
    let (Some(x), Some(y), Some(z)) = (find(&["x"]), find(&["y"]), find(&["z"])) else {
        return Err(Error::Unsupported("PCD without x, y and z".into()));
    };
    Ok(Pick {
        xyz: [x, y, z],
        rgb: find(&["rgb", "rgba"]),
        intensity: find(&["intensity", "i"]),
        label: find(&["label", "classification"]),
    })
}

/// A field's intensity as 16 bits (floats in 0‥1 scaled up; other floats and integers as
/// they are, clamped).
#[inline]
fn intensity16(v: f64, f: &Field) -> u16 {
    if f.ty == Ty::F && v <= 1.0 {
        (v.max(0.0) * 65_535.0) as u16
    } else {
        v.clamp(0.0, 65_535.0) as u16
    }
}

pub(crate) fn read(b: &[u8], limits: &Limits) -> Result<PointCloud> {
    let h = header(b)?;
    let pk = pick(&h.fields)?;
    if h.points > limits.max_declared {
        return Err(Error::TooLarge(format!("{} points", h.points)));
    }
    let f = &h.fields;
    let mut info = CloudInfo {
        format: Some(PointFormat::Pcd),
        version: Some(h.version.clone()),
        coding: Some(
            match h.data {
                Data::Ascii => "ascii",
                Data::Binary => "binary",
                Data::Compressed => "binary_compressed",
            }
            .into(),
        ),
        channels: Channels {
            rgb: pk.rgb.is_some(),
            intensity: pk.intensity.is_some(),
            classification: pk.label.is_some(),
            returns: false,
        },
        ..CloudInfo::default()
    };
    let body = &b[h.body..];
    let mut c = Collector::new(limits.max_points, h.points);
    let mut put = |get: &dyn Fn(usize) -> f64, bits: &dyn Fn(usize) -> u32| {
        let p = [get(pk.xyz[0]), get(pk.xyz[1]), get(pk.xyz[2])];
        let rgb = pk.rgb.map_or([0; 3], |i| {
            let v = bits(i);
            [(v >> 16) as u8, (v >> 8) as u8, v as u8]
        });
        let intensity = pk.intensity.map_or(0, |i| intensity16(get(i), &f[i]));
        let class = pk.label.map_or(0, |i| get(i).clamp(0.0, 255.0) as u8);
        c.push(p, rgb, class, intensity, 0);
    };
    match h.data {
        Data::Binary => {
            let n = (body.len() / h.record).min(usize::try_from(h.points).unwrap_or(usize::MAX));
            for r in body.chunks_exact(h.record).take(n) {
                put(&|i| value(&r[f[i].offset..], &f[i]), &|i| {
                    colour_bits(&r[f[i].offset..], &f[i])
                });
            }
        }
        Data::Compressed => {
            if body.len() < 8 {
                return malformed("PCD compressed data truncated");
            }
            let packed = u32::from_le_bytes(body[..4].try_into().expect("4")) as usize;
            let size = u32::from_le_bytes(body[4..8].try_into().expect("4")) as usize;
            let Some(src) = body.get(8..8 + packed) else {
                return malformed("PCD compressed data truncated");
            };
            let n = usize::try_from(h.points).unwrap_or(usize::MAX);
            if n.checked_mul(h.record) != Some(size) {
                return malformed(format!(
                    "PCD: {size} bytes uncompressed for {n} points of {} bytes",
                    h.record
                ));
            }
            // LZF expands at most 88× (a 3-byte back reference copies 264 bytes); anything
            // claiming more is corrupt, and would be allocated before any data proves it.
            if size > packed.saturating_mul(MAX_LZF_RATIO).max(1024) {
                return malformed("PCD compressed size implausible");
            }
            let raw = lzf(src, size)?;
            // Columns: field by field, each for all points.
            let mut col = Vec::with_capacity(f.len());
            let mut off = 0;
            for fd in f {
                col.push(off);
                off += fd.size * fd.count * n;
            }
            for k in 0..n {
                let at = |i: usize| col[i] + k * f[i].size * f[i].count;
                put(&|i| value(&raw[at(i)..], &f[i]), &|i| {
                    colour_bits(&raw[at(i)..], &f[i])
                });
            }
        }
        Data::Ascii => {
            let per: Vec<usize> = f
                .iter()
                .scan(0, |s, fd| {
                    let o = *s;
                    *s += fd.count;
                    Some(o)
                })
                .collect();
            let mut toks: Vec<&str> = Vec::new();
            let text = String::from_utf8_lossy(body);
            let mut rows = 0u64;
            for line in text.lines() {
                if rows >= h.points {
                    break;
                }
                toks.clear();
                toks.extend(line.split_whitespace());
                if toks.is_empty() {
                    continue;
                }
                let tok = |i: usize| toks.get(per[i]).copied().unwrap_or("nan");
                put(&|i| tok(i).parse::<f64>().unwrap_or(f64::NAN), &|i| {
                    let t = tok(i);
                    if f[i].ty == Ty::F {
                        t.parse::<f32>().map(f32::to_bits).unwrap_or(0)
                    } else {
                        t.parse::<f64>().map(|v| v as u32).unwrap_or(0)
                    }
                });
                rows += 1;
            }
        }
    }
    info.total = h.points;
    c.finish(info)
}

/// The most LZF data can expand: 264 bytes from a 3-byte back reference.
const MAX_LZF_RATIO: usize = 88;

/// LZF decompression into exactly `size` bytes.
pub(crate) fn lzf(src: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(size);
    let mut i = 0;
    let bad = || Error::Malformed("PCD: corrupt LZF data".into());
    while i < src.len() {
        let ctrl = usize::from(src[i]);
        i += 1;
        if ctrl < 32 {
            let n = ctrl + 1;
            let lit = src.get(i..i + n).ok_or_else(bad)?;
            if out.len() + n > size {
                return Err(bad());
            }
            out.extend_from_slice(lit);
            i += n;
        } else {
            let mut n = ctrl >> 5;
            if n == 7 {
                n += usize::from(*src.get(i).ok_or_else(bad)?);
                i += 1;
            }
            n += 2;
            let back = ((ctrl & 0x1F) << 8) + usize::from(*src.get(i).ok_or_else(bad)?) + 1;
            i += 1;
            if back > out.len() || out.len() + n > size {
                return Err(bad());
            }
            let from = out.len() - back;
            for k in 0..n {
                let v = out[from + k];
                out.push(v);
            }
        }
    }
    if out.len() != size {
        return Err(bad());
    }
    Ok(out)
}

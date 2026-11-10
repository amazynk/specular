//! PLY files without faces: the `vertex` element's x, y, z, colour (red/green/blue, r/g/b or
//! diffuse_*), intensity and classification (also as `scalar_*`, as CloudCompare writes
//! them). ASCII and binary, either byte order; other elements are skipped.

use crate::build::Collector;
use crate::{Channels, CloudInfo, Error, Limits, PointCloud, PointFormat, Result, malformed};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Kind {
    fn of(s: &str) -> Option<Kind> {
        Some(match s {
            "char" | "int8" => Kind::I8,
            "uchar" | "uint8" => Kind::U8,
            "short" | "int16" => Kind::I16,
            "ushort" | "uint16" => Kind::U16,
            "int" | "int32" => Kind::I32,
            "uint" | "uint32" => Kind::U32,
            "float" | "float32" => Kind::F32,
            "double" | "float64" => Kind::F64,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Kind::I8 | Kind::U8 => 1,
            Kind::I16 | Kind::U16 => 2,
            Kind::I32 | Kind::U32 | Kind::F32 => 4,
            Kind::F64 => 8,
        }
    }

    fn read(self, b: &[u8], le: bool) -> f64 {
        macro_rules! get {
            ($t:ty, $n:expr) => {{
                let a: [u8; $n] = b[..$n].try_into().expect("size");
                if le {
                    <$t>::from_le_bytes(a)
                } else {
                    <$t>::from_be_bytes(a)
                }
            }};
        }
        match self {
            Kind::I8 => f64::from(b[0] as i8),
            Kind::U8 => f64::from(b[0]),
            Kind::I16 => f64::from(get!(i16, 2)),
            Kind::U16 => f64::from(get!(u16, 2)),
            Kind::I32 => f64::from(get!(i32, 4)),
            Kind::U32 => f64::from(get!(u32, 4)),
            Kind::F32 => f64::from(get!(f32, 4)),
            Kind::F64 => get!(f64, 8),
        }
    }
}

#[derive(Clone, Debug)]
struct Prop {
    name: String,
    kind: Kind,
    list: Option<Kind>,
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    count: u64,
    props: Vec<Prop>,
}

#[derive(Clone, Copy, PartialEq)]
enum Enc {
    Ascii,
    Le,
    Be,
}

fn header(b: &[u8]) -> Result<(Enc, Vec<Element>, usize)> {
    if !b.starts_with(b"ply") {
        return malformed("not a PLY file");
    }
    let end = b
        .windows(10)
        .take(1 << 16)
        .position(|w| w == b"end_header")
        .ok_or_else(|| Error::Malformed("PLY without end_header".into()))?;
    let mut body = end + 10;
    if b.get(body) == Some(&b'\r') {
        body += 1;
    }
    if b.get(body) == Some(&b'\n') {
        body += 1;
    }
    let text = String::from_utf8_lossy(&b[..end]);
    let mut enc = None;
    let mut els: Vec<Element> = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.as_slice() {
            ["format", f, ..] => {
                enc = Some(match *f {
                    "ascii" => Enc::Ascii,
                    "binary_little_endian" => Enc::Le,
                    "binary_big_endian" => Enc::Be,
                    _ => return Err(Error::Unsupported(format!("PLY format {f}"))),
                })
            }
            ["element", name, count] => els.push(Element {
                name: (*name).to_string(),
                count: count
                    .parse()
                    .map_err(|_| Error::Malformed(format!("PLY element count {count}")))?,
                props: Vec::new(),
            }),
            ["property", "list", c, k, name] => {
                let (Some(c), Some(k)) = (Kind::of(c), Kind::of(k)) else {
                    return malformed("PLY list property type");
                };
                let Some(e) = els.last_mut() else {
                    return malformed("PLY property before element");
                };
                e.props.push(Prop {
                    name: (*name).to_string(),
                    kind: k,
                    list: Some(c),
                });
            }
            ["property", k, name] => {
                let Some(k) = Kind::of(k) else {
                    return malformed(format!("PLY property type {k}"));
                };
                let Some(e) = els.last_mut() else {
                    return malformed("PLY property before element");
                };
                e.props.push(Prop {
                    name: (*name).to_string(),
                    kind: k,
                    list: None,
                });
            }
            _ => {}
        }
    }
    let enc = enc.ok_or_else(|| Error::Malformed("PLY without format".into()))?;
    Ok((enc, els, body))
}

/// Vertices and no faces (nor strips, nor edges alone).
pub(crate) fn is_points(b: &[u8]) -> bool {
    let Ok((_, els, _)) = header(b) else {
        return false;
    };
    let has = |n: &str| els.iter().any(|e| e.name == n && e.count > 0);
    has("vertex") && !has("face") && !has("tristrips")
}

/// Column indices of what is read.
struct Pick {
    xyz: [usize; 3],
    rgb: Option<[usize; 3]>,
    intensity: Option<usize>,
    class: Option<usize>,
}

fn pick(e: &Element) -> Result<Pick> {
    let find = |names: &[&str]| {
        e.props
            .iter()
            .position(|p| p.list.is_none() && names.contains(&p.name.as_str()))
    };
    let (Some(x), Some(y), Some(z)) = (find(&["x"]), find(&["y"]), find(&["z"])) else {
        return Err(Error::Unsupported("PLY vertices without x, y and z".into()));
    };
    let rgb = match (
        find(&["red", "r", "diffuse_red"]),
        find(&["green", "g", "diffuse_green"]),
        find(&["blue", "b", "diffuse_blue"]),
    ) {
        (Some(r), Some(g), Some(b)) => Some([r, g, b]),
        _ => None,
    };
    Ok(Pick {
        xyz: [x, y, z],
        rgb,
        intensity: find(&["intensity", "scalar_intensity", "scalar_Intensity"]),
        class: find(&[
            "classification",
            "scalar_classification",
            "scalar_Classification",
            "label",
        ]),
    })
}

fn colour8(v: f64, k: Kind) -> u8 {
    match k {
        Kind::F32 | Kind::F64 => (v * 255.0).round().clamp(0.0, 255.0) as u8,
        Kind::U16 | Kind::I16 => (v / 257.0).round().clamp(0.0, 255.0) as u8,
        _ => v.clamp(0.0, 255.0) as u8,
    }
}

fn intensity16(v: f64, k: Kind) -> u16 {
    match k {
        Kind::F32 | Kind::F64 if v <= 1.0 => (v.max(0.0) * 65_535.0) as u16,
        Kind::U8 => (v * 257.0) as u16,
        _ => v.clamp(0.0, 65_535.0) as u16,
    }
}

pub(crate) fn read(b: &[u8], limits: &Limits) -> Result<PointCloud> {
    let (enc, els, body) = header(b)?;
    let total: u64 = els
        .iter()
        .find(|e| e.name == "vertex")
        .map_or(0, |e| e.count);
    if total > limits.max_declared {
        return Err(Error::TooLarge(format!("{total} vertices")));
    }
    let mut c = Collector::new(limits.max_points, total);
    let mut channels = Channels::default();
    let mut pos = body;
    let ascii: &[u8] = if enc == Enc::Ascii { &b[body..] } else { &[] };
    let mut tokens = ascii
        .split(u8::is_ascii_whitespace)
        .filter(|t| !t.is_empty());
    let le = enc == Enc::Le;
    let mut vals: Vec<f64> = Vec::new();
    for e in &els {
        let is_vertex = e.name == "vertex";
        let pk = if is_vertex { Some(pick(e)?) } else { None };
        if let Some(pk) = &pk {
            channels.rgb = pk.rgb.is_some();
            channels.intensity = pk.intensity.is_some();
            channels.classification = pk.class.is_some();
        }
        // An element without properties takes no bytes: its count bounds nothing.
        let count = if e.props.is_empty() { 0 } else { e.count };
        for _ in 0..count {
            vals.clear();
            for p in &e.props {
                if enc == Enc::Ascii {
                    let mut next = || -> Result<f64> {
                        tokens
                            .next()
                            .and_then(|t| std::str::from_utf8(t).ok()?.parse::<f64>().ok())
                            .ok_or_else(|| Error::Malformed("PLY ASCII data ended early".into()))
                    };
                    match p.list {
                        Some(_) => {
                            let n = next()? as usize;
                            for _ in 0..n {
                                next()?;
                            }
                            vals.push(0.0);
                        }
                        None => vals.push(next()?),
                    }
                } else {
                    match p.list {
                        Some(ck) => {
                            let Some(s) = b.get(pos..pos.saturating_add(ck.size())) else {
                                return malformed("PLY data ended early");
                            };
                            let n = ck.read(s, le).max(0.0) as usize;
                            // A list length near 2⁶⁴ must not overflow the position.
                            pos = (pos + ck.size()).saturating_add(n.saturating_mul(p.kind.size()));
                            vals.push(0.0);
                        }
                        None => {
                            let Some(s) = b.get(pos..pos.saturating_add(p.kind.size())) else {
                                return malformed("PLY data ended early");
                            };
                            vals.push(p.kind.read(s, le));
                            pos += p.kind.size();
                        }
                    }
                }
            }
            if let Some(pk) = &pk {
                let rgb = pk.rgb.map_or([0; 3], |[r, g, bl]| {
                    [
                        colour8(vals[r], e.props[r].kind),
                        colour8(vals[g], e.props[g].kind),
                        colour8(vals[bl], e.props[bl].kind),
                    ]
                });
                let i = pk
                    .intensity
                    .map_or(0, |i| intensity16(vals[i], e.props[i].kind));
                let class = pk.class.map_or(0, |i| vals[i].clamp(0.0, 255.0) as u8);
                c.push(
                    [vals[pk.xyz[0]], vals[pk.xyz[1]], vals[pk.xyz[2]]],
                    rgb,
                    class,
                    i,
                    0,
                );
            }
        }
        if is_vertex {
            // Faces and the rest are not needed.
            break;
        }
    }
    let info = CloudInfo {
        format: Some(PointFormat::Ply),
        coding: Some(
            match enc {
                Enc::Ascii => "ascii",
                Enc::Le => "binary little-endian",
                Enc::Be => "binary big-endian",
            }
            .into(),
        ),
        total,
        channels,
        ..CloudInfo::default()
    };
    c.finish(info)
}

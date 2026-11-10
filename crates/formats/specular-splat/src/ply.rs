//! Splat PLY files. The 3DGS layout: a `vertex` element with x, y, z, the DC colour
//! `f_dc_0..2` (or 8-bit red, green, blue), the higher harmonics `f_rest_*` (channel-major:
//! all of red's, then green's, then blue's), `opacity` (a logit), `scale_0..2` (logs) and
//! `rot_0..3` (w, x, y, z, unnormalised). SuperSplat's compressed layout: a `chunk` element
//! (per 256 splats: position and log-scale ranges, optionally colour ranges), `vertex` with
//! `packed_position`, `packed_rotation`, `packed_scale` and `packed_color`, and an optional
//! `sh` element of 8-bit `f_rest_*`. ASCII and binary, either byte order.

use crate::half::f32_to_f16;
use crate::math::{SH_C0, sigmoid};
use crate::{Error, FileUp, Limits, Raw, Result, SplatFormat, bad, keep};

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

    /// The value at the start of `b` (which holds at least [`Kind::size`] bytes), as f64 so
    /// 32-bit packed words stay exact.
    fn read(self, b: &[u8], le: bool) -> f64 {
        macro_rules! get {
            ($t:ty, $n:expr) => {{
                let mut a = [0u8; $n];
                a.copy_from_slice(&b[..$n]);
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
    /// The count type of a list property.
    list: Option<Kind>,
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    count: u64,
    props: Vec<Prop>,
}

impl Element {
    fn prop(&self, name: &str) -> Option<usize> {
        self.props.iter().position(|p| p.name == name)
    }

    /// Bytes per row in binary, `None` with list properties.
    fn stride(&self) -> Option<usize> {
        self.props
            .iter()
            .map(|p| p.list.is_none().then_some(p.kind.size()))
            .sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Coding {
    Ascii,
    Le,
    Be,
}

struct Header {
    coding: Coding,
    elements: Vec<Element>,
    /// Where the body starts.
    body: usize,
}

fn header(bytes: &[u8]) -> Result<Header> {
    if !(bytes.starts_with(b"ply\n") || bytes.starts_with(b"ply\r\n")) {
        return bad("not a PLY file");
    }
    let end = find(bytes, b"end_header")
        .filter(|&i| i < 1 << 20)
        .ok_or_else(|| Error::Malformed("no end_header".into()))?;
    let mut body = end + b"end_header".len();
    if bytes.get(body) == Some(&b'\r') {
        body += 1;
    }
    if bytes.get(body) == Some(&b'\n') {
        body += 1;
    }
    let text = String::from_utf8_lossy(&bytes[..end]);
    let mut coding = None;
    let mut elements: Vec<Element> = Vec::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["format", f, ..] => {
                coding = Some(match *f {
                    "ascii" => Coding::Ascii,
                    "binary_little_endian" => Coding::Le,
                    "binary_big_endian" => Coding::Be,
                    _ => return bad(format!("unknown PLY format {f}")),
                })
            }
            ["element", name, count] => elements.push(Element {
                name: (*name).to_owned(),
                count: count
                    .parse()
                    .map_err(|_| Error::Malformed(format!("bad element count {count}")))?,
                props: Vec::new(),
            }),
            ["property", "list", c, k, name] => {
                let (Some(c), Some(k)) = (Kind::of(c), Kind::of(k)) else {
                    return bad(format!("unknown list types in {line}"));
                };
                let e = elements
                    .last_mut()
                    .ok_or_else(|| Error::Malformed("property before element".into()))?;
                e.props.push(Prop {
                    name: (*name).to_owned(),
                    kind: k,
                    list: Some(c),
                });
            }
            ["property", t, name] => {
                let k = Kind::of(t).ok_or_else(|| Error::Malformed(format!("type {t}")))?;
                let e = elements
                    .last_mut()
                    .ok_or_else(|| Error::Malformed("property before element".into()))?;
                e.props.push(Prop {
                    name: (*name).to_owned(),
                    kind: k,
                    list: None,
                });
            }
            _ => {}
        }
    }
    Ok(Header {
        coding: coding.ok_or_else(|| Error::Malformed("no format line".into()))?,
        elements,
        body,
    })
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Whether the vertex element is a 3DGS one, or the file is a compressed splat PLY.
fn kind_of(h: &Header) -> Option<SplatFormat> {
    let v = h.elements.iter().find(|e| e.name == "vertex")?;
    let has = |n: &str| v.prop(n).is_some();
    if h.elements.iter().any(|e| e.name == "chunk") && has("packed_position") {
        return Some(SplatFormat::CompressedPly);
    }
    let colour = has("f_dc_0") || (has("red") && has("green") && has("blue"));
    (colour && has("opacity") && has("scale_0") && has("rot_0")).then_some(SplatFormat::Ply)
}

pub(crate) fn is_splats(bytes: &[u8]) -> bool {
    header(bytes).ok().as_ref().and_then(kind_of).is_some()
}

/// One element's rows, by property index.
enum Table<'a> {
    Binary {
        data: &'a [u8],
        stride: usize,
        offsets: Vec<usize>,
        kinds: Vec<Kind>,
        le: bool,
    },
    Ascii {
        values: Vec<f64>,
        cols: usize,
    },
}

impl Table<'_> {
    fn get(&self, row: usize, prop: usize) -> f64 {
        match self {
            Table::Binary {
                data,
                stride,
                offsets,
                kinds,
                le,
            } => kinds[prop].read(&data[row * stride + offsets[prop]..], *le),
            Table::Ascii { values, cols } => values[row * cols + prop],
        }
    }
}

/// The tables of every element (ASCII: the elements up to the last one needed are parsed).
fn tables<'a>(h: &Header, bytes: &'a [u8], need: &[&str]) -> Result<Vec<Option<Table<'a>>>> {
    let mut out = Vec::new();
    let mut at = h.body;
    match h.coding {
        Coding::Le | Coding::Be => {
            for e in &h.elements {
                let wanted = need.contains(&e.name.as_str());
                let Some(stride) = e.stride() else {
                    if wanted || out.len() < h.elements.len() - 1 {
                        return Err(Error::Unsupported(format!(
                            "list properties in the PLY element {}",
                            e.name
                        )));
                    }
                    out.push(None);
                    continue;
                };
                let len = usize::try_from(e.count)
                    .ok()
                    .and_then(|c| c.checked_mul(stride))
                    .filter(|&l| at.checked_add(l).is_some_and(|end| end <= bytes.len()))
                    .ok_or_else(|| {
                        Error::Malformed(format!("the PLY element {} runs past the end", e.name))
                    })?;
                let mut offsets = Vec::new();
                let mut o = 0;
                for p in &e.props {
                    offsets.push(o);
                    o += p.kind.size();
                }
                out.push(wanted.then(|| Table::Binary {
                    data: &bytes[at..at + len],
                    stride,
                    offsets,
                    kinds: e.props.iter().map(|p| p.kind).collect(),
                    le: h.coding == Coding::Le,
                }));
                at += len;
            }
        }
        Coding::Ascii => {
            let text = std::str::from_utf8(&bytes[h.body..])
                .map_err(|_| Error::Malformed("ASCII PLY is not text".into()))?;
            let mut lines = text.lines().filter(|l| !l.trim().is_empty());
            for e in &h.elements {
                let wanted = need.contains(&e.name.as_str());
                if e.props.iter().any(|p| p.list.is_some()) && wanted {
                    return Err(Error::Unsupported(format!(
                        "list properties in the PLY element {}",
                        e.name
                    )));
                }
                let count = usize::try_from(e.count)
                    .ok()
                    .filter(|&c| c <= text.len() / 2 + 1)
                    .ok_or_else(|| Error::Malformed("element count past the file".into()))?;
                let cols = e.props.len();
                // Each value takes at least two characters ("0 ").
                let total = count
                    .checked_mul(cols)
                    .filter(|&n| n <= text.len() / 2 + 1)
                    .ok_or_else(|| Error::Malformed("element count past the file".into()))?;
                let mut values = if wanted {
                    Vec::with_capacity(total)
                } else {
                    Vec::new()
                };
                for _ in 0..count {
                    let line = lines
                        .next()
                        .ok_or_else(|| Error::Malformed("ASCII PLY ends early".into()))?;
                    if wanted {
                        let row: Vec<f64> = line
                            .split_whitespace()
                            .map(|t| t.parse::<f64>().unwrap_or(f64::NAN))
                            .collect();
                        if row.len() < cols {
                            return bad("short ASCII PLY row");
                        }
                        values.extend_from_slice(&row[..cols]);
                    }
                }
                out.push(wanted.then_some(Table::Ascii { values, cols }));
            }
        }
    }
    Ok(out)
}

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Raw> {
    let h = header(bytes)?;
    match kind_of(&h) {
        Some(SplatFormat::Ply) => read_3dgs(&h, bytes, limits),
        Some(SplatFormat::CompressedPly) => read_compressed(&h, bytes, limits),
        _ => bad("not a Gaussian splat PLY (no f_dc, opacity, scale and rot properties)"),
    }
}

fn read_3dgs(h: &Header, bytes: &[u8], limits: &Limits) -> Result<Raw> {
    let vi = h
        .elements
        .iter()
        .position(|e| e.name == "vertex")
        .expect("checked");
    let v = &h.elements[vi];
    if v.count == 0 {
        return Err(Error::Empty);
    }
    if v.count > limits.max_declared {
        return Err(Error::TooLarge(format!("{} splats", v.count)));
    }
    let t = tables(h, bytes, &["vertex"])?;
    let t = t[vi].as_ref().expect("wanted");
    let p = |n: &str| v.prop(n);
    let need = |n: &str| p(n).ok_or_else(|| Error::Malformed(format!("no {n} property")));
    let pos = [need("x")?, need("y")?, need("z")?];
    let scale = [need("scale_0")?, need("scale_1")?, need("scale_2")?];
    let rot = [
        need("rot_0")?,
        need("rot_1")?,
        need("rot_2")?,
        need("rot_3")?,
    ];
    let opacity = need("opacity")?;
    let dc = match (p("f_dc_0"), p("f_dc_1"), p("f_dc_2")) {
        (Some(a), Some(b), Some(c)) => Ok([a, b, c]),
        _ => Err([need("red")?, need("green")?, need("blue")?]),
    };
    // f_rest_0.. in order; a count that is not a whole degree keeps the degrees it covers.
    let mut rest = Vec::new();
    while let Some(i) = p(&format!("f_rest_{}", rest.len())) {
        rest.push(i);
    }
    let per = rest.len() / 3;
    let degree: u8 = match per {
        15.. => 3,
        8..=14 => 2,
        3..=7 => 1,
        _ => 0,
    };
    let k = crate::sh_coeffs(degree);
    let n = v.count;
    let (kept, stride) = keep(n, limits.max_splats);
    let mut raw = Raw::new(SplatFormat::Ply, n, FileUp::NegY, kept, k);
    raw.sh_degree = degree;
    if per != k {
        raw.notes.push(format!(
            "{} harmonic coefficients a channel; degree {degree} used",
            per
        ));
    }
    if stride > 1 {
        raw.notes
            .push(format!("every {stride}th of {n} splats shown (the cap)"));
    }
    let f = |row: usize, i: usize| t.get(row, i) as f32;
    for row in (0..n as usize).step_by(stride as usize) {
        raw.positions.push(pos.map(|i| f(row, i)));
        raw.scales.push(scale.map(|i| f(row, i).exp()));
        raw.rotations.push(rot.map(|i| f(row, i)));
        let c = match dc {
            Ok(dc) => dc.map(|i| 0.5 + SH_C0 * f(row, i)),
            Err(rgb) => rgb.map(|i| f(row, i) / 255.0),
        };
        raw.colors
            .push([c[0], c[1], c[2], sigmoid(f(row, opacity))]);
        for j in 0..k {
            for ch in 0..3 {
                raw.sh.push(f32_to_f16(f(row, rest[ch * per + j])));
            }
        }
    }
    Ok(raw)
}

/// Splats per chunk of a compressed PLY.
const CHUNK: usize = 256;

fn read_compressed(h: &Header, bytes: &[u8], limits: &Limits) -> Result<Raw> {
    let index = |n: &str| h.elements.iter().position(|e| e.name == n);
    let (ci, vi) = (
        index("chunk").expect("checked"),
        index("vertex").expect("checked"),
    );
    let si = index("sh");
    let (c, v) = (&h.elements[ci], &h.elements[vi]);
    let n = v.count;
    if n == 0 {
        return Err(Error::Empty);
    }
    if n > limits.max_declared {
        return Err(Error::TooLarge(format!("{n} splats")));
    }
    if c.count < n.div_ceil(CHUNK as u64) {
        return bad("fewer chunks than the splats need");
    }
    let t = tables(h, bytes, &["chunk", "vertex", "sh"])?;
    let (ct, vt) = (
        t[ci].as_ref().expect("wanted"),
        t[vi].as_ref().expect("wanted"),
    );
    let cprop = |n: &str| {
        c.prop(n)
            .ok_or_else(|| Error::Malformed(format!("no chunk {n}")))
    };
    let names = |s: &str| [format!("min_{s}"), format!("max_{s}")];
    let mut ranges = Vec::new();
    for s in ["x", "y", "z", "scale_x", "scale_y", "scale_z"] {
        let [a, b] = names(s);
        ranges.push([cprop(&a)?, cprop(&b)?]);
    }
    let colour_ranges = ["r", "g", "b"]
        .iter()
        .map(|s| {
            let [a, b] = names(s);
            Some([c.prop(&a)?, c.prop(&b)?])
        })
        .collect::<Option<Vec<_>>>();
    let vprop = |n: &str| v.prop(n).ok_or_else(|| Error::Malformed(format!("no {n}")));
    let (pp, pr, ps, pc) = (
        vprop("packed_position")?,
        vprop("packed_rotation")?,
        vprop("packed_scale")?,
        vprop("packed_color")?,
    );
    let sh = si.and_then(|i| Some((&h.elements[i], t[i].as_ref()?)));
    let (degree, rest) = match sh {
        Some((e, _)) if e.count == n => {
            let mut rest = Vec::new();
            while let Some(i) = e.prop(&format!("f_rest_{}", rest.len())) {
                rest.push(i);
            }
            let per = rest.len() / 3;
            let d: u8 = match per {
                15.. => 3,
                8..=14 => 2,
                3..=7 => 1,
                _ => 0,
            };
            (d, rest)
        }
        _ => (0, Vec::new()),
    };
    let per = rest.len() / 3;
    let k = crate::sh_coeffs(degree);
    let (kept, stride) = keep(n, limits.max_splats);
    let mut raw = Raw::new(SplatFormat::CompressedPly, n, FileUp::NegY, kept, k);
    raw.sh_degree = degree;
    if stride > 1 {
        raw.notes
            .push(format!("every {stride}th of {n} splats shown (the cap)"));
    }
    let lerp = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
    let unpack111011 = |w: u32| {
        [
            ((w >> 21) & 0x7ff) as f32 / 2047.0,
            ((w >> 11) & 0x3ff) as f32 / 1023.0,
            (w & 0x7ff) as f32 / 2047.0,
        ]
    };
    for row in (0..n as usize).step_by(stride as usize) {
        let ch = row / CHUNK;
        let cv = |i: usize| ct.get(ch, i) as f32;
        let word = |i: usize| vt.get(row, i) as u32;
        let u = unpack111011(word(pp));
        raw.positions.push(std::array::from_fn(|a| {
            lerp(cv(ranges[a][0]), cv(ranges[a][1]), u[a])
        }));
        let u = unpack111011(word(ps));
        raw.scales.push(std::array::from_fn(|a| {
            lerp(cv(ranges[3 + a][0]), cv(ranges[3 + a][1]), u[a]).exp()
        }));
        // Smallest three: the dropped component's index (w, x, y, z order), then the others
        // in order, each (v / 1023 − 0.5) √2.
        let r = word(pr);
        let big = (r >> 30) as usize;
        let s2 = std::f32::consts::SQRT_2;
        let parts = [(r >> 20) & 0x3ff, (r >> 10) & 0x3ff, r & 0x3ff]
            .map(|v| (v as f32 / 1023.0 - 0.5) * s2);
        let mut q = [0f32; 4];
        let mut it = parts.iter();
        for (i, slot) in q.iter_mut().enumerate() {
            if i != big {
                *slot = *it.next().expect("three");
            }
        }
        q[big] = (1.0 - parts.iter().map(|v| v * v).sum::<f32>())
            .max(0.0)
            .sqrt();
        raw.rotations.push(q);
        let col = word(pc);
        let b = |s: u32| ((col >> s) & 0xff) as f32 / 255.0;
        let mut rgb = [b(24), b(16), b(8)];
        if let Some(cr) = &colour_ranges {
            for (a, v) in rgb.iter_mut().enumerate() {
                *v = lerp(cv(cr[a][0]), cv(cr[a][1]), *v);
            }
        }
        raw.colors.push([rgb[0], rgb[1], rgb[2], b(0)]);
        if let Some((_, st)) = sh {
            for j in 0..k {
                for c in 0..3 {
                    let byte = st.get(row, rest[c * per + j]) as f32;
                    raw.sh.push(f32_to_f16(byte * (8.0 / 255.0) - 4.0));
                }
            }
        }
    }
    Ok(raw)
}

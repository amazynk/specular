//! PLY (Stanford polygon file): ASCII and binary, little- and big-endian. The `vertex`
//! element gives positions, normals, colours and texture coordinates; `face` gives polygons
//! (as fans) through `vertex_indices` or `vertex_index`; `tristrips` gives strips with `-1`
//! restarts. Other elements are skipped. A `TextureFile` comment names a texture beside the
//! file.

use crate::builder::Builder;
use crate::text::{f64_of, lines, string, tokens, trim};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Up, bad};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    fn of(name: &[u8]) -> Option<Kind> {
        Some(match name {
            b"char" | b"int8" => Kind::I8,
            b"uchar" | b"uint8" => Kind::U8,
            b"short" | b"int16" => Kind::I16,
            b"ushort" | b"uint16" => Kind::U16,
            b"int" | b"int32" => Kind::I32,
            b"uint" | b"uint32" => Kind::U32,
            b"float" | b"float32" => Kind::F32,
            b"double" | b"float64" => Kind::F64,
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

    /// The largest value of an unsigned integer kind, for colour scaling.
    fn full_scale(self) -> f64 {
        match self {
            Kind::U8 | Kind::I8 => 255.0,
            Kind::U16 | Kind::I16 => 65535.0,
            Kind::U32 | Kind::I32 => 4_294_967_295.0,
            Kind::F32 | Kind::F64 => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Property {
    name: Vec<u8>,
    kind: Kind,
    /// For a list: the kind of its count.
    list: Option<Kind>,
}

#[derive(Clone, Debug)]
struct Element {
    name: Vec<u8>,
    count: usize,
    props: Vec<Property>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Ascii,
    Le,
    Be,
}

pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let mut b = Builder::new(limits);
    b.up = Up::Z;
    let (enc, elements, body, texture) = header(bytes)?;
    let mut body = Body {
        bytes: body,
        pos: 0,
        enc,
    };
    let has_uv = elements
        .iter()
        .find(|e| e.name == b"vertex")
        .is_some_and(|e| prop_index(e, &[b"u", b"s", b"texture_u", b"texture_s"]).is_some());
    if let Some(file) = texture
        && has_uv
    {
        let image = match resolver.read(&file, limits.max_side_file) {
            Ok(bytes) => ImageData::Bytes { bytes, mime: None },
            Err(reason) => ImageData::Missing { name: file, reason },
        };
        let texture = b.add_image(image);
        let m = b.add_material(Material {
            texture,
            ..Material::default()
        });
        b.material(Some(m));
    }
    let mut have_faces = false;
    for el in &elements {
        match el.name.as_slice() {
            b"vertex" if b.vertex_count() == 0 => vertices(el, &mut body, &mut b)?,
            b"face" => {
                have_faces = true;
                faces(el, &mut body, &mut b, false)?;
            }
            b"tristrips" => {
                have_faces = true;
                faces(el, &mut body, &mut b, true)?;
            }
            _ => skip(el, &mut body)?,
        }
    }
    if !have_faces {
        return Err(Error::Unsupported("a point cloud (no faces)".into()));
    }
    b.finish()
}

type Header<'a> = (Encoding, Vec<Element>, &'a [u8], Option<String>);

fn header(bytes: &[u8]) -> Result<Header<'_>> {
    let end = find(bytes, b"end_header").ok_or_else(|| Error::Malformed("no end_header".into()))?;
    // The body starts after the end_header line's line break.
    let mut start = end + b"end_header".len();
    while start < bytes.len() && bytes[start] != b'\n' {
        start += 1;
    }
    start = (start + 1).min(bytes.len());
    let mut enc = None;
    let mut elements: Vec<Element> = Vec::new();
    let mut texture = None;
    for line in lines(&bytes[..end]) {
        let mut t = tokens(line);
        match t.next() {
            Some(b"format") => {
                enc = Some(match t.next() {
                    Some(b"ascii") => Encoding::Ascii,
                    Some(b"binary_little_endian") => Encoding::Le,
                    Some(b"binary_big_endian") => Encoding::Be,
                    _ => return bad("unknown PLY format"),
                });
            }
            Some(b"element") => {
                let name = t.next().unwrap_or_default().to_vec();
                let count = t
                    .next()
                    .and_then(|c| std::str::from_utf8(c).ok()?.parse::<usize>().ok())
                    .ok_or_else(|| Error::Malformed("bad element count".into()))?;
                if elements.len() > 64 {
                    return bad("too many PLY elements");
                }
                elements.push(Element {
                    name,
                    count,
                    props: Vec::new(),
                });
            }
            Some(b"property") => {
                let el = elements
                    .last_mut()
                    .ok_or_else(|| Error::Malformed("property before element".into()))?;
                let first = t.next().unwrap_or_default();
                let prop = if first == b"list" {
                    let count = t.next().and_then(Kind::of);
                    let kind = t.next().and_then(Kind::of);
                    let name = t.next().unwrap_or_default().to_vec();
                    match (count, kind) {
                        (Some(c), Some(k)) if !matches!(c, Kind::F32 | Kind::F64) => Property {
                            name,
                            kind: k,
                            list: Some(c),
                        },
                        _ => return bad("bad PLY list property"),
                    }
                } else {
                    let kind =
                        Kind::of(first).ok_or_else(|| Error::Malformed("bad PLY type".into()))?;
                    Property {
                        name: t.next().unwrap_or_default().to_vec(),
                        kind,
                        list: None,
                    }
                };
                if el.props.len() > 256 {
                    return bad("too many PLY properties");
                }
                el.props.push(prop);
            }
            Some(b"comment") => {
                let mut rest = t;
                if let Some(k) = rest.next()
                    && k.eq_ignore_ascii_case(b"TextureFile")
                {
                    let line = trim(line);
                    let at = find(line, k).unwrap_or(0) + k.len();
                    let name = trim(&line[at..]);
                    if !name.is_empty() {
                        texture = Some(string(name));
                    }
                }
            }
            _ => {}
        }
    }
    let enc = enc.ok_or_else(|| Error::Malformed("no PLY format line".into()))?;
    Ok((enc, elements, &bytes[start..], texture))
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

struct Body<'a> {
    bytes: &'a [u8],
    pos: usize,
    enc: Encoding,
}

impl<'a> Body<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// The next ASCII line's tokens.
    fn line(&mut self) -> Result<impl Iterator<Item = &'a [u8]> + use<'a>> {
        loop {
            if self.pos >= self.bytes.len() {
                return bad("PLY body cut short");
            }
            let rest: &'a [u8] = &self.bytes[self.pos..];
            let end = rest.iter().position(|&c| c == b'\n').unwrap_or(rest.len());
            self.pos += (end + 1).min(rest.len());
            if !trim(&rest[..end]).is_empty() {
                return Ok(tokens(&rest[..end]));
            }
        }
    }

    fn scalar(&mut self, k: Kind) -> Result<f64> {
        let n = k.size();
        let Some(s) = self.bytes.get(self.pos..self.pos + n) else {
            return bad("PLY body cut short");
        };
        self.pos += n;
        let mut a = [0u8; 8];
        a[..n].copy_from_slice(s);
        if self.enc == Encoding::Be {
            a[..n].reverse();
        }
        Ok(match k {
            Kind::I8 => f64::from(a[0] as i8),
            Kind::U8 => f64::from(a[0]),
            Kind::I16 => f64::from(i16::from_le_bytes([a[0], a[1]])),
            Kind::U16 => f64::from(u16::from_le_bytes([a[0], a[1]])),
            Kind::I32 => f64::from(i32::from_le_bytes([a[0], a[1], a[2], a[3]])),
            Kind::U32 => f64::from(u32::from_le_bytes([a[0], a[1], a[2], a[3]])),
            Kind::F32 => f64::from(f32::from_le_bytes([a[0], a[1], a[2], a[3]])),
            Kind::F64 => f64::from_le_bytes(a),
        })
    }
}

/// The values of one element instance: scalars into `vals`, lists into `lists` (flattened,
/// with `list_at` giving each list's start), in property order.
fn record(
    el: &Element,
    body: &mut Body,
    vals: &mut Vec<f64>,
    lists: &mut Vec<f64>,
    list_at: &mut Vec<(usize, usize)>,
) -> Result<()> {
    vals.clear();
    lists.clear();
    list_at.clear();
    if body.enc == Encoding::Ascii {
        let mut t = body.line()?;
        for p in &el.props {
            let mut next = || -> Result<f64> {
                t.next()
                    .and_then(f64_of)
                    .ok_or_else(|| Error::Malformed("bad PLY value".into()))
            };
            match p.list {
                None => vals.push(next()?),
                Some(_) => {
                    let n = next()?;
                    if !(0.0..=1e6).contains(&n) {
                        return bad("bad PLY list length");
                    }
                    let s = lists.len();
                    for _ in 0..n as usize {
                        lists.push(next()?);
                    }
                    list_at.push((s, n as usize));
                    vals.push(f64::NAN);
                }
            }
        }
    } else {
        for p in &el.props {
            match p.list {
                None => vals.push(body.scalar(p.kind)?),
                Some(ck) => {
                    let n = body.scalar(ck)?;
                    if !(0.0..=1e6).contains(&n) || n as usize * p.kind.size() > body.remaining() {
                        return bad("bad PLY list length");
                    }
                    let s = lists.len();
                    for _ in 0..n as usize {
                        lists.push(body.scalar(p.kind)?);
                    }
                    list_at.push((s, n as usize));
                    vals.push(f64::NAN);
                }
            }
        }
    }
    Ok(())
}

/// The least bytes one record can take, to check a count against the body.
fn min_record(el: &Element, enc: Encoding) -> usize {
    match enc {
        Encoding::Ascii => 1,
        _ => el
            .props
            .iter()
            .map(|p| p.list.map_or(p.kind.size(), Kind::size))
            .sum::<usize>()
            .max(1),
    }
}

fn check_count(el: &Element, body: &Body) -> Result<()> {
    if el.count.saturating_mul(min_record(el, body.enc)) > body.remaining() {
        return bad(format!(
            "PLY {} count larger than the file",
            String::from_utf8_lossy(&el.name)
        ));
    }
    Ok(())
}

fn skip(el: &Element, body: &mut Body) -> Result<()> {
    check_count(el, body)?;
    if body.enc != Encoding::Ascii && el.props.iter().all(|p| p.list.is_none()) {
        let n = el.count * min_record(el, body.enc);
        body.pos += n;
        return Ok(());
    }
    let (mut v, mut l, mut a) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..el.count {
        record(el, body, &mut v, &mut l, &mut a)?;
    }
    Ok(())
}

fn prop_index(el: &Element, names: &[&[u8]]) -> Option<usize> {
    names.iter().find_map(|n| {
        el.props
            .iter()
            .position(|p| p.list.is_none() && p.name == *n)
    })
}

fn vertices(el: &Element, body: &mut Body, b: &mut Builder) -> Result<()> {
    check_count(el, body)?;
    b.reserve_vertices(el.count)?;
    let xyz = [
        prop_index(el, &[b"x"]),
        prop_index(el, &[b"y"]),
        prop_index(el, &[b"z"]),
    ];
    let [Some(x), Some(y), Some(z)] = xyz else {
        return bad("PLY vertices without x, y, z");
    };
    let nrm = [
        prop_index(el, &[b"nx"]),
        prop_index(el, &[b"ny"]),
        prop_index(el, &[b"nz"]),
    ];
    let nrm = match nrm {
        [Some(a), Some(b), Some(c)] => Some([a, b, c]),
        _ => None,
    };
    let rgb = [
        prop_index(el, &[b"red", b"diffuse_red", b"r"]),
        prop_index(el, &[b"green", b"diffuse_green", b"g"]),
        prop_index(el, &[b"blue", b"diffuse_blue", b"b"]),
    ];
    let rgb = match rgb {
        [Some(a), Some(b), Some(c)] => Some([a, b, c]),
        _ => None,
    };
    let alpha = prop_index(el, &[b"alpha", b"diffuse_alpha", b"a"]);
    let uv = match (
        prop_index(el, &[b"u", b"s", b"texture_u", b"texture_s"]),
        prop_index(el, &[b"v", b"t", b"texture_v", b"texture_t"]),
    ) {
        (Some(a), Some(b)) => Some([a, b]),
        _ => None,
    };
    let scale = |i: usize| el.props[i].kind.full_scale();
    let (mut v, mut l, mut a) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..el.count {
        record(el, body, &mut v, &mut l, &mut a)?;
        let p = [v[x], v[y], v[z]].map(|c| c as f32);
        let n = nrm.map(|[a, bb, c]| [v[a], v[bb], v[c]].map(|c| c as f32));
        let c = rgb.map(|[r, g, bl]| {
            let q = |i: usize| ((v[i] / scale(i)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            [q(r), q(g), q(bl), alpha.map_or(255, q)]
        });
        // PLY's v points up; the model keeps glTF's (down).
        let t = uv.map(|[u, w]| [v[u] as f32, 1.0 - v[w] as f32]);
        b.vertex(p, n, c, t)?;
    }
    Ok(())
}

fn faces(el: &Element, body: &mut Body, b: &mut Builder, strips: bool) -> Result<()> {
    check_count(el, body)?;
    let Some(li) = el
        .props
        .iter()
        .filter(|p| p.list.is_some())
        .position(|p| p.name == b"vertex_indices" || p.name == b"vertex_index")
    else {
        return bad("PLY faces without vertex_indices");
    };
    if !strips {
        b.reserve_triangles(el.count)?;
    }
    let nv = b.vertex_count() as f64;
    let (mut v, mut l, mut a) = (Vec::new(), Vec::new(), Vec::new());
    let mut bad_faces = 0usize;
    for _ in 0..el.count {
        record(el, body, &mut v, &mut l, &mut a)?;
        let (s, n) = a[li];
        let idx = &l[s..s + n];
        if strips {
            // Runs separated by -1; alternate winding.
            for run in idx.split(|&i| i < 0.0) {
                for k in 2..run.len() {
                    let (i0, i1, i2) = (run[k - 2], run[k - 1], run[k]);
                    if i0 == i1 || i1 == i2 || i0 == i2 {
                        continue;
                    }
                    let t = if k % 2 == 0 {
                        [i0, i1, i2]
                    } else {
                        [i1, i0, i2]
                    };
                    if t.iter().all(|&i| (0.0..nv).contains(&i)) {
                        b.triangle(t[0] as u32, t[1] as u32, t[2] as u32)?;
                    }
                }
            }
            continue;
        }
        if n < 3
            || idx
                .iter()
                .any(|&i| !(0.0..nv).contains(&i) || i.fract() != 0.0)
        {
            bad_faces += 1;
            continue;
        }
        for k in 2..n {
            b.triangle(idx[0] as u32, idx[k - 1] as u32, idx[k] as u32)?;
        }
    }
    if bad_faces > 0 {
        b.note(format!("{bad_faces} unreadable faces skipped"));
    }
    Ok(())
}

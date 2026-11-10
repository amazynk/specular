//! VTK XML files: `<VTKFile type="…">` with one dataset element of pieces, data arrays in
//! ascii, inline base64 or appended (raw or base64) encodings, optionally compressed in
//! blocks.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ops::Range;

use quick_xml::events::{BytesStart, Event};

use crate::base64;
use crate::cells::{self, CellArray, Cells, Polyhedra, Triangles};
use crate::num::{self, Ty};
use crate::{DataArray, Dataset, Error, Grid, Kind, MeshData, Result, Vtk, bad, find, limits};

/// One element: name, attributes, the byte ranges of its own text, children.
#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: Vec<Range<usize>>,
    children: Vec<Node>,
}

impl Node {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }

    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    fn arrays(&self) -> impl Iterator<Item = &Node> {
        self.all("DataArray")
    }

    fn array(&self, name: &str) -> Option<&Node> {
        self.arrays().find(|a| a.attr("Name") == Some(name))
    }
}

/// The value of attribute `name` in a start tag's bytes (no entity decoding).
pub(crate) fn attr_in_tag(tag: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    let mut i = 0;
    while let Some(at) = find(&tag[i..], name).map(|p| p + i) {
        i = at + name.len();
        let before_ok = at > 0 && tag[at - 1].is_ascii_whitespace();
        let mut j = i;
        while j < tag.len() && tag[j].is_ascii_whitespace() {
            j += 1;
        }
        if !before_ok || tag.get(j) != Some(&b'=') {
            continue;
        }
        j += 1;
        while j < tag.len() && tag[j].is_ascii_whitespace() {
            j += 1;
        }
        let q = *tag.get(j)?;
        if q != b'"' && q != b'\'' {
            continue;
        }
        let end = tag[j + 1..].iter().position(|&b| b == q)? + j + 1;
        return Some(tag[j + 1..end].to_vec());
    }
    None
}

fn unescape(s: &[u8]) -> String {
    let t = String::from_utf8_lossy(s);
    if !t.contains('&') {
        return t.into_owned();
    }
    t.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn element(e: &BytesStart<'_>) -> Result<Node> {
    let mut n = Node {
        name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
        ..Node::default()
    };
    for a in e.attributes() {
        let a = a.map_err(|x| Error::Malformed(format!("XML attribute: {x}")))?;
        n.attrs.push((
            String::from_utf8_lossy(a.key.as_ref()).into_owned(),
            unescape(&a.value),
        ));
    }
    Ok(n)
}

/// The element tree of `xml`; elements left open at the end are closed.
fn tree(xml: &[u8]) -> Result<Node> {
    let mut r = quick_xml::Reader::from_reader(xml);
    r.config_mut().trim_text(false);
    r.config_mut().check_end_names = false;
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut nodes = 0usize;
    loop {
        let before = r.buffer_position() as usize;
        let ev = r
            .read_event()
            .map_err(|e| Error::Malformed(format!("XML: {e}")))?;
        let after = r.buffer_position() as usize;
        match ev {
            Event::Start(e) => {
                nodes += 1;
                if nodes > 4 << 20 || stack.len() > 256 {
                    return Err(Error::TooLarge);
                }
                stack.push(element(&e)?);
            }
            Event::Empty(e) => {
                nodes += 1;
                let n = element(&e)?;
                if let Some(top) = stack.last_mut() {
                    top.children.push(n);
                }
            }
            Event::End(_) => {
                if stack.len() > 1 {
                    let n = stack.pop().unwrap_or_default();
                    if let Some(top) = stack.last_mut() {
                        top.children.push(n);
                    }
                }
            }
            Event::Text(_) | Event::CData(_) => {
                if let Some(top) = stack.last_mut()
                    && before < after
                {
                    top.text.push(before..after.min(xml.len()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap_or_default();
        if let Some(top) = stack.last_mut() {
            top.children.push(n);
        }
    }
    let mut root = stack.pop().unwrap_or_default();
    let i = root
        .children
        .iter()
        .position(|c| c.name == "VTKFile")
        .ok_or(Error::NotVtk)?;
    Ok(root.children.swap_remove(i))
}

#[derive(Clone, Copy, PartialEq)]
enum Comp {
    None,
    Zlib,
    Lz4,
}

struct Ctx<'a> {
    data: &'a [u8],
    big: bool,
    header64: bool,
    comp: Comp,
    /// Bytes after the `_` of `<AppendedData>`, and whether they are base64.
    appended: Option<(&'a [u8], bool)>,
    used: std::cell::RefCell<BTreeSet<&'static str>>,
}

/// An array's stored contents: its text or bytes in the file's byte order.
enum Payload<'a> {
    Ascii(Vec<&'a [u8]>),
    Bytes(Cow<'a, [u8]>),
}

/// The white-space separated tokens of an array's text.
fn tokens<'a>(parts: &'a [&'a [u8]]) -> impl Iterator<Item = &'a str> + 'a {
    parts.iter().flat_map(|p| {
        p.split(|b| b.is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .map(|t| std::str::from_utf8(t).unwrap_or("?"))
    })
}

/// `want` tokens parsed by `f` (all of them when `want` is `None`).
fn parse_tokens<T>(
    parts: &[&[u8]],
    want: Option<u64>,
    f: impl Fn(&str) -> Option<T>,
) -> Result<Vec<T>> {
    let k = match want {
        Some(k) => k,
        None => tokens(parts).count() as u64,
    };
    let room: u64 = parts.iter().map(|p| p.len() as u64 + 1).sum();
    if k.saturating_mul(2) > room + 1 {
        return Err(Error::Truncated("ascii array".into()));
    }
    let mut out = Vec::new();
    num::reserve(&mut out, k)?;
    for t in tokens(parts).take(k as usize) {
        out.push(f(t).ok_or_else(|| Error::Malformed(format!("value {t}")))?);
    }
    if (out.len() as u64) < k {
        return Err(Error::Truncated("ascii array".into()));
    }
    Ok(out)
}

impl<'a> Ctx<'a> {
    fn word(&self, b: &[u8]) -> u64 {
        // A short header reads as 0 (callers check lengths first).
        let n = self.wsize();
        let Some(b) = b.get(..n) else {
            return 0;
        };
        b.iter().enumerate().fold(0u64, |v, (k, &x)| {
            let x = u64::from(x);
            v | if self.big {
                x << (8 * (n - 1 - k))
            } else {
                x << (8 * k)
            }
        })
    }

    fn wsize(&self) -> usize {
        if self.header64 { 8 } else { 4 }
    }

    fn text_bytes(&self, n: &Node) -> Vec<u8> {
        let mut out = Vec::new();
        for r in &n.text {
            out.extend(
                self.data[r.clone()]
                    .iter()
                    .copied()
                    .filter(|b| !b.is_ascii_whitespace()),
            );
        }
        out
    }

    fn payload(&self, n: &Node) -> Result<Payload<'a>> {
        let format = n.attr("format").unwrap_or("ascii");
        match format {
            "ascii" => {
                self.used.borrow_mut().insert("ascii");
                let parts = n.text.iter().map(|r| &self.data[r.clone()]).collect();
                Ok(Payload::Ascii(parts))
            }
            "binary" => {
                self.used.borrow_mut().insert("base64");
                let text = self.text_bytes(n);
                Ok(Payload::Bytes(Cow::Owned(self.decode_base64(&text)?)))
            }
            "appended" => {
                let (app, b64) = self
                    .appended
                    .ok_or_else(|| Error::Malformed("no AppendedData".into()))?;
                let off: usize = n
                    .attr("offset")
                    .and_then(|o| o.trim().parse().ok())
                    .ok_or_else(|| Error::Malformed("array offset".into()))?;
                let src = app
                    .get(off..)
                    .ok_or_else(|| Error::Truncated("appended data".into()))?;
                if b64 {
                    self.used.borrow_mut().insert("appended base64");
                    Ok(Payload::Bytes(Cow::Owned(self.decode_base64(src)?)))
                } else {
                    self.used.borrow_mut().insert("appended raw");
                    self.decode_raw(src).map(Payload::Bytes)
                }
            }
            f => bad(format!("array format {f}")),
        }
    }

    /// The compressed-block header's length for the block count in `first`.
    fn header_len(&self, first: &[u8]) -> Result<usize> {
        if self.comp == Comp::None {
            return Ok(self.wsize());
        }
        let nb = self.word(first);
        if nb > (limits::MAX_BYTES >> 10) {
            return Err(Error::TooLarge);
        }
        Ok(self.wsize() * (3 + nb as usize))
    }

    /// Raw appended data: the header, then the data or its blocks.
    fn decode_raw(&self, src: &'a [u8]) -> Result<Cow<'a, [u8]>> {
        let w = self.wsize();
        if src.len() < w {
            return Err(Error::Truncated("block header".into()));
        }
        let hl = self.header_len(src)?;
        let h = src
            .get(..hl)
            .ok_or_else(|| Error::Truncated("block header".into()))?;
        let body = &src[hl..];
        if self.comp == Comp::None {
            let n = self.word(h);
            let n = usize::try_from(n).map_err(|_| Error::TooLarge)?;
            return body
                .get(..n)
                .map(Cow::Borrowed)
                .ok_or_else(|| Error::Truncated("appended data".into()));
        }
        self.inflate(h, body).map(Cow::Owned)
    }

    /// Base64 data: an uncompressed header and data in one stream, or a compressed header
    /// encoded on its own followed by the blocks' stream.
    fn decode_base64(&self, src: &[u8]) -> Result<Vec<u8>> {
        let w = self.wsize();
        let first_chars = base64::encoded_len(w as u64) as usize;
        let first = base64::decode(
            src.get(..first_chars)
                .ok_or_else(|| Error::Truncated("base64 header".into()))?,
        )?;
        if first.len() < w {
            return Err(Error::Truncated("base64 header".into()));
        }
        let hl = self.header_len(&first)?;
        let hchars = base64::encoded_len(hl as u64) as usize;
        let hsrc = src
            .get(..hchars)
            .ok_or_else(|| Error::Truncated("base64 header".into()))?;
        let separate = hl % 3 == 0 || hsrc.last() == Some(&b'=');
        if separate {
            let h = base64::decode(hsrc)?;
            if h.len() < hl {
                return Err(Error::Truncated("base64 header".into()));
            }
            let h = &h[..hl];
            let dlen = self.data_len(h)?;
            let dchars = base64::encoded_len(dlen) as usize;
            let rest = &src[hchars..];
            let body = base64::decode(&rest[..dchars.min(rest.len())])?;
            if (body.len() as u64) < dlen {
                return Err(Error::Truncated("base64 data".into()));
            }
            if self.comp == Comp::None {
                let mut body = body;
                body.truncate(dlen as usize);
                return Ok(body);
            }
            return self.inflate(h, &body);
        }
        // One stream: header and data together.
        let dlen = self.data_len(&first[..w])?;
        let total = (hl as u64).checked_add(dlen).ok_or(Error::TooLarge)?;
        let chars = base64::encoded_len(total) as usize;
        let all = base64::decode(&src[..chars.min(src.len())])?;
        if (all.len() as u64) < total {
            return Err(Error::Truncated("base64 data".into()));
        }
        if self.comp == Comp::None {
            return Ok(all[hl..total as usize].to_vec());
        }
        self.inflate(&all[..hl], &all[hl..])
    }

    /// The stored length of the data after header `h`: its size, or the compressed sizes'
    /// sum.
    fn data_len(&self, h: &[u8]) -> Result<u64> {
        let w = self.wsize();
        if self.comp == Comp::None {
            let n = self.word(h);
            if n > limits::MAX_BYTES {
                return Err(Error::TooLarge);
            }
            return Ok(n);
        }
        let nb = self.word(h) as usize;
        let mut sum = 0u64;
        for k in 0..nb {
            let s = h
                .get((3 + k) * w..(4 + k) * w)
                .ok_or_else(|| Error::Truncated("block header".into()))?;
            sum = sum.checked_add(self.word(s)).ok_or(Error::TooLarge)?;
        }
        Ok(sum)
    }

    fn inflate(&self, h: &[u8], body: &[u8]) -> Result<Vec<u8>> {
        let w = self.wsize();
        if h.len() < 3 * w {
            return Err(Error::Truncated("block header".into()));
        }
        let nb = self.word(h) as usize;
        let bs = self.word(&h[w..]);
        let last = self.word(&h[2 * w..]);
        let last = if last == 0 { bs } else { last };
        if nb == 0 {
            return Ok(Vec::new());
        }
        let total = (nb as u64 - 1)
            .checked_mul(bs)
            .and_then(|t| t.checked_add(last))
            .ok_or(Error::TooLarge)?;
        let csum = self.data_len(h)?;
        // The compressed sizes must be there, and they (not the claimed block size, which
        // could be anything) bound what the blocks decompress to.
        if csum > body.len() as u64 {
            return Err(Error::Truncated("compressed block".into()));
        }
        let max_ratio = if self.comp == Comp::Lz4 { 256 } else { 1100 };
        if total > limits::MAX_BYTES || total > csum.saturating_mul(max_ratio) + 65_536 {
            return Err(Error::TooLarge);
        }
        self.used.borrow_mut().insert(match self.comp {
            Comp::Zlib => "zlib",
            _ => "LZ4",
        });
        let mut out = Vec::new();
        out.try_reserve_exact(total as usize)
            .map_err(|_| Error::TooLarge)?;
        let mut at = 0usize;
        for k in 0..nb {
            let c = self.word(&h[(3 + k) * w..]) as usize;
            let block = body
                .get(at..at.checked_add(c).ok_or(Error::TooLarge)?)
                .ok_or_else(|| Error::Truncated("compressed block".into()))?;
            at += c;
            let want = if k + 1 == nb { last } else { bs } as usize;
            let raw = match self.comp {
                Comp::Zlib => specular_png::inflate_zlib(block, want)
                    .map_err(|e| Error::Malformed(format!("zlib block: {e}")))?,
                _ => specular_hdf5::codec::lz4_block(block, want)
                    .map_err(|e| Error::Malformed(format!("LZ4 block: {e}")))?,
            };
            if raw.len() != want {
                return Err(Error::Malformed("block size".into()));
            }
            out.extend_from_slice(&raw);
        }
        Ok(out)
    }

    fn ty(n: &Node) -> Result<(Ty, &'static str)> {
        Ty::xml(n.attr("type").unwrap_or("Float32"))
    }

    fn comps(n: &Node) -> Result<usize> {
        let c = n
            .attr("NumberOfComponents")
            .map_or(Ok(1), |c| c.trim().parse::<usize>())
            .map_err(|_| Error::Malformed("NumberOfComponents".into()))?;
        if c == 0 || c > 1 << 16 {
            return bad("NumberOfComponents");
        }
        Ok(c)
    }

    /// Values of an array; `want` values when known, else as many as are stored.
    fn f32s(&self, n: &Node, want: Option<u64>) -> Result<Vec<f32>> {
        let (ty, _) = Ctx::ty(n)?;
        let mut out = Vec::new();
        match self.payload(n)? {
            Payload::Ascii(t) => out = parse_tokens(&t, want, |s| num::parse_f32(ty, s))?,
            Payload::Bytes(b) => {
                let k = want.unwrap_or(if ty == Ty::Bit {
                    b.len() as u64 * 8
                } else {
                    (b.len() / ty.size()) as u64
                });
                num::to_f32(
                    ty,
                    &b,
                    usize::try_from(k).map_err(|_| Error::TooLarge)?,
                    self.big,
                    &mut out,
                )?;
            }
        }
        Ok(out)
    }

    fn f64s(&self, n: &Node, want: Option<u64>) -> Result<Vec<f64>> {
        let (ty, _) = Ctx::ty(n)?;
        match self.payload(n)? {
            Payload::Ascii(t) => parse_tokens(&t, want, num::parse_f64),
            Payload::Bytes(b) => {
                let k = want.unwrap_or(if ty == Ty::Bit {
                    b.len() as u64 * 8
                } else {
                    (b.len() / ty.size()) as u64
                });
                num::to_f64(
                    ty,
                    &b,
                    usize::try_from(k).map_err(|_| Error::TooLarge)?,
                    self.big,
                )
            }
        }
    }

    fn i64s(&self, n: &Node, want: Option<u64>) -> Result<Vec<i64>> {
        let (ty, _) = Ctx::ty(n)?;
        match self.payload(n)? {
            Payload::Ascii(t) => parse_tokens(&t, want, |s| {
                num::parse_int(s).and_then(|v| i64::try_from(v).ok())
            }),
            Payload::Bytes(b) => {
                let k = want.unwrap_or(if ty == Ty::Bit {
                    b.len() as u64 * 8
                } else {
                    (b.len() / ty.size()) as u64
                });
                num::to_i64(
                    ty,
                    &b,
                    usize::try_from(k).map_err(|_| Error::TooLarge)?,
                    self.big,
                )
            }
        }
    }

    /// The data arrays of a `PointData`/`CellData`/`FieldData` section with `tuples` tuples
    /// each (field data: as the array says).
    fn section(&self, sec: Option<&Node>, tuples: Option<u64>) -> Result<Vec<DataArray>> {
        let Some(sec) = sec else {
            return Ok(Vec::new());
        };
        let roles = [
            ("Scalars", "Scalars"),
            ("Vectors", "Vectors"),
            ("Normals", "Normals"),
            ("Tensors", "Tensors"),
            ("TCoords", "TCoords"),
            ("GlobalIds", "GlobalIds"),
            ("PedigreeIds", "PedigreeIds"),
        ];
        let mut out = Vec::new();
        for a in sec.arrays() {
            if out.len() >= limits::MAX_ARRAYS {
                return Err(Error::TooLarge);
            }
            let name = a.attr("Name").unwrap_or("").to_owned();
            let (_, dtype) = match Ctx::ty(a) {
                Ok(t) => t,
                // A string array among numbers (field data names) is left out.
                Err(Error::Unsupported(_)) if tuples.is_none() => continue,
                Err(e) => return Err(e),
            };
            let comps = Ctx::comps(a)?;
            let t = tuples.or_else(|| a.attr("NumberOfTuples").and_then(|v| v.trim().parse().ok()));
            let want = t
                .map(|t| t.checked_mul(comps as u64).ok_or(Error::TooLarge))
                .transpose()?;
            let mut values = self.f32s(a, want)?;
            values.truncate(values.len() / comps * comps);
            let attribute = roles
                .iter()
                .find(|(k, _)| sec.attr(k) == Some(name.as_str()) && !name.is_empty())
                .map(|(_, r)| *r);
            out.push(DataArray {
                name,
                components: comps,
                dtype,
                values,
                attribute,
            });
        }
        Ok(out)
    }
}

fn ints<const N: usize>(s: Option<&str>, what: &str) -> Result<[i64; N]> {
    let v: Vec<i64> = s
        .unwrap_or("")
        .split_whitespace()
        .map(|t| t.parse::<i64>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error::Malformed(what.into()))?;
    v.try_into().map_err(|_| Error::Malformed(what.into()))
}

fn floats<const N: usize>(s: Option<&str>, default: [f64; N]) -> Result<[f64; N]> {
    let Some(s) = s else {
        return Ok(default);
    };
    let v: Vec<f64> = s
        .split_whitespace()
        .map(num::parse_f64)
        .collect::<Option<_>>()
        .ok_or_else(|| Error::Malformed("number list".into()))?;
    v.try_into()
        .map_err(|_| Error::Malformed("number list".into()))
}

/// Points along each axis of an extent.
fn extent_dims(e: [i64; 6]) -> Result<[usize; 3]> {
    let mut d = [0usize; 3];
    for k in 0..3 {
        let n = e[2 * k + 1]
            .checked_sub(e[2 * k])
            .and_then(|v| v.checked_add(1))
            .filter(|&v| v >= 1)
            .ok_or_else(|| Error::Malformed("extent".into()))?;
        d[k] = usize::try_from(n).map_err(|_| Error::TooLarge)?;
    }
    if d.iter()
        .try_fold(1u64, |a, &x| a.checked_mul(x as u64))
        .is_none_or(|n| n > limits::MAX_VALUES)
    {
        return Err(Error::TooLarge);
    }
    Ok(d)
}

pub(crate) fn parse(data: &[u8]) -> Result<Vtk> {
    // Appended raw data is not XML: the tree is read from the bytes before it.
    let (xml_end, appended) = match find(data, b"<AppendedData") {
        Some(at) => {
            let tag_end = data[at..]
                .iter()
                .position(|&b| b == b'>')
                .map(|p| at + p)
                .ok_or_else(|| Error::Truncated("AppendedData".into()))?;
            let enc = attr_in_tag(&data[at..tag_end], b"encoding").unwrap_or_default();
            let us = data[tag_end..]
                .iter()
                .position(|&b| b == b'_')
                .map(|p| tag_end + p + 1)
                .ok_or_else(|| Error::Malformed("AppendedData without '_'".into()))?;
            let b64 = enc != b"raw";
            let mut body = &data[us..];
            if b64 && let Some(e) = find(body, b"</AppendedData") {
                body = &body[..e];
            }
            (at, Some((body, b64)))
        }
        None => (data.len(), None),
    };
    let root = tree(&data[..xml_end])?;
    let ty = root.attr("type").unwrap_or("");
    let kind = match ty {
        "ImageData" => Kind::ImageData,
        "RectilinearGrid" => Kind::RectilinearGrid,
        "StructuredGrid" => Kind::StructuredGrid,
        "UnstructuredGrid" => Kind::UnstructuredGrid,
        "PolyData" => Kind::PolyData,
        t if t.starts_with('P') => {
            return Err(Error::Unsupported(
                "parallel VTK files only list their pieces; open one of the piece files".into(),
            ));
        }
        "vtkMultiBlockDataSet" | "vtkMultiPieceDataSet" | "Collection" => {
            return Err(Error::Unsupported(
                "VTK multiblock files only list other files; open one of them".into(),
            ));
        }
        t => {
            return Err(Error::Unsupported(format!(
                "VTK {t} files are not supported"
            )));
        }
    };
    let comp = match root.attr("compressor") {
        None | Some("") => Comp::None,
        Some("vtkZLibDataCompressor") => Comp::Zlib,
        Some("vtkLZ4DataCompressor") => Comp::Lz4,
        Some("vtkLZMADataCompressor") => {
            return Err(Error::Unsupported(
                "VTK LZMA compression is not supported".into(),
            ));
        }
        Some(c) => {
            return Err(Error::Unsupported(format!(
                "VTK compressor {c} is not supported"
            )));
        }
    };
    let big = root.attr("byte_order") == Some("BigEndian");
    let header64 = match root.attr("header_type") {
        None | Some("UInt32") => false,
        Some("UInt64") => true,
        Some(h) => return bad(format!("header_type {h}")),
    };
    let cx = Ctx {
        data,
        big,
        header64,
        comp,
        appended,
        used: Default::default(),
    };
    let ds = root
        .child(ty)
        .ok_or_else(|| Error::Malformed(format!("no {ty} element")))?;
    let field_data = cx.section(ds.child("FieldData"), None)?;
    let pieces: Vec<&Node> = ds.all("Piece").collect();
    if pieces.len() > limits::MAX_PIECES {
        return Err(Error::TooLarge);
    }
    let dataset = match kind {
        Kind::ImageData | Kind::RectilinearGrid | Kind::StructuredGrid => {
            Dataset::Grid(structured(&cx, kind, ds, &pieces)?)
        }
        _ => Dataset::Mesh(unstructured(&cx, kind, &pieces)?),
    };
    let used = cx.used.borrow();
    let mut encoding: Vec<&str> = used
        .iter()
        .copied()
        .filter(|u| !matches!(*u, "zlib" | "LZ4"))
        .collect();
    if encoding.is_empty() {
        encoding.push("ascii");
    }
    let mut encoding = encoding.join(" and ");
    for c in ["zlib", "LZ4"] {
        if used.contains(c) {
            encoding.push_str(", ");
            encoding.push_str(c);
        }
    }
    if big && used.iter().any(|u| *u != "ascii") {
        encoding.push_str(", big-endian");
    }
    Ok(Vtk {
        kind,
        title: None,
        version: root.attr("version").unwrap_or("").to_owned(),
        encoding,
        dataset,
        field_data,
    })
}

/// Copies a piece's tuples (`pd` points along each axis, at `off` in the whole) into the
/// whole array.
fn place(
    whole: &mut [f32],
    wd: [usize; 3],
    part: &[f32],
    pd: [usize; 3],
    off: [usize; 3],
    c: usize,
) {
    let row = pd[0] * c;
    for z in 0..pd[2] {
        for y in 0..pd[1] {
            let src = (z * pd[1] + y) * row;
            let dst = (((z + off[2]) * wd[1] + y + off[1]) * wd[0] + off[0]) * c;
            if let (Some(s), Some(d)) = (part.get(src..src + row), whole.get_mut(dst..dst + row)) {
                d.copy_from_slice(s);
            }
        }
    }
}

fn structured(cx: &Ctx<'_>, kind: Kind, ds: &Node, pieces: &[&Node]) -> Result<Grid> {
    let whole: [i64; 6] = ints(ds.attr("WholeExtent"), "WholeExtent")?;
    let dims = extent_dims(whole)?;
    let cell = |d: [usize; 3]| d.map(|v| v.saturating_sub(1).max(1));
    let mut g = Grid {
        structure: match kind {
            Kind::ImageData => "ImageData",
            Kind::RectilinearGrid => "RectilinearGrid",
            _ => "StructuredGrid",
        },
        dims,
        ..Grid::default()
    };
    if kind == Kind::ImageData {
        let origin = floats(ds.attr("Origin"), [0.0; 3])?;
        g.spacing = floats(ds.attr("Spacing"), [1.0; 3])?;
        g.direction = ds
            .attr("Direction")
            .map(|_| floats(ds.attr("Direction"), [0.0; 9]))
            .transpose()?;
        // The first point of the whole extent.
        g.origin = [0, 1, 2].map(|k| origin[k] + whole[2 * k] as f64 * g.spacing[k]);
    }
    let n_points: usize = dims.iter().product();
    let n_cells: usize = cell(dims).iter().product();
    let mut coords: [Vec<f64>; 3] = Default::default();
    if kind == Kind::RectilinearGrid {
        for (k, c) in coords.iter_mut().enumerate() {
            *c = vec![f64::NAN; dims[k]];
        }
    }
    let mut points: Vec<f32> = Vec::new();
    let single = pieces.len() == 1;
    // Pieces must cover the whole extent: the arrays allocated for it then hold no more
    // than the pieces' own data.
    let mut covered = 0u64;
    for p in pieces {
        let e: [i64; 6] = ints(p.attr("Extent"), "Extent")?;
        covered = covered.saturating_add(extent_dims(e)?.iter().map(|&d| d as u64).product());
    }
    if covered < n_points as u64 {
        return bad("the pieces do not cover the whole extent");
    }
    for (pi, p) in pieces.iter().enumerate() {
        let e: [i64; 6] = ints(p.attr("Extent"), "Extent")?;
        let pd = extent_dims(e)?;
        let mut off = [0usize; 3];
        for k in 0..3 {
            let o = e[2 * k] - whole[2 * k];
            if o < 0 || o as usize + pd[k] > dims[k] {
                return bad("piece extent outside the whole extent");
            }
            off[k] = o as usize;
        }
        let np: usize = pd.iter().product();
        let nc: usize = cell(pd).iter().product();
        let pdata = cx.section(p.child("PointData"), Some(np as u64))?;
        let cdata = cx.section(p.child("CellData"), Some(nc as u64))?;
        let coff = [0, 1, 2].map(|k| if dims[k] > 1 { off[k] } else { 0 });
        if pi == 0 && single && pd == dims {
            g.point_data = pdata;
            g.cell_data = cdata;
        } else {
            if pi == 0 {
                for (src, dst, n) in [
                    (&pdata, &mut g.point_data, n_points),
                    (&cdata, &mut g.cell_data, n_cells),
                ] {
                    for a in src {
                        let mut v = Vec::new();
                        let len = n.checked_mul(a.components).ok_or(Error::TooLarge)?;
                        num::reserve(&mut v, len as u64)?;
                        v.resize(len, f32::NAN);
                        dst.push(DataArray {
                            name: a.name.clone(),
                            components: a.components,
                            dtype: a.dtype,
                            values: v,
                            attribute: a.attribute,
                        });
                    }
                }
            }
            for a in &pdata {
                if let Some(w) = g
                    .point_data
                    .iter_mut()
                    .find(|w| w.name == a.name && w.components == a.components)
                {
                    place(&mut w.values, dims, &a.values, pd, off, a.components);
                }
            }
            for a in &cdata {
                if let Some(w) = g
                    .cell_data
                    .iter_mut()
                    .find(|w| w.name == a.name && w.components == a.components)
                {
                    place(
                        &mut w.values,
                        cell(dims),
                        &a.values,
                        cell(pd),
                        coff,
                        a.components,
                    );
                }
            }
        }
        match kind {
            Kind::RectilinearGrid => {
                let cn = p
                    .child("Coordinates")
                    .ok_or_else(|| Error::Malformed("no Coordinates".into()))?;
                let arrs: Vec<&Node> = cn.arrays().collect();
                if arrs.len() < 3 {
                    return bad("Coordinates needs three arrays");
                }
                for k in 0..3 {
                    let v = cx.f64s(arrs[k], Some(pd[k] as u64))?;
                    coords[k][off[k]..off[k] + pd[k]].copy_from_slice(&v);
                }
            }
            Kind::StructuredGrid => {
                let a = p
                    .child("Points")
                    .and_then(|n| n.arrays().next())
                    .ok_or_else(|| Error::Malformed("no Points".into()))?;
                let v = cx.f32s(a, Some(np as u64 * 3))?;
                if single && pd == dims {
                    points = v;
                } else {
                    if points.is_empty() {
                        num::reserve(&mut points, n_points as u64 * 3)?;
                        points.resize(n_points * 3, f32::NAN);
                    }
                    place(&mut points, dims, &v, pd, off, 3);
                }
            }
            _ => {}
        }
    }
    match kind {
        Kind::RectilinearGrid => g.coords = Some(coords),
        Kind::StructuredGrid => {
            g.points = Some(points.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
        }
        _ => {}
    }
    Ok(g)
}

fn count(p: &Node, k: &str) -> Result<u64> {
    let v = p.attr(k).unwrap_or("0").trim();
    let n: u64 = v
        .parse()
        .map_err(|_| Error::Malformed(format!("{k} {v}")))?;
    if n > limits::MAX_VALUES {
        return Err(Error::TooLarge);
    }
    Ok(n)
}

/// A `Verts`/`Lines`/`Polys`/`Strips`/`Cells` section's connectivity and offsets.
fn cell_array(cx: &Ctx<'_>, sec: Option<&Node>, n: u64) -> Result<CellArray> {
    let Some(sec) = sec else {
        if n > 0 {
            return bad("cells listed but missing");
        }
        return Ok(CellArray::default());
    };
    if n == 0 {
        return Ok(CellArray::default());
    }
    let conn = sec
        .array("connectivity")
        .ok_or_else(|| Error::Malformed("no connectivity".into()))?;
    let offs = sec
        .array("offsets")
        .ok_or_else(|| Error::Malformed("no offsets".into()))?;
    let ends = cx.i64s(offs, Some(n))?;
    let total = ends.last().copied().unwrap_or(0);
    let total = u64::try_from(total).map_err(|_| Error::Malformed("offsets".into()))?;
    let conn = cx.i64s(conn, Some(total))?;
    CellArray::from_ends(ends, conn)
}

fn unstructured(cx: &Ctx<'_>, kind: Kind, pieces: &[&Node]) -> Result<MeshData> {
    let mut m = MeshData::default();
    let mut notes = BTreeSet::new();
    // Polygonal data: each piece's vertex, line, polygon and strip counts.
    let mut kinds: Vec<[u64; 4]> = Vec::new();
    for p in pieces {
        let np = count(p, "NumberOfPoints")?;
        let base = m.points.len() as u64;
        if base + np > u64::from(u32::MAX) {
            return Err(Error::TooLarge);
        }
        if np > 0 {
            let a = p
                .child("Points")
                .and_then(|n| n.arrays().next())
                .ok_or_else(|| Error::Malformed("no Points".into()))?;
            let v = cx.f32s(a, Some(np * 3))?;
            num::reserve(&mut m.points, np)?;
            m.points
                .extend(v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]));
        }
        let pdata = cx.section(p.child("PointData"), Some(np))?;
        let t: Triangles = if kind == Kind::PolyData {
            let verts = cell_array(cx, p.child("Verts"), count(p, "NumberOfVerts")?)?;
            let lines = cell_array(cx, p.child("Lines"), count(p, "NumberOfLines")?)?;
            let strips = cell_array(cx, p.child("Strips"), count(p, "NumberOfStrips")?)?;
            let polys = cell_array(cx, p.child("Polys"), count(p, "NumberOfPolys")?)?;
            kinds.push([verts.len(), lines.len(), polys.len(), strips.len()].map(|n| n as u64));
            cells::triangulate(
                np as usize,
                Cells::Poly {
                    verts: &verts,
                    lines: &lines,
                    polys: &polys,
                    strips: &strips,
                },
            )?
        } else {
            let nc = count(p, "NumberOfCells")?;
            let sec = p.child("Cells");
            let ca = cell_array(cx, sec, nc)?;
            let types = match sec.and_then(|s| s.array("types")) {
                Some(t) => cx.i64s(t, Some(nc))?,
                None if nc == 0 => Vec::new(),
                None => return bad("no cell types"),
            };
            let polyhedra = match sec {
                Some(s) if s.array("face_connectivity").is_some() => {
                    let fc = s.array("face_connectivity");
                    let fo = s.array("face_offsets");
                    let pf = s.array("polyhedron_to_faces");
                    let po = s.array("polyhedron_offsets");
                    let (Some(fc), Some(fo), Some(pf), Some(po)) = (fc, fo, pf, po) else {
                        return bad("incomplete polyhedron faces");
                    };
                    let faces = CellArray::from_ends(cx.i64s(fo, None)?, cx.i64s(fc, None)?)?;
                    let cell_faces = CellArray::from_ends(cx.i64s(po, None)?, cx.i64s(pf, None)?)?;
                    Polyhedra::Lists { faces, cell_faces }
                }
                Some(s) if s.array("faces").is_some() => {
                    let f = s.array("faces");
                    let fo = s.array("faceoffsets");
                    let (Some(f), Some(fo)) = (f, fo) else {
                        return bad("incomplete polyhedron faces");
                    };
                    Polyhedra::Stream {
                        faces: cx.i64s(f, None)?,
                        ends: cx.i64s(fo, None)?,
                    }
                }
                _ => Polyhedra::InCells,
            };
            cells::triangulate(
                np as usize,
                Cells::Unstructured {
                    cells: &ca,
                    types: &types,
                    polyhedra: &polyhedra,
                },
            )?
        };
        let cdata = cx.section(p.child("CellData"), Some(t.cells))?;
        let cbase = u32::try_from(m.cells).map_err(|_| Error::TooLarge)?;
        let b = base as u32;
        m.triangles
            .extend(t.triangles.iter().map(|tr| tr.map(|i| i + b)));
        m.triangle_cell
            .extend(t.triangle_cell.iter().map(|&c| c + cbase));
        m.cells += t.cells;
        m.vertices += t.vertices;
        m.lines += t.lines;
        notes.extend(t.notes);
        merge(&mut m.point_data, pdata);
        merge(&mut m.cell_data, cdata);
    }
    m.notes = notes.into_iter().collect();
    if kinds.len() > 1 {
        poly_order(&mut m, &kinds);
    }
    Ok(m)
}

/// VTK puts the cells of polygonal pieces in one list per kind (all pieces' vertices, then
/// lines, polygons and strips): renumbers the cells and reorders their arrays to match.
fn poly_order(m: &mut MeshData, kinds: &[[u64; 4]]) {
    let mut totals = [0u64; 4];
    for k in kinds {
        for (t, n) in totals.iter_mut().zip(k) {
            *t += n;
        }
    }
    let mut map = Vec::with_capacity(m.cells as usize);
    let mut seen = [0u64; 4];
    for k in kinds {
        for kind in 0..4 {
            let start: u64 = totals[..kind].iter().sum::<u64>() + seen[kind];
            map.extend((0..k[kind]).map(|j| (start + j) as u32));
            seen[kind] += k[kind];
        }
    }
    for c in &mut m.triangle_cell {
        *c = map.get(*c as usize).copied().unwrap_or(*c);
    }
    for a in &mut m.cell_data {
        let n = a.components.max(1);
        if a.values.len() != map.len() * n {
            continue;
        }
        let mut v = vec![0f32; a.values.len()];
        for (i, &to) in map.iter().enumerate() {
            let to = to as usize;
            v[to * n..to * n + n].copy_from_slice(&a.values[i * n..i * n + n]);
        }
        a.values = v;
    }
}

/// Appends a piece's arrays to the arrays of the pieces before it (by name; the first
/// piece's set is kept).
fn merge(into: &mut Vec<DataArray>, more: Vec<DataArray>) {
    if into.is_empty() {
        *into = more;
        return;
    }
    for a in into.iter_mut() {
        match more
            .iter()
            .find(|b| b.name == a.name && b.components == a.components)
        {
            Some(b) => a.values.extend_from_slice(&b.values),
            None => {
                let n = more.first().map_or(0, DataArray::tuples) * a.components;
                a.values.extend(std::iter::repeat_n(f32::NAN, n));
            }
        }
    }
}

#[cfg(test)]
mod bomb_tests {
    use super::*;

    /// One zlib block claiming 4 GiB from 10 compressed bytes (the block-size slack let any
    /// size through), and compressed sizes past the end of the data: refused before the
    /// output is reserved.
    #[test]
    fn block_size_bomb() {
        let cx = Ctx {
            data: &[],
            big: false,
            header64: false,
            comp: Comp::Zlib,
            appended: None,
            used: Default::default(),
        };
        let h = [1u32, u32::MAX, 0, 10].map(u32::to_le_bytes).concat();
        let r = cx.inflate(&h, &[0u8; 10]);
        assert!(matches!(r, Err(Error::TooLarge)), "{r:?}");
        let h = [1u32, 64, 0, 1 << 30].map(u32::to_le_bytes).concat();
        let r = cx.inflate(&h, &[0u8; 10]);
        assert!(matches!(r, Err(Error::Truncated(_))), "{r:?}");
    }
}

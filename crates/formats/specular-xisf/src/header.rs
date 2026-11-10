//! The XML header: images, FITS keywords, properties, data block locations.

use std::collections::HashMap;

use roxmltree::{Document, Node};

use crate::{
    ByteOrder, Codec, ColorSpace, Compression, Error, FitsKeyword, Image, Location, MAX_BLOCK,
    MAX_SAMPLES, Property, PropertyValue, Result, SampleFormat, Storage, Xisf,
};

/// Largest XML header accepted.
const MAX_HEADER: usize = 256 << 20;

pub(crate) fn parse(data: &[u8]) -> Result<Xisf> {
    if !crate::sniff(data) {
        return Err(Error::NotXisf);
    }
    let len = data
        .get(8..12)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or(Error::Truncated("header length"))?;
    if len > MAX_HEADER {
        return Err(Error::TooLarge);
    }
    let xml = data
        .get(16..16 + len)
        .ok_or(Error::Truncated("XML header"))?;
    let xml = std::str::from_utf8(xml).map_err(|_| Error::Malformed("header not UTF-8".into()))?;
    let xml = xml.trim_end_matches(['\0', ' ', '\n', '\r', '\t']);
    // roxmltree recurses once per element level: a deeply nested header overflows the stack.
    if !specular_limits::xml_within_depth(xml.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err(Error::Malformed("XML nested too deeply".into()));
    }
    let doc = Document::parse(xml).map_err(|e| Error::Malformed(format!("XML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "xisf" {
        return Err(Error::Malformed("root element is not <xisf>".into()));
    }
    let uids = Uids::new(&doc);
    let mut out = Xisf::default();
    for n in children(&uids, root) {
        match n.tag_name().name() {
            "Image" => out.images.push(image(&uids, n, data)?),
            "Property" => out.properties.extend(property(n, data)),
            "Metadata" => {
                for p in children(&uids, n).filter(|p| p.tag_name().name() == "Property") {
                    out.properties.extend(property(p, data));
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The first element (in document order) of each `uid`, `<Reference>`s aside: found once,
/// as a document search per reference was quadratic in a header of many references.
///
/// Each resolved reference is charged the size of the element it stands for: references
/// that all name one large embedded block would decode it once each (10,000 references to a
/// 1 MB block were 7.5 GB). Past four times the header, references resolve to nothing.
struct Uids<'a, 'i>(HashMap<&'a str, Node<'a, 'i>>, std::cell::Cell<usize>);

impl<'a, 'i> Uids<'a, 'i> {
    fn new(doc: &'a Document<'i>) -> Self {
        let mut map = HashMap::new();
        for d in doc.descendants() {
            if !d.is_element() || d.tag_name().name() == "Reference" {
                continue;
            }
            if let Some(u) = d.attribute("uid") {
                map.entry(u).or_insert(d);
            }
        }
        let allowance = doc.input_text().len().saturating_mul(4).max(1 << 20);
        Uids(map, std::cell::Cell::new(allowance))
    }

    /// The element `uid` names, while the references' allowance lasts.
    fn resolve(&self, uid: &str) -> Option<Node<'a, 'i>> {
        let n = *self.0.get(uid)?;
        let left = self.1.get().checked_sub(n.range().len())?;
        self.1.set(left);
        Some(n)
    }
}

/// Element children, with `<Reference ref="…"/>` replaced by the element of that `uid`.
fn children<'a, 'i>(uids: &Uids<'a, 'i>, n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children()
        .filter(|c| c.is_element())
        .filter_map(move |c| {
            if c.tag_name().name() == "Reference" {
                uids.resolve(c.attribute("ref")?)
            } else {
                Some(c)
            }
        })
}

fn malformed(m: impl Into<String>) -> Error {
    Error::Malformed(m.into())
}

fn image(uids: &Uids<'_, '_>, n: Node<'_, '_>, data: &[u8]) -> Result<Image> {
    let geometry: Vec<u32> = n
        .attribute("geometry")
        .ok_or_else(|| malformed("image without geometry"))?
        .split(':')
        .map(|s| s.trim().parse::<u32>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| malformed("bad geometry"))?;
    if geometry.len() < 2 || geometry.contains(&0) {
        return Err(malformed("bad geometry"));
    }
    let (dims, channels) = geometry.split_at(geometry.len() - 1);
    let channels = channels[0];
    let width = dims[0];
    let height = dims.get(1).copied().unwrap_or(1);
    let depth = dims
        .iter()
        .skip(2)
        .try_fold(1u64, |a, &d| {
            a.checked_mul(u64::from(d)).filter(|&v| v <= MAX_SAMPLES)
        })
        .ok_or(Error::TooLarge)?;
    [width, height, channels]
        .iter()
        .try_fold(depth, |a, &d| a.checked_mul(u64::from(d)))
        .filter(|&v| v <= MAX_SAMPLES)
        .ok_or(Error::TooLarge)?;
    let sample = match n.attribute("sampleFormat").unwrap_or("") {
        "UInt8" => SampleFormat::UInt8,
        "UInt16" => SampleFormat::UInt16,
        "UInt32" => SampleFormat::UInt32,
        "UInt64" => SampleFormat::UInt64,
        "Float32" => SampleFormat::Float32,
        "Float64" => SampleFormat::Float64,
        "Complex32" | "Complex64" => {
            return Err(Error::Unsupported("complex samples".into()));
        }
        s => return Err(malformed(format!("sample format {s:?}"))),
    };
    let storage = match n.attribute("pixelStorage") {
        None | Some("Planar") => Storage::Planar,
        Some("Normal") => Storage::Normal,
        Some(s) => return Err(malformed(format!("pixel storage {s:?}"))),
    };
    let color_space = match n.attribute("colorSpace") {
        None | Some("Gray") => ColorSpace::Gray,
        Some("RGB") => ColorSpace::Rgb,
        Some("CIELab") => ColorSpace::CieLab,
        Some(s) => return Err(Error::Unsupported(format!("colour space {s:?}"))),
    };
    let bounds = n.attribute("bounds").and_then(|b| {
        let (lo, hi) = b.split_once(':')?;
        Some((lo.trim().parse().ok()?, hi.trim().parse().ok()?))
    });
    let (location, compression) = block(n)?;
    let mut img = Image {
        geometry: dims.to_vec(),
        width,
        height,
        depth: depth as u32,
        channels,
        sample,
        storage,
        color_space,
        byte_order: byte_order(n),
        bounds,
        location,
        compression,
        fits_keywords: Vec::new(),
        properties: Vec::new(),
        image_type: n.attribute("imageType").map(str::to_owned),
        offset: n.attribute("offset").map(str::to_owned),
        orientation: n.attribute("orientation").map(str::to_owned),
        id: n.attribute("id").map(str::to_owned),
        color_filter: None,
    };
    for c in children(uids, n) {
        match c.tag_name().name() {
            "FITSKeyword" => img.fits_keywords.push(FitsKeyword {
                name: c.attribute("name").unwrap_or("").to_owned(),
                value: c.attribute("value").unwrap_or("").to_owned(),
                comment: c.attribute("comment").unwrap_or("").to_owned(),
            }),
            "Property" => img.properties.extend(property(c, data)),
            "ColorFilterArray" => {
                let size = |a: &str| c.attribute(a).and_then(|v| v.trim().parse::<u32>().ok());
                if let (Some(p), Some(w), Some(h)) =
                    (c.attribute("pattern"), size("width"), size("height"))
                {
                    img.color_filter = Some((p.trim().to_owned(), w, h));
                }
            }
            _ => {}
        }
    }
    Ok(img)
}

fn byte_order(n: Node<'_, '_>) -> ByteOrder {
    match n.attribute("byteOrder") {
        Some("big") => ByteOrder::Big,
        _ => ByteOrder::Little,
    }
}

/// A data block's location and compression (`location`, `compression`, `subblocks`
/// attributes, or a `<Data>` child for embedded blocks).
fn block(n: Node<'_, '_>) -> Result<(Location, Option<Compression>)> {
    let loc = n
        .attribute("location")
        .ok_or_else(|| malformed("data block without a location"))?;
    let mut parts = loc.split(':');
    let mut compressed_on = n;
    let location = match parts.next().unwrap_or("") {
        "attachment" => {
            let mut num = || -> Result<u64> {
                parts
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .ok_or_else(|| malformed("bad attachment location"))
            };
            let (offset, len) = (num()?, num()?);
            if len > MAX_BLOCK {
                return Err(Error::TooLarge);
            }
            Location::Attachment { offset, len }
        }
        "inline" => {
            let enc = parts.next().unwrap_or("base64");
            Location::Inline(decode_text(enc, &text_of(n))?)
        }
        "embedded" => {
            let d = n
                .children()
                .find(|c| c.is_element() && c.tag_name().name() == "Data")
                .ok_or_else(|| malformed("embedded block without <Data>"))?;
            if d.attribute("compression").is_some() {
                compressed_on = d;
            }
            let enc = d.attribute("encoding").unwrap_or("base64");
            Location::Embedded(decode_text(enc, &text_of(d))?)
        }
        s => return Err(Error::Unsupported(format!("block location {s:?}"))),
    };
    let compression = compressed_on
        .attribute("compression")
        .map(|c| compression(c, compressed_on.attribute("subblocks")))
        .transpose()?;
    Ok((location, compression))
}

fn text_of(n: Node<'_, '_>) -> String {
    n.children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect()
}

/// Parses `codec[+sh]:uncompressed-size[:item-size]` and `c,u:c,u…`.
fn compression(attr: &str, subblocks: Option<&str>) -> Result<Compression> {
    let mut parts = attr.split(':');
    let name = parts.next().unwrap_or("");
    let (name, shuffled) = match name.strip_suffix("+sh") {
        Some(n) => (n, true),
        None => (name, false),
    };
    let codec = match name {
        "zlib" => Codec::Zlib,
        "lz4" => Codec::Lz4,
        "lz4hc" => Codec::Lz4Hc,
        "zstd" => Codec::Zstd,
        s => return Err(Error::Unsupported(format!("compression codec {s:?}"))),
    };
    let uncompressed: u64 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| malformed("bad compression attribute"))?;
    let item: Option<u32> = parts.next().and_then(|s| s.trim().parse().ok());
    let shuffle_item = if shuffled {
        Some(item.ok_or_else(|| malformed("byte shuffling without an item size"))?)
    } else {
        None
    };
    let subblocks = match subblocks {
        Some(s) => s
            .split(':')
            .filter(|p| !p.trim().is_empty())
            .map(|p| {
                let (c, u) = p.split_once(',')?;
                Some((c.trim().parse().ok()?, u.trim().parse().ok()?))
            })
            .collect::<Option<Vec<(u64, u64)>>>()
            .ok_or_else(|| malformed("bad subblocks attribute"))?,
        None => Vec::new(),
    };
    Ok(Compression {
        codec,
        uncompressed,
        shuffle_item,
        subblocks,
    })
}

fn decode_text(enc: &str, t: &str) -> Result<Vec<u8>> {
    match enc {
        "base64" => base64(t),
        "hex" => hex(t),
        s => Err(Error::Unsupported(format!("block encoding {s:?}"))),
    }
}

fn base64(t: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in t.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return Err(malformed("bad base64")),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn hex(t: &str) -> Result<Vec<u8>> {
    let digits: Vec<u8> = t
        .bytes()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(malformed("bad hex")),
        })
        .collect::<Result<_>>()?;
    if !digits.len().is_multiple_of(2) {
        return Err(malformed("odd hex length"));
    }
    Ok(digits.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

/// A `<Property>`; `None` when it has no id or type.
fn property(n: Node<'_, '_>, data: &[u8]) -> Option<Property> {
    let id = n.attribute("id")?.to_owned();
    let kind = n.attribute("type")?.to_owned();
    let attr = n.attribute("value");
    let value = match kind.as_str() {
        "Boolean" => PropertyValue::Bool(matches!(attr.map(str::trim), Some("true" | "1"))),
        "TimePoint" => PropertyValue::TimePoint(attr.unwrap_or("").to_owned()),
        "String" => {
            let s = match (attr, n.attribute("location")) {
                (Some(v), _) => v.to_owned(),
                (None, Some(_)) => block_bytes(n, data)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default(),
                (None, None) => text_of(n),
            };
            PropertyValue::String(s)
        }
        "F32Vector" | "F64Vector" | "F32Matrix" | "F64Matrix" => {
            float_block(n, data, &kind).unwrap_or_else(|| PropertyValue::Other(dims(n)))
        }
        "Int8" | "UInt8" | "Byte" | "UByte" | "Int16" | "UInt16" | "Short" | "UShort" | "Int32"
        | "UInt32" | "Int" | "UInt" | "Int64" | "UInt64" | "Float32" | "Float64" | "Float"
        | "Double" => match attr.and_then(|v| v.trim().parse::<f64>().ok()) {
            Some(x) => PropertyValue::Number(x),
            None => PropertyValue::Other(attr.unwrap_or("").to_owned()),
        },
        _ => PropertyValue::Other(match attr {
            Some(v) => v.to_owned(),
            None => dims(n),
        }),
    };
    Some(Property { id, kind, value })
}

/// A vector's or matrix's stated size, as `length=3` or `rows=2 columns=2`.
fn dims(n: Node<'_, '_>) -> String {
    ["length", "rows", "columns"]
        .iter()
        .filter_map(|a| n.attribute(*a).map(|v| format!("{a}={v}")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The bytes of a property's uncompressed data block.
fn block_bytes(n: Node<'_, '_>, data: &[u8]) -> Option<Vec<u8>> {
    let (loc, comp) = block(n).ok()?;
    if comp.is_some() {
        return None;
    }
    Some(match loc {
        Location::Attachment { offset, len } => data
            .get(usize::try_from(offset).ok()?..usize::try_from(offset.checked_add(len)?).ok()?)?
            .to_vec(),
        Location::Inline(v) | Location::Embedded(v) => v,
    })
}

/// A float vector or matrix property: its block (little-endian unless `byteOrder="big"`)
/// must hold exactly the stated `length` or `rows` × `columns` elements.
fn float_block(n: Node<'_, '_>, data: &[u8], kind: &str) -> Option<PropertyValue> {
    let size = |a: &str| n.attribute(a)?.trim().parse::<usize>().ok();
    let matrix = kind.ends_with("Matrix");
    let (rows, columns) = if matrix {
        (size("rows")?, size("columns")?)
    } else {
        (1, size("length")?)
    };
    let count = rows.checked_mul(columns)?;
    let bytes = block_bytes(n, data)?;
    let width = if kind.starts_with("F32") { 4 } else { 8 };
    if count.checked_mul(width)? != bytes.len() {
        return None;
    }
    let big = n.attribute("byteOrder") == Some("big");
    let values: Vec<f64> = bytes
        .chunks_exact(width)
        .map(|c| match (width, big) {
            (4, false) => f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]])),
            (4, true) => f64::from(f32::from_be_bytes([c[0], c[1], c[2], c[3]])),
            (_, false) => f64::from_le_bytes(c.try_into().unwrap_or_default()),
            (_, true) => f64::from_be_bytes(c.try_into().unwrap_or_default()),
        })
        .collect();
    Some(if matrix {
        PropertyValue::Matrix {
            rows,
            columns,
            values,
        }
    } else {
        PropertyValue::Vector(values)
    })
}

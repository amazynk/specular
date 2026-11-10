//! MRtrix images (`.mif`, gzip-compressed `.mif.gz`, and `.mih` headers with their data in
//! another file), from MRtrix's file format documentation.
//!
//! A text header: `mrtrix image`, then `key: value` lines up to `END`. `dim` gives every
//! axis's size, `vox` the voxel sizes, `layout` each axis's place in the storage order
//! (`+0` fastest; a `-` axis is stored backwards), `datatype` the sample type and byte
//! order, three `transform` lines the image-to-scanner rotation and translation (RAS
//! millimetres, without the voxel sizes), `scaling` an offset and a scale, and `file` where
//! the data is: `.` and an offset into this file, or another file's name and an offset.
//!
//! The image's axes are kept as the layout stores them: an axis stored backwards is read
//! forwards and its affine column is turned around instead, so the voxels are never copied.

use std::borrow::Cow;
use std::path::PathBuf;

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Param,
    ParamValue, Result, Skip,
};

const MAGIC: &[u8] = b"mrtrix image";
const GZIP: [u8; 2] = [0x1f, 0x8b];
/// Longest header accepted (MRtrix headers carry the command history and diffusion
/// schemes: a few hundred kilobytes at most).
const MAX_HEADER: usize = 4 << 20;
/// Most axes accepted.
const MAX_AXES: usize = 16;

/// Whether `data` (a file's first bytes, or a whole `.mif.gz`) is an MRtrix image header.
pub fn sniff(data: &[u8]) -> bool {
    if data.starts_with(&GZIP) {
        return specular_png::inflate_gzip_prefix(data, MAGIC.len() + 1).is_ok_and(|b| magic(&b));
    }
    magic(data)
}

fn magic(b: &[u8]) -> bool {
    b.starts_with(MAGIC) && matches!(b.get(MAGIC.len()), Some(b'\n' | b'\r'))
}

/// The header's text (up to `END`), from the file's bytes (inflated first for gzip).
fn text(data: &[u8]) -> Result<Cow<'_, [u8]>> {
    let b: Cow<'_, [u8]> = if data.starts_with(&GZIP) {
        // The header is short; inflate a prefix, growing it until `END` shows.
        let mut n = 1 << 16;
        loop {
            let b = specular_png::inflate_gzip_prefix(data, n)
                .map_err(|e| Error::Malformed(format!("gzip stream: {e}")))?;
            if find_end(&b).is_some() || b.len() < n || n >= MAX_HEADER {
                break Cow::Owned(b);
            }
            n *= 4;
        }
    } else {
        Cow::Borrowed(data)
    };
    let end = find_end(&b).ok_or_else(|| Error::Truncated("MRtrix header".into()))?;
    Ok(match b {
        Cow::Borrowed(s) => Cow::Borrowed(&s[..end]),
        Cow::Owned(mut v) => {
            v.truncate(end);
            Cow::Owned(v)
        }
    })
}

/// Where the line `END` starts.
fn find_end(b: &[u8]) -> Option<usize> {
    let b = &b[..b.len().min(MAX_HEADER)];
    let mut at = 0;
    while at < b.len() {
        let stop = b[at..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(b.len(), |p| at + p);
        let line = &b[at..stop];
        if line.strip_suffix(b"\r").unwrap_or(line) == b"END" {
            return Some(at);
        }
        at = stop + 1;
    }
    None
}

fn dtype(name: &str) -> Result<(DataType, bool)> {
    let (base, big) = match name.len().checked_sub(2).map(|n| name.split_at(n)) {
        Some((b, "LE")) => (b, false),
        Some((b, "BE")) => (b, true),
        // No suffix: one-byte types, or the writer's own order (little-endian machines).
        _ => (name, false),
    };
    let t = match base {
        "UInt8" => DataType::U8,
        "Int8" => DataType::I8,
        "UInt16" => DataType::U16,
        "Int16" => DataType::I16,
        "UInt32" => DataType::U32,
        "Int32" => DataType::I32,
        "UInt64" => DataType::U64,
        "Int64" => DataType::I64,
        "Float32" => DataType::F32,
        "Float64" => DataType::F64,
        "Bit" => {
            return Err(Error::Unsupported(
                "MRtrix bit images are not supported".into(),
            ));
        }
        t if t.starts_with("CFloat") => {
            return Err(Error::Unsupported(
                "MRtrix complex images are not supported".into(),
            ));
        }
        _ => return Err(Error::Malformed(format!("MRtrix datatype {name}"))),
    };
    Ok((t, big))
}

/// Parses the header of an MRtrix image (the whole file for a `.mif.gz`).
pub fn parse(data: &[u8]) -> Result<Header> {
    let gz = data.starts_with(&GZIP);
    let head = text(data)?;
    let head = String::from_utf8_lossy(&head);
    let mut lines = head.lines();
    if lines.next().map(str::trim_end) != Some("mrtrix image") {
        return Err(Error::NotRecognised);
    }
    let mut dims: Option<Vec<usize>> = None;
    let mut vox: Option<Vec<f64>> = None;
    let mut layout: Option<Vec<(bool, usize)>> = None;
    let mut dt = None;
    let mut transform: Vec<[f64; 4]> = Vec::new();
    let mut scaling = None;
    let mut files: Vec<(String, usize)> = Vec::new();
    let mut comments = Vec::new();
    let mut history = Vec::new();
    let bad = |what: &str| Error::Malformed(format!("MRtrix {what}"));
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "dim" => {
                let d: Option<Vec<usize>> = v.split(',').map(|x| x.trim().parse().ok()).collect();
                dims = Some(
                    d.filter(|d| (1..=MAX_AXES).contains(&d.len()))
                        .ok_or_else(|| bad("dim"))?,
                );
            }
            "vox" => vox = Some(crate::numbers(v).ok_or_else(|| bad("vox"))?),
            "layout" => {
                let l: Option<Vec<(bool, usize)>> = v
                    .split(',')
                    .map(|x| {
                        let x = x.trim();
                        let (back, r) = match x.as_bytes().first() {
                            Some(b'-') => (true, &x[1..]),
                            Some(b'+') => (false, &x[1..]),
                            _ => (false, x),
                        };
                        Some((back, r.parse().ok()?))
                    })
                    .collect();
                layout = Some(l.ok_or_else(|| bad("layout"))?);
            }
            "datatype" => dt = Some(dtype(v)?),
            "transform" => {
                let t = crate::numbers(v)
                    .filter(|t| t.len() == 4)
                    .ok_or_else(|| bad("transform"))?;
                transform.push([t[0], t[1], t[2], t[3]]);
            }
            "scaling" => {
                let s = crate::numbers(v)
                    .filter(|s| s.len() == 2)
                    .ok_or_else(|| bad("scaling"))?;
                scaling = Some((s[0], s[1]));
            }
            "file" => {
                let mut it = v.split_whitespace();
                let name = it.next().ok_or_else(|| bad("file"))?.to_owned();
                let off = match it.next() {
                    Some(o) => o.parse().map_err(|_| bad("file offset"))?,
                    None => 0,
                };
                files.push((name, off));
            }
            "comments" => comments.push(v.to_owned()),
            "command_history" => history.push(v.to_owned()),
            _ => {}
        }
    }
    let dims = dims.ok_or_else(|| bad("header: no dim"))?;
    let nd = dims.len();
    if dims.contains(&0) {
        return Err(bad("dim"));
    }
    let vox = vox.ok_or_else(|| bad("header: no vox"))?;
    if vox.len() < nd.min(3) {
        return Err(bad("vox"));
    }
    let (dtype, big_endian) = dt.ok_or_else(|| bad("header: no datatype"))?;
    let layout = match layout {
        Some(l) if l.len() == nd => l,
        Some(_) => return Err(bad("layout")),
        None => (0..nd).map(|a| (false, a)).collect(),
    };
    // Strides from the ranks: the lowest rank is fastest (ties in axis order).
    let mut order: Vec<usize> = (0..nd).collect();
    order.sort_by_key(|&a| (layout[a].1, a));
    let mut strides = vec![0usize; nd];
    let mut s = 1usize;
    for &a in &order {
        strides[a] = s;
        s = s.checked_mul(dims[a]).ok_or(Error::TooLarge)?;
    }
    let samples = s;
    if (3..nd).any(|a| layout[a].0 && dims[a] > 1) {
        return Err(Error::Unsupported(
            "MRtrix images stored backwards along a volume axis are not supported".into(),
        ));
    }
    let size = [0, 1, 2].map(|a| dims.get(a).copied().unwrap_or(1));
    let stride = [0, 1, 2].map(|a| strides.get(a).copied().unwrap_or(samples));
    let extra: Vec<(usize, usize)> = (3..nd).map(|a| (dims[a], strides[a])).collect();
    let spacing = [0, 1, 2].map(|a| {
        vox.get(a)
            .copied()
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0)
    });

    // Image voxel to scanner RAS, then turned to the stored order and to LPS.
    let t = match transform.len() {
        0 => [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        3 => [transform[0], transform[1], transform[2]],
        _ => return Err(bad("transform")),
    };
    if !t.iter().flatten().all(|x| x.is_finite()) {
        return Err(bad("transform"));
    }
    let mut affine = [[0.0; 4]; 3];
    for r in 0..3 {
        for c in 0..3 {
            affine[r][c] = t[r][c] * spacing[c];
        }
        affine[r][3] = t[r][3];
    }
    for c in 0..3.min(nd) {
        if layout[c].0 {
            let far = (size[c] - 1) as f64;
            for row in &mut affine {
                row[3] += row[c] * far;
                row[c] = -row[c];
            }
        }
    }
    for row in &mut affine[..2] {
        for v in row.iter_mut() {
            *v = -*v;
        }
    }

    let (file, offset) = match files.as_slice() {
        [] => return Err(bad("header: no file")),
        [(name, off)] if name == "." => (DataFile::Attached, *off),
        [(name, off)] => {
            if gz {
                return Err(Error::Unsupported(
                    "a compressed MRtrix header with its data in another file".into(),
                ));
            }
            (DataFile::Detached(vec![PathBuf::from(name)]), *off)
        }
        _ => {
            return Err(Error::Unsupported(
                "MRtrix images split over several files are not supported".into(),
            ));
        }
    };
    if file == DataFile::Attached && !gz && offset < find_end(data).unwrap_or(usize::MAX) {
        return Err(bad("file offset"));
    }
    let mut params = Vec::new();
    if !history.is_empty() {
        params.push(Param {
            key: "cmdline",
            value: ParamValue::Text(history.join("\n")),
        });
    }
    Ok(Header {
        kind: Kind::Mif,
        dtype,
        layout: Layout {
            size,
            stride,
            extra,
            colour: None,
            samples,
        },
        affine,
        orientation: OrientationSource::Transform,
        dims,
        spacing,
        // MRtrix: value = offset + scale × stored.
        scale: scaling.map(|(offset, scale)| (scale, offset)),
        cal: None,
        description: (!comments.is_empty()).then(|| comments.join("\n")),
        intent: None,
        anatomical: None,
        params,
        data: DataRef {
            file,
            skip: Skip::Start {
                lines: 0,
                bytes: offset,
            },
            encoding: if gz { Encoding::Gzip } else { Encoding::Raw },
            big_endian,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(head: &str, data: &[u8]) -> Vec<u8> {
        let mut b = format!("mrtrix image\n{head}file: . 256\nEND\n").into_bytes();
        b.resize(256, 0);
        b.extend_from_slice(data);
        b
    }

    #[test]
    fn layout_and_affine() {
        let f = file(
            "dim: 2,3,4\nvox: 1,2,3\nlayout: +2,-0,+1\ndatatype: UInt8\n\
             transform: 1,0,0,10\ntransform: 0,1,0,20\ntransform: 0,0,1,30\n",
            &[0; 24],
        );
        assert!(sniff(&f));
        let h = parse(&f).unwrap();
        assert_eq!(h.kind, Kind::Mif);
        assert_eq!(h.layout.size, [2, 3, 4]);
        assert_eq!(h.layout.stride, [12, 1, 3]);
        assert_eq!(h.layout.samples, 24);
        // j stored backwards: stored j' = 2 - j, so its column turns and the origin moves to
        // the far end (y = 20 + 2·2); LPS negates x and y.
        assert_eq!(h.affine[0], [-1.0, 0.0, 0.0, -10.0]);
        assert_eq!(h.affine[1], [0.0, 2.0, 0.0, -24.0]);
        assert_eq!(h.affine[2], [0.0, 0.0, 3.0, 30.0]);
        assert_eq!(
            h.data.skip,
            Skip::Start {
                lines: 0,
                bytes: 256
            }
        );
    }

    #[test]
    fn refusals() {
        let ok = "dim: 2,2,2\nvox: 1,1,1\ndatatype: Int16LE\n";
        assert!(parse(&file(ok, &[0; 16])).is_ok());
        assert!(matches!(
            parse(&file("dim: 2,2,2\nvox: 1,1,1\ndatatype: CFloat32LE\n", &[])),
            Err(Error::Unsupported(_))
        ));
        assert!(parse(&file("dim: 2,0,2\nvox: 1,1,1\ndatatype: UInt8\n", &[])).is_err());
        assert!(
            parse(&file(
                "dim: 2,2,2,2\nvox: 1,1,1,1\nlayout: +0,+1,+2,-3\ndatatype: UInt8\n",
                &[]
            ))
            .is_err()
        );
        assert!(parse(b"mrtrix image\ndim: 2,2,2\n").is_err());
        assert!(!sniff(b"mrtrix tracks\n"));
    }
}

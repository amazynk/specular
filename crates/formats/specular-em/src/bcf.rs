//! Bruker Esprit `.bcf` files: SEM / TEM images and EDS hypermaps.
//!
//! A BCF is an "AAMVHFSS" single-file system: fixed-size chunks, a tree of named items
//! (directories and files), each file a table of chunk numbers (itself chained across chunks
//! when long), optionally compressed as "AACS" zlib blocks. `EDSDatabase/HeaderData` is an XML
//! description of the acquisition (microscope, stage, scan, the images as base64 planes, the
//! sum spectrum and its energy calibration); `EDSDatabase/SpectrumDataN` is the packed
//! hypermap: per line the pixels present, per pixel a header and the spectrum packed as 16-bit
//! pulse channels, 12-bit pulse channels, or "instructed" bunches of channel counts (nibbles,
//! bytes, 16- or 32-bit values above a gain) with extra pulses after them.
//!
//! The images become one page per image set (planes per detector); each hypermap becomes a
//! page of total counts per pixel with the spectrum at each pixel unpacked on demand from an
//! index of pixel records built at open.

use std::sync::Arc;

use crate::dataset::{Axis, AxisKind, Dataset, Field, Lateral, Page, Plane, Spectra};
use crate::samples::Sample;
use crate::xml::{self, Doc};
use crate::{Error, MAX_PLANE, Result};

const MAGIC: &[u8; 8] = b"AAMVHFSS";
/// Largest decompressed internal file.
const MAX_FILE: usize = 1 << 34;
/// Largest number of channels in a spectrum.
const MAX_CHANNELS: usize = 1 << 16;
/// Largest hypermap grid (pixels).
const MAX_GRID: usize = 1 << 26;

/// Whether `head` starts a BCF (single-file-system) container.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

/// Reads the images and hypermaps of a BCF.
pub fn read(file: &[u8]) -> Result<Dataset> {
    let sfs = Sfs::parse(file)?;
    let header = sfs
        .find("EDSDatabase/HeaderData")
        .ok_or_else(|| Error::Malformed("BCF without EDSDatabase/HeaderData".into()))?;
    let header = sfs.contents(file, header)?;
    let d = Doc::parse(&header);
    let root = d
        .descendants(d.root(), "ClassInstance")
        .into_iter()
        .find(|&n| d.attr(n, "Type") == Some("TRTSpectrumDatabase"))
        .ok_or_else(|| Error::Malformed("BCF header without TRTSpectrumDatabase".into()))?;
    let base = Base::parse(&d, root);
    let mut pages = Vec::new();
    // The images: the unnamed TRTImageData, then (version 2) the overview.
    let images = class(&d, root, "TRTImageData").find(|&n| d.attr(n, "Name").is_none());
    if let Some(n) = images {
        pages.extend(image_page(&d, n, &base, None));
    }
    let overview = class(&d, root, "TRTContainerClass")
        .filter_map(|c| d.child(c, "ChildClassInstances"))
        .flat_map(|c| d.children(c, "ClassInstance").collect::<Vec<_>>())
        .filter(|&c| d.attr(c, "Name") == Some("OverviewImages"))
        .filter_map(|c| d.child(c, "ChildClassInstances"))
        .flat_map(|c| class(&d, c, "TRTImageData").collect::<Vec<_>>())
        .next();
    if let Some(n) = overview {
        pages.extend(image_page(&d, n, &base, Some("Overview")));
    }
    // The hypermaps, in index order.
    let mut maps: Vec<(usize, usize)> = sfs
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| !it.dir && sfs.parent_is(it, "EDSDatabase"))
        .filter_map(|(k, it)| Some((it.name.strip_prefix("SpectrumData")?.parse().ok()?, k)))
        .collect();
    maps.sort_unstable();
    for (index, item) in maps {
        let Some(spec) = d
            .child(root, &format!("SpectrumData{index}"))
            .and_then(|n| d.child(n, "ClassInstance"))
        else {
            continue;
        };
        let stream = sfs.contents(file, item)?;
        pages.push(hypermap(&d, spec, stream, &base, index)?);
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Dataset { pages })
}

/// One item of the file tree.
#[derive(Clone, Debug)]
struct Item {
    table: u32,
    size: u64,
    parent: i32,
    dir: bool,
    name: String,
}

/// The container: chunk geometry and the file tree.
#[derive(Debug)]
struct Sfs {
    chunk: usize,
    items: Vec<Item>,
}

impl Sfs {
    fn parse(b: &[u8]) -> Result<Sfs> {
        if !probe(b) {
            return Err(Error::Unsupported("not a BCF container".into()));
        }
        let chunk = u32le(b, 0x128).ok_or(Error::Truncated("BCF header"))? as usize;
        let tree = u32le(b, 0x140).ok_or(Error::Truncated("BCF header"))? as usize;
        let n = u32le(b, 0x144).ok_or(Error::Truncated("BCF header"))? as usize;
        if chunk <= 0x138 || n > b.len() / 0x200 + 1 {
            return Err(Error::Malformed("BCF chunk geometry".into()));
        }
        let raw = chained(b, chunk, tree, n * 0x200)?;
        let mut items = Vec::with_capacity(n);
        for r in raw.chunks_exact(0x200) {
            let name = &r[0xE0..0x1E0];
            let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
            items.push(Item {
                table: u32le(r, 0).unwrap_or(0),
                size: u64le(r, 4).unwrap_or(0),
                parent: u32le(r, 0x28).unwrap_or(u32::MAX) as i32,
                dir: r[0xDC] != 0,
                name: String::from_utf8_lossy(name).into_owned(),
            });
        }
        Ok(Sfs { chunk, items })
    }

    fn parent_is(&self, it: &Item, name: &str) -> bool {
        usize::try_from(it.parent)
            .ok()
            .and_then(|p| self.items.get(p))
            .is_some_and(|p| p.dir && p.name == name)
    }

    /// The file at a `/`-separated path.
    fn find(&self, path: &str) -> Option<usize> {
        let mut parent = -1i32;
        let mut found = None;
        for part in path.split('/') {
            let k = self
                .items
                .iter()
                .position(|it| it.parent == parent && it.name == part)?;
            parent = i32::try_from(k).ok()?;
            found = Some(k);
        }
        found.filter(|&k| !self.items[k].dir)
    }

    /// The bytes of file `k`, decompressed.
    fn contents(&self, b: &[u8], k: usize) -> Result<Vec<u8>> {
        let it = &self.items[k];
        let usable = self.chunk - 32;
        let size = usize::try_from(it.size).map_err(|_| Error::TooLarge)?;
        if size > b.len() {
            return Err(Error::Truncated("BCF internal file"));
        }
        let n = size.div_ceil(usable);
        let table = chained(b, self.chunk, it.table as usize, n * 4)?;
        let mut out = Vec::with_capacity(size);
        for (i, p) in table.chunks_exact(4).enumerate() {
            let c = u32::from_le_bytes([p[0], p[1], p[2], p[3]]) as usize;
            let at = c
                .checked_mul(self.chunk)
                .and_then(|a| a.checked_add(0x138))
                .ok_or(Error::Malformed("BCF chunk pointer".into()))?;
            let len = usable.min(size - i * usable);
            let s = b.get(at..at + len).ok_or(Error::Truncated("BCF chunk"))?;
            out.extend_from_slice(s);
        }
        if out.starts_with(b"AACS") {
            return aacs(&out);
        }
        Ok(out)
    }
}

/// `len` bytes of a structure stored from chunk `first` on, continued through the chunk
/// numbers in each chunk's header when it spans several.
fn chained(b: &[u8], chunk: usize, first: usize, len: usize) -> Result<Vec<u8>> {
    let usable = chunk - 32;
    let mut out = Vec::with_capacity(len.min(b.len()));
    let mut c = first;
    if len <= usable {
        let at = c.checked_mul(chunk).ok_or(Error::TooLarge)? + 0x138;
        out.extend_from_slice(b.get(at..at + len).ok_or(Error::Truncated("BCF table"))?);
        return Ok(out);
    }
    let mut seen = 0usize;
    while out.len() < len {
        let at = c.checked_mul(chunk).ok_or(Error::TooLarge)?;
        let next = u32le(b, at + 0x118).ok_or(Error::Truncated("BCF table chain"))? as usize;
        let take = usable.min(len - out.len());
        out.extend_from_slice(
            b.get(at + 0x138..at + 0x138 + take)
                .ok_or(Error::Truncated("BCF table chain"))?,
        );
        c = next;
        seen += 1;
        if seen > b.len() / chunk + 1 {
            return Err(Error::Malformed("BCF chunk chain loops".into()));
        }
    }
    Ok(out)
}

/// Inflates an "AACS" compressed file: block headers from 0x80, each a compressed size then
/// twelve bytes, followed by a zlib stream.
fn aacs(b: &[u8]) -> Result<Vec<u8>> {
    let block = u32le(b, 4).ok_or(Error::Truncated("AACS header"))? as usize;
    let n = u32le(b, 12).ok_or(Error::Truncated("AACS header"))? as usize;
    if block == 0 || n.saturating_mul(block) > MAX_FILE || n > b.len() / 16 {
        return Err(Error::Malformed("AACS block count".into()));
    }
    // Deflate expands at most about 1032:1: reserve no more than the blocks can inflate to.
    let mut out = Vec::with_capacity((n * block).min(b.len().saturating_mul(1032)));
    let mut at = 0x80usize;
    for _ in 0..n {
        let c = u32le(b, at).ok_or(Error::Truncated("AACS block"))? as usize;
        at += 16;
        let z = b
            .get(at..at.checked_add(c).ok_or(Error::TooLarge)?)
            .ok_or(Error::Truncated("AACS block"))?;
        let v = specular_png::inflate_zlib(z, block)
            .map_err(|e| Error::Malformed(format!("AACS block: {e:?}")))?;
        out.extend_from_slice(&v);
        at += c;
    }
    Ok(out)
}

/// The facts shared by every page of a file.
#[derive(Debug, Default)]
struct Base {
    title: Option<String>,
    date: Option<String>,
    voltage: Option<f64>,
    magnification: Option<f64>,
    /// Micrometres per pixel.
    dx: Option<f64>,
    dy: Option<f64>,
    meta: Vec<(String, String)>,
}

impl Base {
    fn parse(d: &Doc, root: usize) -> Base {
        let mut b = Base {
            title: d.attr(root, "Name").map(str::to_string),
            ..Base::default()
        };
        if let Some(h) = d.child(root, "Header") {
            b.date = date(d, h);
        }
        for (ty, group) in [
            ("TRTSEMData", "Microscope"),
            ("TRTSEMStageData", "Stage"),
            ("TRTDSPConfiguration", "DSP"),
        ] {
            let Some(n) = class(d, root, ty).next() else {
                continue;
            };
            flat(d, n, group, &mut b.meta);
            if ty == "TRTSEMData" {
                b.voltage = d.text(n, "HV").and_then(xml::number).map(|v| v * 1e3);
                b.magnification = d.text(n, "Mag").and_then(xml::number);
                b.dx = d.text(n, "DX").and_then(xml::number);
                b.dy = d.text(n, "DY").and_then(xml::number);
            }
        }
        let elements: Vec<String> = class(d, root, "TRTContainerClass")
            .flat_map(|c| d.descendants(c, "ClassInstance"))
            .filter(|&n| d.attr(n, "Type") == Some("TRTSpectrumRegion"))
            .filter_map(|n| {
                let el = d.text(n, "XmlClassName")?;
                Some(match d.text(n, "Line") {
                    Some(l) if !l.is_empty() => format!("{el} {l}"),
                    _ => el.to_string(),
                })
            })
            .collect();
        if !elements.is_empty() {
            b.meta.push(("Elements".into(), elements.join(", ")));
        }
        b
    }

    fn apply(&self, page: &mut Page) {
        page.field = Field::ElectronMicroscopy;
        page.title = self.title.clone();
        page.date = self.date.clone();
        page.voltage = self.voltage;
        page.magnification = self.magnification;
        page.meta = self.meta.clone();
        if let (Some(dx), Some(dy)) = (self.dx, self.dy) {
            page.x = Some(Lateral::new(0.0, dx, "µm")).filter(Lateral::is_usable);
            page.y = Some(Lateral::new(0.0, dy, "µm")).filter(Lateral::is_usable);
        }
    }
}

/// "Date Time" of a node with `Date` and `Time` children.
fn date(d: &Doc, n: usize) -> Option<String> {
    let day = d.text(n, "Date").filter(|s| !s.is_empty())?;
    Some(match d.text(n, "Time").filter(|s| !s.is_empty()) {
        Some(t) => format!("{day} {t}"),
        None => day.to_string(),
    })
}

/// The leaf children of `n` as `group: name` meta fields.
fn flat(d: &Doc, n: usize, group: &str, out: &mut Vec<(String, String)>) {
    for &c in &d.el(n).children {
        let el = d.el(c);
        let t = el.text.trim();
        if el.children.is_empty() && !t.is_empty() && t.len() <= 256 {
            out.push((format!("{group}: {}", el.name), t.to_string()));
        }
    }
}

/// `ClassInstance` children of `n` of type `ty`.
fn class<'a>(d: &'a Doc, n: usize, ty: &'a str) -> impl Iterator<Item = usize> + 'a {
    d.children(n, "ClassInstance")
        .filter(move |&c| d.attr(c, "Type") == Some(ty))
}

/// A page of the planes of a TRTImageData node (all-zero planes skipped, as Esprit leaves
/// unused detector slots empty).
fn image_page(d: &Doc, n: usize, base: &Base, name: Option<&str>) -> Option<Page> {
    let w: usize = d.text(n, "Width")?.parse().ok()?;
    let h: usize = d.text(n, "Height")?.parse().ok()?;
    let size: usize = d.text(n, "ItemSize")?.parse().ok()?;
    let count: usize = d.text(n, "PlaneCount")?.parse().ok()?;
    let sample = match size {
        1 => Sample::U8,
        2 => Sample::U16,
        4 => Sample::U32,
        _ => return None,
    };
    let px = w
        .checked_mul(h)
        .filter(|&p| p > 0 && p as u64 <= MAX_PLANE)?;
    let mut planes = Vec::new();
    for i in 0..count.min(64) {
        let Some(p) = d.child(n, &format!("Plane{i}")) else {
            continue;
        };
        let Some(raw) = d.text(p, "Data").and_then(base64) else {
            continue;
        };
        if raw.len() < px * size || raw.iter().all(|&c| c == 0) {
            continue;
        }
        let v: Vec<f32> = raw[..px * size]
            .chunks_exact(size)
            .map(|c| match c {
                [a] => f32::from(*a),
                [a, b] => f32::from(u16::from_le_bytes([*a, *b])),
                [a, b, c, e] => u32::from_le_bytes([*a, *b, *c, *e]) as f32,
                _ => f32::NAN,
            })
            .collect();
        let label = d.text(p, "Description").unwrap_or_default().to_string();
        planes.push(Plane::values(label, "", v));
    }
    if planes.is_empty() {
        return None;
    }
    let mut page = Page {
        width: w,
        height: h,
        planes,
        axis: Axis {
            kind: AxisKind::Channel,
            name: "Detector".into(),
            linear: None,
        },
        sample: Some(sample),
        ..Page::default()
    };
    base.apply(&mut page);
    if let Some(name) = name {
        page.title = Some(match &base.title {
            Some(t) => format!("{t} ({name})"),
            None => name.to_string(),
        });
        // The overview covers a larger field than the map's calibration describes.
        page.x = None;
        page.y = None;
    }
    Some(page)
}

/// Start of the pixel records in a hypermap stream.
const MAP_START: usize = 0x1A0;
/// Size of a pixel record's header.
const PIXEL_HEAD: usize = 22;

/// A packed EDS hypermap with the offset of each pixel's record.
struct Hypermap {
    stream: Arc<[u8]>,
    width: usize,
    height: usize,
    channels: usize,
    axis: Lateral,
    /// Offset of each pixel's record header, row-major; `usize::MAX` when absent.
    index: Vec<usize>,
    sum: Vec<f32>,
}

impl std::fmt::Debug for Hypermap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hypermap")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("channels", &self.channels)
            .field("bytes", &self.stream.len())
            .finish_non_exhaustive()
    }
}

impl Spectra for Hypermap {
    fn grid(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn axis(&self) -> Lateral {
        self.axis.clone()
    }

    fn at(&self, _file: &[u8], x: usize, y: usize) -> Option<Vec<f32>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let mut counts = vec![0u32; self.channels];
        let r = self.index[y * self.width + x];
        if r != usize::MAX {
            unpack(&self.stream, r, self.channels, |c, v| {
                counts[c] = counts[c].wrapping_add(v);
            })?;
        }
        Some(counts.into_iter().map(|c| c as f32).collect())
    }

    fn sum(&self) -> Option<Vec<f32>> {
        Some(self.sum.clone())
    }
}

/// Unpacks the pixel record at `r`, calling `add(channel, count)` for every channel below
/// `channels` with counts; returns the offset past the record.
fn unpack(s: &[u8], r: usize, channels: usize, mut add: impl FnMut(usize, u32)) -> Option<usize> {
    let flag = u16le(s, r + 12)?;
    let pulses = usize::from(u16le(s, r + 16)?);
    let size = u32le(s, r + 18)? as usize;
    let at = r + PIXEL_HEAD;
    let data = s.get(at..at.checked_add(size)?)?;
    match flag {
        0 => {
            for c in data.chunks_exact(2).take(pulses) {
                let ch = usize::from(u16::from_le_bytes([c[0], c[1]]));
                if ch < channels {
                    add(ch, 1);
                }
            }
            Some(at + size)
        }
        1 => {
            let g = |k: usize| u16::from(data.get(k).copied().unwrap_or(0));
            for i in 0..pulses {
                let k = 6 * (i / 4);
                let ch = match i % 4 {
                    0 => (g(k) >> 4) + (g(k + 1) << 4),
                    1 => ((g(k) << 8) + g(k + 3)) & 0xFFF,
                    2 => (g(k + 2) << 4) + (g(k + 5) >> 4),
                    _ => ((g(k + 5) << 8) + g(k + 4)) & 0xFFF,
                };
                let ch = usize::from(ch & 0xFFF);
                if ch < channels {
                    add(ch, 1);
                }
            }
            Some(at + size)
        }
        _ => {
            let body = data.get(..size.checked_sub(4)?)?;
            let mut o = 0;
            let mut ch = 0usize;
            while o + 2 <= body.len() {
                let (width, n) = (usize::from(body[o]), usize::from(body[o + 1]));
                o += 2;
                if width == 0 {
                    ch += n;
                    continue;
                }
                let gain = match width {
                    1 => u32::from(*body.get(o)?),
                    2 => u32::from(u16le(body, o)?),
                    4 => u32le(body, o)?,
                    _ => u64le(body, o)? as u32,
                };
                o += width;
                let len = if width == 1 {
                    n.div_ceil(2)
                } else {
                    n * width / 2
                };
                let v = body.get(o..o + len)?;
                for i in 0..n {
                    let value = match width {
                        1 => u32::from(if i % 2 == 0 {
                            v[i / 2] & 15
                        } else {
                            v[i / 2] >> 4
                        }),
                        2 => u32::from(v[i]),
                        4 => u32::from(u16::from_le_bytes([v[2 * i], v[2 * i + 1]])),
                        // Widths other than 1, 2 and 4 hold 32-bit values; a hostile width
                        // (3, 5…) leaves too few bytes for them.
                        _ => u32::from_le_bytes(v.get(4 * i..4 * i + 4)?.try_into().ok()?),
                    };
                    let value = value.wrapping_add(gain);
                    if ch + i < channels && value != 0 {
                        add(ch + i, value);
                    }
                }
                o += len;
                ch += n;
            }
            let mut end = at + size;
            if pulses > 0 {
                let extra = u32le(s, at + size - 4)? as usize;
                let p = s.get(end..end.checked_add(extra)?)?;
                for c in p.chunks_exact(2).take(pulses) {
                    let ch = usize::from(u16::from_le_bytes([c[0], c[1]]));
                    if ch < channels {
                        add(ch, 1);
                    }
                }
                end += extra;
            }
            Some(end)
        }
    }
}

/// The hypermap page of `SpectrumDataN`.
fn hypermap(d: &Doc, spec: usize, stream: Vec<u8>, base: &Base, index: usize) -> Result<Page> {
    let header = class(d, spec, "TRTSpectrumHeader").next().unwrap_or(spec);
    let sum: Vec<f32> = d
        .text(spec, "Channels")
        .unwrap_or_default()
        .split(',')
        .filter_map(|t| t.trim().parse::<f64>().ok())
        .map(|v| v as f32)
        .collect();
    let channels = if sum.is_empty() {
        d.text(header, "ChannelCount")
            .and_then(|t| t.parse().ok())
            .unwrap_or(0)
    } else {
        sum.len()
    };
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(Error::Malformed("BCF spectrum channel count".into()));
    }
    let offset = d
        .text(header, "CalibAbs")
        .and_then(xml::number)
        .unwrap_or(0.0);
    let step = d
        .text(header, "CalibLin")
        .and_then(xml::number)
        .unwrap_or(1.0);
    let height = i32le(&stream, 0).ok_or(Error::Truncated("BCF hypermap"))?;
    let width = i32le(&stream, 4).ok_or(Error::Truncated("BCF hypermap"))?;
    let (width, height) = (
        usize::try_from(width).map_err(|_| Error::Malformed("hypermap width".into()))?,
        usize::try_from(height).map_err(|_| Error::Malformed("hypermap height".into()))?,
    );
    let n = width
        .checked_mul(height)
        .filter(|&n| n > 0 && n as u64 <= MAX_PLANE && n <= MAX_GRID)
        .ok_or(Error::TooLarge)?;
    // One pass over the record headers to find each pixel.
    let mut idx = vec![usize::MAX; n];
    let mut o = MAP_START;
    'lines: for y in 0..height {
        let Some(count) = u32le(&stream, o) else {
            break;
        };
        o += 4;
        for _ in 0..count {
            let Some(x) = u32le(&stream, o) else {
                break 'lines;
            };
            let x = x as usize;
            let Some(next) = record_end(&stream, o) else {
                break 'lines;
            };
            if x < width {
                idx[y * width + x] = o;
            }
            o = next;
        }
    }
    let totals: Vec<f32> = {
        use rayon::prelude::*;
        idx.par_iter()
            .map(|&r| {
                if r == usize::MAX {
                    return 0.0;
                }
                let mut t = 0u64;
                unpack(&stream, r, channels, |_, v| t += u64::from(v));
                t as f32
            })
            .collect()
    };
    let sp = Hypermap {
        stream: Arc::from(stream),
        width,
        height,
        channels,
        axis: Lateral::new(offset, step, "keV"),
        index: idx,
        sum,
    };
    let mut page = Page {
        width,
        height,
        planes: vec![Plane::values("Counts", "", totals)],
        ..Page::default()
    };
    base.apply(&mut page);
    let title = if index == 0 {
        "EDS".to_string()
    } else {
        format!("EDS {index}")
    };
    page.title = Some(match &base.title {
        Some(t) => format!("{t} ({title})"),
        None => title,
    });
    if let Some(e) = d.child(spec, "TRTHeaderedClass") {
        if let Some(det) = class(d, e, "TRTDetectorHeader").next() {
            page.detector = d.text(det, "Type").map(str::to_string);
        }
        for ty in ["TRTESMAHeader", "TRTXrfHeader"] {
            if let Some(h) = class(d, e, ty).next() {
                flat(d, h, "Analysis", &mut page.meta);
                let kv = d
                    .text(h, "PrimaryEnergy")
                    .or_else(|| d.text(h, "Voltage"))
                    .and_then(xml::number);
                if let Some(kv) = kv {
                    page.voltage = Some(kv * 1e3);
                }
            }
        }
        if let Some(h) = class(d, e, "TRTSpectrumHardwareHeader").next() {
            flat(d, h, "Hardware", &mut page.meta);
        }
    }
    if let Some(t) = date(d, header) {
        page.date = Some(t);
    }
    page.meta.push(("Channels".into(), channels.to_string()));
    page.spectra = Some(Arc::new(sp));
    Ok(page)
}

/// Offset past the pixel record at `r` (header, packed data and extra pulses).
fn record_end(s: &[u8], r: usize) -> Option<usize> {
    let flag = u16le(s, r + 12)?;
    let pulses = u16le(s, r + 16)?;
    let size = u32le(s, r + 18)? as usize;
    let mut end = r.checked_add(PIXEL_HEAD)?.checked_add(size)?;
    if flag > 1 && pulses > 0 {
        end = end.checked_add(u32le(s, end.checked_sub(4)?)? as usize)?;
    }
    (end <= s.len()).then_some(end)
}

/// Standard base64 (whitespace ignored, padding optional).
fn base64(t: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in t.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn i32le(b: &[u8], at: usize) -> Option<i32> {
    u32le(b, at).map(|v| v as i32)
}

fn u64le(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        b.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHUNK: usize = 0x2000 + 32;

    /// A stored-block zlib stream.
    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut z = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = data.chunks(0xFFFF).collect();
        for (i, b) in blocks.iter().enumerate() {
            z.push(u8::from(i + 1 == blocks.len()));
            let n = b.len() as u16;
            z.extend_from_slice(&n.to_le_bytes());
            z.extend_from_slice(&(!n).to_le_bytes());
            z.extend_from_slice(b);
        }
        let (mut a, mut s) = (1u32, 0u32);
        for &x in data {
            a = (a + u32::from(x)) % 65521;
            s = (s + a) % 65521;
        }
        z.extend_from_slice(&((s << 16) | a).to_be_bytes());
        z
    }

    /// AACS-compressed contents in blocks of `block` bytes.
    fn compress(data: &[u8], block: usize) -> Vec<u8> {
        let blocks: Vec<&[u8]> = data.chunks(block).collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"AACS");
        out.extend_from_slice(&(block as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
        out.resize(0x80, 0);
        for b in blocks {
            let z = zlib(b);
            out.extend_from_slice(&(z.len() as u32).to_le_bytes());
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&(z.len() as u32 + 16).to_le_bytes());
            out.extend_from_slice(&z);
        }
        out
    }

    /// One chunk's contents: data, or a pointer-table piece with the next chunk's number.
    enum Body {
        Data(Vec<u8>),
        Table(u32, Vec<u8>),
    }

    /// An SFS container holding `EDSDatabase/<name>` files, pointer tables chained over
    /// several chunks when long.
    fn container(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let usable = CHUNK - 32;
        let item = |table: usize, size: usize, parent: i32, dir: bool, name: &str| {
            let mut r = vec![0u8; 0x200];
            r[0..4].copy_from_slice(&(table as u32).to_le_bytes());
            r[4..12].copy_from_slice(&(size as u64).to_le_bytes());
            r[0x28..0x2C].copy_from_slice(&parent.to_le_bytes());
            r[0xDC] = u8::from(dir);
            r[0xE0..0xE0 + name.len()].copy_from_slice(name.as_bytes());
            r
        };
        // Chunk k + 1 holds chunks[k]; chunk 1 is the tree.
        let mut chunks = vec![Body::Data(Vec::new())];
        let mut items = vec![item(0, 0, -1, true, "EDSDatabase")];
        for (name, data) in files {
            let first = chunks.len() + 1;
            for piece in data.chunks(usable) {
                chunks.push(Body::Data(piece.to_vec()));
            }
            let table: Vec<u8> = (0..data.len().div_ceil(usable))
                .flat_map(|k| ((first + k) as u32).to_le_bytes())
                .collect();
            let pieces: Vec<&[u8]> = table.chunks(usable).collect();
            let t0 = chunks.len() + 1;
            for (k, p) in pieces.iter().enumerate() {
                let next = if k + 1 < pieces.len() {
                    (t0 + k + 1) as u32
                } else {
                    0
                };
                chunks.push(Body::Table(next, p.to_vec()));
            }
            items.push(item(t0, data.len(), 0, false, name));
        }
        chunks[0] = Body::Data(items.concat());
        let mut b = vec![0u8; CHUNK * (chunks.len() + 1) + 0x118];
        b[..8].copy_from_slice(MAGIC);
        b[0x124..0x128].copy_from_slice(&2.6f32.to_le_bytes());
        b[0x128..0x12C].copy_from_slice(&(CHUNK as u32).to_le_bytes());
        b[0x140..0x144].copy_from_slice(&1u32.to_le_bytes());
        b[0x144..0x148].copy_from_slice(&(items.len() as u32).to_le_bytes());
        b[0x148..0x14C].copy_from_slice(&(chunks.len() as u32 + 1).to_le_bytes());
        for (k, body) in chunks.iter().enumerate() {
            let at = (k + 1) * CHUNK;
            let data = match body {
                Body::Data(d) => d,
                Body::Table(next, d) => {
                    b[at + 0x118..at + 0x11C].copy_from_slice(&next.to_le_bytes());
                    d
                }
            };
            b[at + 0x138..at + 0x138 + data.len()].copy_from_slice(data);
        }
        b
    }

    fn b64(data: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in data.chunks(3) {
            let v = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for k in 0..4 {
                if k <= c.len() {
                    s.push(A[((v >> (18 - 6 * k)) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }

    fn header(w: usize, h: usize, channels: usize, image: &[u16]) -> Vec<u8> {
        let img: Vec<u8> = image.iter().flat_map(|v| v.to_le_bytes()).collect();
        let sum = vec!["7"; channels].join(",");
        format!(
            "<?xml version=\"1.0\"?><TRTSpectrumDatabase>\
             <ClassInstance Type=\"TRTSpectrumDatabase\" Name=\"Map\">\
             <Header><Date>12.9.2018</Date><Time>13:9:11</Time><FileVersion>1</FileVersion></Header>\
             <ClassInstance Type=\"TRTSEMData\"><HV>20</HV><Mag>3138,5</Mag><DX>0,5</DX>\
             <DY>0,25</DY></ClassInstance>\
             <ClassInstance Type=\"TRTImageData\"><Width>{w}</Width><Height>{h}</Height>\
             <ItemSize>2</ItemSize><PlaneCount>2</PlaneCount><Plane0><Description>SE</Description>\
             <Data>{}</Data></Plane0><Plane1><Data>{}</Data></Plane1></ClassInstance>\
             <SpectrumData0><ClassInstance Type=\"TRTSpectrum\"><TRTHeaderedClass>\
             <ClassInstance Type=\"TRTDetectorHeader\"><Type>XFlash</Type></ClassInstance>\
             <ClassInstance Type=\"TRTESMAHeader\"><PrimaryEnergy>15</PrimaryEnergy></ClassInstance>\
             </TRTHeaderedClass><ClassInstance Type=\"TRTSpectrumHeader\"><CalibAbs>-4,7E-1</CalibAbs>\
             <CalibLin>1E-2</CalibLin></ClassInstance><Channels>{sum}</Channels></ClassInstance>\
             </SpectrumData0></ClassInstance></TRTSpectrumDatabase>",
            b64(&img),
            b64(&vec![0u8; img.len()])
        )
        .into_bytes()
    }

    fn record(
        x: u32,
        channels: u16,
        flag: u16,
        pulses: u16,
        data: &[u8],
        extra: &[u16],
    ) -> Vec<u8> {
        let tail = if flag > 1 { 4 } else { 0 };
        let mut r = Vec::new();
        r.extend_from_slice(&x.to_le_bytes());
        r.extend_from_slice(&channels.to_le_bytes());
        r.extend_from_slice(&channels.to_le_bytes());
        r.extend_from_slice(&0x80u32.to_le_bytes());
        r.extend_from_slice(&flag.to_le_bytes());
        r.extend_from_slice(&(data.len() as u16).to_le_bytes());
        r.extend_from_slice(&pulses.to_le_bytes());
        r.extend_from_slice(&((data.len() + tail) as u32).to_le_bytes());
        r.extend_from_slice(data);
        if flag > 1 {
            r.extend_from_slice(&((extra.len() * 2) as u32).to_le_bytes());
            for e in extra {
                r.extend_from_slice(&e.to_le_bytes());
            }
        }
        r
    }

    fn pulses16(ch: &[u16]) -> Vec<u8> {
        ch.iter().flat_map(|c| c.to_le_bytes()).collect()
    }

    fn pulses12(ch: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        for g in ch.chunks(4) {
            let c = |k: usize| g.get(k).copied().unwrap_or(0);
            out.extend_from_slice(&[
                (((c(0) & 0xF) << 4) | (c(1) >> 8)) as u8,
                (c(0) >> 4) as u8,
                (c(2) >> 4) as u8,
                (c(1) & 0xFF) as u8,
                (c(3) & 0xFF) as u8,
                (((c(2) & 0xF) << 4) | (c(3) >> 8)) as u8,
            ]);
        }
        out
    }

    /// Instructed bunches: (width, gain, values).
    fn bunches(b: &[(u8, u32, Vec<u32>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (w, gain, v) in b {
            out.push(*w);
            out.push(v.len() as u8);
            match w {
                0 => continue,
                1 => out.push(*gain as u8),
                2 => out.extend_from_slice(&(*gain as u16).to_le_bytes()),
                4 => out.extend_from_slice(&gain.to_le_bytes()),
                _ => out.extend_from_slice(&u64::from(*gain).to_le_bytes()),
            }
            match w {
                1 => {
                    for p in v.chunks(2) {
                        out.push((p[0] as u8) | ((*p.get(1).unwrap_or(&0) as u8) << 4));
                    }
                }
                2 => out.extend(v.iter().map(|&x| x as u8)),
                4 => out.extend(v.iter().flat_map(|&x| (x as u16).to_le_bytes())),
                _ => out.extend(v.iter().flat_map(|&x| x.to_le_bytes())),
            }
        }
        out
    }

    /// A 3 × 2 map with a pixel of each packing; pixel (2, 1) absent.
    fn map(channels: u16) -> Vec<u8> {
        let mut s = vec![0u8; MAP_START];
        s[0..4].copy_from_slice(&2i32.to_le_bytes());
        s[4..8].copy_from_slice(&3i32.to_le_bytes());
        let c = channels;
        let instructed = bunches(&[
            (1, 1, vec![0, 15, 2]),
            (0, 0, vec![0, 0]),
            (2, 10, vec![200]),
            (4, 0, vec![1000, 2]),
            (8, 5, vec![70000]),
        ]);
        s.extend_from_slice(&3u32.to_le_bytes());
        s.extend_from_slice(&record(0, c, 0, 3, &pulses16(&[1, 1, 5]), &[]));
        s.extend_from_slice(&record(1, c, 1, 5, &pulses12(&[2, 3, 3, 4095, 7]), &[]));
        s.extend_from_slice(&record(2, c, 2, 2, &instructed, &[0, 9]));
        s.extend_from_slice(&2u32.to_le_bytes());
        s.extend_from_slice(&record(1, c, 0, 1, &pulses16(&[7]), &[]));
        s.extend_from_slice(&record(0, c, 0, 0, &[], &[]));
        s
    }

    fn expected() -> Vec<Vec<f32>> {
        let mut px = vec![vec![0f32; 16]; 6];
        px[0][1] = 2.0;
        px[0][5] = 1.0;
        for c in [2, 3, 3, 7] {
            px[1][c] += 1.0;
        }
        // Nibbles 1, 16, 3; two skipped; 210; 1000, 2; 70005; extra pulses at 0 and 9.
        px[2][..10].copy_from_slice(&[2.0, 16.0, 3.0, 0.0, 0.0, 210.0, 1000.0, 2.0, 70005.0, 1.0]);
        px[4][7] = 1.0;
        px
    }

    fn check(b: &[u8]) {
        let d = read(b).unwrap();
        assert_eq!(d.pages.len(), 2);
        let im = &d.pages[0];
        assert_eq!((im.width, im.height, im.planes.len()), (3, 2, 1));
        assert_eq!(im.planes[0].label, "SE");
        assert_eq!(
            im.read_plane(b, 0).unwrap().0,
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
        assert_eq!(im.x, Some(Lateral::new(0.0, 0.5, "µm")));
        assert_eq!(im.y, Some(Lateral::new(0.0, 0.25, "µm")));
        assert_eq!(im.voltage, Some(20e3));
        assert_eq!(im.magnification, Some(3138.5));
        let m = &d.pages[1];
        assert_eq!(m.voltage, Some(15e3));
        assert_eq!(m.detector.as_deref(), Some("XFlash"));
        let s = m.spectra.as_ref().unwrap();
        assert_eq!((s.grid(), s.channels()), ((3, 2), 16));
        assert_eq!(s.axis(), Lateral::new(-0.47, 0.01, "keV"));
        assert_eq!(s.sum().unwrap(), vec![7.0; 16]);
        let totals = m.read_plane(b, 0).unwrap().0;
        for (i, w) in expected().iter().enumerate() {
            assert_eq!(&s.at(b, i % 3, i / 3).unwrap(), w, "pixel {i}");
            assert_eq!(totals[i], w.iter().sum::<f32>());
        }
        assert!(s.at(b, 3, 0).is_none());
    }

    #[test]
    fn packings_plain_and_compressed() {
        let h = header(3, 2, 16, &[1, 2, 3, 4, 5, 6]);
        let plain = container(&[("HeaderData", h.clone()), ("SpectrumData0", map(16))]);
        assert!(probe(&plain));
        check(&plain);
        check(&container(&[
            ("HeaderData", compress(&h, 512)),
            ("SpectrumData0", compress(&map(16), 64)),
        ]));
    }

    #[test]
    fn truncated_never_panics() {
        let b = container(&[
            (
                "HeaderData",
                compress(&header(3, 2, 16, &[1, 2, 3, 4, 5, 6]), 512),
            ),
            ("SpectrumData0", map(16)),
        ]);
        for n in (0..b.len()).step_by(7) {
            let _ = read(&b[..n]);
        }
        let s = map(16);
        for n in 0..s.len() {
            for r in [MAP_START + 4, n] {
                let _ = unpack(&s[..n], r, 16, |_, _| {});
                let _ = record_end(&s[..n], r);
            }
        }
        assert!(read(b"AAMVHFSS").is_err());
        assert_eq!(base64("QUJD"), Some(b"ABC".to_vec()));
    }

    /// A 512 × 512 × 2048 map of 60 16-bit pulses per pixel: open, and one spectrum.
    #[test]
    fn large_map_is_quick() {
        let (w, h, c) = (512usize, 512usize, 2048u16);
        let mut s = vec![0u8; MAP_START];
        s[0..4].copy_from_slice(&(h as i32).to_le_bytes());
        s[4..8].copy_from_slice(&(w as i32).to_le_bytes());
        let pulses: Vec<u16> = (0..60u16).map(|k| (k * 37) % 2048).collect();
        let line: Vec<u8> = (0..w as u32)
            .flat_map(|x| record(x, c, 0, 60, &pulses16(&pulses), &[]))
            .collect();
        for _ in 0..h {
            s.extend_from_slice(&(w as u32).to_le_bytes());
            s.extend_from_slice(&line);
        }
        let b = container(&[
            ("HeaderData", header(w, h, usize::from(c), &vec![1; w * h])),
            ("SpectrumData0", s),
        ]);
        let t = std::time::Instant::now();
        let d = read(&b).unwrap();
        let open = t.elapsed();
        let sp = d.pages[1].spectra.as_ref().unwrap();
        let t = std::time::Instant::now();
        let v = sp.at(&b, 300, 400).unwrap();
        let at = t.elapsed();
        eprintln!("512x512x2048 BCF: open {open:?}, spectrum {at:?}");
        assert_eq!(v.iter().sum::<f32>(), 60.0);
        assert!(at.as_millis() < 5, "{at:?}");
    }

    mod hostile {
        use super::*;

        #[test]
        fn aacs_huge_block_size_reserves_little() {
            // Four blocks of 3.75 GiB declared in a 192-byte file: no 15 GiB reservation.
            let mut b = b"AACS".to_vec();
            b.extend_from_slice(&0xF000_0000u32.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&4u32.to_le_bytes());
            b.resize(0x80 + 4 * 16, 0);
            assert!(aacs(&b).is_err());
        }

        #[test]
        fn instructed_bunch_of_odd_width() {
            // Width 3 leaves 3 bytes for two values read as 32-bit: refused, no panic.
            let body = [3, 2, 0, 0, 0, 0, 0, 0, 0, 0];
            let r = record(0, 16, 2, 0, &body, &[]);
            assert_eq!(unpack(&r, 0, 16, |_, _| {}), None);
        }
    }
}

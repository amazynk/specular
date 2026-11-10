//! Olympus/Evident cellSens VSI: a TIFF file (its own IFDs hold a small preview) whose
//! metadata is a tree of tagged volumes right after the TIFF header, and a folder
//! `_<name>_` beside it with one `stackN/frame_*.ets` file per image.
//!
//! - **Metadata** (from byte 8): containers of a 24-byte header (size, the version 21321,
//!   volume version, the offset of the first field, a field count) and fields (type with
//!   flags for nested volumes, inline values and an extra tag; tag; next field; size). Each
//!   image ("pyramid") starts at an external-file property set after an image frame volume;
//!   from it come the image's name, its size (the image boundary), the physical pixel size
//!   (`RWC frame scale`), the objective, channel names and wavelengths, the Z positions and
//!   which tile dimension holds Z, C and T.
//! - **ETS**: `SIS` header (dimensions, the `ETS` header's offset, the chunk table), the
//!   `ETS` header (sample type, samples per pixel, compression, tile size, background
//!   colour, BGR order, whether there is a pyramid) and per chunk its coordinates (x, y,
//!   further dimensions, then the pyramid level), offset and size. Tiles are raw, JPEG,
//!   JPEG 2000, lossless JPEG, PNG or BMP; each level halves the one before.
//!
//! Files are matched with the metadata's images in order (stack folders sorted by name),
//! or by size when there are more files than images. The first image that is not the
//! label or the overview is the slide; the others are pages. Fluorescence channels are
//! composited in their wavelength's colour; Z and T are axes.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use crate::composite::Plane;
use crate::mosaic::{Mosaic, MosaicLevel, Placed, TileSource};
use crate::stitched::{Assoc, Stitched};
use crate::{
    AssociatedKind, Channel, Error, Files, Level, Position, Props, Result, Rgba, Slide, Vendor,
};

/// Deepest nesting of metadata volumes read.
const MAX_DEPTH: u32 = 48;
/// Most metadata fields read.
const MAX_FIELDS: u32 = 1 << 20;
/// Most chunks of a frame file.
const MAX_CHUNKS: usize = 1 << 24;
/// Most dimensions of a chunk's coordinates.
const MAX_DIMS: usize = 16;

// Field types.
const T_CHAR: u32 = 1;
const T_UCHAR: u32 = 2;
const T_SHORT: u32 = 3;
const T_USHORT: u32 = 4;
const T_INT: u32 = 5;
const T_UINT: u32 = 6;
const T_LONG: u32 = 7;
const T_ULONG: u32 = 8;
const T_FLOAT: u32 = 9;
const T_DOUBLE: u32 = 10;
const T_TCHAR: u32 = 13;
const T_DWORD: u32 = 14;
const T_TIMESTAMP: u32 = 17;
const T_DATE: u32 = 18;
const T_UNICODE: u32 = 8192;
const NEW_VOLUME_HEADER: u32 = 0;
const PROPERTY_SET_VOLUME: u32 = 1;
const NEW_MDIM_VOLUME_HEADER: u32 = 2;

// Tags.
const IMAGE_FRAME_VOLUME: i32 = 2002;
const DIMENSION_DESCRIPTION_VOLUME: i32 = 2007;
const CHANNEL_PROPERTIES: i32 = 2008;
const EXTERNAL_FILE_PROPERTIES: i32 = 2018;
const DEFAULT_BACKGROUND_COLOR: i32 = 2034;
const DOCUMENT_PROPERTIES: i32 = 2109;
const SLIDE_PROPERTIES: i32 = 2062;
const IMAGE_BOUNDARY: i32 = 2053;
const TILE_ORIGIN: i32 = 2410;
const HAS_EXTERNAL_FILE: i32 = 20005;
const Z_START: i32 = 2012;
const Z_INCREMENT: i32 = 2013;
const Z_VALUE: i32 = 2014;
const TIME_START: i32 = 2100;
const TIME_VALUE: i32 = 2017;
const LAMBDA_START: i32 = 2039;
const DIMENSION_MEANING: i32 = 2023;
const DIMENSION_VALUE_ID: i32 = 2027;
const CHANNEL_NAME: i32 = 2419;
const CREATION_TIME: i32 = 2015;
const RWC_FRAME_ORIGIN: i32 = 2018;
const RWC_FRAME_SCALE: i32 = 2019;
const STACK_NAME: i32 = 2030;
const OPTICAL_PATH: i32 = 2043;
const WAVELENGTH_VOLUME: i32 = 2417;
const OBJECTIVE_MAG: i32 = 120060;
const NUMERICAL_APERTURE: i32 = 120061;
const WORKING_DISTANCE: i32 = 120062;
const OBJECTIVE_NAME: i32 = 120063;
const OBJECTIVE_TYPE: i32 = 120064;
const DEVICE_NAME: i32 = 120116;
const DEVICE_SUBTYPE: i32 = 120130;
const EXPOSURE_TIME: i32 = 100002;
const BIT_DEPTH: i32 = 100049;
const VALUE: i32 = 268435458;
const DOCUMENT_NAME: i32 = 11;
const DOCUMENT_AUTHOR: i32 = 15;
const DOCUMENT_TIME: i32 = 14;

/// Whether `data` is a VSI file: a little-endian TIFF header followed by the metadata
/// container header (size 24, version 21321).
pub fn probe(data: &[u8]) -> bool {
    data.len() >= 12
        && data.starts_with(b"II*\0")
        && data[8..10] == 24u16.to_le_bytes()
        && data[10..12] == 21321u16.to_le_bytes()
}

/// What the metadata says about one image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Image {
    pub name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub tile_origin: Option<(i32, i32)>,
    /// Microns per pixel.
    pub physical: Option<(f64, f64)>,
    pub origin: Option<(f64, f64)>,
    pub magnification: Option<f64>,
    pub numerical_aperture: Option<f64>,
    pub objectives: Vec<(String, i64)>,
    pub channel_names: Vec<String>,
    /// Emission wavelengths, nm.
    pub wavelengths: Vec<f64>,
    /// Exposure times, microseconds.
    pub exposures: Vec<i64>,
    pub z_start: Option<f64>,
    pub z_increment: Option<f64>,
    pub z_values: Vec<f64>,
    /// Milliseconds.
    pub t_values: Vec<f64>,
    /// Unix seconds.
    pub acquired: Option<i64>,
    pub bit_depth: Option<i64>,
    pub fill: Option<u8>,
    pub devices: Vec<(String, String)>,
    /// The dimension tag holding Z, T, C and λ ("Z", "T", "C", "L").
    pub dims: Vec<(char, i32)>,
}

impl Image {
    fn dim(&self, k: char) -> Option<i32> {
        self.dims.iter().find(|d| d.0 == k).map(|d| d.1)
    }

    fn set_dim(&mut self, k: char, v: i32) {
        match self.dims.iter_mut().find(|d| d.0 == k) {
            Some(d) => d.1 = v,
            None => self.dims.push((k, v)),
        }
    }

    fn has_dim_value(&self, v: i32) -> bool {
        self.dims.iter().any(|d| d.1 == v)
    }
}

/// The VSI metadata: its images and document facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub images: Vec<Image>,
    /// The file says its pixels are in frame files.
    pub external: bool,
    pub document: Vec<(String, String)>,
}

/// The tag walk's state across volumes (as the format's readers keep it).
struct Walk<'a> {
    b: &'a [u8],
    meta: Meta,
    index: i64,
    previous: i32,
    dimension_tag: i32,
    in_dimensions: bool,
    found_channel: bool,
    fields: u32,
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(o..o.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn i32_at(b: &[u8], o: usize) -> Option<i32> {
    u32_at(b, o).map(|v| v as i32)
}
fn i64_at(b: &[u8], o: usize) -> Option<i64> {
    Some(i64::from_le_bytes(
        b.get(o..o.checked_add(8)?)?.try_into().ok()?,
    ))
}
fn f64_at(b: &[u8], o: usize) -> Option<f64> {
    i64_at(b, o).map(|v| f64::from_bits(v as u64))
}

/// A leaf value as the format's readers word it.
#[derive(Clone, Debug)]
enum Val {
    Int(i64),
    Num(f64),
    Text(String),
    Ints(Vec<i32>),
    Nums(Vec<f64>),
    None,
}

impl Val {
    fn int(&self) -> Option<i64> {
        match self {
            Val::Int(v) => Some(*v),
            Val::Num(v) => Some(*v as i64),
            Val::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }

    fn num(&self) -> Option<f64> {
        match self {
            Val::Int(v) => Some(*v as f64),
            Val::Num(v) => Some(*v),
            Val::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
        .filter(|v| v.is_finite())
    }

    fn text(&self) -> String {
        match self {
            Val::Int(v) => v.to_string(),
            Val::Num(v) => v.to_string(),
            Val::Text(t) => t.clone(),
            Val::Ints(v) => format!("{v:?}"),
            Val::Nums(v) => format!("{v:?}"),
            Val::None => String::new(),
        }
    }
}

fn text(b: &[u8], unicode: bool) -> String {
    let s = if unicode {
        let u: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        b.iter().map(|&c| char::from(c)).collect()
    };
    s.replace('\0', "").trim().to_owned()
}

impl Walk<'_> {
    fn image(&mut self) -> Option<&mut Image> {
        usize::try_from(self.index)
            .ok()
            .and_then(|i| self.meta.images.get_mut(i))
    }

    /// The value of a leaf of type `ty` at `at`, `size` bytes.
    fn value(&self, ty: u32, at: usize, size: usize) -> Val {
        let b = self.b;
        let d = match b.get(at..at.saturating_add(size)) {
            Some(d) if size > 0 => d,
            _ => return Val::None,
        };
        match ty {
            T_CHAR | T_UCHAR => Val::Int(i64::from(d[0])),
            T_SHORT | T_USHORT => u16_at(d, 0).map_or(Val::None, |v| Val::Int(i64::from(v as i16))),
            T_INT | T_UINT | T_DWORD | 271..=273 => {
                i32_at(d, 0).map_or(Val::None, |v| Val::Int(i64::from(v)))
            }
            T_LONG | T_ULONG | T_TIMESTAMP => i64_at(d, 0).map_or(Val::None, Val::Int),
            T_FLOAT => u32_at(d, 0).map_or(Val::None, |v| Val::Num(f64::from(f32::from_bits(v)))),
            T_DOUBLE | T_DATE => f64_at(d, 0).map_or(Val::None, Val::Num),
            12 => Val::Int(i64::from(d[0] != 0)),
            T_TCHAR | T_UNICODE => Val::Text(text(d, ty == T_UNICODE)),
            256..=259 | 267 | 274..=277 | 8195 | 8199 | 8200 | 8470 => Val::Ints(
                d.chunks_exact(4)
                    .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
            ),
            11 | 260..=266 | 268 => Val::Nums(
                d.chunks_exact(8)
                    .map(|c| f64::from_bits(u64::from_le_bytes(c.try_into().unwrap_or([0; 8]))))
                    .collect(),
            ),
            _ => Val::None,
        }
    }

    /// One container at `fp`; returns where reading continues.
    fn container(&mut self, fp: usize, prefix: &str, depth: u32) -> Option<usize> {
        let b = self.b;
        if depth > MAX_DEPTH || fp + 24 >= b.len() {
            return None;
        }
        let data_offset = usize::try_from(i64_at(b, fp + 8)?).ok()?;
        let flags = u32_at(b, fp + 16)?;
        let count = flags & 0x0fff_ffff;
        let mut at = fp.checked_add(data_offset)?;
        if at >= b.len() || count as usize > b.len() {
            return None;
        }
        for _ in 0..count {
            self.fields += 1;
            if self.fields > MAX_FIELDS || at + 16 >= b.len() {
                return None;
            }
            let ftype = u32_at(b, at)?;
            let tag = i32_at(b, at + 4)?;
            let next = u32_at(b, at + 8)? as usize;
            let size = u32_at(b, at + 12)? as usize;
            let extra = (ftype >> 27) & 1 == 1;
            let extended = (ftype >> 28) & 1 == 1;
            let inline = (ftype >> 30) & 1 == 1;
            let ty = ftype & 0x00ff_ffff;
            let mut pos = at + 16;
            let mut second = -1;
            if extra {
                second = i32_at(b, pos)?;
                pos += 4;
            }
            if tag < 0 {
                return Some(if inline {
                    pos
                } else {
                    pos.saturating_add(size)
                });
            }
            if tag == EXTERNAL_FILE_PROPERTIES && self.previous == IMAGE_FRAME_VOLUME {
                self.index += 1;
            } else if tag == DOCUMENT_PROPERTIES || tag == SLIDE_PROPERTIES {
                self.index = -1;
            }
            self.previous = tag;
            while self.index >= 0 && self.index as usize >= self.meta.images.len() {
                if self.meta.images.len() > 4096 {
                    return None;
                }
                self.meta.images.push(Image::default());
            }
            let mut stored: Option<Val> = None;
            if extended && ty == NEW_VOLUME_HEADER {
                if tag == DIMENSION_DESCRIPTION_VOLUME {
                    self.dimension_tag = second;
                    self.in_dimensions = true;
                }
                let end = pos.saturating_add(size).min(b.len());
                let mut p = pos;
                let name = volume_name(tag);
                while p < end {
                    match self.container(p, name, depth + 1) {
                        Some(q) if q > p => p = q,
                        _ => break,
                    }
                }
                if tag == DIMENSION_DESCRIPTION_VOLUME {
                    self.in_dimensions = false;
                    self.found_channel = false;
                }
            } else if extended && (ty == PROPERTY_SET_VOLUME || ty == NEW_MDIM_VOLUME_HEADER) {
                let name = if ty == NEW_MDIM_VOLUME_HEADER {
                    match (volume_name(tag), tag) {
                        ("", Z_START) => "Z start position",
                        ("", Z_INCREMENT) => "Z increment",
                        ("", Z_VALUE) => "Z value",
                        (n, _) => n,
                    }
                } else {
                    prefix
                };
                let name = name.to_owned();
                self.container(pos, &name, depth + 1);
            } else {
                let v = if inline {
                    Val::Int(size as i64)
                } else {
                    self.value(ty, pos, size)
                };
                self.leaf(tag, ty, prefix, &v);
                stored = Some(v);
            }
            if self.in_dimensions {
                let dt = self.dimension_tag;
                let found = self.found_channel;
                let mut found_now = found;
                if let Some(img) = self.image() {
                    if tag == Z_START && !img.has_dim_value(dt) {
                        img.set_dim('Z', dt);
                    } else if (tag == TIME_START || tag == DIMENSION_VALUE_ID)
                        && !img.has_dim_value(dt)
                    {
                        img.set_dim('T', dt);
                    } else if tag == LAMBDA_START && !img.has_dim_value(dt) {
                        img.set_dim('L', dt);
                    } else if tag == CHANNEL_PROPERTIES && found && !img.has_dim_value(dt) {
                        img.set_dim('C', dt);
                    } else if tag == CHANNEL_PROPERTIES {
                        found_now = true;
                    } else if tag == DIMENSION_MEANING
                        && let Some(m) = stored.as_ref().and_then(Val::int)
                    {
                        match m {
                            1 => img.set_dim('Z', dt),
                            2 => img.set_dim('T', dt),
                            3 => img.set_dim('L', dt),
                            4 => img.set_dim('C', dt),
                            _ => {}
                        }
                    }
                }
                self.found_channel = found_now;
            }
            if next == 0 {
                return Some(fp.saturating_add(size).saturating_add(32).min(b.len()));
            }
            let n = fp.checked_add(next)?;
            if n >= b.len() || n <= at {
                return None;
            }
            at = n;
        }
        Some(at)
    }

    fn leaf(&mut self, tag: i32, ty: u32, prefix: &str, v: &Val) {
        if tag == HAS_EXTERNAL_FILE {
            self.meta.external = v.int() == Some(1);
        }
        if self.index < 0 {
            let name = match tag {
                DOCUMENT_NAME if ty == T_TCHAR || ty == T_UNICODE => Some("Document name"),
                DOCUMENT_AUTHOR => Some("Document author"),
                DOCUMENT_TIME => Some("Document time"),
                _ => None,
            };
            if let (Some(n), Val::Text(t)) = (name, v)
                && !t.is_empty()
            {
                self.meta.document.push((n.to_owned(), t.clone()));
            }
            return;
        }
        let Some(img) = self.image() else {
            return;
        };
        match tag {
            CHANNEL_NAME => {
                if let Val::Text(t) = v {
                    img.channel_names.push(t.clone());
                }
            }
            STACK_NAME => {
                if let Val::Text(t) = v
                    && t != "0"
                    && img.name.is_none()
                {
                    img.name = Some(t.clone());
                }
            }
            IMAGE_BOUNDARY => {
                if let Val::Ints(i) = v
                    && i.len() >= 4
                    && img.width.is_none()
                {
                    img.width = u32::try_from(i[2]).ok();
                    img.height = u32::try_from(i[3]).ok();
                }
            }
            TILE_ORIGIN => {
                if let Val::Ints(i) = v
                    && i.len() >= 2
                {
                    img.tile_origin = Some((i[0], i[1]));
                }
            }
            RWC_FRAME_SCALE => {
                if let Val::Nums(d) = v
                    && d.len() >= 2
                    && img.physical.is_none()
                {
                    img.physical = Some((d[0], d[1]));
                }
            }
            RWC_FRAME_ORIGIN if matches!(v, Val::Nums(_)) => {
                if let Val::Nums(d) = v
                    && d.len() >= 2
                    && img.origin.is_none()
                {
                    img.origin = Some((d[0], d[1]));
                }
            }
            EXPOSURE_TIME if prefix.is_empty() => {
                if let Some(e) = v.int() {
                    img.exposures.push(e);
                }
            }
            CREATION_TIME if img.acquired.is_none() => img.acquired = v.int(),
            OBJECTIVE_MAG => img.magnification = v.num(),
            NUMERICAL_APERTURE => img.numerical_aperture = v.num(),
            OBJECTIVE_NAME => img.objectives.push((v.text(), 0)),
            OBJECTIVE_TYPE => {
                if let (Some(o), Some(t)) = (img.objectives.last_mut(), v.int()) {
                    o.1 = t;
                }
            }
            BIT_DEPTH => img.bit_depth = v.int(),
            DEFAULT_BACKGROUND_COLOR => img.fill = v.int().map(|b| b as u8),
            DEVICE_SUBTYPE => img
                .devices
                .push((device_type(v.int().unwrap_or(-1)), String::new())),
            DEVICE_NAME => {
                let t = v.text();
                match img.devices.last_mut() {
                    Some(d) if d.1.is_empty() => d.1 = t,
                    _ => img.devices.push((String::new(), t)),
                }
            }
            VALUE => match prefix {
                "Channel Wavelength " => {
                    if let Some(w) = v.num() {
                        img.wavelengths.push(w);
                    }
                }
                "Z start position" => img.z_start = v.num(),
                "Z increment" => img.z_increment = v.num(),
                "Z value" => {
                    if let Some(z) = v.num() {
                        img.z_values.push(z);
                    }
                }
                "Timestamp " => {
                    if let Some(t) = v.num() {
                        img.t_values.push(t);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn volume_name(tag: i32) -> &'static str {
    match tag {
        OPTICAL_PATH => "Microscope ",
        WAVELENGTH_VOLUME => "Channel Wavelength ",
        WORKING_DISTANCE => "Objective Working Distance ",
        TIME_VALUE => "Timestamp ",
        20051 => "Calibration Function ",
        _ => "",
    }
}

fn device_type(t: i64) -> String {
    match t {
        0 => "Camera".into(),
        10000 => "Stage".into(),
        20006 => "Objective".into(),
        20002 => "Filter wheel".into(),
        20003 => "Lamp".into(),
        40500 => "Microscope frame".into(),
        t => t.to_string(),
    }
}

/// The metadata after the TIFF header.
pub fn metadata(data: &[u8]) -> Meta {
    let mut w = Walk {
        b: data,
        meta: Meta::default(),
        index: -1,
        previous: 0,
        dimension_tag: 0,
        in_dimensions: false,
        found_channel: false,
        fields: 0,
    };
    w.container(8, "", 0);
    w.meta
}

/// A stored sample type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Sample {
    fn from_code(c: i32) -> Option<Sample> {
        Some(match c {
            1 => Sample::I8,
            2 => Sample::U8,
            3 => Sample::I16,
            4 => Sample::U16,
            5 => Sample::I32,
            6 => Sample::U32,
            9 => Sample::F32,
            10 => Sample::F64,
            _ => return None,
        })
    }

    fn bytes(self) -> usize {
        match self {
            Sample::I8 | Sample::U8 => 1,
            Sample::I16 | Sample::U16 => 2,
            Sample::I32 | Sample::U32 | Sample::F32 => 4,
            Sample::F64 => 8,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Sample::I8 => "8-bit signed",
            Sample::U8 => "8-bit",
            Sample::I16 => "16-bit signed",
            Sample::U16 => "16-bit unsigned",
            Sample::I32 => "32-bit signed",
            Sample::U32 => "32-bit unsigned",
            Sample::F32 => "32-bit float",
            Sample::F64 => "64-bit float",
        }
    }

    fn get(self, d: &[u8], i: usize) -> f32 {
        let o = i * self.bytes();
        let b = |n: usize| d.get(o..o + n);
        match self {
            Sample::I8 => b(1).map_or(0.0, |v| f32::from(v[0] as i8)),
            Sample::U8 => b(1).map_or(0.0, |v| f32::from(v[0])),
            Sample::I16 => b(2).map_or(0.0, |v| f32::from(i16::from_le_bytes([v[0], v[1]]))),
            Sample::U16 => b(2).map_or(0.0, |v| f32::from(u16::from_le_bytes([v[0], v[1]]))),
            Sample::I32 => {
                b(4).map_or(0.0, |v| i32::from_le_bytes([v[0], v[1], v[2], v[3]]) as f32)
            }
            Sample::U32 => {
                b(4).map_or(0.0, |v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]) as f32)
            }
            Sample::F32 => b(4).map_or(0.0, |v| f32::from_le_bytes([v[0], v[1], v[2], v[3]])),
            Sample::F64 => b(8).map_or(0.0, |v| {
                f64::from_le_bytes(v.try_into().unwrap_or([0; 8])) as f32
            }),
        }
    }
}

/// Tile compression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Raw,
    Jpeg,
    Jpeg2000,
    JpegLossless,
    Png,
    Bmp,
}

impl Compression {
    fn from_code(c: i32) -> Option<Compression> {
        Some(match c {
            0 => Compression::Raw,
            2 => Compression::Jpeg,
            3 => Compression::Jpeg2000,
            5 => Compression::JpegLossless,
            8 => Compression::Png,
            9 => Compression::Bmp,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Compression::Raw => "uncompressed",
            Compression::Jpeg => "JPEG",
            Compression::Jpeg2000 => "JPEG 2000",
            Compression::JpegLossless => "lossless JPEG",
            Compression::Png => "PNG",
            Compression::Bmp => "BMP",
        }
    }
}

/// A frame file's header and chunk table.
#[derive(Clone, Debug)]
pub struct Ets {
    pub dims: usize,
    pub sample: Sample,
    /// Samples per pixel.
    pub samples: usize,
    pub compression: Compression,
    pub tile_w: u32,
    pub tile_h: u32,
    pub background: Vec<u8>,
    /// Raw samples in blue, green, red order.
    pub bgr: bool,
    pub pyramid: bool,
    /// Chunk coordinates to (offset, size).
    pub chunks: HashMap<Vec<i32>, (u64, u32)>,
    /// Per pyramid level, the largest value of each coordinate.
    pub max: Vec<Vec<i32>>,
}

/// Parse a frame file's headers and chunk table.
pub fn parse_ets(b: &[u8]) -> Result<Ets> {
    let bad = |m: &str| Error::Malformed(format!("ETS: {m}"));
    if !b.starts_with(b"SIS") {
        return Err(bad("no SIS header"));
    }
    let dims = i32_at(b, 12).ok_or_else(|| bad("truncated"))?;
    let dims = usize::try_from(dims)
        .ok()
        .filter(|&d| (2..=MAX_DIMS).contains(&d))
        .ok_or_else(|| bad("dimensions"))?;
    let extra = usize::try_from(i64_at(b, 16).ok_or_else(|| bad("truncated"))?)
        .map_err(|_| bad("header offset"))?;
    let chunk_at = usize::try_from(i64_at(b, 32).ok_or_else(|| bad("truncated"))?)
        .map_err(|_| bad("chunk offset"))?;
    let n = usize::try_from(i32_at(b, 40).ok_or_else(|| bad("truncated"))?)
        .map_err(|_| bad("chunk count"))?;
    if n > MAX_CHUNKS {
        return Err(Error::TooLarge);
    }
    if b.get(extra..extra + 3) != Some(b"ETS") {
        return Err(bad("no ETS header"));
    }
    let e = |k: usize| i32_at(b, extra + 8 + 4 * k).ok_or_else(|| bad("truncated header"));
    let sample =
        Sample::from_code(e(0)?).ok_or_else(|| Error::Unsupported("VSI sample type".into()))?;
    let samples = usize::try_from(e(1)?)
        .ok()
        .filter(|&s| (1..=16).contains(&s))
        .ok_or_else(|| bad("samples per pixel"))?;
    let compression = Compression::from_code(e(3)?).ok_or_else(|| {
        Error::Unsupported(format!("VSI tile compression {}", e(3).unwrap_or(-1)))
    })?;
    let (tw, th) = (e(5)?, e(6)?);
    if !(1..=1 << 17).contains(&tw) || !(1..=1 << 17).contains(&th) {
        return Err(bad("tile size"));
    }
    // After 17 pixel-information words: 40 bytes of background colour, the component order and
    // whether there is a pyramid.
    let bg_at = extra + 8 + 4 * 8 + 4 * 17;
    let bg_len = (samples * sample.bytes()).min(40);
    let background = b
        .get(bg_at..bg_at + bg_len)
        .ok_or_else(|| bad("truncated header"))?
        .to_vec();
    let order = i32_at(b, bg_at + 40).ok_or_else(|| bad("truncated header"))?;
    let pyramid = i32_at(b, bg_at + 44).ok_or_else(|| bad("truncated header"))? != 0;
    let entry = 4 + 4 * dims + 8 + 4 + 4;
    let table = b
        .get(
            chunk_at
                ..chunk_at
                    .checked_add(entry.checked_mul(n).ok_or(Error::TooLarge)?)
                    .ok_or(Error::TooLarge)?,
        )
        .ok_or_else(|| Error::Truncated("ETS chunk table".into()))?;
    let mut chunks = HashMap::with_capacity(n);
    let mut max: Vec<Vec<i32>> = Vec::new();
    for c in table.chunks_exact(entry) {
        let coords: Vec<i32> = (0..dims).filter_map(|k| i32_at(c, 4 + 4 * k)).collect();
        let off = i64_at(c, 4 + 4 * dims).unwrap_or(-1);
        let size = i32_at(c, 12 + 4 * dims).unwrap_or(-1);
        let (Ok(off), Ok(size)) = (u64::try_from(off), u32::try_from(size)) else {
            continue;
        };
        if coords.iter().any(|&v| v < 0) {
            continue;
        }
        let level = if pyramid {
            coords[dims - 1] as usize
        } else {
            0
        };
        if level > 30 {
            continue;
        }
        if max.len() <= level {
            max.resize(level + 1, vec![0; dims]);
        }
        for (m, &v) in max[level].iter_mut().zip(&coords) {
            *m = (*m).max(v);
        }
        chunks.insert(coords, (off, size));
    }
    if chunks.is_empty() {
        return Err(bad("no tiles"));
    }
    Ok(Ets {
        dims,
        sample,
        samples,
        compression,
        tile_w: tw as u32,
        tile_h: th as u32,
        background,
        bgr: order == 1 && compression == Compression::Raw,
        pyramid,
        chunks,
        max,
    })
}

/// Where Z, C and T are among a chunk's coordinates (as the format's readers place them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimMap {
    pub z: Option<usize>,
    pub c: Option<usize>,
    pub t: Option<usize>,
}

fn dim_map(ets: &Ets, img: &Image) -> DimMap {
    let n = ets.dims as i64;
    let upper = if ets.pyramid { n - 1 } else { n };
    let at = |k: char| img.dim(k).map(|v| i64::from(v) + 2);
    let (mut z, mut c, mut t) = (at('Z'), at('C'), at('T'));
    if ets.pyramid && t == Some(n - 1) {
        t = None;
    }
    if ets.pyramid && z == Some(n - 1) {
        z = None;
    }
    let outside = |v: Option<i64>| v.is_none_or(|v| v < 0 || v >= upper);
    if outside(t) && outside(z) && outside(c) {
        t = t.map(|v| v - 1);
        z = z.map(|v| v - 1);
        c = c.map(|v| v - 1);
    }
    if img.dim('T').is_none() && img.dim('Z').is_none() {
        if n > 4 && c.is_none() {
            c = Some(2);
        }
        if n > 4 {
            let ti = if img.dim('C').is_none() {
                3
            } else {
                c.unwrap_or(2) + 2
            };
            t = (ti < n).then_some(ti);
        }
        if n > 5 {
            let zi = if img.dim('C').is_none() {
                4
            } else {
                c.unwrap_or(2) + 1
            };
            z = (zi < n).then_some(zi);
        }
    }
    let ok = |v: Option<i64>| v.filter(|&v| v >= 2 && v < upper).map(|v| v as usize);
    let m = DimMap {
        z: ok(z),
        c: ok(c),
        t: ok(t),
    };
    // The positions of two axes never coincide.
    if m.z.is_some() && (m.z == m.c || m.z == m.t) {
        return DimMap { z: None, ..m };
    }
    m
}

/// One frame file read as an image.
struct Frame {
    file: usize,
    ets: Ets,
    map: DimMap,
    /// Tile columns per level.
    cols: Vec<u32>,
}

impl Frame {
    fn coords(&self, level: usize, src: u32, cdim: u32, pos: Position) -> Vec<i32> {
        let cols = self.cols.get(level).copied().unwrap_or(1).max(1);
        let mut c = vec![0i32; self.ets.dims];
        c[0] = (src % cols) as i32;
        c[1] = (src / cols) as i32;
        if let Some(i) = self.map.z {
            c[i] = pos.z as i32;
        }
        if let Some(i) = self.map.t {
            c[i] = pos.t as i32;
        }
        if let Some(i) = self.map.c {
            c[i] = cdim as i32;
        }
        if self.ets.pyramid {
            c[self.ets.dims - 1] = level as i32;
        }
        c
    }

    fn bytes<'a>(&self, files: Files<'a>, coords: &[i32]) -> Result<Option<&'a [u8]>> {
        let Some(&(off, size)) = self.ets.chunks.get(coords) else {
            return Ok(None);
        };
        let data = files.get(self.file)?;
        let (o, n) = (
            usize::try_from(off).map_err(|_| Error::TooLarge)?,
            size as usize,
        );
        data.get(o..o.saturating_add(n))
            .map(Some)
            .ok_or_else(|| Error::Truncated("VSI tile past the end of its frame file".into()))
    }

    /// A tile's samples (interleaved) as values.
    fn values(&self, b: &[u8], cancel: Option<&AtomicBool>) -> Result<(u32, u32, usize, Vec<f32>)> {
        let (tw, th) = (self.ets.tile_w, self.ets.tile_h);
        match self.ets.compression {
            Compression::Raw => {
                let s = self.ets.samples;
                let n = tw as usize * th as usize * s;
                // The tile size is the header's: the samples must be there before they are
                // expanded to floats (a missing one would read as 0).
                if n.saturating_mul(self.ets.sample.bytes()) > b.len() {
                    return Err(Error::Truncated("VSI tile shorter than its size".into()));
                }
                let mut v: Vec<f32> = (0..n).map(|i| self.ets.sample.get(b, i)).collect();
                if self.ets.bgr && s >= 3 {
                    for p in v.chunks_exact_mut(s) {
                        p.swap(0, 2);
                    }
                }
                Ok((tw, th, s, v))
            }
            _ => crate::blob::values(b, cancel),
        }
    }
}

struct VsiTiles {
    frame: Frame,
    /// RGB slide: tiles decode as colour.
    rgb: bool,
}

impl TileSource for VsiTiles {
    fn rgba(
        &self,
        files: Files,
        level: usize,
        src: u32,
        pos: Position,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Rgba>> {
        let f = &self.frame;
        let Some(b) = f.bytes(files, &f.coords(level, src, 0, pos))? else {
            return Ok(None);
        };
        if f.ets.compression != Compression::Raw && f.ets.sample == Sample::U8 && !f.ets.bgr {
            return crate::blob::rgba(b, cancel).map(Some);
        }
        let (w, h, c, v) = f.values(b, cancel)?;
        let mut data = Vec::with_capacity(w as usize * h as usize * 4);
        for p in v.chunks_exact(c.max(1)) {
            let g = |k: usize| p[k.min(c - 1)].clamp(0.0, 255.0) as u8;
            data.extend_from_slice(&[g(0), g(1), g(2), 255]);
        }
        Ok(Some(Rgba {
            width: w,
            height: h,
            data,
            truncated: false,
        }))
    }

    fn channel(
        &self,
        files: Files,
        level: usize,
        src: u32,
        channel: u32,
        pos: Position,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Plane>> {
        let f = &self.frame;
        if self.rgb {
            return Err(Error::Unsupported("channels of an RGB slide".into()));
        }
        let s = f.ets.samples as u32;
        let (cdim, sample) = (channel / s, (channel % s) as usize);
        let Some(b) = f.bytes(files, &f.coords(level, src, cdim, pos))? else {
            return Ok(None);
        };
        let (w, h, c, v) = f.values(b, cancel)?;
        let k = sample.min(c.saturating_sub(1));
        Ok(Some(Plane {
            width: w,
            height: h,
            values: v.chunks_exact(c.max(1)).map(|p| p[k]).collect(),
            truncated: false,
        }))
    }
}

/// The level sizes of a frame: the image's size, halved per level (an odd size rounds up
/// while the stored tiles reach that far).
fn level_sizes(ets: &Ets, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut out = vec![(w, h)];
    for m in ets.max.iter().skip(1) {
        let (pw, ph) = *out.last().unwrap_or(&(w, h));
        let mx = ets.tile_w.saturating_mul(m[0].max(0) as u32 + 1);
        let my = ets.tile_h.saturating_mul(m[1].max(0) as u32 + 1);
        let half = |p: u32, max: u32| {
            let mut v = p / 2;
            if p % 2 == 1 && v < max {
                v += 1;
            } else if v > max {
                v = max;
            }
            v
        };
        let (nw, nh) = (half(pw, mx), half(ph, my));
        if nw == 0 || nh == 0 {
            break;
        }
        out.push((nw, nh));
    }
    out
}

/// A frame file as a stitched image: its mosaic, size, channels, Z and T.
struct Built {
    stitched: Stitched,
    width: u32,
    height: u32,
    levels: Vec<Level>,
    size_z: u32,
    size_t: u32,
    rgb: bool,
}

/// The colour of an emission wavelength (nm).
fn wavelength_colour(nm: f64) -> Option<[u8; 3]> {
    Some(match nm {
        n if !(350.0..=800.0).contains(&n) => return None,
        n if n < 480.0 => [0, 0, 255],
        n if n < 540.0 => [0, 255, 0],
        n if n < 600.0 => [255, 255, 0],
        n if n < 650.0 => [255, 0, 0],
        _ => [255, 0, 255],
    })
}

fn build(file: usize, ets: Ets, img: Image) -> Result<Built> {
    let map = dim_map(&ets, &img);
    let m0 = ets
        .max
        .first()
        .cloned()
        .unwrap_or_else(|| vec![0; ets.dims]);
    let at = |i: Option<usize>| i.map_or(1, |i| m0[i].max(0) as u32 + 1);
    let (cdims, size_z, size_t) = (at(map.c), at(map.z), at(map.t));
    let size_c = (cdims as usize).saturating_mul(ets.samples);
    let (tw, th) = (ets.tile_w, ets.tile_h);
    let width = img.width.unwrap_or(tw.saturating_mul(m0[0] as u32 + 1));
    let height = img.height.unwrap_or(th.saturating_mul(m0[1] as u32 + 1));
    if width == 0 || height == 0 {
        return Err(Error::Malformed("VSI image without size".into()));
    }
    let rgb = ets.sample == Sample::U8 && ets.samples >= 3 && cdims == 1;
    let sizes = level_sizes(&ets, width, height);
    let mut levels = Vec::new();
    let mut slide_levels = Vec::new();
    let mut cols = Vec::new();
    for (l, &(lw, lh)) in sizes.iter().enumerate() {
        let m = &ets.max[l];
        let (nc, nr) = (m[0].max(0) as u32 + 1, m[1].max(0) as u32 + 1);
        if u64::from(nc) * u64::from(nr) > MAX_CHUNKS as u64 {
            return Err(Error::TooLarge);
        }
        let f = 1u32 << l;
        let (ox, oy) = img
            .tile_origin
            .map_or((0, 0), |(x, y)| (x / f as i32, y / f as i32));
        let mut tiles = Vec::with_capacity(nc as usize * nr as usize);
        for r in 0..nr {
            for c in 0..nc {
                tiles.push(Placed {
                    x: i64::from(c) * i64::from(tw) + i64::from(ox),
                    y: i64::from(r) * i64::from(th) + i64::from(oy),
                    w: tw,
                    h: th,
                    src: r * nc + c,
                    sx: 0,
                    sy: 0,
                });
            }
        }
        cols.push(nc);
        levels.push(MosaicLevel::new(lw, lh, f, tiles));
        slide_levels.push(Level {
            width: lw,
            height: lh,
            downsample: f64::from(width) / f64::from(lw),
        });
    }
    let bgv = |k: usize| ets.sample.get(&ets.background, k);
    let background = if rgb {
        let g = |k: usize| bgv(if ets.bgr { 2 - k } else { k }).clamp(0.0, 255.0) as u8;
        [g(0), g(1), g(2), 255]
    } else {
        [0, 0, 0, 255]
    };
    let bg_value = if rgb { 0.0 } else { bgv(0) };
    let channels: Vec<Channel> = if rgb {
        vec![Channel::default()]
    } else {
        (0..size_c.max(1))
            .map(|k| Channel {
                name: img.channel_names.get(k).cloned().unwrap_or_default(),
                colour: img
                    .wavelengths
                    .get(k)
                    .and_then(|&w| wavelength_colour(w))
                    .or_else(|| (size_c == 1).then_some([255, 255, 255])),
                window: None,
            })
            .collect()
    };
    let frame = Frame {
        file,
        ets,
        map,
        cols,
    };
    let mut mosaic = Mosaic::new(levels, background, Box::new(VsiTiles { frame, rgb }));
    mosaic.background_value = bg_value;
    Ok(Built {
        stitched: Stitched::new(mosaic, !rgb, channels, Vec::new()),
        width,
        height,
        levels: slide_levels,
        size_z,
        size_t,
        rgb,
    })
}

fn is_overview(name: &str) -> bool {
    name.eq_ignore_ascii_case("overview")
}

fn is_label(name: &str) -> bool {
    name.eq_ignore_ascii_case("label")
}

/// Whether the metadata image `img` fits a frame whose level 0 holds `ets`'s tiles.
fn fits(img: &Image, ets: &Ets) -> bool {
    let m = &ets.max[0];
    let (mw, mh) = (
        u64::from(ets.tile_w) * (m[0] as u64 + 1),
        u64::from(ets.tile_h) * (m[1] as u64 + 1),
    );
    let (Some(w), Some(h)) = (img.width, img.height) else {
        return false;
    };
    let (w, h) = (u64::from(w), u64::from(h));
    w <= mw + u64::from(ets.tile_w)
        && w + u64::from(ets.tile_w) >= mw
        && h <= mh + u64::from(ets.tile_h)
        && h + u64::from(ets.tile_h) >= mh
}

/// Open a VSI slide. `members[0]` is the `.vsi` file (its name and bytes); the others are its
/// frame files, named by their path from the `.vsi`'s folder (`_name_/stackN/frame_t.ets`),
/// in name order. The frame files used are [`Slide::files`].
pub fn open(members: &[(&str, &[u8])]) -> Result<Slide> {
    let (_, vsi) = *members
        .first()
        .ok_or_else(|| Error::Malformed("no VSI file".into()))?;
    if !probe(vsi) {
        return Err(Error::NotSlide);
    }
    let meta = metadata(vsi);
    let mut frames = Vec::new();
    for (k, (name, data)) in members.iter().enumerate().skip(1) {
        // A frame file that does not read is left out: its image is missing.
        if let Ok(e) = parse_ets(data) {
            frames.push((k, name.to_string(), e));
        }
    }
    // Match frames with images: in order, or by size when there are more frames.
    let mut used = vec![false; meta.images.len()];
    let orphans = frames.len() > meta.images.len();
    let mut matched: Vec<(usize, String, Ets, Image)> = Vec::new();
    for (i, (k, name, ets)) in frames.into_iter().enumerate() {
        let pick = if !orphans
            && meta
                .images
                .get(i)
                .is_some_and(|m| fits(m, &ets) || m.width.is_none())
            && !used[i]
        {
            Some(i)
        } else {
            (0..meta.images.len()).find(|&j| !used[j] && fits(&meta.images[j], &ets))
        };
        if let Some(j) = pick {
            used[j] = true;
            matched.push((k, name, ets, meta.images[j].clone()));
        } else if !orphans {
            matched.push((k, name, ets, Image::default()));
        }
    }
    let tiff = specular_tiff::Directories::read(vsi).ok();
    let preview = tiff.as_ref().and_then(|d| {
        let p = crate::tiffslide::page_of(d, 0)?;
        p.check_supported().ok()?;
        Some((p.width(), p.height()))
    });
    if matched.is_empty() {
        let mut why = "the VSI file's frame files (.ets) are missing".to_owned();
        if !meta.external && preview.is_some() {
            why = "the VSI file holds no slide image".into();
        }
        return Err(Error::Unsupported(why));
    }
    let slide_like = |i: &Image| {
        i.name
            .as_deref()
            .is_none_or(|n| !is_overview(n) && !is_label(n))
    };
    let main = matched.iter().position(|m| slide_like(&m.3));
    // The metadata names a slide image but only its label or overview is there.
    if main.is_none()
        && meta
            .images
            .iter()
            .any(|i| slide_like(i) && i.width.is_some())
    {
        return Err(Error::Unsupported(
            "the slide image's frame file (.ets) is missing beside the VSI file".into(),
        ));
    }
    let main = main.unwrap_or(0);
    let mut built = Vec::new();
    for (i, (k, name, ets, img)) in matched.into_iter().enumerate() {
        let label = img.name.clone().unwrap_or_else(|| name.clone());
        match build(k, ets, img.clone()) {
            Ok(b) => built.push((k, name, b, img, label)),
            Err(e) if i == main => return Err(e),
            // Another image that does not read is left out.
            Err(_) => {}
        }
    }
    let main = built
        .iter()
        .position(|b| {
            b.3.name
                .as_deref()
                .is_none_or(|n| !is_overview(n) && !is_label(n))
        })
        .unwrap_or(0);
    // Frames keep their member's index: the files are every member after the `.vsi`.
    let files: Vec<String> = members.iter().skip(1).map(|m| m.0.to_owned()).collect();
    let (_, _, m, img, _) = built.remove(main);
    let mut assoc = Vec::new();
    let mut assoc_sizes = Vec::new();
    for (_, _, b, _, label) in built {
        let kind = if is_label(&label) {
            AssociatedKind::Label
        } else if is_overview(&label) {
            AssociatedKind::Macro
        } else {
            AssociatedKind::Other(label)
        };
        assoc_sizes.push((kind, b.width, b.height));
        assoc.push(Assoc::Stitched(Box::new(b.stitched), b.width, b.height));
    }
    if let Some((w, h)) = preview {
        assoc_sizes.push((AssociatedKind::Thumbnail, w, h));
        assoc.push(Assoc::Tiff { ifd: 0 });
    }
    let mut props = Props::new(Vendor::Olympus);
    props.mpp = img
        .physical
        .filter(|p| p.0 > 0.0 && p.1 > 0.0 && p.0.is_finite());
    props.objective = img.magnification.filter(|m| *m > 0.0);
    props.name = img.name.clone();
    props.z_step = img
        .z_increment
        .filter(|z| *z != 0.0)
        .or_else(|| (img.z_values.len() > 1).then(|| img.z_values[1] - img.z_values[0]));
    props.t_step = (img.t_values.len() > 1).then(|| (img.t_values[1] - img.t_values[0]) / 1000.0);
    props.date = img.acquired.and_then(unix_date);
    props.scanner = img
        .devices
        .iter()
        .find(|d| d.0 == "Camera" && !d.1.is_empty())
        .map(|d| d.1.clone());
    let frame = m
        .stitched
        .frame()
        .map(|f| (f.ets.sample, f.ets.compression, f.ets.samples));
    if let Some((s, c, n)) = frame {
        props.sample = format!(
            "{}{}, {}",
            s.name(),
            if m.rgb {
                " RGB"
            } else if n > 1 {
                " multi-sample"
            } else {
                ""
            },
            c.name()
        );
    }
    if let Some(o) = img
        .objectives
        .iter()
        .find(|o| o.1 == 1)
        .or(img.objectives.first())
    {
        props.properties.push(("Objective".into(), o.0.clone()));
    }
    if let Some(na) = img.numerical_aperture {
        props
            .properties
            .push(("Numerical aperture".into(), format!("{na}")));
    }
    for (k, w) in img.wavelengths.iter().enumerate() {
        let name = img
            .channel_names
            .get(k)
            .cloned()
            .unwrap_or_else(|| format!("{}", k + 1));
        props
            .properties
            .push((format!("Channel {name}"), format!("emission {w} nm")));
    }
    if let Some(e) = img.exposures.first() {
        props
            .properties
            .push(("Exposure".into(), format!("{} ms", *e as f64 / 1000.0)));
    }
    if let Some(b) = img.bit_depth {
        props.properties.push(("Bit depth".into(), b.to_string()));
    }
    props.properties.extend(meta.document.iter().cloned());
    let channels = m.stitched.channels.clone();
    Ok(Slide {
        props,
        width: m.width,
        height: m.height,
        levels: m.levels,
        channels: if m.rgb {
            vec![Channel::default()]
        } else {
            channels
        },
        size_z: m.size_z,
        size_t: m.size_t,
        associated: assoc_sizes,
        files,
        backend: crate::Backend::Stitched(Box::new({
            let mut s = m.stitched;
            s.assoc = assoc;
            s
        })),
    })
}

/// "2025-11-12 10:35:03 UTC" from Unix seconds.
fn unix_date(s: i64) -> Option<String> {
    if !(0..=253_402_300_799).contains(&s) {
        return None;
    }
    let days = s.div_euclid(86_400);
    let rem = s.rem_euclid(86_400);
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

impl Stitched {
    /// The VSI frame behind this mosaic, when it is one.
    fn frame(&self) -> Option<&Frame> {
        self.mosaic
            .source_any()
            .downcast_ref::<VsiTiles>()
            .map(|t| &t.frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_from_unix_seconds() {
        assert_eq!(unix_date(0).as_deref(), Some("1970-01-01 00:00:00 UTC"));
        assert_eq!(
            unix_date(1_762_943_703).as_deref(),
            Some("2025-11-12 10:35:03 UTC")
        );
        assert_eq!(unix_date(-5), None);
    }

    #[test]
    fn probe_needs_the_metadata_header() {
        let mut v = b"II*\0\x08\0\0\0".to_vec();
        v.extend_from_slice(&24u16.to_le_bytes());
        v.extend_from_slice(&21321u16.to_le_bytes());
        assert!(probe(&v));
        v[10] = 0;
        assert!(!probe(&v));
        assert!(!probe(b"II*\0"));
    }

    /// An ETS file of `dims` dimensions with the given chunks (coordinates).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ets(
        dims: usize,
        pyramid: bool,
        chunks: &[Vec<i32>],
        tile: (u32, u32),
        sample: i32,
        samples: i32,
        comp: i32,
        data: &[Vec<u8>],
    ) -> Vec<u8> {
        let mut b = vec![0u8; 64];
        b[..4].copy_from_slice(b"SIS\0");
        b[4..8].copy_from_slice(&64i32.to_le_bytes());
        b[8..12].copy_from_slice(&3i32.to_le_bytes());
        b[12..16].copy_from_slice(&(dims as i32).to_le_bytes());
        b[16..24].copy_from_slice(&64i64.to_le_bytes());
        b[24..28].copy_from_slice(&228i32.to_le_bytes());
        let mut e = vec![0u8; 228];
        e[..4].copy_from_slice(b"ETS\0");
        let words = [
            sample,
            samples,
            1,
            comp,
            90,
            tile.0 as i32,
            tile.1 as i32,
            1,
        ];
        for (k, w) in words.iter().enumerate() {
            e[8 + 4 * k..12 + 4 * k].copy_from_slice(&w.to_le_bytes());
        }
        let bg = 8 + 32 + 68;
        e[bg + 40..bg + 44].copy_from_slice(&(-1i32).to_le_bytes());
        e[bg + 44..bg + 48].copy_from_slice(&i32::from(pyramid).to_le_bytes());
        b.extend_from_slice(&e);
        let table_at = b.len();
        let entry = 4 + 4 * dims + 16;
        let data_at = table_at + entry * chunks.len();
        let mut table = Vec::new();
        let mut blobs = Vec::new();
        let mut off = data_at;
        for (c, d) in chunks.iter().zip(data) {
            table.extend_from_slice(&0i32.to_le_bytes());
            for v in c {
                table.extend_from_slice(&v.to_le_bytes());
            }
            table.extend_from_slice(&(off as i64).to_le_bytes());
            table.extend_from_slice(&(d.len() as i32).to_le_bytes());
            table.extend_from_slice(&0i32.to_le_bytes());
            blobs.extend_from_slice(d);
            off += d.len();
        }
        b.extend_from_slice(&table);
        b.extend_from_slice(&blobs);
        b[32..40].copy_from_slice(&(table_at as i64).to_le_bytes());
        b[40..44].copy_from_slice(&(chunks.len() as i32).to_le_bytes());
        b
    }

    #[test]
    fn ets_tables_and_raw_tiles() {
        // 2×1 tiles of 2×2 u16 pixels, two levels.
        let tile = |v: u16| v.to_le_bytes().repeat(4);
        let f = ets(
            3,
            true,
            &[vec![0, 0, 0], vec![1, 0, 0], vec![0, 0, 1]],
            (2, 2),
            4,
            1,
            0,
            &[tile(10), tile(20), tile(30)],
        );
        let e = parse_ets(&f).unwrap();
        assert_eq!(
            (e.dims, e.samples, e.sample, e.compression),
            (3, 1, Sample::U16, Compression::Raw)
        );
        assert_eq!(e.max, vec![vec![1, 0, 0], vec![0, 0, 1]]);
        let b = build(
            1,
            e,
            Image {
                width: Some(4),
                height: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((b.width, b.height, b.levels.len(), b.rgb), (4, 2, 2, false));
        let data: Vec<&[u8]> = vec![b"", &f];
        let files = Files { data: &data };
        let o = crate::ReadOptions {
            scale: 1,
            region: Some(crate::Region {
                x: 0,
                y: 0,
                w: 4,
                h: 2,
            }),
            ..Default::default()
        };
        let p = b.stitched.read_channel(files, 0, &o).unwrap();
        assert_eq!(p.values, [10.0, 10.0, 20.0, 20.0, 10.0, 10.0, 20.0, 20.0]);
        let o2 = crate::ReadOptions {
            scale: 2,
            region: Some(crate::Region {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            }),
            ..Default::default()
        };
        let p = b.stitched.read_channel(files, 0, &o2).unwrap();
        assert_eq!(p.values, [30.0, 30.0]);
    }

    /// A raw tile declared 131072² (64 GiB of floats) with 8 bytes behind it: refused before
    /// the samples are expanded.
    #[test]
    fn raw_tile_larger_than_its_bytes_is_refused() {
        let big = 1 << 17;
        let f = ets(
            3,
            false,
            &[vec![0, 0, 0]],
            (big, big),
            4,
            1,
            0,
            &[vec![0; 8]],
        );
        let frame = Frame {
            file: 0,
            ets: parse_ets(&f).unwrap(),
            map: DimMap {
                z: None,
                c: None,
                t: None,
            },
            cols: vec![1],
        };
        assert!(matches!(
            frame.values(&[0; 8], None),
            Err(Error::Truncated(_))
        ));
    }

    #[test]
    fn hostile_ets_headers_are_refused() {
        assert!(parse_ets(b"SIS\0").is_err());
        let mut f = ets(3, false, &[vec![0, 0, 0]], (2, 2), 2, 1, 0, &[vec![0; 4]]);
        f[40..44].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(parse_ets(&f).is_err());
        let mut g = ets(3, false, &[vec![0, 0, 0]], (2, 2), 2, 1, 0, &[vec![0; 4]]);
        g[12..16].copy_from_slice(&1000i32.to_le_bytes());
        assert!(parse_ets(&g).is_err());
    }

    #[test]
    fn metadata_walk_survives_garbage() {
        let mut v = b"II*\0\x08\0\0\0".to_vec();
        v.extend_from_slice(&24u16.to_le_bytes());
        v.extend_from_slice(&21321u16.to_le_bytes());
        v.extend_from_slice(&[0xff; 64]);
        let m = metadata(&v);
        assert!(m.images.is_empty());
    }
}

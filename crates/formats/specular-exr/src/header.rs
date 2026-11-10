//! Header, attribute and offset-table parsing.

use std::ops::Range;

use crate::error::{ExrError, Result};

pub(crate) const MAGIC: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];
const FLAG_TILED: u32 = 0x200;
const FLAG_LONG_NAMES: u32 = 0x400;
const FLAG_DEEP: u32 = 0x800;
const FLAG_MULTIPART: u32 = 0x1000;
/// Most parts we list in a multi-part file.
const MAX_PARTS: usize = 1 << 12;
/// Most channels per part.
const MAX_CHANNELS: usize = 1 << 12;

/// Little-endian cursor over a byte slice; every read is bounds-checked.
#[derive(Clone)]
pub(crate) struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos }
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(ExrError::Truncated)?;
        let s = self.data.get(self.pos..end).ok_or(ExrError::Truncated)?;
        self.pos = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn u64(&mut self) -> Result<u64> {
        let b = self.bytes(8)?;
        Ok(u64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// Null-terminated string of at most `max` bytes (without the terminator).
    pub fn cstr(&mut self, max: usize) -> Result<&'a [u8]> {
        let rest = self.data.get(self.pos..).ok_or(ExrError::Truncated)?;
        let n = rest
            .iter()
            .take(max + 1)
            .position(|&b| b == 0)
            .ok_or(if rest.len() <= max {
                ExrError::Truncated
            } else {
                ExrError::InvalidHeader("name too long")
            })?;
        self.pos += n + 1;
        Ok(&rest[..n])
    }
}

/// Sample type of a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelType {
    /// 32-bit unsigned integer.
    Uint,
    /// 16-bit float.
    Half,
    /// 32-bit float.
    Float,
}

impl PixelType {
    pub fn bytes(self) -> usize {
        match self {
            PixelType::Half => 2,
            PixelType::Uint | PixelType::Float => 4,
        }
    }
}

/// One entry of the `channels` attribute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub name: String,
    pub pixel_type: PixelType,
    /// `pLinear`: the channel is perceptually linear (affects B44 only).
    pub p_linear: bool,
    pub x_sampling: i32,
    pub y_sampling: i32,
}

impl Channel {
    /// The layer part of the name (`"diffuse"` for `"diffuse.R"`, `""` for `"R"`).
    pub fn layer(&self) -> &str {
        self.name.rsplit_once('.').map_or("", |(l, _)| l)
    }
    /// The name without its layer (`"R"` for `"diffuse.R"`).
    pub fn base_name(&self) -> &str {
        self.name.rsplit_once('.').map_or(&self.name, |(_, b)| b)
    }
}

/// Compression method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Rle,
    Zips,
    Zip,
    Piz,
    Pxr24,
    B44,
    B44a,
    Dwaa,
    Dwab,
    Htj2k256,
    Htj2k32,
    /// OpenEXR 3.5's lossy LJ2K (id 12).
    Lj2k,
    /// OpenEXR 3.5's ZSTD (id 13).
    Zstd,
    Unknown(u8),
}

impl Compression {
    fn from_u8(v: u8) -> Self {
        use Compression::*;
        match v {
            0 => None,
            1 => Rle,
            2 => Zips,
            3 => Zip,
            4 => Piz,
            5 => Pxr24,
            6 => B44,
            7 => B44a,
            8 => Dwaa,
            9 => Dwab,
            10 => Htj2k256,
            11 => Htj2k32,
            12 => Lj2k,
            13 => Zstd,
            v => Unknown(v),
        }
    }

    /// Scanlines per chunk in scanline files (`None` for unknown methods).
    pub fn lines_per_chunk(self) -> Option<u32> {
        use Compression::*;
        Some(match self {
            None | Rle | Zips | Zstd => 1,
            Zip | Pxr24 => 16,
            Piz | B44 | B44a | Dwaa | Htj2k32 => 32,
            Dwab | Htj2k256 | Lj2k => 256,
            Unknown(_) => return Option::None,
        })
    }

    /// Whether this crate can decode it.
    pub fn is_supported(self) -> bool {
        use Compression::*;
        matches!(
            self,
            None | Rle
                | Zips
                | Zip
                | Piz
                | Pxr24
                | B44
                | B44a
                | Dwaa
                | Dwab
                | Htj2k256
                | Htj2k32
                | Lj2k
                | Zstd
        )
    }

    pub fn name(self) -> &'static str {
        use Compression::*;
        match self {
            None => "none",
            Rle => "RLE",
            Zips => "ZIPS",
            Zip => "ZIP",
            Piz => "PIZ",
            Pxr24 => "PXR24",
            B44 => "B44",
            B44a => "B44A",
            Dwaa => "DWAA",
            Dwab => "DWAB",
            Htj2k256 => "HTJ2K256",
            Htj2k32 => "HTJ2K32",
            Lj2k => "LJ2K",
            Zstd => "ZSTD",
            Unknown(_) => "unknown",
        }
    }
}

/// Inclusive integer rectangle (`box2i`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Box2i {
    pub x_min: i32,
    pub y_min: i32,
    pub x_max: i32,
    pub y_max: i32,
}

impl Box2i {
    /// Width (0 if the box is empty or inverted).
    pub fn width(&self) -> u64 {
        (self.x_max as i64 - self.x_min as i64 + 1).max(0) as u64
    }
    pub fn height(&self) -> u64 {
        (self.y_max as i64 - self.y_min as i64 + 1).max(0) as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOrder {
    IncreasingY,
    DecreasingY,
    RandomY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelMode {
    One,
    Mipmap,
    Ripmap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelRounding {
    Down,
    Up,
}

/// The `tiles` attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileDesc {
    pub x_size: u32,
    pub y_size: u32,
    pub mode: LevelMode,
    pub rounding: LevelRounding,
}

/// Part storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Scanline,
    Tiled(TileDesc),
    /// Deep data (decoded flattened).
    DeepScanline,
    DeepTiled(Option<TileDesc>),
}

/// The `chromaticities` attribute (CIE xy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chromaticities {
    pub red: [f32; 2],
    pub green: [f32; 2],
    pub blue: [f32; 2],
    pub white: [f32; 2],
}

impl Chromaticities {
    /// Rec. 709 / sRGB primaries with D65, OpenEXR's default.
    pub const REC709: Chromaticities = Chromaticities {
        red: [0.64, 0.33],
        green: [0.30, 0.60],
        blue: [0.15, 0.06],
        white: [0.3127, 0.3290],
    };
}

/// A resolution level of a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// Level index along x (equal to `ly` for mipmaps).
    pub lx: u32,
    pub ly: u32,
    pub width: u32,
    pub height: u32,
}

/// A raw attribute: name, type name and the byte range of its value in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub type_name: String,
    pub value: Range<usize>,
}

/// Header of one part.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// `name` attribute (multi-part files).
    pub name: Option<String>,
    pub storage: Storage,
    /// In file order (alphabetical by name in valid files).
    pub channels: Vec<Channel>,
    pub compression: Compression,
    pub data_window: Box2i,
    pub display_window: Box2i,
    pub line_order: LineOrder,
    pub pixel_aspect_ratio: f32,
    pub screen_window_center: [f32; 2],
    pub screen_window_width: f32,
    pub chromaticities: Option<Chromaticities>,
    pub white_luminance: Option<f32>,
    pub adopted_neutral: Option<[f32; 2]>,
    /// Resolution levels. Index 0 is full resolution; mipmaps are listed by level, ripmaps
    /// with `lx` varying fastest (the offset-table order).
    pub levels: Vec<Level>,
    /// Number of chunks (offset-table entries).
    pub chunk_count: u64,
    /// Every attribute, including the ones parsed above.
    pub attributes: Vec<Attribute>,
    /// File offset of this part's offset table.
    pub(crate) offset_table: usize,
    /// Number of x / y levels (1 for single-level parts).
    pub(crate) num_x_levels: u32,
    pub(crate) num_y_levels: u32,
}

impl Part {
    /// Width of the data window.
    pub fn width(&self) -> u64 {
        self.data_window.width()
    }
    pub fn height(&self) -> u64 {
        self.data_window.height()
    }
    pub fn is_deep(&self) -> bool {
        matches!(self.storage, Storage::DeepScanline | Storage::DeepTiled(_))
    }
    pub fn tile_desc(&self) -> Option<TileDesc> {
        match self.storage {
            Storage::Tiled(t) | Storage::DeepTiled(Some(t)) => Some(t),
            _ => None,
        }
    }
    /// The smallest level that is at least `min_width` x `min_height` (or the full level if none
    /// is smaller). For ripmaps x and y are chosen independently. This is the cheap preview path.
    pub fn level_for_size(&self, min_width: u32, min_height: u32) -> usize {
        let mut best = 0usize;
        for (i, l) in self.levels.iter().enumerate() {
            let b = &self.levels[best];
            if l.width >= min_width.min(self.levels[0].width)
                && l.height >= min_height.min(self.levels[0].height)
                && (l.width as u64 * l.height as u64) < (b.width as u64 * b.height as u64)
            {
                best = i;
            }
        }
        best
    }
    /// Index into [`Part::levels`] of level `(lx, ly)`.
    pub fn level_index(&self, lx: u32, ly: u32) -> Option<usize> {
        self.levels.iter().position(|l| l.lx == lx && l.ly == ly)
    }
    /// Names of the layers (channel-name prefixes), `""` for the root layer, sorted.
    pub fn layers(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.channels.iter().map(|c| c.layer()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
    /// The value bytes of attribute `name`.
    pub fn attribute<'a>(&self, data: &'a [u8], name: &str) -> Option<&'a [u8]> {
        let a = self.attributes.iter().find(|a| a.name == name)?;
        data.get(a.value.clone())
    }
    /// The tile grid of level `level` (tiles across, tiles down); `(1, n_chunks)` for scanlines.
    pub(crate) fn tiles_in_level(&self, level: &Level) -> (u64, u64) {
        match self.tile_desc() {
            Some(t) => (
                (level.width as u64).div_ceil(t.x_size as u64),
                (level.height as u64).div_ceil(t.y_size as u64),
            ),
            None => {
                let lpb = self.compression.lines_per_chunk().unwrap_or(1) as u64;
                (1, (level.height as u64).div_ceil(lpb))
            }
        }
    }
}

/// File-level information.
#[derive(Clone, Debug, PartialEq)]
pub struct ExrInfo {
    /// Format version (2).
    pub version: u8,
    pub multipart: bool,
    pub long_names: bool,
    pub parts: Vec<Part>,
}

fn floor_log2(mut x: u64) -> u32 {
    let mut y = 0;
    while x > 1 {
        y += 1;
        x >>= 1;
    }
    y
}

fn ceil_log2(x: u64) -> u32 {
    let f = floor_log2(x);
    if x > (1u64 << f) { f + 1 } else { f }
}

fn round_log2(x: u64, r: LevelRounding) -> u32 {
    match r {
        LevelRounding::Down => floor_log2(x),
        LevelRounding::Up => ceil_log2(x),
    }
}

fn level_size(full: u64, l: u32, r: LevelRounding) -> u64 {
    let b = 1u64 << l.min(63);
    let mut s = full / b;
    if r == LevelRounding::Up && s * b < full {
        s += 1;
    }
    s.max(1)
}

#[derive(Default)]
struct RawHeader {
    name: Option<String>,
    type_name: Option<String>,
    channels: Option<Vec<Channel>>,
    compression: Option<u8>,
    data_window: Option<Box2i>,
    display_window: Option<Box2i>,
    line_order: Option<u8>,
    pixel_aspect: Option<f32>,
    swc: Option<[f32; 2]>,
    sww: Option<f32>,
    tiles: Option<TileDesc>,
    chroma: Option<Chromaticities>,
    white_lum: Option<f32>,
    adopted: Option<[f32; 2]>,
    chunk_count: Option<i32>,
    attrs: Vec<Attribute>,
}

fn read_box(r: &mut Reader) -> Result<Box2i> {
    Ok(Box2i {
        x_min: r.i32()?,
        y_min: r.i32()?,
        x_max: r.i32()?,
        y_max: r.i32()?,
    })
}

fn parse_channels(v: &[u8], name_max: usize) -> Result<Vec<Channel>> {
    let mut r = Reader::new(v, 0);
    let mut out = Vec::new();
    loop {
        let name = r.cstr(name_max)?;
        if name.is_empty() {
            break;
        }
        let ty = match r.i32()? {
            0 => PixelType::Uint,
            1 => PixelType::Half,
            2 => PixelType::Float,
            _ => return Err(ExrError::InvalidHeader("channel pixel type")),
        };
        let p_linear = r.u8()? != 0;
        r.bytes(3)?;
        let xs = r.i32()?;
        let ys = r.i32()?;
        if !(1..=1 << 16).contains(&xs) || !(1..=1 << 16).contains(&ys) {
            return Err(ExrError::InvalidHeader("channel sampling"));
        }
        if out.len() >= MAX_CHANNELS {
            return Err(ExrError::InvalidHeader("too many channels"));
        }
        out.push(Channel {
            name: String::from_utf8_lossy(name).into_owned(),
            pixel_type: ty,
            p_linear,
            x_sampling: xs,
            y_sampling: ys,
        });
    }
    Ok(out)
}

fn parse_header(r: &mut Reader, name_max: usize) -> Result<Option<RawHeader>> {
    let mut h = RawHeader::default();
    let mut first = true;
    loop {
        let name = r.cstr(name_max)?;
        if name.is_empty() {
            // End of header (or, for the first read in a multi-part file, end of headers).
            return Ok(if first { None } else { Some(h) });
        }
        first = false;
        let ty = r.cstr(name_max)?;
        let size = r.i32()?;
        if size < 0 {
            return Err(ExrError::InvalidHeader("negative attribute size"));
        }
        let start = r.pos;
        let v = r.bytes(size as usize)?;
        let mut vr = Reader::new(v, 0);
        let name_s = String::from_utf8_lossy(name).into_owned();
        // Known attributes are only interpreted if their type matches; otherwise kept raw.
        match (name, ty) {
            (b"channels", b"chlist") => h.channels = Some(parse_channels(v, name_max)?),
            (b"compression", b"compression") => h.compression = Some(vr.u8()?),
            (b"dataWindow", b"box2i") => h.data_window = Some(read_box(&mut vr)?),
            (b"displayWindow", b"box2i") => h.display_window = Some(read_box(&mut vr)?),
            (b"lineOrder", b"lineOrder") => h.line_order = Some(vr.u8()?),
            (b"pixelAspectRatio", b"float") => h.pixel_aspect = Some(vr.f32()?),
            (b"screenWindowCenter", b"v2f") => h.swc = Some([vr.f32()?, vr.f32()?]),
            (b"screenWindowWidth", b"float") => h.sww = Some(vr.f32()?),
            (b"tiles", b"tiledesc") => {
                let xs = vr.u32()?;
                let ys = vr.u32()?;
                let m = vr.u8()?;
                let mode = match m & 0xf {
                    0 => LevelMode::One,
                    1 => LevelMode::Mipmap,
                    2 => LevelMode::Ripmap,
                    _ => return Err(ExrError::InvalidHeader("tile level mode")),
                };
                let rounding = match m >> 4 {
                    0 => LevelRounding::Down,
                    1 => LevelRounding::Up,
                    _ => return Err(ExrError::InvalidHeader("tile rounding mode")),
                };
                h.tiles = Some(TileDesc {
                    x_size: xs,
                    y_size: ys,
                    mode,
                    rounding,
                });
            }
            (b"chromaticities", b"chromaticities") => {
                let mut f = [0f32; 8];
                for x in &mut f {
                    *x = vr.f32()?;
                }
                h.chroma = Some(Chromaticities {
                    red: [f[0], f[1]],
                    green: [f[2], f[3]],
                    blue: [f[4], f[5]],
                    white: [f[6], f[7]],
                });
            }
            (b"whiteLuminance", b"float") => h.white_lum = Some(vr.f32()?),
            (b"adoptedNeutral", b"v2f") => h.adopted = Some([vr.f32()?, vr.f32()?]),
            (b"name", b"string") => h.name = Some(String::from_utf8_lossy(v).into_owned()),
            (b"type", b"string") => h.type_name = Some(String::from_utf8_lossy(v).into_owned()),
            (b"chunkCount", b"int") => h.chunk_count = Some(vr.i32()?),
            _ => {}
        }
        h.attrs.push(Attribute {
            name: name_s,
            type_name: String::from_utf8_lossy(ty).into_owned(),
            value: start..start + size as usize,
        });
    }
}

/// Does `data` start with the OpenEXR magic number?
pub fn probe(data: &[u8]) -> bool {
    data.len() >= 8 && data[..4] == MAGIC
}

pub(crate) fn read_info(data: &[u8]) -> Result<ExrInfo> {
    if !probe(data) {
        return Err(ExrError::NotExr);
    }
    let mut r = Reader::new(data, 4);
    let ver = r.u32()?;
    let version = (ver & 0xff) as u8;
    if version != 2 {
        return Err(ExrError::Unsupported("file format version"));
    }
    let tiled_flag = ver & FLAG_TILED != 0;
    let long_names = ver & FLAG_LONG_NAMES != 0;
    let deep_flag = ver & FLAG_DEEP != 0;
    let multipart = ver & FLAG_MULTIPART != 0;
    if ver & !(0xff | FLAG_TILED | FLAG_LONG_NAMES | FLAG_DEEP | FLAG_MULTIPART) != 0 {
        return Err(ExrError::Unsupported("unknown version flags"));
    }
    let name_max = if long_names { 255 } else { 31 };
    let mut raws = Vec::new();
    if multipart {
        while let Some(h) = parse_header(&mut r, name_max)? {
            if raws.len() >= MAX_PARTS {
                return Err(ExrError::InvalidHeader("too many parts"));
            }
            raws.push(h);
        }
        if raws.is_empty() {
            return Err(ExrError::InvalidHeader("no parts"));
        }
    } else {
        raws.push(parse_header(&mut r, name_max)?.ok_or(ExrError::InvalidHeader("empty header"))?);
    }
    let mut table = r.pos;
    let mut parts = Vec::with_capacity(raws.len());
    for h in raws {
        let storage = match h.type_name.as_deref() {
            Some("scanlineimage") => Storage::Scanline,
            Some("tiledimage") => Storage::Tiled(h.tiles.ok_or(ExrError::InvalidHeader("tiles"))?),
            Some("deepscanline") => Storage::DeepScanline,
            Some("deeptile") => Storage::DeepTiled(h.tiles),
            Some(_) => return Err(ExrError::Unsupported("part type")),
            None if multipart => return Err(ExrError::InvalidHeader("missing part type")),
            None if deep_flag => Storage::DeepScanline,
            None if tiled_flag => Storage::Tiled(h.tiles.ok_or(ExrError::InvalidHeader("tiles"))?),
            None => Storage::Scanline,
        };
        let channels = h
            .channels
            .ok_or(ExrError::InvalidHeader("missing channels"))?;
        let compression = Compression::from_u8(
            h.compression
                .ok_or(ExrError::InvalidHeader("missing compression"))?,
        );
        let data_window = h
            .data_window
            .ok_or(ExrError::InvalidHeader("missing dataWindow"))?;
        let display_window = h.display_window.unwrap_or(data_window);
        if data_window.width() == 0 || data_window.height() == 0 {
            return Err(ExrError::InvalidHeader("empty dataWindow"));
        }
        let line_order = match h.line_order.unwrap_or(0) {
            0 => LineOrder::IncreasingY,
            1 => LineOrder::DecreasingY,
            2 => LineOrder::RandomY,
            _ => return Err(ExrError::InvalidHeader("lineOrder")),
        };
        let (w, hgt) = (data_window.width(), data_window.height());
        if w > u32::MAX as u64 || hgt > u32::MAX as u64 {
            return Err(ExrError::InvalidHeader("dataWindow too large"));
        }
        let mut levels = Vec::new();
        let (mut nxl, mut nyl) = (1u32, 1u32);
        match storage {
            Storage::Tiled(t) | Storage::DeepTiled(Some(t)) => {
                if t.x_size == 0 || t.y_size == 0 || t.x_size > 1 << 24 || t.y_size > 1 << 24 {
                    return Err(ExrError::InvalidHeader("tile size"));
                }
                match t.mode {
                    LevelMode::One => levels.push(Level {
                        lx: 0,
                        ly: 0,
                        width: w as u32,
                        height: hgt as u32,
                    }),
                    LevelMode::Mipmap => {
                        let n = round_log2(w.max(hgt), t.rounding) + 1;
                        nxl = n;
                        nyl = n;
                        for l in 0..n {
                            levels.push(Level {
                                lx: l,
                                ly: l,
                                width: level_size(w, l, t.rounding) as u32,
                                height: level_size(hgt, l, t.rounding) as u32,
                            });
                        }
                    }
                    LevelMode::Ripmap => {
                        nxl = round_log2(w, t.rounding) + 1;
                        nyl = round_log2(hgt, t.rounding) + 1;
                        for ly in 0..nyl {
                            for lx in 0..nxl {
                                levels.push(Level {
                                    lx,
                                    ly,
                                    width: level_size(w, lx, t.rounding) as u32,
                                    height: level_size(hgt, ly, t.rounding) as u32,
                                });
                            }
                        }
                    }
                }
            }
            _ => levels.push(Level {
                lx: 0,
                ly: 0,
                width: w.min(u32::MAX as u64) as u32,
                height: hgt.min(u32::MAX as u64) as u32,
            }),
        }
        let mut part = Part {
            name: h.name,
            storage,
            channels,
            compression,
            data_window,
            display_window,
            line_order,
            pixel_aspect_ratio: h.pixel_aspect.unwrap_or(1.0),
            screen_window_center: h.swc.unwrap_or([0.0; 2]),
            screen_window_width: h.sww.unwrap_or(1.0),
            chromaticities: h.chroma,
            white_luminance: h.white_lum,
            adopted_neutral: h.adopted,
            levels,
            chunk_count: 0,
            attributes: h.attrs,
            offset_table: 0,
            num_x_levels: nxl,
            num_y_levels: nyl,
        };
        let computed: u64 = match part.storage {
            Storage::Scanline
            | Storage::Tiled(_)
            | Storage::DeepScanline
            | Storage::DeepTiled(Some(_)) => {
                if part.tile_desc().is_none() && compression.lines_per_chunk().is_none() {
                    0
                } else {
                    // Each level has under 2^64 tiles, but their sum can overflow (1x1 tiles
                    // over a 2^32-wide mipmap): saturate, the offset table is read with checks.
                    part.levels
                        .iter()
                        .map(|l| {
                            let (a, b) = part.tiles_in_level(l);
                            a * b
                        })
                        .fold(0u64, u64::saturating_add)
                }
            }
            _ => 0,
        };
        part.chunk_count = match h.chunk_count {
            Some(c) if c >= 0 => c as u64,
            Some(_) => return Err(ExrError::InvalidHeader("chunkCount")),
            None if multipart => return Err(ExrError::InvalidHeader("missing chunkCount")),
            None if computed == 0 => {
                return Err(ExrError::Unsupported("cannot size the offset table"));
            }
            None => computed,
        };
        part.offset_table = table;
        // The table may be cut short by truncation; decode treats missing entries as missing chunks.
        table = table
            .saturating_add(part.chunk_count.saturating_mul(8).min(usize::MAX as u64) as usize);
        parts.push(part);
    }
    Ok(ExrInfo {
        version,
        multipart,
        long_names,
        parts,
    })
}

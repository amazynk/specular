//! Decoders for 8-bit home computer screen dumps and Kodak Photo CD.
//!
//! | Module | Format | Coverage |
//! |---|---|---|
//! | [`cpc`] | Amstrad CPC screen (`.scr`) | 16 KiB screen memory, with or without an AMSDOS header; modes 0, 1 and 2; firmware palette or an OCP Art Studio `.pal` |
//! | [`msx`] | MSX BSAVE screens (`.sc2`, `.sc5`–`.sc8`, `.sca`, `.scc`) | screen 2 patterns, screens 5–8 bitmaps with their saved palettes, screens 10 and 12 (YJK) |
//! | [`apple2`] | Apple II hi-res (`.hgr`) and double hi-res (`.dhr`, `.a2fc`) | NTSC artefact colour |
//! | [`bbc`] | BBC Micro screen dumps (`.bb0`–`.bb2`, `.bb4`, `.bb5`) | modes 0, 1, 2, 4 and 5 with the default palettes |
//! | [`atari8`] | Atari 8-bit Micro Painter (`.mic`), Graphics 8 (`.gr8`) and 9 (`.gr9`) | the colour registers saved with the picture, else the system's defaults |
//! | [`photocd`] | Kodak Photo CD (`.pcd`) | Base/16, Base/4 and Base (768×512) image packs; Base×4 and up are Huffman coded and not read |
//!
//! Screen dumps have no signature: their size, the extension (which names the screen
//! mode) and, where present, a loader header (AMSDOS, MSX BSAVE) identify them. Every
//! decoder outputs straight RGBA, top row first, with pixels repeated so the picture keeps
//! the machine's pixel aspect.
//!
//! Robustness: no input panics; allocation follows only fixed screen sizes.

pub mod apple2;
pub mod atari8;
pub mod bbc;
pub mod cpc;
pub mod msx;
pub mod photocd;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not this format.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    Truncated(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRecognised => write!(f, "format not recognised"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// The formats of this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    CpcScreen,
    MsxScreen,
    AppleHgr,
    AppleDhr,
    BbcScreen,
    AtariMic,
    AtariGr8,
    AtariGr9,
    PhotoCd,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::CpcScreen,
        Kind::MsxScreen,
        Kind::AppleHgr,
        Kind::AppleDhr,
        Kind::BbcScreen,
        Kind::AtariMic,
        Kind::AtariGr8,
        Kind::AtariGr9,
        Kind::PhotoCd,
    ];

    /// Lower-case extensions.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Kind::CpcScreen => &["scr"],
            Kind::MsxScreen => msx::EXTENSIONS,
            Kind::AppleHgr => &["hgr"],
            Kind::AppleDhr => &["dhr", "a2fc"],
            Kind::BbcScreen => &["bb0", "bb1", "bb2", "bb4", "bb5"],
            Kind::AtariMic => &["mic"],
            Kind::AtariGr8 => &["gr8"],
            Kind::AtariGr9 => &["gr9"],
            Kind::PhotoCd => &["pcd"],
        }
    }
}

/// What the file's surroundings say: its extension (lower case, without the dot; it names
/// the screen mode of MSX and BBC dumps) and a palette file beside it (CPC `.pal`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Hints<'a> {
    pub ext: &'a str,
    pub palette: Option<&'a [u8]>,
}

/// What [`info`] reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// Ways to show the same data (CPC screen modes); 1 elsewhere.
    pub pages: u32,
    /// Bits per stored pixel.
    pub bit_depth: u8,
    /// Indexed colours (`true`) or direct colour.
    pub palette: bool,
    /// EXIF orientation (Photo CD scans can be portrait).
    pub orientation: u8,
    /// One line on the coding, e.g. "mode 1, 4 colours".
    pub coding: String,
    /// Header and layout facts as (name, value), in display order.
    pub fields: Vec<(String, String)>,
}

/// Straight RGBA pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Pixel data ended early; the missing part is black.
    pub truncated: bool,
}

/// Whether `data` carries a signature or loader header of `kind` (AMSDOS screen header,
/// MSX BSAVE header, Photo CD's `PCD_IPI`). The other dumps have none: they are known by
/// their extension and size only.
pub fn signature(kind: Kind, data: &[u8]) -> bool {
    match kind {
        Kind::CpcScreen => cpc::amsdos_screen(data),
        Kind::MsxScreen => msx::bsave(data).is_some(),
        Kind::PhotoCd => photocd::probe(data),
        _ => false,
    }
}

pub fn info(kind: Kind, data: &[u8], hints: &Hints, page: u32) -> Result<Info> {
    match kind {
        Kind::CpcScreen => cpc::info(data, hints, page),
        Kind::MsxScreen => msx::info(data, hints),
        Kind::AppleHgr => apple2::info(data, false),
        Kind::AppleDhr => apple2::info(data, true),
        Kind::BbcScreen => bbc::info(data, hints),
        Kind::AtariMic | Kind::AtariGr8 | Kind::AtariGr9 => atari8::info(kind, data),
        Kind::PhotoCd => photocd::info(data),
    }
}

pub fn decode(kind: Kind, data: &[u8], hints: &Hints, page: u32) -> Result<Image> {
    match kind {
        Kind::CpcScreen => cpc::decode(data, hints, page),
        Kind::MsxScreen => msx::decode(data, hints),
        Kind::AppleHgr => apple2::decode(data, false),
        Kind::AppleDhr => apple2::decode(data, true),
        Kind::BbcScreen => bbc::decode(data, hints),
        Kind::AtariMic | Kind::AtariGr8 | Kind::AtariGr9 => atari8::decode(kind, data),
        Kind::PhotoCd => photocd::decode(data, photocd::Resolution::Base),
    }
}

/// An RGBA canvas whose pixels are machine pixels drawn `sx`×`sy` output pixels large.
pub(crate) struct Canvas {
    pub w: u32,
    pub h: u32,
    sx: u32,
    sy: u32,
    pub rgba: Vec<u8>,
}

impl Canvas {
    /// `w`×`h` machine pixels, each drawn `sx`×`sy`; opaque black.
    pub fn new(w: u32, h: u32, sx: u32, sy: u32) -> Canvas {
        let (ow, oh) = (w * sx, h * sy);
        let mut rgba = vec![0u8; ow as usize * oh as usize * 4];
        for p in rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
        Canvas { w, h, sx, sy, rgba }
    }

    pub fn put(&mut self, x: u32, y: u32, c: [u8; 3]) {
        if x >= self.w || y >= self.h {
            return;
        }
        let ow = (self.w * self.sx) as usize;
        for dy in 0..self.sy {
            let row = (y * self.sy + dy) as usize * ow;
            for dx in 0..self.sx {
                let i = (row + (x * self.sx + dx) as usize) * 4;
                self.rgba[i..i + 3].copy_from_slice(&c);
            }
        }
    }

    pub fn into_image(self, truncated: bool) -> Image {
        Image {
            width: self.w * self.sx,
            height: self.h * self.sy,
            rgba: self.rgba,
            truncated,
        }
    }
}

/// Byte `i` of `d`, 0 past the end (cut-short dumps show black there).
pub(crate) fn at(d: &[u8], i: usize) -> u8 {
    d.get(i).copied().unwrap_or(0)
}

pub(crate) fn field(name: &str, value: impl ToString) -> (String, String) {
    (name.to_owned(), value.to_string())
}

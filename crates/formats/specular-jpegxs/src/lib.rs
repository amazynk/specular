//! JPEG XS (ISO/IEC 21122): the low-latency wavelet codec of broadcast and pro-AV links.
//!
//! Reads bare codestreams (`.jxs` codestream files, MP4 `jxsm` samples), the JPEG XS image
//! file of ISO/IEC 21122-3 (signature box, `jxs ` brand, header and codestream boxes) and HEIF
//! files whose primary item is a `jxs1` image item ([`heif`]; `image/jxsi`). Every
//! profile decodes: Light, Main, High (4:2:0, 4:2:2, 4:4:4, 4:4:4:4), the lossless MLS
//! profiles, the Bayer profiles with the Star-Tetrix colour transform (shown demosaicked) and
//! the TDC profiles (temporal prediction; [`Sequence`] keeps the frame buffer across frames);
//! 8 to 16 bits; the quadratic and extended non-linear transforms; horizontal-only and two-way
//! wavelet levels; raw, unary and vertically predicted bit-plane counts with significance
//! flags; dead-zone and uniform quantisers. Slices decode in parallel. Refused in words: more
//! than four components, components of different depths, deeper than 16 bits (no profile of
//! ISO/IEC 21122-2 allows more, and the reference software stops at 16), unknown colour or
//! non-linear transforms and colour filter layouts.
//!
//! intoPIX TicoRAW, the codestream of Nikon High Efficiency raw files, decodes up to the
//! inverse wavelet transform ([`decode_coefficients`]; `specular-raw` does the rest). It is
//! JPEG XS with these changes, worked out from the files (no specification is public): 13
//! more bytes in the picture header; a third component-table byte giving each component its
//! own wavelet depths (one component has none); components on a grid half the picture size
//! each way; long packet headers always; a precinct packed row by row and, within a row,
//! component by component (one packet each), with the weights table in that order; and
//! vertically predicted lines without a line above coded in plain unary.
//!
//! The arithmetic is exact integer arithmetic, so samples match the ISO reference software
//! bit for bit. Robustness: no input panics; a codestream that ends early or is damaged gives
//! the precincts decoded so far with [`Image::truncated`] set.

use std::borrow::Cow;

mod bits;
mod decode;
mod display;
mod dwt;
mod entropy;
pub mod file;
pub mod header;
pub mod heif;
mod layout;
mod transform;

pub use decode::{
    Coefficients, SamplePlane, Samples, Sequence, decode_coefficients, decode_samples,
};
pub use header::{Cfa, Header, Nlt};

/// Largest sample count (all components) accepted.
pub const MAX_SAMPLES: u64 = 1 << 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not JPEG XS.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement (plain-language reason).
    Unsupported(String),
    /// The header ends early.
    Truncated(String),
    Malformed(String),
    /// Over [`MAX_SAMPLES`].
    TooLarge,
    /// The memory budget cannot hold the decode.
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRecognised => write!(f, "not a JPEG XS file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Grey,
    Rgb,
    Palette,
    YCbCr,
    Bilevel,
}

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub pages: u32,
    pub bit_depth: u8,
    pub model: Model,
    pub has_alpha: bool,
    /// Always 1 (JPEG XS has no orientation).
    pub orientation: u8,
    pub coding: String,
    /// Key facts by stable key: "profile", "level", "decomposition", "colour-transform",
    /// "compression", "comment".
    pub summary: Vec<(&'static str, String)>,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    Gray8(Vec<u8>),
    Rgba8(Vec<u8>),
    Gray16(Vec<u16>),
    Rgba16(Vec<u16>),
    RgbaF32(Vec<f32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
    pub truncated: bool,
}

/// Whether `head` starts a JPEG XS codestream (SOC then CAP) or a JPEG XS file, or is a HEIF
/// file whose primary item is a JPEG XS image (its `meta` box must lie within `head`).
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&[0xFF, 0x10, 0xFF, 0x50]) || file::is_file(head) || heif::is_heif(head)
}

/// The codestream of `data` (itself, the one a JPEG XS file carries, or a HEIF file's
/// primary `jxs1` item) and the file's facts.
pub fn codestream(data: &[u8]) -> Result<(Cow<'_, [u8]>, Option<file::FileInfo>)> {
    container(data).map(|(cs, f, _)| (cs, f))
}

/// A codestream, the container's facts and the crop a HEIF `clap` asks for.
type Contained<'a> = (Cow<'a, [u8]>, Option<file::FileInfo>, Option<heif::Crop>);

/// [`codestream`] plus the crop a HEIF `clap` asks for.
fn container(data: &[u8]) -> Result<Contained<'_>> {
    if file::is_file(data) {
        let f = file::parse(data)?;
        let (a, b) = f.codestream;
        Ok((Cow::Borrowed(&data[a..b]), Some(f), None))
    } else if data.starts_with(&[0xFF, 0x10]) {
        Ok((Cow::Borrowed(data), None, None))
    } else if data.get(4..8) == Some(b"ftyp") {
        let (cs, f, crop) = heif::parse(data)?;
        Ok((cs, Some(f), crop))
    } else {
        Err(Error::NotRecognised)
    }
}

/// The crop when it lies inside a `w`×`h` picture.
fn fitting(crop: Option<heif::Crop>, w: u32, h: u32) -> Option<heif::Crop> {
    crop.filter(|c| {
        c.x.checked_add(c.w).is_some_and(|r| r <= w) && c.y.checked_add(c.h).is_some_and(|b| b <= h)
    })
}

fn crop_pixels(p: Pixels, w: usize, c: heif::Crop) -> Pixels {
    fn cut<T: Copy>(v: &[T], w: usize, ch: usize, c: heif::Crop) -> Vec<T> {
        let (x, y, cw, chh) = (c.x as usize, c.y as usize, c.w as usize, c.h as usize);
        let mut out = Vec::with_capacity(cw * chh * ch);
        for row in v.chunks_exact(w * ch).skip(y).take(chh) {
            out.extend_from_slice(&row[x * ch..(x + cw) * ch]);
        }
        out
    }
    match p {
        Pixels::Gray8(v) => Pixels::Gray8(cut(&v, w, 1, c)),
        Pixels::Rgba8(v) => Pixels::Rgba8(cut(&v, w, 4, c)),
        Pixels::Gray16(v) => Pixels::Gray16(cut(&v, w, 1, c)),
        Pixels::Rgba16(v) => Pixels::Rgba16(cut(&v, w, 4, c)),
        Pixels::RgbaF32(v) => Pixels::RgbaF32(cut(&v, w, 4, c)),
    }
}

fn sampling(h: &Header) -> &'static str {
    let s: Vec<(u8, u8)> = h.components.iter().map(|c| (c.sx, c.sy)).collect();
    match s.as_slice() {
        [_] => "",
        [(1, 1), (2, 1), (2, 1)] => "4:2:2",
        [(1, 1), (2, 1), (2, 1), (1, 1)] => "4:2:2:4",
        [(1, 1), (2, 2), (2, 2)] => "4:2:0",
        [(1, 1), (2, 2), (2, 2), (1, 1)] => "4:2:0:4",
        s if s.iter().all(|&c| c == (1, 1)) => match s.len() {
            3 => "4:4:4",
            4 => "4:4:4:4",
            _ => "",
        },
        _ => "subsampled",
    }
}

fn colour_transform(h: &Header) -> String {
    match h.cpih {
        0 => "None".into(),
        1 => "Reversible colour transform (RCT)".into(),
        3 => format!(
            "Star-Tetrix{}, {} mosaic",
            if h.tetrix.is_some_and(|t| t.inline) {
                " (in-line)"
            } else {
                ""
            },
            h.cfa.map(|c| c.name()).unwrap_or("?")
        ),
        t => format!("type {t}"),
    }
}

fn nlt_name(n: Nlt) -> String {
    match n {
        Nlt::Linear => "None (linear)".into(),
        Nlt::Quadratic { sigma, alpha } => format!(
            "Quadratic, DC offset {}",
            alpha as i32 - if sigma { 32768 } else { 0 }
        ),
        Nlt::Extended { t1, t2, e } => format!("Extended, thresholds {t1} and {t2}, slope 2^{e}"),
    }
}

/// Display size and colour model of a header.
fn shape(h: &Header, f: Option<&file::FileInfo>) -> (u32, u32, display::Kind) {
    let sub = h.components.iter().any(|c| c.sx != 1 || c.sy != 1);
    let k = display::kind(
        h.components.len(),
        sub,
        h.cpih == 1,
        if h.cpih == 3 { h.cfa } else { None },
        f.and_then(|f| f.cicp),
    );
    match k {
        display::Kind::Mosaic(_) => (2 * h.width, 2 * h.height, k),
        _ => (h.width, h.height, k),
    }
}

pub fn info(data: &[u8], page: u32) -> Result<Info> {
    if page != 0 {
        return Err(Error::Malformed("JPEG XS images have one page".into()));
    }
    let (cs, f, crop) = container(data)?;
    let cs = &*cs;
    let h = header::parse(cs)?;
    decode::check(&h)?;
    let (w, ht, k) = shape(&h, f.as_ref());
    let crop = fitting(crop, w, ht);
    let depth = h.depth();
    let profile = header::profile_name(h.profile)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{:#06x}", h.profile));
    let (level, sublevel) = header::level_name(h.plev);
    let samp = sampling(&h);
    let bpp = cs.len() as f64 * 8.0 / (w as f64 * ht as f64);
    let lossless = h.profile & 0xFF00 == 0x6E00
        || (h.fq == 0 && h.qpih == 0 && h.cap & header::cap::LOSSLESS != 0);
    let mut summary: Vec<(&'static str, String)> = vec![
        ("profile", profile.clone()),
        ("level", format!("{level}, sublevel {sublevel}")),
        (
            "decomposition",
            format!("{} horizontal, {} vertical", h.nlx, h.nly),
        ),
        ("colour-transform", colour_transform(&h)),
        (
            "compression",
            if lossless {
                format!("JPEG XS lossless, {bpp:.2} bits per pixel")
            } else {
                format!("JPEG XS, {bpp:.2} bits per pixel")
            },
        ),
    ];
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut add = |k: &str, v: String| fields.push((k.to_owned(), v));
    add("Profile", profile.clone());
    add("Level", level);
    add("Sublevel", sublevel);
    add("Width", h.width.to_string());
    add("Height", h.height.to_string());
    add("Components", h.components.len().to_string());
    if !samp.is_empty() {
        add("Sampling", samp.to_owned());
    }
    add("Bit depth", depth.to_string());
    add("Coefficient bits (Bw)", h.bw.to_string());
    add("Fractional bits (Fq)", h.fq.to_string());
    add(
        "Wavelet levels",
        format!("{} horizontal, {} vertical", h.nlx, h.nly),
    );
    if h.sd > 0 {
        add("Components without wavelet", h.sd.to_string());
    }
    add("Colour transform", colour_transform(&h));
    add("Non-linear transform", nlt_name(h.nlt));
    add(
        "Precinct width",
        if h.cw == 0 {
            "full width".into()
        } else {
            format!("{} × 8 × 2^{} samples", h.cw, h.nlx)
        },
    );
    add("Slice height", format!("{} lines", h.slice_height));
    if h.slice_height > 0 {
        add("Slices", h.height.div_ceil(h.slice_height).to_string());
    }
    add("Code group size", h.ng.to_string());
    add("Significance group size", h.ss.to_string());
    add(
        "Quantiser",
        if h.qpih == 1 { "uniform" } else { "dead zone" }.into(),
    );
    add(
        "Signs",
        if h.fs == 1 {
            "separate sub-packet"
        } else {
            "with the coefficients"
        }
        .into(),
    );
    add(
        "Run mode",
        if h.rm == 1 {
            "zero coefficients"
        } else {
            "zero prediction residuals"
        }
        .into(),
    );
    add(
        "Raw bit-plane counts per packet",
        if h.rl { "allowed" } else { "not used" }.into(),
    );
    add(
        "Long precinct headers",
        if h.long_headers() { "yes" } else { "no" }.into(),
    );
    add("Codestream bytes", cs.len().to_string());
    if let Some(fi) = &f {
        add(
            "File brand",
            String::from_utf8_lossy(&fi.brand).trim().to_owned(),
        );
        if heif::is_heif(data) {
            add("Container", "HEIF, jxs1 image item".into());
        }
        if let Some(c) = crop {
            add(
                "Clean aperture",
                format!("{}×{} at {}, {}", c.w, c.h, c.x, c.y),
            );
        }
        if let Some(c) = fi.cicp {
            add(
                "Colour (CICP)",
                format!(
                    "primaries {}, transfer {}, matrix {}, {} range",
                    c.primaries,
                    c.transfer,
                    c.matrix,
                    if c.full_range { "full" } else { "narrow" }
                ),
            );
        }
        if let Some((_, frat)) = fi.video
            && let Some(r) = file::frame_rate(frat)
        {
            add("Frame rate", format!("{r:.3}"));
        }
    }
    for (t, text) in &h.comments {
        let s = String::from_utf8_lossy(text)
            .trim_matches('\0')
            .trim()
            .to_owned();
        if !s.is_empty() {
            if *t == 0 {
                add("Encoder", s.clone());
            } else {
                add("Comment", s.clone());
            }
            summary.push(("comment", s));
        }
    }
    let (model, alpha) = match k {
        display::Kind::Grey => (Model::Grey, false),
        display::Kind::GreyAlpha => (Model::Grey, true),
        display::Kind::Rgb { alpha } => (Model::Rgb, alpha),
        display::Kind::YCbCr { alpha, .. } => (Model::YCbCr, alpha),
        display::Kind::Mosaic(_) => (Model::Rgb, false),
    };
    let what = match k {
        display::Kind::Mosaic(c) => format!("{depth}-bit {} Bayer", c.name()),
        _ if samp.is_empty() => format!("{depth}-bit"),
        _ => format!("{depth}-bit {samp}"),
    };
    let (w, ht) = crop.map_or((w, ht), |c| (c.w, c.h));
    Ok(Info {
        width: w,
        height: ht,
        pages: 1,
        bit_depth: depth,
        model,
        has_alpha: alpha,
        orientation: f.as_ref().map_or(1, |f| f.orientation.max(1)),
        coding: format!("JPEG XS {profile}, {what}, {bpp:.2} bits per pixel"),
        summary,
        fields,
    })
}

pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    if page != 0 {
        return Err(Error::Malformed("JPEG XS images have one page".into()));
    }
    let (cs, f, crop) = container(data)?;
    let cs = &*cs;
    let h = header::parse(cs)?;
    let (w, ht, k) = shape(&h, f.as_ref());
    let s = decode_samples(cs)?;
    let sub: Vec<(usize, usize)> = h
        .components
        .iter()
        .map(|c| (c.sx as usize, c.sy as usize))
        .collect();
    let mut pixels = display::pixels(&s, k, &sub, w as usize, ht as usize);
    let (mut w, mut ht) = (w, ht);
    if let Some(c) = fitting(crop, w, ht) {
        pixels = crop_pixels(pixels, w as usize, c);
        (w, ht) = (c.w, c.h);
    }
    Ok(Image {
        width: w,
        height: ht,
        pixels,
        truncated: s.truncated,
    })
}

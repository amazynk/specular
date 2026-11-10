//! JBIG1 (ITU-T T.82 | ISO/IEC 11544) bi-level image entities: `.jbg`, `.jbig`, `.bie`.
//!
//! Written from the Recommendation. Covered: every stripe ordering (HITOLO, SEQ, ILEAVE,
//! SMID), any number of resolution layers (decoded up to the highest, D), the two- and
//! three-line lowest-layer templates, differential layers, typical prediction in both kinds
//! of layer, deterministic prediction with the default or a private table, adaptive-template
//! moves, SDRST resets, NEWLEN (also just before an SDE's end), comments, ABORT, and up to 16
//! bit planes.
//!
//! Refused, with a reason: a BIE that only continues another (DL > 0) and one that reuses a
//! prediction table from an earlier BIE (DPLAST), both meaningless on their own.
//!
//! Output: bi-level images as grey, black (JBIG 1) as 0 and white as 255. Several bit planes
//! are a grey level, most significant plane first, Gray-coded as JBIG-KIT writes them (the
//! Recommendation leaves the mapping open), 0 black.
//!
//! Robustness: no input panics. Sizes are capped ([`MAX_DIMENSION`], [`MAX_PIXELS`]); data
//! that ends early gives the rows decoded so far, the rest white, with [`Image::truncated`].

mod arith;
mod bie;
mod dp_table;
mod layers;

use bie::{Bid, Bih};

/// Largest width or height accepted.
pub const MAX_DIMENSION: u64 = 1 << 20;
/// Largest pixel count accepted.
pub const MAX_PIXELS: u64 = 1 << 28;
/// Most bit planes decoded.
pub const MAX_PLANES: u8 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a JBIG BIE.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement (a plain reason).
    Unsupported(String),
    /// The header is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over [`MAX_DIMENSION`] / [`MAX_PIXELS`].
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
            Error::NotRecognised => write!(f, "format not recognised"),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub pages: u32,
    pub bit_depth: u8,
    pub model: Model,
    pub has_alpha: bool,
    /// EXIF orientation; JBIG has none, always 1.
    pub orientation: u8,
    pub coding: String,
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
    /// Data ended early or was cut; the missing rows are white.
    pub truncated: bool,
}

/// Whether `head` (the first bytes of a file, at least 20) looks like a JBIG BIE. JBIG has no
/// signature; this checks every header field for a value the Recommendation allows and
/// encoders write.
pub fn probe(head: &[u8]) -> bool {
    bie::probe(head)
}

/// The header checked for what this decoder can show, and the image height (YD, or the
/// NEWLEN value).
fn open(data: &[u8], keep: bool) -> Result<(Bih, Bid, u32)> {
    let bih = bie::parse(data)?;
    if bih.dl != 0 {
        return Err(Error::Unsupported(
            "this JBIG file only adds detail to an image stored in another file, so it cannot \
             be shown on its own"
                .into(),
        ));
    }
    if bih.planes > MAX_PLANES {
        return Err(Error::Unsupported(format!(
            "JBIG images with more than {MAX_PLANES} bit planes are not supported (this one has {})",
            bih.planes
        )));
    }
    if matches!(bih.order & 7, 1 | 7) {
        return Err(Error::Malformed("invalid stripe order".into()));
    }
    let bid = bie::scan(data, bih.data_offset, keep)?;
    let mut yd = bih.yd;
    if let Some(n) = bid.newlen {
        if n == 0 {
            return Err(Error::Malformed("NEWLEN gives an empty image".into()));
        }
        yd = n.min(bih.yd);
    }
    if u64::from(bih.xd) > MAX_DIMENSION || u64::from(yd) > MAX_DIMENSION {
        return Err(Error::TooLarge);
    }
    if u64::from(bih.xd) * u64::from(yd) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    Ok((bih, bid, yd))
}

fn flags(v: u8, names: &[(u8, &str)]) -> String {
    let set: Vec<&str> = names
        .iter()
        .filter(|(b, _)| v & b != 0)
        .map(|(_, n)| *n)
        .collect();
    if set.is_empty() {
        "none".into()
    } else {
        set.join(", ")
    }
}

/// Header facts. `page` must be 0 (a BIE holds one image).
pub fn info(data: &[u8], page: u32) -> Result<Info> {
    if page != 0 {
        return Err(Error::Malformed("JBIG files hold one image".into()));
    }
    let (bih, bid, yd) = open(data, false)?;
    let two = bih.options & bie::LRLTWO != 0;
    let mut coding = format!(
        "JBIG, {} resolution layer{}, {}-line template",
        bih.layers(),
        if bih.layers() == 1 { "" } else { "s" },
        if two { 2 } else { 3 }
    );
    if bih.planes > 1 {
        coding += &format!(", {} bit planes", bih.planes);
    }
    let mut summary = Vec::new();
    if !bid.comments.is_empty() {
        summary.push(("comment", bid.comments.join("; ")));
    }
    summary.push(("compression", "JBIG (ITU-T T.82)".to_owned()));
    let stripes = bih.stripes(yd);
    let mut fields = vec![
        ("Width (XD)".to_owned(), bih.xd.to_string()),
        ("Height (YD)".to_owned(), bih.yd.to_string()),
    ];
    if let Some(n) = bid.newlen {
        fields.push(("New height (NEWLEN)".into(), n.to_string()));
    }
    fields.extend([
        ("Bit planes (P)".to_owned(), bih.planes.to_string()),
        ("Initial layer (DL)".to_owned(), bih.dl.to_string()),
        ("Final layer (D)".to_owned(), bih.d.to_string()),
        (
            "Lines per stripe in layer 0 (L0)".to_owned(),
            bih.l0.to_string(),
        ),
        ("Stripes".to_owned(), stripes.to_string()),
    ]);
    if bih.layers() > 1 {
        let sizes: Vec<String> = (bih.dl..=bih.d)
            .map(|d| {
                let (w, h) = bih.layer_size(d, yd);
                format!("{w}×{h}")
            })
            .collect();
        fields.push(("Layer sizes".into(), sizes.join(", ")));
    }
    fields.extend([
        (
            "Adaptive template range (MX, MY)".to_owned(),
            format!("{}, {}", bih.mx, bih.my),
        ),
        (
            "Order".to_owned(),
            flags(
                bih.order,
                &[
                    (bie::HITOLO, "HITOLO"),
                    (bie::SEQ, "SEQ"),
                    (bie::ILEAVE, "ILEAVE"),
                    (bie::SMID, "SMID"),
                ],
            ),
        ),
        (
            "Options".to_owned(),
            flags(
                bih.options,
                &[
                    (bie::LRLTWO, "LRLTWO"),
                    (bie::VLENGTH, "VLENGTH"),
                    (bie::TPDON, "TPDON"),
                    (bie::TPBON, "TPBON"),
                    (bie::DPON, "DPON"),
                    (bie::DPPRIV, "DPPRIV"),
                    (bie::DPLAST, "DPLAST"),
                ],
            ),
        ),
    ]);
    if bih.options & bie::DPON != 0 {
        fields.push((
            "Prediction table".into(),
            if bih.dptable.is_some() {
                "private"
            } else {
                "default"
            }
            .into(),
        ));
    }
    if bid.at_moves > 0 {
        fields.push(("Adaptive template moves".into(), bid.at_moves.to_string()));
    }
    for (i, c) in bid.comments.iter().enumerate() {
        fields.push((format!("Comment {}", i + 1), c.clone()));
    }
    Ok(Info {
        width: bih.xd,
        height: yd,
        pages: 1,
        bit_depth: bih.planes,
        model: if bih.planes == 1 {
            Model::Bilevel
        } else {
            Model::Grey
        },
        has_alpha: false,
        orientation: 1,
        coding,
        summary,
        fields,
    })
}

/// Decode the image at its highest resolution. `page` must be 0.
pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    if page != 0 {
        return Err(Error::Malformed("JBIG files hold one image".into()));
    }
    let (bih, bid, yd) = open(data, true)?;
    let dp_unpacked = (bih.options & bie::DPON != 0)
        .then(|| layers::dp_entries(bih.dptable.as_ref().unwrap_or(&dp_table::DEFAULT_DP)));
    let (w, h) = (bih.xd as usize, yd as usize);
    let planes = bih.planes;
    let mut truncated = bid.aborted;
    // A plane's resolution layers (a byte per pixel, a third more for the lower layers) and
    // the output (8-bit grey, or 16-bit plane bits then the samples).
    let per_pixel = if planes == 1 { 2 + 1 } else { 2 + 2 + 2 };
    let _res = specular_core::budget::reserve_n((w * h) as u64, per_pixel)?;
    // Per pixel: the plane bits, most significant plane highest.
    let mut acc: Vec<u16> = Vec::new();
    let mut gray8: Vec<u8> = Vec::new();
    if planes == 1 {
        gray8
            .try_reserve_exact(w * h)
            .map_err(|_| Error::TooLarge)?;
    } else {
        acc.try_reserve_exact(w * h).map_err(|_| Error::TooLarge)?;
        acc.resize(w * h, 0);
    }
    for p in (0..planes).rev() {
        let (lay, cut) = layers::decode_plane(&bih, &bid, yd, p, dp_unpacked.as_deref())?;
        truncated |= cut;
        let bits = |x: usize, y: usize| -> u8 { plane_pixel(&lay, bih.d - bih.dl, x, y) };
        if planes == 1 {
            let top = lay.last().filter(|l| l.width == w && l.height == h);
            for y in 0..h {
                match top.filter(|l| y < l.rows) {
                    Some(l) => {
                        let r = &l.px[y * l.stride + layers::PAD..][..w];
                        gray8.extend(r.iter().map(|&b| if b != 0 { 0 } else { 255 }));
                    }
                    None => gray8.extend((0..w).map(|x| match bits(x, y) {
                        1 => 0,
                        _ => 255,
                    })),
                }
            }
        } else {
            for y in 0..h {
                for x in 0..w {
                    let b = bits(x, y);
                    acc[y * w + x] |= u16::from(b & 1) << p;
                }
            }
        }
    }
    let pixels = if planes == 1 {
        Pixels::Gray8(gray8)
    } else {
        let max = (1u32 << planes) - 1;
        // Gray code to binary, MSB first.
        for v in acc.iter_mut() {
            let mut b = *v;
            let mut s = b >> 1;
            while s != 0 {
                b ^= s;
                s >>= 1;
            }
            *v = b;
        }
        if planes <= 8 {
            Pixels::Gray8(
                acc.iter()
                    .map(|&v| ((u32::from(v) * 255 + max / 2) / max) as u8)
                    .collect(),
            )
        } else {
            Pixels::Gray16(
                acc.iter()
                    .map(|&v| ((u32::from(v) * 65535 + max / 2) / max) as u16)
                    .collect(),
            )
        }
    };
    Ok(Image {
        width: bih.xd,
        height: yd,
        pixels,
        truncated,
    })
}

/// Pixel (x, y) of the full image from the best layer decoded: the final layer where its row
/// is decoded, else the highest complete layer enlarged, else 0 (white).
fn plane_pixel(lay: &[layers::Layer], top: u8, x: usize, y: usize) -> u8 {
    for (i, l) in lay.iter().enumerate().rev() {
        let k = u32::from(top) - i as u32;
        let (lx, ly) = (x >> k, y >> k);
        let complete = l.rows >= l.height;
        if (k == 0 && ly < l.rows) || complete {
            if lx < l.width && ly < l.height {
                return l.px[ly * l.stride + layers::PAD + lx];
            }
            return 0;
        }
    }
    0
}

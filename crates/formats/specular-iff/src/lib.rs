//! Amiga IFF pictures: ILBM, PBM and ACBM (Deluxe Paint and friends), RGB8 and RGBN
//! (Impulse Turbo Silver), DEEP (TVPaint) and ANIM animations.
//!
//! An IFF file is a big-endian chunk list inside `FORM` + length + form type. The forms read
//! here share the chunks that matter:
//!
//! * `BMHD` — width, height, plane count, masking, compression, transparent colour index and
//!   the pixel aspect ratio (`xAspect:yAspect`);
//! * `CMAP` — RGB triplets;
//! * `CAMG` — the Amiga display mode: `HAM` (0x0800) and `EHB` (0x0080) change how pixel
//!   values become colours;
//! * `BODY` (`ABIT` in ACBM) — the pixels.
//!
//! **ILBM** stores each row as one row of bits per plane (16-bit aligned), followed by a mask
//! plane when `masking` is 1. **PBM** stores one byte per pixel (rows padded to even length).
//! **ACBM** stores whole planes one after the other, uncompressed. `BODY` is either raw or
//! ByteRun1 coded: a control byte `n` in 0..=127 copies the next `n + 1` bytes, 129..=255
//! repeats the next byte `257 - n` times, 128 is a no-op. Runs that cross row or plane
//! boundaries (some writers produce them) carry over. ILBMs from Atari ST programs use
//! ByteRun2 (compression 2): a `VDAT` chunk per plane, coded down the columns (`vdat`).
//!
//! **RGB8** and **RGBN** hold 24-bit and 12-bit true colour as pixel runs with a genlock bit,
//! shown as transparency (`rgbx`). **DEEP** describes its pixels element by element (red,
//! green, blue, alpha; 8 or 16 bits), raw, ByteRun1 in whole pixels or TVDC delta coded
//! (`deep`). **ANIM** is a list of ILBM frames: the first whole, the others deltas
//! (operations 0, 3, 5, 7, 8, J and l) played by [`Player`]; [`decode`] gives the first.
//!
//! Pixel values become colours by the display mode:
//!
//! | Mode | Colour |
//! |---|---|
//! | 1–8 planes | `CMAP` entry (a grey ramp without a `CMAP`) |
//! | Extra Half-Brite (6 planes) | entries 32–63 are 0–31 at half brightness |
//! | HAM6 / HAM8 | the top two bits choose: palette entry, or modify blue, red or green of the previous pixel |
//! | 24 / 32 planes (deep) | bits 0–7 red, 8–15 green, 16–23 blue, 24–31 alpha |
//!
//! A `CMAP` holding only 4-bit values in the high nibble (OCS-era files) has the nibble
//! replicated (`0xA0` → `0xAA`), as the Amiga's display did. HAM6 modify values are 4-bit
//! and replicated the same way; HAM8 modify values replace the top six bits and keep the
//! low two of the previous pixel (AGA behaviour). Files without `CAMG` with 6 planes and a
//! 16-entry `CMAP` are taken as HAM6, with 32 entries as Extra Half-Brite.
//!
//! Line palettes change colours from row to row (`lines`): Sliced HAM (`SHAM`), Dynamic
//! HiRes (`CTBL`) and Palette Change (`PCHG`, uncompressed or Huffman-compressed) chunks.
//!
//! Masking: a mask plane (`mskHasMask`) and a transparent colour index
//! (`mskHasTransparentColor`, for palette images) become alpha; lasso masks are ignored.
//! The pixel aspect ratio is reported in [`Info::pixel_aspect`], not applied.
//!
//! Colour cycling (Deluxe Paint's `CRNG`, Graphicraft's `CCRT`, Deluxe Paint IV's `DRNG`)
//! rotates palette entries over time: [`cycling`] plays the active ranges of a palette image
//! as frames, each a `CMAP` to draw the pixels with ([`decode_with_cmap`]). `CRNG` steps
//! `rate / 16384 × 60` times a second (flag 1: active, 2: backwards); `CCRT` steps every
//! `seconds + microseconds` (direction 1 forwards, −1 backwards); a `DRNG` range of cells
//! `min..=max` holds palette registers and true colours (a cell without either keeps the
//! colour before it) and moves them across its registers. Forwards, each colour moves to the
//! next entry up and the last wraps to the first. The frames run on a 1/60 s clock until
//! every range is back where it started (or [`MAX_CYCLE_FRAMES`]) and then loop.
//!
//! Not supported (refused by name): ILBM plane counts other than 1–8, 24 and 32 (no reader
//! or writer defines them and no files exist; FFmpeg and TwelveMonkeys refuse them too);
//! DEEP Huffman and JPEG compression; ANIM operations 1 (XOR ILBM), 2 and 4 (no samples).
//!
//! Robustness: no input panics. Sizes are capped by [`MAX_PIXELS`]; allocation follows
//! only checked header sizes. Pixel data that ends early yields an image with
//! [`Image::truncated`] set and the missing rows grey.

mod anim;
mod deep;
mod lines;
mod rgbx;
mod vdat;

use std::fmt;

pub use anim::{AnimFrame, Player, anim_delays, anim_frames};
pub use lines::LinePalette;

/// Largest pixel count accepted (256 MP: 1 GiB as RGBA). Width and height are 16-bit.
pub const MAX_PIXELS: u64 = 1 << 28;
/// Upper bound on chunks walked; real files have a handful.
pub const MAX_CHUNKS: usize = 1 << 16;
/// The value missing pixels get.
pub const GREY: u8 = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not an ILBM, PBM, ACBM, RGB8, RGBN, DEEP or ANIM `FORM`.
    NotIff,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The header (or the structure needed to find the pixels) is cut short.
    Truncated(&'static str),
    Malformed(&'static str),
    /// Over [`MAX_PIXELS`].
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotIff => f.write_str("not an IFF picture"),
            Error::Unsupported(s) => write!(f, "unsupported IFF bitmap: {s}"),
            Error::Truncated(s) => write!(f, "IFF file is truncated: {s}"),
            Error::Malformed(s) => write!(f, "corrupt IFF file: {s}"),
            Error::TooLarge => f.write_str("IFF image too large"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// The `FORM` type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// Interleaved bitplanes.
    Ilbm,
    /// Chunky 8-bit pixels (Deluxe Paint II PC / "Planar BitMap" in name only).
    Pbm,
    /// Contiguous bitplanes (Amiga Contiguous BitMap).
    Acbm,
    /// Impulse Turbo Silver 24-bit run-length pixels with a genlock bit.
    Rgb8,
    /// Impulse Turbo Silver 12-bit run-length pixels with a genlock bit.
    Rgbn,
    /// TVPaint / NewTek deep chunky pixels (`DGBL`, `DPEL`, `DLOC`, `DBOD`).
    Deep,
}

impl Form {
    pub fn name(self) -> &'static str {
        match self {
            Form::Ilbm => "ILBM",
            Form::Pbm => "PBM",
            Form::Acbm => "ACBM",
            Form::Rgb8 => "RGB8",
            Form::Rgbn => "RGBN",
            Form::Deep => "DEEP",
        }
    }
}

/// How pixel values become colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 1–8 planes indexing the `CMAP`.
    Indexed,
    /// Extra Half-Brite: 6 planes, values 32–63 are colours 0–31 at half brightness.
    Ehb,
    /// Hold-And-Modify with `planes - 2` data bits (HAM6: 6 planes, HAM8: 8).
    Ham,
    /// 24-bit deep ILBM.
    Rgb24,
    /// 32-bit deep ILBM (alpha in bits 24–31), or DEEP with an alpha element.
    Rgba32,
    /// RGB8 (24-bit) or RGBN (12-bit) true colour; the genlock bit is transparency.
    Genlock,
}

/// How the pixels are coded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    /// ByteRun1 (PackBits-style) runs.
    ByteRun1,
    /// ByteRun2 (Atari ST): one `VDAT` chunk per plane, coded down the columns in words.
    Vdat,
    /// RGB8 / RGBN pixel runs (compression 4).
    Rgbx,
    /// DEEP TVDC: nibbles indexing a table of deltas, row by row and element by element.
    Tvdc,
}

impl Compression {
    pub fn name(self) -> &'static str {
        match self {
            Compression::None => "uncompressed",
            Compression::ByteRun1 => "ByteRun1",
            Compression::Vdat => "ByteRun2 (VDAT)",
            Compression::Rgbx => "run-length",
            Compression::Tvdc => "TVDC",
        }
    }
}

/// `BMHD` masking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Masking {
    None,
    /// A mask plane follows the colour planes of each row.
    Mask,
    /// Pixels of this palette index are transparent.
    TransparentColor(u16),
    /// Lasso mask (a drawing-tool hint; ignored).
    Lasso,
}

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub form: Form,
    pub width: u32,
    pub height: u32,
    /// Bitplanes (8 for PBM), mask plane not counted.
    pub planes: u8,
    pub kind: Kind,
    pub masking: Masking,
    /// `BODY` is compressed ([`Info::compression`] says how).
    pub compressed: bool,
    pub compression: Compression,
    /// `xAspect:yAspect` when the pixels are not square (e.g. 10:11 for PAL low-res). Not
    /// applied by [`decode`].
    pub pixel_aspect: Option<(u8, u8)>,
    /// The file has colour-cycling ranges (`CRNG`, `CCRT`, `DRNG`); [`cycling`] plays the
    /// active ones.
    pub colour_cycling: bool,
    /// Some pixel may be less than opaque (mask plane, transparent colour, 32-bit deep).
    pub has_alpha: bool,
    /// Entries in the `CMAP` (0 without one).
    pub palette_len: usize,
    /// Frames of an `ANIM` (0 for a still picture); the picture is its first frame.
    pub anim_frames: usize,
    /// Colours that change from row to row (`SHAM`, `CTBL`, `PCHG`).
    pub line_palette: Option<LinePalette>,
}

impl Info {
    /// Short technical description, e.g. "ILBM HAM6, ByteRun1".
    pub fn coding(&self) -> String {
        let mut s = String::from(self.form.name());
        match self.kind {
            Kind::Indexed if self.form == Form::Pbm => s.push_str(" 8-bit chunky"),
            Kind::Indexed => {
                let p = self.planes;
                s.push_str(&format!(" {p} bitplane{}", if p == 1 { "" } else { "s" }))
            }
            Kind::Ehb => s.push_str(" Extra Half-Brite"),
            Kind::Ham => s.push_str(&format!(" HAM{}", self.planes)),
            Kind::Rgb24 => s.push_str(" 24-bit"),
            Kind::Rgba32 => s.push_str(" 32-bit"),
            Kind::Genlock if self.form == Form::Rgbn => s.push_str(" 12-bit"),
            Kind::Genlock => s.push_str(" 24-bit"),
        }
        s.push_str(", ");
        s.push_str(self.compression.name());
        match self.masking {
            Masking::Mask => s.push_str(", mask plane"),
            Masking::TransparentColor(i) if self.has_alpha => {
                s.push_str(&format!(", transparent colour {i}"))
            }
            _ => {}
        }
        if let Some((x, y)) = self.pixel_aspect {
            s.push_str(&format!(", pixel aspect {x}:{y}"));
        }
        if let Some(l) = self.line_palette {
            s.push_str(", ");
            s.push_str(l.name());
        }
        if self.anim_frames > 0 {
            s = format!("ANIM of {} frames, {s}", self.anim_frames);
        }
        s
    }
}

/// Decoded pixels: straight RGBA8, `width × height`, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Pixel data ended early; the missing rows are grey.
    pub truncated: bool,
}

/// Whether `head` starts like an ILBM, PBM, ACBM, RGB8, RGBN, DEEP or ANIM file (12 bytes
/// are enough).
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 12
        && &head[..4] == b"FORM"
        && (form_of(&head[8..12]).is_some() || &head[8..12] == b"ANIM")
}

fn form_of(t: &[u8]) -> Option<Form> {
    match t {
        b"ILBM" => Some(Form::Ilbm),
        b"PBM " => Some(Form::Pbm),
        b"ACBM" => Some(Form::Acbm),
        b"RGB8" => Some(Form::Rgb8),
        b"RGBN" => Some(Form::Rgbn),
        b"DEEP" => Some(Form::Deep),
        _ => None,
    }
}

fn be16(d: &[u8], p: usize) -> u16 {
    u16::from_be_bytes([d[p], d[p + 1]])
}

fn be32(d: &[u8], p: usize) -> u32 {
    u32::from_be_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]])
}

const CAMG_HAM: u32 = 0x0800;
const CAMG_EHB: u32 = 0x0080;

struct Parsed<'a> {
    info: Info,
    cmap: &'a [u8],
    /// `BODY` (ILBM, PBM, RGB8, RGBN), `ABIT` (ACBM) or `DBOD` (DEEP) payload.
    body: &'a [u8],
    /// The colour-cycling chunks, in file order.
    ranges: Vec<(&'a [u8; 4], &'a [u8])>,
    /// DEEP: the pixel layout.
    deep: Option<deep::Layout>,
    /// The line palette chunk.
    lines: Option<(LinePalette, &'a [u8])>,
}

/// The chunks of the `FORM` at the start of `data`: (form type, end of its data).
fn form_bounds(data: &[u8]) -> Result<(&[u8], usize)> {
    if data.len() < 12 || &data[..4] != b"FORM" {
        let n = data.len().min(4);
        return Err(if data.len() < 12 && b"FORM"[..n] == data[..n] {
            Error::Truncated("header")
        } else {
            Error::NotIff
        });
    }
    let declared = be32(data, 4) as usize;
    // A bogus FORM length (some writers leave 0) falls back to the data.
    let end = if declared >= 4 {
        declared.saturating_add(8).min(data.len())
    } else {
        data.len()
    };
    Ok((&data[8..12], end))
}

fn parse(data: &[u8]) -> Result<Parsed<'_>> {
    let (kind, _) = form_bounds(data)?;
    if kind == b"ANIM" {
        let (first, frames) = anim::first_frame(data)?;
        let mut p = parse(first)?;
        p.info.anim_frames = frames;
        return Ok(p);
    }
    let form = form_of(kind).ok_or(Error::NotIff)?;
    if form == Form::Deep {
        return deep::parse(data);
    }
    let (_, end) = form_bounds(data)?;
    let (mut bmhd, mut cmap, mut camg, mut body) = (None, None, None, None);
    let mut cycling = false;
    let mut ranges = Vec::new();
    let mut lines = None;
    let mut pos = 12usize;
    let mut n = 0;
    while pos.saturating_add(8) <= end && n < MAX_CHUNKS {
        n += 1;
        let id = &data[pos..pos + 4];
        let len = be32(data, pos + 4) as usize;
        let start = pos + 8;
        let stop = start.saturating_add(len);
        let payload = &data[start..stop.min(end)];
        match id {
            b"BMHD" if bmhd.is_none() => bmhd = Some(payload),
            b"CMAP" if cmap.is_none() => cmap = Some(payload),
            b"CAMG" if payload.len() >= 4 => camg = Some(be32(payload, 0)),
            b"BODY" if form != Form::Acbm && body.is_none() => body = Some(payload),
            b"ABIT" if form == Form::Acbm && body.is_none() => body = Some(payload),
            b"SHAM" if lines.is_none() => lines = Some((LinePalette::Sham, payload)),
            b"CTBL" if lines.is_none() => lines = Some((LinePalette::Ctbl, payload)),
            // Uncompressed (0) or Huffman-compressed (1) changes.
            b"PCHG" if lines.is_none() && payload.len() >= 2 && be16(payload, 0) <= 1 => {
                lines = Some((LinePalette::Pchg, payload))
            }
            b"CRNG" | b"CCRT" | b"DRNG" => {
                cycling = true;
                if ranges.len() < MAX_RANGES {
                    ranges.push((
                        if id == b"CRNG" {
                            b"CRNG"
                        } else if id == b"CCRT" {
                            b"CCRT"
                        } else {
                            b"DRNG"
                        },
                        payload,
                    ));
                }
            }
            _ => {}
        }
        pos = stop.saturating_add(len & 1);
    }
    let bmhd = bmhd.ok_or(Error::Malformed("no BMHD chunk"))?;
    if bmhd.len() < 20 {
        return Err(Error::Truncated("BMHD"));
    }
    let width = u32::from(be16(bmhd, 0));
    let height = u32::from(be16(bmhd, 2));
    let planes = bmhd[8];
    let masking = match bmhd[9] {
        1 => Masking::Mask,
        2 => Masking::TransparentColor(be16(bmhd, 12)),
        3 => Masking::Lasso,
        _ => Masking::None,
    };
    let compression = match (form, bmhd[10]) {
        // ABIT is never compressed.
        (Form::Acbm, _) => Compression::None,
        (Form::Rgb8 | Form::Rgbn, 4) => Compression::Rgbx,
        (Form::Rgb8 | Form::Rgbn, c) => {
            return Err(Error::Unsupported(format!(
                "{} compression type {c}",
                form.name()
            )));
        }
        (_, 0) => Compression::None,
        (_, 1) => Compression::ByteRun1,
        (Form::Ilbm, 2) => Compression::Vdat,
        (_, c) => return Err(Error::Unsupported(format!("compression type {c}"))),
    };
    let (xa, ya) = (bmhd[14], bmhd[15]);
    let pixel_aspect = (xa != 0 && ya != 0 && xa != ya).then_some((xa, ya));
    if width == 0 || height == 0 {
        return Err(Error::Malformed("zero width or height"));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let cmap = cmap.unwrap_or(&[]);
    let palette_len = (cmap.len() / 3).min(256);
    let kind = match (form, planes) {
        (Form::Rgb8, _) | (Form::Rgbn, _) => Kind::Genlock,
        (Form::Pbm, 1..=8) => Kind::Indexed,
        (Form::Pbm, p) => {
            return Err(Error::Unsupported(format!("PBM with {p} planes")));
        }
        (_, 24) => Kind::Rgb24,
        (_, 32) => Kind::Rgba32,
        (_, 1..=8) => match camg {
            Some(m) if m & CAMG_HAM != 0 && planes >= 3 => Kind::Ham,
            Some(m) if m & CAMG_EHB != 0 && planes == 6 => Kind::Ehb,
            // Older writers left CAMG out; the palette size tells.
            None if planes == 6 && (1..=16).contains(&palette_len) => Kind::Ham,
            None if planes == 6 && palette_len == 32 => Kind::Ehb,
            _ => Kind::Indexed,
        },
        (_, 0) => return Err(Error::Malformed("zero bitplanes")),
        // No reader or writer defines other depths (9–23, 25–31 planes): no files exist.
        (_, p) => return Err(Error::Unsupported(format!("{p} bitplanes"))),
    };
    let has_alpha = match masking {
        _ if kind == Kind::Genlock => true,
        Masking::Mask => true,
        Masking::TransparentColor(_) => matches!(kind, Kind::Indexed | Kind::Ehb),
        _ => kind == Kind::Rgba32,
    };
    let body = body.ok_or(Error::Malformed("no BODY chunk"))?;
    Ok(Parsed {
        info: Info {
            form,
            width,
            height,
            planes,
            kind,
            masking,
            compressed: compression != Compression::None,
            compression,
            pixel_aspect,
            colour_cycling: cycling,
            has_alpha,
            palette_len,
            anim_frames: 0,
            line_palette: lines
                .filter(|_| matches!(kind, Kind::Indexed | Kind::Ehb | Kind::Ham))
                .map(|l| l.0),
        },
        cmap,
        body,
        ranges,
        deep: None,
        lines,
    })
}

/// Header facts without decoding pixels.
pub fn info(data: &[u8]) -> Result<Info> {
    parse(data).map(|p| p.info)
}

/// The colour table: the `CMAP` (4-bit values widened), Extra Half-Brite's halves, or a
/// grey ramp when there is no `CMAP`.
pub(crate) fn palette(cmap: &[u8], info: &Info) -> Vec<[u8; 3]> {
    let mut p: Vec<[u8; 3]> = cmap
        .chunks_exact(3)
        .take(256)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let flat = p.iter().flatten();
    let ocs = flat.clone().all(|v| v & 0x0f == 0) && flat.clone().any(|&v| v != 0);
    if info.kind == Kind::Ehb {
        // The hardware ignores colours 32–63 of the CMAP (often junk) and shows 0–31
        // shifted right one bit — in 4-bit precision on OCS palettes.
        p.resize(32, [0; 3]);
        let keep = if ocs { 0xf0 } else { 0xff };
        for i in 0..32 {
            let [r, g, b] = p[i];
            p.push([(r >> 1) & keep, (g >> 1) & keep, (b >> 1) & keep]);
        }
    }
    if ocs {
        for v in p.iter_mut().flatten() {
            *v |= *v >> 4;
        }
    }
    if p.is_empty() && info.planes <= 8 {
        let levels = 1u32 << info.planes;
        let max = (levels - 1).max(1);
        p = (0..levels)
            .map(|i| {
                let v = (i * 255 / max) as u8;
                [v; 3]
            })
            .collect();
    }
    p
}

/// Widens a `bits`-bit value to 8 bits by bit replication.
fn widen(v: u32, bits: u32) -> u8 {
    let mut out = 0u32;
    let mut shift = 8i32;
    while shift > 0 {
        shift -= bits as i32;
        out |= if shift >= 0 { v << shift } else { v >> -shift };
    }
    out as u8
}

/// Streaming ByteRun1 decoder; runs may carry over from one row into the next.
pub(crate) struct ByteRun1<'a> {
    src: &'a [u8],
    pos: usize,
    literal: usize,
    run: usize,
    value: u8,
}

impl<'a> ByteRun1<'a> {
    pub(crate) fn new(src: &'a [u8]) -> Self {
        ByteRun1 {
            src,
            pos: 0,
            literal: 0,
            run: 0,
            value: 0,
        }
    }

    /// Fills `out`; false when the data ends first (the rest of `out` is unchanged).
    pub(crate) fn fill(&mut self, out: &mut [u8]) -> bool {
        let mut o = 0;
        while o < out.len() {
            if self.run > 0 {
                let n = self.run.min(out.len() - o);
                out[o..o + n].fill(self.value);
                o += n;
                self.run -= n;
            } else if self.literal > 0 {
                let avail = self.src.len() - self.pos;
                if avail == 0 {
                    return false;
                }
                let n = self.literal.min(out.len() - o).min(avail);
                out[o..o + n].copy_from_slice(&self.src[self.pos..self.pos + n]);
                o += n;
                self.pos += n;
                self.literal -= n;
            } else {
                let Some(&c) = self.src.get(self.pos) else {
                    return false;
                };
                self.pos += 1;
                match c {
                    0..=127 => self.literal = usize::from(c) + 1,
                    128 => {}
                    _ => {
                        let Some(&v) = self.src.get(self.pos) else {
                            return false;
                        };
                        self.pos += 1;
                        self.run = 257 - usize::from(c);
                        self.value = v;
                    }
                }
            }
        }
        true
    }
}

/// Decodes the whole image to RGBA8.
pub fn decode(data: &[u8]) -> Result<Image> {
    decode_inner(data, None)
}

/// [`decode`] with the palette taken from `cmap` (RGB triplets, as a `CMAP` chunk holds them)
/// instead of the file's: a frame of [`cycling`].
pub fn decode_with_cmap(data: &[u8], cmap: &[u8]) -> Result<Image> {
    decode_inner(data, Some(cmap))
}

fn decode_inner(data: &[u8], cmap_override: Option<&[u8]>) -> Result<Image> {
    let p = parse(data)?;
    if p.info.anim_frames > 0 && cmap_override.is_none() {
        // The first frame, as the player draws it (its BODY may be ByteRun1 or VDAT).
        return anim::Player::new(data)?
            .next_frame(data)
            .unwrap_or(Err(Error::Malformed("no frames")))
            .map(|f| f.image);
    }
    render(p, cmap_override)
}

/// Decodes a parsed picture; `cmap_override` replaces its `CMAP`.
fn render(p: Parsed, cmap_override: Option<&[u8]>) -> Result<Image> {
    let Parsed {
        mut info,
        cmap,
        body,
        deep,
        lines,
        ..
    } = p;
    let lines = match (info.line_palette, lines) {
        (Some(_), Some((k, d))) => lines::Lines::parse(k, d, info.height as usize),
        _ => None,
    };
    let cmap = cmap_override.unwrap_or(cmap);
    match info.form {
        Form::Rgb8 | Form::Rgbn => return Ok(rgbx::decode(&info, body)),
        Form::Deep => {
            let layout = deep.ok_or(Error::Malformed("DEEP layout"))?;
            return Ok(deep::decode(&info, &layout, body));
        }
        _ => {}
    }
    // ByteRun2: the planes come out whole, laid out as an ACBM's.
    let vdat_planes;
    let body = if info.compression == Compression::Vdat {
        let n = usize::from(info.planes) + usize::from(info.masking == Masking::Mask);
        vdat_planes = vdat::decode(body, info.width as usize, info.height as usize, n);
        info.form = Form::Acbm;
        &vdat_planes[..]
    } else {
        body
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let planes = usize::from(info.planes);
    let has_mask = info.masking == Masking::Mask;
    // Bytes of one plane row (16-bit aligned).
    let plane_row = w.div_ceil(16) * 2;
    let (chunky_row, row_len) = match info.form {
        Form::Pbm => {
            let c = w + (w & 1);
            (c, c + if has_mask { plane_row } else { 0 })
        }
        _ => (0, plane_row * (planes + usize::from(has_mask))),
    };
    let mut pal = palette(cmap, &info);
    // The registers before Extra Half-Brite's halves, which line palettes change.
    let mut regs: Vec<[u8; 3]> = pal.clone();
    if info.kind == Kind::Ehb {
        regs.truncate(32);
    }
    let transparent = match info.masking {
        Masking::TransparentColor(t) if info.has_alpha => Some(u32::from(t)),
        _ => None,
    };
    let mut pixels = vec![0u8; w * h * 4];
    let mut row = vec![0u8; row_len];
    let mut vals = vec![0u32; w];
    let mut rle = ByteRun1 {
        src: body,
        pos: 0,
        literal: 0,
        run: 0,
        value: 0,
    };
    let mut truncated = false;
    let ham_bits = u32::from(info.planes).saturating_sub(2);
    for (y, out) in pixels.chunks_exact_mut(w * 4).enumerate() {
        let complete = if truncated {
            false
        } else if info.form == Form::Acbm {
            // Gather row `y` of every plane into the ILBM row layout.
            let mut ok = true;
            for p in 0..planes + usize::from(has_mask) {
                let at = (p * h + y) * plane_row;
                match body.get(at..at + plane_row) {
                    Some(s) => row[p * plane_row..(p + 1) * plane_row].copy_from_slice(s),
                    None => ok = false,
                }
            }
            ok
        } else if info.compressed {
            rle.fill(&mut row)
        } else {
            match body.get(y * row_len..(y + 1) * row_len) {
                Some(s) => {
                    row.copy_from_slice(s);
                    true
                }
                None => false,
            }
        };
        if !complete {
            truncated = true;
            for px in out.chunks_exact_mut(4) {
                px.copy_from_slice(&[GREY, GREY, GREY, 255]);
            }
            continue;
        }
        if let Some(l) = &lines
            && l.apply(y, &mut regs)
        {
            pal.clone_from(&regs);
            if info.kind == Kind::Ehb {
                pal.resize(32, [0; 3]);
                for i in 0..32 {
                    let [r, g, b] = pal[i];
                    pal.push([r >> 1, g >> 1, b >> 1]);
                }
            }
        }
        let colour = |i: u32| pal.get(i as usize).copied().unwrap_or([0; 3]);
        // Pixel values.
        let mask_row;
        if info.form == Form::Pbm {
            for (v, &b) in vals.iter_mut().zip(&row[..w]) {
                *v = u32::from(b);
            }
            mask_row = &row[chunky_row..];
        } else {
            vals.fill(0);
            for p in 0..planes {
                let src = &row[p * plane_row..(p + 1) * plane_row];
                for (bi, &b) in src.iter().enumerate().take(w.div_ceil(8)) {
                    if b == 0 {
                        continue;
                    }
                    for k in 0..8 {
                        let x = bi * 8 + k;
                        if x < w && b & (0x80 >> k) != 0 {
                            vals[x] |= 1 << p;
                        }
                    }
                }
            }
            mask_row = &row[planes * plane_row..];
        }
        // Colours.
        match info.kind {
            Kind::Indexed | Kind::Ehb => {
                for (px, &v) in out.chunks_exact_mut(4).zip(&vals) {
                    let [r, g, b] = colour(v);
                    let a = if transparent == Some(v) { 0 } else { 255 };
                    px.copy_from_slice(&[r, g, b, a]);
                }
            }
            Kind::Ham => {
                let dmask = (1u32 << ham_bits) - 1;
                let mut cur = colour(0);
                for (px, &v) in out.chunks_exact_mut(4).zip(&vals) {
                    let d = v & dmask;
                    let modify = |c: u8| {
                        if ham_bits >= 6 {
                            // AGA: replace the top bits, keep the low ones.
                            let keep = (1u32 << (8 - ham_bits.min(8))) - 1;
                            ((u32::from(c) & keep) | (d << (8 - ham_bits.min(8)))) as u8
                        } else {
                            widen(d, ham_bits)
                        }
                    };
                    match v >> ham_bits {
                        0 => cur = colour(d),
                        1 => cur[2] = modify(cur[2]),
                        2 => cur[0] = modify(cur[0]),
                        _ => cur[1] = modify(cur[1]),
                    }
                    px.copy_from_slice(&[cur[0], cur[1], cur[2], 255]);
                }
            }
            Kind::Rgb24 | Kind::Rgba32 | Kind::Genlock => {
                let alpha = info.kind == Kind::Rgba32;
                for (px, &v) in out.chunks_exact_mut(4).zip(&vals) {
                    let a = if alpha { (v >> 24) as u8 } else { 255 };
                    px.copy_from_slice(&[v as u8, (v >> 8) as u8, (v >> 16) as u8, a]);
                }
            }
        }
        if has_mask {
            for (x, px) in out.chunks_exact_mut(4).enumerate() {
                if mask_row
                    .get(x / 8)
                    .is_some_and(|b| b & (0x80 >> (x % 8)) == 0)
                {
                    px[3] = 0;
                }
            }
        }
    }
    // A 32-bit file whose alpha planes are all clear meant no alpha.
    if info.kind == Kind::Rgba32 && !has_mask && pixels.chunks_exact(4).all(|p| p[3] == 0) {
        pixels.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    Ok(Image {
        width: info.width,
        height: info.height,
        pixels,
        truncated,
    })
}

/// Colour-cycling ranges kept from one file, at most.
const MAX_RANGES: usize = 64;
/// Frames of a colour-cycling animation, at most.
pub const MAX_CYCLE_FRAMES: usize = 2048;
/// The clock the ranges step on: 60 ticks a second.
const TICK_HZ: u64 = 60;
/// Ticks looked at for the animation to come back to its start, at most (20 minutes).
const MAX_TICKS: u64 = TICK_HZ * 60 * 20;

/// One frame of a colour-cycling animation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CycleFrame {
    /// The palette: RGB triplets to draw the pixels with ([`decode_with_cmap`]).
    pub cmap: Vec<u8>,
    /// How long the frame shows.
    pub delay_ms: u32,
}

/// One range: the palette entries it moves (in order), the colours it starts with, how
/// often it steps and which way.
#[derive(Clone, Debug)]
struct Range {
    /// Palette registers, in cycle order.
    regs: Vec<usize>,
    /// The colours on the cells, in the same order (at step 0).
    cells: Vec<[u8; 3]>,
    /// Steps per tick: `num / den` (CRNG: `rate / 16384`).
    num: u64,
    den: u64,
    reverse: bool,
}

/// The colour-cycling frames of a palette image (ILBM, PBM or ACBM with up to 8 planes,
/// not HAM or deep): `None` when it has no active range. The first frame is the file's own
/// palette; the animation loops.
pub fn cycling(data: &[u8]) -> Result<Option<Vec<CycleFrame>>> {
    let p = parse(data)?;
    if !matches!(p.info.kind, Kind::Indexed | Kind::Ehb) || p.ranges.is_empty() {
        return Ok(None);
    }
    let entries = (p.cmap.len() / 3).min(256);
    if entries < 2 {
        return Ok(None);
    }
    let base: Vec<[u8; 3]> = p
        .cmap
        .chunks_exact(3)
        .take(entries)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let ranges: Vec<Range> = p
        .ranges
        .iter()
        .filter_map(|&(id, c)| range(id, c, &base))
        .collect();
    if ranges.is_empty() {
        return Ok(None);
    }
    // Phase accumulators (`den` = one step), position in the cycle, steps taken in all.
    let mut acc = vec![0u64; ranges.len()];
    let mut steps = vec![0usize; ranges.len()];
    let mut taken = vec![0usize; ranges.len()];
    let palette = |steps: &[usize], ranges: &[Range]| -> Vec<u8> {
        let mut pal = base.clone();
        for (r, &k) in ranges.iter().zip(steps) {
            let n = r.regs.len();
            for (i, &reg) in r.regs.iter().enumerate() {
                // Forwards, the colour of cell i - k is now at register i.
                let from = if r.reverse {
                    (i + k) % n
                } else {
                    (i + n - k % n) % n
                };
                // Cells without a register of their own (DRNG) are not drawn.
                if let Some(slot) = pal.get_mut(reg) {
                    *slot = r.cells[from];
                }
            }
        }
        pal.into_iter().flatten().collect()
    };
    let mut frames = vec![CycleFrame {
        cmap: palette(&steps, &ranges),
        delay_ms: 0,
    }];
    let mut since = 0u64;
    for _ in 0..MAX_TICKS {
        since += 1;
        let mut changed = false;
        for (i, r) in ranges.iter().enumerate() {
            acc[i] += r.num;
            while acc[i] >= r.den {
                acc[i] -= r.den;
                steps[i] = (steps[i] + 1) % r.regs.len();
                taken[i] += 1;
                changed = true;
            }
        }
        if !changed {
            continue;
        }
        if let Some(last) = frames.last_mut() {
            last.delay_ms = (since * 1000 / TICK_HZ) as u32;
        }
        since = 0;
        // Every range back at its start after going round at least once: the animation
        // loops from frame 0 (a remainder of a step left on a range's clock shifts its next
        // step by under a tick).
        if steps.iter().all(|&k| k == 0) && taken.iter().all(|&t| t > 0) {
            break;
        }
        if frames.len() >= MAX_CYCLE_FRAMES {
            break;
        }
        frames.push(CycleFrame {
            cmap: palette(&steps, &ranges),
            delay_ms: 0,
        });
    }
    if frames.len() < 2 {
        return Ok(None);
    }
    if let Some(last) = frames.last_mut()
        && last.delay_ms == 0
    {
        last.delay_ms = ((since.max(1)) * 1000 / TICK_HZ) as u32;
    }
    Ok(Some(frames))
}

/// A cycling range from its chunk, `None` when inactive or empty.
fn range(id: &[u8; 4], c: &[u8], base: &[[u8; 3]]) -> Option<Range> {
    let n = base.len();
    let be16 = |p: usize| Some(i16::from_be_bytes([*c.get(p)?, *c.get(p + 1)?]));
    let regs = |lo: usize, hi: usize| -> Option<Vec<usize>> {
        (lo < hi && hi < n).then(|| (lo..=hi).collect())
    };
    match id {
        b"CRNG" => {
            let rate = be16(2)?;
            let flags = be16(4)?;
            let (lo, hi) = (usize::from(*c.get(6)?), usize::from(*c.get(7)?));
            // Bits other than active (1) and backwards (2) set: not a range (padding).
            if flags & 1 == 0 || flags & !3 != 0 || rate <= 0 {
                return None;
            }
            let regs = regs(lo, hi)?;
            let cells = regs.iter().map(|&r| base[r]).collect();
            Some(Range {
                regs,
                cells,
                num: rate as u64,
                den: 1 << 14,
                reverse: flags & 2 != 0,
            })
        }
        b"CCRT" => {
            let dir = be16(0)?;
            let (lo, hi) = (usize::from(*c.get(2)?), usize::from(*c.get(3)?));
            let secs = u32::from_be_bytes(c.get(4..8)?.try_into().ok()?);
            let micros = u32::from_be_bytes(c.get(8..12)?.try_into().ok()?);
            if dir == 0 {
                return None;
            }
            let us = u64::from(secs) * 1_000_000 + u64::from(micros);
            if us == 0 {
                return None;
            }
            let regs = regs(lo, hi)?;
            let cells = regs.iter().map(|&r| base[r]).collect();
            Some(Range {
                regs,
                cells,
                // Steps per tick: (1e6 / 60) / us.
                num: 1_000_000,
                den: TICK_HZ * us,
                reverse: dir < 0,
            })
        }
        _ => {
            // DRNG: min, max, rate, flags, ntrue, nregs, then the true colours (cell, r, g,
            // b) and the registers (cell, index).
            let (min, max) = (usize::from(*c.first()?), usize::from(*c.get(1)?));
            let rate = be16(2)?;
            let flags = be16(4)?;
            let (ntrue, nregs) = (usize::from(*c.get(6)?), usize::from(*c.get(7)?));
            if flags & 1 == 0 || flags & !3 != 0 || rate <= 0 || min >= max {
                return None;
            }
            let cells_n = max - min + 1;
            let mut cells: Vec<Option<[u8; 3]>> = vec![None; cells_n];
            let mut reg_at: Vec<Option<usize>> = vec![None; cells_n];
            let mut at = 8;
            for _ in 0..ntrue {
                let e = c.get(at..at + 4)?;
                at += 4;
                let cell = usize::from(e[0]);
                if (min..=max).contains(&cell) {
                    cells[cell - min] = Some([e[1], e[2], e[3]]);
                }
            }
            for _ in 0..nregs {
                let e = c.get(at..at + 2)?;
                at += 2;
                let (cell, index) = (usize::from(e[0]), usize::from(e[1]));
                if (min..=max).contains(&cell) && index < n {
                    reg_at[cell - min] = Some(index);
                    cells[cell - min] = Some(base[index]);
                }
            }
            // Cells without a colour keep the one before them.
            let mut last = cells.iter().flatten().next().copied()?;
            let colours: Vec<[u8; 3]> = cells
                .iter()
                .map(|c| {
                    if let Some(v) = c {
                        last = *v;
                    }
                    last
                })
                .collect();
            // Registers sit on their cells; a cell without a register shows only in passing.
            let mut all_regs = Vec::with_capacity(cells_n);
            let mut all_cells = Vec::with_capacity(cells_n);
            for (i, col) in colours.into_iter().enumerate() {
                all_regs.push(reg_at[i]);
                all_cells.push(col);
            }
            drng(all_regs, all_cells, rate as u64, flags & 2 != 0)
        }
    }
}

/// A DRNG range as a [`Range`]: registers on some of the cells; every cell's colour moves.
/// Cells without a register are given none (their colour is not shown until it reaches one).
fn drng(regs: Vec<Option<usize>>, cells: Vec<[u8; 3]>, rate: u64, reverse: bool) -> Option<Range> {
    // A plain range over the cells; registers stand where the file puts them, and cells
    // without one get a register of their own past the palette (dropped when drawn).
    let n = cells.len();
    if n < 2 || regs.iter().all(Option::is_none) {
        return None;
    }
    Some(Range {
        regs: regs
            .into_iter()
            .enumerate()
            .map(|(i, r)| r.unwrap_or(usize::MAX - i))
            .collect(),
        cells,
        num: rate,
        den: 1 << 14,
        reverse,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], d: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend_from_slice(&(d.len() as u32).to_be_bytes());
        v.extend_from_slice(d);
        if d.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    fn bmhd(w: u16, h: u16, planes: u8, masking: u8, comp: u8) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[0, 0, 0, 0, planes, masking, comp, 0, 0, 0, 10, 11]);
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b
    }

    fn form(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
        let body = chunks.concat();
        let mut f = b"FORM".to_vec();
        f.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
        f.extend_from_slice(kind);
        f.extend_from_slice(&body);
        f
    }

    /// 2-plane 3×2 ILBM: values 0..3 cycling.
    fn tiny() -> Vec<u8> {
        // Row values: [0,1,2] then [3,0,1]. Plane 0 bits, plane 1 bits per row.
        let rows: [[u8; 4]; 2] = [
            [0b0100_0000, 0, 0b0010_0000, 0],
            [0b1010_0000, 0, 0b1000_0000, 0],
        ];
        let body: Vec<u8> = rows.concat();
        form(
            b"ILBM",
            &[
                chunk(b"BMHD", &bmhd(3, 2, 2, 0, 0)),
                chunk(b"CMAP", &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]),
                chunk(b"BODY", &body),
            ],
        )
    }

    /// A 1-plane 2×3 picture, value 1 everywhere, whose colour 1 changes on every row.
    #[test]
    fn line_palettes_change_colours_by_row() {
        let rows: Vec<u8> = [[0xc0u8, 0]; 3].concat();
        let base = |extra: Vec<u8>| {
            form(
                b"ILBM",
                &[
                    chunk(b"BMHD", &bmhd(2, 3, 1, 0, 0)),
                    chunk(b"CMAP", &[0, 0, 0, 9, 9, 9]),
                    extra,
                    chunk(b"BODY", &rows),
                ],
            )
        };
        let row_colours = |img: &Image| -> Vec<[u8; 3]> {
            img.pixels
                .chunks_exact(8)
                .map(|p| [p[0], p[1], p[2]])
                .collect()
        };
        // CTBL: 16 words a row.
        let mut ctbl = Vec::new();
        for y in 0..3u16 {
            for r in 0..16u16 {
                let c: u16 = if r == 1 { 0xf00 >> (4 * y) } else { 0 };
                ctbl.extend_from_slice(&c.to_be_bytes());
            }
        }
        let f = base(chunk(b"CTBL", &ctbl));
        assert_eq!(info(&f).unwrap().line_palette, Some(LinePalette::Ctbl));
        assert!(info(&f).unwrap().coding().ends_with("Dynamic HiRes"));
        let img = decode(&f).unwrap();
        assert_eq!(row_colours(&img), [[255, 0, 0], [0, 255, 0], [0, 0, 255]]);
        // PCHG: row 1 only; the change lasts.
        let mut d = vec![0u8; 20];
        d[2..4].copy_from_slice(&1u16.to_be_bytes());
        d[6..8].copy_from_slice(&3u16.to_be_bytes());
        d.extend_from_slice(&[0x40, 0, 0, 0, 1, 0, 0x1f, 0x00]);
        let img = decode(&base(chunk(b"PCHG", &d))).unwrap();
        assert_eq!(row_colours(&img), [[9, 9, 9], [255, 0, 0], [255, 0, 0]]);
        // The same changes Huffman-compressed.
        let f = base(chunk(b"PCHG", &lines::huff_enc::compress_chunk(&d)));
        assert_eq!(info(&f).unwrap().line_palette, Some(LinePalette::Pchg));
        assert_eq!(
            row_colours(&decode(&f).unwrap()),
            [[9, 9, 9], [255, 0, 0], [255, 0, 0]]
        );
        // A damaged tree: the CMAP colours.
        let mut bad = lines::huff_enc::compress_chunk(&d);
        bad[20..24].copy_from_slice(&0u32.to_be_bytes());
        let f = base(chunk(b"PCHG", &bad));
        assert_eq!(row_colours(&decode(&f).unwrap()), [[9, 9, 9]; 3]);
    }

    /// The real PCHG pictures of PCHGLib 1.4 (Aminet `dev/misc/PCHGLib14.lha`: `Lake.mp`,
    /// `LightHouse.mp`, uncompressed PCHG; in `$SPECULAR_PCHG_CORPUS`, else
    /// `$CARGO_TARGET_DIR/corpus/pchg/arc`) with their PCHG Huffman-compressed by the test
    /// encoder: both must draw the same pixels. No Huffman-coded PCHG file is known.
    #[test]
    fn real_pchg_pictures_draw_the_same_huffman_coded() {
        let dir = std::env::var_os("SPECULAR_PCHG_CORPUS")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let t = std::env::var_os("CARGO_TARGET_DIR")?;
                Some(std::path::Path::new(&t).join("corpus/pchg/arc"))
            });
        let Some(dir) = dir.filter(|d| d.is_dir()) else {
            return;
        };
        for name in ["Lake.mp", "LightHouse.mp"] {
            let Ok(f) = std::fs::read(dir.join(name)) else {
                continue;
            };
            // Rebuild the FORM with the PCHG chunk compressed.
            let mut out = f[..12].to_vec();
            let mut p = 12;
            while p + 8 <= f.len() {
                let len = u32::from_be_bytes(f[p + 4..p + 8].try_into().unwrap()) as usize;
                let body = &f[p + 8..(p + 8 + len).min(f.len())];
                if &f[p..p + 4] == b"PCHG" {
                    assert_eq!(be16(body, 0), 0, "{name}");
                    out.extend(chunk(b"PCHG", &lines::huff_enc::compress_chunk(body)));
                } else {
                    out.extend(chunk(&f[p..p + 4].try_into().unwrap(), body));
                }
                p += 8 + len + (len & 1);
            }
            let n = (out.len() - 8) as u32;
            out[4..8].copy_from_slice(&n.to_be_bytes());
            let (a, b) = (decode(&f).unwrap(), decode(&out).unwrap());
            assert_eq!(info(&out).unwrap().line_palette, Some(LinePalette::Pchg));
            assert!(a.pixels == b.pixels, "{name}");
        }
    }

    #[test]
    fn decodes_planes() {
        let img = decode(&tiny()).unwrap();
        let px: Vec<[u8; 4]> = img
            .pixels
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();
        assert_eq!(
            px,
            [
                [0, 0, 0, 255],
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [0, 0, 0, 255],
                [255, 0, 0, 255]
            ]
        );
        let i = info(&tiny()).unwrap();
        assert_eq!(i.pixel_aspect, Some((10, 11)));
        assert!(!img.truncated);
    }

    #[test]
    fn truncated_and_garbage_never_panic() {
        let f = tiny();
        for n in 0..f.len() {
            let _ = info(&f[..n]);
            if let Ok(img) = decode(&f[..n]) {
                assert_eq!(img.pixels.len(), 3 * 2 * 4);
            }
        }
        // Flip every byte in turn.
        for i in 0..f.len() {
            let mut g = f.clone();
            g[i] ^= 0xa5;
            if let Ok(img) = decode(&g) {
                assert_eq!(
                    img.pixels.len(),
                    img.width as usize * img.height as usize * 4
                );
            }
        }
        // Pseudo-random garbage behind a valid header.
        let mut s = 0x1234_5678u32;
        for _ in 0..200 {
            let mut g = b"FORM\0\0\x01\0ILBM".to_vec();
            for _ in 0..256 {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                g.push(s as u8);
            }
            let _ = decode(&g);
        }
        assert_eq!(decode(b"FORM"), Err(Error::Truncated("header")));
        assert_eq!(decode(b"GIF89a......"), Err(Error::NotIff));
    }

    #[test]
    fn widen_replicates() {
        assert_eq!(widen(0xf, 4), 0xff);
        assert_eq!(widen(0x5, 4), 0x55);
        assert_eq!(widen(0x3f, 6), 0xff);
        assert_eq!(widen(1, 1), 0xff);
        assert_eq!(widen(0b10, 2), 0b1010_1010);
    }

    /// [`tiny`] with cycling chunks added before the body.
    fn tiny_with(extra: &[Vec<u8>]) -> Vec<u8> {
        let rows: [[u8; 4]; 2] = [
            [0b0100_0000, 0, 0b0010_0000, 0],
            [0b1010_0000, 0, 0b1000_0000, 0],
        ];
        let mut chunks = vec![
            chunk(b"BMHD", &bmhd(3, 2, 2, 0, 0)),
            chunk(b"CMAP", &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]),
        ];
        chunks.extend_from_slice(extra);
        chunks.push(chunk(b"BODY", &rows.concat()));
        form(b"ILBM", &chunks)
    }

    fn crng(rate: i16, flags: i16, lo: u8, hi: u8) -> Vec<u8> {
        let mut c = 0i16.to_be_bytes().to_vec();
        c.extend(rate.to_be_bytes());
        c.extend(flags.to_be_bytes());
        c.extend([lo, hi]);
        chunk(b"CRNG", &c)
    }

    #[test]
    fn crng_rotates_forwards_and_loops() {
        // Entries 1..=3 at 8192 (30 steps a second): three frames of 2 ticks (33 ms).
        let f = tiny_with(&[crng(8192, 1, 1, 3), crng(16384, 0, 0, 3)]);
        assert!(info(&f).unwrap().colour_cycling);
        let frames = cycling(&f).unwrap().unwrap();
        assert_eq!(frames.len(), 3);
        assert!(frames.iter().all(|fr| fr.delay_ms == 33), "{frames:?}");
        // Frame 0 is the file's palette; then red moves from entry 1 to 2, green to 3, blue
        // wraps to 1.
        assert_eq!(frames[0].cmap, [0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
        assert_eq!(frames[1].cmap, [0, 0, 0, 0, 0, 255, 255, 0, 0, 0, 255, 0]);
        let still = decode(&f).unwrap();
        assert_eq!(decode_with_cmap(&f, &frames[0].cmap).unwrap(), still);
        let one = decode_with_cmap(&f, &frames[1].cmap).unwrap();
        // Pixel 1 (value 1) is now blue.
        assert_eq!(&one.pixels[4..8], &[0, 0, 255, 255]);
        // Backwards: red moves down to entry 3 (wrapping).
        let back = cycling(&tiny_with(&[crng(8192, 3, 1, 3)]))
            .unwrap()
            .unwrap();
        assert_eq!(back[1].cmap, [0, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 0]);
        // Inactive or empty ranges: no animation.
        assert!(
            cycling(&tiny_with(&[crng(8192, 0, 1, 3)]))
                .unwrap()
                .is_none()
        );
        assert!(
            cycling(&tiny_with(&[crng(8192, 1, 2, 2)]))
                .unwrap()
                .is_none()
        );
        assert!(cycling(&tiny()).unwrap().is_none());
    }

    #[test]
    fn two_ranges_share_one_clock() {
        // Entries 0..=1 every tick, 2..=3 every other tick: 4 frames of 1 tick (16 ms).
        let f = tiny_with(&[crng(16384, 1, 0, 1), crng(8192, 1, 2, 3)]);
        let frames = cycling(&f).unwrap().unwrap();
        assert_eq!(frames.len(), 4, "{frames:?}");
        assert!(frames.iter().all(|fr| fr.delay_ms == 16));
    }

    #[test]
    fn ccrt_and_drng() {
        // CCRT: backwards, a step every 0.5 s.
        let mut c = (-1i16).to_be_bytes().to_vec();
        c.extend([1, 3]);
        c.extend(0u32.to_be_bytes());
        c.extend(500_000u32.to_be_bytes());
        c.extend([0, 0]);
        let frames = cycling(&tiny_with(&[chunk(b"CCRT", &c)])).unwrap().unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].delay_ms, 500);
        assert_eq!(frames[1].cmap, [0, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 0]);
        // DRNG over cells 0..=2: a true colour (white) on cell 0, registers 2 and 3 on cells
        // 1 and 2. Forwards, white reaches register 2 after one step.
        let mut d = vec![0, 2];
        d.extend(16384i16.to_be_bytes());
        d.extend(1i16.to_be_bytes());
        d.extend([1, 2]);
        d.extend([0, 255, 255, 255]);
        d.extend([1, 2, 2, 3]);
        let frames = cycling(&tiny_with(&[chunk(b"DRNG", &d)])).unwrap().unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(&frames[0].cmap[6..12], &[0, 255, 0, 0, 0, 255]);
        assert_eq!(&frames[1].cmap[6..12], &[255, 255, 255, 0, 255, 0]);
        // Hostile ranges never panic.
        for bad in [vec![], vec![0, 255], vec![9, 1, 0, 1, 0, 1, 200, 200]] {
            let _ = cycling(&tiny_with(&[chunk(b"DRNG", &bad)]));
            let _ = cycling(&tiny_with(&[chunk(b"CCRT", &bad)]));
            let _ = cycling(&tiny_with(&[chunk(b"CRNG", &bad)]));
        }
    }
}

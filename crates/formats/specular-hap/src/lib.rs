//! Vidvox HAP video frames (the HAP specification, `HapVideoDRAFT.md`, BSD licence), decoded
//! on the CPU to 8-bit RGBA.
//!
//! A frame is a tree of sections, each behind a 4-byte header (24-bit little-endian size and a
//! type byte) or an 8-byte one (three zero bytes, the type, then a 32-bit size). The single
//! top-level section's type names the texture format in its low nibble and the second-stage
//! compressor in its high nibble: `0xA_` none, `0xB_` Snappy, `0xC_` decode instructions
//! (chunks, each stored or Snappy-compressed, sizes and optional offsets in tables). Type
//! `0x0D` holds one or two images; two are Scaled YCoCg DXT5 plus a BC4 alpha (HAP Q Alpha).
//!
//! | Texture | Kind | fourcc |
//! |---|---|---|
//! | `_B` DXT1/BC1 RGB | HAP | `Hap1` |
//! | `_E` DXT5/BC3 RGBA | HAP Alpha | `Hap5` |
//! | `_F` Scaled YCoCg in DXT5 | HAP Q | `HapY` |
//! | `_F` + `_1` | HAP Q Alpha | `HapM` |
//! | `_1` RGTC1/BC4 | HAP Alpha-Only | `HapA` |
//! | `_C` BC7 | HAP R | `Hap7` |
//! | `_2`/`_3` BC6H unsigned/signed | HAP HDR | `HapH` |
//!
//! Textures cover the picture rounded up to whole 4×4 blocks; the output is cropped to the
//! picture size the container gives. Blocks decode through `specular-texture` (bit-exact BCn).
//! Scaled YCoCg follows van Waveren and Castaño, "Real-Time YCoCg-DXT Compression" (2007):
//! Co in red, Cg in green, the scale in blue, Y in alpha. A HAP Alpha-Only frame is a matte and
//! shows as grey; HAP HDR is clamped to [0, 1] and sRGB encoded as `specular-texture` does for
//! BC6H.

mod snappy;

use std::fmt;

use rayon::prelude::*;
use specular_texture::Format;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
    Unsupported(String),
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "malformed HAP frame: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::NeedsMemory(n) => write!(f, "HAP frame: {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The HAP variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Hap,
    HapAlpha,
    HapQ,
    HapQAlpha,
    HapAlphaOnly,
    HapR,
    HapHdr,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Hap => "HAP",
            Kind::HapAlpha => "HAP Alpha",
            Kind::HapQ => "HAP Q",
            Kind::HapQAlpha => "HAP Q Alpha",
            Kind::HapAlphaOnly => "HAP Alpha-Only",
            Kind::HapR => "HAP R",
            Kind::HapHdr => "HAP HDR",
        }
    }

    /// Whether frames carry transparency.
    pub fn has_alpha(self) -> bool {
        matches!(self, Kind::HapAlpha | Kind::HapQAlpha | Kind::HapR)
    }
}

/// The variant a container's four-character code names (the specification's list).
pub fn kind_of_fourcc(fourcc: &[u8; 4]) -> Option<Kind> {
    Some(match fourcc {
        b"Hap1" => Kind::Hap,
        b"Hap5" => Kind::HapAlpha,
        b"HapY" => Kind::HapQ,
        b"HapM" => Kind::HapQAlpha,
        b"HapA" => Kind::HapAlphaOnly,
        b"Hap7" => Kind::HapR,
        b"HapH" => Kind::HapHdr,
        _ => return None,
    })
}

/// A decoded frame: `width`×`height` RGBA8, rows top to bottom.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub kind: Kind,
}

/// The texture formats (the low nibble of an image section's type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tex {
    Rgb,
    Rgba,
    YCoCg,
    Bc7,
    Alpha,
    Bc6u,
    Bc6s,
}

impl Tex {
    fn of(t: u8) -> Option<Tex> {
        Some(match t & 0x0F {
            0x0B => Tex::Rgb,
            0x0E => Tex::Rgba,
            0x0F => Tex::YCoCg,
            0x0C => Tex::Bc7,
            0x01 => Tex::Alpha,
            0x02 => Tex::Bc6u,
            0x03 => Tex::Bc6s,
            _ => return None,
        })
    }

    fn format(self) -> Format {
        match self {
            Tex::Rgb => Format::Bc1 { alpha: false },
            Tex::Rgba | Tex::YCoCg => Format::Bc3,
            Tex::Bc7 => Format::Bc7,
            Tex::Alpha => Format::Bc4 { signed: false },
            Tex::Bc6u => Format::Bc6h { signed: false },
            Tex::Bc6s => Format::Bc6h { signed: true },
        }
    }

    fn kind(self) -> Kind {
        match self {
            Tex::Rgb => Kind::Hap,
            Tex::Rgba => Kind::HapAlpha,
            Tex::YCoCg => Kind::HapQ,
            Tex::Bc7 => Kind::HapR,
            Tex::Alpha => Kind::HapAlphaOnly,
            Tex::Bc6u | Tex::Bc6s => Kind::HapHdr,
        }
    }
}

/// One section: its type, its body and what follows it.
fn section(d: &[u8]) -> Result<(u8, &[u8], &[u8])> {
    if d.len() < 4 {
        return Err(bad("section header is truncated"));
    }
    let (size, head): (usize, usize) = if d[..3] == [0, 0, 0] {
        let b = d
            .get(4..8)
            .ok_or_else(|| bad("section header is truncated"))?;
        (u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize, 8)
    } else {
        (
            usize::from(d[0]) | usize::from(d[1]) << 8 | usize::from(d[2]) << 16,
            4,
        )
    };
    let body = d
        .get(head..head.saturating_add(size))
        .ok_or_else(|| bad("section is larger than the frame"))?;
    Ok((d[3], body, &d[head + size..]))
}

/// The sections at one level, in order.
fn sections(mut d: &[u8]) -> Result<Vec<(u8, &[u8])>> {
    let mut v = Vec::new();
    while !d.is_empty() {
        let (t, body, rest) = section(d)?;
        v.push((t, body));
        d = rest;
        if v.len() > 4096 {
            return Err(bad("too many sections"));
        }
    }
    Ok(v)
}

fn unknown_texture(t: u8) -> Error {
    Error::Unsupported(format!("HAP texture format 0x{t:02X}"))
}

/// The image sections of a frame: one, or the two of a multiple-image section.
fn images(frame: &[u8]) -> Result<Vec<(u8, &[u8])>> {
    let (t, body, _) = section(frame)?;
    if t != 0x0D {
        return Ok(vec![(t, body)]);
    }
    let inner: Vec<_> = sections(body)?
        .into_iter()
        .filter(|(t, _)| Tex::of(*t).is_some() && (0xA..=0xC).contains(&(t >> 4)))
        .collect();
    if inner.is_empty() || inner.len() > 2 {
        return Err(bad("multiple-image section without one or two images"));
    }
    Ok(inner)
}

/// The variant of a frame, from its sections.
pub fn probe(frame: &[u8]) -> Result<Kind> {
    let imgs = images(frame)?;
    let texs: Vec<Tex> = imgs
        .iter()
        .map(|&(t, _)| image_tex(t))
        .collect::<Result<_>>()?;
    match texs[..] {
        [t] => Ok(t.kind()),
        [Tex::YCoCg, Tex::Alpha] | [Tex::Alpha, Tex::YCoCg] => Ok(Kind::HapQAlpha),
        _ => Err(Error::Unsupported(
            "HAP frame combining these two texture formats".into(),
        )),
    }
}

fn image_tex(t: u8) -> Result<Tex> {
    let tex = Tex::of(t).ok_or_else(|| unknown_texture(t))?;
    if !(0xA..=0xC).contains(&(t >> 4)) {
        return Err(Error::Unsupported(format!(
            "HAP second-stage compressor 0x{:X}",
            t >> 4
        )));
    }
    Ok(tex)
}

/// One image section's texture blocks (`need` bytes), second stage undone.
fn texture(t: u8, body: &[u8], need: usize) -> Result<Vec<u8>> {
    let _held = specular_core::budget::reserve(need as u64)?;
    match t >> 4 {
        0xA => {
            let d = body
                .get(..need)
                .ok_or_else(|| bad("texture is shorter than the picture"))?;
            Ok(d.to_vec())
        }
        0xB => {
            let (len, _) = snappy::length(body).map_err(bad)?;
            if len < need || len > need.saturating_mul(2) {
                return Err(bad("texture has another size than the picture"));
            }
            let mut out = vec![0u8; len];
            snappy::decompress_into(body, &mut out).map_err(bad)?;
            out.truncate(need);
            Ok(out)
        }
        _ => chunked(body, need),
    }
}

/// Decode instructions: a container of chunk tables, then the frame data.
fn chunked(body: &[u8], need: usize) -> Result<Vec<u8>> {
    let (t, instr, data) = section(body)?;
    if t != 0x01 {
        return Err(bad("decode instructions are missing"));
    }
    let (mut comp, mut sizes, mut offsets) = (None, None, None);
    for (t, b) in sections(instr)? {
        match t {
            0x02 => comp = Some(b),
            0x03 => sizes = Some(b),
            0x04 => offsets = Some(b),
            _ => {}
        }
    }
    let le32 = |b: &[u8]| -> Vec<usize> {
        b.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize)
            .collect()
    };
    let comp = comp.ok_or_else(|| bad("chunk compressor table is missing"))?;
    let sizes = le32(sizes.ok_or_else(|| bad("chunk size table is missing"))?);
    let n = comp.len();
    if n == 0 || sizes.len() != n {
        return Err(bad("chunk tables disagree"));
    }
    let offsets = match offsets {
        Some(o) => {
            let o = le32(o);
            if o.len() != n {
                return Err(bad("chunk tables disagree"));
            }
            o
        }
        None => sizes
            .iter()
            .scan(0usize, |a, &s| {
                let o = *a;
                *a = a.saturating_add(s);
                Some(o)
            })
            .collect(),
    };
    // Each chunk's compressed bytes and decompressed length.
    let mut parts = Vec::with_capacity(n);
    let mut total = 0usize;
    for i in 0..n {
        let src = data
            .get(offsets[i]..offsets[i].saturating_add(sizes[i]))
            .ok_or_else(|| bad("a chunk lies outside the frame"))?;
        let len = match comp[i] {
            0x0A => src.len(),
            0x0B => snappy::length(src).map_err(bad)?.0,
            c => {
                return Err(Error::Unsupported(format!(
                    "HAP chunk compressor 0x{c:02X}"
                )));
            }
        };
        total = total.saturating_add(len);
        if total > need.saturating_mul(2) {
            return Err(bad("chunks are larger than the picture"));
        }
        parts.push((comp[i], src, len));
    }
    if total < need {
        return Err(bad("chunks are shorter than the picture"));
    }
    let _held = specular_core::budget::reserve(total as u64)?;
    let mut out = vec![0u8; total];
    let mut slots = Vec::with_capacity(n);
    let mut rest = out.as_mut_slice();
    for &(c, src, len) in &parts {
        let (a, b) = rest.split_at_mut(len);
        slots.push((c, src, a));
        rest = b;
    }
    slots
        .into_par_iter()
        .try_for_each(|(c, src, dst)| match c {
            0x0B => snappy::decompress_into(src, dst),
            _ => {
                dst.copy_from_slice(src);
                Ok(())
            }
        })
        .map_err(bad)?;
    out.truncate(need);
    Ok(out)
}

fn blocks(tex: Tex, data: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    specular_texture::decode_blocks(tex.format(), data, w, h).map_err(|e| match e {
        specular_texture::Error::NeedsMemory(n) => Error::NeedsMemory(n),
        e => bad(e.to_string()),
    })
}

/// Scaled YCoCg (DXT5 channels R = Co, G = Cg, B = scale, A = Y) to RGB in place; `alpha`
/// gives the alpha channel (else opaque).
fn ycocg_to_rgb(px: &mut [u8], alpha: Option<&[u8]>) {
    let conv = |p: &mut [u8], a: u8| {
        let scale = i32::from(p[2] >> 3) + 1;
        let co = i32::from(p[0]) - 128;
        let cg = i32::from(p[1]) - 128;
        let y = i32::from(p[3]);
        // Co and Cg are divided by the scale, rounding toward zero (the encoder multiplied).
        let (co, cg) = (co / scale, cg / scale);
        let c = |v: i32| v.clamp(0, 255) as u8;
        p.copy_from_slice(&[c(y + co - cg), c(y + cg), c(y - co - cg), a]);
    };
    match alpha {
        Some(a) => px
            .par_chunks_mut(4 * 4096)
            .zip(a.par_chunks(4 * 4096))
            .for_each(|(p, a)| {
                for (p, a) in p.chunks_exact_mut(4).zip(a.chunks_exact(4)) {
                    conv(p, a[0]);
                }
            }),
        None => px.par_chunks_mut(4 * 4096).for_each(|p| {
            for p in p.chunks_exact_mut(4) {
                conv(p, 255);
            }
        }),
    }
}

/// Decodes one frame to `width`×`height` RGBA8.
pub fn decode(frame: &[u8], width: u32, height: u32) -> Result<Image> {
    if width == 0 || height == 0 || width > 1 << 16 || height > 1 << 16 {
        return Err(bad(format!("picture size {width}×{height}")));
    }
    let kind = probe(frame)?;
    let bytes_per_block = |tex: Tex| tex.format().block().2;
    let nblocks = (width as usize).div_ceil(4) * (height as usize).div_ceil(4);
    let mut colour = None;
    let mut alpha = None;
    for (t, body) in images(frame)? {
        let tex = image_tex(t)?;
        let data = texture(t, body, nblocks * bytes_per_block(tex))?;
        let px = blocks(tex, &data, width, height)?;
        if kind == Kind::HapQAlpha && tex == Tex::Alpha {
            alpha = Some(px);
        } else {
            colour = Some((tex, px));
        }
    }
    let (tex, mut rgba) = colour.ok_or_else(|| bad("no colour image"))?;
    if tex == Tex::YCoCg {
        ycocg_to_rgb(&mut rgba, alpha.as_deref());
    }
    Ok(Image {
        width,
        height,
        rgba,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(t: u8, len: usize) -> Vec<u8> {
        if len < 1 << 24 {
            vec![len as u8, (len >> 8) as u8, (len >> 16) as u8, t]
        } else {
            let mut v = vec![0, 0, 0, t];
            v.extend_from_slice(&(len as u32).to_le_bytes());
            v
        }
    }

    fn sect(t: u8, body: &[u8]) -> Vec<u8> {
        let mut v = head(t, body.len());
        v.extend_from_slice(body);
        v
    }

    /// One solid red BC1 block.
    const RED: [u8; 8] = [0x00, 0xF8, 0, 0, 0, 0, 0, 0];

    #[test]
    fn plain_and_extended_headers() {
        let f = sect(0xAB, &RED);
        let img = decode(&f, 3, 2).unwrap();
        assert_eq!(img.kind, Kind::Hap);
        assert_eq!(img.rgba.len(), 3 * 2 * 4);
        assert_eq!(&img.rgba[..4], &[255, 0, 0, 255]);
        let mut ext = vec![0, 0, 0, 0xAB, 8, 0, 0, 0];
        ext.extend_from_slice(&RED);
        assert_eq!(decode(&ext, 4, 4).unwrap().rgba[..4], [255, 0, 0, 255]);
    }

    #[test]
    fn chunked_with_and_without_offsets() {
        // Two blocks (8×4), one stored chunk each; the second listed first by offset.
        let blue = [0x1F, 0x00, 0, 0, 0, 0, 0, 0];
        let data: Vec<u8> = blue.iter().chain(RED.iter()).copied().collect();
        let mut instr = sect(0x02, &[0x0A, 0x0A]);
        instr.extend(sect(0x03, &[8, 0, 0, 0, 8, 0, 0, 0]));
        instr.extend(sect(0x04, &[8, 0, 0, 0, 0, 0, 0, 0]));
        let mut body = sect(0x01, &instr);
        body.extend_from_slice(&data);
        let img = decode(&sect(0xCB, &body), 8, 4).unwrap();
        assert_eq!(&img.rgba[..4], &[255, 0, 0, 255]);
        assert_eq!(&img.rgba[16..20], &[0, 0, 255, 255]);
        // Without offsets the chunks follow each other.
        let mut instr = sect(0x02, &[0x0A, 0x0A]);
        instr.extend(sect(0x03, &[8, 0, 0, 0, 8, 0, 0, 0]));
        let mut body = sect(0x01, &instr);
        body.extend_from_slice(&data);
        let img = decode(&sect(0xCB, &body), 8, 4).unwrap();
        assert_eq!(&img.rgba[..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn kinds_and_refusals() {
        assert_eq!(kind_of_fourcc(b"HapM"), Some(Kind::HapQAlpha));
        assert_eq!(kind_of_fourcc(b"avc1"), None);
        let q = sect(0xAF, &[0; 16]);
        let a = sect(0xA1, &[0; 8]);
        let multi = sect(0x0D, &[q.clone(), a].concat());
        assert_eq!(probe(&multi).unwrap(), Kind::HapQAlpha);
        assert_eq!(probe(&sect(0x0D, &q)).unwrap(), Kind::HapQ);
        assert!(matches!(
            probe(&sect(0xA7, &[0; 8])),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            probe(&sect(0xDB, &[0; 8])),
            Err(Error::Unsupported(_))
        ));
        let mut instr = sect(0x02, &[0x0C]);
        instr.extend(sect(0x03, &[8, 0, 0, 0]));
        let mut body = sect(0x01, &instr);
        body.extend_from_slice(&RED);
        assert!(matches!(
            decode(&sect(0xCB, &body), 4, 4),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn ycocg_grey_and_colour() {
        // Co = Cg = 0 (128), scale 1: grey Y.
        let mut px = [128, 128, 0, 77];
        ycocg_to_rgb(&mut px, None);
        assert_eq!(px, [77, 77, 77, 255]);
        // Scale 2 (blue 8): Co 40 → 20, Cg -20 → -10.
        let mut px = [168, 108, 8, 100];
        ycocg_to_rgb(&mut px, Some(&[9, 9, 9, 255]));
        assert_eq!(px, [130, 90, 90, 9]);
    }

    #[test]
    fn hostile_frames_do_not_panic() {
        let good = sect(0xAB, &RED);
        for n in 0..good.len() {
            let _ = decode(&good[..n], 4, 4);
        }
        // Absurd sizes: a section claiming 16 MiB, an extended one 4 GiB.
        let _ = decode(&[0xFF, 0xFF, 0xFF, 0xAB], 4, 4);
        let _ = decode(&[0, 0, 0, 0xAB, 0xFF, 0xFF, 0xFF, 0xFF], 4, 4);
        // Chunk table pointing outside the data, sizes overflowing.
        let mut instr = sect(0x02, &[0x0A, 0x0B]);
        instr.extend(sect(0x03, &[0xFF, 0xFF, 0xFF, 0xFF, 8, 0, 0, 0]));
        instr.extend(sect(0x04, &[0xF0, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]));
        let mut body = sect(0x01, &instr);
        body.extend_from_slice(&RED);
        assert!(decode(&sect(0xCB, &body), 4, 4).is_err());
        // Snappy claiming a huge texture.
        assert!(decode(&sect(0xBB, &[0xFF, 0xFF, 0xFF, 0x7F]), 4, 4).is_err());
        // Picture larger than the texture.
        assert!(decode(&good, 4096, 4096).is_err());
        assert!(decode(&good, 0, 4).is_err());
        // Empty multi-image.
        assert!(decode(&sect(0x0D, &[]), 4, 4).is_err());
    }
}

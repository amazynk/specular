//! Motion JPEG (QuickTime `jpeg`/`mjpa`/`mjpb`/`AVDJ`, AVI `MJPG`/`AVRN`, Matroska `V_MJPEG`)
//! through the in-tree JPEG decoder `specular-jpeg`: every sample one picture, as YCbCr
//! planes for [`crate::play`].
//!
//! - Samples without Huffman tables (AVI1 and most capture cards) get the standard tables of
//!   ITU T.81 Annex K.3, as the AVI1 convention says.
//! - Interlaced: a sample holding two JPEG images (one per field, `mjpa` and AVI1 capture)
//!   is woven into one frame. The first field is the top one unless an Avid comment or the
//!   container says bottom first (QuickTime `fiel`; AVI, which can't say, means bottom
//!   first, as FFmpeg reads it).
//! - Motion JPEG format B (`mjpb`): the marker-less fields are rebuilt into JPEG images from
//!   the offsets of their header (Apple's QuickTime documentation of format B), with the
//!   entropy data byte-stuffed again.
//! - 4:1:1 and 4:4:0 sampling stay as coded (the viewer interpolates the chroma; they were
//!   widened by repetition before fidelity round 8); grey pictures get mid-grey chroma.
//! - RGB pictures (Adobe transform 0) keep their samples: G, B, R planes under the identity
//!   matrix (they were converted to YCbCr, rounding them).

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};
use specular_jpeg as jpeg;

/// One track's decoder: the field order the container says.
#[derive(Default)]
pub struct MjpegDec {
    bottom_first: bool,
}

/// ITU T.81 Table K.3: luminance DC code lengths, then the symbols.
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
/// Table K.4: chrominance DC.
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
/// Table K.5: luminance AC.
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];
/// Table K.6: chrominance AC.
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
    0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
    0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

/// The DHT segment (marker included) of the four Annex K tables.
fn default_dht() -> Vec<u8> {
    let tables: [(u8, &[u8; 16], &[u8]); 4] = [
        (0x00, &DC_LUMA_BITS, &DC_VALS),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALS),
        (0x01, &DC_CHROMA_BITS, &DC_VALS),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS),
    ];
    let mut body = Vec::new();
    for (class, bits, vals) in tables {
        body.push(class);
        body.extend_from_slice(bits);
        body.extend_from_slice(vals);
    }
    let mut out = vec![0xFF, 0xC4];
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn be32(d: &[u8], o: usize) -> Option<usize> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize)
}

fn be16(d: &[u8], o: usize) -> Option<usize> {
    Some(usize::from(u16::from_be_bytes(
        d.get(o..o + 2)?.try_into().ok()?,
    )))
}

/// One JPEG image in a sample: where it ends, whether it carries Huffman tables, and the
/// field order of Avid's `AVID` comment if it says (1 NTSC: bottom first, 2 PAL: top first).
struct Image {
    end: usize,
    has_dht: bool,
    sos: usize,
    polarity: Option<u8>,
}

/// Walks the JPEG image starting with SOI at `at`: its marker segments and entropy data, to
/// its EOI (or the end of the data).
fn image(d: &[u8], at: usize) -> Option<Image> {
    if d.get(at..at + 2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut p = at + 2;
    let mut has_dht = false;
    let mut sos = None;
    let mut polarity = None;
    loop {
        // Fill bytes before a marker.
        while d.get(p) == Some(&0xFF) && d.get(p + 1) == Some(&0xFF) {
            p += 1;
        }
        let Some(&[0xFF, m]) = d.get(p..p + 2) else {
            return Some(Image {
                end: d.len(),
                has_dht,
                sos: sos?,
                polarity,
            });
        };
        match m {
            0xD9 => {
                return Some(Image {
                    end: p + 2,
                    has_dht,
                    sos: sos?,
                    polarity,
                });
            }
            0xD0..=0xD7 | 0x01 => p += 2,
            _ => {
                let len = be16(d, p + 2)?;
                if len < 2 {
                    return None;
                }
                let body = d.get(p + 4..p + 2 + len)?;
                if m == 0xC4 {
                    has_dht = true;
                }
                // AVI1's own polarity byte is left alone: writers disagree on what it means.
                if m == 0xFE && body.starts_with(b"AVID") {
                    polarity = body.get(12).copied().filter(|p| matches!(p, 1 | 2));
                }
                let next = p + 2 + len;
                if m == 0xDA {
                    sos.get_or_insert(p);
                    // Entropy data up to the next marker that isn't stuffing or a restart.
                    let mut q = next;
                    while q + 1 < d.len() {
                        if d[q] == 0xFF && d[q + 1] != 0 && !(0xD0..=0xD7).contains(&d[q + 1]) {
                            break;
                        }
                        q += 1;
                    }
                    if q + 1 >= d.len() {
                        return Some(Image {
                            end: d.len(),
                            has_dht,
                            sos: sos?,
                            polarity,
                        });
                    }
                    p = q;
                } else {
                    p = next;
                }
            }
        }
    }
}

/// The first SOI at or after `from`.
fn find_soi(d: &[u8], from: usize) -> Option<usize> {
    d.get(from..)?
        .windows(3)
        .position(|w| w[0] == 0xFF && w[1] == 0xD8 && w[2] == 0xFF)
        .map(|i| from + i)
}

/// Motion JPEG format B: a field starts with four unused bytes and the tag `mjpg` (no SOI).
fn is_format_b(d: &[u8]) -> bool {
    d.len() >= 40 && d[4..8] == *b"mjpg" && d[..2] != [0xFF, 0xD8]
}

/// Rebuilds the format B field at `at` as a JPEG image; also the offset of the next field
/// (0: none).
fn format_b_field(d: &[u8], at: usize) -> Option<(Vec<u8>, usize)> {
    let f = d.get(at..)?;
    if f.get(4..8)? != b"mjpg" {
        return None;
    }
    let field_size = be32(f, 8)?;
    let next = be32(f, 16)?;
    let (dqt, dht, sof, sos, sod) = (
        be32(f, 20)?,
        be32(f, 24)?,
        be32(f, 28)?,
        be32(f, 32)?,
        be32(f, 36)?,
    );
    let end = if field_size > sod && field_size <= f.len() {
        field_size
    } else {
        f.len()
    };
    if sof == 0 || sos == 0 || sod == 0 || sod > end {
        return None;
    }
    let mut out = vec![0xFF, 0xD8];
    // Each offset holds a marker segment without its marker: length first.
    let segment = |out: &mut Vec<u8>, marker: u8, off: usize| -> Option<()> {
        let len = be16(f, off)?;
        out.extend_from_slice(&[0xFF, marker]);
        out.extend_from_slice(f.get(off..off + len)?);
        Some(())
    };
    if dqt != 0 {
        segment(&mut out, 0xDB, dqt)?;
    }
    if dht != 0 {
        segment(&mut out, 0xC4, dht)?;
    } else {
        out.extend_from_slice(&default_dht());
    }
    segment(&mut out, 0xC0, sof)?;
    segment(&mut out, 0xDA, sos)?;
    for &b in &f[sod..end] {
        out.push(b);
        if b == 0xFF {
            out.push(0);
        }
    }
    out.extend_from_slice(&[0xFF, 0xD9]);
    let next = if next > 0 && at + next < d.len() {
        at + next
    } else {
        0
    };
    Some((out, next))
}

/// The JPEG images (fields, or one frame) of a sample, each ready for the decoder, and
/// whether an Avid comment says the bottom field comes first.
fn images(d: &[u8]) -> Option<(Vec<Vec<u8>>, Option<bool>)> {
    if is_format_b(d) {
        let (first, next) = format_b_field(d, 0)?;
        let mut v = vec![first];
        if next != 0
            && let Some((second, _)) = format_b_field(d, next)
        {
            v.push(second);
        }
        return Some((v, None));
    }
    let mut out = Vec::new();
    let mut polarity = None;
    let mut at = find_soi(d, 0)?;
    while out.len() < 2 {
        let Some(img) = image(d, at) else {
            break;
        };
        polarity = polarity.or(img.polarity);
        let bytes = &d[at..img.end];
        out.push(if img.has_dht {
            bytes.to_vec()
        } else {
            // The tables go before the first scan.
            let k = img.sos - at;
            let mut v = Vec::with_capacity(bytes.len() + 432);
            v.extend_from_slice(&bytes[..k]);
            v.extend_from_slice(&default_dht());
            v.extend_from_slice(&bytes[k..]);
            v
        });
        match find_soi(d, img.end) {
            Some(n) => at = n,
            None => break,
        }
    }
    if out.is_empty() {
        return None;
    }
    Some((out, polarity.map(|p| p == 1)))
}

/// Tightly packed 8-bit planes of a decoded JPEG and their layout.
struct Planes {
    width: u32,
    height: u32,
    chroma: Chroma,
    /// G, B, R planes (an RGB picture), not Y, Cb, Cr.
    rgb: bool,
    planes: [Vec<u8>; 3],
}

fn tight(p: &jpeg::Plane) -> Vec<u8> {
    let w = p.width as usize;
    let mut v = Vec::with_capacity(w * p.height as usize);
    for row in p.data.chunks(p.stride).take(p.height as usize) {
        v.extend_from_slice(&row[..w.min(row.len())]);
    }
    v
}

fn decode_one(bytes: &[u8]) -> Result<Planes, DecodeError> {
    let info = jpeg::read_info(bytes).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    if info.precision != 8 {
        return Err(DecodeError::Unsupported(format!(
            "{}-bit Motion JPEG",
            info.precision
        )));
    }
    if matches!(info.color, jpeg::ColorModel::Cmyk | jpeg::ColorModel::Ycck) {
        return Err(DecodeError::Unsupported("CMYK Motion JPEG".into()));
    }
    // The decoded planes and their tight copies.
    let _res = specular_core::budget::reserve(u64::from(info.width) * u64::from(info.height) * 6)
        .map_err(DecodeError::NeedsMemory)?;
    let opts = jpeg::DecodeOptions::default();
    let d = jpeg::decode(bytes, &opts).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    let (w, h) = (d.width, d.height);
    let (cw2, ch2) = (w.div_ceil(2) as usize, h.div_ceil(2) as usize);
    match d.layout {
        jpeg::PlanesLayout::Gray => {
            let y = tight(&d.planes[0]);
            Ok(Planes {
                width: w,
                height: h,
                chroma: Chroma::Yuv420,
                rgb: false,
                planes: [y, vec![128; cw2 * ch2], vec![128; cw2 * ch2]],
            })
        }
        jpeg::PlanesLayout::YCbCr if d.planes.len() == 3 => {
            let y = &d.planes[0];
            let (cb, cr) = (&d.planes[1], &d.planes[2]);
            if cb.h_samp != cr.h_samp || cb.v_samp != cr.v_samp {
                return Err(DecodeError::Unsupported(
                    "Motion JPEG with unequal chroma sampling".into(),
                ));
            }
            let (hs, vs) = (y.h_samp / cb.h_samp.max(1), y.v_samp / cb.v_samp.max(1));
            let chroma = match (hs, vs) {
                (2, 2) => Chroma::Yuv420,
                (2, 1) => Chroma::Yuv422,
                (1, 1) => Chroma::Yuv444,
                (4, 1) => Chroma::Yuv411,
                (1, 2) => Chroma::Yuv440,
                _ => {
                    return Err(DecodeError::Unsupported(format!(
                        "Motion JPEG with {hs}:{vs} chroma sampling"
                    )));
                }
            };
            let (cw, chh) = chroma.plane_size(w, h);
            if (cb.width, cb.height) != (cw, chh) {
                return Err(DecodeError::Malformed(format!(
                    "Motion JPEG chroma {}x{} for a {w}x{h} picture",
                    cb.width, cb.height
                )));
            }
            Ok(Planes {
                width: w,
                height: h,
                chroma,
                rgb: false,
                planes: [tight(y), tight(cb), tight(cr)],
            })
        }
        jpeg::PlanesLayout::Rgb if d.planes.len() == 3 => {
            let [r, g, b] = [0, 1, 2].map(|i| tight(&d.planes[i]));
            let n = (w * h) as usize;
            if r.len() < n || g.len() < n || b.len() < n {
                return Err(DecodeError::Unsupported(
                    "subsampled RGB Motion JPEG".into(),
                ));
            }
            // G, B, R as the identity matrix (H.273 MC 0) takes them.
            Ok(Planes {
                width: w,
                height: h,
                chroma: Chroma::Yuv444,
                rgb: true,
                planes: [g, b, r],
            })
        }
        _ => Err(DecodeError::Unsupported(
            "Motion JPEG in this colour model".into(),
        )),
    }
}

/// Weaves two fields (same layout) into a frame: `first` on the top lines unless
/// `bottom_first`.
fn weave(first: Planes, second: Planes, bottom_first: bool) -> Option<Planes> {
    if first.width != second.width
        || first.height != second.height
        || first.chroma != second.chroma
        || first.rgb != second.rgb
    {
        return None;
    }
    let (top, bottom) = if bottom_first {
        (second, first)
    } else {
        (first, second)
    };
    let (w, fh) = (top.width as usize, top.height as usize);
    let cw = top.chroma.plane_size(top.width, top.height).0 as usize;
    let mut planes: [Vec<u8>; 3] = Default::default();
    for (i, out) in planes.iter_mut().enumerate() {
        let pw = if i == 0 { w } else { cw };
        let (t, b) = (&top.planes[i], &bottom.planes[i]);
        let rows = t.len() / pw.max(1);
        out.reserve(2 * rows * pw);
        for y in 0..rows {
            out.extend_from_slice(&t[y * pw..(y + 1) * pw]);
            out.extend_from_slice(b.get(y * pw..(y + 1) * pw)?);
        }
    }
    Some(Planes {
        width: top.width,
        height: (2 * fh) as u32,
        chroma: top.chroma,
        rgb: top.rgb,
        planes,
    })
}

impl MjpegDec {
    /// A decoder for a track whose container says the bottom field comes first (or not).
    pub fn new(bottom_first: bool) -> MjpegDec {
        MjpegDec { bottom_first }
    }

    /// Decodes one sample. A damaged sample costs its picture (`Ok(None)`); a coding the
    /// player can't show is an error.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        let Some((imgs, avid_bottom)) = images(data) else {
            specular_log::debug!(target: specular_log::DECODE, "video: Motion JPEG sample without a picture");
            return Ok(None);
        };
        let mut pics = Vec::with_capacity(2);
        for img in &imgs {
            match decode_one(img) {
                Ok(p) => pics.push(p),
                Err(e @ (DecodeError::Unsupported(_) | DecodeError::NeedsMemory(_))) => {
                    return Err(e);
                }
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "video: Motion JPEG field lost: {e}");
                }
            }
        }
        let limited = imgs.first().is_some_and(|i| itu601(i));
        let bottom_first = avid_bottom.unwrap_or(self.bottom_first);
        let p = match (pics.len(), imgs.len()) {
            (2, _) => {
                let second = pics.pop();
                let first = pics.pop();
                match (first, second) {
                    (Some(a), Some(b)) => weave(a, b, bottom_first),
                    _ => None,
                }
            }
            (1, 1) => pics.pop(),
            // One field of two lost: the frame is lost.
            _ => None,
        };
        Ok(p.map(|p| VideoFrame {
            width: p.width,
            height: p.height,
            bit_depth: 8,
            chroma: p.chroma,
            planes: p.planes,
            // JFIF: BT.601, full range; limited when the encoder says so in a comment. RGB:
            // the identity matrix, full range, sRGB.
            signal: Some(if p.rgb {
                Signal {
                    full_range: true,
                    primaries: Some(1),
                    transfer: Some(13),
                    matrix: Some(0),
                }
            } else {
                Signal {
                    full_range: !limited,
                    primaries: None,
                    transfer: None,
                    matrix: Some(6),
                }
            }),
        }))
    }
}

/// The image's header has a `CS=ITU601` comment: its samples are limited range (16–235),
/// as FFmpeg's and capture cards' Motion JPEG of TV-range video says (libavcodec reads it
/// the same way). Searched up to the first scan.
fn itu601(img: &[u8]) -> bool {
    let mut i = 2;
    while let Some(&[0xFF, m, hi, lo]) = img.get(i..i + 4) {
        let len = usize::from(u16::from_be_bytes([hi, lo]));
        match m {
            0xFE => {
                if img
                    .get(i + 4..i + 2 + len)
                    .is_some_and(|c| c.starts_with(b"CS=ITU601"))
                {
                    return true;
                }
            }
            0xDA | 0xD9 => return false,
            _ => {}
        }
        i += 2 + len;
    }
    false
}

/// Whether a sample holds two fields (two JPEG images, or two format B fields).
pub fn two_fields(data: &[u8]) -> bool {
    images(data).is_some_and(|(v, _)| v.len() == 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fidelity round 7: FFmpeg's limited-range Motion JPEG (`CS=ITU601` comment) was shown
    /// as full range, 20 of 255 too dark and too contrasty.
    #[test]
    fn itu601_comment_means_limited_range() {
        let head = |com: &[u8]| {
            let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xFE];
            v.extend_from_slice(&(com.len() as u16 + 2).to_be_bytes());
            v.extend_from_slice(com);
            v.extend_from_slice(&[0xFF, 0xDA, 0, 2]);
            v
        };
        assert!(itu601(&head(b"CS=ITU601\0")));
        assert!(!itu601(&head(b"Lavc61")));
        // After the scan it is image data, not a header.
        let mut late = vec![0xFF, 0xD8, 0xFF, 0xDA, 0, 2, 0xFF, 0xFE, 0, 11];
        late.extend_from_slice(b"CS=ITU601");
        assert!(!itu601(&late));
    }

    #[test]
    fn annex_k_tables_have_their_counts() {
        let n = |b: &[u8; 16]| b.iter().map(|&x| usize::from(x)).sum::<usize>();
        assert_eq!(n(&DC_LUMA_BITS), 12);
        assert_eq!(n(&DC_CHROMA_BITS), 12);
        assert_eq!(n(&AC_LUMA_BITS), 162);
        assert_eq!(n(&AC_CHROMA_BITS), 162);
        assert_eq!(default_dht().len(), 4 + 2 * (17 + 12) + 2 * (17 + 162));
    }

    #[test]
    fn damaged_samples_cost_their_picture() {
        let mut d = MjpegDec::default();
        assert!(matches!(d.decode(b"not a jpeg"), Ok(None)));
        assert!(matches!(
            d.decode(&[0xFF, 0xD8, 0xFF, 0xDA, 0, 2]),
            Ok(None)
        ));
        let mut b = vec![0u8; 64];
        b[4..8].copy_from_slice(b"mjpg");
        assert!(matches!(d.decode(&b), Ok(None)));
    }
}

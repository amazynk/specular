//! JPEG: the lossless DCT-domain rotation (specular-jpeg's transform), or the Exif
//! Orientation tag when that can't be done without losing anything.

use specular_jpeg::{Alignment, Transform, TransformOptions};

use super::{TagReason, tiff};

const EXIF_ID: &[u8] = b"Exif\0\0";
const XMP_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// One marker segment before the first scan: marker byte, offset of the marker, total length
/// including the marker and length bytes.
#[derive(Clone, Copy, Debug)]
struct Seg {
    marker: u8,
    at: usize,
    len: usize,
}

impl Seg {
    fn payload<'a>(&self, d: &'a [u8]) -> &'a [u8] {
        &d[self.at + 4..self.at + self.len]
    }
}

/// The segments from SOI up to (not including) the first SOS.
fn segments(d: &[u8]) -> Option<Vec<Seg>> {
    if d.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 2;
    loop {
        while d.get(i)? == &0xFF && d.get(i + 1)? == &0xFF {
            i += 1;
        }
        if d.get(i)? != &0xFF {
            return None;
        }
        let m = *d.get(i + 1)?;
        if m == 0xDA || m == 0xD9 {
            return Some(out);
        }
        let len = u16::from_be_bytes([*d.get(i + 2)?, *d.get(i + 3)?]) as usize;
        if len < 2 || i + 2 + len > d.len() {
            return None;
        }
        out.push(Seg {
            marker: m,
            at: i,
            len: len + 2,
        });
        i += 2 + len;
    }
}

fn exif_seg(d: &[u8], segs: &[Seg]) -> Option<Seg> {
    segs.iter()
        .find(|s| s.marker == 0xE1 && s.payload(d).starts_with(EXIF_ID))
        .copied()
}

/// The Orientation in the file's Exif (1 when there is none).
pub fn orientation(d: &[u8]) -> u8 {
    segments(d)
        .and_then(|s| exif_seg(d, &s))
        .and_then(|s| tiff::orientation(&s.payload(d)[6..]))
        .unwrap_or(1)
}

/// Where the primary image ends (just after its EOI), walking the scans.
fn primary_end(d: &[u8]) -> Option<usize> {
    let mut i = 2;
    loop {
        while d.get(i)? == &0xFF && d.get(i + 1)? == &0xFF {
            i += 1;
        }
        if d.get(i)? != &0xFF {
            return None;
        }
        let m = *d.get(i + 1)?;
        match m {
            0xD9 => return Some(i + 2),
            0xD0..=0xD7 | 0x01 => i += 2,
            _ => {
                let len = u16::from_be_bytes([*d.get(i + 2)?, *d.get(i + 3)?]) as usize;
                i += 2 + len;
                if m == 0xDA {
                    // Entropy-coded data: up to the next marker that isn't a stuffed zero or a
                    // restart.
                    loop {
                        let p = memchr_ff(d, i)?;
                        let n = *d.get(p + 1)?;
                        if n == 0x00 || (0xD0..=0xD7).contains(&n) || n == 0xFF {
                            i = p + if n == 0xFF { 1 } else { 2 };
                            continue;
                        }
                        i = p;
                        break;
                    }
                }
            }
        }
    }
}

fn memchr_ff(d: &[u8], from: usize) -> Option<usize> {
    d.get(from..)?
        .iter()
        .position(|&b| b == 0xFF)
        .map(|p| from + p)
}

/// Why the lossless route can't be taken for this file (so the tag is written instead), or
/// `None` when it can (`trim` being the user's consent to drop a partial edge block).
pub fn lossless_blocker(d: &[u8], t: Transform, trim: bool) -> Option<TagReason> {
    let segs = match segments(d) {
        Some(s) => s,
        None => return Some(TagReason::Unreadable),
    };
    if segs
        .iter()
        .any(|s| s.marker == 0xE2 && s.payload(d).starts_with(b"MPF\0"))
    {
        return Some(TagReason::MultiPicture);
    }
    if let Some(x) = segs
        .iter()
        .find(|s| s.marker == 0xE1 && s.payload(d).starts_with(XMP_ID))
    {
        let p = x.payload(d);
        if contains(p, b"Container:Directory") || contains(p, b"MicroVideo") {
            return Some(TagReason::MultiPicture);
        }
    }
    match primary_end(d) {
        Some(end) if d[end..].iter().all(|&b| b == 0) => {}
        Some(_) => return Some(TagReason::Trailer),
        None => return Some(TagReason::Unreadable),
    }
    match specular_jpeg::transform_alignment(d, t) {
        Ok(Alignment::Perfect) => None,
        Ok(Alignment::NeedsTrim { width, height }) if !trim => {
            Some(TagReason::NotAligned { width, height })
        }
        Ok(Alignment::NeedsTrim { .. }) => None,
        Ok(Alignment::Impossible) | Err(_) => Some(TagReason::Unreadable),
    }
}

/// What a quarter-turn lossless rotation would trim: the stored size after trimming,
/// or `None` when nothing is trimmed.
pub fn trimmed_size(d: &[u8], t: Transform) -> Option<(u32, u32)> {
    match specular_jpeg::transform_alignment(d, t) {
        Ok(Alignment::NeedsTrim { width, height }) => Some((width, height)),
        _ => None,
    }
}

fn contains(h: &[u8], n: &[u8]) -> bool {
    h.windows(n.len()).any(|w| w == n)
}

/// Rewrite the Exif TIFF of `d` with `f`, rebuilding its APP1 (or inserting one, starting
/// from [`tiff::minimal`], when the file has none and `create` is set). `None` when the
/// segment would be too long or the TIFF doesn't parse.
fn edit_exif(
    d: &[u8],
    create: bool,
    f: impl FnOnce(&mut Vec<u8>) -> Option<()>,
) -> Option<Vec<u8>> {
    let segs = segments(d)?;
    let (mut tiff_bytes, range) = match exif_seg(d, &segs) {
        Some(s) => (s.payload(d)[6..].to_vec(), Some((s.at, s.at + s.len))),
        None if create => (tiff::minimal(1), None),
        None => return Some(d.to_vec()),
    };
    f(&mut tiff_bytes)?;
    let seg_len = 2 + EXIF_ID.len() + tiff_bytes.len();
    if seg_len > 0xFFFF {
        return None;
    }
    let mut seg = vec![0xFF, 0xE1];
    seg.extend_from_slice(&(seg_len as u16).to_be_bytes());
    seg.extend_from_slice(EXIF_ID);
    seg.extend_from_slice(&tiff_bytes);
    let mut out = Vec::with_capacity(d.len() + seg.len());
    match range {
        Some((a, b)) => {
            out.extend_from_slice(&d[..a]);
            out.extend_from_slice(&seg);
            out.extend_from_slice(&d[b..]);
        }
        None => {
            // After SOI, and after a JFIF APP0 (which must come first).
            let at = segs
                .first()
                .filter(|s| s.marker == 0xE0 && s.payload(d).starts_with(b"JFIF\0"))
                .map_or(2, |s| s.at + s.len);
            out.extend_from_slice(&d[..at]);
            out.extend_from_slice(&seg);
            out.extend_from_slice(&d[at..]);
        }
    }
    Some(out)
}

/// Set `tiff:Orientation` in an XMP APP1 in place, when one is there with a value.
fn patch_xmp(d: &mut [u8], o: u8) {
    let Some(segs) = segments(d) else { return };
    for s in segs {
        if s.marker != 0xE1 || !s.payload(d).starts_with(XMP_ID) {
            continue;
        }
        let range = s.at + 4..s.at + s.len;
        super::xmp::patch_orientation_in_place(&mut d[range], o);
    }
}

/// The tag route: Exif Orientation set to `o` (created when missing), XMP's too.
pub fn set_tag(d: &[u8], o: u8) -> Option<Vec<u8>> {
    let mut out = edit_exif(d, true, |t| tiff::set_orientation(t, o))?;
    patch_xmp(&mut out, o);
    Some(out)
}

/// The lossless route: the pixels transformed by `total` (the stored orientation already
/// folded in), Orientation reset to 1, Exif pixel dimensions swapped on a quarter turn, the
/// Exif thumbnail transformed too (dropped when it no longer fits its place).
pub fn lossless(d: &[u8], total: Transform, trim: bool) -> Result<Vec<u8>, String> {
    let rotated =
        specular_jpeg::transform(d, total, TransformOptions { trim }).map_err(|e| e.to_string())?;
    let mut out = edit_exif(&rotated, false, |t| {
        tiff::set_orientation(t, 1)?;
        if total.transpose {
            let _ = tiff::swap_pixel_dimensions(t);
        }
        if let Some((off, len)) = tiff::thumbnail(t) {
            let thumb = t[off..off + len].to_vec();
            match specular_jpeg::transform(&thumb, total, TransformOptions { trim: true }) {
                Ok(n) if tiff::replace_thumbnail(t, &n).is_some() => {}
                _ => {
                    let _ = tiff::drop_thumbnail(t);
                }
            }
        }
        Some(())
    })
    .ok_or_else(|| "the Exif block could not be rewritten".to_owned())?;
    patch_xmp(&mut out, 1);
    Ok(out)
}

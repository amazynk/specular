//! Burned-in text masking: rectangles of image pixels set to black (stored value 0; for YBR
//! samples luma 0 and neutral chroma) in every frame.
//!
//! Native pixel data is masked in place and keeps its layout. Encapsulated (compressed) pixel
//! data is decoded frame by frame, masked, and written as native samples for Explicit VR
//! Little Endian: grey and palette indices keep their bit depth, colour becomes interleaved
//! RGB. The descriptive attributes are updated to match.

use std::borrow::Cow;
use std::sync::atomic::AtomicBool;

use crate::dict::Vr;
use crate::pixel::{Grey, Photometric, PixelInfo, Pixels};
use crate::write::{Object, Value};
use crate::{Dicom, Error, Result};

/// A rectangle of image pixels: columns `x..x + w`, rows `y..y + h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    /// The part inside a `columns` × `rows` image, if any.
    pub fn clip(self, columns: u32, rows: u32) -> Option<Rect> {
        let x1 = self.x.saturating_add(self.w).min(columns);
        let y1 = self.y.saturating_add(self.h).min(rows);
        (self.x < x1 && self.y < y1).then(|| Rect::new(self.x, self.y, x1 - self.x, y1 - self.y))
    }

    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x - self.x < self.w && y - self.y < self.h
    }
}

/// What masking did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub masked: bool,
    pub transcoded: bool,
}

const PIXEL_TAGS: [u32; 3] = [0x7FE0_0010, 0x7FE0_0008, 0x7FE0_0009];

/// Masks the pixel data of `obj` (built from `d`).
pub(crate) fn apply(
    d: &Dicom,
    file: &[u8],
    obj: &mut Object,
    masks: &[Rect],
    cancel: Option<&AtomicBool>,
) -> Result<Outcome> {
    rewrite(d, file, obj, masks, false, cancel)
}

/// Replaces encapsulated (compressed) pixel data in `obj` (built from `d`) with native
/// little-endian pixel data, every frame decoded by the in-tree decoders: the data set can
/// then be written in Explicit or Implicit VR Little Endian (sending to a server that does
/// not accept the stored syntax). Whether anything changed (native data stays as it is).
pub fn decompress(
    d: &Dicom,
    file: &[u8],
    obj: &mut Object,
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    rewrite(d, file, obj, &[], true, cancel).map(|o| o.transcoded)
}

/// Masks `masks` and, when `native` is set, decodes encapsulated pixel data even with no
/// mask to apply.
fn rewrite(
    d: &Dicom,
    file: &[u8],
    obj: &mut Object,
    masks: &[Rect],
    native: bool,
    cancel: Option<&AtomicBool>,
) -> Result<Outcome> {
    let Some(info) = d.pixel.clone() else {
        return Ok(Outcome::default());
    };
    let rects: Vec<Rect> = masks
        .iter()
        .filter_map(|r| r.clip(info.columns, info.rows))
        .collect();
    if rects.is_empty() && !native {
        return Ok(Outcome::default());
    }
    let Some(tag) = PIXEL_TAGS.into_iter().find(|&t| obj.get(t).is_some()) else {
        return Ok(Outcome::default());
    };
    let encapsulated = matches!(
        obj.get(tag).map(|e| &e.value),
        Some(Value::Fragments { .. })
    );
    if !encapsulated && rects.is_empty() {
        return Ok(Outcome::default());
    }
    if !encapsulated {
        let e = obj.get_mut(tag).ok_or(Error::NoPixelData)?;
        let mut data = e.bytes().to_vec();
        mask_native(&info, &mut data, &rects);
        e.value = Value::Bytes(Cow::Owned(data));
        return Ok(Outcome {
            masked: true,
            transcoded: false,
        });
    }
    d.check_supported()?;
    let mut out: Vec<u8> = Vec::new();
    // Every frame (as many as Number of Frames claims) ends up in `out`.
    let mut res = specular_core::Reservation::none();
    let mut layout: Option<(u16, u16, bool)> = None; // bits allocated, stored, rgb
    for i in 0..info.frames {
        crate::cancelled(cancel)?;
        res.grow_to(out.capacity() as u64)?;
        let mut f = d.decode_stored(file, i, cancel)?;
        let w = f.width;
        let inside = |idx: usize| {
            let (x, y) = ((idx % w as usize) as u32, (idx / w as usize) as u32);
            rects.iter().any(|r| r.contains(x, y))
        };
        let this = match &mut f.pixels {
            Pixels::Grey(g) => {
                let alloc = match g {
                    Grey::U16(_) if info.bits_allocated <= 8 && f.bits <= 8 => 8,
                    Grey::U16(_) | Grey::I16(_) => 16,
                    Grey::I32(_) => 32,
                    Grey::F32(_) | Grey::F64(_) => {
                        return Err(Error::Unsupported("compressed float pixel data".into()));
                    }
                };
                let n = g.len();
                for idx in 0..n {
                    let v = if inside(idx) {
                        0
                    } else {
                        g.get(idx).unwrap_or(0)
                    };
                    match alloc {
                        8 => out.push(v as u8),
                        16 => out.extend_from_slice(&(v as u16).to_le_bytes()),
                        _ => out.extend_from_slice(&(v as u32).to_le_bytes()),
                    }
                }
                (alloc, f.bits.min(alloc), false)
            }
            Pixels::Rgb8(v) => {
                for (idx, px) in v.chunks_exact_mut(3).enumerate() {
                    if inside(idx) {
                        px.fill(0);
                    }
                }
                out.extend_from_slice(v);
                (8, f.bits.clamp(1, 8), true)
            }
            Pixels::Rgb16(v) => {
                for (idx, px) in v.chunks_exact_mut(3).enumerate() {
                    if inside(idx) {
                        px.fill(0);
                    }
                    for s in px {
                        out.extend_from_slice(&s.to_le_bytes());
                    }
                }
                (16, f.bits.clamp(1, 16), true)
            }
        };
        if layout.is_some_and(|l| l != this) {
            return Err(Error::Malformed("frames decode to different layouts"));
        }
        layout = Some(this);
    }
    let Some((alloc, stored, rgb)) = layout else {
        return Ok(Outcome::default());
    };
    let us = |v: u16| v.to_le_bytes().to_vec();
    obj.put_bytes(0x0028_0100, Vr::US, us(alloc));
    obj.put_bytes(0x0028_0101, Vr::US, us(stored));
    obj.put_bytes(0x0028_0102, Vr::US, us(stored - 1));
    if rgb {
        obj.put_bytes(0x0028_0002, Vr::US, us(3));
        obj.put_text(0x0028_0004, Vr::CS, "RGB");
        obj.put_bytes(0x0028_0006, Vr::US, us(0));
    } else if info.samples == 1 && info.photometric != Photometric::Palette {
        obj.remove(0x0028_0006);
    }
    // Offset tables describe the fragments that are gone.
    obj.remove(0x7FE0_0001);
    obj.remove(0x7FE0_0002);
    let vr = if alloc > 8 { Vr::OW } else { Vr::OB };
    obj.put(tag, vr, Value::Bytes(Cow::Owned(out)));
    Ok(Outcome {
        masked: !rects.is_empty(),
        transcoded: true,
    })
}

/// Bit offset of native frame `frame`, `None` once it is past the data (Number of Frames may
/// claim a billion frames of a gigapixel each).
fn frame_start(frame: u64, frame_bits: u64, len: usize) -> Option<u64> {
    frame
        .checked_mul(frame_bits)
        .filter(|&b| b / 8 < len as u64)
}

/// Masks native little-endian pixel data in place.
pub(crate) fn mask_native(info: &PixelInfo, data: &mut [u8], rects: &[Rect]) {
    let cols = info.columns as usize;
    let pixels = info.pixels() as usize;
    let frame_bits = info.frame_bits();
    if info.bits_allocated == 1 {
        // One bit per pixel, frames packed back to back, first pixel in the low bit.
        for frame in 0..info.frames as u64 {
            let Some(start) = frame_start(frame, frame_bits, data.len()) else {
                break;
            };
            for r in rects {
                for y in r.y as usize..(r.y + r.h) as usize {
                    for x in r.x as usize..(r.x + r.w) as usize {
                        let bit = start + (y * cols + x) as u64;
                        if let Some(b) = data.get_mut((bit / 8) as usize) {
                            *b &= !(1 << (bit % 8));
                        }
                    }
                }
            }
        }
        return;
    }
    let bps = info.bits_allocated as usize / 8;
    let ybr = info.photometric.is_ybr();
    let mid: u64 = 1 << (info.bits_stored.clamp(1, 32) - 1);
    let pair = info.photometric == Photometric::YbrFull422 && info.samples == 3;
    let len = data.len();
    let mut put = |off: usize, v: u64| {
        if let Some(s) = data.get_mut(off..off + bps) {
            s.copy_from_slice(&v.to_le_bytes()[..bps]);
        }
    };
    for frame in 0..info.frames as u64 {
        let Some(start) = frame_start(frame, frame_bits, len) else {
            break;
        };
        let base = (start / 8) as usize;
        for r in rects {
            let (mut x0, mut x1) = (r.x as usize, (r.x + r.w) as usize);
            if pair {
                // Two pixels share their chroma: mask whole pairs.
                x0 &= !1;
                x1 = (x1 + 1) & !1;
            }
            for y in r.y as usize..(r.y + r.h) as usize {
                for x in x0..x1.min(cols) {
                    let idx = y * cols + x;
                    if pair {
                        let p = base + (idx / 2) * 4 * bps;
                        put(p + (idx % 2) * bps, 0);
                        put(p + 2 * bps, mid);
                        put(p + 3 * bps, mid);
                        continue;
                    }
                    for s in 0..info.samples as usize {
                        let off = if info.planar {
                            base + (s * pixels + idx) * bps
                        } else {
                            base + (idx * info.samples as usize + s) * bps
                        };
                        put(off, if ybr && s > 0 { mid } else { 0 });
                    }
                }
            }
        }
    }
}

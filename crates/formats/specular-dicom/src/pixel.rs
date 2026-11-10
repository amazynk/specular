//! The image pixel module (PS3.3 C.7.6.3): what the pixel data holds, and turning one frame of
//! native (uncompressed) samples into stored values: BitsStored / HighBit masking with sign
//! extension, planar configuration, and colour photometrics converted to RGB.

use crate::parse::{DataSet, Element, Values};
use crate::{Error, Result};

/// Largest frame accepted, in pixels.
pub const MAX_FRAME_PIXELS: u64 = 1 << 30;

/// Photometric interpretation (0028,0004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Photometric {
    Monochrome1,
    Monochrome2,
    Rgb,
    Palette,
    YbrFull,
    YbrFull422,
    YbrPartial422,
    YbrPartial420,
    YbrIct,
    YbrRct,
    /// Anything else (ARGB, CMYK, HSV are retired), as stored.
    Other(String),
}

impl Photometric {
    pub fn parse(s: &str) -> Photometric {
        match s.trim() {
            "MONOCHROME1" => Photometric::Monochrome1,
            "MONOCHROME2" | "" => Photometric::Monochrome2,
            "RGB" => Photometric::Rgb,
            "PALETTE COLOR" => Photometric::Palette,
            "YBR_FULL" => Photometric::YbrFull,
            "YBR_FULL_422" => Photometric::YbrFull422,
            "YBR_PARTIAL_422" => Photometric::YbrPartial422,
            "YBR_PARTIAL_420" => Photometric::YbrPartial420,
            "YBR_ICT" => Photometric::YbrIct,
            "YBR_RCT" => Photometric::YbrRct,
            o => Photometric::Other(o.to_owned()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Photometric::Monochrome1 => "MONOCHROME1",
            Photometric::Monochrome2 => "MONOCHROME2",
            Photometric::Rgb => "RGB",
            Photometric::Palette => "PALETTE COLOR",
            Photometric::YbrFull => "YBR_FULL",
            Photometric::YbrFull422 => "YBR_FULL_422",
            Photometric::YbrPartial422 => "YBR_PARTIAL_422",
            Photometric::YbrPartial420 => "YBR_PARTIAL_420",
            Photometric::YbrIct => "YBR_ICT",
            Photometric::YbrRct => "YBR_RCT",
            Photometric::Other(s) => s,
        }
    }

    pub fn is_grey(&self) -> bool {
        matches!(self, Photometric::Monochrome1 | Photometric::Monochrome2)
    }

    pub fn is_ybr(&self) -> bool {
        matches!(
            self,
            Photometric::YbrFull
                | Photometric::YbrFull422
                | Photometric::YbrPartial422
                | Photometric::YbrPartial420
        )
    }
}

/// The pixel description of an image (the data set's, or an icon's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelInfo {
    pub rows: u32,
    pub columns: u32,
    pub samples: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    /// Pixel Representation 1: two's complement.
    pub signed: bool,
    /// Planar Configuration 1: colour-by-plane.
    pub planar: bool,
    pub photometric: Photometric,
    pub frames: u32,
    /// Float Pixel Data (32) or Double Float Pixel Data (64): IEEE samples, no masking.
    pub float: bool,
}

pub(crate) const ROWS: u32 = 0x0028_0010;
pub(crate) const COLUMNS: u32 = 0x0028_0011;

impl PixelInfo {
    pub(crate) fn read(ds: &DataSet, v: &Values) -> Result<PixelInfo> {
        Self::read_as(ds, v, false)
    }

    /// Reads the description of Float (`float`, 32-bit) or Double Float (64-bit) Pixel
    /// Data, or of integer Pixel Data.
    pub(crate) fn read_as(ds: &DataSet, v: &Values, float: bool) -> Result<PixelInfo> {
        let int = |tag: u32| ds.get(tag).and_then(|e| v.int(e));
        let rows = int(ROWS).unwrap_or(0);
        let columns = int(COLUMNS).unwrap_or(0);
        if rows <= 0 || columns <= 0 || rows > 0xFFFF || columns > 0xFFFF {
            return Err(Error::Malformed("no image size"));
        }
        let samples = int(0x0028_0002).unwrap_or(1).clamp(1, 4) as u16;
        let bits_allocated = int(0x0028_0100).unwrap_or(0);
        let bits_allocated = match bits_allocated {
            32 | 64 if float => bits_allocated as u16,
            _ if float => return Err(Error::Malformed("float pixel data not 32 or 64 bits")),
            1 | 8 | 16 | 32 => bits_allocated as u16,
            0 => return Err(Error::Malformed("no BitsAllocated")),
            b => return Err(Error::Unsupported(format!("{b}-bit samples"))),
        };
        let mut bits_stored = int(0x0028_0101).unwrap_or(bits_allocated as i64) as u16;
        if bits_stored == 0 || bits_stored > bits_allocated {
            bits_stored = bits_allocated;
        }
        let mut high_bit = int(0x0028_0102).unwrap_or(bits_stored as i64 - 1) as u16;
        if high_bit >= bits_allocated || high_bit + 1 < bits_stored {
            high_bit = bits_stored - 1;
        }
        let signed = float || int(0x0028_0103) == Some(1);
        let planar = int(0x0028_0006) == Some(1);
        let photometric = ds
            .get(0x0028_0004)
            .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
            .map(Photometric::parse)
            .unwrap_or(if samples == 3 {
                Photometric::Rgb
            } else {
                Photometric::Monochrome2
            });
        let frames = ds
            .get(0x0028_0008)
            .and_then(|e| v.number(e, 0))
            .map_or(1, |f| f.clamp(1.0, 1e9) as u32);
        let info = PixelInfo {
            rows: rows as u32,
            columns: columns as u32,
            samples,
            bits_allocated,
            bits_stored,
            high_bit,
            signed,
            planar,
            photometric,
            frames,
            float,
        };
        if float && (info.samples != 1 || !info.photometric.is_grey()) {
            return Err(Error::Unsupported("colour float pixel data".into()));
        }
        if info.pixels() > MAX_FRAME_PIXELS {
            return Err(Error::TooLarge(
                info.pixels() * u64::from(bits_allocated / 8).max(1),
            ));
        }
        match info.samples {
            1 | 3 => {}
            n => return Err(Error::Unsupported(format!("{n} samples per pixel"))),
        }
        Ok(info)
    }

    pub fn pixels(&self) -> u64 {
        self.rows as u64 * self.columns as u64
    }

    /// Bits one native frame occupies.
    pub fn frame_bits(&self) -> u64 {
        let per_pixel = if self.photometric == Photometric::YbrFull422 && self.samples == 3 {
            // Two luma and one pair of chroma samples per two pixels.
            2 * self.bits_allocated as u64
        } else {
            self.samples as u64 * self.bits_allocated as u64
        };
        self.pixels() * per_pixel
    }

    pub fn is_colour(&self) -> bool {
        self.samples == 3 || self.photometric == Photometric::Palette
    }
}

/// Stored grey values of one frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Grey {
    U16(Vec<u16>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    /// Float Pixel Data.
    F32(Vec<f32>),
    /// Double Float Pixel Data.
    F64(Vec<f64>),
}

impl Grey {
    pub fn len(&self) -> usize {
        match self {
            Grey::U16(v) => v.len(),
            Grey::I16(v) => v.len(),
            Grey::I32(v) => v.len(),
            Grey::F32(v) => v.len(),
            Grey::F64(v) => v.len(),
        }
    }

    /// Whether the samples are floating point.
    pub fn is_float(&self) -> bool {
        matches!(self, Grey::F32(_) | Grey::F64(_))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The stored value at `i`; floats rounded to the nearest integer (NaN as 0).
    pub fn get(&self, i: usize) -> Option<i64> {
        match self {
            Grey::U16(v) => v.get(i).map(|&x| x as i64),
            Grey::I16(v) => v.get(i).map(|&x| x as i64),
            Grey::I32(v) => v.get(i).map(|&x| x as i64),
            Grey::F32(v) => v.get(i).map(|&x| x.round() as i64),
            Grey::F64(v) => v.get(i).map(|&x| x.round() as i64),
        }
    }

    /// The stored value at `i`, exact for every container.
    pub fn get_f64(&self, i: usize) -> Option<f64> {
        match self {
            Grey::F32(v) => v.get(i).map(|&x| x as f64),
            Grey::F64(v) => v.get(i).copied(),
            _ => self.get(i).map(|x| x as f64),
        }
    }

    pub fn bytes(&self) -> usize {
        match self {
            Grey::U16(v) => v.len() * 2,
            Grey::I16(v) => v.len() * 2,
            Grey::I32(v) => v.len() * 4,
            Grey::F32(v) => v.len() * 4,
            Grey::F64(v) => v.len() * 8,
        }
    }
}

/// Decoded samples of one frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    /// Stored values (before the modality transform).
    Grey(Grey),
    /// Interleaved RGB.
    Rgb8(Vec<u8>),
    /// Interleaved RGB of `Frame::bits` significant bits.
    Rgb16(Vec<u16>),
}

/// One decoded frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
    /// Significant bits of the samples (BitsStored, or the codec's precision for colour).
    pub bits: u16,
    /// The stored values are two's complement.
    pub signed: bool,
    /// Pixel data ended early; the missing part is 0.
    pub truncated: bool,
}

impl Frame {
    pub fn bytes(&self) -> usize {
        match &self.pixels {
            Pixels::Grey(g) => g.bytes(),
            Pixels::Rgb8(v) => v.len(),
            Pixels::Rgb16(v) => v.len() * 2,
        }
    }
}

pub(crate) fn alloc<T: Clone + Default>(n: usize) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| Error::TooLarge((n * size_of::<T>()) as u64))?;
    v.resize(n, T::default());
    Ok(v)
}

/// Converts raw unsigned sample bits (`raw`, BitsAllocated wide) to the stored value.
#[derive(Clone, Copy)]
pub(crate) struct Mask {
    shift: u32,
    mask: u32,
    sign: u32,
}

impl Mask {
    pub(crate) fn new(bits_stored: u16, high_bit: u16, signed: bool) -> Mask {
        let bits = bits_stored.clamp(1, 32) as u32;
        Mask {
            shift: (high_bit as u32 + 1).saturating_sub(bits),
            mask: if bits >= 32 {
                u32::MAX
            } else {
                (1 << bits) - 1
            },
            sign: if signed { 1 << (bits - 1) } else { 0 },
        }
    }

    #[inline]
    pub(crate) fn apply(self, raw: u32) -> i64 {
        let v = (raw >> self.shift) & self.mask;
        if v & self.sign != 0 {
            v as i64 - (self.mask as i64 + 1)
        } else {
            v as i64
        }
    }
}

/// Where native frame bytes come from.
pub(crate) struct Native<'a> {
    pub data: &'a [u8],
    /// Bit offset of the frame in `data` (only 1-bit frames start inside a byte).
    pub bit_offset: u64,
    pub big: bool,
    /// Byte-swap 8-bit samples stored as OW in big endian.
    pub swap8: bool,
    pub planar: bool,
}

/// Unpacks one native frame. A short `data` gives a truncated frame.
pub(crate) fn unpack(info: &PixelInfo, src: &Native) -> Result<Frame> {
    let (w, h) = (info.columns, info.rows);
    let n = info.pixels() as usize;
    let need_bits = info.frame_bits();
    let have_bits = (src.data.len() as u64 * 8).saturating_sub(src.bit_offset);
    let truncated = have_bits < need_bits;
    if have_bits == 0 {
        return Err(Error::Truncated);
    }
    let mut frame = Frame {
        width: w,
        height: h,
        pixels: Pixels::Rgb8(Vec::new()),
        bits: info.bits_stored,
        signed: info.signed,
        truncated,
    };
    if info.float {
        let data = src.data;
        frame.bits = info.bits_allocated;
        frame.signed = true;
        frame.pixels = Pixels::Grey(if info.bits_allocated == 64 {
            let mut out = alloc::<f64>(n)?;
            for (o, b) in out.iter_mut().zip(data.chunks_exact(8)) {
                let b: [u8; 8] = b.try_into().unwrap_or_default();
                *o = if src.big {
                    f64::from_be_bytes(b)
                } else {
                    f64::from_le_bytes(b)
                };
            }
            Grey::F64(out)
        } else {
            let mut out = alloc::<f32>(n)?;
            for (o, b) in out.iter_mut().zip(data.chunks_exact(4)) {
                let b: [u8; 4] = b.try_into().unwrap_or_default();
                *o = if src.big {
                    f32::from_be_bytes(b)
                } else {
                    f32::from_le_bytes(b)
                };
            }
            Grey::F32(out)
        });
        return Ok(frame);
    }
    let mask = Mask::new(info.bits_stored, info.high_bit, info.signed);
    let ba = info.bits_allocated;
    if ba == 1 {
        let mut out = alloc::<u16>(n * info.samples as usize)?;
        let base = src.bit_offset;
        for (i, o) in out.iter_mut().enumerate() {
            let bit = base + i as u64;
            if let Some(&b) = src.data.get((bit / 8) as usize) {
                *o = (b >> (bit % 8)) as u16 & 1;
            }
        }
        frame.signed = false;
        frame.bits = 1;
        frame.pixels = if info.samples == 1 {
            Pixels::Grey(Grey::U16(out))
        } else {
            Pixels::Rgb8(out.iter().map(|&v| (v * 255) as u8).collect())
        };
        return Ok(frame);
    }
    let start = (src.bit_offset / 8) as usize;
    let data = src.data.get(start..).unwrap_or(&[]);
    let bps = ba as usize / 8;
    let raw_at = |i: usize| -> u32 {
        let Some(b) = data.get(i * bps..i * bps + bps) else {
            return 0;
        };
        match bps {
            1 => {
                if src.swap8 {
                    data.get(i ^ 1).copied().unwrap_or(0) as u32
                } else {
                    b[0] as u32
                }
            }
            2 if src.big => u16::from_be_bytes([b[0], b[1]]) as u32,
            2 => u16::from_le_bytes([b[0], b[1]]) as u32,
            _ if src.big => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        }
    };
    if info.samples == 1 {
        // Palette indices too: the caller maps them.
        frame.pixels = Pixels::Grey(grey_from(n, info, |i| mask.apply(raw_at(i)))?);
        return Ok(frame);
    }
    // Colour: interleave to RGB, keeping `bits` significant bits.
    let ybr422 = info.photometric == Photometric::YbrFull422;
    let colour_mask = Mask::new(info.bits_stored, info.high_bit, false);
    let at = |s: usize, p: usize| -> u32 {
        let i = if ybr422 {
            // Y0 Y1 Cb Cr for each pair of pixels (rows padded to even width by the encoder
            // per row: PS3.3 C.7.6.3.1.2 requires an even number of columns).
            let pair = p / 2;
            match s {
                0 => pair * 4 + (p & 1),
                1 => pair * 4 + 2,
                _ => pair * 4 + 3,
            }
        } else if src.planar {
            s * n + p
        } else {
            p * 3 + s
        };
        colour_mask.apply(raw_at(i)) as u32
    };
    let bits = info.bits_stored.min(16);
    let down = info.bits_stored.saturating_sub(16) as u32;
    if bits <= 8 {
        let mut out = alloc::<u8>(n * 3)?;
        for (p, px) in out.chunks_exact_mut(3).enumerate() {
            for (s, o) in px.iter_mut().enumerate() {
                *o = at(s, p) as u8;
            }
        }
        frame.pixels = Pixels::Rgb8(out);
    } else {
        let mut out = alloc::<u16>(n * 3)?;
        for (p, px) in out.chunks_exact_mut(3).enumerate() {
            for (s, o) in px.iter_mut().enumerate() {
                *o = (at(s, p) >> down) as u16;
            }
        }
        frame.pixels = Pixels::Rgb16(out);
    }
    frame.bits = bits;
    frame.signed = false;
    ybr_to_rgb(&mut frame, &info.photometric);
    Ok(frame)
}

/// Stored grey values `f(0..n)` in the narrowest container.
pub(crate) fn grey_from(n: usize, info: &PixelInfo, f: impl Fn(usize) -> i64) -> Result<Grey> {
    Ok(if info.bits_stored <= 16 && !info.signed {
        let mut v = alloc::<u16>(n)?;
        for (i, o) in v.iter_mut().enumerate() {
            *o = f(i) as u16;
        }
        Grey::U16(v)
    } else if info.bits_stored <= 16 {
        let mut v = alloc::<i16>(n)?;
        for (i, o) in v.iter_mut().enumerate() {
            *o = f(i) as i16;
        }
        Grey::I16(v)
    } else {
        let mut v = alloc::<i32>(n)?;
        for (i, o) in v.iter_mut().enumerate() {
            *o = f(i).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
        Grey::I32(v)
    })
}

/// Converts YBR_FULL / YBR_FULL_422 (full range) and YBR_PARTIAL_* (studio range) samples to
/// RGB in place (PS3.3 C.7.6.3.1.2).
pub(crate) fn ybr_to_rgb(frame: &mut Frame, pm: &Photometric) {
    let full = matches!(pm, Photometric::YbrFull | Photometric::YbrFull422);
    let partial = matches!(pm, Photometric::YbrPartial422 | Photometric::YbrPartial420);
    if !full && !partial {
        return;
    }
    let bits = frame.bits.clamp(1, 16) as u32;
    let half = (1u32 << (bits - 1)) as f32;
    let max = ((1u32 << bits) - 1) as f32;
    // Studio range offsets scale with the bit depth (16 and 235/240 at 8 bits).
    let k = (1u32 << bits.saturating_sub(8)) as f32;
    let conv = |y: f32, cb: f32, cr: f32| -> [f32; 3] {
        let (cb, cr) = (cb - half, cr - half);
        if full {
            [
                y + 1.402 * cr,
                y - 0.114 * 1.772 / 0.587 * cb - 0.299 * 1.402 / 0.587 * cr,
                y + 1.772 * cb,
            ]
        } else {
            let y = (y - 16.0 * k) * (255.0 / 219.0);
            let (cb, cr) = (cb * (255.0 / 224.0), cr * (255.0 / 224.0));
            [
                y + 1.402 * cr,
                y - 0.114 * 1.772 / 0.587 * cb - 0.299 * 1.402 / 0.587 * cr,
                y + 1.772 * cb,
            ]
        }
    };
    let fix = |v: f32| (v + 0.5).floor().clamp(0.0, max);
    match &mut frame.pixels {
        Pixels::Rgb8(d) => {
            for px in d.chunks_exact_mut(3) {
                let c = conv(px[0] as f32, px[1] as f32, px[2] as f32);
                for (o, v) in px.iter_mut().zip(c) {
                    *o = fix(v) as u8;
                }
            }
        }
        Pixels::Rgb16(d) => {
            for px in d.chunks_exact_mut(3) {
                let c = conv(px[0] as f32, px[1] as f32, px[2] as f32);
                for (o, v) in px.iter_mut().zip(c) {
                    *o = fix(v) as u16;
                }
            }
        }
        Pixels::Grey(_) => {}
    }
}

// ----------------------------------------------------------------------------- palettes

/// One channel of a palette colour lookup table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Lut {
    /// Stored value mapped to the first entry.
    pub first: i64,
    pub data: Vec<u16>,
    /// Bits per entry (8 or 16).
    pub bits: u16,
    /// Bits of the entries' range from the descriptor (8 to 16): a 12-bit VOI LUT maps to
    /// 0..=4095 in 16-bit words.
    pub nominal: u16,
}

impl Lut {
    #[inline]
    pub(crate) fn map(&self, v: i64) -> u16 {
        let i = (v - self.first).clamp(0, self.data.len() as i64 - 1);
        self.data.get(i as usize).copied().unwrap_or(0)
    }
}

/// Reads a LUT descriptor (number of entries, first mapped value, bits) and its data,
/// segmented data (`segmented`, PS3.3 C.7.9.2) if there is no plain data.
pub(crate) fn read_lut(
    ds: &DataSet,
    v: &Values,
    descriptor: u32,
    data: u32,
    segmented: Option<u32>,
    first_signed: bool,
) -> Option<Lut> {
    let d = ds.get(descriptor)?;
    let desc: Vec<u16> = v.u16s(d).take(3).collect();
    let &[n, first, bits] = desc.as_slice() else {
        return None;
    };
    let n = if n == 0 { 65536 } else { n as usize };
    let first = if first_signed {
        first as i16 as i64
    } else {
        first as i64
    };
    let nominal = bits.clamp(8, 16);
    let bits = if bits <= 8 { 8 } else { 16 };
    let mut out: Vec<u16> = if let Some(e) = ds.get(data) {
        let raw = v.bytes(e);
        if raw.len() == n && bits == 8 {
            // 8-bit entries packed in bytes (seen in the wild).
            raw.iter().map(|&b| b as u16).collect()
        } else {
            v.u16s(e).take(n).collect()
        }
    } else {
        let e = segmented.and_then(|t| ds.get(t))?;
        expand_segments(&v.u16s(e).collect::<Vec<_>>(), n)
    };
    if out.is_empty() {
        return None;
    }
    if bits == 8 && out.iter().any(|&x| x > 255) {
        // 8-bit entries in the high byte of each word.
        for x in &mut out {
            *x >>= 8;
        }
    }
    Some(Lut {
        first,
        data: out,
        bits,
        nominal,
    })
}

/// Expands segmented palette data: discrete (0), linear (1) and indirect (2) segments.
fn expand_segments(words: &[u16], max: usize) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    expand_into(words, max, &mut out, 0);
    out
}

fn expand_into(words: &[u16], max: usize, out: &mut Vec<u16>, depth: u32) {
    let mut i = 0;
    while i + 1 < words.len() && out.len() < max {
        let (op, len) = (words[i], words[i + 1] as usize);
        i += 2;
        match op {
            0 => {
                let end = (i + len).min(words.len());
                out.extend(&words[i..end]);
                i = end;
            }
            1 => {
                let Some(&y1) = words.get(i) else { break };
                i += 1;
                let y0 = out.last().copied().unwrap_or(y1) as f64;
                for k in 1..=len {
                    if out.len() >= max {
                        break;
                    }
                    let t = k as f64 / len as f64;
                    out.push((y0 + (y1 as f64 - y0) * t).round() as u16);
                }
            }
            2 if depth == 0 => {
                // `len` segments, starting at the word offset in the next two words (low
                // word first).
                let (Some(&lo), Some(&hi)) = (words.get(i), words.get(i + 1)) else {
                    break;
                };
                i += 2;
                let off = ((hi as usize) << 16) | lo as usize;
                if off < words.len() {
                    // Copy `len` segments from `off`: find where they end.
                    let mut j = off;
                    for _ in 0..len {
                        if j + 1 >= words.len() {
                            break;
                        }
                        j += 2 + match words[j] {
                            0 => words[j + 1] as usize,
                            1 => 1,
                            _ => 2,
                        };
                    }
                    let seg = &words[off..j.min(words.len())];
                    expand_into(seg, max, out, depth + 1);
                }
            }
            _ => break,
        }
    }
    out.truncate(max);
}

/// The palette of a PALETTE COLOR image.
pub(crate) fn palette(ds: &DataSet, v: &Values, signed: bool) -> Result<[Lut; 3]> {
    let ch = |k: u32| {
        read_lut(
            ds,
            v,
            0x0028_1101 + k,
            0x0028_1201 + k,
            Some(0x0028_1221 + k),
            signed,
        )
    };
    match (ch(0), ch(1), ch(2)) {
        (Some(r), Some(g), Some(b)) => Ok([r, g, b]),
        _ => Err(Error::Malformed("palette colour without a palette")),
    }
}

/// Maps stored indices through a palette to RGB.
pub(crate) fn apply_palette(frame: &mut Frame, lut: &[Lut; 3]) -> Result<()> {
    let Pixels::Grey(g) = &frame.pixels else {
        return Ok(());
    };
    let n = g.len();
    let bits = lut.iter().map(|l| l.bits).max().unwrap_or(8);
    if bits <= 8 {
        let mut out = alloc::<u8>(n * 3)?;
        for (i, px) in out.chunks_exact_mut(3).enumerate() {
            let s = g.get(i).unwrap_or(0);
            for (o, l) in px.iter_mut().zip(lut) {
                *o = l.map(s) as u8;
            }
        }
        frame.pixels = Pixels::Rgb8(out);
        frame.bits = 8;
    } else {
        let mut out = alloc::<u16>(n * 3)?;
        for (i, px) in out.chunks_exact_mut(3).enumerate() {
            let s = g.get(i).unwrap_or(0);
            for (o, l) in px.iter_mut().zip(lut) {
                *o = l.map(s);
            }
        }
        frame.pixels = Pixels::Rgb16(out);
        frame.bits = 16;
    }
    frame.signed = false;
    Ok(())
}

/// The native frame `index` inside the pixel data element.
pub(crate) fn native_frame<'a>(
    info: &PixelInfo,
    e: &Element,
    v: &Values<'a>,
    index: u32,
) -> Result<Native<'a>> {
    let data = v.bytes(e);
    let bits = info.frame_bits();
    // Number of Frames may claim a billion frames of a gigapixel each: past u64 is past the data.
    let off = bits
        .checked_mul(index as u64)
        .filter(|off| off / 8 < data.len() as u64)
        .ok_or(Error::Truncated)?;
    let (start, bit_offset) = if info.bits_allocated == 1 {
        ((off / 8) as usize, off % 8)
    } else {
        ((off / 8) as usize, 0)
    };
    let end = (start as u64 + bits.div_ceil(8) + 1).min(data.len() as u64) as usize;
    Ok(Native {
        data: &data[start..end],
        bit_offset,
        big: v.big,
        swap8: v.big && info.bits_allocated == 8 && e.vr == crate::dict::Vr::OW,
        planar: info.planar,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(bits_allocated: u16, bits_stored: u16, high_bit: u16, signed: bool) -> PixelInfo {
        PixelInfo {
            rows: 1,
            columns: 2,
            samples: 1,
            bits_allocated,
            bits_stored,
            high_bit,
            signed,
            planar: false,
            photometric: Photometric::Monochrome2,
            frames: 1,
            float: false,
        }
    }

    #[test]
    fn masks_and_sign_extends() {
        // 12 bits stored in 16, high bit 11, signed: 0x0FFF is -1, 0xF800 garbage above.
        let i = info(16, 12, 11, true);
        let data = [0xFF, 0xFF, 0x00, 0x08];
        let f = unpack(
            &i,
            &Native {
                data: &data,
                bit_offset: 0,
                big: false,
                swap8: false,
                planar: false,
            },
        )
        .unwrap();
        assert_eq!(f.pixels, Pixels::Grey(Grey::I16(vec![-1, -2048])));
        // High bit 15 with 12 stored: shift right by 4.
        let i = info(16, 12, 15, false);
        let f = unpack(
            &i,
            &Native {
                data: &[0xF0, 0xFF, 0x10, 0x00],
                bit_offset: 0,
                big: false,
                swap8: false,
                planar: false,
            },
        )
        .unwrap();
        assert_eq!(f.pixels, Pixels::Grey(Grey::U16(vec![0xFFF, 1])));
    }

    #[test]
    fn segments_expand() {
        // Discrete 2 entries, then linear to 100 over 2, then indirect copy of the first
        // segment.
        let w = [0, 2, 10, 20, 1, 2, 100, 2, 1, 0, 0];
        assert_eq!(expand_segments(&w, 100), vec![10, 20, 60, 100, 10, 20]);
    }
}

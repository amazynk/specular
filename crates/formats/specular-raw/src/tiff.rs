//! A small, bounds-checked TIFF IFD walker for RAW containers.
//!
//! RAW files are TIFF variants with quirks (CR2 header, ORF/RW2 magic numbers, maker notes with
//! their own byte order and offset base, SR2 encrypted sub-IFDs), so this walker is deliberately
//! minimal and tolerant: every read is checked, entries whose value lies outside the buffer are
//! dropped, IFD loops are cut, and the number of IFDs/entries visited is capped.

use std::collections::HashSet;

/// Upper bound on entries read from one IFD (real files have < 300).
const MAX_ENTRIES: usize = 1024;
/// Upper bound on IFDs visited per walk (main chain + sub-IFDs + maker notes).
pub(crate) const MAX_IFDS: usize = 128;

pub(crate) mod tag {
    pub const NEW_SUBFILE_TYPE: u16 = 0x00fe;
    pub const IMAGE_WIDTH: u16 = 0x0100;
    pub const IMAGE_LENGTH: u16 = 0x0101;
    pub const BITS_PER_SAMPLE: u16 = 0x0102;
    pub const COMPRESSION: u16 = 0x0103;
    pub const PHOTOMETRIC: u16 = 0x0106;
    pub const MAKE: u16 = 0x010f;
    pub const MODEL: u16 = 0x0110;
    pub const STRIP_OFFSETS: u16 = 0x0111;
    pub const ORIENTATION: u16 = 0x0112;
    pub const SAMPLES_PER_PIXEL: u16 = 0x0115;
    pub const ROWS_PER_STRIP: u16 = 0x0116;
    pub const STRIP_BYTE_COUNTS: u16 = 0x0117;
    pub const SOFTWARE: u16 = 0x0131;
    pub const DATE_TIME: u16 = 0x0132;
    pub const PREDICTOR: u16 = 0x013d;
    pub const TILE_WIDTH: u16 = 0x0142;
    pub const TILE_LENGTH: u16 = 0x0143;
    pub const TILE_OFFSETS: u16 = 0x0144;
    pub const TILE_BYTE_COUNTS: u16 = 0x0145;
    pub const SUB_IFDS: u16 = 0x014a;
    pub const SAMPLE_FORMAT: u16 = 0x0153;
    pub const JPEG_IF_OFFSET: u16 = 0x0201;
    pub const JPEG_IF_LENGTH: u16 = 0x0202;
    pub const CFA_REPEAT_DIM: u16 = 0x828d;
    pub const CFA_PATTERN: u16 = 0x828e;
    pub const EXPOSURE_TIME: u16 = 0x829a;
    pub const F_NUMBER: u16 = 0x829d;
    pub const EXIF_IFD: u16 = 0x8769;
    pub const ISO: u16 = 0x8827;
    pub const DATE_TIME_ORIGINAL: u16 = 0x9003;
    pub const EXPOSURE_BIAS: u16 = 0x9204;
    pub const FOCAL_LENGTH: u16 = 0x920a;
    pub const MAKER_NOTE: u16 = 0x927c;
    pub const EXIF_CFA_PATTERN: u16 = 0xa302;
    pub const LENS_MODEL: u16 = 0xa434;
    pub const DNG_VERSION: u16 = 0xc612;
    pub const CFA_PLANE_COLOR: u16 = 0xc616;
    pub const CFA_LAYOUT: u16 = 0xc617;
    pub const LINEARIZATION_TABLE: u16 = 0xc618;
    pub const BLACK_LEVEL_REPEAT_DIM: u16 = 0xc619;
    pub const BLACK_LEVEL: u16 = 0xc61a;
    pub const BLACK_LEVEL_DELTA_H: u16 = 0xc61b;
    pub const BLACK_LEVEL_DELTA_V: u16 = 0xc61c;
    pub const WHITE_LEVEL: u16 = 0xc61d;
    pub const DEFAULT_SCALE: u16 = 0xc61e;
    pub const DEFAULT_CROP_ORIGIN: u16 = 0xc61f;
    pub const DEFAULT_CROP_SIZE: u16 = 0xc620;
    pub const COLOR_MATRIX1: u16 = 0xc621;
    pub const COLOR_MATRIX2: u16 = 0xc622;
    pub const CAMERA_CALIBRATION1: u16 = 0xc623;
    pub const CAMERA_CALIBRATION2: u16 = 0xc624;
    pub const ANALOG_BALANCE: u16 = 0xc627;
    pub const AS_SHOT_NEUTRAL: u16 = 0xc628;
    pub const AS_SHOT_WHITE_XY: u16 = 0xc629;
    pub const BASELINE_EXPOSURE: u16 = 0xc62a;
    pub const DNG_PRIVATE_DATA: u16 = 0xc634;
    pub const CALIBRATION_ILLUMINANT1: u16 = 0xc65a;
    pub const CALIBRATION_ILLUMINANT2: u16 = 0xc65b;
    pub const ACTIVE_AREA: u16 = 0xc68d;
    pub const FORWARD_MATRIX1: u16 = 0xc714;
    pub const NOISE_PROFILE: u16 = 0xc761;
    pub const FORWARD_MATRIX2: u16 = 0xc715;
    pub const CAMERA_CALIBRATION_SIGNATURE: u16 = 0xc6f3;
    pub const PROFILE_CALIBRATION_SIGNATURE: u16 = 0xc6f4;
    pub const UNIQUE_CAMERA_MODEL: u16 = 0xc614;
    pub const PROFILE_EMBED_POLICY: u16 = 0xc6fd;
    pub const PROFILE_NAME: u16 = 0xc6f8;
    pub const PROFILE_HUE_SAT_MAP_DIMS: u16 = 0xc6f9;
    pub const PROFILE_HUE_SAT_MAP_DATA1: u16 = 0xc6fa;
    pub const PROFILE_HUE_SAT_MAP_DATA2: u16 = 0xc6fb;
    pub const PROFILE_TONE_CURVE: u16 = 0xc6fc;
    pub const PROFILE_COPYRIGHT: u16 = 0xc6fe;
    pub const PROFILE_LOOK_TABLE_DIMS: u16 = 0xc725;
    pub const PROFILE_LOOK_TABLE_DATA: u16 = 0xc726;
    pub const PROFILE_HUE_SAT_MAP_ENCODING: u16 = 0xc7a3;
    pub const PROFILE_LOOK_TABLE_ENCODING: u16 = 0xc7a4;
    pub const BASELINE_EXPOSURE_OFFSET: u16 = 0xc7a5;
    pub const PROFILE_DYNAMIC_RANGE: u16 = 0xcd47;
    /// Canon CR2 slice layout of the lossless JPEG stream.
    pub const CR2_SLICE: u16 = 0xc640;
}

/// Field types.
const BYTE: u16 = 1;
const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;
const SBYTE: u16 = 6;
const UNDEFINED: u16 = 7;
const SSHORT: u16 = 8;
const SLONG: u16 = 9;
const SRATIONAL: u16 = 10;
const FLOAT: u16 = 11;
const DOUBLE: u16 = 12;
const IFD: u16 = 13;

fn type_size(t: u16) -> Option<usize> {
    Some(match t {
        BYTE | ASCII | SBYTE | UNDEFINED => 1,
        SHORT | SSHORT => 2,
        LONG | SLONG | FLOAT | IFD => 4,
        RATIONAL | SRATIONAL | DOUBLE => 8,
        _ => return None,
    })
}

/// Endian-aware bounds-checked reads.
#[derive(Clone, Copy)]
pub(crate) struct Endian {
    pub le: bool,
}

impl Endian {
    pub fn u16(self, d: &[u8], off: usize) -> Option<u16> {
        let b: [u8; 2] = d.get(off..off.checked_add(2)?)?.try_into().ok()?;
        Some(if self.le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    pub fn u32(self, d: &[u8], off: usize) -> Option<u32> {
        let b: [u8; 4] = d.get(off..off.checked_add(4)?)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    pub fn u64(self, d: &[u8], off: usize) -> Option<u64> {
        let b: [u8; 8] = d.get(off..off.checked_add(8)?)?.try_into().ok()?;
        Some(if self.le {
            u64::from_le_bytes(b)
        } else {
            u64::from_be_bytes(b)
        })
    }
}

pub(crate) fn be_u16(d: &[u8], off: usize) -> Option<u16> {
    Endian { le: false }.u16(d, off)
}
pub(crate) fn be_u32(d: &[u8], off: usize) -> Option<u32> {
    Endian { le: false }.u32(d, off)
}
pub(crate) fn be_u64(d: &[u8], off: usize) -> Option<u64> {
    Endian { le: false }.u64(d, off)
}

/// One IFD entry; `pos` is the absolute position of the value bytes in the buffer the IFD was
/// read from (already validated to be in bounds).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub tag: u16,
    pub typ: u16,
    pub count: u32,
    pub pos: usize,
    pub le: bool,
}

impl Entry {
    fn e(&self) -> Endian {
        Endian { le: self.le }
    }
    pub fn byte_len(&self) -> usize {
        type_size(self.typ).unwrap_or(1) * self.count as usize
    }
    /// Raw value bytes.
    pub fn bytes<'a>(&self, d: &'a [u8]) -> Option<&'a [u8]> {
        d.get(self.pos..self.pos.checked_add(self.byte_len())?)
    }
    /// Integer value `i` (BYTE/SHORT/LONG/IFD and signed variants; rationals are truncated).
    pub fn uint(&self, d: &[u8], i: usize) -> Option<u32> {
        if i >= self.count as usize {
            return None;
        }
        let e = self.e();
        match self.typ {
            BYTE | UNDEFINED | ASCII => d.get(self.pos + i).map(|&b| b as u32),
            SBYTE => d.get(self.pos + i).map(|&b| b as i8 as i32 as u32),
            SHORT => e.u16(d, self.pos + 2 * i).map(u32::from),
            SSHORT => e.u16(d, self.pos + 2 * i).map(|v| v as i16 as i32 as u32),
            LONG | SLONG | IFD => e.u32(d, self.pos + 4 * i),
            RATIONAL | SRATIONAL | FLOAT | DOUBLE => self.float(d, i).map(|f| f as u32),
            _ => None,
        }
    }
    /// Numeric value `i` as f64 (any numeric type).
    pub fn float(&self, d: &[u8], i: usize) -> Option<f64> {
        if i >= self.count as usize {
            return None;
        }
        let e = self.e();
        match self.typ {
            RATIONAL => {
                let n = e.u32(d, self.pos + 8 * i)? as f64;
                let m = e.u32(d, self.pos + 8 * i + 4)? as f64;
                (m != 0.0).then(|| n / m)
            }
            SRATIONAL => {
                let n = e.u32(d, self.pos + 8 * i)? as i32 as f64;
                let m = e.u32(d, self.pos + 8 * i + 4)? as i32 as f64;
                (m != 0.0).then(|| n / m)
            }
            FLOAT => e.u32(d, self.pos + 4 * i).map(|b| f32::from_bits(b) as f64),
            DOUBLE => e.u64(d, self.pos + 8 * i).map(f64::from_bits),
            SBYTE | SSHORT | SLONG => self.uint(d, i).map(|v| v as i32 as f64),
            _ => self.uint(d, i).map(f64::from),
        }
        .filter(|f| f.is_finite())
    }
    pub fn uints(&self, d: &[u8], max: usize) -> Vec<u32> {
        (0..(self.count as usize).min(max))
            .map_while(|i| self.uint(d, i))
            .collect()
    }
    pub fn floats(&self, d: &[u8], max: usize) -> Vec<f64> {
        (0..(self.count as usize).min(max))
            .map_while(|i| self.float(d, i))
            .collect()
    }
    /// ASCII value, cut at the first NUL and trimmed.
    pub fn string(&self, d: &[u8]) -> Option<String> {
        let b = self.bytes(d)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        let s = String::from_utf8_lossy(&b[..end]).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// A parsed IFD. Offsets inside it were resolved against `base`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Ifd {
    pub entries: Vec<Entry>,
    /// Absolute offset of the IFD.
    pub offset: usize,
    /// Absolute offset of the next IFD (0 = none).
    pub next: usize,
    pub base: usize,
    pub le: bool,
}

impl Ifd {
    pub fn get(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.tag == tag)
    }
    pub fn uint(&self, d: &[u8], tag: u16) -> Option<u32> {
        self.get(tag)?.uint(d, 0)
    }
    pub fn float(&self, d: &[u8], tag: u16) -> Option<f64> {
        self.get(tag)?.float(d, 0)
    }
    pub fn string(&self, d: &[u8], tag: u16) -> Option<String> {
        self.get(tag)?.string(d)
    }
    pub fn uints(&self, d: &[u8], tag: u16, max: usize) -> Vec<u32> {
        self.get(tag).map(|e| e.uints(d, max)).unwrap_or_default()
    }
    pub fn floats(&self, d: &[u8], tag: u16, max: usize) -> Vec<f64> {
        self.get(tag).map(|e| e.floats(d, max)).unwrap_or_default()
    }
    /// Absolute offsets referenced by an offset-type tag (`SubIFDs`, `ExifIFD`, ...), resolved
    /// against this IFD's base.
    pub fn offsets(&self, d: &[u8], tag: u16, max: usize) -> Vec<usize> {
        self.uints(d, tag, max)
            .into_iter()
            .filter(|&o| o != 0)
            .filter_map(|o| self.base.checked_add(o as usize))
            .collect()
    }
}

/// Parse the IFD at absolute offset `off`; value offsets are relative to `base`.
pub(crate) fn read_ifd(d: &[u8], off: usize, base: usize, le: bool) -> Option<Ifd> {
    let mut ifd = read_ifd_with(d, off, le, &|o| base.checked_add(o as usize))?;
    ifd.base = base;
    Some(ifd)
}

/// Like [`read_ifd`] with a custom resolver from stored offsets to buffer positions (e.g. the
/// decrypted Sony SR2 block, whose offsets are relative to the original file).
pub(crate) fn read_ifd_with(
    d: &[u8],
    off: usize,
    le: bool,
    resolve: &dyn Fn(u32) -> Option<usize>,
) -> Option<Ifd> {
    let e = Endian { le };
    let n = e.u16(d, off)? as usize;
    if n == 0 {
        return None;
    }
    let mut entries = Vec::with_capacity(n.min(MAX_ENTRIES));
    let mut last = off + 2;
    for i in 0..n.min(MAX_ENTRIES) {
        let p = off + 2 + 12 * i;
        let (Some(tag), Some(typ), Some(count)) = (e.u16(d, p), e.u16(d, p + 2), e.u32(d, p + 4))
        else {
            break;
        };
        last = p + 12;
        let Some(sz) = type_size(typ) else { continue };
        let Some(len) = (count as usize).checked_mul(sz) else {
            continue;
        };
        let pos = if len <= 4 {
            p + 8
        } else {
            match e.u32(d, p + 8).and_then(resolve) {
                Some(o) => o,
                None => continue,
            }
        };
        if pos.checked_add(len).is_none_or(|end| end > d.len()) {
            continue;
        }
        entries.push(Entry {
            tag,
            typ,
            count,
            pos,
            le,
        });
    }
    if entries.is_empty() {
        return None;
    }
    let next = if n <= MAX_ENTRIES {
        e.u32(d, last)
            .filter(|&o| o != 0)
            .and_then(resolve)
            .unwrap_or(0)
    } else {
        0
    };
    Some(Ifd {
        entries,
        offset: off,
        next,
        base: 0,
        le,
    })
}

/// Byte order and first-IFD offset of a TIFF-like header at `base`. Accepts the standard magic
/// (42), ORF (`IIRO`/`IIRS`/`MMOR`) and RW2 (`IIU\0`) variants.
pub(crate) fn header(d: &[u8], base: usize) -> Option<(bool, usize)> {
    let le = match d.get(base..base + 2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let e = Endian { le };
    let magic = e.u16(d, base + 2)?;
    if !matches!(magic, 42 | 0x4f52 | 0x5352 | 0x0055) {
        return None;
    }
    let off = e.u32(d, base + 4)? as usize;
    Some((le, base.checked_add(off)?))
}

/// Tracks visited IFD offsets to cut loops and bound the walk.
#[derive(Default)]
pub(crate) struct Visited {
    seen: HashSet<usize>,
}

impl Visited {
    /// Returns false if `off` was seen before or the walk budget is exhausted.
    pub fn insert(&mut self, off: usize) -> bool {
        self.seen.len() < MAX_IFDS && self.seen.insert(off)
    }
}

/// The main IFD chain starting at `first`.
pub(crate) fn chain(d: &[u8], first: usize, base: usize, le: bool, v: &mut Visited) -> Vec<Ifd> {
    let mut out = Vec::new();
    let mut off = first;
    while off != 0 && v.insert(off) {
        let Some(ifd) = read_ifd(d, off, base, le) else {
            break;
        };
        off = ifd.next;
        out.push(ifd);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_without_panicking() {
        for len in 0..64 {
            let d: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            for off in 0..len + 4 {
                let _ = read_ifd(&d, off, 0, true);
                let _ = read_ifd(&d, off, usize::MAX - 2, false);
            }
            let _ = header(&d, 0);
        }
    }

    #[test]
    fn loops_are_cut() {
        // IFD at 8 whose next pointer points to itself.
        let mut d = b"II*\0\x08\0\0\0".to_vec();
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&0x100u16.to_le_bytes());
        d.extend_from_slice(&3u16.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&7u32.to_le_bytes());
        d.extend_from_slice(&8u32.to_le_bytes());
        let (le, first) = header(&d, 0).unwrap();
        let ifds = chain(&d, first, 0, le, &mut Visited::default());
        assert_eq!(ifds.len(), 1);
        assert_eq!(ifds[0].uint(&d, tag::IMAGE_WIDTH), Some(7));
    }
}

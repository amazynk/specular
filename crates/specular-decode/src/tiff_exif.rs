//! The Exif of a TIFF-based file (TIFF, BigTIFF, DNG, NEF, CR2, ARW, PEF, ORF, RW2…) as a compact
//! TIFF structure, the way a JPEG's APP1 carries it: IFD0's descriptive tags (Make, Model,
//! Orientation, DateTime, Artist, Copyright, resolution…), the Exif IFD (with its Interop IFD)
//! and the GPS IFD, rewritten as a classic little-endian TIFF into one small buffer. BigTIFF's
//! 8-byte counts and offsets are read the same way; its LONG8/SLONG8/IFD8 values become
//! LONG/SLONG when they fit.
//!
//! The image structure of the file is left out: strip/tile layout, sample format, sub-IFDs,
//! the JPEG thumbnail pointers, the following IFDs (pages, previews, raw data), the metadata
//! read elsewhere (XMP 700, IPTC 33723, Photoshop resources 34377, ICC 34675), DNGPrivateData
//! and a DNG profile's tables. Other values over [`MAX_VALUE`] are dropped too.
//!
//! The maker note: the notes of Canon, Sony, Panasonic and Samsung (IFDs whose offsets count
//! from the TIFF header or from the note) are rebuilt as a fresh little-endian IFD after the
//! rest, their offsets rewritten from the new header: Canon's with only the [`CANON_KEEP`]
//! tags (as a CR3's), Sony's and Panasonic's whole but for their serial numbers and the
//! entries that name people or places ([`SONY_DROP`], [`PANASONIC_DROP`]), after their 12-byte
//! header, Samsung's (the IFD-style "Type 2" note of the NX cameras, no header) with only the
//! [`SAMSUNG_KEEP`] tags. A Sony, Panasonic or Samsung note is rebuilt only when intact (every
//! value inside the note, its offsets counting from the TIFF header or, in some variants,
//! from the note itself; when both read, the one whose values start right after the IFD) and
//! is dropped otherwise. A big-endian one is swapped into the little-endian IFD value by value; its
//! blocks of bytes keep their own order, so only those known to hold bytes or 16-bit words
//! (swapped) are kept ([`Maker::swap_block`]). A DNG from Adobe DNG Converter keeps the original note in DNGPrivateData
//! (an "Adobe\0" block whose "MakN" chunk holds a big-endian length, the note's byte order and
//! its offset in the original file, then the note); when the Exif IFD has no note, it is read
//! from there, its offsets rebased on that original offset. Any other note is kept as a blob:
//! at its original offset when that lies past the rest of the structure and within
//! [`MAX_EXIF`], so offsets from the TIFF header still resolve; else moved after the rest when
//! its offsets count from the note itself (Nikon, Fujifilm, Olympus, Pentax, Apple), and
//! otherwise dropped rather than passed on broken. Only the header, the IFDs and their values
//! are read, through [`At`], so a mapped file and a reader cost alike.

use std::borrow::Cow;

use crate::container_locate::CANON_KEEP;
use crate::exif_locate::{At, MAX_EXIF};

/// Largest single value kept (the maker note excepted).
const MAX_VALUE: u64 = 64 << 10;
/// Most entries read from one IFD.
const MAX_ENTRIES: u64 = 1024;
/// Most DNGPrivateData chunks walked looking for "MakN".
const MAX_CHUNKS: usize = 64;

const MAKE: u16 = 0x010F;
const EXIF_IFD: u16 = 0x8769;
const GPS_IFD: u16 = 0x8825;
const INTEROP_IFD: u16 = 0xA005;
const MAKER_NOTE: u16 = 0x927C;
const DNG_PRIVATE: u16 = 0xC634;
/// Panasonic RW2's JpgFromRaw, in IFD0: a JPEG preview carrying the Exif.
const JPG_FROM_RAW: u16 = 0x002E;
/// Panasonic RW2's ISO (SHORT) and, on later bodies, its 32-bit ISO, in IFD0.
const RW2_ISO: u16 = 0x0017;
const RW2_ISO_LONG: u16 = 0x0037;
/// Panasonic RW2's CameraIFD, in IFD0 (where TIFF has FreeOffsets): an embedded TIFF ("II*\0"
/// and its IFD) whose offsets count from its own start.
const CAMERA_IFD: u16 = 0x0120;

/// The magic numbers of TIFF headers read like classic TIFF's 42: Panasonic RW2 (and Leica
/// RWL), "IIU\0"; Olympus ORF, "IIRO", "IIRS" or "MMOR".
const RW2_MAGIC: u16 = 0x0055;
const ORF_MAGIC_RO: u16 = 0x4F52;
const ORF_MAGIC_RS: u16 = 0x5352;
/// JPEG XR's container ("II\xBC\x01"; version 0 files have 0x00BC): classic TIFF layout, its
/// own image structure in tags 0xBC01..=0xBCFF.
const JXR_MAGIC: u16 = 0x01BC;
const JXR_MAGIC_V0: u16 = 0x00BC;

/// Exif IFD tags of an RW2's JpgFromRaw that describe the preview, not the raw image:
/// CompressedBitsPerPixel, PixelXDimension, PixelYDimension.
const PREVIEW_ONLY: [u16; 3] = [0x9102, 0xA002, 0xA003];

/// Tags describing the file's image data or pointing into it, metadata found elsewhere, and
/// the DNG colour profile's tables (hue/saturation maps, tone curve, look table); sorted.
const DROP: [u16; 48] = [
    254, 255, 256, 257, 258, 259, 262, 263, 264, 265, 266, 273, 277, 278, 279, 280, 281, 284, 288,
    289, 290, 291, 292, 293, 317, 320, 322, 323, 324, 325, 330, 338, 339, 340, 341, 347, 513, 514,
    700, 33723, 34377, 34675, 37724, 50740, 50938, 50939, 50940, 50982,
];

/// Starts of maker notes whose offsets count from the note itself (or that carry their own
/// TIFF header), so they can be moved: Nikon types 2 and 3, Fujifilm, Olympus and OM System
/// (new style), Pentax ("PENTAX \0" notes), Apple.
const SELF_CONTAINED: [&[u8]; 6] = [
    b"Nikon\0\x02",
    b"FUJIFILM",
    b"OLYMPUS\0",
    b"OM SYSTEM\0",
    b"PENTAX \0",
    b"Apple iOS\0",
];

/// Sony maker-note entries left out of a rebuilt note: SerialNumber (0x2031) and the
/// enciphered block holding the internal serial number and shutter count (0x9050).
const SONY_DROP: [u16; 2] = [0x2031, 0x9050];

/// Panasonic maker-note entries left out of a rebuilt note: the data dump (0x21), serial
/// numbers (internal 0x25, lens 0x52, accessory 0x54), and what names people or places: baby
/// age and name (0x33, 0x66, 0x8010), recognised faces (0x61), title (0x65), location,
/// country, state, city and landmark (0x67–0x6F, 0x80).
const PANASONIC_DROP: [u16; 15] = [
    0x21, 0x25, 0x33, 0x52, 0x54, 0x61, 0x65, 0x66, 0x67, 0x69, 0x6B, 0x6D, 0x6F, 0x80, 0x8010,
];

/// Samsung maker-note entries kept in a rebuilt note, sorted: MakerNoteVersion, DeviceType,
/// SamsungModelID, SmartAlbumColor, PictureWizard, RawDataByteOrder, WhiteBalanceSetup,
/// CameraTemperature, FaceDetect, FaceRecognition (on or off, not the names), FirmwareName,
/// LensType, LensFirmware, SensorAreas, ColorSpace, SmartRange, ExposureCompensation, ISO,
/// ExposureTime, FNumber, FocalLengthIn35mmFormat. Left out: the serial numbers (0xA002,
/// lens 0xA005), face and location names, the PreviewIFD pointer, the encryption key and the
/// enciphered or undocumented blocks.
const SAMSUNG_KEEP: [u16; 21] = [
    0x0001, 0x0002, 0x0003, 0x0020, 0x0021, 0x0040, 0x0041, 0x0043, 0x0100, 0x0120, 0xA001, 0xA003,
    0xA004, 0xA010, 0xA011, 0xA012, 0xA013, 0xA014, 0xA018, 0xA019, 0xA01A,
];

/// Headers (12 bytes with their padding) before the IFD of a Sony note: "SONY DSC \0",
/// "SONY CAM \0", "SONY MOBILE\0" and the Sony-made Hasselblads' "VHAB     \0". The note of an
/// ARW or SR2 has none: it starts with its IFD.
const SONY_HEADERS: [&[u8]; 4] = [
    b"SONY DSC \0",
    b"SONY CAM \0",
    b"SONY MOBILE\0",
    b"VHAB     \0",
];

/// Whose maker note is rebuilt, which says what it keeps.
#[derive(Clone, Copy, PartialEq)]
enum Maker {
    /// Only [`CANON_KEEP`].
    Canon,
    /// All but [`SONY_DROP`].
    Sony,
    /// All but [`PANASONIC_DROP`].
    Panasonic,
    /// Only [`SAMSUNG_KEEP`].
    Samsung,
}

impl Maker {
    /// From IFD0's Make.
    fn of(make: &[u8]) -> Option<Maker> {
        let is = |p: &[u8]| {
            make.get(..p.len())
                .is_some_and(|m| m.eq_ignore_ascii_case(p))
        };
        if is(b"canon") {
            Some(Maker::Canon)
        } else if is(b"sony") {
            Some(Maker::Sony)
        } else if is(b"panasonic") {
            Some(Maker::Panasonic)
        } else if is(b"samsung") {
            Some(Maker::Samsung)
        } else {
            None
        }
    }

    fn keeps(self, tag: u16) -> bool {
        match self {
            Maker::Canon => CANON_KEEP.contains(&tag),
            Maker::Sony => !SONY_DROP.contains(&tag),
            Maker::Panasonic => !PANASONIC_DROP.contains(&tag),
            Maker::Samsung => SAMSUNG_KEEP.binary_search(&tag).is_ok(),
        }
    }

    /// Whether the UNDEFINED entry `e` of a big-endian note keeps its meaning in the rebuilt
    /// little-endian one: bytes (versions) as they are, 16-bit words once swapped. Other
    /// blocks (Panasonic's TimeInfo and PrintIM, Sony's enciphered and binary blocks) are left
    /// out.
    fn swap_block(self, e: &mut Entry) -> bool {
        let unit = match (self, e.tag) {
            // FirmwareVersion, PanasonicExifVersion, LensFirmwareVersion, MakerNoteVersion.
            (Maker::Panasonic, 0x02 | 0x26 | 0x60 | 0x8000) => 1,
            // FaceDetInfo (16-bit words), Transform (two 16-bit integers).
            (Maker::Panasonic, 0x4E | 0x59) => 2,
            // MakerNoteVersion.
            (Maker::Samsung, 0x0001) => 1,
            _ => return false,
        };
        if !e.value.len().is_multiple_of(unit) {
            return false;
        }
        for u in e.value.chunks_exact_mut(unit) {
            u.reverse();
        }
        true
    }

    /// The length of the header before the note's IFD, from the note's first bytes; None when
    /// the note is of another kind (left as it is).
    fn header(self, head: &[u8], le: bool) -> Option<u64> {
        match self {
            Maker::Canon => Some(0),
            Maker::Sony if SONY_HEADERS.iter().any(|h| head.starts_with(h)) => Some(12),
            // An IFD straight away: a small entry count (not Sony Ericsson's "SEMC MS\0", an
            // older "SONY PI\0" or "\0\0SONY PIC\0" note, or a Minolta-style "\x01\0" block).
            Maker::Sony => {
                let n = match *head.get(..2)? {
                    [a, b] if le => u16::from_le_bytes([a, b]),
                    [a, b] => u16::from_be_bytes([a, b]),
                    _ => return None,
                };
                (2..=512).contains(&n).then_some(0)
            }
            Maker::Panasonic if head.starts_with(b"Panasonic\0\0\0") => Some(12),
            Maker::Panasonic => None,
            // An IFD straight away whose first entry is MakerNoteVersion, UNDEFINED[4] "0100"
            // (not the "STMN" note of Samsung's compacts or a Type 1 note).
            Maker::Samsung => {
                let first: [u8; 12] = if le {
                    *b"\x01\0\x07\0\x04\0\0\x000100"
                } else {
                    *b"\0\x01\0\x07\0\0\0\x040100"
                };
                let n = match *head.get(..2)? {
                    [a, b] if le => u16::from_le_bytes([a, b]),
                    [a, b] => u16::from_be_bytes([a, b]),
                    _ => return None,
                };
                ((2..=512).contains(&n) && head.get(2..14)? == first).then_some(0)
            }
        }
    }
}

/// Where a maker note lies: `len` bytes at `pos` in the source, in byte order `le`; `orig` is
/// the offset it had in the file its offsets count from (`pos`, unless it was copied, as into
/// DNGPrivateData).
struct Place {
    pos: u64,
    len: u64,
    orig: u64,
    le: bool,
    /// Still in the file it was written to, so values past it are the file's.
    in_file: bool,
}

/// A maker note rebuilt as a fresh IFD: the header before it, then the IFD.
struct Rebuilt {
    header: Vec<u8>,
    ifd: Ifd,
}

/// A maker note's offset space: offset `o` lies at `o + shift` in the source, and only bytes
/// within `lo..hi` of the source are read.
struct Window<'s, S> {
    s: &'s S,
    shift: i64,
    lo: u64,
    hi: u64,
}

impl<S> Window<'_, S> {
    /// Where `len` bytes at offset `off` lie in the source, when within the window.
    fn place(&self, off: u64, len: u64) -> Option<u64> {
        let p = off.checked_add_signed(self.shift)?;
        (p >= self.lo && p.checked_add(len)? <= self.hi).then_some(p)
    }
}

impl<'a, S: At<'a>> At<'a> for Window<'_, S> {
    fn size(&self) -> u64 {
        self.s.size()
    }
    fn at(&self, off: u64, len: u64) -> Option<Cow<'a, [u8]>> {
        self.s.at(self.place(off, len)?, len)
    }
}

/// What an IFD is read as.
#[derive(Clone, Copy, PartialEq)]
enum Dir {
    /// IFD0, the GPS or the Interop IFD.
    Tiff,
    /// The Exif IFD: its maker note may run to [`MAX_EXIF`].
    Exif,
    /// A maker note's IFD: its tags are the maker's, none of them image layout or sub-IFDs.
    Note,
}

struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    /// Little-endian.
    value: Vec<u8>,
    /// The value's offset in the source, for out-of-line values.
    from: Option<u64>,
}

#[derive(Default)]
struct Ifd {
    entries: Vec<Entry>,
    /// Sub-IFDs, by their pointer tag.
    subs: Vec<(u16, Ifd)>,
    /// Where DNGPrivateData lies in the source, and its length.
    private: Option<(u64, u64)>,
    /// Where an RW2's JpgFromRaw lies in the source, and its length.
    preview: Option<(u64, u64)>,
    /// Where an RW2's CameraIFD lies in the source, and its length.
    camera: Option<(u64, u64)>,
}

impl Ifd {
    fn entry(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.tag == tag)
    }
    fn sub_mut(&mut self, tag: u16) -> Option<&mut Ifd> {
        self.subs.iter_mut().find(|s| s.0 == tag).map(|s| &mut s.1)
    }
}

struct Src<'s, S> {
    s: &'s S,
    le: bool,
}

/// An IFD's layout: classic TIFF (2-byte entry count, 12-byte entries, 4-byte value field) or
/// BigTIFF (8, 20, 8).
#[derive(Clone, Copy)]
struct Layout {
    big: bool,
}

impl Layout {
    const CLASSIC: Layout = Layout { big: false };
    fn count_size(self) -> u64 {
        if self.big { 8 } else { 2 }
    }
    fn entry_size(self) -> usize {
        if self.big { 20 } else { 12 }
    }
    /// The value (or offset) field's size.
    fn field(self) -> usize {
        if self.big { 8 } else { 4 }
    }
}

/// Bytes per value of a field type, and the unit swapped between byte orders (rationals are
/// two LONGs); None for unknown types.
fn type_size(typ: u16) -> Option<(u64, usize)> {
    Some(match typ {
        1 | 2 | 6 | 7 | 129 => (1, 1),
        3 | 8 => (2, 2),
        4 | 9 | 11 | 13 => (4, 4),
        5 | 10 => (8, 4),
        12 | 16 | 17 | 18 => (8, 8),
        _ => return None,
    })
}

impl<'a, S: At<'a>> Src<'_, S> {
    fn u16(&self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.le {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }
    }
    fn u32(&self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    }
    fn u64(&self, b: &[u8]) -> u64 {
        let a: [u8; 8] = b[..8].try_into().unwrap_or_default();
        if self.le {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        }
    }
    /// An offset or count field of `l`'s width.
    fn uint(&self, l: Layout, b: &[u8]) -> u64 {
        if l.big {
            self.u64(b)
        } else {
            u64::from(self.u32(b))
        }
    }

    /// The IFD at `off` and, below `depth`, the sub-IFDs it points to.
    fn ifd(&self, l: Layout, off: u64, depth: u8, dir: Dir) -> Option<Ifd> {
        let count = self.s.at(off, l.count_size())?;
        let n = if l.big {
            self.u64(&count)
        } else {
            u64::from(self.u16(&count))
        }
        .min(MAX_ENTRIES);
        let es = l.entry_size();
        let raw = self.s.at(off + l.count_size(), es as u64 * n)?;
        let mut ifd = Ifd::default();
        let in_note = dir == Dir::Note;
        // Bytes of values other than the maker note held so far: a hostile IFD repeating
        // one large value must not copy it a thousand times.
        let mut kept = 0u64;
        for e in raw.chunks_exact(es) {
            let tag = self.u16(&e[..2]);
            let typ = self.u16(&e[2..4]);
            let (count, field) = if l.big {
                (self.u64(&e[4..12]), &e[12..20])
            } else {
                (u64::from(self.u32(&e[4..8])), &e[8..12])
            };
            if !in_note && tag == DNG_PRIVATE && matches!(typ, 1 | 7) && count > l.field() as u64 {
                ifd.private = Some((self.uint(l, field), count));
            }
            if dir == Dir::Tiff
                && !l.big
                && tag == JPG_FROM_RAW
                && matches!(typ, 1 | 7)
                && count > l.field() as u64
            {
                ifd.preview = Some((self.uint(l, field), count));
            }
            if dir == Dir::Tiff
                && !l.big
                && tag == CAMERA_IFD
                && typ == 7
                && (8..=MAX_VALUE).contains(&count)
            {
                ifd.camera = Some((self.uint(l, field), count));
            }
            if !in_note && DROP.binary_search(&tag).is_ok() {
                continue;
            }
            if !in_note && matches!(tag, EXIF_IFD | GPS_IFD | INTEROP_IFD) {
                let at = match typ {
                    4 | 13 => Some(u64::from(self.u32(field))),
                    16 | 18 if l.big => Some(self.u64(field)),
                    _ => None,
                };
                let sub_dir = if tag == EXIF_IFD {
                    Dir::Exif
                } else {
                    Dir::Tiff
                };
                if depth > 0
                    && count == 1
                    && let Some(at) = at
                    && !ifd.subs.iter().any(|s| s.0 == tag)
                    && let Some(sub) = self.ifd(l, at, depth - 1, sub_dir)
                {
                    ifd.subs.push((tag, sub));
                }
                continue;
            }
            let Some((size, unit)) = type_size(typ) else {
                continue;
            };
            let Some(len) = count.checked_mul(size) else {
                continue;
            };
            let note = dir == Dir::Exif && tag == MAKER_NOTE;
            if len > if note { MAX_EXIF } else { MAX_VALUE } {
                continue;
            }
            // The first of repeated tags is the one kept (as the dedup below does).
            if ifd.entries.iter().any(|e| e.tag == tag) {
                continue;
            }
            if !note && kept + len > MAX_EXIF {
                continue;
            }
            let (mut value, from) = if len <= l.field() as u64 {
                (field[..len as usize].to_vec(), None)
            } else {
                let at = self.uint(l, field);
                match self.s.at(at, len) {
                    Some(v) => (v.into_owned(), Some(at)),
                    None => continue,
                }
            };
            if !note {
                kept += len;
            }
            if !self.le {
                for u in value.chunks_exact_mut(unit) {
                    u.reverse();
                }
            }
            let (typ, value) = narrow(typ, value);
            ifd.entries.push(Entry {
                tag,
                typ,
                // At most MAX_EXIF bytes, so the count fits (narrowing keeps it).
                count: count as u32,
                value,
                from,
            });
        }
        ifd.entries.sort_by_key(|e| e.tag);
        ifd.entries.dedup_by_key(|e| e.tag);
        Some(ifd)
    }
}

/// BigTIFF's LONG8 and IFD8 (16, 18) as LONG, and SLONG8 (17) as SLONG, when every value
/// fits; other types as they are. `value` is little-endian.
fn narrow(typ: u16, value: Vec<u8>) -> (u16, Vec<u8>) {
    let to = match typ {
        16 | 18 => 4,
        17 => 9,
        _ => return (typ, value),
    };
    let mut out = Vec::with_capacity(value.len() / 2);
    for v in value.chunks_exact(8) {
        let x = u64::from_le_bytes(v.try_into().unwrap_or_default());
        let ok = if to == 4 {
            u32::try_from(x).is_ok()
        } else {
            i32::try_from(x as i64).is_ok()
        };
        if !ok {
            return (typ, value);
        }
        out.extend_from_slice(&v[..4]);
    }
    (to, out)
}

/// The compact Exif TIFF structure of a classic or BigTIFF TIFF-based file (or a Panasonic
/// RW2 or Olympus ORF, whose headers differ only in their magic number), or None when it has
/// no descriptive tags (or its IFD0 cannot be read).
///
/// An RW2's IFD0 holds Panasonic's raw settings below tag 254 (sensor size, levels, the
/// JpgFromRaw), left out, and an Exif IFD without the maker note, ISO or lens: when it has no
/// maker note, the structure is that of the JpgFromRaw's Exif (its IFD0, Exif, Interop and
/// GPS IFDs and its maker note, rebuilt from there) with the RW2's own IFD0 and sub-IFD
/// entries over it, less what describes the preview only ([`PREVIEW_ONLY`]).
///
/// Then, as exiftool reads them, the RW2's CameraIFD ([`RawExtras`]) gives the 35 mm focal
/// length over the preview's (a GH5's 1:1 crop: 50 mm, not the preview's 59) and the APEX
/// aperture and shutter speed where the Exif has none, and the raw ISO stands in for a
/// missing ISOSpeedRatings (an RW2 without a usable preview).
pub(crate) fn tiff_exif<'a>(s: &impl At<'a>) -> Option<Vec<u8>> {
    let (mut ifd0, le, magic) = read_ifd0(s)?;
    if magic == RW2_MAGIC {
        let extras = RawExtras::of(s, &ifd0);
        ifd0.entries.retain(|e| e.tag >= 254);
        let has_note = ifd0
            .sub_mut(EXIF_IFD)
            .is_some_and(|x| x.entry(MAKER_NOTE).is_some());
        if !has_note
            && let Some(t) = ifd0.preview.and_then(|p| preview_exif(s, p))
            && let t = &*t
            && let Some((mut pv0, pv_le, _)) = read_ifd0(&t)
        {
            if let Some(x) = pv0.sub_mut(EXIF_IFD) {
                x.entries.retain(|e| !PREVIEW_ONLY.contains(&e.tag));
            }
            overlay(&mut pv0, ifd0);
            extras.apply(&mut pv0);
            return finish(&t, pv_le, pv0);
        }
        extras.apply(&mut ifd0);
    }
    if magic == JXR_MAGIC || magic == JXR_MAGIC_V0 {
        // Pixel format, transformation, size, resolution and the codestream pointers.
        ifd0.entries.retain(|e| !(0xBC00..=0xBCFF).contains(&e.tag));
    }
    finish(s, le, ifd0)
}

/// What an RW2 holds outside its Exif: the PanasonicRaw ISO and the CameraIFD's 35 mm focal
/// length, APEX aperture and shutter speed (exiftool's FocalLengthIn35mmFormat, ApertureValue
/// and ShutterSpeedValue; the last two only where nothing else gives them), whether the flash
/// fired and the white balance set (FlashFired, WhiteBalanceSet: only where the Exif has no
/// Flash or WhiteBalance, which a preview always has). Only these values are taken from the
/// CameraIFD, never the block itself. Its Orientation is not: IFD0 always has one.
#[derive(Default)]
struct RawExtras {
    iso: Option<u32>,
    focal_35: Option<u16>,
    /// APEX values in 1/256 EV.
    aperture: Option<i16>,
    shutter: Option<i16>,
    /// Exif Flash: 1 (fired) or 0 (did not).
    flash: Option<u16>,
    /// Exif WhiteBalance (0 auto, 1 manual) and, for a preset, its Exif LightSource.
    white_balance: Option<(u16, Option<u16>)>,
}

impl RawExtras {
    fn of<'a>(s: &impl At<'a>, ifd0: &Ifd) -> RawExtras {
        let raw = |tag: u16| {
            let e = ifd0.entry(tag)?;
            let b = e.value.as_slice();
            match (e.typ, b.len()) {
                (3, 2..) => Some(u32::from(u16::from_le_bytes([b[0], b[1]]))),
                (4, 4..) => Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                _ => None,
            }
        };
        let mut x = RawExtras {
            iso: raw(RW2_ISO_LONG)
                .filter(|&v| v > 0)
                .or_else(|| raw(RW2_ISO))
                .filter(|&v| v > 0),
            ..RawExtras::default()
        };
        if let Some(c) = ifd0.camera {
            x.camera(s, c);
        }
        x
    }

    /// Reads the CameraIFD of `len` bytes at `at`: a TIFF header and an IFD of inline values,
    /// its own types 0x101 and 0x102 read as LONG (as exiftool does).
    fn camera<'a>(&mut self, s: &impl At<'a>, (at, len): (u64, u64)) -> Option<()> {
        let w = Window {
            s,
            shift: i64::try_from(at).ok()?,
            lo: at,
            hi: at.checked_add(len)?,
        };
        let h = w.at(0, 8)?;
        let src = Src {
            s: &w,
            le: match &h[..4] {
                b"II*\0" => true,
                b"MM\0*" => false,
                _ => return None,
            },
        };
        let ifd = u64::from(src.u32(&h[4..8]));
        let n = u64::from(src.u16(&w.at(ifd, 2)?)).min(MAX_ENTRIES);
        let raw = w.at(ifd + 2, 12 * n)?;
        let (mut lens, mut focal, mut av, mut tv) = (None, None, None, None);
        let (mut flash, mut wb) = (None, None);
        for e in raw.chunks_exact(12) {
            let (tag, typ) = (src.u16(&e[..2]), src.u16(&e[2..4]));
            if src.u32(&e[4..8]) != 1 {
                continue;
            }
            let v = &e[8..12];
            match (tag, typ) {
                // LensAttached: many of the others are void without a lens.
                (0x1200, 4 | 0x101 | 0x102) => lens = Some(src.u32(v)),
                // FocalLengthIn35mmFormat.
                (0x1203, 3) => focal = Some(src.u16(v)),
                // ApertureValue and ShutterSpeedValue, in 1/256 EV.
                (0x1301, 8) => av = Some(src.u16(v) as i16),
                (0x1302, 8) => tv = Some(src.u16(v) as i16),
                // FlashFired: 0 no, 1 yes.
                (0x1102, 3) => flash = Some(u32::from(src.u16(v))),
                (0x1102, 4 | 0x101 | 0x102) => flash = Some(src.u32(v)),
                // WhiteBalanceSet (BYTE).
                (0x3300, 1) => wb = Some(v[0]),
                _ => {}
            }
        }
        let lens = lens != Some(0);
        self.focal_35 = focal.filter(|&f| lens && f > 0);
        self.aperture = av.filter(|&a| lens && a > 0);
        // As exiftool, nothing past ±100 EV (its "no value").
        self.shutter = tv.filter(|&t| i32::from(t).abs() < 100 * 256);
        self.flash = flash.filter(|&f| f <= 1).map(|f| f as u16);
        // Auto and AWBc are auto; the presets give their light source (daylight, cloudy,
        // tungsten, flash, shade); custom and Kelvin settings are manual without one.
        self.white_balance = match wb {
            Some(0 | 16) => Some((0, None)),
            Some(1) => Some((1, Some(1))),
            Some(2) => Some((1, Some(10))),
            Some(3) => Some((1, Some(3))),
            Some(5) => Some((1, Some(4))),
            Some(12) => Some((1, Some(11))),
            Some(8..=11 | 13) => Some((1, None)),
            _ => None,
        };
        Some(())
    }

    /// Lays the values into the Exif IFD of `ifd0` (made when missing, unless `ifd0` holds
    /// nothing else): the 35 mm focal length over the Exif's, the others where it has none.
    fn apply(&self, ifd0: &mut Ifd) {
        let mut add: Vec<(Entry, bool)> = Vec::new();
        let entry = |tag, typ, value: Vec<u8>| Entry {
            tag,
            typ,
            count: 1,
            value,
            from: None,
        };
        if let Some(f) = self.focal_35 {
            add.push((entry(0xA405, 3, f.to_le_bytes().to_vec()), true));
        }
        if let Some(a) = self.aperture {
            let v = [(a as u32).to_le_bytes(), 256u32.to_le_bytes()].concat();
            add.push((entry(0x9202, 5, v), false));
        }
        if let Some(t) = self.shutter {
            let v = [i32::from(t).to_le_bytes(), 256i32.to_le_bytes()].concat();
            add.push((entry(0x9201, 10, v), false));
        }
        if let Some(iso) = self.iso {
            let e = match u16::try_from(iso) {
                Ok(v) => entry(0x8827, 3, v.to_le_bytes().to_vec()),
                Err(_) => entry(0x8827, 4, iso.to_le_bytes().to_vec()),
            };
            add.push((e, false));
        }
        if let Some(f) = self.flash {
            add.push((entry(0x9209, 3, f.to_le_bytes().to_vec()), false));
        }
        let nothing = add.is_empty() && self.white_balance.is_none();
        if nothing || (ifd0.entries.is_empty() && ifd0.subs.is_empty()) {
            return;
        }
        if ifd0.sub_mut(EXIF_IFD).is_none() {
            ifd0.subs.push((EXIF_IFD, Ifd::default()));
        }
        let Some(x) = ifd0.sub_mut(EXIF_IFD) else {
            return;
        };
        // The white balance and its light source together, only where the Exif has neither.
        if let Some((wb, light)) = self.white_balance
            && x.entry(0xA403).is_none()
            && x.entry(0x9208).is_none()
        {
            add.push((entry(0xA403, 3, wb.to_le_bytes().to_vec()), false));
            if let Some(l) = light {
                add.push((entry(0x9208, 3, l.to_le_bytes().to_vec()), false));
            }
        }
        for (e, over) in add {
            match x.entries.iter_mut().find(|b| b.tag == e.tag) {
                Some(b) if over => *b = e,
                Some(_) => {}
                None => x.entries.push(e),
            }
        }
        x.entries.sort_by_key(|e| e.tag);
    }
}

/// IFD0 of a TIFF-based file with its sub-IFDs, its byte order and its magic number.
fn read_ifd0<'a>(s: &impl At<'a>) -> Option<(Ifd, bool, u16)> {
    let h = s.at(0, 8)?;
    let le = match &h[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let src = Src { s, le };
    let magic = src.u16(&h[2..4]);
    let (l, ifd0_at) = match magic {
        42 | RW2_MAGIC | ORF_MAGIC_RO | ORF_MAGIC_RS | JXR_MAGIC | JXR_MAGIC_V0 => {
            (Layout::CLASSIC, u64::from(src.u32(&h[4..8])))
        }
        // BigTIFF: offset size 8, a reserved 0, then the 8-byte offset of IFD0.
        43 if src.u16(&h[4..6]) == 8 && src.u16(&h[6..8]) == 0 => {
            (Layout { big: true }, src.u64(&s.at(8, 8)?))
        }
        _ => return None,
    };
    Some((src.ifd(l, ifd0_at, 2, Dir::Tiff)?, le, magic))
}

/// The structure of `ifd0` read from `s`, its maker note rebuilt when it is one to rebuild.
fn finish<'a>(s: &impl At<'a>, le: bool, mut ifd0: Ifd) -> Option<Vec<u8>> {
    if ifd0.entries.is_empty() && ifd0.subs.is_empty() {
        return None;
    }
    let note = maker_note(s, le, &mut ifd0);
    write(&ifd0, note.as_ref(), true).or_else(|| write(&ifd0, None, false))
}

/// The Exif TIFF structure in the APP1 segment of the JPEG of `len` bytes at `at`.
fn preview_exif<'a, S: At<'a>>(s: &S, (at, len): (u64, u64)) -> Option<Cow<'a, [u8]>> {
    let w = Window {
        s,
        shift: i64::try_from(at).ok()?,
        lo: at,
        hi: at.checked_add(len)?,
    };
    if w.at(0, 2)?.as_ref() != [0xFF, 0xD8] {
        return None;
    }
    crate::exif_locate::jpeg(&w)
}

/// `top`'s entries and sub-IFDs over `base`'s: an entry replaces the one of its tag, a
/// sub-IFD is laid over the one of its tag.
fn overlay(base: &mut Ifd, top: Ifd) {
    for e in top.entries {
        match base.entries.iter_mut().find(|b| b.tag == e.tag) {
            Some(b) => *b = e,
            None => base.entries.push(e),
        }
    }
    base.entries.sort_by_key(|e| e.tag);
    for (tag, sub) in top.subs {
        match base.sub_mut(tag) {
            Some(b) => overlay(b, sub),
            None => base.subs.push((tag, sub)),
        }
    }
}

/// The rebuilt maker note of a Canon, Sony, Panasonic or Samsung file, from the Exif IFD or else from
/// DNGPrivateData. The Exif IFD's maker-note entry is the placeholder [`write`] fills with it:
/// added for a note from DNGPrivateData, removed for a note of the maker's kind that cannot be
/// rebuilt. Other notes are left as they are.
fn maker_note<'a, S: At<'a>>(s: &S, le: bool, ifd0: &mut Ifd) -> Option<Rebuilt> {
    let maker = Maker::of(&ifd0.entry(MAKE)?.value)?;
    let private = ifd0.private;
    let blob = ifd0
        .sub_mut(EXIF_IFD)
        .and_then(|x| x.entry(MAKER_NOTE))
        .map(|e| (e.from, e.value.len() as u64));
    match blob {
        Some((Some(pos), len)) => {
            let place = Place {
                pos,
                len,
                orig: pos,
                le,
                in_file: true,
            };
            let note = rebuild(s, maker, &place)?;
            if note.is_none()
                && let Some(x) = ifd0.sub_mut(EXIF_IFD)
            {
                x.entries.retain(|e| e.tag != MAKER_NOTE);
            }
            note
        }
        // Inline (four bytes at most): not a note.
        Some((None, _)) => None,
        None => {
            let note = rebuild(s, maker, &makn(s, private?)?)??;
            if ifd0.sub_mut(EXIF_IFD).is_none() {
                ifd0.subs.push((EXIF_IFD, Ifd::default()));
            }
            let x = ifd0.sub_mut(EXIF_IFD)?;
            x.entries.push(Entry {
                tag: MAKER_NOTE,
                typ: 7,
                count: 0,
                value: vec![0; 8],
                from: None,
            });
            x.entries.sort_by_key(|e| e.tag);
            Some(note)
        }
    }
}

/// The note at `p` rebuilt with what `maker` keeps. None when it is not a note of `maker`'s
/// kind; Some(None) when it is but cannot be read whole, or keeps nothing.
fn rebuild<'a, S: At<'a>>(s: &S, maker: Maker, p: &Place) -> Option<Option<Rebuilt>> {
    let head = s.at(p.pos, p.len.min(16))?;
    let h = maker.header(&head, p.le)?;
    let (Ok(pos), Ok(orig)) = (i64::try_from(p.pos), i64::try_from(p.orig)) else {
        return Some(None);
    };
    let (lo, hi) = (p.pos, p.pos.saturating_add(p.len));
    // Offsets from the (original) TIFF header.
    let header_rel = Window {
        s,
        shift: pos - orig,
        lo,
        hi,
    };
    let ifd = if maker == Maker::Canon {
        // An IFD whose entries lie within the note, else not a note to read. Its values may
        // lie past the note in its own file.
        let n = match head.get(..2) {
            Some(&[a, b]) if p.le => u16::from_le_bytes([a, b]),
            Some(&[a, b]) => u16::from_be_bytes([a, b]),
            _ => 0,
        };
        let w = if p.in_file {
            Window {
                s,
                shift: 0,
                lo: 0,
                hi: u64::MAX,
            }
        } else {
            header_rel
        };
        (2 + 12 * u64::from(n) <= p.len)
            .then(|| Src { s: &w, le: p.le }.ifd(Layout::CLASSIC, p.orig, 0, Dir::Note))
            .flatten()
    } else {
        // Offsets from the note's start.
        let self_rel = Window {
            s,
            shift: pos,
            lo,
            hi,
        };
        // Both bases may read whole when the note's values lie well inside it (Samsung's
        // self-relative note holds its preview); then the one whose values start right after
        // the IFD, as a camera writes them.
        let mut whole: Vec<_> = [(header_rel, p.orig + h), (self_rel, h)]
            .into_iter()
            .filter(|(w, at)| intact(w, *at, p.le))
            .collect();
        if let Some(k) = whole.iter().position(|(w, at)| values_follow(w, *at, p.le)) {
            whole.swap(0, k);
        }
        whole
            .into_iter()
            .next()
            .and_then(|(w, at)| Src { s: &w, le: p.le }.ifd(Layout::CLASSIC, at, 0, Dir::Note))
    };
    let Some(mut ifd) = ifd else {
        return Some(None);
    };
    ifd.entries.retain(|e| maker.keeps(e.tag));
    if !p.le && maker != Maker::Canon {
        // Typed values were swapped as they were read; a block of bytes keeps the order it
        // was written in, so only those known to hold single bytes or 16-bit words stay.
        ifd.entries
            .retain_mut(|e| e.typ != 7 || maker.swap_block(e));
    }
    if ifd.entries.is_empty() {
        return Some(None);
    }
    let header = s.at(p.pos, h)?.into_owned();
    Some(Some(Rebuilt { header, ifd }))
}

/// Whether the IFD at `at` (byte order `le`) lies within `w` with all its values, every
/// entry of a known type: a note read from the right base, not one whose offsets point
/// elsewhere.
fn intact<'a, S: At<'a>>(w: &Window<'_, S>, at: u64, le: bool) -> bool {
    let src = Src { s: w, le };
    let u32f = |b: &[u8]| u64::from(src.u32(b));
    let n = match w.at(at, 2) {
        Some(c) => u64::from(src.u16(&c)),
        None => return false,
    };
    if n == 0 || n > MAX_ENTRIES {
        return false;
    }
    let Some(raw) = w.at(at + 2, 12 * n) else {
        return false;
    };
    raw.chunks_exact(12).all(|e| {
        let Some((size, _)) = type_size(src.u16(&e[2..4])) else {
            return false;
        };
        let len = size * u32f(&e[4..8]);
        len <= 4 || w.place(u32f(&e[8..12]), len).is_some()
    })
}

/// Whether the first out-of-line value of the IFD at `at` (byte order `le`) lies right after
/// it (its entries and the next-IFD offset), read through `w`.
fn values_follow<'a, S: At<'a>>(w: &Window<'_, S>, at: u64, le: bool) -> bool {
    let src = Src { s: w, le };
    let Some(n) = w.at(at, 2).map(|c| u64::from(src.u16(&c))) else {
        return false;
    };
    let Some(raw) = w.at(at + 2, 12 * n.min(MAX_ENTRIES)) else {
        return false;
    };
    let first = raw
        .chunks_exact(12)
        .filter_map(|e| {
            let (size, _) = type_size(src.u16(&e[2..4]))?;
            (size * u64::from(src.u32(&e[4..8])) > 4).then(|| u64::from(src.u32(&e[8..12])))
        })
        .min();
    first == Some(at + 2 + 12 * n + 4)
}

/// The maker note Adobe DNG Converter copied into DNGPrivateData (`at`, `len` bytes):
/// "Adobe\0", then chunks of a four-character type and a big-endian length; "MakN" holds the
/// note's byte order, its big-endian offset in the original file, then the note.
fn makn<'a, S: At<'a>>(s: &S, (at, len): (u64, u64)) -> Option<Place> {
    let end = at.checked_add(len)?;
    if s.at(at, 6)?.as_ref() != b"Adobe\0" {
        return None;
    }
    let mut c = at + 6;
    for _ in 0..MAX_CHUNKS {
        let body = c.checked_add(8).filter(|&b| b <= end)?;
        let h = s.at(c, 8)?;
        let n = u64::from(u32::from_be_bytes([h[4], h[5], h[6], h[7]]));
        let next = body.checked_add(n).filter(|&e| e <= end)?;
        if &h[..4] == b"MakN" {
            let b = s.at(body, 6).filter(|_| n > 6)?;
            let le = match &b[..2] {
                b"II" => true,
                b"MM" => false,
                _ => return None,
            };
            return Some(Place {
                pos: body + 6,
                len: n - 6,
                orig: u64::from(u32::from_be_bytes([b[2], b[3], b[4], b[5]])),
                le,
                in_file: false,
            });
        }
        c = next;
    }
    None
}

/// Writes the structure, with or without the maker note (`rebuilt` replacing its blob); None
/// past [`MAX_EXIF`], or when a header-relative note would have to move.
fn write(ifd0: &Ifd, rebuilt: Option<&Rebuilt>, maker_note: bool) -> Option<Vec<u8>> {
    let mut out = b"II*\0\x08\0\0\0".to_vec();
    let mut note = None;
    write_ifd(&mut out, ifd0, maker_note, &mut note);
    if let Some((field, e)) = note {
        if out.len() % 2 == 1 {
            out.push(0);
        }
        let end = out.len() as u64;
        if let Some(r) = rebuilt {
            // Its header, then a fresh IFD after the rest, its offsets from this header.
            out.extend_from_slice(&r.header);
            write_ifd(&mut out, &r.ifd, false, &mut None);
            let len = u32::try_from(out.len() as u64 - end).ok()?;
            out[field - 4..field].copy_from_slice(&len.to_le_bytes());
            patch(&mut out, field, end);
        } else {
            let len = e.value.len() as u64;
            // At its original offset when that is free, so offsets from the header still hold;
            // else moved only if its offsets do not count from the header.
            let at = match e.from.filter(|&f| f >= end && f + len <= MAX_EXIF) {
                Some(f) => f,
                None if SELF_CONTAINED.iter().any(|p| e.value.starts_with(p)) => end,
                None => return None,
            };
            out.resize(usize::try_from(at).ok()?, 0);
            out.extend_from_slice(&e.value);
            patch(&mut out, field, at);
        }
    }
    (out.len() as u64 <= MAX_EXIF).then_some(out)
}

fn patch(out: &mut [u8], field: usize, v: u64) {
    out[field..field + 4].copy_from_slice(&(v as u32).to_le_bytes());
}

/// Appends `ifd` (sorted entries, values after it, word-aligned, no next IFD), then its
/// sub-IFDs. The maker note's value is left for the caller to place (`note`).
fn write_ifd<'i>(
    out: &mut Vec<u8>,
    ifd: &'i Ifd,
    maker_note: bool,
    note: &mut Option<(usize, &'i Entry)>,
) {
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let entries: Vec<&Entry> = ifd
        .entries
        .iter()
        .filter(|e| maker_note || e.tag != MAKER_NOTE)
        .collect();
    // Entries in tag order, sub-IFD pointers among them.
    let mut rows: Vec<(u16, Option<&Entry>)> = entries.iter().map(|e| (e.tag, Some(*e))).collect();
    rows.extend(ifd.subs.iter().map(|s| (s.0, None)));
    rows.sort_by_key(|r| r.0);
    let start = out.len();
    let base = start + 2 + 12 * rows.len() + 4;
    let mut tail = Vec::new();
    let mut sub_fields = Vec::new();
    out.extend_from_slice(&(rows.len() as u16).to_le_bytes());
    for (tag, e) in rows {
        out.extend_from_slice(&tag.to_le_bytes());
        let Some(e) = e else {
            out.extend_from_slice(&4u16.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes());
            sub_fields.push((tag, out.len()));
            out.extend_from_slice(&[0; 4]);
            continue;
        };
        out.extend_from_slice(&e.typ.to_le_bytes());
        out.extend_from_slice(&e.count.to_le_bytes());
        if maker_note && e.tag == MAKER_NOTE && e.value.len() > 4 {
            *note = Some((out.len(), e));
            out.extend_from_slice(&[0; 4]);
        } else if e.value.len() <= 4 {
            let mut v = [0; 4];
            v[..e.value.len()].copy_from_slice(&e.value);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&((base + tail.len()) as u32).to_le_bytes());
            tail.extend_from_slice(&e.value);
            if tail.len() % 2 == 1 {
                tail.push(0);
            }
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&tail);
    for (tag, field) in sub_fields {
        let Some((_, sub)) = ifd.subs.iter().find(|s| s.0 == tag) else {
            continue;
        };
        if out.len() % 2 == 1 {
            out.push(0);
        }
        let at = out.len() as u64;
        patch(out, field, at);
        write_ifd(out, sub, maker_note, note);
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    /// A classic or BigTIFF writer in either byte order: values are encoded in file order.
    struct W {
        le: bool,
        big: bool,
        b: Vec<u8>,
    }

    type E = (u16, u16, u32, Vec<u8>);

    impl W {
        /// A header whose IFD0 offset is set by [`W::ifd0`].
        fn new(le: bool, big: bool) -> W {
            let mut w = W {
                le,
                big,
                b: match (le, big) {
                    (true, false) => b"II*\0".to_vec(),
                    (false, false) => b"MM\0*".to_vec(),
                    (true, true) => b"II+\0".to_vec(),
                    (false, true) => b"MM\0+".to_vec(),
                },
            };
            if big {
                w.put16(8);
                w.put16(0);
            }
            w.put_off(0);
            w
        }
        fn ifd0(&mut self, at: u32) {
            self.set_off(if self.big { 8 } else { 4 }, at.into());
        }
        fn short(&self, v: &[u16]) -> Vec<u8> {
            v.iter()
                .flat_map(|x| {
                    if self.le {
                        x.to_le_bytes()
                    } else {
                        x.to_be_bytes()
                    }
                })
                .collect()
        }
        fn long(&self, v: &[u32]) -> Vec<u8> {
            v.iter()
                .flat_map(|x| {
                    if self.le {
                        x.to_le_bytes()
                    } else {
                        x.to_be_bytes()
                    }
                })
                .collect()
        }
        /// Offsets and counts: 4 bytes, or 8 in a BigTIFF.
        fn off(&self, v: &[u64]) -> Vec<u8> {
            v.iter()
                .flat_map(|&x| match (self.big, self.le) {
                    (true, true) => x.to_le_bytes().to_vec(),
                    (true, false) => x.to_be_bytes().to_vec(),
                    (false, _) => self.long(&[x as u32]),
                })
                .collect()
        }
        /// A sub-IFD pointer entry's type and value: LONG, or IFD8 in a BigTIFF.
        fn ptr(&self, tag: u16, at: u32) -> E {
            (
                tag,
                if self.big { 18 } else { 4 },
                1,
                self.off(&[at.into()]),
            )
        }
        fn rat(&self, v: &[(u32, u32)]) -> Vec<u8> {
            v.iter().flat_map(|&(a, b)| self.long(&[a, b])).collect()
        }
        fn put16(&mut self, v: u16) {
            let b = self.short(&[v]);
            self.b.extend(b);
        }
        fn put_off(&mut self, v: u64) {
            let b = self.off(&[v]);
            self.b.extend(b);
        }
        fn set_off(&mut self, at: usize, v: u64) {
            let b = self.off(&[v]);
            self.b[at..at + b.len()].copy_from_slice(&b);
        }
        /// The offset of the value field of entry `k` of the IFD at `ifd`.
        fn field(&self, ifd: u32, k: usize) -> usize {
            let (cs, es, f) = if self.big { (8, 20, 12) } else { (2, 12, 8) };
            ifd as usize + cs + es * k + f
        }
        /// Appends an IFD (values after it) whose next-IFD link is `next`; its offset.
        fn ifd(&mut self, entries: Vec<E>, next: u32) -> u32 {
            if self.b.len() % 2 == 1 {
                self.b.push(0);
            }
            let (cs, es, fs) = if self.big { (8, 20, 8) } else { (2, 12, 4) };
            let start = self.b.len();
            let mut data = start + cs + es * entries.len() + fs;
            let mut tail = Vec::new();
            if self.big {
                self.put_off(entries.len() as u64);
            } else {
                self.put16(entries.len() as u16);
            }
            for (tag, typ, count, mut v) in entries {
                self.put16(tag);
                self.put16(typ);
                self.put_off(count.into());
                if v.len() <= fs {
                    v.resize(fs, 0);
                    self.b.extend(v);
                } else {
                    self.put_off(data as u64);
                    data += v.len() + v.len() % 2;
                    tail.extend_from_slice(&v);
                    if v.len() % 2 == 1 {
                        tail.push(0);
                    }
                }
            }
            self.put_off(next.into());
            self.b.extend(tail);
            start as u32
        }
    }

    fn ascii(s: &str) -> (u16, u32, Vec<u8>) {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        (2, v.len() as u32, v)
    }

    const NOTE: &[u8] = b"Unknown note, offsets from the TIFF header.";
    const NIKON: &[u8] = b"Nikon\0\x02\x10\0\0MM\0*\0\0\0\x08\0\0\0\0\0\0";
    const LENS: &str = "EF-S17-85mm f/4-5.6 IS USM";
    const SERIAL: &str = "NOTE-SERIAL-0042";

    /// CameraSettings of the fixture Canon note.
    fn settings() -> Vec<u16> {
        (1..=40).collect()
    }

    /// A Canon maker note at `at` in a file of byte order `le`: an IFD with offsets from the
    /// TIFF header (CameraSettings, a serial, the lens, another serial).
    fn canon(le: bool, at: usize) -> Vec<u8> {
        let mut w = W {
            le,
            big: false,
            b: vec![0; at],
        };
        let (lens, serial) = (ascii(LENS), ascii(SERIAL));
        let e = vec![
            (0x0001, 3, 40, w.short(&settings())),
            (0x000C, 4, 1, w.long(&[0x0BAD_F00D])),
            (0x0095, lens.0, lens.1, lens.2),
            (0x0096, serial.0, serial.1, serial.2),
        ];
        w.ifd(e, 0);
        w.b.split_off(at)
    }

    /// A DNG-like file of `make`: image data, a maker note at `note_at`, the GPS, Interop and
    /// Exif IFDs, a second IFD (a preview) and IFD0 with layout tags, XMP, SubIFDs, a DNG tag
    /// and an oversized profile table. In a BigTIFF, PixelXDimension is a LONG8 and the
    /// pointers are IFD8s.
    fn file(le: bool, big: bool, make: &str, note: &[u8], note_at: usize) -> Vec<u8> {
        file_with(le, big, make, Some(note), note_at, vec![])
    }

    /// [`file`], with or without a maker note, with `extra` entries in IFD0.
    fn file_with(
        le: bool,
        big: bool,
        make: &str,
        note: Option<&[u8]>,
        note_at: usize,
        extra: Vec<E>,
    ) -> Vec<u8> {
        let mut w = W::new(le, big);
        w.b.resize(note_at, 0xAB);
        w.b.extend_from_slice(note.unwrap_or_default());
        let a = ascii("N");
        let e = ascii("E");
        let gps = vec![
            (0, 1, 4, vec![2, 3, 0, 0]),
            (1, a.0, a.1, a.2),
            (2, 5, 3, w.rat(&[(10, 1), (0, 1), (0, 1)])),
            (3, e.0, e.1, e.2),
            (4, 5, 3, w.rat(&[(20, 1), (0, 1), (0, 1)])),
        ];
        let gps = w.ifd(gps, 0);
        let r98 = ascii("R98");
        let interop = w.ifd(vec![(1, r98.0, r98.1, r98.2)], 0);
        let date = ascii("2021:07:14 09:26:53");
        let width = if big {
            (0xA002, 16, 1, w.off(&[640]))
        } else {
            (0xA002, 4, 1, w.long(&[640]))
        };
        let mut exif = vec![
            (0x829A, 5, 1, w.rat(&[(1, 250)])),
            (0x9003, date.0, date.1, date.2),
            width,
            w.ptr(0xA005, interop),
        ];
        if let Some(note) = note {
            exif.insert(2, (0x927C, 7, note.len() as u32, vec![0; 9])); // patched below
        }
        let exif = w.ifd(exif, 0);
        if note.is_some() {
            let field = w.field(exif, 2);
            w.set_off(field, note_at as u64);
        }
        let other = ascii("Other");
        let preview = vec![(256, 3, 1, w.short(&[8])), (271, other.0, other.1, other.2)];
        let ifd1 = w.ifd(preview, 0);
        let (mk, md) = (ascii(make), ascii("Fixture Cam"));
        let mut ifd0 = vec![
            (256, 3, 1, w.short(&[16])),
            (257, 3, 1, w.short(&[12])),
            (258, 3, 3, w.short(&[16, 16, 16])),
            (259, 3, 1, w.short(&[1])),
            (262, 3, 1, w.short(&[2])),
            (271, mk.0, mk.1, mk.2),
            (272, md.0, md.1, md.2),
            (273, 4, 1, w.long(&[8])),
            (274, 3, 1, w.short(&[6])),
            (279, 4, 1, w.long(&[100])),
            (282, 5, 1, w.rat(&[(300, 1)])),
            w.ptr(330, ifd1),
            (700, 1, 9, b"<x:xmp/>\0".to_vec()),
            w.ptr(0x8769, exif),
            w.ptr(0x8825, gps),
            (0xC612, 1, 4, vec![1, 4, 0, 0]),
            (0xC6FA, 11, 20000, vec![0; 80000]),
        ];
        ifd0.extend(extra);
        ifd0.sort_by_key(|e| e.0);
        let ifd0 = w.ifd(ifd0, ifd1);
        w.ifd0(ifd0);
        w.b
    }

    fn le16(t: &[u8], o: usize) -> u16 {
        u16::from_le_bytes([t[o], t[o + 1]])
    }
    fn le32(t: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(t[o..o + 4].try_into().unwrap())
    }
    /// The tags of the (little-endian) IFD at `o`, each with its value field's offset, and
    /// the next-IFD link.
    fn ifd(t: &[u8], o: usize) -> (Vec<(u16, usize)>, u32) {
        let n = usize::from(le16(t, o));
        let tags = (0..n)
            .map(|k| (le16(t, o + 2 + 12 * k), o + 2 + 12 * k + 8))
            .collect();
        (tags, le32(t, o + 2 + 12 * n))
    }
    fn tags(t: &[u8], o: usize) -> Vec<u16> {
        ifd(t, o).0.into_iter().map(|x| x.0).collect()
    }
    fn field(t: &[u8], o: usize, tag: u16) -> usize {
        ifd(t, o).0.into_iter().find(|x| x.0 == tag).unwrap().1
    }
    fn sub(t: &[u8], o: usize, tag: u16) -> usize {
        le32(t, field(t, o, tag)) as usize
    }

    fn both(f: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        (
            crate::exif_tiff(f).map(Cow::into_owned),
            crate::read_exif_tiff(std::io::Cursor::new(f)),
        )
    }

    /// The structure of `file(…)` in all four layouts, which must agree.
    fn layouts(make: &str, note: &dyn Fn(bool) -> Vec<u8>, note_at: usize) -> Vec<u8> {
        let mut outs = Vec::new();
        for big in [false, true] {
            for le in [true, false] {
                let f = file(le, big, make, &note(le), note_at);
                let (t, read) = both(&f);
                let t = t.expect("exif");
                assert_eq!(read.as_ref(), Some(&t), "reader and memory differ");
                outs.push(t);
            }
        }
        // Every byte order and layout gives the same little-endian classic structure.
        assert!(outs.iter().all(|t| *t == outs[0]), "{make} at {note_at}");
        outs.swap_remove(0)
    }

    #[test]
    fn ifd0_exif_interop_and_gps_without_the_image_layout() {
        for note_at in [100, 3000] {
            let t = layouts("Specular", &|_| NOTE.to_vec(), note_at);
            assert!(t.starts_with(b"II*\0\x08\0\0\0"));
            let (_, next) = ifd(&t, 8);
            assert_eq!(next, 0, "no IFD after IFD0");
            assert_eq!(tags(&t, 8), [271, 272, 274, 282, 0x8769, 0x8825, 0xC612]);
            let exif = sub(&t, 8, 0x8769);
            // A note of unknown kind is kept only where it was: its offsets may count from
            // the header, and moved they would point elsewhere.
            let kept = note_at == 3000;
            let mut want = vec![0x829A, 0x9003, 0x927C, 0xA002, 0xA005];
            if !kept {
                want.remove(2);
            }
            assert_eq!(tags(&t, exif), want);
            if kept {
                let at = sub(&t, exif, 0x927C);
                assert_eq!(at, note_at);
                assert_eq!(&t[at..at + NOTE.len()], NOTE);
            }
            // LONG8 narrowed to LONG.
            let w = field(&t, exif, 0xA002);
            assert_eq!((le16(&t, w - 6), le32(&t, w)), (4, 640));
            assert_eq!(tags(&t, sub(&t, exif, 0xA005)), [1]);
            assert_eq!(tags(&t, sub(&t, 8, 0x8825)), [0, 1, 2, 3, 4]);
            // Read like a JPEG's Exif.
            let (cam, gps) = crate::exif_facts(&t).unwrap();
            assert_eq!(cam.camera, "Specular Fixture Cam");
            assert_eq!(cam.exposure_time, Some(1.0 / 250.0));
            assert!(gps);
            assert_eq!(crate::exif_position(&t), Some((10.0, 20.0)));
            assert_eq!(
                crate::exif_date_taken(&t).as_deref(),
                Some("2021:07:14 09:26:53")
            );
            assert_eq!(crate::exif_orientation(&t), Some(6));
            assert!(t.len() < 400 + if kept { 3000 } else { 0 });
        }
    }

    /// A note whose offsets count from itself is moved after the rest when its place is taken.
    #[test]
    fn self_contained_notes_move() {
        for note_at in [100, 3000] {
            let t = layouts("Nikon", &|_| NIKON.to_vec(), note_at);
            let exif = sub(&t, 8, 0x8769);
            let at = sub(&t, exif, 0x927C);
            assert_eq!(&t[at..at + NIKON.len()], NIKON);
            assert_eq!(at == note_at, note_at == 3000, "note at {at}");
            assert_eq!(t.len(), at + NIKON.len());
        }
    }

    /// A Canon note is rebuilt after the rest wherever it was, its offsets from the new
    /// header, with only the tags read (no serial numbers).
    #[test]
    fn canon_notes_are_rebuilt() {
        for note_at in [100, 3000] {
            let t = layouts("Canon", &|le| canon(le, note_at), note_at);
            let exif = sub(&t, 8, 0x8769);
            let f = field(&t, exif, 0x927C);
            let at = le32(&t, f) as usize;
            assert_eq!(
                le32(&t, f - 4) as usize,
                t.len() - at,
                "count is the note's size"
            );
            assert!(t.len() < 700, "{} bytes", t.len());
            assert_eq!(tags(&t, at), [0x0001, 0x0095]);
            let cs = le32(&t, field(&t, at, 1)) as usize;
            let got: Vec<u16> = (0..40).map(|k| le16(&t, cs + 2 * k)).collect();
            assert_eq!(got, settings());
            let lens = le32(&t, field(&t, at, 0x95)) as usize;
            assert_eq!(&t[lens..lens + LENS.len()], LENS.as_bytes());
            let has = |n: &[u8]| t.windows(n.len()).any(|w| w == n);
            assert!(!has(SERIAL.as_bytes()));
            assert!(!has(&0x0BAD_F00Du32.to_le_bytes()));
            assert_eq!(crate::exif_facts(&t).unwrap().0.camera, "Canon Fixture Cam");
        }
        // A Canon note that is not an IFD is dropped, not passed on.
        let t = layouts("Canon", &|_| NOTE.to_vec(), 100);
        assert!(!tags(&t, sub(&t, 8, 0x8769)).contains(&0x927C));
    }

    #[test]
    fn truncated_files_give_what_they_hold() {
        let cases = [
            file(false, false, "Specular", NOTE, 100),
            file(true, true, "Specular", NOTE, 3000),
            file(false, true, "Canon", &canon(false, 100), 100),
            file(false, false, "Canon", &canon(false, 3000), 3000),
            file(true, false, "SONY", &sony(true, 100, SONY_DSC, false), 100),
            file(true, true, "SONY", &sony(true, 3000, b"", false), 3000),
            file(
                false,
                false,
                "SONY",
                &sony(false, 100, SONY_DSC, false),
                100,
            ),
            file(false, false, "Panasonic", &panasonic(false, 100), 100),
            rw2_with(Some(&rw2_preview_tiff()), true, vec![camera_ifd(false, 1)]),
            file_with(
                false,
                false,
                "Canon",
                None,
                100,
                vec![{
                    let p = adobe(true, 996);
                    (0xC634, 7, p.len() as u32, p)
                }],
            ),
        ];
        for f in cases {
            for n in 0..f.len() {
                let (a, b) = both(&f[..n]);
                assert_eq!(a, b, "cut at {n}");
            }
        }
        // Not a TIFF, BigTIFF with IFD0 at 0 or another offset size, an IFD0 of layout tags
        // only.
        assert_eq!(both(b"II*\0\x08\0\0\0\0\0\0\0\0\0"), (None, None));
        assert_eq!(both(b"II+\0\x08\0\0\0\0\0\0\0\0\0\0\0"), (None, None));
        let mut f = file(true, true, "Specular", NOTE, 100);
        assert!(both(&f).0.is_some());
        f[4] = 4;
        assert_eq!(both(&f), (None, None));
        for big in [false, true] {
            let mut w = W::new(true, big);
            let e = vec![(256, 3, 1, w.short(&[1])), (273, 4, 1, w.long(&[0]))];
            let at = w.ifd(e, 0);
            w.ifd0(at);
            assert_eq!(both(&w.b), (None, None));
        }
    }

    /// A sub-IFD pointing back at IFD0 is followed only to the fixed depth.
    #[test]
    fn looping_pointers_end() {
        for big in [false, true] {
            let mut w = W::new(true, big);
            let at = if big { 16 } else { 8 };
            let e = vec![w.ptr(0x8769, at), w.ptr(0xA005, at)];
            w.ifd(e, at);
            w.ifd0(at);
            let t = both(&w.b).0.expect("pointers only");
            assert!(t.len() < 200);
        }
    }

    const SONY_DSC: &[u8] = b"SONY DSC \0\0\0";
    const KEPT: [u8; 40] = *b"Sony kept block, forty bytes of content.";

    /// A Sony note of byte order `le` at `at` after `header` (none in an ARW), its offsets from the TIFF header
    /// or, `self_rel`, from the note itself: kept entries, a serial number and the enciphered
    /// block.
    fn sony(le: bool, at: usize, header: &[u8], self_rel: bool) -> Vec<u8> {
        let base = if self_rel { 0 } else { at };
        let mut w = W {
            le,
            big: false,
            b: vec![0; base],
        };
        w.b.extend_from_slice(header);
        let serial = ascii(SERIAL);
        let e = vec![
            (0x0102, 4, 1, w.long(&[2])),
            (0x0104, 10, 1, w.long(&[u32::MAX, 3])),
            (0x2031, serial.0, serial.1, serial.2),
            (0x9050, 7, 64, vec![0x5A; 64]),
            (0x9400, 7, 40, KEPT.to_vec()),
            (0xB027, 4, 1, w.long(&[32830])),
        ];
        w.ifd(e, 0);
        w.b.split_off(base)
    }

    /// The rebuilt note of `t` checked against `header` and `want` tags: where it lies.
    fn rebuilt(t: &[u8], header: &[u8], want: &[u16]) -> usize {
        let exif = sub(t, 8, 0x8769);
        let f = field(t, exif, 0x927C);
        let at = le32(t, f) as usize;
        assert_eq!(
            le32(t, f - 4) as usize,
            t.len() - at,
            "count is the note's size"
        );
        assert_eq!(&t[at..at + header.len()], header);
        assert_eq!(tags(t, at + header.len()), want);
        let has = |n: &[u8]| t.windows(n.len()).any(|w| w == n);
        assert!(!has(SERIAL.as_bytes()));
        at + header.len()
    }

    /// A Sony note is rebuilt after the rest, with or without its header and whichever base
    /// its offsets count from, its offsets from the new header and without the serials.
    #[test]
    fn sony_notes_are_rebuilt() {
        let kinds: [(&[u8], bool); 3] = [(b"", false), (SONY_DSC, false), (SONY_DSC, true)];
        for note_at in [100, 3000] {
            for (header, self_rel) in kinds {
                let mut outs = Vec::new();
                for big in [false, true] {
                    let f = file(
                        true,
                        big,
                        "SONY",
                        &sony(true, note_at, header, self_rel),
                        note_at,
                    );
                    let (t, read) = both(&f);
                    assert_eq!(t, read, "reader and memory differ");
                    outs.push(t.expect("exif"));
                }
                assert_eq!(outs[0], outs[1]);
                let t = &outs[0];
                let ifd = rebuilt(t, header, &[0x0102, 0x0104, 0x9400, 0xB027]);
                let k = le32(t, field(t, ifd, 0x9400)) as usize;
                assert_eq!(&t[k..k + 40], KEPT);
                let r = le32(t, field(t, ifd, 0x0104)) as usize;
                assert_eq!((le32(t, r), le32(t, r + 4)), (u32::MAX, 3));
                assert!(!t.windows(64).any(|w| w == [0x5A; 64]));
                assert!(t.len() < 600, "{} bytes", t.len());
            }
        }
    }

    /// A big-endian Sony note is swapped into the little-endian IFD value by value, without
    /// its blocks (whose inner order would no longer match).
    #[test]
    fn big_endian_sony_notes_are_swapped() {
        let kinds: [(&[u8], bool); 3] = [(b"", false), (SONY_DSC, false), (SONY_DSC, true)];
        for note_at in [100, 3000] {
            for (header, self_rel) in kinds {
                let note = sony(false, note_at, header, self_rel);
                let (t, read) = both(&file(false, false, "SONY", &note, note_at));
                assert_eq!(t, read, "reader and memory differ");
                let t = t.expect("exif");
                let ifd = rebuilt(&t, header, &[0x0102, 0x0104, 0xB027]);
                assert_eq!(le32(&t, field(&t, ifd, 0x0102)), 2);
                assert_eq!(le32(&t, field(&t, ifd, 0xB027)), 32830);
                let r = le32(&t, field(&t, ifd, 0x0104)) as usize;
                assert_eq!((le32(&t, r), le32(&t, r + 4)), (u32::MAX, 3));
                assert!(!t.windows(40).any(|w| w == KEPT));
            }
        }
    }

    /// A Sony note that cannot be read whole is dropped wherever it was rather than passed on
    /// with offsets that no longer hold.
    #[test]
    fn broken_sony_notes_are_dropped() {
        for note_at in [100, 3000] {
            let mut cut = sony(true, note_at, SONY_DSC, false);
            cut.truncate(cut.len() - 30);
            let mut bad_type = sony(true, note_at, b"", false);
            bad_type[2 + 12 + 2] = 99;
            let mut cut_be = sony(false, note_at, b"", false);
            cut_be.truncate(cut_be.len() - 30);
            for (le, note) in [(true, cut), (true, bad_type), (false, cut_be)] {
                let t = both(&file(le, false, "SONY", &note, note_at)).0.unwrap();
                assert!(
                    !tags(&t, sub(&t, 8, 0x8769)).contains(&0x927C),
                    "at {note_at}"
                );
                assert!(t.len() < 400);
            }
        }
    }

    /// SensorAreas of the fixture Samsung note.
    const SAMSUNG_AREAS: [u32; 8] = [0, 0, 6496, 4336, 0, 0, 6480, 4320];

    /// A Samsung "Type 2" note of byte order `le` at `at` (no header), its offsets from the
    /// TIFF header or, `self_rel`, from the note itself: kept entries, two serials, a
    /// PreviewIFD pointer and an undocumented block, then 600 bytes of preview inside the note
    /// (so both bases may read it whole).
    fn samsung(le: bool, at: usize, self_rel: bool) -> Vec<u8> {
        let base = if self_rel { 0 } else { at };
        let mut w = W {
            le,
            big: false,
            b: vec![0; base],
        };
        let (serial, fw) = (ascii(SERIAL), ascii("1.40"));
        let e = vec![
            (0x0001, 7, 4, b"0100".to_vec()),
            (0x0003, 4, 1, w.long(&[0x0500_1038])),
            (0x0035, 4, 1, w.long(&[400])),
            (0x0060, 7, 24, vec![0x77; 24]),
            (0xA001, fw.0, fw.1, fw.2),
            (0xA002, serial.0, serial.1, serial.2.clone()),
            (0xA003, 3, 1, w.short(&[13])),
            (0xA005, serial.0, serial.1, serial.2),
            (0xA010, 4, 8, w.long(&SAMSUNG_AREAS)),
        ];
        w.ifd(e, 0);
        w.b.extend([0x77; 600]);
        w.b.split_off(base)
    }

    /// A Samsung note is rebuilt after the rest, whichever base its offsets count from, with
    /// only the kept entries (values swapped from big-endian, offsets from the new header):
    /// no serials, preview pointer or blocks.
    #[test]
    fn samsung_notes_are_rebuilt() {
        for le in [true, false] {
            for note_at in [100, 3000] {
                for self_rel in [true, false] {
                    let f = file(
                        le,
                        false,
                        "SAMSUNG",
                        &samsung(le, note_at, self_rel),
                        note_at,
                    );
                    let (t, read) = both(&f);
                    let t = t.expect("exif");
                    assert_eq!(read.as_ref(), Some(&t), "reader and memory differ");
                    let ifd = rebuilt(&t, b"", &[0x0001, 0x0003, 0xA001, 0xA003, 0xA010]);
                    assert_eq!(&t[field(&t, ifd, 0x0001)..][..4], b"0100");
                    assert_eq!(le32(&t, field(&t, ifd, 0x0003)), 0x0500_1038);
                    assert_eq!(le16(&t, field(&t, ifd, 0xA003)), 13);
                    let fw = le32(&t, field(&t, ifd, 0xA001)) as usize;
                    assert_eq!(&t[fw..fw + 5], b"1.40\0");
                    let areas = le32(&t, field(&t, ifd, 0xA010)) as usize;
                    let got: Vec<u32> = (0..8).map(|k| le32(&t, areas + 4 * k)).collect();
                    assert_eq!(got, SAMSUNG_AREAS, "le {le} at {note_at} self {self_rel}");
                    assert!(!t.windows(8).any(|w| w == [0x77; 8]));
                }
            }
        }
        // Not a Type 2 note (another first entry): left as it is, not rebuilt.
        let mut other = samsung(true, 3000, true);
        other[2] = 0x02;
        let t = both(&file(true, false, "SAMSUNG", &other, 3000)).0.unwrap();
        let x = sub(&t, 8, 0x8769);
        assert_eq!(le32(&t, field(&t, x, 0x927C)), 3000);
        // Cut short: dropped.
        let mut cut = samsung(true, 100, true);
        cut.truncate(60);
        let t = both(&file(true, false, "SAMSUNG", &cut, 100)).0.unwrap();
        assert!(!tags(&t, sub(&t, 8, 0x8769)).contains(&0x927C));
    }

    const PANA_HEADER: &[u8] = b"Panasonic\0\0\0";
    const PANA_LENS: &str = "LUMIX G VARIO 12-60/F3.5-5.6";

    /// The tags of the rebuilt fixture Panasonic note (little-endian; a big-endian one loses
    /// its TimeInfo block, 0x2003).
    const PANA_KEPT: [u16; 7] = [0x0001, 0x0002, 0x0007, 0x001A, 0x0051, 0x0059, 0x2003];
    /// Transform (0x59): two 16-bit integers in an UNDEFINED block.
    const PANA_TRANSFORM: [i16; 2] = [-1, 1];

    /// A Panasonic note of byte order `le` at `at`, its offsets from the TIFF header: a
    /// setting, the firmware version, focus mode AF-C, Dual2 IS, the lens, the Transform and
    /// TimeInfo blocks, serials, a name and a city.
    fn panasonic(le: bool, at: usize) -> Vec<u8> {
        let mut w = W {
            le,
            big: false,
            b: vec![0; at],
        };
        w.b.extend_from_slice(PANA_HEADER);
        let (lens, serial) = (ascii(PANA_LENS), ascii(SERIAL));
        let (baby, city) = (ascii("BABY-NAME-X"), ascii("CITY-NAME-X"));
        let mut internal = SERIAL.as_bytes().to_vec();
        internal.resize(16, 0);
        let transform = w.short(&PANA_TRANSFORM.map(|v| v as u16));
        let mut time = b"\x20\x21\x07\x14\x09\x26\x53\0".to_vec();
        time.extend([0; 8]);
        time.extend(w.long(&[42]));
        let e = vec![
            (0x0001, 3, 1, w.short(&[2])),
            (0x0002, 7, 4, vec![0, 1, 3, 0]),
            (0x0007, 3, 1, w.short(&[7])),
            (0x001A, 3, 1, w.short(&[11])),
            (0x0025, 7, 16, internal),
            (0x0051, lens.0, lens.1, lens.2),
            (0x0052, serial.0, serial.1, serial.2),
            (0x0059, 7, 4, transform),
            (0x0066, baby.0, baby.1, baby.2.clone()),
            (0x006D, city.0, city.1, city.2),
            (0x2003, 7, 20, time),
            (0x8010, baby.0, baby.1, baby.2),
        ];
        w.ifd(e, 0);
        w.b.split_off(at)
    }

    /// Checks the rebuilt Panasonic note at `ifd` of `t`: little-endian values whatever the
    /// note's order.
    fn panasonic_values(t: &[u8], ifd: usize) {
        let l = le32(t, field(t, ifd, 0x51)) as usize;
        assert_eq!(&t[l..l + PANA_LENS.len()], PANA_LENS.as_bytes());
        assert_eq!(le16(t, field(t, ifd, 0x07)), 7);
        assert_eq!(le16(t, field(t, ifd, 0x1A)), 11);
        assert_eq!(le32(t, field(t, ifd, 0x02)).to_le_bytes(), [0, 1, 3, 0]);
        let f = field(t, ifd, 0x59);
        let got = [le16(t, f) as i16, le16(t, f + 2) as i16];
        assert_eq!(got, PANA_TRANSFORM);
        assert!(!t.windows(6).any(|w| w == b"NAME-X"));
    }

    /// A Panasonic note keeps its header and loses its serials and the entries naming people
    /// and places. A big-endian one is swapped value by value, its byte and word blocks with
    /// them, and loses the blocks it cannot swap.
    #[test]
    fn panasonic_notes_are_rebuilt() {
        for note_at in [100, 3000] {
            for le in [true, false] {
                let note = panasonic(le, note_at);
                let mut outs = Vec::new();
                for big in [false, true] {
                    let (t, read) = both(&file(le, big, "Panasonic", &note, note_at));
                    assert_eq!(t, read, "reader and memory differ");
                    outs.push(t.expect("exif"));
                }
                assert_eq!(outs[0], outs[1]);
                let t = &outs[0];
                let want = if le { &PANA_KEPT[..] } else { &PANA_KEPT[..6] };
                let ifd = rebuilt(t, PANA_HEADER, want);
                panasonic_values(t, ifd);
                if le {
                    let b = le32(t, field(t, ifd, 0x2003)) as usize;
                    assert_eq!(le32(t, b + 16), 42);
                }
                // Without its header it is not one to rebuild.
                let bare = file(le, false, "Panasonic", &note[12..], note_at + 12);
                let t = both(&bare).0.unwrap();
                assert_eq!(
                    tags(&t, sub(&t, 8, 0x8769)).contains(&0x927C),
                    note_at == 3000
                );
            }
        }
        // A big-endian note whose values do not all lie within it is dropped.
        let mut cut = panasonic(false, 100);
        cut.truncate(cut.len() - 30);
        let t = both(&file(false, false, "Panasonic", &cut, 100)).0.unwrap();
        assert!(!tags(&t, sub(&t, 8, 0x8769)).contains(&0x927C));
    }

    /// DNGPrivateData as Adobe DNG Converter writes it: another chunk, then "MakN" with the
    /// Canon note (byte order `le`) that lay at `orig` in the original file.
    fn adobe(le: bool, orig: usize) -> Vec<u8> {
        let note = canon(le, orig);
        let mut v = b"Adobe\0Pano".to_vec();
        v.extend(4u32.to_be_bytes());
        v.extend([1, 2, 3, 4]);
        v.extend(b"MakN");
        v.extend(((6 + note.len()) as u32).to_be_bytes());
        v.extend(if le { b"II" } else { b"MM" });
        v.extend((orig as u32).to_be_bytes());
        v.extend(note);
        v
    }

    /// The Canon note of a converted DNG is read from DNGPrivateData and rebuilt like a CR2's.
    #[test]
    fn dng_private_notes_are_rebuilt() {
        for orig in [8, 996] {
            for note_le in [true, false] {
                let p = adobe(note_le, orig);
                let private = (0xC634, 1, p.len() as u32, p);
                let mut outs = Vec::new();
                for big in [false, true] {
                    for le in [true, false] {
                        let f = file_with(le, big, "Canon", None, 100, vec![private.clone()]);
                        let (t, read) = both(&f);
                        assert_eq!(t, read, "reader and memory differ");
                        outs.push(t.expect("exif"));
                    }
                }
                assert!(outs.iter().all(|t| *t == outs[0]));
                let t = &outs[0];
                assert!(!tags(t, 8).contains(&0xC634));
                let ifd = rebuilt(t, b"", &[0x0001, 0x0095]);
                let cs = le32(t, field(t, ifd, 1)) as usize;
                let got: Vec<u16> = (0..40).map(|k| le16(t, cs + 2 * k)).collect();
                assert_eq!(got, settings());
                let info = crate::exif_facts(t).unwrap().0;
                assert_eq!(info.lens.as_deref(), Some(LENS));
                assert!(t.len() < 700, "{} bytes", t.len());
            }
        }
        // A damaged block gives no note.
        let p = adobe(true, 8);
        let mut cut = p.clone();
        cut.truncate(p.len() - 20);
        let mut bad = p.clone();
        bad[13] = 0xFF;
        for (private, note) in [(p, true), (cut, false), (bad, false)] {
            let e = (0xC634, 1, private.len() as u32, private);
            let t = both(&file_with(true, false, "Canon", None, 100, vec![e]))
                .0
                .unwrap();
            assert_eq!(tags(&t, sub(&t, 8, 0x8769)).contains(&0x927C), note);
        }
    }

    /// The Exif TIFF structure of a JpgFromRaw: IFD0 (Make, Model, Orientation 1, a
    /// resolution), the Exif IFD (exposure, ISO, a Panasonic note, the preview's size) and the
    /// GPS IFD.
    fn rw2_preview_tiff() -> Vec<u8> {
        let mut w = W::new(true, false);
        w.b.resize(64, 0);
        let note_at = w.b.len();
        let note = panasonic(true, note_at);
        let note_len = note.len() as u32;
        w.b.extend(note);
        let gps = w.ifd(vec![(0, 1, 4, vec![2, 3, 0, 0])], 0);
        let exif = vec![
            (0x829A, 5, 1, w.rat(&[(1, 60)])),
            (0x8827, 3, 1, w.short(&[400])),
            (0x9102, 5, 1, w.rat(&[(2, 1)])),
            (0x927C, 7, note_len, vec![0; 8]), // patched below
            (0xA002, 4, 1, w.long(&[1920])),
            (0xA003, 4, 1, w.long(&[1440])),
            (0xA405, 3, 1, w.short(&[59])),
        ];
        let exif = w.ifd(exif, 0);
        let f = w.field(exif, 3);
        w.set_off(f, note_at as u64);
        let (mk, md) = (ascii("Panasonic"), ascii("DC-FIXTURE"));
        let ifd0 = vec![
            (271, mk.0, mk.1, mk.2),
            (272, md.0, md.1, md.2),
            (274, 3, 1, w.short(&[1])),
            (282, 5, 1, w.rat(&[(180, 1)])),
            w.ptr(0x8769, exif),
            w.ptr(0x8825, gps),
        ];
        let ifd0 = w.ifd(ifd0, 0);
        w.ifd0(ifd0);
        w.b
    }

    /// An RW2 ("IIU\0"): Panasonic raw tags, a JpgFromRaw carrying `preview` (when given) as
    /// its APP1 Exif, Make, Model, Orientation 6 and, `exif`, an Exif IFD with only the
    /// exposure (another one than the preview's) and the date.
    fn rw2(preview: Option<&[u8]>, exif: bool) -> Vec<u8> {
        rw2_with(preview, exif, vec![])
    }

    /// [`rw2`] with `extra` entries in IFD0. Its raw ISO (0x17) is 640, the preview's 400.
    fn rw2_with(preview: Option<&[u8]>, exif: bool, extra: Vec<E>) -> Vec<u8> {
        let mut w = W::new(true, false);
        w.b[2] = 0x55;
        let jpeg = preview.map(|t| {
            let mut j = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xE1];
            j.extend(((t.len() + 8) as u16).to_be_bytes());
            j.extend(b"Exif\0\0");
            j.extend(t);
            j.extend([0xFF, 0xDA, 0, 2, 0xFF, 0xD9]);
            j
        });
        let date = ascii("2021:07:14 09:26:53");
        let exif = exif.then(|| {
            let e = vec![
                (0x829A, 5, 1, w.rat(&[(1, 250)])),
                (0x9003, date.0, date.1, date.2),
            ];
            w.ifd(e, 0)
        });
        let (mk, md) = (ascii("Panasonic"), ascii("DC-FIXTURE"));
        let mut ifd0 = vec![
            (0x0002, 3, 1, w.short(&[5264])),
            (0x0017, 3, 1, w.short(&[640])),
            (271, mk.0, mk.1, mk.2),
            (272, md.0, md.1, md.2),
            (274, 3, 1, w.short(&[6])),
        ];
        if let Some(j) = jpeg {
            ifd0.push((0x002E, 7, j.len() as u32, j));
        }
        if let Some(exif) = exif {
            ifd0.push(w.ptr(0x8769, exif));
        }
        ifd0.extend(extra);
        ifd0.sort_by_key(|e| e.0);
        let ifd0 = w.ifd(ifd0, 0);
        w.ifd0(ifd0);
        w.b
    }

    /// An RW2's Exif is its JpgFromRaw's (with the rebuilt maker note) under its own IFD0 and
    /// Exif entries, without the raw settings or what describes the preview only.
    #[test]
    fn rw2_takes_the_preview_exif() {
        let pv = rw2_preview_tiff();
        for exif in [true, false] {
            let (t, read) = both(&rw2(Some(&pv), exif));
            let t = t.expect("exif");
            assert_eq!(read.as_ref(), Some(&t), "reader and memory differ");
            assert_eq!(tags(&t, 8), [271, 272, 274, 282, 0x8769, 0x8825]);
            // The RW2's orientation over the preview's.
            assert_eq!(crate::exif_orientation(&t), Some(6));
            let x = sub(&t, 8, 0x8769);
            let mut want = vec![0x829A, 0x8827, 0x927C, 0xA405];
            if exif {
                want.insert(2, 0x9003);
            }
            assert_eq!(tags(&t, x), want);
            let cam = crate::exif_facts(&t).unwrap().0;
            assert_eq!(cam.camera, "Panasonic DC-FIXTURE");
            let shutter = if exif { 1.0 / 250.0 } else { 1.0 / 60.0 };
            assert_eq!(cam.exposure_time, Some(shutter));
            // The preview's ISO over the raw one.
            assert_eq!(cam.iso, Some(400));
            assert_eq!(tags(&t, sub(&t, 8, 0x8825)), [0]);
            let ifd = rebuilt(&t, PANA_HEADER, &PANA_KEPT);
            panasonic_values(&t, ifd);
        }
    }

    /// Without a usable JpgFromRaw an RW2 gives its own IFD0 and Exif IFD.
    #[test]
    fn rw2_without_a_preview() {
        let mut bad = rw2_preview_tiff();
        bad[0] = b'X';
        for f in [rw2(None, true), rw2(Some(&bad), true)] {
            let (t, read) = both(&f);
            let t = t.expect("exif");
            assert_eq!(read.as_ref(), Some(&t), "reader and memory differ");
            assert_eq!(tags(&t, 8), [271, 272, 274, 0x8769]);
            // The raw ISO stands in for the preview's.
            assert_eq!(tags(&t, sub(&t, 8, 0x8769)), [0x829A, 0x8827, 0x9003]);
            assert_eq!(crate::exif_facts(&t).unwrap().0.iso, Some(640));
        }
        // With no Exif IFD of its own, one is made for the ISO.
        let (t, _) = both(&rw2(None, false));
        let t = t.expect("exif");
        assert_eq!(tags(&t, 8), [271, 272, 274, 0x8769]);
        assert_eq!(tags(&t, sub(&t, 8, 0x8769)), [0x8827]);
        // Nothing but raw settings: no Exif.
        let mut w = W::new(true, false);
        w.b[2] = 0x55;
        let e = vec![(0x0002, 3, 1, w.short(&[5264]))];
        let ifd0 = w.ifd(e, 0);
        w.ifd0(ifd0);
        assert_eq!(both(&w.b), (None, None));
    }

    /// An RW2 CameraIFD ("II*\0" or "MM\0*" and an IFD, offsets from its start): its own
    /// types 0x101/0x102, LensAttached, FocalLengthIn35mmFormat 50, ApertureValue 5 EV and
    /// ShutterSpeedValue 7.84 EV (in 1/256 EV), FlashFired yes, WhiteBalanceSet cloudy, an
    /// orientation, a block.
    fn camera_ifd(le: bool, lens: u32) -> E {
        let mut w = W::new(le, false);
        let e = vec![
            (0x1001, 0x102, 1, w.long(&[0])),
            (0x1102, 4, 1, w.long(&[1])),
            (0x1200, 0x101, 1, w.long(&[lens])),
            (0x1203, 3, 1, w.short(&[50])),
            (0x1301, 8, 1, w.short(&[1280])),
            (0x1302, 8, 1, w.short(&[2007])),
            (0x2009, 7, 12, vec![0x77; 12]),
            (0x3300, 1, 1, vec![2]),
            (0x3501, 1, 1, vec![1]),
        ];
        let at = w.ifd(e, 0);
        w.ifd0(at);
        (0x0120, 7, w.b.len() as u32, w.b)
    }

    /// An RW2's CameraIFD gives the 35 mm focal length over the preview's, and the APEX
    /// aperture and shutter speed, the flash and the white balance the Exif lacks; with no
    /// lens attached, not the aperture or the focal length.
    #[test]
    fn rw2_camera_ifd() {
        let pv = rw2_preview_tiff();
        let rat = |t: &[u8], x: usize, tag: u16| {
            let r = le32(t, field(t, x, tag)) as usize;
            (le32(t, r), le32(t, r + 4))
        };
        for preview in [true, false] {
            for le in [true, false] {
                for lens in [1, 0] {
                    let f = rw2_with(preview.then_some(&pv[..]), true, vec![camera_ifd(le, lens)]);
                    let (t, read) = both(&f);
                    let t = t.expect("exif");
                    assert_eq!(read.as_ref(), Some(&t), "reader and memory differ");
                    let x = sub(&t, 8, 0x8769);
                    let got = tags(&t, x);
                    assert!(got.contains(&0x9201));
                    assert_eq!(rat(&t, x, 0x9201), (2007, 256));
                    assert_eq!(got.contains(&0x9202), lens == 1);
                    if lens == 1 {
                        assert_eq!(rat(&t, x, 0x9202), (1280, 256));
                        assert_eq!(le16(&t, field(&t, x, 0xA405)), 50);
                    } else if preview {
                        assert_eq!(le16(&t, field(&t, x, 0xA405)), 59);
                    } else {
                        assert!(!got.contains(&0xA405));
                    }
                    // Fired; cloudy: manual white balance, light source cloudy.
                    assert_eq!(le16(&t, field(&t, x, 0x9209)), 1);
                    assert_eq!(le16(&t, field(&t, x, 0xA403)), 1);
                    assert_eq!(le16(&t, field(&t, x, 0x9208)), 10);
                    // Nothing else of the block comes through.
                    assert!(!t.windows(12).any(|w| w == [0x77; 12]));
                }
            }
        }
        // A damaged block (no TIFF header, or cut short) gives nothing.
        let (tag, typ, _, mut block) = camera_ifd(true, 1);
        block[0] = b'X';
        let (t, _) = both(&rw2_with(
            Some(&pv),
            true,
            vec![(tag, typ, block.len() as u32, block)],
        ));
        let t = t.unwrap();
        let x = sub(&t, 8, 0x8769);
        assert!(!tags(&t, x).contains(&0x9201));
        assert_eq!(le16(&t, field(&t, x, 0xA405)), 59);
        let (tag, typ, _, mut block) = camera_ifd(true, 1);
        block.truncate(40);
        let (t, _) = both(&rw2_with(None, true, vec![(tag, typ, 40, block)]));
        let t = t.unwrap();
        assert_eq!(tags(&t, sub(&t, 8, 0x8769)), [0x829A, 0x8827, 0x9003]);
    }

    /// An ORF ("IIRO", "IIRS", "MMOR") reads like the TIFF it is.
    #[test]
    fn orf_headers_read_as_tiff() {
        for (le, magic) in [(true, *b"RO"), (true, *b"RS"), (false, *b"OR")] {
            let tiff = file(le, false, "OLYMPUS", NIKON, 100);
            let mut orf = tiff.clone();
            orf[2..4].copy_from_slice(&magic);
            let want = both(&tiff).0.expect("exif");
            assert_eq!(both(&orf), (Some(want.clone()), Some(want)));
        }
    }
}

/// Synthetic hostile inputs: none may panic or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    /// A little-endian IFD at `at` of `entries` (tag, type, count, value field).
    fn ifd(b: &mut Vec<u8>, at: usize, entries: &[(u16, u16, u32, u32)]) {
        b.resize(b.len().max(at + 2 + 12 * entries.len() + 4), 0);
        b[at..at + 2].copy_from_slice(&(entries.len() as u16).to_le_bytes());
        for (k, &(tag, typ, count, field)) in entries.iter().enumerate() {
            let e = at + 2 + 12 * k;
            b[e..e + 2].copy_from_slice(&tag.to_le_bytes());
            b[e + 2..e + 4].copy_from_slice(&typ.to_le_bytes());
            b[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
            b[e + 8..e + 12].copy_from_slice(&field.to_le_bytes());
        }
    }

    /// IFD0 with a thousand 60 KB values and an Exif IFD with a thousand 3 MB maker notes,
    /// all at one offset: each IFD keeps at most MAX_EXIF bytes and a single note.
    #[test]
    fn repeated_large_values_are_held_once() {
        const BLOB: usize = 3 << 20;
        let (ifd0_at, exif_at, blob_at) = (8usize, 16 << 10, 32 << 10);
        let mut b = b"II*\0".to_vec();
        b.extend_from_slice(&(ifd0_at as u32).to_le_bytes());
        let mut e0 = vec![(EXIF_IFD, 4, 1, exif_at as u32)];
        e0.extend((0..1000).map(|k| (0xC800 + k as u16, 7, 60_000, blob_at as u32)));
        e0.sort_by_key(|e| e.0);
        ifd(&mut b, ifd0_at, &e0);
        let notes: Vec<_> = (0..1000)
            .map(|_| (MAKER_NOTE, 7, BLOB as u32, blob_at as u32))
            .collect();
        ifd(&mut b, exif_at, &notes);
        b.resize(blob_at + BLOB, 0);
        let (ifd0, ..) = read_ifd0(&&b[..]).expect("ifd0");
        let held: usize = ifd0.entries.iter().map(|e| e.value.len()).sum();
        assert!(held as u64 <= MAX_EXIF, "{held} bytes held");
        let x = &ifd0.subs.iter().find(|s| s.0 == EXIF_IFD).expect("exif").1;
        assert_eq!(x.entries.len(), 1);
        // The whole path stays sane too.
        let _ = tiff_exif(&&b[..]);
    }
}

//! Exif for converted files: the source's TIFF-structured Exif is parsed into its IFD0, Exif
//! and GPS directories, the image-structure tags are dropped (they describe the source's
//! pixels, not ours), the orientation becomes 1 (pixels are written upright), the pixel
//! dimensions become the output's, the interoperability directory and the thumbnail go, and
//! GPS goes too unless all metadata is kept. The result is written little-endian, either as a
//! standalone TIFF structure (JPEG, PNG, WebP, JPEG XL) or as directories inside a TIFF file.

/// One directory entry, with its value normalised to little-endian.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tag: u16,
    pub ty: u16,
    pub count: u32,
    pub data: Vec<u8>,
}

impl Entry {
    pub fn short(tag: u16, v: &[u16]) -> Entry {
        Entry {
            tag,
            ty: 3,
            count: v.len() as u32,
            data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
        }
    }

    pub fn long(tag: u16, v: &[u32]) -> Entry {
        Entry {
            tag,
            ty: 4,
            count: v.len() as u32,
            data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
        }
    }

    pub fn rational(tag: u16, num: u32, den: u32) -> Entry {
        let mut data = num.to_le_bytes().to_vec();
        data.extend_from_slice(&den.to_le_bytes());
        Entry {
            tag,
            ty: 5,
            count: 1,
            data,
        }
    }

    pub fn ascii(tag: u16, s: &str) -> Entry {
        let mut data = s.as_bytes().to_vec();
        data.push(0);
        Entry {
            tag,
            ty: 2,
            count: data.len() as u32,
            data,
        }
    }

    pub fn undefined(tag: u16, bytes: &[u8]) -> Entry {
        Entry {
            tag,
            ty: 7,
            count: bytes.len() as u32,
            data: bytes.to_vec(),
        }
    }
}

/// Bytes per value of a TIFF field type; `None` for unknown types.
fn type_size(ty: u16) -> Option<usize> {
    Some(match ty {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

/// Size of the units a value is byte-swapped in (rationals are two 4-byte halves).
fn swap_unit(ty: u16) -> usize {
    match ty {
        3 | 8 => 2,
        4 | 9 | 11 | 13 | 5 | 10 => 4,
        12 => 8,
        _ => 1,
    }
}

pub const TAG_ORIENTATION: u16 = 0x0112;
pub const TAG_EXIF_IFD: u16 = 0x8769;
pub const TAG_GPS_IFD: u16 = 0x8825;
const TAG_INTEROP_IFD: u16 = 0xA005;
const TAG_PIXEL_X: u16 = 0xA002;
const TAG_PIXEL_Y: u16 = 0xA003;
const TAG_MAKER_NOTE: u16 = 0x927C;
pub const TAG_X_RESOLUTION: u16 = 0x011A;
pub const TAG_Y_RESOLUTION: u16 = 0x011B;
pub const TAG_RESOLUTION_UNIT: u16 = 0x0128;

/// IFD0 tags that describe the picture rather than its pixel storage; everything else in
/// IFD0 (strips, tiles, compression, sample format, colour maps, DNG and maker tags) belongs
/// to the source's encoding and is not carried over.
const IFD0_KEEP: &[u16] = &[
    0x010D, // DocumentName
    0x010E, // ImageDescription
    0x010F, // Make
    0x0110, // Model
    TAG_X_RESOLUTION,
    TAG_Y_RESOLUTION,
    TAG_RESOLUTION_UNIT,
    0x0131, // Software
    0x0132, // DateTime
    0x013B, // Artist
    0x013C, // HostComputer
    0x013E, // WhitePoint
    0x013F, // PrimaryChromaticities
    0x4746, // Rating
    0x4749, // RatingPercent
    0x8298, // Copyright
    0x9C9B, // XPTitle
    0x9C9C, // XPComment
    0x9C9D, // XPAuthor
    0x9C9E, // XPKeywords
    0x9C9F, // XPSubject
];

/// The parsed directories of an Exif block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exif {
    pub ifd0: Vec<Entry>,
    pub exif: Vec<Entry>,
    pub gps: Vec<Entry>,
}

impl Exif {
    /// Parses TIFF-structured Exif (the payload after "Exif\0\0"). `None` when the header is
    /// not TIFF; damaged entries and directories are skipped.
    pub fn parse(tiff: &[u8]) -> Option<Exif> {
        let be = match tiff.get(..4)? {
            [b'I', b'I', 42, 0] => false,
            [b'M', b'M', 0, 42] => true,
            _ => return None,
        };
        let r = Reader { d: tiff, be };
        let ifd0 = r.ifd(r.u32(4)? as usize);
        let sub = |tag: u16| {
            ifd0.iter()
                .find(|e| e.tag == tag && e.data.len() >= 4)
                .map(|e| u32::from_le_bytes([e.data[0], e.data[1], e.data[2], e.data[3]]) as usize)
                .map(|o| r.ifd(o))
                .unwrap_or_default()
        };
        let exif = sub(TAG_EXIF_IFD);
        let gps = sub(TAG_GPS_IFD);
        Some(Exif { ifd0, exif, gps })
    }

    /// The Exif to write for an upright `width × height` output: descriptive IFD0 tags with
    /// Orientation 1, the Exif directory without the interoperability pointer and with the
    /// new pixel dimensions, GPS only when `keep_location`. `None` when nothing is left.
    pub fn for_output(&self, width: u32, height: u32, keep_location: bool) -> Option<Exif> {
        let mut ifd0: Vec<Entry> = self
            .ifd0
            .iter()
            .filter(|e| IFD0_KEEP.contains(&e.tag))
            .cloned()
            .collect();
        let mut exif: Vec<Entry> = self
            .exif
            .iter()
            .filter(|e| e.ty != 13 && !matches!(e.tag, TAG_INTEROP_IFD | TAG_PIXEL_X | TAG_PIXEL_Y))
            .cloned()
            .collect();
        if ifd0.is_empty() && exif.is_empty() && (!keep_location || self.gps.is_empty()) {
            return None;
        }
        ifd0.push(Entry::short(TAG_ORIENTATION, &[1]));
        if !exif.is_empty() {
            exif.push(Entry::long(TAG_PIXEL_X, &[width]));
            exif.push(Entry::long(TAG_PIXEL_Y, &[height]));
        }
        let gps = if keep_location {
            self.gps.iter().filter(|e| e.ty != 13).cloned().collect()
        } else {
            Vec::new()
        };
        Some(Exif { ifd0, exif, gps })
    }

    /// Drops the maker note (for containers whose Exif must stay under 64 KiB).
    pub fn without_maker_note(&self) -> Exif {
        let mut e = self.clone();
        e.exif.retain(|x| x.tag != TAG_MAKER_NOTE);
        e
    }

    /// A standalone little-endian TIFF structure: header, IFD0, then the Exif and GPS
    /// directories it points to.
    pub fn to_tiff(&self) -> Vec<u8> {
        let mut out = b"II\x2a\0\x08\0\0\0".to_vec();
        write_ifds(&mut out, self.ifd0.clone(), &self.exif, &self.gps);
        out
    }

    /// Resolution tags of IFD0 (for a TIFF file's own IFD).
    pub fn resolution(&self) -> Vec<Entry> {
        self.ifd0
            .iter()
            .filter(|e| {
                matches!(
                    e.tag,
                    TAG_X_RESOLUTION | TAG_Y_RESOLUTION | TAG_RESOLUTION_UNIT
                )
            })
            .cloned()
            .collect()
    }
}

/// Appends IFD0 (`ifd0` plus pointers to non-empty `exif` / `gps` directories) and the
/// directories it points to at the end of `out`, whose start is the TIFF header; patches
/// the header's IFD0 offset when `out` holds just the header, and returns IFD0's offset.
/// Entries are sorted by tag; values are word-aligned.
pub fn write_ifds(out: &mut Vec<u8>, mut ifd0: Vec<Entry>, exif: &[Entry], gps: &[Entry]) -> u32 {
    ifd0.retain(|e| e.tag != TAG_EXIF_IFD && e.tag != TAG_GPS_IFD);
    if !exif.is_empty() {
        ifd0.push(Entry::long(TAG_EXIF_IFD, &[0]));
    }
    if !gps.is_empty() {
        ifd0.push(Entry::long(TAG_GPS_IFD, &[0]));
    }
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let ifd0_at = write_ifd(out, &mut ifd0);
    let patch = |tag: u16, at: u32, out: &mut Vec<u8>| {
        if let Some(i) = ifd0.iter().position(|e| e.tag == tag) {
            let p = ifd0_at as usize + 2 + i * 12 + 8;
            out[p..p + 4].copy_from_slice(&at.to_le_bytes());
        }
    };
    if !exif.is_empty() {
        let at = write_ifd(out, &mut exif.to_vec());
        patch(TAG_EXIF_IFD, at, out);
    }
    if !gps.is_empty() {
        let at = write_ifd(out, &mut gps.to_vec());
        patch(TAG_GPS_IFD, at, out);
    }
    if out.len() >= 8 && out[4..8] == [8, 0, 0, 0] && ifd0_at != 8 {
        out[4..8].copy_from_slice(&ifd0_at.to_le_bytes());
    }
    ifd0_at
}

/// Writes one directory (sorted, duplicates dropped) followed by its out-of-line values;
/// returns its offset.
fn write_ifd(out: &mut Vec<u8>, entries: &mut Vec<Entry>) -> u32 {
    entries.sort_by_key(|e| e.tag);
    entries.dedup_by_key(|e| e.tag);
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let at = out.len();
    let n = entries.len();
    out.extend_from_slice(&(n as u16).to_le_bytes());
    let table = out.len();
    out.resize(table + n * 12 + 4, 0);
    for (i, e) in entries.iter().enumerate() {
        let p = table + i * 12;
        out[p..p + 2].copy_from_slice(&e.tag.to_le_bytes());
        out[p + 2..p + 4].copy_from_slice(&e.ty.to_le_bytes());
        out[p + 4..p + 8].copy_from_slice(&e.count.to_le_bytes());
        if e.data.len() <= 4 {
            out[p + 8..p + 8 + e.data.len()].copy_from_slice(&e.data);
        } else {
            if out.len() % 2 == 1 {
                out.push(0);
            }
            let v = out.len() as u32;
            out.extend_from_slice(&e.data);
            out[p + 8..p + 12].copy_from_slice(&v.to_le_bytes());
        }
    }
    at as u32
}

struct Reader<'a> {
    d: &'a [u8],
    be: bool,
}

impl Reader<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.d.get(at..at + 2)?;
        Some(if self.be {
            u16::from_be_bytes([b[0], b[1]])
        } else {
            u16::from_le_bytes([b[0], b[1]])
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.d.get(at..at + 4)?;
        Some(if self.be {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        })
    }

    /// The entries of the directory at `at`, values little-endian; unreadable ones skipped.
    fn ifd(&self, at: usize) -> Vec<Entry> {
        let Some(n) = self.u16(at) else {
            return Vec::new();
        };
        let mut v = Vec::new();
        for i in 0..n as usize {
            let p = at + 2 + i * 12;
            let (Some(tag), Some(ty), Some(count)) =
                (self.u16(p), self.u16(p + 2), self.u32(p + 4))
            else {
                break;
            };
            let Some(size) = type_size(ty) else {
                continue;
            };
            let Some(len) = (count as usize)
                .checked_mul(size)
                .filter(|&l| l <= 16 << 20)
            else {
                continue;
            };
            let src = if len <= 4 {
                self.d.get(p + 8..p + 8 + len)
            } else {
                self.u32(p + 8)
                    .and_then(|o| self.d.get(o as usize..(o as usize).checked_add(len)?))
            };
            let Some(src) = src else {
                continue;
            };
            let mut data = src.to_vec();
            let unit = swap_unit(ty);
            if self.be && unit > 1 {
                for c in data.chunks_exact_mut(unit) {
                    c.reverse();
                }
            }
            v.push(Entry {
                tag,
                ty,
                count,
                data,
            });
        }
        v
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A big-endian camera-style Exif: IFD0 with structure tags, Exif IFD with an interop
    /// pointer and a maker note, a GPS IFD with a latitude.
    pub(crate) fn sample_be() -> Vec<u8> {
        let mut d = b"MM\0\x2a\0\0\0\x08".to_vec();
        let be16 = |v: u16| v.to_be_bytes();
        let be32 = |v: u32| v.to_be_bytes();
        // IFD0 at 8: Make (ASCII "Canon\0", out of line), Orientation 6, StripOffsets,
        // ExifIFD, GPS.
        let ifd0 = 8usize;
        let n0 = 5u16;
        let vals0 = ifd0 + 2 + n0 as usize * 12 + 4;
        let exif_at = vals0 + 6;
        let n1 = 4u16;
        let vals1 = exif_at + 2 + n1 as usize * 12 + 4;
        let gps_at = vals1 + 20 + 24;
        let e = |d: &mut Vec<u8>, tag: u16, ty: u16, count: u32, val: [u8; 4]| {
            d.extend_from_slice(&be16(tag));
            d.extend_from_slice(&be16(ty));
            d.extend_from_slice(&be32(count));
            d.extend_from_slice(&val);
        };
        d.extend_from_slice(&be16(n0));
        e(&mut d, 0x010F, 2, 6, be32(vals0 as u32));
        e(&mut d, 0x0111, 4, 1, be32(1234));
        e(&mut d, 0x0112, 3, 1, [0, 6, 0, 0]);
        e(&mut d, 0x8769, 4, 1, be32(exif_at as u32));
        e(&mut d, 0x8825, 4, 1, be32(gps_at as u32));
        d.extend_from_slice(&be32(0));
        d.extend_from_slice(b"Canon\0");
        assert_eq!(d.len(), exif_at);
        d.extend_from_slice(&be16(n1));
        // ExposureTime 1/250 (rational, out of line), DateTimeOriginal (ASCII 20, out of
        // line), MakerNote (UNDEFINED 4, inline), Interop pointer.
        e(&mut d, 0x829A, 5, 1, be32(vals1 as u32));
        e(&mut d, 0x9003, 2, 20, be32(vals1 as u32 + 8 + 12));
        e(&mut d, 0x927C, 7, 4, *b"MKNT");
        e(&mut d, 0xA005, 4, 1, be32(0));
        d.extend_from_slice(&be32(0));
        d.extend_from_slice(&be32(1));
        d.extend_from_slice(&be32(250));
        d.extend_from_slice(&[0; 12]);
        d.extend_from_slice(b"2023:07:01 10:20:30\0");
        d.extend_from_slice(&[0; 4]);
        assert_eq!(d.len(), gps_at);
        // GPS: LatitudeRef "N", Latitude, LongitudeRef "E", Longitude (3 rationals each).
        let gvals = gps_at + 2 + 4 * 12 + 4;
        d.extend_from_slice(&be16(4));
        e(&mut d, 0x0001, 2, 2, *b"N\0\0\0");
        e(&mut d, 0x0002, 5, 3, be32(gvals as u32));
        e(&mut d, 0x0003, 2, 2, *b"E\0\0\0");
        e(&mut d, 0x0004, 5, 3, be32(gvals as u32 + 24));
        d.extend_from_slice(&be32(0));
        for v in [48u32, 1, 51, 1, 30, 1, 2, 1, 21, 1, 0, 1] {
            d.extend_from_slice(&be32(v));
        }
        d
    }

    #[test]
    fn parse_big_endian_and_rebuild() {
        let src = sample_be();
        let x = Exif::parse(&src).unwrap();
        assert_eq!(x.ifd0.len(), 5);
        assert_eq!(x.exif.len(), 4);
        assert_eq!(x.gps.len(), 4);
        let exp = x.exif.iter().find(|e| e.tag == 0x829A).unwrap();
        assert_eq!(exp.data, [1, 0, 0, 0, 250, 0, 0, 0]);

        let out = x.for_output(800, 600, false).unwrap();
        assert!(out.gps.is_empty());
        assert!(out.ifd0.iter().all(|e| e.tag != 0x0111));
        let tiff = out.to_tiff();
        // Our own Exif readers agree with what we wrote.
        assert_eq!(specular_decode::exif_orientation(&tiff), Some(1));
        assert_eq!(
            specular_decode::exif_date_taken(&tiff).as_deref(),
            Some("2023:07:01 10:20:30")
        );
        assert_eq!(specular_decode::exif_position(&tiff), None);
        let back = Exif::parse(&tiff).unwrap();
        assert!(back.exif.iter().any(|e| e.tag == TAG_MAKER_NOTE));
        assert!(back.exif.iter().all(|e| e.tag != TAG_INTEROP_IFD));
        let px = back.exif.iter().find(|e| e.tag == TAG_PIXEL_X).unwrap();
        assert_eq!(px.data, 800u32.to_le_bytes());
        assert!(back.gps.is_empty());
        let make = back.ifd0.iter().find(|e| e.tag == 0x010F).unwrap();
        assert_eq!(make.data, b"Canon\0");

        let all = x.for_output(800, 600, true).unwrap().to_tiff();
        let (lat, _) = specular_decode::exif_position(&all).unwrap();
        assert!(
            (lat - (48.0 + 51.0 / 60.0 + 30.0 / 3600.0)).abs() < 1e-9,
            "{lat}"
        );
        assert!(
            Exif::parse(&all)
                .unwrap()
                .without_maker_note()
                .exif
                .iter()
                .all(|e| e.tag != TAG_MAKER_NOTE)
        );
    }

    #[test]
    fn junk_is_rejected_or_skipped() {
        assert_eq!(Exif::parse(b"nope"), None);
        // An IFD0 offset past the end gives empty directories, not a panic.
        let x = Exif::parse(b"II\x2a\0\xff\0\0\0").unwrap();
        assert!(x.ifd0.is_empty());
        assert_eq!(x.for_output(1, 1, true), None);
        // Truncated entries are skipped.
        let mut t = sample_be();
        t.truncate(60);
        let x = Exif::parse(&t).unwrap();
        assert!(x.exif.is_empty());
    }
}

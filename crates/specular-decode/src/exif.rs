/// Read the orientation tag (0x0112) from IFD0 of a TIFF structure (the payload of an
/// `Exif\0\0` APP1 segment or a PNG eXIf chunk).
pub fn exif_orientation(tiff: &[u8]) -> Option<u8> {
    if tiff.len() < 8 {
        return None;
    }
    let le = match &tiff[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u16> {
        let b = tiff.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = tiff.get(o..o + 4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let ifd = u32_at(4)? as usize;
    let n = u16_at(ifd)? as usize;
    for i in 0..n.min(512) {
        let e = ifd + 2 + i * 12;
        if u16_at(e)? == 0x0112 {
            let typ = u16_at(e + 2)?;
            let v = if typ == 3 {
                u16_at(e + 8)? as u32
            } else {
                u32_at(e + 8)?
            };
            return u8::try_from(v).ok().filter(|v| (1..=8).contains(v));
        }
    }
    None
}

/// The TIFF payload of the first `Exif\0\0` APP1 segment in the head of a JPEG file.
pub fn jpeg_exif(head: &[u8]) -> Option<&[u8]> {
    if head.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    while i + 4 <= head.len() {
        if head[i] != 0xFF {
            return None;
        }
        let m = head[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        if m == 0xDA || m == 0xD9 {
            return None;
        }
        let len = u16::from_be_bytes([head[i + 2], head[i + 3]]) as usize;
        let seg = head.get(i + 4..(i + 2 + len).min(head.len()))?;
        if m == 0xE1 && seg.starts_with(b"Exif\0\0") {
            return Some(&seg[6..]);
        }
        i += 2 + len;
    }
    None
}

/// Date the picture was taken (EXIF DateTimeOriginal, else DateTime) as written in the
/// file, "YYYY:MM:DD HH:MM:SS", which sorts chronologically as text.
pub fn exif_date_taken(tiff: &[u8]) -> Option<String> {
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u16> {
        let b = tiff.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = tiff.get(o..o + 4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    };
    let find = |ifd: usize, tag: u16| -> Option<usize> {
        let n = u16_at(ifd)? as usize;
        (0..n.min(512))
            .map(|i| ifd + 2 + i * 12)
            .find(|&e| u16_at(e) == Some(tag))
    };
    let ascii = |e: usize| -> Option<String> {
        let count = u32_at(e + 4)? as usize;
        let off = if count <= 4 {
            e + 8
        } else {
            u32_at(e + 8)? as usize
        };
        let b = tiff.get(off..off + count.min(32))?;
        let t: String = b
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        (t.len() >= 10 && t.as_bytes()[0].is_ascii_digit() && !t.starts_with("0000")).then_some(t)
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let ifd0 = u32_at(4)? as usize;
    let exif = find(ifd0, 0x8769)
        .and_then(|e| u32_at(e + 8))
        .map(|o| o as usize);
    exif.and_then(|x| find(x, 0x9003))
        .and_then(ascii)
        .or_else(|| find(ifd0, 0x0132).and_then(ascii))
}

/// One IFD walker over a byte-order-tagged buffer (a TIFF structure or an Apple maker note,
/// whose offsets are relative to its own start).
struct Ifd<'a> {
    b: &'a [u8],
    le: bool,
}

impl Ifd<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        let b = self.b.get(o..o.checked_add(2)?)?;
        Some(if self.le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let b = self.b.get(o..o.checked_add(4)?)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    /// The entry of `tag` in the IFD at `ifd`.
    fn find(&self, ifd: usize, tag: u16) -> Option<usize> {
        let n = self.u16(ifd)? as usize;
        (0..n.min(512))
            .map(|i| ifd + 2 + i * 12)
            .find(|&e| self.u16(e) == Some(tag))
    }
    /// The first value of a numeric entry as f64.
    fn number(&self, e: usize) -> Option<f64> {
        let at = |size: usize| -> Option<usize> {
            if size <= 4 {
                Some(e + 8)
            } else {
                Some(self.u32(e + 8)? as usize)
            }
        };
        let v = match self.u16(e + 2)? {
            3 => self.u16(at(2)?)? as f64,
            4 => self.u32(at(4)?)? as f64,
            8 => self.u16(at(2)?)? as i16 as f64,
            9 => self.u32(at(4)?)? as i32 as f64,
            5 => {
                let o = at(8)?;
                let d = self.u32(o + 4)?;
                (d != 0).then(|| self.u32(o).map(|n| n as f64 / d as f64))??
            }
            10 => {
                let o = at(8)?;
                let d = self.u32(o + 4)? as i32;
                (d != 0).then(|| self.u32(o).map(|n| n as i32 as f64 / d as f64))??
            }
            11 => f32::from_bits(self.u32(at(4)?)?) as f64,
            _ => return None,
        };
        v.is_finite().then_some(v)
    }
}

impl Ifd<'_> {
    /// An ASCII entry, trimmed; None when empty.
    fn ascii(&self, e: usize) -> Option<String> {
        if self.u16(e + 2)? != 2 {
            return None;
        }
        let count = (self.u32(e + 4)? as usize).min(256);
        let off = if count <= 4 {
            e + 8
        } else {
            self.u32(e + 8)? as usize
        };
        let b = self.b.get(off..off.checked_add(count)?)?;
        let t: String = b
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        let t = t.trim();
        (!t.is_empty()).then(|| t.to_owned())
    }
}

/// Camera, lens and exposure facts of a TIFF structure (the Exif of a JPEG, or the head of a
/// TIFF-based file), and whether it records a position (a GPS IFD with a latitude). The date
/// is left to [`exif_date_taken`].
pub fn exif_facts(tiff: &[u8]) -> Option<(crate::CameraInfo, bool)> {
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Ifd { b: tiff, le };
    if t.u16(2)? != 42 {
        return None;
    }
    let ifd0 = t.u32(4)? as usize;
    let make = t.find(ifd0, 0x010F).and_then(|e| t.ascii(e));
    let model = t.find(ifd0, 0x0110).and_then(|e| t.ascii(e));
    let canon = make
        .as_deref()
        .is_some_and(|m| m.to_ascii_lowercase().starts_with("canon"));
    let camera = match (make, model) {
        (Some(make), Some(model)) => {
            let first = make.split_whitespace().next().unwrap_or("");
            if model
                .to_ascii_lowercase()
                .starts_with(&first.to_ascii_lowercase())
            {
                model
            } else {
                format!("{make} {model}")
            }
        }
        (a, b) => a.or(b).unwrap_or_default(),
    };
    let mut info = crate::CameraInfo {
        camera,
        ..crate::CameraInfo::default()
    };
    if let Some(x) = t.find(ifd0, 0x8769).and_then(|e| t.u32(e + 8)) {
        let x = x as usize;
        let num = |tag: u16| t.find(x, tag).and_then(|e| t.number(e));
        info.iso = num(0x8827)
            .filter(|v| *v > 0.0 && *v < 1e7)
            .map(|v| v as u32);
        info.exposure_time = num(0x829A).filter(|v| *v > 0.0);
        info.f_number = num(0x829D).filter(|v| *v > 0.0);
        info.focal_length = num(0x920A).filter(|v| *v > 0.0);
        let named = |l: &String| l.chars().any(char::is_alphanumeric);
        info.lens = t.find(x, 0xA434).and_then(|e| t.ascii(e)).filter(named);
        // Canon: else the maker note's LensModel (0x0095). The note is an IFD at its start,
        // its offsets from the TIFF header.
        if info.lens.is_none() && canon {
            info.lens = t
                .find(x, 0x927C)
                .and_then(|e| t.u32(e + 8))
                .and_then(|note| t.find(note as usize, 0x0095))
                .and_then(|e| t.ascii(e))
                .filter(named);
        }
    }
    let gps = t
        .find(ifd0, 0x8825)
        .and_then(|e| t.u32(e + 8))
        .and_then(|g| t.find(g as usize, 0x0002))
        .is_some();
    Some((info, gps))
}

impl crate::CameraInfo {
    /// These facts as a small Exif TIFF structure (little-endian: IFD0 with Model and
    /// Software, an Exif IFD with the exposure, date and lens), for the info panel of a RAW
    /// format whose metadata is not Exif (Canon CRW, Sigma X3F…). `None` when there is
    /// nothing to say.
    pub fn exif_tiff(&self) -> Option<Vec<u8>> {
        // (tag, type, count, value bytes)
        type E = (u16, u16, u32, Vec<u8>);
        let ascii = |tag: u16, s: &str| -> Option<E> {
            let s = s.trim();
            (!s.is_empty()).then(|| {
                let mut v = s.as_bytes().to_vec();
                v.push(0);
                (tag, 2, v.len() as u32, v)
            })
        };
        let rational = |tag: u16, typ: u16, n: i64, d: i64| -> E {
            let (n, d) = (n.clamp(i32::MIN.into(), u32::MAX.into()), d.max(1));
            let mut v = (n as u32).to_le_bytes().to_vec();
            v.extend((d as u32).to_le_bytes());
            (tag, typ, 1, v)
        };
        let mut ifd0: Vec<E> = Vec::new();
        ifd0.extend(ascii(0x0110, &self.camera));
        ifd0.extend(self.software.as_deref().and_then(|s| ascii(0x0131, s)));
        let mut exif: Vec<E> = Vec::new();
        if let Some(t) = self.exposure_time.filter(|t| t.is_finite() && *t > 0.0) {
            exif.push(if t < 1.0 {
                rational(0x829A, 5, 1, (1.0 / t).round() as i64)
            } else {
                rational(0x829A, 5, (t * 10.0).round() as i64, 10)
            });
        }
        if let Some(f) = self.f_number.filter(|f| f.is_finite() && *f > 0.0) {
            exif.push(rational(0x829D, 5, (f * 10.0).round() as i64, 10));
        }
        if let Some(iso) = self.iso.filter(|&i| i > 0) {
            exif.push((0x8827, 3, 1, (iso.min(65535) as u16).to_le_bytes().to_vec()));
        }
        exif.extend(self.date_time.as_deref().and_then(|d| ascii(0x9003, d)));
        if let Some(b) = self.exposure_bias.filter(|b| b.is_finite()) {
            exif.push(rational(0x9204, 10, (b * 100.0).round() as i64, 100));
        }
        if let Some(f) = self.focal_length.filter(|f| f.is_finite() && *f > 0.0) {
            exif.push(rational(0x920A, 5, (f * 10.0).round() as i64, 10));
        }
        exif.extend(self.lens.as_deref().and_then(|l| ascii(0xA434, l)));
        if ifd0.is_empty() && exif.is_empty() {
            return None;
        }
        // Lays an IFD at `at`, its values after it.
        let lay = |at: u32, mut e: Vec<E>| -> Vec<u8> {
            e.sort_by_key(|x| x.0);
            let mut data = at + 2 + 12 * e.len() as u32 + 4;
            let (mut b, mut tail) = ((e.len() as u16).to_le_bytes().to_vec(), Vec::new());
            for (tag, typ, count, mut v) in e {
                b.extend(tag.to_le_bytes());
                b.extend(typ.to_le_bytes());
                b.extend(count.to_le_bytes());
                if v.len() <= 4 {
                    v.resize(4, 0);
                    b.extend(v);
                } else {
                    b.extend(data.to_le_bytes());
                    if v.len() % 2 == 1 {
                        v.push(0);
                    }
                    data += v.len() as u32;
                    tail.extend(v);
                }
            }
            b.extend([0; 4]);
            b.extend(tail);
            b
        };
        let mut out = b"II*\0\x08\0\0\0".to_vec();
        let exif_ifd = (!exif.is_empty()).then(|| {
            // IFD0 first (with the pointer), then the Exif IFD after it.
            let size0 = 2
                + 12 * (ifd0.len() + 1)
                + 4
                + ifd0
                    .iter()
                    .map(|e| {
                        if e.3.len() > 4 {
                            e.3.len().div_ceil(2) * 2
                        } else {
                            0
                        }
                    })
                    .sum::<usize>();
            8 + size0 as u32
        });
        if let Some(at) = exif_ifd {
            ifd0.push((0x8769, 4, 1, at.to_le_bytes().to_vec()));
        }
        out.extend(lay(8, ifd0));
        if let Some(at) = exif_ifd {
            debug_assert_eq!(out.len() as u32, at);
            out.extend(lay(at, exif));
        }
        Some(out)
    }
}

/// The colour space a TIFF structure's Exif names: ColorSpace (0xA001) 1 is sRGB; an
/// uncalibrated one with the interoperability index "R03" (DCF option file) is Adobe RGB.
pub(crate) fn exif_colour_space(tiff: &[u8]) -> Option<&'static str> {
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Ifd { b: tiff, le };
    if t.u16(2)? != 42 {
        return None;
    }
    let ifd0 = t.u32(4)? as usize;
    let x = t.u32(t.find(ifd0, 0x8769)? + 8)? as usize;
    let space = t.find(x, 0xA001).and_then(|e| t.u16(e + 8));
    if space == Some(1) {
        return Some("srgb");
    }
    let interop = t.find(x, 0xA005).and_then(|e| t.u32(e + 8))? as usize;
    let index = t.find(interop, 0x0001).and_then(|e| t.ascii(e))?;
    (index == "R03").then_some("adobergb")
}

/// The position a TIFF structure records, as (latitude, longitude) in signed decimal degrees
/// (south and west negative): GPSLatitude / GPSLongitude (degrees, minutes, seconds as
/// rationals, or fewer values) with their N/S and E/W references. None without both, or when
/// out of range.
pub fn exif_position(tiff: &[u8]) -> Option<(f64, f64)> {
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Ifd { b: tiff, le };
    if t.u16(2)? != 42 {
        return None;
    }
    let ifd0 = t.u32(4)? as usize;
    let g = t.u32(t.find(ifd0, 0x8825)? + 8)? as usize;
    // Up to three unsigned rationals: degrees, minutes, seconds.
    let dms = |tag: u16| -> Option<f64> {
        let e = t.find(g, tag)?;
        if t.u16(e + 2)? != 5 {
            return None;
        }
        let count = (t.u32(e + 4)? as usize).clamp(1, 3);
        let at = t.u32(e + 8)? as usize;
        let mut v = 0.0;
        for (i, scale) in [1.0, 60.0, 3600.0].into_iter().take(count).enumerate() {
            let o = at.checked_add(i * 8)?;
            let d = t.u32(o + 4)?;
            if d != 0 {
                v += t.u32(o)? as f64 / d as f64 / scale;
            }
        }
        v.is_finite().then_some(v)
    };
    // The reference letter: ASCII, inline for one character.
    let negative = |tag: u16, neg: u8| t.find(g, tag).and_then(|e| t.b.get(e + 8)) == Some(&neg);
    let mut lat = dms(0x0002)?;
    let mut lon = dms(0x0004)?;
    if negative(0x0001, b'S') {
        lat = -lat;
    }
    if negative(0x0003, b'W') {
        lon = -lon;
    }
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

/// Apple's HDR headroom (linear, ≥ 1) from the maker notes of an iPhone photo's Exif (TIFF
/// structure): tags 0x21 (`HDRHeadroom`) and 0x30 (`HDRGain`) of the Apple maker note, mapped
/// to stops by Apple's published piecewise-linear fit. Used for gain maps whose own XMP has no
/// headroom (`HDRGainMapVersion` < 2). `None` without an Apple maker note or those tags.
pub fn apple_hdr_headroom(tiff: &[u8]) -> Option<f32> {
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Ifd { b: tiff, le };
    if t.u16(2)? != 42 {
        return None;
    }
    let ifd0 = t.u32(4)? as usize;
    let exif = t.u32(t.find(ifd0, 0x8769)? + 8)? as usize;
    let mn = t.find(exif, 0x927C)?;
    let (len, off) = (t.u32(mn + 4)? as usize, t.u32(mn + 8)? as usize);
    let note = tiff.get(off..off.checked_add(len)?)?;
    // "Apple iOS\0", a version word, the byte order, then the IFD; offsets from the note start.
    if !note.starts_with(b"Apple iOS\0") {
        return None;
    }
    let le = match note.get(12..14)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let m = Ifd { b: note, le };
    let headroom = m.number(m.find(14, 0x21)?)?;
    let gain = m.number(m.find(14, 0x30)?)?;
    let stops = match (headroom < 1.0, gain <= 0.01) {
        (true, true) => -20.0 * gain + 1.8,
        (true, false) => -0.101 * gain + 1.601,
        (false, true) => -70.0 * gain + 3.0,
        (false, false) => -0.303 * gain + 2.303,
    };
    Some((stops.clamp(0.0, 8.0) as f32).exp2())
}

/// Date a PNG was taken, "YYYY:MM:DD HH:MM:SS": from its eXIf chunk (DateTimeOriginal /
/// DateTime), else from a tEXt or uncompressed iTXt "Creation Time" in RFC 1123 or ISO 8601
/// form. Walks the chunk headers and seeks over image data, reading only the small chunks.
pub fn png_date_taken<R: std::io::Read + std::io::Seek>(r: &mut R) -> Option<String> {
    use std::io::SeekFrom;
    let mut sig = [0u8; 8];
    r.read_exact(&mut sig).ok()?;
    if sig != *b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut created = None;
    for _ in 0..4096 {
        let mut h = [0u8; 8];
        r.read_exact(&mut h).ok()?;
        let len = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
        let kind = [h[4], h[5], h[6], h[7]];
        match &kind {
            b"IEND" => break,
            b"eXIf" | b"tEXt" | b"iTXt" if len <= 1 << 20 => {
                let mut data = vec![0u8; len as usize];
                r.read_exact(&mut data).ok()?;
                r.seek(SeekFrom::Current(4)).ok()?;
                if &kind == b"eXIf" {
                    if let Some(d) = exif_date_taken(&data) {
                        return Some(d);
                    }
                } else if created.is_none() {
                    created = png_creation_time(&kind, &data);
                }
            }
            _ => {
                r.seek(SeekFrom::Current(len as i64 + 4)).ok()?;
            }
        }
    }
    created
}

/// The "Creation Time" of a tEXt or iTXt chunk, normalised to EXIF form.
fn png_creation_time(kind: &[u8; 4], data: &[u8]) -> Option<String> {
    let rest = data.strip_prefix(b"Creation Time\0")?;
    let text = if kind == b"iTXt" {
        // compression flag, method, language\0, translated keyword\0, text
        if rest.first() != Some(&0) {
            return None;
        }
        let rest = rest.get(2..)?;
        let lang = rest.iter().position(|&b| b == 0)?;
        let rest = &rest[lang + 1..];
        let kw = rest.iter().position(|&b| b == 0)?;
        std::str::from_utf8(&rest[kw + 1..]).ok()?
    } else {
        // Latin-1; the dates are ASCII.
        std::str::from_utf8(rest).ok()?
    };
    parse_creation_time(text.trim())
}

/// "2016-08-02T07:05:00Z", "2016:08:02 07:05:00", "Tue, 02 Aug 2016 07:05:00 +0000" (the
/// form the PNG spec suggests) or a bare date, as "YYYY:MM:DD HH:MM:SS".
fn parse_creation_time(t: &str) -> Option<String> {
    let num = |s: &str, n: usize| -> Option<u32> {
        (s.len() == n && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let time = |s: Option<&str>| -> Option<(u32, u32, u32)> {
        let Some(s) = s else { return Some((0, 0, 0)) };
        let mut p = s.get(..8.min(s.len()))?.split(':');
        let h = num(p.next()?, 2)?;
        let m = num(p.next()?, 2)?;
        let sec = p.next().map_or(Some(0), |x| num(x, 2))?;
        (h < 24 && m < 60 && sec < 61).then_some((h, m, sec))
    };
    let (y, mo, d, hms) = if t.len() >= 10
        && t.as_bytes()[4] == t.as_bytes()[7]
        && matches!(t.as_bytes()[4], b'-' | b':')
    {
        // `get`: the bytes after the separators may start a multi-byte character.
        let (y, mo, d) = (
            num(t.get(..4)?, 4)?,
            num(t.get(5..7)?, 2)?,
            num(t.get(8..10)?, 2)?,
        );
        let rest = t.get(10..)?.trim_start_matches(['T', ' ']);
        (y, mo, d, time((!rest.is_empty()).then_some(rest))?)
    } else {
        const MONTHS: [&str; 12] = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ];
        let t = t.split_once(',').map_or(t, |(_, r)| r);
        let mut w = t.split_whitespace();
        let dd = w.next()?;
        let d = num(dd, dd.len().min(2)).filter(|_| dd.len() <= 2)?;
        let mon = w.next()?.to_ascii_lowercase();
        let mo = MONTHS.iter().position(|m| mon.starts_with(m))? as u32 + 1;
        let y = num(w.next()?, 4)?;
        (y, mo, d, time(w.next())?)
    };
    ((1..=12).contains(&mo) && (1..=31).contains(&d)).then(|| {
        format!(
            "{y:04}:{mo:02}:{d:02} {:02}:{:02}:{:02}",
            hms.0, hms.1, hms.2
        )
    })
}

#[cfg(test)]
mod tests {
    /// A RAW's camera facts written as Exif read back the same.
    #[test]
    fn camera_facts_as_exif() {
        let c = crate::CameraInfo {
            camera: "Canon EOS 300D DIGITAL".into(),
            lens: Some("EF-S18-55mm".into()),
            iso: Some(100),
            exposure_time: Some(1.0 / 25.0),
            f_number: Some(10.1),
            focal_length: Some(18.0),
            exposure_bias: Some(-0.67),
            date_time: Some("2007:06:18 17:37:18".into()),
            software: Some("Firmware 1.1.1".into()),
        };
        let t = c.exif_tiff().unwrap();
        let (back, gps) = super::exif_facts(&t).unwrap();
        assert!(!gps);
        assert_eq!(back.camera, c.camera);
        assert_eq!(back.lens, c.lens);
        assert_eq!(back.iso, Some(100));
        assert_eq!(back.exposure_time, Some(0.04));
        assert_eq!(back.f_number, Some(10.1));
        assert_eq!(back.focal_length, Some(18.0));
        assert_eq!(
            super::exif_date_taken(&t).as_deref(),
            Some("2007:06:18 17:37:18")
        );
        assert!(crate::CameraInfo::default().exif_tiff().is_none());
    }

    #[test]
    fn reads_orientation() {
        let mut t = b"MM\0\x2a\0\0\0\x08\0\x01".to_vec();
        t.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0]);
        assert_eq!(super::exif_orientation(&t), Some(6));
    }

    #[test]
    fn reads_camera_facts_and_gps() {
        // IFD0 (8): Make, Model, ExifIFD -> 62, GPS -> 98. Strings at 128 / 136.
        let le = |v: u32| v.to_le_bytes();
        let entry = |tag: u16, typ: u16, count: u32, value: u32| {
            let mut e = tag.to_le_bytes().to_vec();
            e.extend_from_slice(&typ.to_le_bytes());
            e.extend_from_slice(&le(count));
            e.extend_from_slice(&le(value));
            e
        };
        let mut t = b"II\x2a\0\x08\0\0\0".to_vec();
        t.extend_from_slice(&4u16.to_le_bytes());
        t.extend(entry(0x010F, 2, 9, 128));
        t.extend(entry(0x0110, 2, 5, 140));
        t.extend(entry(0x8769, 4, 1, 62));
        t.extend(entry(0x8825, 4, 1, 98));
        t.extend_from_slice(&[0; 4]);
        assert_eq!(t.len(), 62);
        t.extend_from_slice(&2u16.to_le_bytes());
        t.extend(entry(0x8827, 3, 1, 640));
        t.extend(entry(0x829D, 5, 1, 112));
        t.extend_from_slice(&[0; 4]);
        assert_eq!(t.len(), 92);
        t.extend_from_slice(&[0; 6]);
        // GPS IFD: version and latitude.
        t.extend_from_slice(&1u16.to_le_bytes());
        t.extend(entry(0x0002, 5, 3, 0));
        assert_eq!(t.len(), 112);
        t.extend_from_slice(&le(28));
        t.extend_from_slice(&le(10));
        t.resize(128, 0);
        t.extend_from_slice(b"FUJIFILM\0\0\0\0");
        t.extend_from_slice(b"X-T5\0");
        let (info, gps) = super::exif_facts(&t).unwrap();
        assert_eq!(info.camera, "FUJIFILM X-T5");
        assert_eq!(info.iso, Some(640));
        assert!((info.f_number.unwrap() - 2.8).abs() < 1e-9);
        assert!(gps);
        // No GPS IFD.
        let mut no = t.clone();
        no[46..48].copy_from_slice(&0x1234u16.to_le_bytes());
        assert!(!super::exif_facts(&no).unwrap().1);
        for n in 0..t.len() {
            let _ = super::exif_facts(&t[..n]);
        }
    }

    #[test]
    fn reads_colour_space() {
        // IFD0 (8): ExifIFD -> 26. Exif IFD: ColorSpace, Interop -> 56. Interop: index "R03".
        let entry = |tag: u16, typ: u16, count: u32, value: [u8; 4]| {
            let mut e = tag.to_le_bytes().to_vec();
            e.extend_from_slice(&typ.to_le_bytes());
            e.extend_from_slice(&count.to_le_bytes());
            e.extend_from_slice(&value);
            e
        };
        let tiff = |space: u16, index: &[u8; 4]| {
            let mut t = b"II\x2a\0\x08\0\0\0".to_vec();
            t.extend_from_slice(&1u16.to_le_bytes());
            t.extend(entry(0x8769, 4, 1, 26u32.to_le_bytes()));
            t.extend_from_slice(&[0; 4]);
            assert_eq!(t.len(), 26);
            t.extend_from_slice(&2u16.to_le_bytes());
            let [a, b] = space.to_le_bytes();
            t.extend(entry(0xA001, 3, 1, [a, b, 0, 0]));
            t.extend(entry(0xA005, 4, 1, 56u32.to_le_bytes()));
            t.extend_from_slice(&[0; 4]);
            assert_eq!(t.len(), 56);
            t.extend_from_slice(&1u16.to_le_bytes());
            t.extend(entry(0x0001, 2, 4, *index));
            t.extend_from_slice(&[0; 4]);
            t
        };
        assert_eq!(super::exif_colour_space(&tiff(1, b"R98\0")), Some("srgb"));
        // Uncalibrated, with the Adobe RGB (DCF option file) index.
        assert_eq!(
            super::exif_colour_space(&tiff(0xFFFF, b"R03\0")),
            Some("adobergb")
        );
        assert_eq!(super::exif_colour_space(&tiff(0xFFFF, b"R98\0")), None);
        let t = tiff(0xFFFF, b"R03\0");
        for n in 0..t.len() {
            let _ = super::exif_colour_space(&t[..n]);
        }
    }

    #[test]
    fn reads_position() {
        // IFD0 (8): GPS -> 26. GPS IFD: refs inline, rationals at 80 and 104. Made-up place.
        let be = |v: u32| v.to_be_bytes();
        let entry = |tag: u16, typ: u16, count: u32, value: [u8; 4]| {
            let mut e = tag.to_be_bytes().to_vec();
            e.extend_from_slice(&typ.to_be_bytes());
            e.extend_from_slice(&be(count));
            e.extend_from_slice(&value);
            e
        };
        let mut t = b"MM\0\x2a\0\0\0\x08\0\x01".to_vec();
        t.extend(entry(0x8825, 4, 1, be(26)));
        t.extend_from_slice(&[0; 4]);
        assert_eq!(t.len(), 26);
        t.extend_from_slice(&4u16.to_be_bytes());
        t.extend(entry(0x0001, 2, 2, *b"S\0\0\0"));
        t.extend(entry(0x0002, 5, 3, be(80)));
        t.extend(entry(0x0003, 2, 2, *b"W\0\0\0"));
        t.extend(entry(0x0004, 5, 3, be(104)));
        t.extend_from_slice(&[0; 4]);
        assert_eq!(t.len(), 80);
        for (n, d) in [(12, 1), (30, 1), (0, 1), (45, 1), (0, 1), (3600, 100)] {
            t.extend_from_slice(&be(n));
            t.extend_from_slice(&be(d));
        }
        let (lat, lon) = super::exif_position(&t).unwrap();
        assert!((lat + 12.5).abs() < 1e-9, "{lat}");
        assert!((lon + 45.01).abs() < 1e-9, "{lon}");
        // North and east without references.
        let mut ne = t.clone();
        ne[28..30].copy_from_slice(&0x7777u16.to_be_bytes());
        ne[52..54].copy_from_slice(&0x7778u16.to_be_bytes());
        let (lat, lon) = super::exif_position(&ne).unwrap();
        assert!(lat > 0.0 && lon > 0.0);
        // Out of range: 120 degrees of latitude.
        let mut far = t.clone();
        far[83] = 120;
        assert!(super::exif_position(&far).is_none());
        for n in 0..t.len() {
            assert!(super::exif_position(&t[..n]).is_none());
        }
    }

    #[test]
    fn reads_apple_hdr_headroom() {
        let be = |v: u32| v.to_be_bytes();
        let entry = |tag: u16, typ: u16, count: u32, value: u32| {
            let mut e = tag.to_be_bytes().to_vec();
            e.extend_from_slice(&typ.to_be_bytes());
            e.extend_from_slice(&be(count));
            e.extend_from_slice(&be(value));
            e
        };
        let note = |headroom: (u32, u32), gain: (u32, u32)| {
            let mut n = b"Apple iOS\0\0\x01MM\0\x02".to_vec();
            n.extend(entry(0x21, 10, 1, 44));
            n.extend(entry(0x30, 10, 1, 52));
            n.extend_from_slice(&[0; 4]);
            for (a, b) in [headroom, gain] {
                n.extend_from_slice(&be(a));
                n.extend_from_slice(&be(b));
            }
            n
        };
        let tiff = |note: &[u8]| {
            // IFD0 (8) -> Exif IFD (26) -> maker note (44).
            let mut t = b"MM\0\x2a\0\0\0\x08\0\x01".to_vec();
            t.extend(entry(0x8769, 4, 1, 26));
            t.extend_from_slice(&[0, 0, 0, 0, 0, 1]);
            t.extend(entry(0x927C, 7, note.len() as u32, 44));
            t.extend_from_slice(&[0; 4]);
            t.extend_from_slice(note);
            t
        };
        // The iPhone 15 Pro HEIC case: 1.01 / 0.0203 -> 2.2968 stops (its tmap says 2.296875).
        let h = super::apple_hdr_headroom(&tiff(&note((101, 100), (203, 10_000)))).unwrap();
        assert!((h.log2() - 2.2968).abs() < 1e-3, "{h}");
        // Lower branch: headroom < 1, small gain.
        let h = super::apple_hdr_headroom(&tiff(&note((5, 10), (5, 1000)))).unwrap();
        assert!((h.log2() - 1.7).abs() < 1e-3, "{h}");
        // Not an Apple note; truncated.
        let mut other = note((1, 1), (0, 1));
        other[..5].copy_from_slice(b"Canon");
        assert!(super::apple_hdr_headroom(&tiff(&other)).is_none());
        let t = tiff(&note((1, 1), (0, 1)));
        for n in 0..t.len() {
            assert!(super::apple_hdr_headroom(&t[..n]).is_none());
        }
    }

    #[test]
    fn reads_date_taken() {
        // IFD0 with one entry: ExifIFD pointer -> IFD at 26 with DateTimeOriginal at 44.
        let mut t = b"II\x2a\0\x08\0\0\0".to_vec();
        t.extend_from_slice(&[1, 0, 0x69, 0x87, 4, 0, 1, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0]);
        t.extend_from_slice(&[1, 0, 0x03, 0x90, 2, 0, 20, 0, 0, 0, 44, 0, 0, 0, 0, 0, 0, 0]);
        t.extend_from_slice(b"2024:05:06 07:08:09\0");
        assert_eq!(
            super::exif_date_taken(&t).as_deref(),
            Some("2024:05:06 07:08:09")
        );
        let mut j = vec![0xFF, 0xD8, 0xFF, 0xE1];
        j.extend_from_slice(&((t.len() + 8) as u16).to_be_bytes());
        j.extend_from_slice(b"Exif\0\0");
        j.extend_from_slice(&t);
        assert_eq!(super::jpeg_exif(&j), Some(&t[..]));
    }
    #[test]
    fn hostile_creation_time_characters() {
        // A multi-byte character straddling the day's end, or right after a separator.
        for t in [
            "2016-08-0\u{e9}",
            "2016-08-0\u{e9}T07:05",
            "2016-\u{e9}-02",
            "2016-08-\u{20ac}",
        ] {
            assert_eq!(super::parse_creation_time(t), None, "{t}");
        }
    }

    #[test]
    fn reads_png_creation_time() {
        fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(kind);
            c.extend_from_slice(data);
            c.extend_from_slice(&[0; 4]);
            c
        }
        let png = |chunks: &[Vec<u8>]| {
            let mut f = b"\x89PNG\r\n\x1a\n".to_vec();
            f.extend(chunk(b"IHDR", &[0; 13]));
            f.extend(chunk(b"IDAT", &[0; 100]));
            for c in chunks {
                f.extend_from_slice(c);
            }
            f.extend(chunk(b"IEND", &[]));
            std::io::Cursor::new(f)
        };
        let read = |chunks: &[Vec<u8>]| super::png_date_taken(&mut png(chunks));
        let text = |v: &str| chunk(b"tEXt", format!("Creation Time\0{v}").as_bytes());
        assert_eq!(
            read(&[text("Tue, 02 Aug 2016 07:05:00 +0000")]).as_deref(),
            Some("2016:08:02 07:05:00")
        );
        assert_eq!(
            read(&[text("2016-08-02T07:05:09Z")]).as_deref(),
            Some("2016:08:02 07:05:09")
        );
        assert_eq!(
            read(&[text("2016-08-02")]).as_deref(),
            Some("2016:08:02 00:00:00")
        );
        assert_eq!(read(&[text("yesterday")]), None);
        assert_eq!(read(&[chunk(b"tEXt", b"Author\0x")]), None);
        let itxt = chunk(b"iTXt", b"Creation Time\0\0\0en\0\x002 Aug 2016 07:05");
        assert_eq!(read(&[itxt]).as_deref(), Some("2016:08:02 07:05:00"));
        // eXIf wins over tEXt.
        let mut t = b"II\x2a\0\x08\0\0\0".to_vec();
        t.extend_from_slice(&[1, 0, 0x32, 0x01, 2, 0, 20, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0]);
        t.extend_from_slice(b"2024:05:06 07:08:09\0");
        assert_eq!(
            read(&[text("2016-08-02"), chunk(b"eXIf", &t)]).as_deref(),
            Some("2024:05:06 07:08:09")
        );
    }
}

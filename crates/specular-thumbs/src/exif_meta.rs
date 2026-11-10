//! The Exif fields search names beyond the camera settings ([`ExifMeta`]), read from the Exif
//! TIFF structure [`crate::generate::meta_of`] already has in hand (no other read): flash,
//! white balance, exposure program and compensation, metering, the 35 mm focal length, maker,
//! serial numbers, software, artist, copyright, description, GPS altitude, and every tag
//! `exif:` names ([`specular_query::exif::TAGS`]) as text for the Library.

use specular_query::exif::{self, TagKind};

/// What the Exif says beyond [`crate::Meta`]'s camera settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExifMeta {
    pub make: Option<String>,
    pub serial: Option<String>,
    pub lens_serial: Option<String>,
    pub software: Option<String>,
    pub artist: Option<String>,
    pub copyright: Option<String>,
    /// ImageDescription, unless it is a camera's placeholder ("OLYMPUS DIGITAL CAMERA").
    pub description: Option<String>,
    pub flash: Option<u16>,
    pub white_balance: Option<u8>,
    pub light_source: Option<u16>,
    /// EV.
    pub exposure_bias: Option<f32>,
    pub exposure_program: Option<u8>,
    pub exposure_mode: Option<u8>,
    pub metering: Option<u16>,
    /// FocalLengthIn35mmFilm, else the focal length times the crop factor the focal plane
    /// resolution gives.
    pub focal35: Option<f32>,
    /// Metres; below sea level negative.
    pub altitude: Option<f32>,
    /// Every tag `exif:` names ([`specular_query::exif::push_tag`]). None where not kept: the
    /// thumbnail cache does not keep them (the gallery does not search them), the Library
    /// loads them for `exif:` searches only.
    pub tags: Option<String>,
}

/// Longest text value kept, in bytes.
const MAX_TEXT: usize = 200;
/// Most numbers kept of one tag.
const MAX_NUMBERS: usize = 8;

/// Placeholders cameras write as the description.
const NO_DESCRIPTION: [&str; 6] = [
    "OLYMPUS DIGITAL CAMERA",
    "SONY DSC",
    "DIGITAL CAMERA",
    "KODAK Digital Still Camera",
    "Exif_JPEG_PICTURE",
    "SAMSUNG",
];

struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
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

    /// The entries of the IFD at `ifd`: (tag, type, count, offset of the value).
    fn entries(&self, ifd: usize) -> impl Iterator<Item = (u16, u16, usize, usize)> + '_ {
        let n = self.u16(ifd).unwrap_or(0) as usize;
        (0..n.min(512)).filter_map(move |i| {
            let e = ifd.checked_add(2 + i * 12)?;
            let (tag, typ, count) = (self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)? as usize);
            let size = count.checked_mul(type_size(typ)?)?;
            let at = if size <= 4 {
                e + 8
            } else {
                self.u32(e + 8)? as usize
            };
            self.b.get(at..at.checked_add(size)?)?;
            Some((tag, typ, count, at))
        })
    }

    /// The `k`th number of a numeric value.
    fn number(&self, typ: u16, at: usize, k: usize) -> Option<f64> {
        let o = at + k * type_size(typ)?;
        let v = match typ {
            1 | 7 => f64::from(*self.b.get(o)?),
            6 => f64::from(*self.b.get(o)? as i8),
            3 => f64::from(self.u16(o)?),
            8 => f64::from(self.u16(o)? as i16),
            4 => f64::from(self.u32(o)?),
            9 => f64::from(self.u32(o)? as i32),
            5 | 10 => {
                let (n, d) = (self.u32(o)?, self.u32(o + 4)?);
                if d == 0 {
                    return None;
                }
                if typ == 5 {
                    f64::from(n) / f64::from(d)
                } else {
                    f64::from(n as i32) / f64::from(d as i32)
                }
            }
            11 => f64::from(f32::from_bits(self.u32(o)?)),
            12 => {
                let b: [u8; 8] = self.b.get(o..o + 8)?.try_into().ok()?;
                if self.le {
                    f64::from_le_bytes(b)
                } else {
                    f64::from_be_bytes(b)
                }
            }
            _ => return None,
        };
        v.is_finite().then_some(v)
    }

    /// Text: ASCII up to its first NUL (UTF-8, else Latin-1), trimmed; a UserComment-style
    /// value (8 bytes naming its character code first) decoded.
    fn text(&self, typ: u16, count: usize, at: usize) -> Option<String> {
        let raw = self.b.get(at..at + count)?;
        let raw = if typ == 7 && raw.len() >= 8 {
            match &raw[..8] {
                b"ASCII\0\0\0" | b"\0\0\0\0\0\0\0\0" => &raw[8..],
                b"UNICODE\0" => {
                    let units = raw[8..].chunks_exact(2).map(|c| {
                        if self.le {
                            u16::from_le_bytes([c[0], c[1]])
                        } else {
                            u16::from_be_bytes([c[0], c[1]])
                        }
                    });
                    let s: String = char::decode_utf16(units)
                        .map(|c| c.unwrap_or('\u{FFFD}'))
                        .take_while(|c| *c != '\0')
                        .collect();
                    return clean(&s);
                }
                _ if raw
                    .iter()
                    .all(|b| *b == 0 || b.is_ascii_graphic() || *b == b' ') =>
                {
                    raw
                }
                _ => return None,
            }
        } else {
            raw
        };
        let raw = &raw[..raw.iter().position(|b| *b == 0).unwrap_or(raw.len())];
        let s = match std::str::from_utf8(raw) {
            Ok(s) => s.to_owned(),
            Err(_) => raw.iter().map(|b| char::from(*b)).collect(),
        };
        clean(&s)
    }
}

fn clean(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut end = s.len().min(MAX_TEXT);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    Some(s[..end].trim_end().to_owned())
}

fn type_size(typ: u16) -> Option<usize> {
    Some(match typ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

/// A number in decimal, six significant digits at most (`0.004`, `2.8`, `-0.333333`).
fn fmt_num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let digits = (5 - v.abs().log10().floor() as i32).clamp(0, 12) as usize;
    let s = format!("{v:.digits$}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_owned() }
}

/// The fields of an Exif TIFF structure (None: not one). Tags and named fields come from IFD0,
/// the Exif IFD and the GPS IFD.
pub fn exif_meta(tiff: &[u8]) -> Option<ExifMeta> {
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Tiff { b: tiff, le };
    if t.u16(2)? != 42 {
        return None;
    }
    let ifd0 = t.u32(4)? as usize;
    let mut m = ExifMeta::default();
    let mut tags = String::new();
    // Numbers wanted for the named fields: (gps, id) → first value.
    let mut nums: Vec<((bool, u16), f64)> = Vec::new();
    let mut ifds = vec![(false, ifd0)];
    let mut seen = Vec::new();
    while let Some((gps, ifd)) = ifds.pop() {
        if seen.contains(&ifd) || seen.len() > 3 {
            continue;
        }
        seen.push(ifd);
        for (tag, typ, count, at) in t.entries(ifd) {
            if !gps && matches!(tag, 0x8769 | 0x8825) {
                if let Some(o) = t.number(typ, at, 0) {
                    ifds.push((tag == 0x8825, o as usize));
                }
                continue;
            }
            let Some(def) = exif::tag_of(gps, tag) else {
                continue;
            };
            let numeric = !matches!(typ, 2) && !(typ == 7 && def.kind == TagKind::Text);
            let value = if numeric {
                let n = count.clamp(0, MAX_NUMBERS);
                let v: Vec<f64> = (0..n).filter_map(|k| t.number(typ, at, k)).collect();
                if v.is_empty() {
                    continue;
                }
                nums.push(((gps, tag), v[0]));
                // Degrees, minutes, seconds as degrees.
                if gps && matches!(tag, 2 | 4 | 0x14 | 0x16) && v.len() == 3 {
                    fmt_num(v[0] + v[1] / 60.0 + v[2] / 3600.0)
                } else {
                    v.iter().map(|x| fmt_num(*x)).collect::<Vec<_>>().join(" ")
                }
            } else {
                let Some(s) = t.text(typ, count, at) else {
                    continue;
                };
                let slot = match (gps, tag) {
                    (false, 0x010F) => Some(&mut m.make),
                    (false, 0xA431) => Some(&mut m.serial),
                    (false, 0xA435) => Some(&mut m.lens_serial),
                    (false, 0x0131) => Some(&mut m.software),
                    (false, 0x013B) => Some(&mut m.artist),
                    (false, 0x8298) => Some(&mut m.copyright),
                    (false, 0x010E)
                        if !NO_DESCRIPTION.iter().any(|d| d.eq_ignore_ascii_case(&s)) =>
                    {
                        Some(&mut m.description)
                    }
                    _ => None,
                };
                if let Some(slot) = slot {
                    slot.get_or_insert_with(|| s.clone());
                }
                s
            };
            exif::push_tag(&mut tags, gps, tag, &value);
        }
    }
    let num = |gps: bool, id: u16| nums.iter().find(|(k, _)| *k == (gps, id)).map(|(_, v)| *v);
    let small = |v: f64, max: f64| (v >= 0.0 && v <= max).then_some(v);
    m.flash = num(false, 0x9209)
        .and_then(|v| small(v, 65535.0))
        .map(|v| v as u16);
    m.white_balance = num(false, 0xA403)
        .and_then(|v| small(v, 255.0))
        .map(|v| v as u8);
    m.light_source = num(false, 0x9208)
        .and_then(|v| small(v, 65535.0))
        .map(|v| v as u16);
    m.exposure_bias = num(false, 0x9204)
        .filter(|v| v.abs() < 100.0)
        .map(|v| v as f32);
    m.exposure_program = num(false, 0x8822)
        .and_then(|v| small(v, 255.0))
        .map(|v| v as u8);
    m.exposure_mode = num(false, 0xA402)
        .and_then(|v| small(v, 255.0))
        .map(|v| v as u8);
    m.metering = num(false, 0x9207)
        .and_then(|v| small(v, 65535.0))
        .map(|v| v as u16);
    m.focal35 = num(false, 0xA405)
        .filter(|v| *v > 0.0)
        .or_else(|| {
            let f = num(false, 0x920A)?;
            let crop = crop_factor(
                num(false, 0xA002)?,
                num(false, 0xA003)?,
                num(false, 0xA20E)?,
                num(false, 0xA20F)?,
                num(false, 0xA210).unwrap_or(2.0),
            )?;
            Some((f * crop).round())
        })
        .map(|v| v as f32);
    m.altitude = num(true, 6).filter(|v| v.abs() < 1e6).map(|a| {
        let below = num(true, 5) == Some(1.0);
        (if below { -a } else { a }) as f32
    });
    m.tags = Some(tags);
    Some(m)
}

/// The crop factor a sensor of `w` × `h` pixels at `xres` × `yres` pixels per unit (2 inch, 3
/// cm, 4 mm, 5 µm) has against 35 mm film (43.27 mm diagonal), when plausible.
fn crop_factor(w: f64, h: f64, xres: f64, yres: f64, unit: f64) -> Option<f64> {
    let mm = match unit as u32 {
        2 => 25.4,
        3 => 10.0,
        4 => 1.0,
        5 => 0.001,
        _ => return None,
    };
    if !(xres > 0.0 && yres > 0.0 && w > 0.0 && h > 0.0) {
        return None;
    }
    let (sw, sh) = (w / xres * mm, h / yres * mm);
    let crop = 43.27 / (sw * sw + sh * sh).sqrt();
    (0.5..=12.0).contains(&crop).then_some(crop)
}

impl ExifMeta {
    /// The query's view of it.
    pub fn facts(&self) -> specular_query::ExifFacts<'_> {
        specular_query::ExifFacts {
            make: self.make.as_deref(),
            serial: self.serial.as_deref(),
            lens_serial: self.lens_serial.as_deref(),
            software: self.software.as_deref(),
            artist: self.artist.as_deref(),
            copyright: self.copyright.as_deref(),
            description: self.description.as_deref(),
            flash: self.flash,
            white_balance: self.white_balance,
            light_source: self.light_source,
            exposure_bias: self.exposure_bias,
            exposure_program: self.exposure_program,
            exposure_mode: self.exposure_mode,
            metering: self.metering,
            focal35: self.focal35,
            altitude: self.altitude,
            tags: self.tags.as_deref(),
        }
    }

    /// The named fields as one record (not the tags), for the cache entry and the catalogue.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        self.write(&mut b);
        b
    }

    /// Reads [`ExifMeta::to_bytes`] (`tags` None).
    pub fn from_bytes(b: &[u8]) -> Option<ExifMeta> {
        Self::read(&mut crate::cache::Reader::new(b))
    }

    fn strings(&self) -> [&Option<String>; 7] {
        [
            &self.make,
            &self.serial,
            &self.lens_serial,
            &self.software,
            &self.artist,
            &self.copyright,
            &self.description,
        ]
    }

    pub(crate) fn write(&self, b: &mut Vec<u8>) {
        let mut present = 0u16;
        let opts = [
            self.flash.is_some(),
            self.white_balance.is_some(),
            self.light_source.is_some(),
            self.exposure_bias.is_some(),
            self.exposure_program.is_some(),
            self.exposure_mode.is_some(),
            self.metering.is_some(),
            self.focal35.is_some(),
            self.altitude.is_some(),
        ];
        for (i, p) in self
            .strings()
            .iter()
            .map(|s| s.is_some())
            .chain(opts)
            .enumerate()
        {
            present |= u16::from(p) << i;
        }
        b.extend_from_slice(&present.to_le_bytes());
        for s in self.strings().into_iter().flatten() {
            crate::cache::put_str(b, s);
        }
        for v in [self.flash, self.light_source].into_iter().flatten() {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [
            self.white_balance,
            self.exposure_program,
            self.exposure_mode,
        ]
        .into_iter()
        .flatten()
        {
            b.push(v);
        }
        if let Some(v) = self.metering {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [self.exposure_bias, self.focal35, self.altitude]
            .into_iter()
            .flatten()
        {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }

    pub(crate) fn read(r: &mut crate::cache::Reader) -> Option<ExifMeta> {
        let present = r.u16()?;
        let on = |i: u32| present & (1 << i) != 0;
        let mut m = ExifMeta::default();
        let mut strs: [Option<String>; 7] = Default::default();
        for (i, s) in strs.iter_mut().enumerate() {
            if on(i as u32) {
                *s = Some(r.str()?);
            }
        }
        let [
            make,
            serial,
            lens_serial,
            software,
            artist,
            copyright,
            description,
        ] = strs;
        m.make = make;
        m.serial = serial;
        m.lens_serial = lens_serial;
        m.software = software;
        m.artist = artist;
        m.copyright = copyright;
        m.description = description;
        if on(7) {
            m.flash = Some(r.u16()?);
        }
        if on(9) {
            m.light_source = Some(r.u16()?);
        }
        if on(8) {
            m.white_balance = Some(r.u8()?);
        }
        if on(11) {
            m.exposure_program = Some(r.u8()?);
        }
        if on(12) {
            m.exposure_mode = Some(r.u8()?);
        }
        if on(13) {
            m.metering = Some(r.u16()?);
        }
        if on(10) {
            m.exposure_bias = Some(f32::from_bits(r.u32()?));
        }
        if on(14) {
            m.focal35 = Some(f32::from_bits(r.u32()?));
        }
        if on(15) {
            m.altitude = Some(f32::from_bits(r.u32()?));
        }
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A little-endian TIFF: IFD0 (Make, Software, Artist, ImageDescription, Exif and GPS
    /// pointers), an Exif IFD and a GPS IFD.
    pub(crate) fn sample() -> Vec<u8> {
        // (tag, type, count, value bytes)
        type E = (u16, u16, u32, Vec<u8>);
        let ascii = |tag, s: &str| -> E {
            let mut v = s.as_bytes().to_vec();
            v.push(0);
            (tag, 2, v.len() as u32, v)
        };
        let short = |tag, v: u16| -> E { (tag, 3, 1, v.to_le_bytes().to_vec()) };
        let rational = |tag, typ, n: i32, d: u32| -> E {
            let mut v = (n as u32).to_le_bytes().to_vec();
            v.extend(d.to_le_bytes());
            (tag, typ, 1, v)
        };
        let mut user = b"ASCII\0\0\0".to_vec();
        user.extend(b"hello there");
        let exif: Vec<E> = vec![
            rational(0x829A, 5, 1, 250),
            short(0x8822, 3),
            rational(0x9204, 10, -1, 3),
            short(0x9207, 5),
            short(0x9208, 10),
            short(0x9209, 0x59),
            rational(0x920A, 5, 50, 1),
            (0x9286, 7, user.len() as u32, user),
            (0x927C, 7, 4, b"junk".to_vec()),
            short(0xA402, 0),
            short(0xA403, 1),
            short(0xA405, 75),
            short(0xA406, 3),
            ascii(0xA431, "SN 0042"),
        ];
        let gps: Vec<E> = vec![
            (0x0005, 1, 1, vec![1, 0, 0, 0]),
            rational(0x0006, 5, 25, 2),
            {
                let mut v = Vec::new();
                for (n, d) in [(52u32, 1u32), (30, 1), (0, 1)] {
                    v.extend(n.to_le_bytes());
                    v.extend(d.to_le_bytes());
                }
                (0x0002, 5, 3, v)
            },
        ];
        let ifd0: Vec<E> = vec![
            ascii(0x010E, "OLYMPUS DIGITAL CAMERA"),
            ascii(0x010F, "Canon"),
            ascii(0x0131, "Darktable 4.6"),
            ascii(0x013B, "Jane Doe"),
            (0x8769, 4, 1, vec![0; 4]),
            (0x8825, 4, 1, vec![0; 4]),
        ];
        // Layout: header, IFD0, Exif IFD, GPS IFD, then the out-of-line values.
        let size = |n: usize| 2 + n * 12 + 4;
        let at0 = 8;
        let at_exif = at0 + size(ifd0.len());
        let at_gps = at_exif + size(exif.len());
        let mut data_at = at_gps + size(gps.len());
        let mut out = b"II*\0".to_vec();
        out.extend((at0 as u32).to_le_bytes());
        let mut data = Vec::new();
        for (ifd, ptrs) in [(&ifd0, true), (&exif, false), (&gps, false)] {
            out.extend((ifd.len() as u16).to_le_bytes());
            for (tag, typ, count, v) in ifd {
                out.extend(tag.to_le_bytes());
                out.extend(typ.to_le_bytes());
                out.extend(count.to_le_bytes());
                let v = match (*tag, ptrs) {
                    (0x8769, true) => (at_exif as u32).to_le_bytes().to_vec(),
                    (0x8825, true) => (at_gps as u32).to_le_bytes().to_vec(),
                    _ => v.clone(),
                };
                if v.len() <= 4 {
                    let mut v = v;
                    v.resize(4, 0);
                    out.extend(v);
                } else {
                    out.extend((data_at as u32).to_le_bytes());
                    data_at += v.len();
                    data.extend(v);
                }
            }
            out.extend(0u32.to_le_bytes());
        }
        out.extend(data);
        out
    }

    #[test]
    fn reads_the_named_fields_and_tags() {
        let m = exif_meta(&sample()).expect("a TIFF");
        assert_eq!(m.make.as_deref(), Some("Canon"));
        assert_eq!(m.software.as_deref(), Some("Darktable 4.6"));
        assert_eq!(m.artist.as_deref(), Some("Jane Doe"));
        assert_eq!(m.serial.as_deref(), Some("SN 0042"));
        // The camera's placeholder is no description.
        assert_eq!(m.description, None);
        assert_eq!(m.flash, Some(0x59));
        assert_eq!(m.white_balance, Some(1));
        assert_eq!(m.light_source, Some(10));
        assert_eq!(m.exposure_program, Some(3));
        assert_eq!(m.exposure_mode, Some(0));
        assert_eq!(m.metering, Some(5));
        assert!((m.exposure_bias.unwrap() + 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(m.focal35, Some(75.0));
        assert_eq!(m.altitude, Some(-12.5));
        let tags = m.tags.as_deref().unwrap();
        assert_eq!(exif::value_of(tags, "a406"), Some("3"));
        assert_eq!(exif::value_of(tags, "829a"), Some("0.004"));
        assert_eq!(exif::value_of(tags, "9204"), Some("-0.333333"));
        assert_eq!(exif::value_of(tags, "9286"), Some("hello there"));
        assert_eq!(exif::value_of(tags, "g0006"), Some("12.5"));
        assert_eq!(exif::value_of(tags, "g0002"), Some("52.5"));
        assert_eq!(exif::value_of(tags, "010e"), Some("OLYMPUS DIGITAL CAMERA"));
        // The MakerNote and the pointers are not kept.
        assert_eq!(exif::value_of(tags, "927c"), None);
        assert_eq!(exif::value_of(tags, "8769"), None);
        // The record round-trips (without the tags).
        let back = ExifMeta::from_bytes(&m.to_bytes()).unwrap();
        assert_eq!(
            back,
            ExifMeta {
                tags: None,
                ..m.clone()
            }
        );
        assert_eq!(
            ExifMeta::from_bytes(&ExifMeta::default().to_bytes()),
            Some(ExifMeta::default())
        );
        assert!(exif_meta(b"not a tiff").is_none());
        assert!(exif_meta(&sample()[..20]).is_some());
    }

    #[test]
    fn numbers_read_short() {
        assert_eq!(fmt_num(0.004), "0.004");
        assert_eq!(fmt_num(2.8), "2.8");
        assert_eq!(fmt_num(-1.0 / 3.0), "-0.333333");
        assert_eq!(fmt_num(1.0 / 3200.0), "0.0003125");
        assert_eq!(fmt_num(100.0), "100");
        assert_eq!(fmt_num(123456.7), "123457");
    }

    #[test]
    fn crop_factors() {
        // A full-frame 6000 × 4000 sensor: 36 × 24 mm.
        let c = crop_factor(6000.0, 4000.0, 6000.0 / 3.6, 4000.0 / 2.4, 3.0).unwrap();
        assert!((c - 1.0).abs() < 0.01);
        assert!(crop_factor(6000.0, 4000.0, 0.0, 1.0, 2.0).is_none());
    }
}

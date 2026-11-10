//! The few facts import needs from a file: capture date and camera name. Exif is located by
//! `specular_decode` (JPEG, HEIC, CR3, TIFF-based RAW); this reads IFD0's Make, Model and
//! DateTime and the Exif IFD's DateTimeOriginal. Videos give their QuickTime `mvhd` creation
//! time.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// What was found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    /// Capture date (local wall clock): year, month, day, hour, minute, second.
    pub date: Option<[u32; 6]>,
    /// "Make Model", with the make dropped when the model already starts with it.
    pub camera: Option<String>,
}

/// Exif from a still, or the movie header from a video.
pub(crate) fn read(path: &Path, video: bool) -> Facts {
    let Ok(f) = File::open(path) else {
        return Facts::default();
    };
    if video {
        return Facts {
            date: mvhd_date(BufReader::new(f)),
            camera: None,
        };
    }
    specular_decode::read_exif_tiff(BufReader::new(f))
        .map(|t| from_tiff(&t))
        .unwrap_or_default()
}

struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let s: [u8; 2] = self.b.get(at..at + 2)?.try_into().ok()?;
        Some(if self.le {
            u16::from_le_bytes(s)
        } else {
            u16::from_be_bytes(s)
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(at..at + 4)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        })
    }

    /// The 12-byte entry for `tag` in the IFD at `ifd`.
    fn find(&self, ifd: usize, tag: u16) -> Option<usize> {
        let n = self.u16(ifd)? as usize;
        (0..n.min(1000))
            .map(|i| ifd + 2 + 12 * i)
            .find(|&e| self.u16(e) == Some(tag))
    }

    fn ascii(&self, entry: usize) -> Option<String> {
        if self.u16(entry + 2)? != 2 {
            return None;
        }
        let n = self.u32(entry + 4)? as usize;
        let at = if n <= 4 {
            entry + 8
        } else {
            self.u32(entry + 8)? as usize
        };
        let raw = self.b.get(at..at.checked_add(n)?)?;
        let raw = raw.split(|&c| c == 0).next().unwrap_or(raw);
        let s = String::from_utf8_lossy(raw).trim().to_owned();
        (!s.is_empty()).then_some(s)
    }
}

/// Facts from a TIFF-structured Exif payload.
pub(crate) fn from_tiff(b: &[u8]) -> Facts {
    let le = match b.get(..4) {
        Some([b'I', b'I', 42, 0]) => true,
        Some([b'M', b'M', 0, 42]) => false,
        _ => return Facts::default(),
    };
    let t = Tiff { b, le };
    let Some(ifd0) = t.u32(4).map(|o| o as usize) else {
        return Facts::default();
    };
    let get = |ifd: usize, tag: u16| t.find(ifd, tag).and_then(|e| t.ascii(e));
    let original = t
        .find(ifd0, 0x8769)
        .and_then(|e| t.u32(e + 8))
        .and_then(|x| get(x as usize, 0x9003));
    let date = original
        .or_else(|| get(ifd0, 0x0132))
        .and_then(|s| specular_convert::parse_exif_date(&s));
    let make = get(ifd0, 0x010F);
    let model = get(ifd0, 0x0110);
    Facts {
        date,
        camera: camera_name(make.as_deref(), model.as_deref()),
    }
}

/// "Canon" + "Canon EOS R5" is "Canon EOS R5"; "NIKON CORPORATION" + "NIKON Z 6" is
/// "NIKON Z 6"; "Apple" + "iPhone 15" is "Apple iPhone 15".
pub(crate) fn camera_name(make: Option<&str>, model: Option<&str>) -> Option<String> {
    let make = make.map(str::trim).filter(|s| !s.is_empty());
    let model = model.map(str::trim).filter(|s| !s.is_empty());
    match (make, model) {
        (Some(m), Some(md)) => {
            let first = m.split_whitespace().next().unwrap_or(m);
            if md.to_lowercase().starts_with(&first.to_lowercase()) {
                Some(md.to_owned())
            } else {
                Some(format!("{first} {md}"))
            }
        }
        (None, Some(md)) => Some(md.to_owned()),
        (Some(m), None) => Some(m.to_owned()),
        (None, None) => None,
    }
}

/// QuickTime/MP4 `moov/mvhd` creation time as a local date (seconds since 1904, UTC).
pub(crate) fn mvhd_date<R: Read + Seek>(mut r: R) -> Option<[u32; 6]> {
    let end = r.seek(SeekFrom::End(0)).ok()?;
    let moov = find_box(&mut r, 0, end, b"moov")?;
    let mvhd = find_box(&mut r, moov.0, moov.1, b"mvhd")?;
    r.seek(SeekFrom::Start(mvhd.0)).ok()?;
    let mut v = [0u8; 4];
    r.read_exact(&mut v).ok()?;
    let secs = if v[0] == 1 {
        let mut b = [0u8; 8];
        r.read_exact(&mut b).ok()?;
        u64::from_be_bytes(b)
    } else {
        let mut b = [0u8; 4];
        r.read_exact(&mut b).ok()?;
        u64::from(u32::from_be_bytes(b))
    };
    // Zero means "not set"; anything before 1990 is a camera that never had its clock set.
    const UNIX_1904: u64 = 2_082_844_800;
    let unix = i64::try_from(secs.checked_sub(UNIX_1904)?).ok()?;
    if unix < 631_152_000 {
        return None;
    }
    local(chrono::DateTime::from_timestamp(unix, 0)?)
}

/// The body range `(start, end)` of the first box of `kind` among the boxes in `[from, to)`.
fn find_box<R: Read + Seek>(r: &mut R, from: u64, to: u64, kind: &[u8; 4]) -> Option<(u64, u64)> {
    let mut at = from;
    for _ in 0..4096 {
        if at + 8 > to {
            return None;
        }
        r.seek(SeekFrom::Start(at)).ok()?;
        let mut h = [0u8; 8];
        r.read_exact(&mut h).ok()?;
        let size = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let (head, size) = match size {
            0 => (8, to - at),
            1 => {
                let mut b = [0u8; 8];
                r.read_exact(&mut b).ok()?;
                (16, u64::from_be_bytes(b))
            }
            s => (8, s),
        };
        if size < head || at.checked_add(size)? > to {
            return None;
        }
        if &h[4..8] == kind {
            return Some((at + head, at + size));
        }
        at += size;
    }
    None
}

/// A time as local wall-clock fields.
pub(crate) fn local<Tz: chrono::TimeZone>(t: chrono::DateTime<Tz>) -> Option<[u32; 6]> {
    use chrono::{Datelike, Timelike};
    let t = t.with_timezone(&chrono::Local);
    Some([
        u32::try_from(t.year()).ok()?,
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
    ])
}

/// A modification time as local wall-clock fields.
pub(crate) fn local_mtime(t: std::time::SystemTime) -> [u32; 6] {
    local(chrono::DateTime::<chrono::Utc>::from(t)).unwrap_or([1970, 1, 1, 0, 0, 0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{exif_jpeg, mp4};

    #[test]
    fn tiff_facts() {
        let j = exif_jpeg("Canon", "Canon EOS R5", "2024:03:09 14:05:59", 10);
        let t = specular_decode::read_exif_tiff(std::io::Cursor::new(j)).unwrap();
        assert_eq!(
            from_tiff(&t),
            Facts {
                date: Some([2024, 3, 9, 14, 5, 59]),
                camera: Some("Canon EOS R5".into()),
            }
        );
        assert_eq!(from_tiff(b"junk"), Facts::default());
    }

    #[test]
    fn camera_names() {
        assert_eq!(
            camera_name(Some("NIKON CORPORATION"), Some("NIKON Z 6")).as_deref(),
            Some("NIKON Z 6")
        );
        assert_eq!(
            camera_name(Some("Apple"), Some("iPhone 15")).as_deref(),
            Some("Apple iPhone 15")
        );
        assert_eq!(camera_name(Some(" "), None), None);
    }

    #[test]
    fn movie_header_date() {
        // 2024-03-09 14:05:59 UTC.
        let d = mvhd_date(std::io::Cursor::new(mp4(1_709_993_159, 100))).unwrap();
        let want = local(chrono::DateTime::from_timestamp(1_709_993_159, 0).unwrap()).unwrap();
        assert_eq!(d, want);
        assert_eq!(mvhd_date(std::io::Cursor::new(mp4(0, 10))), None);
        assert_eq!(
            mvhd_date(std::io::Cursor::new(b"not a movie".to_vec())),
            None
        );
    }
}

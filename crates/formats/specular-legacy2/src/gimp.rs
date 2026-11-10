//! GIMP brushes (`.gbr`), animated brushes (`.gih`) and patterns (`.pat`).
//!
//! Big-endian headers. A brush: the header length, the version (1 or 2), width, height,
//! bytes per pixel (1 grey, 4 RGBA), then in version 2 the magic `GIMP` at byte 20 and the
//! spacing (percent of the brush size); the rest of the header is the name (UTF-8, NUL
//! ended), then the pixels. A pattern: the header length, version 1, width, height, bytes
//! per pixel (1 grey, 2 grey and alpha, 3 RGB, 4 RGBA), the magic `GPAT` and the name.
//! An animated brush (a brush pipe): a name line, a line with the cell count and the
//! placement parameters, then that many brushes one after another; each is a page.
//!
//! A grey brush stores paint coverage (255 = full paint). It is shown as GIMP's brush
//! dialog shows it, dark paint on white (`255 − coverage`).

use crate::swatch::short;
use crate::{Error, Image, Info, Model, Pixels, Result, check_dims};

/// Largest brush or pattern side (GIMP's own limit).
const MAX_SIDE: u32 = 10_000;
/// Most cells read from a brush pipe.
const MAX_CELLS: usize = 4096;

fn u32be(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// A brush or pattern header.
#[derive(Clone, Debug, PartialEq)]
struct Header {
    version: u32,
    width: u32,
    height: u32,
    bytes: u32,
    spacing: Option<u32>,
    name: String,
    /// Where the pixels start, and where they end.
    data: usize,
    end: usize,
}

fn name_of(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    short(&String::from_utf8_lossy(&b[..end]))
}

fn header(d: &[u8], at: usize, pattern: bool) -> Result<Header> {
    let g = |k: usize| u32be(d, at + k * 4).ok_or_else(|| Error::Truncated("header".into()));
    let (len, version, width, height, bytes) = (g(0)?, g(1)?, g(2)?, g(3)?, g(4)?);
    let magic = d.get(at + 20..at + 24);
    let (base, spacing) = if pattern {
        if magic != Some(b"GPAT") {
            return Err(Error::NotRecognised);
        }
        (24, None)
    } else if version == 2 || version == 3 {
        if magic != Some(b"GIMP") {
            return Err(Error::NotRecognised);
        }
        (28, Some(g(6)?))
    } else if version == 1 {
        (20, None)
    } else {
        return Err(Error::Unsupported(format!(
            "GIMP brush version {version} is not supported"
        )));
    };
    if (len as usize) < base || len > 1 << 20 {
        return Err(Error::Malformed(format!("header length {len}")));
    }
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(Error::TooLarge);
    }
    check_dims(width.into(), height.into())?;
    let ok = if pattern {
        (1..=4).contains(&bytes)
    } else {
        bytes == 1 || bytes == 4
    };
    if !ok {
        return Err(Error::Unsupported(format!(
            "{} with {bytes} bytes per pixel {}",
            if pattern { "a pattern" } else { "a brush" },
            if !pattern && bytes == 2 {
                "(16-bit brushes are not supported)"
            } else {
                "is not supported"
            }
        )));
    }
    let start = at + len as usize;
    let name = d
        .get(at + base..start.min(d.len()))
        .map(name_of)
        .unwrap_or_default();
    let end = start + width as usize * height as usize * bytes as usize;
    Ok(Header {
        version,
        width,
        height,
        bytes,
        spacing,
        name,
        data: start,
        end,
    })
}

fn pixels(d: &[u8], h: &Header, brush: bool) -> Image {
    let n = h.width as usize * h.height as usize;
    let have = d
        .get(h.data.min(d.len())..h.end.min(d.len()))
        .unwrap_or(&[]);
    let truncated = have.len() < n * h.bytes as usize;
    let pixels = match h.bytes {
        1 => {
            let mut v: Vec<u8> = if brush {
                have.iter().map(|&c| 255 - c).collect()
            } else {
                have.to_vec()
            };
            v.resize(n, crate::GREY);
            Pixels::Gray8(v)
        }
        _ => {
            let mut v = Vec::with_capacity(n * 4);
            for p in have.chunks_exact(h.bytes as usize) {
                v.extend_from_slice(&match h.bytes {
                    2 => [p[0], p[0], p[0], p[1]],
                    3 => [p[0], p[1], p[2], 255],
                    _ => [p[0], p[1], p[2], p[3]],
                });
            }
            v.resize(n * 4, crate::GREY);
            Pixels::Rgba8(v)
        }
    };
    Image {
        width: h.width,
        height: h.height,
        pixels,
        truncated,
    }
}

fn facts(h: &Header, kind: &str, pages: u32, extra: Vec<(String, String)>) -> Info {
    let mut fields = Vec::new();
    let mut summary = Vec::new();
    if !h.name.is_empty() {
        fields.push(("Name".into(), h.name.clone()));
        summary.push(("title", h.name.clone()));
    }
    fields.push(("Version".into(), h.version.to_string()));
    if let Some(s) = h.spacing {
        fields.push(("Spacing".into(), format!("{s} %")));
    }
    fields.extend(extra);
    let (model, alpha, coding) = match h.bytes {
        1 => (Model::Grey, false, format!("{kind}, greyscale")),
        2 => (Model::Grey, true, format!("{kind}, greyscale with alpha")),
        3 => (Model::Rgb, false, format!("{kind}, RGB")),
        _ => (Model::Rgb, true, format!("{kind}, RGBA")),
    };
    Info {
        width: h.width,
        height: h.height,
        pages,
        bit_depth: 8,
        model,
        has_alpha: alpha,
        orientation: 1,
        coding,
        summary,
        fields,
    }
}

/// GIMP brushes.
pub mod gbr {
    use super::*;

    /// Version 2's `GIMP` at byte 20, or a version 1 header that adds up.
    pub fn probe(head: &[u8]) -> bool {
        if head.len() < 28 {
            return false;
        }
        let v = u32be(head, 4);
        if head[20..24] == *b"GIMP" {
            return matches!(v, Some(2 | 3));
        }
        v == Some(1)
            && u32be(head, 0).is_some_and(|l| (21..1024).contains(&l))
            && u32be(head, 16) == Some(1)
            && u32be(head, 8).is_some_and(|w| (1..=MAX_SIDE).contains(&w))
            && u32be(head, 12).is_some_and(|h| (1..=MAX_SIDE).contains(&h))
    }

    /// Only version 1, which has no magic, is a structural match.
    pub fn is_weak(head: &[u8]) -> bool {
        head.get(20..24) != Some(b"GIMP")
    }

    pub fn info(data: &[u8], _page: u32) -> Result<Info> {
        if !probe(data) {
            return Err(Error::NotRecognised);
        }
        let h = header(data, 0, false)?;
        Ok(facts(&h, "GIMP brush", 1, Vec::new()))
    }

    pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
        if !probe(data) {
            return Err(Error::NotRecognised);
        }
        Ok(pixels(data, &header(data, 0, false)?, true))
    }
}

/// GIMP patterns.
pub mod pat {
    use super::*;

    pub fn probe(head: &[u8]) -> bool {
        head.len() >= 24 && head[20..24] == *b"GPAT"
    }

    pub fn info(data: &[u8], _page: u32) -> Result<Info> {
        if data.starts_with(b"8BPT") {
            return Err(Error::Unsupported(
                "Photoshop pattern libraries are not supported (GIMP patterns are)".into(),
            ));
        }
        let h = header(data, 0, true)?;
        Ok(facts(&h, "GIMP pattern", 1, Vec::new()))
    }

    pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
        Ok(pixels(data, &header(data, 0, true)?, false))
    }
}

/// GIMP animated brushes (brush pipes): every cell is a page.
pub mod gih {
    use super::*;

    struct Pipe {
        name: String,
        params: String,
        cells: Vec<Header>,
    }

    fn line(d: &[u8], at: usize) -> Option<(&[u8], usize)> {
        let rest = d.get(at..)?;
        let n = rest.iter().take(4096).position(|&c| c == b'\n')?;
        Some((&rest[..n], at + n + 1))
    }

    fn parse(d: &[u8]) -> Result<Pipe> {
        let (name, at) = line(d, 0).ok_or(Error::NotRecognised)?;
        let (params, mut at) = line(d, at).ok_or(Error::NotRecognised)?;
        let params = String::from_utf8_lossy(params).trim().to_owned();
        let count: usize = params
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or(Error::NotRecognised)?;
        if count == 0 {
            return Err(Error::Malformed("no cells".into()));
        }
        let mut cells = Vec::new();
        for _ in 0..count.min(MAX_CELLS) {
            if at >= d.len() {
                break;
            }
            let h = header(d, at, false)?;
            at = h.end;
            cells.push(h);
        }
        if cells.is_empty() {
            return Err(Error::Truncated("no brushes".into()));
        }
        Ok(Pipe {
            name: short(&String::from_utf8_lossy(name)),
            params,
            cells,
        })
    }

    /// A name line, a cell-count line and a brush header after them.
    pub fn probe(head: &[u8]) -> bool {
        let Some((_, at)) = line(head, 0) else {
            return false;
        };
        let Some((p, at)) = line(head, at) else {
            return false;
        };
        let count_ok = std::str::from_utf8(p).ok().is_some_and(|s| {
            s.split_whitespace()
                .next()
                .is_some_and(|n| n.parse::<u32>().is_ok_and(|n| n > 0))
        });
        count_ok && head.get(at..).is_some_and(super::gbr::probe)
    }

    pub fn info(data: &[u8], page: u32) -> Result<Info> {
        let p = parse(data)?;
        let h = p
            .cells
            .get(page as usize)
            .ok_or_else(|| Error::Malformed("no such cell".into()))?;
        let mut extra = vec![("Cells".into(), p.cells.len().to_string())];
        if p.params.split_whitespace().nth(1).is_some() {
            extra.push(("Parameters".into(), short(&p.params)));
        }
        let mut i = facts(h, "GIMP animated brush", p.cells.len() as u32, extra);
        if !p.name.is_empty() {
            i.fields.insert(0, ("Brush name".into(), p.name.clone()));
            i.summary = vec![("title", p.name)];
        }
        Ok(i)
    }

    pub fn decode(data: &[u8], page: u32) -> Result<Image> {
        let p = parse(data)?;
        let h = p
            .cells
            .get(page as usize)
            .ok_or_else(|| Error::Malformed("no such cell".into()))?;
        Ok(pixels(data, h, true))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn brush(w: u32, h: u32, bytes: u32, name: &str, fill: u8) -> Vec<u8> {
        let mut d = Vec::new();
        let len = 28 + name.len() as u32 + 1;
        for v in [len, 2, w, h, bytes] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        d.extend_from_slice(b"GIMP");
        d.extend_from_slice(&25u32.to_be_bytes());
        d.extend_from_slice(name.as_bytes());
        d.push(0);
        d.resize(d.len() + (w * h * bytes) as usize, fill);
        d
    }

    #[test]
    fn brushes_pipes_and_patterns() {
        let b = brush(3, 2, 1, "Dot", 200);
        assert!(gbr::probe(&b) && !gbr::is_weak(&b));
        let i = gbr::info(&b, 0).unwrap();
        assert_eq!((i.width, i.height), (3, 2));
        assert!(i.fields.contains(&("Spacing".into(), "25 %".into())));
        let img = gbr::decode(&b, 0).unwrap();
        assert_eq!(img.pixels, Pixels::Gray8(vec![55; 6]));
        // A version 1 brush.
        let mut v1 = Vec::new();
        for v in [24u32, 1, 2, 2, 1] {
            v1.extend_from_slice(&v.to_be_bytes());
        }
        v1.extend_from_slice(b"one\0");
        v1.extend_from_slice(&[0, 255, 0, 255, 9, 9, 9, 9]);
        assert!(gbr::probe(&v1) && gbr::is_weak(&v1));
        assert_eq!(
            gbr::decode(&v1, 0).unwrap().pixels,
            Pixels::Gray8(vec![255, 0, 255, 0])
        );
        // A pipe of two cells.
        let mut pipe = b"Pipe\n2 ncells:2 dim:1\n".to_vec();
        pipe.extend(brush(2, 2, 4, "a", 1));
        pipe.extend(brush(4, 1, 1, "b", 0));
        assert!(gih::probe(&pipe));
        let i = gih::info(&pipe, 1).unwrap();
        assert_eq!((i.pages, i.width, i.height), (2, 4, 1));
        assert_eq!(
            gih::decode(&pipe, 1).unwrap().pixels,
            Pixels::Gray8(vec![255; 4])
        );
        // A pattern.
        let mut p = Vec::new();
        for v in [26u32, 1, 1, 1, 3] {
            p.extend_from_slice(&v.to_be_bytes());
        }
        p.extend_from_slice(b"GPATx\0");
        p.extend_from_slice(&[1, 2, 3]);
        assert!(pat::probe(&p));
        assert_eq!(
            pat::decode(&p, 0).unwrap().pixels,
            Pixels::Rgba8(vec![1, 2, 3, 255])
        );
        assert!(matches!(
            pat::info(b"8BPT\x00\x01", 0),
            Err(Error::Unsupported(_))
        ));
    }
}

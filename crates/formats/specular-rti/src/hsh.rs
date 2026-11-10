//! Cultural Heritage Imaging's hemispherical-harmonics files.
//!
//! The `.rti` ("universal RTI") header is text: `#` comment lines, the RTI type (3 for HSH),
//! `width height colours` and `terms basis-type element-size`, each on its own line. Then,
//! little-endian, one `f32` scale per term, one `f32` bias per term, and for each pixel (rows
//! top-down) and colour channel one byte per term: `value = byte / 255 · scale + bias`.
//!
//! The older `.hsh` has the same comments followed by binary `i32` width, height, colours and
//! order (terms = order²), then per-term minimum and maximum: `value = byte / 255 · (max −
//! min) + min`.

use crate::{Basis, Colour, Container, Error, Result, Rti, check_size, layer};

/// Lines (and the binary start) after the `#` comments.
fn skip_comments(data: &[u8]) -> usize {
    let mut pos = 0;
    while pos < data.len() && data[pos] == b'#' {
        match data[pos..].iter().position(|&b| b == b'\n') {
            Some(n) => pos += n + 1,
            None => return data.len(),
        }
    }
    pos
}

/// One text line from `pos` (without the line end), and the position after it.
fn line(data: &[u8], pos: usize) -> Option<(&str, usize)> {
    let rest = data.get(pos..)?;
    let n = rest.iter().take(128).position(|&b| b == b'\n')?;
    let s = std::str::from_utf8(&rest[..n]).ok()?;
    Some((s.trim_end_matches('\r'), pos + n + 1))
}

fn ints(s: &str) -> Option<Vec<i64>> {
    s.split_ascii_whitespace().map(|t| t.parse().ok()).collect()
}

struct Text {
    rti_type: i64,
    width: u32,
    height: u32,
    colours: i64,
    terms: i64,
    elem: i64,
    start: usize,
}

fn text_header(data: &[u8]) -> Option<Text> {
    let pos = skip_comments(data);
    let (l1, pos) = line(data, pos)?;
    let t = ints(l1)?;
    let (l2, pos) = line(data, pos)?;
    let size = ints(l2)?;
    let (l3, pos) = line(data, pos)?;
    let basis = ints(l3)?;
    if t.len() != 1 || size.len() != 3 || basis.len() != 3 {
        return None;
    }
    let w = u32::try_from(size[0]).ok()?;
    let h = u32::try_from(size[1]).ok()?;
    Some(Text {
        rti_type: t[0],
        width: w,
        height: h,
        colours: size[2],
        terms: basis[0],
        elem: basis[2],
        start: pos,
    })
}

struct Legacy {
    width: u32,
    height: u32,
    terms: usize,
    start: usize,
}

fn legacy_header(data: &[u8]) -> Option<Legacy> {
    let pos = skip_comments(data);
    if pos == 0 {
        return None;
    }
    let int = |i: usize| -> Option<i32> {
        let b = data.get(pos + 4 * i..pos + 4 * i + 4)?;
        Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (w, h, bands, order) = (int(0)?, int(1)?, int(2)?, int(3)?);
    if !(1..=65536).contains(&w) || !(1..=65536).contains(&h) || bands != 3 {
        return None;
    }
    if !(1..=4).contains(&order) {
        return None;
    }
    Some(Legacy {
        width: w as u32,
        height: h as u32,
        terms: (order * order) as usize,
        start: pos + 16,
    })
}

/// True for the start of a `.rti` or `.hsh` file.
pub(crate) fn probe(head: &[u8]) -> bool {
    if head
        .first()
        .is_none_or(|&b| b != b'#' && !b.is_ascii_digit())
    {
        return false;
    }
    match text_header(head) {
        Some(t) => (1..=4).contains(&t.rti_type) && t.width > 0 && t.height > 0,
        None => legacy_header(head).is_some(),
    }
}

pub(crate) fn read(data: &[u8]) -> Result<Rti> {
    let (width, height, terms, start, legacy) = if let Some(t) = text_header(data) {
        match t.rti_type {
            3 => {}
            1 => return Err(Error::Unsupported("RTI type 1 (PTM)".into())),
            2 => return Err(Error::Unsupported("RTI type 2 (SH)".into())),
            n => return Err(Error::Unsupported(format!("RTI type {n}"))),
        }
        if t.colours != 3 {
            return Err(Error::Unsupported(format!(
                "RTI with {} colours",
                t.colours
            )));
        }
        if t.elem != 1 {
            return Err(Error::Unsupported(format!("RTI element size {}", t.elem)));
        }
        let terms = match t.terms {
            1 | 4 => 4,
            2 | 9 => 9,
            16 => 16,
            n => return Err(Error::Unsupported(format!("HSH with {n} terms"))),
        };
        (t.width, t.height, terms, t.start, false)
    } else if let Some(l) = legacy_header(data) {
        (l.width, l.height, l.terms, l.start, true)
    } else {
        return Err(Error::NotRti);
    };
    check_size(width, height)?;
    let floats = |at: usize| -> Result<Vec<f32>> {
        let b = data
            .get(at..at + terms * 4)
            .ok_or(Error::Truncated("HSH header"))?;
        Ok(b.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    };
    let first = floats(start)?;
    let second = floats(start + terms * 4)?;
    // value = byte · s + o
    let (s, o): (Vec<f32>, Vec<f32>) = if legacy {
        (0..terms)
            .map(|k| ((second[k] - first[k]) / 255.0, first[k]))
            .unzip()
    } else {
        (0..terms).map(|k| (first[k] / 255.0, second[k])).unzip()
    };
    let n = width as usize * height as usize;
    let _layers = crate::reserve_layers(terms, width, height)?;
    let at = start + terms * 8;
    let body = data
        .get(at..at + n * 3 * terms)
        .ok_or(Error::Truncated("HSH coefficients"))?;
    let mut layers: Vec<_> = (0..terms)
        .map(|k| layer(width, height, [s[k]; 3], [o[k]; 3]))
        .collect();
    for (i, px) in body.chunks_exact(3 * terms).enumerate() {
        for c in 0..3 {
            for (k, l) in layers.iter_mut().enumerate() {
                l.data[i * 3 + c] = px[c * terms + k];
            }
        }
    }
    Ok(Rti {
        width,
        height,
        container: Container::Hsh {
            legacy,
            scale: first,
            bias: second,
        },
        basis: Basis::Hsh { terms },
        colour: Colour::Rgb,
        planes: 3 * terms,
        layers,
    })
}

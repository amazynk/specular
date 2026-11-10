//! FBI WSQ grey-scale fingerprint images (Wavelet / Scalar Quantization, specification 3.1).
//!
//! A WSQ file is a JPEG-like marker stream: the frame header (size, shift and scale), a
//! transform table (the two wavelet filters), a quantisation table (bin widths per subband), up
//! to eight Huffman tables, comments (the NISTCOM block of key/value lines) and three coded
//! blocks that together hold the quantised coefficients of 60 of the 64 subbands, in subband
//! order. Decoding undoes the Huffman and run-length coding, the scalar quantisation, then the
//! wavelet decomposition (20 regions, see [`tree`]), and scales to 8-bit grey.
//!
//! The arithmetic follows the reference implementation's single-precision order of operations,
//! so pictures come out identical to NBIS `dwsq`.
//!
//! Robustness: no input panics; sizes are bounded by the 16-bit header fields, by
//! [`MAX_PIXELS`] and by what the coded data could plausibly describe; a stream that ends early
//! gives the image from the coefficients read so far with [`Image::truncated`] set.

mod huffman;
mod parse;
mod transform;
mod tree;

use huffman::{Bits, Stop};
use parse::{Cursor, EOI, Header, SOB, SOI};

/// Largest pixel count accepted.
pub const MAX_PIXELS: u64 = 1 << 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a WSQ file.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement (plain-language reason).
    Unsupported(String),
    /// The header or tables are cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over [`MAX_PIXELS`].
    TooLarge,
    /// The memory budget cannot hold the decode.
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRecognised => write!(f, "not a WSQ file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Grey,
    Rgb,
    Palette,
    YCbCr,
    Bilevel,
}

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub pages: u32,
    pub bit_depth: u8,
    pub model: Model,
    pub has_alpha: bool,
    /// EXIF orientation code (WSQ has none: always 1).
    pub orientation: u8,
    pub coding: String,
    pub summary: Vec<(&'static str, String)>,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    Gray8(Vec<u8>),
    Rgba8(Vec<u8>),
    Gray16(Vec<u16>),
    Rgba16(Vec<u16>),
    RgbaF32(Vec<f32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Always [`Pixels::Gray8`].
    pub pixels: Pixels,
    /// The coded data ended early or was damaged; what follows is flat grey.
    pub truncated: bool,
}

/// Whether `head` starts a WSQ stream: the start-of-image marker followed by a table, comment
/// or frame header marker.
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 4
        && head[0..2] == [0xFF, 0xA0]
        && head[2] == 0xFF
        && (0xA2..=0xA8).contains(&head[3])
        && head[3] != 0xA3
}

fn check_size(h: &Header, data_len: usize) -> Result<(usize, usize)> {
    let (w, ht) = (h.frame.width as usize, h.frame.height as usize);
    if w == 0 || ht == 0 {
        return Err(Error::Malformed(format!("empty image {w}×{ht}")));
    }
    let px = w as u64 * ht as u64;
    if px > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    // Even a blank page needs a few bits per thousand pixels.
    if px
        > (data_len as u64)
            .saturating_mul(4096)
            .saturating_add(1 << 20)
    {
        return Err(Error::Truncated(format!(
            "{data_len} bytes cannot describe {w}×{ht} pixels"
        )));
    }
    Ok((w, ht))
}

/// The NISTCOM block (`NIST_COM n` then `KEY value` lines) as key/value pairs.
fn nistcom(text: &[u8]) -> Option<Vec<(String, String)>> {
    let t = String::from_utf8_lossy(text);
    if !t.starts_with("NIST_COM") {
        return None;
    }
    Some(
        t.lines()
            .filter_map(|l| {
                let l = l.trim_end_matches(['\r', '\0']);
                let (k, v) = l.split_once([' ', '\t']).unwrap_or((l, ""));
                let (k, v) = (k.trim(), v.trim());
                (!k.is_empty() && k != "NIST_COM").then(|| (k.to_owned(), v.to_owned()))
            })
            .collect(),
    )
}

/// Readable text of a comment, if it is text.
fn comment_text(c: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(c).ok()?;
    let s = s.trim_matches(|ch: char| ch.is_whitespace() || ch == '\0');
    (!s.is_empty()
        && !s
            .chars()
            .any(|ch| ch.is_control() && ch != '\n' && ch != '\t'))
    .then(|| s.to_owned())
}

fn float_text(v: f32) -> String {
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Header facts. `page` must be 0.
pub fn info(data: &[u8], page: u32) -> Result<Info> {
    if page != 0 {
        return Err(Error::Malformed(format!("no page {page}")));
    }
    let h = parse::header(data)?;
    let (w, ht) = check_size(&h, data.len())?;
    let f = &h.frame;
    let mut fields = vec![
        ("Width".to_owned(), w.to_string()),
        ("Height".to_owned(), ht.to_string()),
        ("Black".to_owned(), f.black.to_string()),
        ("White".to_owned(), f.white.to_string()),
        ("Shift".to_owned(), float_text(f.m_shift)),
        ("Scale".to_owned(), float_text(f.r_scale)),
        ("Encoder".to_owned(), f.encoder.to_string()),
        ("Software".to_owned(), format!("{:#06x}", f.software)),
    ];
    if let Some(flt) = &h.filters {
        fields.push((
            "Wavelet filters".to_owned(),
            format!("{}/{} taps", flt.hi.len(), flt.lo.len()),
        ));
    }
    if let Some(q) = &h.quant {
        fields.push(("Bin centre".to_owned(), float_text(q.bin_center)));
        let coded = q.q_bin.iter().take(60).filter(|&&b| b != 0.0).count();
        fields.push(("Coded subbands".to_owned(), coded.to_string()));
    }
    let tables = h.huffman.iter().filter(|t| t.is_some()).count();
    fields.push(("Huffman tables".to_owned(), tables.to_string()));
    let mut summary = Vec::new();
    let mut ppi = None;
    let mut bitrate = None;
    let mut other_comment = None;
    for c in &h.comments {
        match nistcom(c) {
            Some(kv) => {
                for (k, v) in kv {
                    match k.as_str() {
                        "PPI" => ppi = v.parse::<i64>().ok().filter(|&p| p > 0),
                        "WSQ_BITRATE" => bitrate = Some(v.clone()),
                        _ => {}
                    }
                    if !v.is_empty() {
                        fields.push((k, v));
                    }
                }
            }
            None => {
                if let Some(t) = comment_text(c) {
                    fields.push(("Comment".to_owned(), t.clone()));
                    other_comment.get_or_insert(t);
                }
            }
        }
    }
    if let Some(p) = ppi {
        summary.push(("resolution", format!("{p} ppi")));
    }
    let bpp = data.len() as f64 * 8.0 / (w as f64 * ht as f64);
    summary.push((
        "compression",
        match &bitrate {
            Some(b) => format!("WSQ, target {b} bits per pixel"),
            None => "WSQ".to_owned(),
        },
    ));
    summary.push((
        "creator",
        format!(
            "WSQ encoder {}, implementation {:#06x}",
            f.encoder, f.software
        ),
    ));
    if let Some(c) = other_comment {
        summary.push(("comment", c));
    }
    let taps = h
        .filters
        .as_ref()
        .map(|flt| format!(", {}/{}-tap wavelet", flt.hi.len(), flt.lo.len()))
        .unwrap_or_default();
    Ok(Info {
        width: w as u32,
        height: ht as u32,
        pages: 1,
        bit_depth: 8,
        model: Model::Grey,
        has_alpha: false,
        orientation: 1,
        coding: format!("WSQ, 8-bit grey{taps}, {bpp:.2} bits per pixel"),
        summary,
        fields,
    })
}

/// The quantised coefficients of the coded subbands, in order, and whether they ran short.
fn entropy_decode<'a>(
    data: &'a [u8],
    mut h: Header<'a>,
    total: usize,
) -> Result<(Vec<i16>, bool, Header<'a>)> {
    let mut q = vec![0i16; total];
    let mut n = 0usize;
    let mut c = Cursor::new(data, h.first_block);
    // Find the first SOB (tables may be redefined between blocks).
    let mut table_id: Option<usize> = None;
    let mut truncated = false;
    'blocks: loop {
        // At a marker: tables until SOB, or the end.
        let m = match c.marker() {
            Ok(m) => m,
            Err(Error::Truncated(_)) => {
                truncated = n < total;
                break;
            }
            Err(e) => return Err(e),
        };
        match m {
            EOI => break,
            SOB => {
                let seg = parse::segment(&mut c, "block header")?;
                let id = seg.first().copied().unwrap_or(0) as usize;
                if id >= 8 || h.huffman[id].is_none() {
                    return Err(Error::Malformed(format!(
                        "a block uses Huffman table {id}, which is not defined"
                    )));
                }
                table_id = Some(id);
            }
            0xFFB0..=0xFFB7 if table_id.is_some() => {}
            SOI => return Err(Error::Malformed("a second start-of-image marker".into())),
            m => {
                parse::table(
                    &mut c,
                    m,
                    &mut h.filters,
                    &mut h.quant,
                    &mut h.huffman,
                    &mut h.comments,
                )?;
                continue;
            }
        }
        let Some(t) = table_id.and_then(|i| h.huffman[i].as_ref()) else {
            continue;
        };
        let mut bits = Bits::new(data, c.pos);
        let stop = loop {
            let v = match bits.decode(t) {
                Ok(v) => v,
                Err(s) => break s,
            };
            match v {
                1..=100 => n += v as usize,
                101..=104 => {
                    let k = if v <= 102 { 8 } else { 16 };
                    let mag = match bits.raw(k) {
                        Ok(x) => x as u16,
                        Err(Stop::End) => break Stop::End,
                        Err(_) => {
                            return Err(Error::Malformed(
                                "a marker inside a coefficient's extra bits".into(),
                            ));
                        }
                    };
                    let val = if v % 2 == 1 {
                        mag as i16
                    } else {
                        mag.wrapping_neg() as i16
                    };
                    if let Some(s) = q.get_mut(n) {
                        *s = val;
                    }
                    n += 1;
                }
                105 | 106 => {
                    let k = if v == 105 { 8 } else { 16 };
                    match bits.raw(k) {
                        Ok(x) => n += x as usize,
                        Err(Stop::End) => break Stop::End,
                        Err(_) => {
                            return Err(Error::Malformed(
                                "a marker inside a zero run's extra bits".into(),
                            ));
                        }
                    }
                }
                107..=254 => {
                    if let Some(s) = q.get_mut(n) {
                        *s = i16::from(v) - 180;
                    }
                    n += 1;
                }
                _ => break Stop::Bad,
            }
        };
        match stop {
            Stop::Marker(p) => c.pos = p,
            Stop::End => {
                truncated = n < total;
                break 'blocks;
            }
            Stop::Bad => {
                truncated = true;
                break 'blocks;
            }
        }
    }
    Ok((q, truncated, h))
}

/// Decodes the image. `page` must be 0.
pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    if page != 0 {
        return Err(Error::Malformed(format!("no page {page}")));
    }
    let h = parse::header(data)?;
    let (w, ht) = check_size(&h, data.len())?;
    // Quantised i16 samples, two f32 planes for the inverse transform and the 8-bit output.
    let _res = specular_core::budget::reserve_n((w * ht) as u64, 2 + 4 + 4 + 1)?;
    let wt = tree::wavelet_tree(w, ht);
    let sub = tree::subbands(&wt);
    // The quantisation table may only come between blocks; the count of coded samples is
    // fixed by the first one seen, as in the reference.
    let probe_q = h.quant.clone();
    let total_of = |q: &parse::Quant| -> usize {
        sub.iter()
            .take(60)
            .zip(q.q_bin.iter())
            .filter(|(_, b)| **b != 0.0)
            .map(|(r, _)| r.w * r.h)
            .sum()
    };
    let total = probe_q.as_ref().map_or(w * ht, total_of);
    let (coef, mut truncated, h) = entropy_decode(data, h, total)?;
    let Some(quant) = h.quant else {
        return Err(Error::Malformed("no quantisation table".into()));
    };
    let Some(filters) = h.filters else {
        return Err(Error::Malformed("no transform table".into()));
    };
    if probe_q.is_none() && total_of(&quant) != total {
        truncated = true;
    }
    // Unquantise into the subband layout.
    let mut img = vec![0f32; w * ht];
    let c = quant.bin_center;
    let mut it = coef.iter();
    for (s, r) in sub.iter().take(60).enumerate() {
        let qb = quant.q_bin[s];
        if qb == 0.0 {
            continue;
        }
        let half_z = f64::from(quant.z_bin[s]) / 2.0;
        for y in r.y..r.y + r.h {
            for o in &mut img[y * w + r.x..][..r.w] {
                let v = *it.next().unwrap_or(&0);
                *o = match v.cmp(&0) {
                    std::cmp::Ordering::Equal => 0.0,
                    std::cmp::Ordering::Greater => {
                        (f64::from(qb * (f32::from(v) - c)) + half_z) as f32
                    }
                    std::cmp::Ordering::Less => {
                        (f64::from(qb * (f32::from(v) + c)) - half_z) as f32
                    }
                };
            }
        }
    }
    drop(coef);
    transform::reconstruct(&mut img, w, &wt, &filters);
    let (m, r) = (h.frame.m_shift, h.frame.r_scale);
    let mut out = vec![0u8; w * ht];
    use rayon::prelude::*;
    out.par_chunks_mut(w)
        .zip(img.par_chunks(w))
        .for_each(|(o, s)| {
            for (o, &v) in o.iter_mut().zip(s) {
                let t = (f64::from(v * r + m) + 0.5) as f32;
                *o = if t < 0.0 {
                    0
                } else if t > 255.0 {
                    255
                } else {
                    t as u8
                };
            }
        });
    Ok(Image {
        width: w as u32,
        height: ht as u32,
        pixels: Pixels::Gray8(out),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_needs_soi_and_a_table() {
        assert!(probe(&[0xFF, 0xA0, 0xFF, 0xA8, 0, 2]));
        assert!(probe(&[0xFF, 0xA0, 0xFF, 0xA4]));
        assert!(!probe(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!probe(&[0xFF, 0xA0, 0xFF, 0xA3]));
        assert!(!probe(&[0xFF, 0xA0]));
    }

    #[test]
    fn nistcom_lines() {
        let kv = nistcom(b"NIST_COM 3\nPIX_WIDTH 500\nPPI 500\nLOSSY 1\n").unwrap();
        assert_eq!(kv[1], ("PPI".to_owned(), "500".to_owned()));
        assert!(nistcom(b"hello").is_none());
    }
}

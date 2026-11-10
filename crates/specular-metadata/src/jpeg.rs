//! Locates metadata segments and frame facts in a JPEG stream without decoding it.
//!
//! This is a convenience for callers (and the tests) that have the whole file in
//! memory; the core API ([`crate::read`]) takes the payloads directly and is
//! format-neutral.

use std::borrow::Cow;

use crate::{FileFacts, MetadataInput};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JpegError {
    #[error("not a JPEG file (no start-of-image marker)")]
    NotJpeg,
    #[error("JPEG segment structure is broken at byte {0}")]
    Truncated(usize),
}

/// Metadata payloads and frame facts found in a JPEG.
#[derive(Debug, Clone, Default)]
pub struct JpegSegments<'a> {
    /// TIFF payload of the EXIF APP1 segment (after "Exif\0\0").
    pub exif: Option<&'a [u8]>,
    /// Standard XMP packet.
    pub xmp: Option<&'a [u8]>,
    /// ICC profile, reassembled from its APP2 chunks.
    pub icc: Option<Cow<'a, [u8]>>,
    /// An APP11 JUMBF box labelled "c2pa" was found.
    pub has_c2pa: bool,
    /// The IPTC-IIM block of the APP13 Photoshop resources.
    pub iptc: Option<&'a [u8]>,
    /// The comment (COM) segments' text.
    pub comment: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bit_depth: Option<u8>,
    pub components: Option<u8>,
    pub progressive: Option<bool>,
    pub lossless: bool,
    /// "YCbCr", "RGB", "Greyscale", "CMYK", "YCCK".
    pub colour_model: Option<&'static str>,
    /// "4:2:0", "4:2:2", "4:4:4", "4:4:0", "4:1:1".
    pub chroma_subsampling: Option<&'static str>,
}

impl<'a> JpegSegments<'a> {
    /// Copies the frame facts into `file` (fields already set are kept).
    pub fn fill_file_facts(&self, file: &mut FileFacts) {
        file.format_name.get_or_insert_with(|| "JPEG".to_owned());
        if file.width.is_none() {
            file.width = self.width;
        }
        if file.height.is_none() {
            file.height = self.height;
        }
        if file.bit_depth.is_none() {
            file.bit_depth = self.bit_depth;
        }
        if file.colour_model.is_none() {
            file.colour_model = self.colour_model.map(str::to_owned);
        }
        if file.progressive.is_none() {
            file.progressive = self.progressive;
        }
        if file.chroma_subsampling.is_none() {
            file.chroma_subsampling = self.chroma_subsampling.map(str::to_owned);
        }
    }

    /// Builds a [`MetadataInput`] borrowing from these segments.
    pub fn input(&self, mut file: FileFacts) -> MetadataInput<'_> {
        self.fill_file_facts(&mut file);
        MetadataInput {
            exif_tiff: self.exif,
            xmp: self.xmp,
            icc: self.icc.as_deref(),
            has_c2pa: self.has_c2pa,
            file,
            medical: None,
            science: None,
            drawing: None,
            spectrum: None,
            mesh: None,
            points: None,
            splats: None,
            rti: None,
            structure: None,
            font: None,
            book: None,
            document: None,
            neuro: None,
            volume: None,
            geo: None,
            header: None,
            iptc: self.iptc,
            tags: self
                .comment
                .iter()
                .map(|c| crate::TagFacts {
                    kind: "JPEG".to_owned(),
                    props: vec![("exif:UserComment".to_owned(), c.clone())],
                    fields: vec![("Comment".to_owned(), c.clone())],
                })
                .collect(),
        }
    }
}

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const ICC_SIG: &[u8] = b"ICC_PROFILE\0";
const PHOTOSHOP_SIG: &[u8] = b"Photoshop 3.0\0";

/// A COM segment's text: UTF-8 when valid, else Latin-1; NULs and blank ends dropped.
fn comment_text(seg: &[u8]) -> String {
    let s = match std::str::from_utf8(seg) {
        Ok(s) => s.to_owned(),
        Err(_) => seg.iter().map(|&c| char::from(c)).collect(),
    };
    s.replace('\0', "").trim().to_owned()
}

/// Scans markers up to the first start-of-scan.
pub fn scan(data: &[u8]) -> Result<JpegSegments<'_>, JpegError> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(JpegError::NotJpeg);
    }
    let mut out = JpegSegments::default();
    let mut icc_chunks: Vec<(u8, &[u8])> = Vec::new();
    let mut adobe_transform: Option<u8> = None;
    let mut ids: Vec<u8> = Vec::new();
    let mut i = 2;
    loop {
        // Skip fill bytes.
        while i < data.len() && data[i] == 0xFF && data.get(i + 1) == Some(&0xFF) {
            i += 1;
        }
        if i + 4 > data.len() || data[i] != 0xFF {
            return if out.width.is_some() {
                Ok(finish(out, icc_chunks, adobe_transform, &ids))
            } else {
                Err(JpegError::Truncated(i))
            };
        }
        let marker = data[i + 1];
        if matches!(marker, 0xD0..=0xD7 | 0x01) {
            i += 2;
            continue;
        }
        if marker == 0xD9 {
            break;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if len < 2 || i + 2 + len > data.len() {
            return Err(JpegError::Truncated(i));
        }
        let seg = &data[i + 4..i + 2 + len];
        match marker {
            0xE1 if seg.starts_with(b"Exif\0\0") => {
                if out.exif.is_none() {
                    out.exif = Some(&seg[6..]);
                }
            }
            0xE1 if seg.starts_with(XMP_SIG) => {
                if out.xmp.is_none() {
                    out.xmp = Some(&seg[XMP_SIG.len()..]);
                }
            }
            0xE2 if seg.starts_with(ICC_SIG) && seg.len() > ICC_SIG.len() + 2 => {
                icc_chunks.push((seg[ICC_SIG.len()], &seg[ICC_SIG.len() + 2..]));
            }
            // JUMBF (ISO 19566-5) in APP11: "JP" + box instance + sequence + box.
            0xEB if seg.starts_with(b"JP") && contains(seg, b"c2pa") => out.has_c2pa = true,
            0xED if out.iptc.is_none() && seg.starts_with(PHOTOSHOP_SIG) => {
                out.iptc = crate::iptc::from_resources(&seg[PHOTOSHOP_SIG.len()..]);
            }
            0xFE => {
                let t = comment_text(seg);
                if !t.is_empty() {
                    match &mut out.comment {
                        Some(c) => {
                            c.push('\n');
                            c.push_str(&t);
                        }
                        None => out.comment = Some(t),
                    }
                }
            }
            0xEE if seg.starts_with(b"Adobe") && seg.len() >= 12 => adobe_transform = Some(seg[11]),
            0xC0..=0xCF
                if !matches!(marker, 0xC4 | 0xC8 | 0xCC)
                    && out.width.is_none()
                    && seg.len() >= 6 =>
            {
                out.bit_depth = Some(seg[0]);
                out.height = Some(u16::from_be_bytes([seg[1], seg[2]]) as u32);
                out.width = Some(u16::from_be_bytes([seg[3], seg[4]]) as u32);
                let n = seg[5];
                out.components = Some(n);
                out.progressive = Some(matches!(marker, 0xC2 | 0xC6 | 0xCA | 0xCE));
                out.lossless = matches!(marker, 0xC3 | 0xC7 | 0xCB | 0xCF);
                let mut factors = Vec::new();
                for c in 0..n as usize {
                    if let Some(comp) = seg.get(6 + c * 3..9 + c * 3) {
                        ids.push(comp[0]);
                        factors.push((comp[1] >> 4, comp[1] & 0xF));
                    }
                }
                if factors.len() >= 3 {
                    out.chroma_subsampling = subsampling(factors[0], factors[1]);
                }
            }
            0xDA => break,
            _ => {}
        }
        i += 2 + len;
    }
    Ok(finish(out, icc_chunks, adobe_transform, &ids))
}

fn finish<'a>(
    mut out: JpegSegments<'a>,
    mut chunks: Vec<(u8, &'a [u8])>,
    adobe: Option<u8>,
    ids: &[u8],
) -> JpegSegments<'a> {
    out.icc = match chunks.len() {
        0 => None,
        1 => Some(Cow::Borrowed(chunks[0].1)),
        _ => {
            chunks.sort_by_key(|c| c.0);
            Some(Cow::Owned(
                chunks.iter().flat_map(|c| c.1.iter().copied()).collect(),
            ))
        }
    };
    out.colour_model = match out.components {
        Some(1) => Some("Greyscale"),
        Some(3) => Some(match adobe {
            Some(0) => "RGB",
            Some(_) => "YCbCr",
            None if ids == b"RGB" => "RGB",
            None => "YCbCr",
        }),
        Some(4) => Some(if adobe == Some(2) { "YCCK" } else { "CMYK" }),
        _ => None,
    };
    if out.colour_model != Some("YCbCr") {
        out.chroma_subsampling = None;
    }
    out
}

fn subsampling(luma: (u8, u8), chroma: (u8, u8)) -> Option<&'static str> {
    if chroma.0 == 0 || chroma.1 == 0 {
        return None;
    }
    Some(match (luma.0 / chroma.0, luma.1 / chroma.1) {
        (1, 1) => "4:4:4",
        (2, 1) => "4:2:2",
        (2, 2) => "4:2:0",
        (1, 2) => "4:4:0",
        (4, 1) => "4:1:1",
        (4, 2) => "4:1:0",
        _ => return None,
    })
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

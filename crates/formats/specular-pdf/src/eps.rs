//! Encapsulated PostScript previews. PostScript is never executed; an EPS file is shown through
//! the preview it carries:
//!
//! - the DOS EPS binary header (`C5 D0 D3 C6`), whose table points at the PostScript section and
//!   an optional TIFF or Windows Metafile preview;
//! - an EPSI preview: a `%%BeginPreview: w h depth lines` comment block of hex rows, where 0 is
//!   white and the largest value is black.

use std::ops::Range;

/// Magic of the DOS EPS binary header.
pub const DOS_MAGIC: [u8; 4] = [0xC5, 0xD0, 0xD3, 0xC6];
/// Largest EPSI preview accepted, in pixels.
const MAX_PREVIEW_PIXELS: u64 = 64 * 1024 * 1024;
/// How far into the PostScript to look for header comments.
const COMMENT_SCAN: usize = 1024 * 1024;

/// Sections of a DOS EPS file, validated to lie inside the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DosEps {
    pub postscript: Range<usize>,
    pub wmf: Option<Range<usize>>,
    pub tiff: Option<Range<usize>>,
}

/// Parses a DOS EPS binary header. Sections that fall outside the file are dropped (a missing
/// PostScript section leaves an empty range).
pub fn dos_header(data: &[u8]) -> Option<DosEps> {
    if data.len() < 30 || data[..4] != DOS_MAGIC {
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let section = |o: usize| -> Option<Range<usize>> {
        let start = usize::try_from(u32_at(o)).ok()?;
        let len = usize::try_from(u32_at(o + 4)).ok()?;
        let end = start.checked_add(len)?;
        (start >= 30 && len > 0 && end <= data.len()).then_some(start..end)
    };
    Some(DosEps {
        postscript: section(4).unwrap_or(0..0),
        wmf: section(12),
        tiff: section(20),
    })
}

/// Whether `head` is an EPS file: a DOS EPS header, or a PostScript header naming EPSF.
pub fn looks_like_eps(head: &[u8]) -> bool {
    if head.starts_with(&DOS_MAGIC) {
        return true;
    }
    let line = first_line(head);
    line.starts_with(b"%!PS-Adobe-") && line.windows(4).any(|w| w == b"EPSF")
}

/// Whether `head` is PostScript at all (`%!PS`), e.g. a pre-PDF Illustrator file.
pub fn looks_like_postscript(head: &[u8]) -> bool {
    head.starts_with(b"%!PS") || head.starts_with(&DOS_MAGIC)
}

fn first_line(b: &[u8]) -> &[u8] {
    let b = &b[..b.len().min(256)];
    let end = b
        .iter()
        .position(|&c| c == b'\n' || c == b'\r')
        .unwrap_or(b.len());
    &b[..end]
}

/// The PostScript part of an EPS file: the DOS section, or the whole file.
pub fn postscript(data: &[u8]) -> &[u8] {
    match dos_header(data) {
        Some(d) => &data[d.postscript],
        None => data,
    }
}

/// The `%%BoundingBox:` (or `%%HiResBoundingBox:`, preferred) in points, as
/// `[llx, lly, urx, ury]`; `None` when absent, `(atend)` or degenerate.
pub fn bounding_box(ps: &[u8]) -> Option<[f64; 4]> {
    let ps = &ps[..ps.len().min(COMMENT_SCAN)];
    let find = |key: &[u8]| -> Option<[f64; 4]> {
        let at = find(ps, key)? + key.len();
        let line = first_line(&ps[at..]);
        let text = std::str::from_utf8(line).ok()?;
        let mut v = text.split_ascii_whitespace().map(|t| t.parse::<f64>().ok());
        let b = [v.next()??, v.next()??, v.next()??, v.next()??];
        (b.iter().all(|x| x.is_finite()) && b[2] > b[0] && b[3] > b[1]).then_some(b)
    };
    find(b"%%HiResBoundingBox:").or_else(|| find(b"%%BoundingBox:"))
}

/// A decoded EPSI preview: 8-bit grey, 0 black, 255 white.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epsi {
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub gray: Vec<u8>,
}

/// Decodes the EPSI preview in `ps`, if there is a well-formed one. Rows missing at the end of a
/// truncated preview stay white.
pub fn epsi_preview(ps: &[u8]) -> Option<Epsi> {
    let scan = &ps[..ps.len().min(COMMENT_SCAN)];
    let key = b"%%BeginPreview:";
    let at = find(scan, key)? + key.len();
    let header = std::str::from_utf8(first_line(&ps[at..])).ok()?;
    let mut v = header
        .split_ascii_whitespace()
        .map(|t| t.parse::<u32>().ok());
    let (width, height, depth) = (v.next()??, v.next()??, v.next()??);
    if width == 0 || height == 0 || !matches!(depth, 1 | 2 | 4 | 8) {
        return None;
    }
    if u64::from(width) * u64::from(height) > MAX_PREVIEW_PIXELS {
        return None;
    }
    let depth = depth as u8;
    let row_bytes = (width as usize * usize::from(depth)).div_ceil(8);
    let total = row_bytes * height as usize;
    let mut packed = Vec::with_capacity(total);

    let mut rest = &ps[at..];
    // Skip the header line itself.
    rest = &rest[first_line(rest).len()..];
    'lines: while packed.len() < total && !rest.is_empty() {
        let line_start = rest
            .iter()
            .position(|&c| c != b'\n' && c != b'\r')
            .unwrap_or(rest.len());
        rest = &rest[line_start..];
        let line = first_line_unbounded(rest);
        rest = &rest[line.len()..];
        if line.starts_with(b"%%EndPreview") {
            break;
        }
        let Some(body) = line.strip_prefix(b"%") else {
            break;
        };
        let mut hi: Option<u8> = None;
        for &c in body {
            let nib = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                b' ' | b'\t' => continue,
                _ => break 'lines,
            };
            match hi.take() {
                None => hi = Some(nib),
                Some(h) => {
                    packed.push(h << 4 | nib);
                    if packed.len() == total {
                        break 'lines;
                    }
                }
            }
        }
    }

    let max = (1u16 << depth) - 1;
    let mut gray = vec![255u8; width as usize * height as usize];
    for (y, row) in packed.chunks(row_bytes).enumerate() {
        let out = &mut gray[y * width as usize..][..width as usize];
        for (x, px) in out.iter_mut().enumerate() {
            let bit = x * usize::from(depth);
            let Some(&byte) = row.get(bit / 8) else {
                break;
            };
            let shift = 8 - usize::from(depth) - bit % 8;
            let value = u16::from(byte >> shift) & max;
            *px = (255 - value * 255 / max) as u8;
        }
    }
    Some(Epsi {
        width,
        height,
        depth,
        gray,
    })
}

fn first_line_unbounded(b: &[u8]) -> &[u8] {
    let end = b
        .iter()
        .position(|&c| c == b'\n' || c == b'\r')
        .unwrap_or(b.len());
    &b[..end]
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

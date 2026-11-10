//! Chunk walking and header / metadata parsing.

use std::ops::Range;

use crate::error::{PngError, Result};
use crate::{
    AnimationInfo, BlendOp, Chromaticities, Cicp, ColorType, ContentLightLevel, DisposeOp,
    FrameInfo, MasteringDisplay, PngInfo, Transparency,
};

pub(crate) const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Largest decompressed iCCP profile accepted.
const MAX_ICC: usize = 64 << 20;

/// One chunk as found in the file. `data` may be shorter than declared if the file is
/// truncated (then `complete` is false).
#[derive(Clone, Debug)]
pub(crate) struct Chunk {
    pub kind: [u8; 4],
    /// Offset of the length field.
    pub start: usize,
    pub data: Range<usize>,
    pub complete: bool,
}

/// Iterate over the chunks of `data` (after the signature). Stops at IEND, at the end of the
/// data, or after a chunk that runs past the end (yielded with `complete == false`).
pub(crate) struct Chunks<'a> {
    data: &'a [u8],
    pos: usize,
    done: bool,
}

impl<'a> Chunks<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Chunks<'a> {
        Chunks {
            data,
            pos: SIGNATURE.len(),
            done: false,
        }
    }
}

impl Iterator for Chunks<'_> {
    type Item = Chunk;
    fn next(&mut self) -> Option<Chunk> {
        if self.done || self.pos + 8 > self.data.len() {
            return None;
        }
        let d = &self.data[self.pos..];
        let len = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as usize;
        let kind = [d[4], d[5], d[6], d[7]];
        let start = self.pos;
        let begin = start + 8;
        let end = begin.saturating_add(len);
        let complete = end <= self.data.len();
        let data = begin..end.min(self.data.len());
        if !complete || &kind == b"IEND" || len > i32::MAX as usize {
            self.done = true;
        }
        self.pos = end.saturating_add(4);
        Some(Chunk {
            kind,
            start,
            data,
            complete,
        })
    }
}

fn be32(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn be16(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// Parse all chunks up to IEND into a [`PngInfo`]. Only IHDR problems are errors; damaged
/// ancillary chunks are ignored.
pub(crate) fn parse(data: &[u8], verify_crc: bool) -> Result<PngInfo> {
    if data.len() < 8 || &data[..8] != SIGNATURE {
        return Err(PngError::NotPng);
    }
    let mut chunks = Chunks::new(data);
    let ihdr = chunks.next().ok_or(PngError::Truncated("no IHDR"))?;
    if &ihdr.kind != b"IHDR" {
        return Err(PngError::Malformed("first chunk is not IHDR"));
    }
    if ihdr.data.len() < 13 {
        return Err(PngError::Truncated("IHDR"));
    }
    if verify_crc {
        check_crc(data, &ihdr)?;
    }
    let h = &data[ihdr.data.clone()];
    let width = be32(h, 0);
    let height = be32(h, 4);
    let bit_depth = h[8];
    let color_type = ColorType::from_code(h[9]).ok_or(PngError::Malformed("bad colour type"))?;
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(PngError::Malformed("bad image size"));
    }
    if !color_type.allowed_depths().contains(&bit_depth) {
        return Err(PngError::Malformed("bad bit depth for colour type"));
    }
    if h[10] != 0 || h[11] != 0 {
        return Err(PngError::Unsupported(
            "unknown compression or filter method",
        ));
    }
    let interlaced = match h[12] {
        0 => false,
        1 => true,
        _ => return Err(PngError::Unsupported("unknown interlace method")),
    };

    let mut info = PngInfo {
        width,
        height,
        bit_depth,
        color_type,
        interlaced,
        palette: None,
        transparency: None,
        gamma: None,
        chromaticities: None,
        srgb_intent: None,
        icc_profile: None,
        icc_name: None,
        cicp: None,
        mastering_display: None,
        content_light_level: None,
        exif: None,
        idat: Vec::new(),
        idat_complete: false,
        animation: None,
        frames: Vec::new(),
        default_image_is_frame: false,
        idot: None,
        pixels_per_unit: None,
    };

    // APNG state: the frame being collected.
    let mut cur_frame: Option<FrameInfo> = None;
    let mut seen_idat = false;
    let mut idat_ended = false;
    let mut saw_iend = false;
    for c in chunks {
        if verify_crc && c.complete {
            check_crc(data, &c)?;
        }
        let d = &data[c.data.clone()];
        match &c.kind {
            b"IDAT" => {
                if idat_ended {
                    // IDAT chunks must be consecutive; ignore stray later ones.
                    continue;
                }
                seen_idat = true;
                if !d.is_empty() {
                    info.idat.push(c.data.clone());
                }
                if let Some(f) = cur_frame.as_mut()
                    && f.from_idat
                {
                    f.data.push(c.data.clone());
                }
                if !c.complete {
                    break;
                }
                continue;
            }
            b"IEND" => {
                saw_iend = true;
                break;
            }
            _ => {}
        }
        if seen_idat {
            idat_ended = true;
            info.idat_complete = true;
        }
        if !c.complete {
            break;
        }
        match &c.kind {
            b"PLTE" if !seen_idat && info.palette.is_none() => {
                let n = (d.len() / 3).min(256);
                if n > 0 {
                    info.palette = Some(
                        d.chunks_exact(3)
                            .take(n)
                            .map(|p| [p[0], p[1], p[2]])
                            .collect(),
                    );
                }
            }
            b"tRNS" if !seen_idat => {
                info.transparency = match color_type {
                    ColorType::Gray if d.len() >= 2 => Some(Transparency::Gray(be16(d, 0))),
                    ColorType::Rgb if d.len() >= 6 => {
                        Some(Transparency::Rgb(be16(d, 0), be16(d, 2), be16(d, 4)))
                    }
                    ColorType::Indexed if !d.is_empty() => {
                        Some(Transparency::Palette(d[..d.len().min(256)].to_vec()))
                    }
                    _ => None,
                };
            }
            b"gAMA" if d.len() >= 4 => info.gamma = Some(be32(d, 0)),
            b"cHRM" if d.len() >= 32 => {
                let v: Vec<u32> = (0..8).map(|i| be32(d, i * 4)).collect();
                info.chromaticities = Some(Chromaticities {
                    white: (v[0], v[1]),
                    red: (v[2], v[3]),
                    green: (v[4], v[5]),
                    blue: (v[6], v[7]),
                });
            }
            b"sRGB" if !d.is_empty() => info.srgb_intent = Some(d[0]),
            b"iCCP" if info.icc_profile.is_none() => {
                if let Some(nul) = d.iter().take(80).position(|&b| b == 0)
                    && d.len() > nul + 2
                    && d[nul + 1] == 0
                    && let Ok(p) = crate::inflate::zlib_to_vec(&d[nul + 2..], MAX_ICC)
                {
                    info.icc_name = Some(String::from_utf8_lossy(&d[..nul]).into_owned());
                    info.icc_profile = Some(p);
                }
            }
            b"cICP" if d.len() >= 4 => {
                info.cicp = Some(Cicp {
                    color_primaries: d[0],
                    transfer_function: d[1],
                    matrix_coefficients: d[2],
                    full_range: d[3] != 0,
                })
            }
            b"mDCv" if d.len() >= 24 => {
                let c16 = |i| be16(d, i);
                info.mastering_display = Some(MasteringDisplay {
                    primaries: [(c16(0), c16(2)), (c16(4), c16(6)), (c16(8), c16(10))],
                    white: (c16(12), c16(14)),
                    max_luminance: be32(d, 16),
                    min_luminance: be32(d, 20),
                })
            }
            b"cLLi" if d.len() >= 8 => {
                info.content_light_level = Some(ContentLightLevel {
                    max_cll: be32(d, 0),
                    max_fall: be32(d, 4),
                })
            }
            b"eXIf" if info.exif.is_none() => info.exif = Some(c.data.clone()),
            b"pHYs" if d.len() >= 9 && d[8] == 1 => {
                info.pixels_per_unit = Some((be32(d, 0), be32(d, 4)))
            }
            b"iDOT" if !seen_idat && d.len() >= 4 => info.idot = parse_idot(d, c.start, height),
            b"acTL" if !seen_idat && d.len() >= 8 => {
                info.animation = Some(AnimationInfo {
                    num_frames: be32(d, 0),
                    num_plays: be32(d, 4),
                })
            }
            b"fcTL" if d.len() >= 26 => {
                if let Some(f) = cur_frame.take() {
                    info.frames.push(f);
                }
                let dispose = match d[24] {
                    1 => DisposeOp::Background,
                    2 => DisposeOp::Previous,
                    _ => DisposeOp::None,
                };
                let blend = if d[25] == 1 {
                    BlendOp::Over
                } else {
                    BlendOp::Source
                };
                let f = FrameInfo {
                    width: be32(d, 4),
                    height: be32(d, 8),
                    x: be32(d, 12),
                    y: be32(d, 16),
                    delay_num: be16(d, 20),
                    delay_den: be16(d, 22),
                    dispose,
                    blend,
                    from_idat: !seen_idat,
                    data: Vec::new(),
                };
                if !seen_idat {
                    info.default_image_is_frame = true;
                }
                cur_frame = Some(f);
            }
            b"fdAT" if d.len() > 4 => {
                if let Some(f) = cur_frame.as_mut()
                    && !f.from_idat
                {
                    f.data.push(c.data.start + 4..c.data.end);
                }
            }
            _ => {}
        }
    }
    if saw_iend || idat_ended {
        info.idat_complete = true;
    }
    if let Some(f) = cur_frame.take() {
        info.frames.push(f);
    }
    if info.animation.is_none() {
        info.frames.clear();
        info.default_image_is_frame = false;
    } else {
        // Drop frames that are empty or do not fit the canvas.
        info.frames.retain(|f| {
            !f.data.is_empty()
                && f.width > 0
                && f.height > 0
                && u64::from(f.x) + u64::from(f.width) <= u64::from(width)
                && u64::from(f.y) + u64::from(f.height) <= u64::from(height)
        });
        if info.frames.is_empty() {
            info.animation = None;
            info.default_image_is_frame = false;
        }
    }
    if color_type == ColorType::Indexed && info.palette.is_none() {
        // Without PLTE every index is out of range; decode as black rather than fail.
        info.palette = Some(Vec::new());
    }
    if info.idat.is_empty() {
        return Err(PngError::Truncated("no image data"));
    }
    Ok(info)
}

/// Apple's iDOT chunk (written by macOS / iOS ImageIO, e.g. for screenshots): the image is split
/// into horizontal parts, and the deflate stream is fully flushed at each part boundary, which
/// falls on an IDAT chunk start. Each part can therefore be inflated independently (from a byte
/// boundary, with no references into earlier parts). Layout: part count, then per part the first
/// row, the row count, and the offset of the part's first IDAT chunk from the iDOT chunk start.
fn parse_idot(d: &[u8], chunk_start: usize, height: u32) -> Option<Vec<crate::IdotPart>> {
    let n = be32(d, 0) as usize;
    if !(2..=64).contains(&n) || d.len() < 4 + n * 12 {
        return None;
    }
    let mut parts = Vec::with_capacity(n);
    let mut next_row = 0u32;
    for i in 0..n {
        let o = 4 + i * 12;
        let first_row = be32(d, o);
        let rows = be32(d, o + 4);
        let offset = be32(d, o + 8) as usize;
        if first_row != next_row || rows == 0 {
            return None;
        }
        next_row = first_row.checked_add(rows)?;
        parts.push(crate::IdotPart {
            first_row,
            rows,
            file_offset: chunk_start.checked_add(offset)?,
        });
    }
    (next_row == height).then_some(parts)
}

fn check_crc(data: &[u8], c: &Chunk) -> Result<()> {
    let end = c.data.end;
    let Some(stored) = data.get(end..end + 4) else {
        return Ok(());
    };
    let stored = u32::from_be_bytes([stored[0], stored[1], stored[2], stored[3]]);
    if crate::crc::crc32(&data[c.start + 4..end]) != stored {
        return Err(PngError::Checksum("chunk CRC mismatch"));
    }
    Ok(())
}

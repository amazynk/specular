//! IFF `DEEP` (TVPaint, NewTek): chunky pixels described element by element.
//!
//! - `DGBL`: display width and height, compression (0 none, 1 ByteRun1, 5 TVDC; 2 and 3
//!   Huffman and 4 JPEG are refused), pixel aspect;
//! - `DPEL`: the elements of a pixel, in order: type (1 red, 2 green, 3 blue, 4 or 17 alpha;
//!   others are skipped) and bit depth (8 or 16, a whole number of bytes);
//! - `DLOC`: the body's width and height (and place on the display);
//! - `DBOD`: the pixels, rows of `width × pixel bytes`; ByteRun1 counts whole pixels (a
//!   control byte `n` under 128 copies `n + 1` pixels, above 128 repeats the next pixel
//!   `257 - n` times), and runs carry over from row to row;
//! - `TVDC`: 16 signed 16-bit deltas for TVDC compression, which codes each element of each
//!   row separately as nibbles: a non-zero table entry is added to the running value (which
//!   starts at 0) and gives one pixel; a zero entry takes the next nibble `n` and repeats the
//!   value `n + 1` times. Each element row ends on a byte boundary.

use crate::{
    Compression, Error, Form, GREY, Image, Info, Kind, Masking, Parsed, Result, be16, be32,
    form_bounds,
};

/// Where each element of a pixel goes.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    /// Per element: bytes, and the output channel (0–3) or `None` when skipped.
    elements: Vec<(usize, Option<usize>)>,
    /// TVDC deltas.
    table: [i16; 16],
}

impl Layout {
    fn bytes(&self) -> usize {
        self.elements.iter().map(|e| e.0).sum()
    }
}

pub(crate) fn parse(data: &[u8]) -> Result<Parsed<'_>> {
    let (_, end) = form_bounds(data)?;
    let (mut dgbl, mut dpel, mut dloc, mut dbod, mut tvdc) = (None, None, None, None, None);
    let mut pos = 12usize;
    let mut n = 0;
    while pos.saturating_add(8) <= end && n < crate::MAX_CHUNKS {
        n += 1;
        let id = &data[pos..pos + 4];
        let len = be32(data, pos + 4) as usize;
        let start = pos + 8;
        let stop = start.saturating_add(len);
        let payload = &data[start..stop.min(end)];
        match id {
            b"DGBL" if dgbl.is_none() => dgbl = Some(payload),
            b"DPEL" if dpel.is_none() => dpel = Some(payload),
            b"DLOC" if dloc.is_none() => dloc = Some(payload),
            b"DBOD" if dbod.is_none() => dbod = Some(payload),
            b"TVDC" if tvdc.is_none() => tvdc = Some(payload),
            _ => {}
        }
        pos = stop.saturating_add(len & 1);
    }
    let dgbl = dgbl.ok_or(Error::Malformed("no DGBL chunk"))?;
    if dgbl.len() < 8 {
        return Err(Error::Truncated("DGBL"));
    }
    let compression = match be16(dgbl, 4) {
        0 => Compression::None,
        1 => Compression::ByteRun1,
        5 => Compression::Tvdc,
        2 | 3 => return Err(Error::Unsupported("DEEP Huffman compression".into())),
        4 => return Err(Error::Unsupported("DEEP JPEG compression".into())),
        c => return Err(Error::Unsupported(format!("DEEP compression type {c}"))),
    };
    let (width, height) = match dloc {
        Some(l) if l.len() >= 4 => (u32::from(be16(l, 0)), u32::from(be16(l, 2))),
        _ => (u32::from(be16(dgbl, 0)), u32::from(be16(dgbl, 2))),
    };
    if width == 0 || height == 0 {
        return Err(Error::Malformed("zero width or height"));
    }
    if u64::from(width) * u64::from(height) > crate::MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let dpel = dpel.ok_or(Error::Malformed("no DPEL chunk"))?;
    if dpel.len() < 4 {
        return Err(Error::Truncated("DPEL"));
    }
    let count = (be32(dpel, 0) as usize).min((dpel.len() - 4) / 4);
    let mut elements = Vec::with_capacity(count);
    let (mut rgb, mut alpha) = (0, false);
    for i in 0..count {
        let (t, bits) = (be16(dpel, 4 + 4 * i), be16(dpel, 6 + 4 * i));
        if bits == 0 || bits % 8 != 0 || bits > 16 {
            return Err(Error::Unsupported(format!("DEEP {bits}-bit elements")));
        }
        let ch = match t {
            1..=3 => {
                rgb += 1;
                Some(usize::from(t - 1))
            }
            4 | 17 => {
                alpha = true;
                Some(3)
            }
            _ => None,
        };
        elements.push((usize::from(bits / 8), ch));
    }
    if rgb < 3 {
        return Err(Error::Unsupported(
            "DEEP without red, green and blue".into(),
        ));
    }
    let mut table = [0i16; 16];
    if let Some(t) = tvdc {
        for (i, v) in table.iter_mut().enumerate() {
            if let Some(b) = t.get(2 * i..2 * i + 2) {
                *v = i16::from_be_bytes([b[0], b[1]]);
            }
        }
    } else if compression == Compression::Tvdc {
        return Err(Error::Malformed("TVDC compression without its table"));
    }
    let layout = Layout { elements, table };
    if compression == Compression::Tvdc && layout.elements.iter().any(|e| e.0 != 1) {
        return Err(Error::Unsupported("TVDC with 16-bit elements".into()));
    }
    let (xa, ya) = (dgbl[6], dgbl[7]);
    Ok(Parsed {
        info: Info {
            form: Form::Deep,
            width,
            height,
            planes: (layout.bytes() * 8).min(255) as u8,
            kind: if alpha { Kind::Rgba32 } else { Kind::Rgb24 },
            masking: Masking::None,
            compressed: compression != Compression::None,
            compression,
            pixel_aspect: (xa != 0 && ya != 0 && xa != ya).then_some((xa, ya)),
            colour_cycling: false,
            has_alpha: alpha,
            palette_len: 0,
            anim_frames: 0,
            line_palette: None,
        },
        cmap: &[],
        body: dbod.ok_or(Error::Malformed("no DBOD chunk"))?,
        ranges: Vec::new(),
        deep: Some(layout),
        lines: None,
    })
}

pub(crate) fn decode(info: &Info, l: &Layout, body: &[u8]) -> Image {
    let (w, h) = (info.width as usize, info.height as usize);
    let bpp = l.bytes();
    let mut pixels = vec![0u8; w * h * 4];
    let mut truncated = false;
    if info.compression == Compression::Tvdc {
        truncated = !tvdc(l, body, w, h, &mut pixels);
    } else {
        let mut row = vec![0u8; w * bpp];
        let mut rle = PixelRuns::new(body, bpp);
        for (y, out) in pixels.chunks_exact_mut(w * 4).enumerate() {
            let ok = !truncated
                && if info.compression == Compression::ByteRun1 {
                    rle.fill(&mut row)
                } else {
                    match body.get(y * row.len()..(y + 1) * row.len()) {
                        Some(s) => {
                            row.copy_from_slice(s);
                            true
                        }
                        None => false,
                    }
                };
            if !ok {
                truncated = true;
                for p in out.chunks_exact_mut(4) {
                    p.copy_from_slice(&[GREY, GREY, GREY, 255]);
                }
                continue;
            }
            for (px, s) in out.chunks_exact_mut(4).zip(row.chunks_exact(bpp)) {
                let mut at = 0;
                for &(n, ch) in &l.elements {
                    if let Some(c) = ch {
                        // The top byte of 16-bit elements.
                        px[c] = s[at];
                    }
                    at += n;
                }
            }
        }
    }
    if info.kind != Kind::Rgba32 {
        for p in pixels.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
    Image {
        width: info.width,
        height: info.height,
        pixels,
        truncated,
    }
}

/// TVDC (8-bit elements). Returns false when the data ends early (the rest is grey).
fn tvdc(l: &Layout, body: &[u8], w: usize, h: usize, pixels: &mut [u8]) -> bool {
    let nibbles = body.len() * 2;
    let nib = |i: usize| -> u8 {
        let b = body[i / 2];
        if i.is_multiple_of(2) { b >> 4 } else { b & 15 }
    };
    let mut i = 0usize;
    for y in 0..h {
        for &(_, ch) in &l.elements {
            let mut v = 0u8;
            let mut x = 0usize;
            while x < w {
                if i >= nibbles {
                    for p in pixels[y * w * 4..].chunks_exact_mut(4) {
                        p.copy_from_slice(&[GREY, GREY, GREY, 255]);
                    }
                    return false;
                }
                let d = l.table[usize::from(nib(i))];
                i += 1;
                let n = if d != 0 {
                    v = v.wrapping_add(d as u8);
                    1
                } else {
                    if i >= nibbles {
                        continue;
                    }
                    let n = usize::from(nib(i)) + 1;
                    i += 1;
                    n
                };
                for _ in 0..n.min(w - x) {
                    if let Some(c) = ch {
                        pixels[(y * w + x) * 4 + c] = v;
                    }
                    x += 1;
                }
            }
            // Each element row ends on a byte boundary.
            i = i.next_multiple_of(2);
        }
    }
    true
}

/// ByteRun1 in units of whole pixels (`unit` bytes), streamed row by row.
struct PixelRuns<'a> {
    src: &'a [u8],
    pos: usize,
    unit: usize,
    /// Literal bytes or repeated pixels left over from the last row.
    literal: usize,
    run: usize,
    value: Vec<u8>,
}

impl<'a> PixelRuns<'a> {
    fn new(src: &'a [u8], unit: usize) -> Self {
        PixelRuns {
            src,
            pos: 0,
            unit,
            literal: 0,
            run: 0,
            value: vec![0; unit],
        }
    }

    /// Fills `out` (a whole number of pixels); false when the data ends first.
    fn fill(&mut self, out: &mut [u8]) -> bool {
        let u = self.unit;
        let mut o = 0;
        while o < out.len() {
            if self.run > 0 {
                out[o..o + u].copy_from_slice(&self.value);
                o += u;
                self.run -= 1;
            } else if self.literal > 0 {
                let n = self.literal.min(out.len() - o);
                let Some(s) = self.src.get(self.pos..self.pos + n) else {
                    return false;
                };
                out[o..o + n].copy_from_slice(s);
                o += n;
                self.pos += n;
                self.literal -= n;
            } else {
                let Some(&c) = self.src.get(self.pos) else {
                    return false;
                };
                self.pos += 1;
                match c {
                    0..=127 => self.literal = (usize::from(c) + 1) * u,
                    128 => {}
                    _ => {
                        let Some(v) = self.src.get(self.pos..self.pos + u) else {
                            return false;
                        };
                        self.value.copy_from_slice(v);
                        self.pos += u;
                        self.run = 257 - usize::from(c);
                    }
                }
            }
        }
        true
    }
}

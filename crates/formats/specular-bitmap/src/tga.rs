//! Truevision TGA: colour-mapped (8-bit indices, 15/16/24/32-bit palette), true-colour
//! (15/16/24/32-bit), grey (8-bit, or 16-bit grey + alpha), each raw or RLE; any origin corner.
//!
//! TGA has no signature, so [`probe`] only checks that the 18-byte header is plausible; callers
//! should use it together with the file extension. Raw data is decoded straight from the file
//! by region and in parallel; RLE data is expanded once into raw pixels first.
//!
//! Alpha: 32-bit pixels use their alpha byte unless the TGA 2.0 extension area says it is not
//! alpha (attribute type 0–2), the descriptor declares no alpha bits, or every alpha value is 0.
//! 16-bit pixels treat the attribute bit the same way (declared by the descriptor's alpha bits
//! or the extension area, and set somewhere). Attribute type 4 (premultiplied) is divided out.

use crate::px::{expand, rgb_to_rgba, rgba_to_rgba};
use crate::scale::{Plan, Rows, fill_grey, render};
use crate::{Cancel, Decode, Error, Image, Info, Options, PixelFormat, Result, u16le, u32le};

/// Header plausibility (no magic number exists).
pub fn probe(head: &[u8]) -> bool {
    if head.len() < 18 {
        return false;
    }
    let (cmap, ty, cbits, depth, desc) = (head[1], head[2], head[7], head[16], head[17]);
    let w = u16le(head, 12).unwrap_or(0);
    let h = u16le(head, 14).unwrap_or(0);
    let ok_ty = match ty {
        1 | 9 => cmap == 1 && depth == 8 && matches!(cbits, 15 | 16 | 24 | 32),
        2 | 10 => cmap <= 1 && matches!(depth, 15 | 16 | 24 | 32),
        3 | 11 => cmap <= 1 && matches!(depth, 8 | 16),
        _ => false,
    };
    ok_ty && w > 0 && h > 0 && desc & 0xc0 == 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Mapped,
    True,
    Gray,
}

#[derive(Clone, Debug)]
pub struct Tga {
    info: Info,
    kind: Kind,
    rle: bool,
    /// Bytes per stored pixel (1–4).
    bpp: usize,
    top_down: bool,
    right_to_left: bool,
    use_alpha: bool,
    /// Attribute type 4: colour is premultiplied by alpha.
    premultiplied: bool,
    palette: Box<[[u8; 4]; 256]>,
    start: usize,
}

#[inline(always)]
fn px16(v: u16, alpha: bool) -> [u8; 4] {
    let c5 = |x: u16| (((x & 31) as u32 * 255 + 15) / 31) as u8;
    [
        c5(v >> 10),
        c5(v >> 5),
        c5(v),
        if alpha && v & 0x8000 == 0 { 0 } else { 255 },
    ]
}

impl Tga {
    pub fn parse(data: &[u8]) -> Result<Tga> {
        if data.len() < 18 {
            return Err(Error::Truncated("TGA header".into()));
        }
        if !probe(data) {
            return Err(Error::Malformed(
                "not a TGA file (or unsupported TGA type)".into(),
            ));
        }
        let id_len = data[0] as usize;
        let ty = data[2];
        let cfirst = u16le(data, 3).unwrap_or(0) as usize;
        let clen = u16le(data, 5).unwrap_or(0) as usize;
        let cbits = data[7] as usize;
        let w = u16le(data, 12).unwrap_or(0) as u32;
        let h = u16le(data, 14).unwrap_or(0) as u32;
        let depth = data[16] as usize;
        let desc = data[17];
        crate::check_dims(w, h)?;
        let kind = match ty & 7 {
            1 => Kind::Mapped,
            2 => Kind::True,
            _ => Kind::Gray,
        };
        let rle = ty & 8 != 0;
        let mut p = 18 + id_len;
        // TGA 2.0 extension area: attribute type says whether alpha is meaningful.
        let mut attr: Option<u8> = None;
        if data.len() >= 26 && data.ends_with(b"TRUEVISION-XFILE.\0") {
            let f = data.len() - 26;
            let ext = u32le(data, f).unwrap_or(0) as usize;
            if ext >= 18 && ext + 495 <= f {
                attr = Some(data[ext + 494]);
            }
        }
        let alpha_bits = desc & 15;
        let mut palette = Box::new([[0u8, 0, 0, 255]; 256]);
        let ces = cbits.div_ceil(8);
        let cmap_alpha_ok = !matches!(attr, Some(0..=2));
        if data[1] == 1 {
            let n = clen.min(65536);
            if kind == Kind::Mapped {
                for i in 0..n {
                    let idx = cfirst + i;
                    if idx > 255 {
                        break;
                    }
                    let Some(e) = data.get(p + i * ces..p + (i + 1) * ces) else {
                        break;
                    };
                    palette[idx] = match ces {
                        2 => px16(u16::from_le_bytes([e[0], e[1]]), false),
                        3 => [e[2], e[1], e[0], 255],
                        _ => [e[2], e[1], e[0], if cmap_alpha_ok { e[3] } else { 255 }],
                    };
                }
            }
            p += clen * ces;
        }
        let bpp = depth.div_ceil(8);
        // Uncompressed true colour: one pass over the alpha (byte or attribute bit) says
        // whether it is used at all (not 0 everywhere: writers that leave it clear mean opaque)
        // and whether any pixel is less than opaque; missing (truncated) pixels decode
        // transparent. RLE is taken at its word.
        let scan = |size: usize, alpha: fn(&[u8]) -> u8| {
            let n = w as usize * h as usize;
            let (mut any, mut below, mut seen) = (false, false, 0usize);
            for px in data.get(p..).unwrap_or(&[]).chunks_exact(size).take(n) {
                let a = alpha(px);
                any |= a != 0;
                below |= a != 255;
                seen += 1;
                if any && below {
                    break;
                }
            }
            (any, any && (below || seen < n))
        };
        let declared = alpha_bits > 0 || attr == Some(3) || attr == Some(4);
        let denied = matches!(attr, Some(0..=2));
        let (use_alpha, has_alpha) = match (kind, depth) {
            (Kind::True, 32 | 15 | 16) if !declared || denied => (false, false),
            (Kind::True, 32 | 15 | 16) if rle => (true, true),
            (Kind::True, 32) => scan(4, |px| px[3]),
            (Kind::True, 15 | 16) => scan(2, |px| if px[1] & 0x80 != 0 { 255 } else { 0 }),
            (Kind::Gray, 16) => (true, true),
            (Kind::Mapped, _) => {
                let on = cbits == 32 && cmap_alpha_ok && alpha_bits > 0;
                (on, on && palette.iter().any(|e| e[3] != 255))
            }
            _ => (false, false),
        };
        let gray = kind == Kind::Gray && depth == 8;
        let coding = format!(
            "TGA {}{}-bit{}",
            match kind {
                Kind::Mapped => "colour-mapped ",
                Kind::True => "",
                Kind::Gray => "grey ",
            },
            depth,
            if rle { ", RLE" } else { "" }
        );
        Ok(Tga {
            info: Info {
                width: w,
                height: h,
                has_alpha,
                alpha: if use_alpha && attr == Some(4) {
                    crate::AlphaKind::Premultiplied
                } else {
                    crate::AlphaKind::Straight
                },
                gray,
                model: match kind {
                    Kind::Mapped => crate::Model::Palette,
                    Kind::True => crate::Model::Rgb,
                    Kind::Gray => crate::Model::Grey,
                },
                // Per sample: 15/16-bit true colour holds 5-bit samples, 24/32-bit 8-bit
                // ones, 16-bit grey a grey and an alpha byte; a colour map index as stored.
                bit_depth: match (kind, depth) {
                    (Kind::True, 15 | 16) => 5,
                    (Kind::True | Kind::Gray, _) => 8,
                    (Kind::Mapped, d) => d as u8,
                },
                coding,
                random_access: !rle,
            },
            kind,
            rle,
            bpp,
            top_down: desc & 0x20 != 0,
            right_to_left: desc & 0x10 != 0,
            use_alpha,
            premultiplied: use_alpha && attr == Some(4),
            palette,
            start: p,
        })
    }

    fn ch(&self) -> usize {
        if self.info.gray { 1 } else { 4 }
    }

    /// Convert stored pixels `x0..x1` (in display order) of a stored row to straight alpha.
    fn conv(&self, row: &[u8], x0: u32, x1: u32, out: &mut [u8]) {
        self.conv_stored(row, x0, x1, out);
        if self.premultiplied {
            for o in out.chunks_exact_mut(4) {
                let a = o[3] as u32;
                if a != 0 && a != 255 {
                    for c in &mut o[..3] {
                        *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                    }
                }
            }
        }
    }

    /// Convert stored pixels `x0..x1` (in display order) of a stored row, colour as stored.
    fn conv_stored(&self, row: &[u8], x0: u32, x1: u32, out: &mut [u8]) {
        let (a, b) = (x0 as usize, x1 as usize);
        let bpp = self.bpp;
        if !self.right_to_left {
            let src = &row[a * bpp..b * bpp];
            match (self.kind, bpp) {
                (Kind::Gray, 1) => return out.copy_from_slice(src),
                (Kind::Mapped, 1) => return expand(src, &self.palette, out),
                (Kind::True, 3) => return rgb_to_rgba(src, out, true),
                (Kind::True, 4) => return rgba_to_rgba(src, out, true, !self.use_alpha),
                _ => {}
            }
        }
        let w = self.info.width as usize;
        let ch = self.ch();
        for (x, o) in (a..b).zip(out.chunks_exact_mut(ch)) {
            let sx = if self.right_to_left { w - 1 - x } else { x };
            let s = &row[sx * bpp..(sx + 1) * bpp];
            match (self.kind, bpp) {
                (Kind::Gray, 1) => o[0] = s[0],
                (Kind::Gray, _) => o.copy_from_slice(&[s[0], s[0], s[0], s[1]]),
                (Kind::Mapped, _) => o.copy_from_slice(&self.palette[s[0] as usize]),
                (_, 2) => {
                    o.copy_from_slice(&px16(u16::from_le_bytes([s[0], s[1]]), self.use_alpha))
                }
                (_, 3) => o.copy_from_slice(&[s[2], s[1], s[0], 255]),
                _ => {
                    o.copy_from_slice(&[s[2], s[1], s[0], if self.use_alpha { s[3] } else { 255 }])
                }
            }
        }
    }

    /// Expand RLE packets into raw stored pixels. Returns whether data ran out.
    fn unrle(&self, d: &[u8], cancel: Cancel) -> Result<(Vec<u8>, bool)> {
        let bpp = self.bpp;
        let total = self.info.width as usize * self.info.height as usize * bpp;
        let mut out = vec![0u8; total];
        let mut p = self.start;
        let mut o = 0;
        let mut n_packets = 0u32;
        while o < total {
            n_packets += 1;
            if n_packets.is_multiple_of(65536) && cancel() {
                return Err(Error::Cancelled);
            }
            let Some(&hd) = d.get(p) else {
                grey_raw(&mut out[o..], bpp, self.kind);
                return Ok((out, true));
            };
            p += 1;
            let n = ((hd & 127) as usize + 1) * bpp;
            let n = n.min(total - o);
            if hd & 128 != 0 {
                let Some(px) = d.get(p..p + bpp) else {
                    grey_raw(&mut out[o..], bpp, self.kind);
                    return Ok((out, true));
                };
                p += bpp;
                for c in out[o..o + n].chunks_exact_mut(bpp) {
                    c.copy_from_slice(px);
                }
            } else {
                let avail = d.len().saturating_sub(p).min(n);
                out[o..o + avail].copy_from_slice(&d[p..p + avail]);
                if avail < n {
                    grey_raw(&mut out[o + avail..], bpp, self.kind);
                    return Ok((out, true));
                }
                p += n;
            }
            o += n;
        }
        Ok((out, false))
    }
}

/// Fill raw stored pixels with something that converts to grey (mid-grey for true colour
/// and grey; index 0 for mapped images).
fn grey_raw(out: &mut [u8], bpp: usize, kind: Kind) {
    match (kind, bpp) {
        (Kind::Mapped, _) => out.fill(0),
        (_, 2) if kind == Kind::Gray => {
            for c in out.chunks_mut(2) {
                c.copy_from_slice(&[crate::GREY, 255][..c.len()]);
            }
        }
        (_, 2) => {
            // 0x4210 | 0x8000: mid-grey 5-5-5, opaque.
            for c in out.chunks_mut(2) {
                c.copy_from_slice(&[0x10, 0xC2][..c.len()]);
            }
        }
        (_, 4) => {
            for c in out.chunks_mut(4) {
                c.copy_from_slice(&[crate::GREY, crate::GREY, crate::GREY, 255][..c.len()]);
            }
        }
        _ => out.fill(crate::GREY),
    }
}

struct RawRows<'a> {
    d: &'a [u8],
    start: usize,
    t: &'a Tga,
}

impl Rows for RawRows<'_> {
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let t = self.t;
        let (w, h) = (t.info.width as usize, t.info.height as usize);
        let fy = if t.top_down {
            y as usize
        } else {
            h - 1 - y as usize
        };
        let rb = w * t.bpp;
        let s = self.start + fy * rb;
        match self.d.get(s..s + rb) {
            Some(row) => t.conv(row, x0, x1, out),
            None => {
                // Partly present: pad with grey-converting bytes.
                let mut tmp = vec![0u8; rb];
                grey_raw(&mut tmp, t.bpp, t.kind);
                if let Some(part) = self.d.get(s..) {
                    tmp[..part.len().min(rb)].copy_from_slice(&part[..part.len().min(rb)]);
                }
                t.conv(&tmp, x0, x1, out);
                if t.kind == Kind::Mapped {
                    let ch = t.ch();
                    let have = self.d.len().saturating_sub(s) / t.bpp;
                    for (x, o) in (x0 as usize..x1 as usize).zip(out.chunks_exact_mut(ch)) {
                        let sx = if t.right_to_left { w - 1 - x } else { x };
                        if sx >= have {
                            fill_grey(o, ch);
                        }
                    }
                }
            }
        }
    }
}

impl Decode for Tga {
    fn info(&self) -> &Info {
        &self.info
    }

    fn decode(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        let (w, h) = (self.info.width, self.info.height);
        let plan = Plan::new(w, h, opts)?;
        let ch = self.ch();
        let opaque = !self.info.has_alpha;
        // A packet is 128 pixels at most.
        crate::proven(data.len(), w, h, if self.rle { 128 } else { 1 })?;
        let (out, truncated) = if self.rle {
            let (raw, truncated) = self.unrle(data, cancel)?;
            let rows = RawRows {
                d: &raw,
                start: 0,
                t: self,
            };
            (
                render(&rows, ch, opaque, &plan, opts.parallel, cancel)?,
                truncated,
            )
        } else {
            let rows = RawRows {
                d: data,
                start: self.start,
                t: self,
            };
            let need = self.start as u64 + w as u64 * h as u64 * self.bpp as u64;
            (
                render(&rows, ch, opaque, &plan, opts.parallel, cancel)?,
                (data.len() as u64) < need,
            )
        };
        Ok(Image {
            width: plan.rw,
            height: plan.rh,
            format: if ch == 1 {
                PixelFormat::Gray8
            } else {
                PixelFormat::Rgba8
            },
            data: out,
            truncated,
        })
    }
}

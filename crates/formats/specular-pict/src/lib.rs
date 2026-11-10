//! Apple QuickDraw PICT (version 1 and 2): bitmaps, QuickDraw vector drawing and QuickTime
//! images.
//!
//! A PICT file is an optional 512-byte application header (usually zeros), then a picture:
//! `picSize` (16-bit, meaningless for large pictures), `picFrame` (top, left, bottom, right)
//! and a stream of QuickDraw opcodes. Version 1 pictures have 8-bit opcodes; version 2
//! pictures start `0011 02FF 0C00` and have 16-bit opcodes, each padded to an even offset.
//!
//! Drawn here, in order, onto a white canvas the size of the frame (or of the extended
//! version 2 header's source rectangle):
//!
//! | Opcode | |
//! |---|---|
//! | `0x90` BitsRect, `0x91` BitsRgn | uncompressed BitMap (1-bit) or PixMap (1/2/4/8-bit indexed with a colour table) |
//! | `0x98` PackBitsRect, `0x99` PackBitsRgn | the same, rows PackBits coded |
//! | `0x9A` DirectBitsRect, `0x9B` DirectBitsRgn | 16-bit (x-5-5-5) and 32-bit pixels, pack types 1–4, alpha with four components |
//! | `0x8200` CompressedQuickTime | QuickTime images: `jpeg` (`specular-jpeg`), `png ` (`specular-png`), `tiff` (`specular-tiff`), `WRLE` (BMP, `specular-bitmap`), `raw `, and Animation, Graphics, Apple Video and Cinepak key frames (`specular-oldvideo`) |
//! | lines, rectangles, rounded rectangles, ovals, arcs, polygons, regions, text, pens, patterns, colours, clip, origin | see the `quickdraw` module, drawn with `specular-drawpage` |
//!
//! Source rectangles are scaled to destination rectangles (nearest neighbour) and clipped
//! to the canvas; the `Rgn` variants clip to their mask region. Bitmap transfer modes are
//! treated as copy. A 32-bit pixel map with alpha that is the first thing drawn leaves the
//! canvas transparent around it ([`Image::alpha`]; QuickDraw itself ignores alpha, but
//! writers such as ImageMagick store real transparency there). What follows a
//! CompressedQuickTime opcode up to the next one is the placeholder for readers without
//! QuickTime and is not drawn. Other QuickTime codecs (Sorenson, H.263 and so on) are
//! refused by name when nothing else is drawn.
//!
//! Robustness: no input panics. The canvas is capped by [`MAX_PIXELS`], the opcode count by
//! [`MAX_OPS`] and the total pixels drawn by a multiple of the canvas size, so hostile
//! files cannot make drawing quadratic. A picture whose opcode stream breaks after
//! something was drawn is returned with [`Image::truncated`] set.

mod qt;
mod quickdraw;

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use specular_drawpage as dp;

/// Largest canvas accepted (256 MP). PICT coordinates are 16-bit.
pub const MAX_PIXELS: u64 = 1 << 28;
/// Upper bound on opcodes walked.
pub const MAX_OPS: usize = 1 << 20;
/// Pixels drawn per canvas pixel before drawing stops (overdraw budget).
const OVERDRAW: u64 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a PICT file.
    NotPict,
    /// A valid picture with nothing this decoder can show, or a feature it does not
    /// implement.
    Unsupported(String),
    Truncated(&'static str),
    Malformed(&'static str),
    /// Over [`MAX_PIXELS`].
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotPict => f.write_str("not a PICT file"),
            Error::Unsupported(s) => write!(f, "unsupported PICT: {s}"),
            Error::Truncated(s) => write!(f, "PICT file is truncated: {s}"),
            Error::Malformed(s) => write!(f, "corrupt PICT file: {s}"),
            Error::TooLarge => f.write_str("PICT image too large"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// A QuickDraw rectangle (`top, left, bottom, right`; bottom and right exclusive).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn is_empty(&self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }
}

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// 1 or 2.
    pub version: u8,
    /// The file starts with the 512-byte application header.
    pub has_header: bool,
    /// Resolution from an extended version 2 header (dots per inch), when not 72.
    pub dpi: Option<(u32, u32)>,
}

/// Decoded pixels: RGBA8 (opaque unless [`Image::alpha`]), `width × height`, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// The opcode stream or some pixel data ended early or broke; what was drawn by then is
    /// shown.
    pub truncated: bool,
    /// Bitmap opcodes drawn.
    pub bitmaps: usize,
    /// QuickTime JPEG images drawn.
    pub jpegs: usize,
    /// QuickTime images in other codecs (PNG, TIFF, BMP, raw) drawn.
    pub quicktime: usize,
    /// QuickDraw shapes, lines and text strings drawn.
    pub shapes: usize,
    /// The picture starts with a 32-bit pixel map that has an alpha channel: the canvas is
    /// transparent where nothing covers it, and `pixels` has straight alpha.
    pub alpha: bool,
}

const V2_SIG: [u8; 6] = [0x00, 0x11, 0x02, 0xff, 0x0c, 0x00];

/// Whether `head` starts like a version 2 picture without the 512-byte header (the
/// signature at offset 10). With the header, the signature is past the usual 64-byte probe
/// window: see [`probe_file`].
pub fn probe(head: &[u8]) -> bool {
    head.get(10..16) == Some(&V2_SIG[..])
}

/// Whether `data` holds a version 2 picture, with or without the 512-byte header.
pub fn probe_file(data: &[u8]) -> bool {
    probe(data) || data.get(522..528) == Some(&V2_SIG[..])
}

/// Whether `data` looks like any PICT (version 1 or 2, with or without the header); weaker
/// than [`probe_file`], for files already named `.pict` / `.pct`.
pub fn plausible(data: &[u8]) -> bool {
    start(data).is_some()
}

/// (picture start, version).
fn start(data: &[u8]) -> Option<(usize, u8)> {
    [512, 0].into_iter().find_map(|off| {
        let v = data.get(off + 10..off + 14)?;
        let frame = data.get(off + 2..off + 10)?;
        let r = rect_at(frame, 0);
        if r.is_empty() {
            return None;
        }
        if v == [0x00, 0x11, 0x02, 0xff] {
            Some((off, 2))
        } else if v[..2] == [0x11, 0x01] {
            Some((off, 1))
        } else {
            None
        }
    })
}

fn be16(d: &[u8], p: usize) -> u16 {
    u16::from_be_bytes([d[p], d[p + 1]])
}

fn be32(d: &[u8], p: usize) -> u32 {
    u32::from_be_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]])
}

fn rect_at(d: &[u8], p: usize) -> Rect {
    let v = |i| i32::from(be16(d, p + i) as i16);
    Rect {
        top: v(0),
        left: v(2),
        bottom: v(4),
        right: v(6),
    }
}

/// Canvas rectangle and resolution: the extended version 2 header's source rectangle when
/// it has one, else the frame.
fn canvas_of(data: &[u8], base: usize, version: u8) -> (Rect, Option<(u32, u32)>) {
    let frame = rect_at(data, base + 2);
    if version == 2
        && let Some(h) = data.get(base + 14..base + 40)
        && h[..2] == [0x0c, 0x00]
        && be16(h, 2) == 0xfffe
    {
        let (hr, vr) = (be32(h, 6) >> 16, be32(h, 10) >> 16);
        let src = rect_at(h, 14);
        let dpi = (hr != 0 && vr != 0 && (hr, vr) != (72, 72)).then_some((hr, vr));
        if !src.is_empty() {
            return (src, dpi);
        }
        return (frame, dpi);
    }
    (frame, None)
}

/// Header facts without drawing.
pub fn info(data: &[u8]) -> Result<Info> {
    let (base, version) = start(data).ok_or(Error::NotPict)?;
    let (canvas, dpi) = canvas_of(data, base, version);
    let (w, h) = (canvas.width() as u32, canvas.height() as u32);
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    Ok(Info {
        width: w,
        height: h,
        version,
        has_header: base == 512,
        dpi,
    })
}

struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
}

const SHORT: Error = Error::Truncated("opcode data");

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self.d.get(self.pos..self.pos.checked_add(n).ok_or(SHORT)?);
        let s = s.ok_or(SHORT)?;
        self.pos += n;
        Ok(s)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(be16(self.take(2)?, 0))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(be32(self.take(4)?, 0))
    }

    fn rect(&mut self) -> Result<Rect> {
        Ok(rect_at(self.take(8)?, 0))
    }
}

/// A BitMap or PixMap header with its colour table.
struct Map {
    row_bytes: usize,
    bounds: Rect,
    /// A 1-bit BitMap (foreground / background colours) rather than a PixMap.
    bitmap: bool,
    pack_type: u16,
    pixel_size: u16,
    cmp_count: u16,
    palette: Vec<[u8; 3]>,
}

fn read_map(r: &mut Reader, direct: bool) -> Result<Map> {
    if direct {
        r.skip(4)?; // baseAddr
    }
    let rb = r.u16()?;
    let bounds = r.rect()?;
    let pixmap = direct || rb & 0x8000 != 0;
    let mut m = Map {
        row_bytes: usize::from(rb & if pixmap { 0x3fff } else { 0x7fff }),
        bounds,
        bitmap: !pixmap,
        pack_type: 0,
        pixel_size: 1,
        cmp_count: 1,
        palette: Vec::new(),
    };
    if pixmap {
        let p = r.take(36)?;
        m.pack_type = be16(p, 2);
        m.pixel_size = be16(p, 18);
        m.cmp_count = be16(p, 20);
        if !direct {
            m.palette = read_ctab(r)?;
        }
    }
    if bounds.is_empty() || bounds.width() > 0x7fff {
        return Err(Error::Malformed("pixmap bounds"));
    }
    let w = bounds.width() as usize;
    let need = match m.pixel_size {
        1 | 2 | 4 | 8 if !direct => (w * usize::from(m.pixel_size)).div_ceil(8),
        16 if direct => w * 2,
        32 if direct => match m.cmp_count {
            3 | 4 => w * 3,
            _ => return Err(Error::Unsupported(format!("{} components", m.cmp_count))),
        },
        s => return Err(Error::Unsupported(format!("{s}-bit pixmap"))),
    };
    if m.row_bytes < need {
        return Err(Error::Malformed("rowBytes too small for the bounds"));
    }
    Ok(m)
}

fn read_ctab(r: &mut Reader) -> Result<Vec<[u8; 3]>> {
    r.skip(4)?; // seed
    let device = r.u16()? & 0x8000 != 0;
    let n = usize::from(r.u16()?.wrapping_add(1));
    let mut pal = vec![[0u8; 3]; 256];
    let entries = r.take(n.checked_mul(8).ok_or(SHORT)?)?;
    for (i, e) in entries.chunks_exact(8).enumerate() {
        let at = if device { i } else { usize::from(be16(e, 0)) };
        if let Some(p) = pal.get_mut(at) {
            *p = [e[2], e[4], e[6]];
        }
    }
    Ok(pal)
}

/// PackBits from `src` into `out` in units of `unit` bytes (1, or 2 for 16-bit pixels).
/// Missing data leaves zeros; excess is dropped.
fn unpack_bits(src: &[u8], out: &mut [u8], unit: usize) {
    let (mut i, mut o) = (0, 0);
    while i < src.len() && o < out.len() {
        let c = src[i];
        i += 1;
        if c < 128 {
            let n = (usize::from(c) + 1) * unit;
            let s = &src[i..(i + n).min(src.len())];
            let m = s.len().min(out.len() - o);
            out[o..o + m].copy_from_slice(&s[..m]);
            i += n;
            o += m;
        } else if c > 128 {
            let n = 257 - usize::from(c);
            let Some(v) = src.get(i..i + unit) else { break };
            i += unit;
            for _ in 0..n {
                let m = unit.min(out.len() - o);
                out[o..o + m].copy_from_slice(&v[..m]);
                o += m;
                if o == out.len() {
                    break;
                }
            }
        }
    }
    out[o..].fill(0);
}

/// The pack type a direct PixMap's rows are actually stored with: 0 means the default for
/// the depth, and rows under 8 bytes are never packed.
fn direct_pack(m: &Map) -> u16 {
    match m.pack_type {
        _ if m.row_bytes < 8 => 1,
        0 if m.pixel_size == 16 => 3,
        0 => 4,
        t => t,
    }
}

/// Reads one row of `m` from `r` into `buf` (unpacked). `packed`: a PackBits opcode.
fn read_row(r: &mut Reader, m: &Map, packed: bool, direct: bool, buf: &mut Vec<u8>) -> Result<()> {
    let w = m.bounds.width() as usize;
    let rb = m.row_bytes;
    // (bytes per row once unpacked, PackBits unit or 0 for raw)
    let (len, unit) = if direct {
        match (m.pixel_size, direct_pack(m)) {
            (16, 3) => (rb, 2),
            (32, 2) => (w * 3, 0),
            (32, 4) => (w * usize::from(m.cmp_count), 1),
            (16, _) | (32, _) => (rb, 0),
            _ => return Err(Error::Malformed("pixel size")),
        }
    } else if packed && rb >= 8 {
        (rb, 1)
    } else {
        (rb, 0)
    };
    buf.resize(len, 0);
    if unit == 0 {
        buf.copy_from_slice(r.take(len)?);
        return Ok(());
    }
    // The count is a word when rowBytes > 250. Some writers (ImageMagick for 32-bit pack
    // type 4) decide by the unpacked length instead; a word count larger than any PackBits
    // coding of the row could be means that happened.
    let most = 2 * len + 4;
    let count = match r.d.get(r.pos..r.pos + 2) {
        Some(c) if rb > 250 && usize::from(be16(c, 0)) <= most => usize::from(r.u16()?),
        _ => usize::from(r.u8()?),
    };
    unpack_bits(r.take(count)?, buf, unit);
    Ok(())
}

/// Converts one unpacked row to RGBA.
fn row_rgba(m: &Map, buf: &[u8], fg: [u8; 3], bg: [u8; 3], out: &mut [u8]) {
    let w = m.bounds.width() as usize;
    let direct_planar = m.pixel_size == 32 && direct_pack(m) == 4;
    for (x, px) in out.chunks_exact_mut(4).enumerate().take(w) {
        let rgb = match m.pixel_size {
            1 | 2 | 4 | 8 => {
                let bits = usize::from(m.pixel_size);
                let bit = x * bits;
                let v = buf.get(bit / 8).map_or(0, |b| {
                    (usize::from(*b) >> (8 - bits - bit % 8)) & ((1 << bits) - 1)
                });
                if m.bitmap {
                    if v == 1 { fg } else { bg }
                } else {
                    m.palette.get(v).copied().unwrap_or([0; 3])
                }
            }
            16 => {
                let v = buf.get(x * 2..x * 2 + 2).map_or(0, |s| be16(s, 0));
                let c = |s: u16| {
                    let c = ((v >> s) & 31) as u8;
                    (c << 3) | (c >> 2)
                };
                [c(10), c(5), c(0)]
            }
            _ if direct_planar => {
                let o = if m.cmp_count == 4 { w } else { 0 };
                [buf[o + x], buf[o + w + x], buf[o + 2 * w + x]]
            }
            _ if buf.len() == w * 3 => [buf[x * 3], buf[x * 3 + 1], buf[x * 3 + 2]],
            _ => buf
                .get(x * 4 + 1..x * 4 + 4)
                .map_or([0; 3], |s| [s[0], s[1], s[2]]),
        };
        // 32-bit maps with four components carry alpha: the first plane (pack type 4) or
        // the first byte of each pixel.
        let a = match m.pixel_size {
            32 if m.cmp_count == 4 && direct_planar => buf[x],
            32 if m.cmp_count == 4 && buf.len() >= w * 4 => buf[x * 4],
            _ => 255,
        };
        px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
    }
}

/// A QuickDraw region, streamed by row: inversion points accumulate down the rows.
struct Region {
    bbox: Rect,
    /// Scanline records: y, x…, 0x7FFF; ends with 0x7FFF.
    data: Vec<i32>,
    pos: usize,
    /// Current inversion points, sorted.
    cur: Vec<i32>,
}

impl Region {
    fn read(r: &mut Reader) -> Result<Region> {
        let size = usize::from(r.u16()?);
        let bbox = r.rect()?;
        let rest = r.take(size.saturating_sub(10))?;
        let data = rest
            .chunks_exact(2)
            .map(|c| i32::from(be16(c, 0) as i16))
            .collect();
        Ok(Region {
            bbox,
            data,
            pos: 0,
            cur: Vec::new(),
        })
    }

    /// Applies the records up to row `y` (rows asked for in increasing order).
    fn advance(&mut self, y: i32) {
        while let Some(&ry) = self.data.get(self.pos) {
            if ry == 0x7fff || ry > y {
                break;
            }
            self.pos += 1;
            while let Some(&x) = self.data.get(self.pos) {
                self.pos += 1;
                if x == 0x7fff {
                    break;
                }
                match self.cur.binary_search(&x) {
                    Ok(i) => {
                        self.cur.remove(i);
                    }
                    Err(i) if self.cur.len() < 1 << 14 => self.cur.insert(i, x),
                    Err(_) => {}
                }
            }
        }
    }

    /// Fills `inside[i]` for x = `x0 + i` on the current row.
    fn row(&self, y: i32, x0: i32, inside: &mut [bool]) {
        let in_box = y >= self.bbox.top && y < self.bbox.bottom;
        if self.data.is_empty() {
            for (i, v) in inside.iter_mut().enumerate() {
                let x = x0 + i as i32;
                *v = in_box && x >= self.bbox.left && x < self.bbox.right;
            }
            return;
        }
        let mut k = self.cur.partition_point(|&p| p <= x0);
        for (i, v) in inside.iter_mut().enumerate() {
            let x = x0 + i as i32;
            while k < self.cur.len() && self.cur[k] <= x {
                k += 1;
            }
            *v = in_box && k % 2 == 1;
        }
    }
}

/// Where a QuickTime image sits and its codec data.
#[derive(Clone, Debug)]
struct QtImage {
    codec: [u8; 4],
    data: Range<usize>,
    width: u32,
    height: u32,
    src: Rect,
    dst: Rect,
}

struct Walker<'a> {
    data: &'a [u8],
    base: usize,
    v2: bool,
    canvas: Rect,
    draw: bool,
    pixels: Vec<u8>,
    /// QuickDraw state: colours, pen, patterns, text, clip.
    qd: quickdraw::State,
    /// Vector drawing not yet composited onto `pixels`.
    ops: Vec<dp::Op>,
    bitmaps: usize,
    jpegs: usize,
    quicktime: usize,
    shapes: usize,
    /// The canvas starts transparent (the first thing drawn has alpha).
    alpha: bool,
    qt: Vec<QtImage>,
    unsupported_codec: Option<[u8; 4]>,
    /// A known QuickTime codec at a depth it does not have, in words.
    unsupported_why: Option<String>,
    /// After CompressedQuickTime: what follows, up to the next one, is the fallback for
    /// readers without QuickTime (a bitmap, or "QuickTime … needed to see this picture"
    /// text), not part of the picture.
    fallback_next: bool,
    /// A QuickTime image failed to decode.
    broken_qt: bool,
    truncated: bool,
    work: u64,
    budget: u64,
}

impl Walker<'_> {
    fn ensure_canvas(&mut self) {
        if self.draw && self.pixels.is_empty() {
            let n = self.canvas.width() as usize * self.canvas.height() as usize;
            self.pixels = vec![if self.alpha { 0 } else { 255 }; n * 4];
        }
    }

    /// Composites the pending vector drawing onto the canvas.
    fn flush(&mut self) {
        if self.ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.ops);
        let Some(mut page) = self.page() else {
            return;
        };
        page.ops.extend(ops);
        if let Some(px) = self.render(&page) {
            self.pixels = px;
            // Vector drawing renders onto white.
            self.alpha = false;
        }
    }

    /// A page of the canvas size holding the canvas drawn so far.
    fn page(&mut self) -> Option<dp::Page> {
        self.ensure_canvas();
        let (w, h) = (self.canvas.width() as u32, self.canvas.height() as u32);
        let mut page = dp::Page::new("", (f64::from(w), f64::from(h)));
        if self.drawn_pixels() {
            let img = dp::Image::from_rgba(w, h, self.pixels.clone())?;
            page.ops.push(dp::Op::Image {
                image: Arc::new(img),
                transform: dp::Affine::IDENTITY,
                opacity: 1.0,
            });
        }
        Some(page)
    }

    fn render(&self, page: &dp::Page) -> Option<Vec<u8>> {
        let (w, h) = (self.canvas.width() as u32, self.canvas.height() as u32);
        let req = dp::RenderRequest {
            scale_x: 1.0,
            scale_y: 1.0,
            x: 0,
            y: 0,
            width: w,
            height: h,
        };
        dp::render(page, req, &|| false).ok()
    }

    /// Whether anything was drawn onto `pixels` yet.
    fn drawn_pixels(&self) -> bool {
        self.bitmaps + self.jpegs + self.quicktime > 0 || self.shapes > 0
    }

    /// Inverts the canvas under `path` (canvas pixels), by its coverage.
    fn invert(&mut self, path: dp::BezPath) {
        self.flush();
        self.ensure_canvas();
        let (w, h) = (self.canvas.width(), self.canvas.height());
        let mut mask = dp::Page::new("", (f64::from(w), f64::from(h)));
        mask.ops.push(dp::Op::Fill {
            path,
            even_odd: true,
            paint: dp::Paint::BLACK,
        });
        let Some(m) = self.render(&mask) else {
            return;
        };
        for (p, m) in self.pixels.chunks_exact_mut(4).zip(m.chunks_exact(4)) {
            let cov = u32::from(255 - m[0]);
            if cov > 0 {
                for c in &mut p[..3] {
                    let v = u32::from(*c);
                    *c = ((v * (255 - cov) + (255 - v) * cov + 127) / 255) as u8;
                }
            }
        }
    }

    /// Draws source row `y` (picture coordinates of `bounds`) into every destination row
    /// mapping to it, starting at `*next_dy`.
    #[allow(clippy::too_many_arguments)]
    fn blit_row(
        &mut self,
        y: i32,
        rgba: &[u8],
        bounds: Rect,
        src: Rect,
        dst: Rect,
        mask: &mut Option<Region>,
        next_dy: &mut i32,
        inside: &mut Vec<bool>,
    ) {
        let (sw, sh, dw, dh) = (
            i64::from(src.width()),
            i64::from(src.height()),
            i64::from(dst.width()),
            i64::from(dst.height()),
        );
        let x0 = dst.left.max(self.canvas.left);
        let x1 = dst.right.min(self.canvas.right);
        let cw = self.canvas.width() as usize;
        // Only rows on the canvas draw: skip the rest at once (walking them one by one was
        // up to 65,535 rows for each of many small bitmaps).
        let top = dst.top.max(self.canvas.top);
        let bottom = dst.bottom.min(self.canvas.bottom);
        if x0 >= x1 || top >= bottom {
            *next_dy = dst.bottom;
            return;
        }
        *next_dy = (*next_dy).max(top);
        while *next_dy < bottom {
            let dy = *next_dy;
            let sy = src.top + (i64::from(dy - dst.top) * sh / dh) as i32;
            if sy > y {
                break;
            }
            *next_dy += 1;
            if sy < y || dy < self.canvas.top || dy >= self.canvas.bottom || x0 >= x1 {
                continue;
            }
            self.work += (x1 - x0) as u64;
            if self.work > self.budget {
                self.truncated = true;
                *next_dy = dst.bottom;
                return;
            }
            if let Some(m) = mask.as_mut() {
                m.advance(dy);
                inside.resize((x1 - x0) as usize, false);
                m.row(dy, x0, inside);
            }
            let row = (dy - self.canvas.top) as usize * cw;
            for dx in x0..x1 {
                if mask.is_some() && !inside[(dx - x0) as usize] {
                    continue;
                }
                let sx = src.left + (i64::from(dx - dst.left) * sw / dw) as i32;
                let bx = sx - bounds.left;
                if bx < 0 || bx >= bounds.width() {
                    continue;
                }
                let s = bx as usize * 4;
                let d = (row + (dx - self.canvas.left) as usize) * 4;
                over(&mut self.pixels[d..d + 4], &rgba[s..s + 4]);
            }
        }
        if *next_dy >= bottom {
            // Past the canvas: the remaining source rows draw nothing.
            *next_dy = dst.bottom;
        }
    }

    /// BitsRect / BitsRgn / PackBitsRect / PackBitsRgn / DirectBitsRect / DirectBitsRgn.
    fn bits(&mut self, r: &mut Reader, op: u16) -> Result<()> {
        let direct = matches!(op, 0x9a | 0x9b);
        let packed = matches!(op, 0x98 | 0x99);
        let m = read_map(r, direct)?;
        let src = r.rect()?;
        let dst = r.rect()?;
        r.skip(2)?; // transfer mode
        let mut mask = if matches!(op, 0x91 | 0x99 | 0x9b) {
            Some(Region::read(r)?)
        } else {
            None
        };
        let fallback = self.fallback_next;
        let visible = self.draw && !fallback && !src.is_empty() && !dst.is_empty();
        // A 32-bit map with alpha drawn first: the canvas starts transparent.
        if !fallback && m.pixel_size == 32 && m.cmp_count == 4 && !self.drawn_pixels() {
            self.alpha = true;
        }
        if visible {
            self.flush();
            self.ensure_canvas();
        }
        let w = m.bounds.width() as usize;
        let mut buf = Vec::with_capacity(m.row_bytes);
        let mut rgba = vec![0u8; if visible { w * 4 } else { 0 }];
        let mut inside = Vec::new();
        let mut next_dy = dst.top;
        for y in m.bounds.top..m.bounds.bottom {
            if let Err(e) = read_row(r, &m, packed, direct, &mut buf) {
                self.bitmaps += usize::from(!fallback);
                return Err(e);
            }
            if visible && y >= src.top && y < src.bottom && next_dy < dst.bottom {
                row_rgba(&m, &buf, self.qd.fg, self.qd.bg, &mut rgba);
                self.blit_row(
                    y,
                    &rgba,
                    m.bounds,
                    src,
                    dst,
                    &mut mask,
                    &mut next_dy,
                    &mut inside,
                );
            }
        }
        self.bitmaps += usize::from(!fallback);
        Ok(())
    }

    /// CompressedQuickTime: records the image; draws it when it is JPEG.
    fn quicktime(&mut self, r: &mut Reader, len: usize) -> Result<()> {
        let start = r.pos;
        let body = r.take(len)?;
        let at = |p: usize, n: usize| body.get(p..p + n).ok_or(Error::Truncated("QuickTime"));
        let fixed = |p: usize| -> Result<i64> { Ok(i64::from(be32(at(p, 4)?, 0) as i32)) };
        let (sx, sy) = (fixed(2)?, fixed(18)?);
        let (tx, ty) = (fixed(26)? >> 16, fixed(30)? >> 16);
        let matte = be32(at(38, 4)?, 0) as usize;
        let src = rect_at(at(52, 8)?, 0);
        let mask = be32(at(64, 4)?, 0) as usize;
        let id = 68usize.saturating_add(matte).saturating_add(mask);
        let desc = at(id, 86)?;
        // The description with what follows it (a colour table) up to its stated size.
        let desc_all = body
            .get(id..id.saturating_add((be32(desc, 0) as usize).max(86)).min(len))
            .unwrap_or(desc);
        let codec: [u8; 4] = desc[4..8].try_into().unwrap_or_default();
        let (w, h) = (be16(desc, 32), be16(desc, 34));
        let depth = be16(desc, 82) as i16;
        let size = be32(desc, 44) as usize;
        let from = id.saturating_add(be32(desc, 0) as usize).min(len);
        let to = if size == 0 {
            len
        } else {
            from.saturating_add(size).min(len)
        };
        let src = if src.is_empty() {
            Rect {
                top: 0,
                left: 0,
                bottom: i32::from(h),
                right: i32::from(w),
            }
        } else {
            src
        };
        let scale = |v: i32, s: i64| ((i64::from(v) * s) >> 16).clamp(-0x7fff, 0x7fff) as i32;
        let dst = Rect {
            top: ty as i32,
            left: tx as i32,
            bottom: ty as i32 + scale(src.height(), sy),
            right: tx as i32 + scale(src.width(), sx),
        };
        let q = QtImage {
            codec,
            data: start + from..start + to,
            width: u32::from(w),
            height: u32::from(h),
            src,
            dst,
        };
        if &codec == b"jpeg" {
            if self.draw && !dst.is_empty() {
                self.flush();
                self.draw_jpeg(&q);
            } else {
                self.jpegs += 1;
            }
        } else if let Some(why) = qt::refusal(&codec, depth) {
            self.unsupported_why.get_or_insert(why);
        } else if qt::known(&codec) {
            if self.draw && !dst.is_empty() {
                self.flush();
                let pixels = &self.data[q.data.clone()];
                match qt::decode(&codec, pixels, q.width, q.height, depth, desc_all) {
                    Some((w, h, px)) => {
                        self.draw_rgba(&q, w, h, &px);
                        self.quicktime += 1;
                    }
                    None => {
                        self.truncated = true;
                        self.broken_qt = true;
                    }
                }
            } else {
                self.quicktime += 1;
            }
        } else {
            self.unsupported_codec.get_or_insert(codec);
        }
        self.qt.push(q);
        self.fallback_next = true;
        Ok(())
    }

    fn draw_jpeg(&mut self, q: &QtImage) {
        let opts = specular_jpeg::DecodeOptions::default();
        let Ok(planes) = specular_jpeg::decode(&self.data[q.data.clone()], &opts) else {
            self.truncated = true;
            self.broken_qt = true;
            return;
        };
        if planes.truncated {
            self.truncated = true;
        }
        let rgba = specular_jpeg::planes_to_rgba(&planes);
        self.draw_rgba(q, planes.width, planes.height, &rgba);
        self.jpegs += 1;
    }

    /// Draws a decoded QuickTime image (`width`×`height` RGBA) at its place.
    fn draw_rgba(&mut self, q: &QtImage, width: u32, height: u32, rgba: &[u8]) {
        let bounds = Rect {
            top: 0,
            left: 0,
            bottom: height as i32,
            right: width as i32,
        };
        self.ensure_canvas();
        let w = width as usize * 4;
        let mut next_dy = q.dst.top;
        let mut inside = Vec::new();
        for (y, row) in rgba.chunks_exact(w.max(1)).enumerate() {
            let y = y as i32;
            if y >= q.src.top && y < q.src.bottom {
                self.blit_row(
                    y,
                    row,
                    bounds,
                    q.src,
                    q.dst,
                    &mut None,
                    &mut next_dy,
                    &mut inside,
                );
            }
        }
    }

    fn drawn(&self) -> bool {
        self.bitmaps + self.jpegs + self.quicktime + self.shapes > 0
    }

    fn run(&mut self) -> Result<()> {
        let mut r = Reader {
            d: self.data,
            pos: self.base + 10,
        };
        for _ in 0..MAX_OPS {
            if self.v2 && (r.pos - self.base) % 2 == 1 {
                r.pos += 1;
            }
            if r.pos >= self.data.len() {
                // No end opcode; what was drawn stands.
                return Ok(());
            }
            if self.qd.elements > dp::MAX_PATH_ELEMENTS {
                return Err(Error::Malformed("too much vector drawing"));
            }
            let op = if self.v2 {
                r.u16()?
            } else {
                u16::from(r.u8()?)
            };
            match op {
                0xff => return Ok(()),
                0x90 | 0x91 | 0x98 | 0x99 | 0x9a | 0x9b => self.bits(&mut r, op)?,
                0x12..=0x14 => {
                    let p = quickdraw::pix_pat(&mut r, self.qd.fg, self.qd.bg)?;
                    self.qd.set_pix_pat(op, p);
                }
                0x1a | 0x1b => {
                    let c = r.take(6)?;
                    let rgb = [c[0], c[2], c[4]];
                    if op == 0x1a {
                        self.qd.fg = rgb;
                    } else {
                        self.qd.bg = rgb;
                    }
                }
                op if quickdraw::State::handles(op) => match self.qd.play(&mut r, op)? {
                    quickdraw::Drawn::Nothing => {}
                    _ if self.fallback_next => {}
                    quickdraw::Drawn::Ops(ops) => {
                        self.shapes += 1;
                        if self.draw {
                            self.ops.extend(ops);
                            // Bound the pending display list's memory.
                            if self.ops.len() > 1 << 16 {
                                self.flush();
                            }
                        }
                    }
                    quickdraw::Drawn::Invert(path) => {
                        self.shapes += 1;
                        if self.draw {
                            self.invert(path);
                        }
                    }
                },
                0x8200 => {
                    let len = r.u32()? as usize;
                    self.quicktime(&mut r, len)?;
                }
                _ => {
                    let n = data_len(&mut r, op, self.v2)?;
                    r.skip(n)?;
                }
            }
        }
        Err(Error::Malformed("too many opcodes"))
    }
}

/// Source-over of straight RGBA `s` onto straight RGBA `d`.
#[inline]
fn over(d: &mut [u8], s: &[u8]) {
    let sa = u32::from(s[3]);
    if sa == 255 || d[3] == 0 {
        d.copy_from_slice(s);
        return;
    }
    if sa == 0 {
        return;
    }
    let da = u32::from(d[3]) * (255 - sa) / 255;
    let oa = sa + da;
    for c in 0..3 {
        d[c] = ((u32::from(s[c]) * sa + u32::from(d[c]) * da + oa / 2) / oa) as u8;
    }
    d[3] = oa as u8;
}

/// Bytes of data following `op` (reading any length field first), for the opcodes that are
/// skipped.
fn data_len(r: &mut Reader, op: u16, v2: bool) -> Result<usize> {
    let word = |r: &mut Reader| r.u16().map(usize::from);
    let long = |r: &mut Reader| r.u32().map(|v| v as usize);
    Ok(match op {
        0x00 | 0x17..=0x19 | 0x1c | 0x1e | 0x38..=0x3f | 0x48..=0x4f | 0x58..=0x5f => 0,
        0x78..=0x7f | 0x88..=0x8f | 0xb0..=0xcf => 0,
        // Region (clip, frame/paint/erase/invert/fill region) and polygon: a size word
        // counting itself.
        0x01 | 0x70..=0x77 | 0x80..=0x87 => word(r)?.checked_sub(2).ok_or(SHORT)?,
        0x04 => 1,
        0x03 | 0x05 | 0x08 | 0x0d | 0x15 | 0x16 | 0x23 | 0xa0 => 2,
        0x06 | 0x07 | 0x0b | 0x0c | 0x0e | 0x0f | 0x21 | 0x68..=0x6f => 4,
        0x1d | 0x1f | 0x22 => 6,
        0x02 | 0x09 | 0x0a | 0x10 | 0x20 | 0x30..=0x37 | 0x40..=0x47 | 0x50..=0x57 => 8,
        0x60..=0x67 => 12,
        0x11 => {
            if v2 {
                2
            } else {
                1
            }
        }
        // Text: LongText (point, count, text), DHText / DVText (delta, count, text),
        // DHDVText (two deltas, count, text).
        0x28 => {
            r.skip(4)?;
            usize::from(r.u8()?)
        }
        0x29 | 0x2a => {
            r.skip(1)?;
            usize::from(r.u8()?)
        }
        0x2b => {
            r.skip(2)?;
            usize::from(r.u8()?)
        }
        0x24..=0x27 | 0x2c..=0x2f | 0x92..=0x97 | 0x9c..=0x9f | 0xa2..=0xaf => word(r)?,
        0xa1 => {
            r.skip(2)?;
            word(r)?
        }
        0xd0..=0xfe | 0x8100..=0xffff => long(r)?,
        0x0100..=0x7fff => usize::from(op >> 8) * 2,
        0x8000..=0x80ff => 0,
        _ => return Err(Error::Malformed("opcode")),
    })
}

fn walk(data: &[u8], draw: bool) -> Result<Walker<'_>> {
    let (base, version) = start(data).ok_or(Error::NotPict)?;
    let (canvas, _) = canvas_of(data, base, version);
    let area = canvas.width() as u64 * canvas.height() as u64;
    if area > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let mut w = Walker {
        data,
        base,
        v2: version == 2,
        canvas,
        draw,
        pixels: Vec::new(),
        qd: quickdraw::State::new(canvas),
        ops: Vec::new(),
        bitmaps: 0,
        jpegs: 0,
        quicktime: 0,
        shapes: 0,
        alpha: false,
        qt: Vec::new(),
        unsupported_codec: None,
        unsupported_why: None,
        fallback_next: false,
        broken_qt: false,
        truncated: false,
        work: 0,
        budget: area.saturating_mul(OVERDRAW).max(1 << 20),
    };
    match w.run() {
        Ok(()) => {}
        Err(_) if w.drawn() => w.truncated = true,
        Err(e) => return Err(e),
    }
    w.flush();
    if !w.drawn() {
        if let Some(why) = w.unsupported_why.take() {
            return Err(Error::Unsupported(why));
        }
        return Err(match w.unsupported_codec {
            None if w.broken_qt => Error::Malformed("QuickTime image data"),
            Some(c) => Error::Unsupported(format!(
                "QuickTime '{}' compressed picture",
                String::from_utf8_lossy(&c).trim_end()
            )),
            None => Error::Unsupported("the picture draws nothing".into()),
        });
    }
    Ok(w)
}

/// Draws the picture's bitmaps onto a white canvas.
pub fn decode(data: &[u8]) -> Result<Image> {
    let mut w = walk(data, true)?;
    // Bitmaps with an empty source or destination count as drawn without touching it.
    w.ensure_canvas();
    Ok(Image {
        width: w.canvas.width() as u32,
        height: w.canvas.height() as u32,
        pixels: w.pixels,
        truncated: w.truncated,
        bitmaps: w.bitmaps,
        jpegs: w.jpegs,
        quicktime: w.quicktime,
        shapes: w.shapes,
        alpha: w.alpha,
    })
}

/// What a picture draws, found by walking its opcodes without drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scan {
    /// Bitmap opcodes (not counting QuickTime's fallback bitmaps).
    pub bitmaps: usize,
    /// QuickTime JPEG images.
    pub jpegs: usize,
    /// QuickTime images in other codecs (PNG, TIFF, BMP, raw).
    pub quicktime: usize,
    /// QuickDraw shapes, lines and text strings.
    pub shapes: usize,
    /// See [`Image::alpha`].
    pub alpha: bool,
    /// The opcodes end early or turn corrupt after something was drawn.
    pub truncated: bool,
    /// See [`sole_jpeg`].
    pub sole_jpeg: Option<Range<usize>>,
}

/// Walks the opcodes without drawing: the errors [`decode`] would give for a picture with
/// nothing to show (vector-only, another QuickTime codec), else what it draws.
pub fn scan(data: &[u8]) -> Result<Scan> {
    let w = walk(data, false)?;
    Ok(Scan {
        bitmaps: w.bitmaps,
        jpegs: w.jpegs,
        quicktime: w.quicktime,
        shapes: w.shapes,
        alpha: w.alpha,
        truncated: w.truncated,
        sole_jpeg: sole_jpeg_of(&w),
    })
}

/// When the picture is exactly one QuickTime JPEG covering the whole canvas at its own
/// size (the common "PICT wrapping a JPEG" case), the byte range of that JPEG, so a caller
/// can decode it directly (scaled, by regions) instead of through [`decode`].
pub fn sole_jpeg(data: &[u8]) -> Option<Range<usize>> {
    sole_jpeg_of(&walk(data, false).ok()?)
}

fn sole_jpeg_of(w: &Walker) -> Option<Range<usize>> {
    let data = w.data;
    let [q] = &w.qt[..] else { return None };
    let size = Rect {
        top: 0,
        left: 0,
        bottom: q.height as i32,
        right: q.width as i32,
    };
    let at_origin = Rect {
        top: q.dst.top - w.canvas.top,
        left: q.dst.left - w.canvas.left,
        bottom: q.dst.bottom - w.canvas.top,
        right: q.dst.right - w.canvas.left,
    };
    let fits = w.bitmaps + w.quicktime + w.shapes == 0
        && !w.truncated
        && &q.codec == b"jpeg"
        && q.src == size
        && at_origin == size
        && w.canvas.width() as u32 == q.width
        && w.canvas.height() as u32 == q.height;
    let jpeg = &data[q.data.clone()];
    let i = specular_jpeg::read_info(jpeg).ok()?;
    (fits && (i.width, i.height) == (q.width, q.height)).then(|| q.data.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packbits_units() {
        let mut out = [0u8; 6];
        unpack_bits(&[1, 7, 8, 0xfe, 9], &mut out, 1);
        assert_eq!(out, [7, 8, 9, 9, 9, 0]);
        let mut out = [0u8; 6];
        unpack_bits(&[0xff, 1, 2, 0, 3, 4], &mut out, 2);
        assert_eq!(out, [1, 2, 1, 2, 3, 4]);
        // Overlong runs and literals are cut, never overrun.
        let mut out = [0u8; 3];
        unpack_bits(&[0x81, 5, 0x7f, 1], &mut out, 1);
        assert_eq!(out, [5; 3]);
    }

    #[test]
    fn probe_needs_the_signature() {
        let mut v2 = vec![0u8; 10];
        v2[6..10].copy_from_slice(&[0, 4, 0, 4]);
        v2.extend_from_slice(&V2_SIG);
        assert!(probe(&v2));
        assert!(!probe(&[]));
        let mut with_header = vec![0u8; 512];
        with_header.extend_from_slice(&v2);
        assert!(!probe(&with_header[..64]));
        assert!(probe_file(&with_header));
        assert!(plausible(&with_header));
        assert_eq!(info(&with_header).unwrap().width, 4);
    }
}

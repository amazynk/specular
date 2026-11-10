//! Overlays (PS3.3 C.9, groups 6000–601E) and display shutters (C.7.6.11, C.7.6.15): what a
//! viewer draws over the windowed image whatever the window. Both become a per-pixel
//! [`Graphics`] mask of a frame: overlay bits from Overlay Data (60xx,3000) — or, in older
//! files, from an unused high bit of the native pixel cells — placed at the Overlay Origin
//! and matched to frames through Number of Frames in Overlay / Image Frame Origin; shutters
//! as the union of the rectangular, circular and polygonal shapes' outsides (and a bitmap
//! shutter's set bits), shown at the Shutter Presentation Value (black by default) or the
//! Shutter Presentation Color.
//!
//! A presentation state ([`presented`]) replaces the image's shutters with its own and shows
//! only the overlays it activates (Overlay Activation Layer, 60xx,1001): its own overlay of
//! that group, else the image's, each in its graphic layer's colour or grey.

use crate::dict::Vr;
use crate::gsps::{PresentationState, cielab_to_srgb};
use crate::parse::{DataSet, Element, Values};
use crate::pixel::PixelInfo;

/// Mask bit: the pixel is outside a display shutter.
pub const SHUTTER: u8 = 1;
/// Mask bits: the overlay ink of the pixel, `(m & INK) >> 1` (0: no overlay), indexing
/// [`Graphics::inks`] from 1.
pub const INK: u8 = 0b1_1110;
/// Mask value of the first ink (the only one outside presentation states).
pub const OVERLAY: u8 = 2;
/// Most overlay inks a frame's mask holds.
pub const MAX_INKS: usize = 15;

/// One overlay plane on one frame: a bit per frame pixel, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FramePlane {
    /// Overlay group (0x6000..=0x601E).
    pub group: u16,
    words: Vec<u64>,
}

impl FramePlane {
    /// Whether pixel `i` (row-major) is set.
    pub fn get(&self, i: usize) -> bool {
        self.words
            .get(i / 64)
            .is_some_and(|w| w >> (i % 64) & 1 == 1)
    }

    fn bytes(&self) -> usize {
        self.words.len() * 8
    }
}

/// Overlays and shutters of one frame, as a mask over its pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graphics {
    pub width: u32,
    pub height: u32,
    /// [`SHUTTER`] | ink << 1 per pixel, row-major.
    pub mask: Vec<u8>,
    /// The grey shown outside the shutter, 0..=65535 (a P-value, after any inversion).
    pub shutter_grey: u16,
    /// The colour shown outside the shutter, sRGB (Shutter Presentation Color CIELab
    /// Value), in place of the grey where colour can be shown.
    pub shutter_rgb: Option<[u8; 3]>,
    /// sRGB of overlay inks 1.. (white for an image's own overlays; a presentation state's
    /// graphic layer colour or grey).
    pub inks: Vec<[u8; 3]>,
    /// Number of overlay planes on this frame (0: shutters only).
    pub overlays: u32,
    /// The frame has a shutter.
    pub shutter: bool,
    /// The image's own overlay planes on the frame, for presentation states to activate.
    pub(crate) planes: Vec<FramePlane>,
}

impl Graphics {
    /// The mask bits at (x, y).
    pub fn at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.mask[y as usize * self.width as usize + x as usize]
    }

    /// The sRGB ink of mask bits `m`, when an overlay sets the pixel.
    pub fn ink(&self, m: u8) -> Option<[u8; 3]> {
        let i = ((m & INK) >> 1) as usize;
        (i != 0).then(|| self.inks.get(i - 1).copied().unwrap_or([255; 3]))
    }

    pub fn bytes(&self) -> usize {
        self.mask.len() + self.planes.iter().map(FramePlane::bytes).sum::<usize>()
    }
}

/// The `i`th integer of `tag` (binary US / SS, or IS text; implicit-VR overlay elements come
/// as UN and are read as 16-bit words, signed when `signed`).
fn word(ds: &DataSet, v: &Values, tag: u32, i: usize, signed: bool) -> Option<i64> {
    let e = ds.get(tag)?;
    match e.vr {
        Vr::IS | Vr::DS => v.number(e, i).map(|x| x as i64),
        _ => v.u16s(e).nth(i).map(|w| {
            if signed {
                i64::from(w as i16)
            } else {
                i64::from(w)
            }
        }),
    }
}

/// The bits of one overlay plane.
enum Bits<'a> {
    /// Packed one bit per pixel, little-endian bit order (60xx,3000). `swap` for OW in a
    /// big-endian data set (the bytes of each 16-bit word are reversed).
    Packed { data: &'a [u8], swap: bool },
    /// Bit `bit` of each native 16-bit pixel cell (retired; PS3.5 8.1.2), `big` for big
    /// endian.
    Embedded { data: &'a [u8], bit: u16, big: bool },
}

impl Bits<'_> {
    fn get(&self, i: u64) -> bool {
        match self {
            Bits::Packed { data, swap } => {
                let mut byte = (i / 8) as usize;
                if *swap {
                    byte ^= 1;
                }
                data.get(byte).is_some_and(|b| b >> (i % 8) & 1 == 1)
            }
            Bits::Embedded { data, bit, big } => {
                let o = i as usize * 2;
                let Some(c) = data.get(o..o + 2) else {
                    return false;
                };
                let w = if *big {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                };
                w >> bit & 1 == 1
            }
        }
    }
}

/// One overlay plane.
struct Plane<'a> {
    group: u16,
    rows: u32,
    cols: u32,
    /// 0-based image row / column of the plane's first pixel.
    origin: (i64, i64),
    /// Image frames the plane's frames cover: first (0-based) and count.
    first_frame: u32,
    frames: u32,
    bits: Bits<'a>,
}

impl Plane<'_> {
    /// Whether the plane has a frame on image frame `frame`.
    fn covers(&self, frame: u32) -> bool {
        frame >= self.first_frame && frame - self.first_frame < self.frames
    }

    /// The plane on image frame `frame` (which it [covers](Plane::covers)), on a
    /// `width` × `height` frame.
    fn raster(&self, frame: u32, width: u32, height: u32) -> FramePlane {
        let (w, h) = (width as usize, height as usize);
        let mut words = vec![0u64; (w * h).div_ceil(64)];
        let base = match self.bits {
            Bits::Packed { .. } => {
                u64::from(frame - self.first_frame) * self.rows as u64 * self.cols as u64
            }
            // Embedded bits live in the image's own frame.
            Bits::Embedded { .. } => u64::from(frame) * self.rows as u64 * self.cols as u64,
        };
        for oy in 0..self.rows {
            let y = self.origin.0 + i64::from(oy);
            if y < 0 || y >= h as i64 {
                continue;
            }
            for ox in 0..self.cols {
                let x = self.origin.1 + i64::from(ox);
                if x < 0 || x >= w as i64 {
                    continue;
                }
                if self
                    .bits
                    .get(base + u64::from(oy) * u64::from(self.cols) + u64::from(ox))
                {
                    let i = y as usize * w + x as usize;
                    words[i / 64] |= 1 << (i % 64);
                }
            }
        }
        FramePlane {
            group: self.group,
            words,
        }
    }
}

/// The overlay planes of a data set. Planes embedded in the pixel cells need the image's
/// `info` and Pixel Data.
fn planes<'a>(
    ds: &DataSet,
    v: &Values<'a>,
    info: Option<&PixelInfo>,
    pixel_data: Option<&Element>,
) -> Vec<Plane<'a>> {
    let mut out = Vec::new();
    for g in (0x6000u32..=0x601E).step_by(2) {
        let t = |e: u32| g << 16 | e;
        let (Some(rows), Some(cols)) = (
            word(ds, v, t(0x0010), 0, false),
            word(ds, v, t(0x0011), 0, false),
        ) else {
            continue;
        };
        if rows <= 0 || cols <= 0 {
            continue;
        }
        let origin = (
            word(ds, v, t(0x0050), 0, true).unwrap_or(1) - 1,
            word(ds, v, t(0x0050), 1, true).unwrap_or(1) - 1,
        );
        let frames = word(ds, v, t(0x0015), 0, false).unwrap_or(1).max(1) as u32;
        let first_frame = word(ds, v, t(0x0051), 0, false).unwrap_or(1).max(1) as u32 - 1;
        let bits = if let Some(e) = ds.get(t(0x3000)) {
            let data = v.bytes(e);
            if (data.len() as u64) * 8 < rows as u64 * cols as u64 {
                continue;
            }
            Bits::Packed {
                data,
                swap: v.big && e.vr == Vr::OW,
            }
        } else {
            // Retired: the overlay in an unused bit of the pixel cells.
            let bit = word(ds, v, t(0x0102), 0, false).unwrap_or(-1);
            let allocated = word(ds, v, t(0x0100), 0, false).unwrap_or(0);
            let (Some(info), Some(pd)) = (info, pixel_data.filter(|e| e.fragments.is_none()))
            else {
                continue;
            };
            if allocated != 16
                || info.bits_allocated != 16
                || info.samples != 1
                || !(0..16).contains(&bit)
                || bit as u16 <= info.high_bit && bit as u16 + info.bits_stored > info.high_bit
                || rows != i64::from(info.rows)
                || cols != i64::from(info.columns)
            {
                continue;
            }
            Bits::Embedded {
                data: v.bytes(pd),
                bit: bit as u16,
                big: v.big,
            }
        };
        // Embedded bits are in every frame's own cells.
        let (first_frame, frames) = match bits {
            Bits::Packed { .. } => (first_frame, frames),
            Bits::Embedded { .. } => (0, u32::MAX),
        };
        out.push(Plane {
            group: g as u16,
            rows: rows as u32,
            cols: cols as u32,
            origin,
            first_frame,
            frames,
            bits,
        });
    }
    out
}

/// The planes on image frame `frame`, in order, on a `width` × `height` frame. The planes
/// embedded in the pixel cells (retired; up to 16 of them) are read in one pass over the cells
/// rather than one pass each.
fn rasters(planes: &[Plane], frame: u32, width: u32, height: u32) -> Vec<FramePlane> {
    let covering: Vec<&Plane> = planes.iter().filter(|p| p.covers(frame)).collect();
    let embedded: Vec<&Plane> = covering
        .iter()
        .copied()
        .filter(|p| matches!(p.bits, Bits::Embedded { .. }))
        .collect();
    let mut shared = embedded_rasters(&embedded, frame, width, height).into_iter();
    covering
        .iter()
        .filter_map(|p| match p.bits {
            Bits::Embedded { .. } => shared.next(),
            Bits::Packed { .. } => Some(p.raster(frame, width, height)),
        })
        .collect()
}

/// [`Plane::raster`] of each of `planes` (all embedded in the same cells, each the image's
/// size), reading each cell once.
fn embedded_rasters(planes: &[&Plane], frame: u32, width: u32, height: u32) -> Vec<FramePlane> {
    let Some(first) = planes.first() else {
        return Vec::new();
    };
    let Bits::Embedded { data, big, .. } = first.bits else {
        return Vec::new();
    };
    let one_pass = planes.iter().all(|p| {
        matches!(p.bits, Bits::Embedded { data: d, big: b, .. }
            if std::ptr::eq(d, data) && b == big)
            && (p.rows, p.cols) == (first.rows, first.cols)
    });
    if !one_pass {
        return planes
            .iter()
            .map(|p| p.raster(frame, width, height))
            .collect();
    }
    let bits: Vec<u16> = planes
        .iter()
        .map(|p| match p.bits {
            Bits::Embedded { bit, .. } => bit,
            Bits::Packed { .. } => 0,
        })
        .collect();
    let any = bits.iter().fold(0u16, |m, &b| m | 1 << b);
    let (w, h) = (width as usize, height as usize);
    let mut words = vec![vec![0u64; (w * h).div_ceil(64)]; planes.len()];
    let (rows, cols) = (first.rows, first.cols);
    let base = u64::from(frame) * rows as u64 * cols as u64;
    for oy in 0..rows {
        for ox in 0..cols {
            let o = (base + u64::from(oy) * u64::from(cols) + u64::from(ox)) as usize * 2;
            let Some(c) = data.get(o..o + 2) else {
                continue;
            };
            let cell = if big {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            };
            if cell & any == 0 {
                continue;
            }
            for ((p, &bit), words) in planes.iter().zip(&bits).zip(words.iter_mut()) {
                if cell >> bit & 1 == 0 {
                    continue;
                }
                let y = p.origin.0 + i64::from(oy);
                let x = p.origin.1 + i64::from(ox);
                if y < 0 || y >= h as i64 || x < 0 || x >= w as i64 {
                    continue;
                }
                let i = y as usize * w + x as usize;
                words[i / 64] |= 1 << (i % 64);
            }
        }
    }
    planes
        .iter()
        .zip(words)
        .map(|(p, words)| FramePlane {
            group: p.group,
            words,
        })
        .collect()
}

/// An overlay plane a presentation state carries (Overlay Plane Module of the state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayPlane {
    /// Overlay group (0x6000..=0x601E).
    pub group: u16,
    pub rows: u32,
    pub cols: u32,
    /// 0-based image row / column of the plane's first pixel.
    pub origin: (i64, i64),
    /// Image frames the plane's frames cover: first (0-based) and count. A single-frame
    /// plane applies to every frame the state applies to.
    pub first_frame: u32,
    pub frames: u32,
    /// One bit per pixel, little-endian bit order.
    pub data: Vec<u8>,
}

impl OverlayPlane {
    fn plane(&self) -> Plane<'_> {
        Plane {
            group: self.group,
            rows: self.rows,
            cols: self.cols,
            origin: self.origin,
            first_frame: self.first_frame,
            frames: self.frames,
            bits: Bits::Packed {
                data: &self.data,
                swap: false,
            },
        }
    }

    /// The plane on image frame `frame`, if it has one there.
    fn raster(&self, frame: u32, width: u32, height: u32) -> Option<FramePlane> {
        let p = self.plane();
        if self.frames <= 1 {
            Some(p.raster(self.first_frame, width, height))
        } else {
            p.covers(frame).then(|| p.raster(frame, width, height))
        }
    }
}

/// The packed overlay planes of a presentation state's data set, owned.
pub(crate) fn owned_planes(ds: &DataSet, v: &Values) -> Vec<OverlayPlane> {
    planes(ds, v, None, None)
        .into_iter()
        .filter_map(|p| {
            let Bits::Packed { data, swap } = p.bits else {
                return None;
            };
            let n = (p.rows as usize * p.cols as usize * p.frames as usize).div_ceil(8);
            let mut data = data[..n.min(data.len())].to_vec();
            if swap {
                for w in data.chunks_exact_mut(2) {
                    w.swap(0, 1);
                }
            }
            Some(OverlayPlane {
                group: p.group,
                rows: p.rows,
                cols: p.cols,
                origin: p.origin,
                first_frame: p.first_frame,
                frames: p.frames,
                data,
            })
        })
        .collect()
}

/// A display shutter shape (coordinates 1-based, as stored).
#[derive(Clone, Debug, PartialEq)]
pub enum ShutterShape {
    /// left, right columns; upper, lower rows (inclusive).
    Rect { l: f64, r: f64, u: f64, d: f64 },
    /// centre (row, column) and radius.
    Circle { row: f64, col: f64, radius: f64 },
    /// vertices (row, column).
    Polygon(Vec<(f64, f64)>),
}

impl ShutterShape {
    /// Whether 1-based pixel (row, col) is inside (shown).
    pub fn inside(&self, row: f64, col: f64) -> bool {
        match self {
            ShutterShape::Rect { l, r, u, d } => col >= *l && col <= *r && row >= *u && row <= *d,
            ShutterShape::Circle {
                row: cr,
                col: cc,
                radius,
            } => (row - cr).powi(2) + (col - cc).powi(2) <= radius * radius,
            ShutterShape::Polygon(p) => {
                // Even-odd crossing test at the pixel centre.
                let mut inside = false;
                let n = p.len();
                for i in 0..n {
                    let (ri, ci) = p[i];
                    let (rj, cj) = p[(i + n - 1) % n];
                    if (ri > row) != (rj > row) && col < (cj - ci) * (row - ri) / (rj - ri) + ci {
                        inside = !inside;
                    }
                }
                inside
            }
        }
    }
}

/// A data set's display shutter: Display Shutter and Bitmap Display Shutter modules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shutter {
    pub shapes: Vec<ShutterShape>,
    /// Shutter Overlay Group of a bitmap shutter: the set bits of the state's overlay of
    /// that group are shuttered.
    pub bitmap: Option<u16>,
    /// Shutter Presentation Value, a P-value 0..=65535 (0 when absent).
    pub grey: u16,
    /// Shutter Presentation Color CIELab Value, as sRGB.
    pub colour: Option<[u8; 3]>,
}

impl Shutter {
    pub(crate) fn read(ds: &DataSet, v: &Values) -> Option<Shutter> {
        let e = ds.get(0x0018_1600)?;
        let num = |tag: u32, i: usize| ds.get(tag).and_then(|e| v.number(e, i));
        let mut out = Shutter::default();
        for k in 0..v.count(e).min(4) {
            match v.text_n(e, k).map(str::trim) {
                Some("RECTANGULAR") => {
                    if let (Some(l), Some(r), Some(u), Some(d)) = (
                        num(0x0018_1602, 0),
                        num(0x0018_1604, 0),
                        num(0x0018_1606, 0),
                        num(0x0018_1608, 0),
                    ) {
                        out.shapes.push(ShutterShape::Rect { l, r, u, d });
                    }
                }
                Some("CIRCULAR") => {
                    if let (Some(row), Some(col), Some(radius)) = (
                        num(0x0018_1610, 0),
                        num(0x0018_1610, 1),
                        num(0x0018_1612, 0),
                    ) {
                        out.shapes.push(ShutterShape::Circle { row, col, radius });
                    }
                }
                Some("POLYGONAL") => {
                    let Some(pe) = ds.get(0x0018_1620) else {
                        continue;
                    };
                    let n = v.count(pe).min(2 * 4096) / 2;
                    let p: Vec<_> = (0..n)
                        .filter_map(|i| Some((v.number(pe, 2 * i)?, v.number(pe, 2 * i + 1)?)))
                        .collect();
                    if p.len() >= 3 {
                        out.shapes.push(ShutterShape::Polygon(p));
                    }
                }
                Some("BITMAP") => {
                    out.bitmap = word(ds, v, 0x0018_1623, 0, false)
                        .map(|g| g as u16)
                        .filter(|g| (0x6000..=0x601E).contains(g) && g % 2 == 0);
                }
                _ => {}
            }
        }
        if out.shapes.is_empty() && out.bitmap.is_none() {
            return None;
        }
        out.grey = ds
            .get(0x0018_1622)
            .and_then(|e| v.int(e))
            .map_or(0, |g| g.clamp(0, 0xFFFF) as u16);
        out.colour = ds.get(0x0018_1624).and_then(|e| {
            let lab: Vec<u16> = v.u16s(e).collect();
            match lab[..] {
                [l, a, b] => Some(cielab_to_srgb(l, a, b)),
                _ => None,
            }
        });
        Some(out)
    }
}

/// The mask of a frame: shutter (outside any shape, or set in `bitmap`) and overlays with
/// their inks (later ones win).
fn compose(
    width: u32,
    height: u32,
    shapes: &[ShutterShape],
    bitmap: Option<&FramePlane>,
    overlays: &[(&FramePlane, u8)],
) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut mask = vec![0u8; w * h];
    if !shapes.is_empty() {
        for (y, row) in mask.chunks_exact_mut(w).enumerate() {
            let r = (y + 1) as f64;
            for (x, m) in row.iter_mut().enumerate() {
                let c = (x + 1) as f64;
                if !shapes.iter().all(|s| s.inside(r, c)) {
                    *m |= SHUTTER;
                }
            }
        }
    }
    if let Some(b) = bitmap {
        for (i, m) in mask.iter_mut().enumerate() {
            if b.get(i) {
                *m |= SHUTTER;
            }
        }
    }
    for (p, ink) in overlays {
        let bits = (ink << 1) & INK;
        for (i, m) in mask.iter_mut().enumerate() {
            if p.get(i) {
                *m = (*m & !INK) | bits;
            }
        }
    }
    mask
}

/// The overlays and shutters of frame `frame` (0-based), or `None` when it has neither.
pub(crate) fn read(
    ds: &DataSet,
    v: &Values,
    info: &PixelInfo,
    frame: u32,
    width: u32,
    height: u32,
) -> Option<Graphics> {
    let pixel_data = ds.get(crate::parse::PIXEL_DATA);
    let planes = rasters(&planes(ds, v, Some(info), pixel_data), frame, width, height);
    // An image's bitmap shutter would name an overlay of a presentation state: none here.
    let shutter = Shutter::read(ds, v).filter(|s| !s.shapes.is_empty());
    if planes.is_empty() && shutter.is_none() {
        return None;
    }
    let shapes = shutter.as_ref().map_or(&[][..], |s| &s.shapes[..]);
    let overlays: Vec<_> = planes.iter().map(|p| (p, 1)).collect();
    let mask = compose(width, height, shapes, None, &overlays);
    Some(Graphics {
        width,
        height,
        mask,
        shutter_grey: shutter.as_ref().map_or(0, |s| s.grey),
        shutter_rgb: shutter.as_ref().and_then(|s| s.colour),
        inks: if planes.is_empty() {
            Vec::new()
        } else {
            vec![[255; 3]]
        },
        overlays: planes.len() as u32,
        shutter: shutter.is_some(),
        planes,
    })
}

/// The graphics of frame `frame` of image `sop` under presentation state `ps`: the state's
/// shutter (the image's own is not shown) and the overlays it activates — its own plane of
/// the group, else the image's (`image`) — in their layers' order, inked in the layer's
/// colour or grey (white without one). Overlays that are not activated are not shown.
pub(crate) fn presented(
    image: Option<&Graphics>,
    ps: &PresentationState,
    frame: u32,
    width: u32,
    height: u32,
) -> Option<Graphics> {
    let own: Vec<FramePlane> = ps
        .overlays
        .iter()
        .filter_map(|o| o.raster(frame, width, height))
        .collect();
    let image_planes = image.map_or(&[][..], |g| &g.planes[..]);
    let mut active: Vec<(i64, &FramePlane, [u8; 3])> = ps
        .activations
        .iter()
        .filter_map(|(group, layer)| {
            let plane = own
                .iter()
                .find(|p| p.group == *group)
                .or_else(|| image_planes.iter().find(|p| p.group == *group))?;
            let l = ps.layer(layer);
            Some((
                l.map_or(i64::MAX, |l| l.order),
                plane,
                l.and_then(|l| l.srgb()).unwrap_or([255; 3]),
            ))
        })
        .collect();
    active.sort_by_key(|a| a.0);
    let shutter = ps.shutter.as_ref();
    let bitmap = shutter
        .and_then(|s| s.bitmap)
        .and_then(|g| own.iter().find(|p| p.group == g));
    let shapes = shutter.map_or(&[][..], |s| &s.shapes[..]);
    if active.is_empty() && shapes.is_empty() && bitmap.is_none() {
        return None;
    }
    let mut inks: Vec<[u8; 3]> = Vec::new();
    let overlays: Vec<(&FramePlane, u8)> = active
        .iter()
        .map(|(_, p, rgb)| {
            let i = match inks.iter().position(|c| c == rgb) {
                Some(i) => i,
                None if inks.len() < MAX_INKS => {
                    inks.push(*rgb);
                    inks.len() - 1
                }
                None => MAX_INKS - 1,
            };
            (*p, i as u8 + 1)
        })
        .collect();
    let mask = compose(width, height, shapes, bitmap, &overlays);
    Some(Graphics {
        width,
        height,
        mask,
        shutter_grey: shutter.map_or(0, |s| s.grey),
        shutter_rgb: shutter.and_then(|s| s.colour),
        inks,
        overlays: overlays.len() as u32,
        shutter: !shapes.is_empty() || bitmap.is_some(),
        planes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_and_circle_contain_their_centres() {
        let tri = ShutterShape::Polygon(vec![(1.0, 1.0), (1.0, 10.0), (10.0, 1.0)]);
        assert!(tri.inside(2.0, 2.0));
        assert!(!tri.inside(9.0, 9.0));
        let c = ShutterShape::Circle {
            row: 5.0,
            col: 5.0,
            radius: 2.0,
        };
        assert!(c.inside(5.0, 7.0));
        assert!(!c.inside(7.0, 7.0));
    }

    #[test]
    fn packed_bits_are_little_endian_within_bytes() {
        let b = Bits::Packed {
            data: &[0b0000_0101, 0x80],
            swap: false,
        };
        assert!(b.get(0) && !b.get(1) && b.get(2) && b.get(15));
        let s = Bits::Packed {
            data: &[0x80, 0b0000_0101],
            swap: true,
        };
        assert!(s.get(0) && s.get(2) && s.get(15));
    }

    #[test]
    fn later_overlays_win_and_inks_index_the_palette() {
        let a = FramePlane {
            group: 0x6000,
            words: vec![0b0011],
        };
        let b = FramePlane {
            group: 0x6002,
            words: vec![0b0110],
        };
        let m = compose(4, 1, &[], None, &[(&a, 1), (&b, 2)]);
        assert_eq!(m, [2, 4, 4, 0]);
        let g = Graphics {
            width: 4,
            height: 1,
            mask: m,
            shutter_grey: 0,
            shutter_rgb: None,
            inks: vec![[1, 2, 3], [4, 5, 6]],
            overlays: 2,
            shutter: false,
            planes: Vec::new(),
        };
        assert_eq!(g.ink(g.at(0, 0)), Some([1, 2, 3]));
        assert_eq!(g.ink(g.at(1, 0) | SHUTTER), Some([4, 5, 6]));
        assert_eq!(g.ink(g.at(3, 0)), None);
    }
}

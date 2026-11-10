//! EMF+ ([MS-EMFPLUS]): the GDI+ records EMF comment records carry.
//!
//! Coordinates go world → page (the page unit and scale, at the header's DPI) → the EMF's
//! device pixels, the same op space the GDI records draw in. In an EMF+ file the GDI records
//! are a fallback for old readers and are not drawn, except those after a GetDC record
//! (the EMF player in `emf.rs` keeps track).

use super::{List, MAX_SAVE, Paint, Pen, arc_to, argb, f32le, i16le, scale_of, u32le};
use crate::mng_dec::{decode_jpeg, decode_png};
use resvg::tiny_skia as sk;
use specular_core::CancelToken;
use std::sync::Arc;

/// EMF+ object ids are one byte.
const OBJECTS: usize = 64;
/// Largest object assembled from continued Object records.
const MAX_OBJECT: usize = 256 << 20;

#[derive(Clone)]
enum PBrush {
    Solid(sk::Color),
    /// Left to right across `rect` (in brush space), then `xf`.
    Linear {
        rect: sk::Rect,
        c0: sk::Color,
        c1: sk::Color,
        xf: sk::Transform,
    },
}

#[derive(Clone)]
struct PPen {
    /// In world units.
    width: f32,
    brush: PBrush,
    cap: sk::LineCap,
    join: sk::LineJoin,
    miter: f32,
    /// In pen widths.
    dash: Option<Vec<f32>>,
}

#[derive(Clone)]
enum PObj {
    Brush(PBrush),
    Pen(PPen),
    Path(sk::Path),
    Image(Arc<sk::Pixmap>),
    Other,
}

#[derive(Clone, Copy)]
struct State {
    world: sk::Transform,
    unit: u32,
    scale: f32,
}

pub(crate) struct Plus {
    /// An EMF+ header was seen.
    pub active: bool,
    /// The last EMF+ record was GetDC: the GDI records that follow draw.
    pub getdc: bool,
    dpi: (f32, f32),
    state: State,
    stack: Vec<(u32, State)>,
    objects: Vec<Option<PObj>>,
    /// A continued object being assembled: (id and type flags, total size, bytes so far).
    pending: Option<(u16, usize, Vec<u8>)>,
    /// The picture in op space (for Clear).
    frame: Option<sk::Rect>,
}

fn rd(d: &[u8], at: usize) -> u32 {
    u32le(d, at).unwrap_or(0)
}

/// `n` points at `at`: 16-bit integers when `compressed`, else floats. `None` when the record
/// is too short.
fn points(d: &[u8], at: usize, n: u32, compressed: bool) -> Option<Vec<(f32, f32)>> {
    let sz = if compressed { 4 } else { 8 };
    let n = n as usize;
    if n > d.len().saturating_sub(at) / sz {
        return None;
    }
    (0..n)
        .map(|k| {
            let o = at + k * sz;
            if compressed {
                Some((i16le(d, o)? as f32, i16le(d, o + 2)? as f32))
            } else {
                Some((f32le(d, o)?, f32le(d, o + 4)?))
            }
        })
        .collect()
}

/// A rectangle (x, y, w, h) at `at`.
fn rect_at(d: &[u8], at: usize, compressed: bool) -> Option<sk::Rect> {
    let p = points(d, at, 2, compressed)?;
    sk::Rect::from_xywh(p[0].0, p[0].1, p[1].0, p[1].1)
        .or_else(|| sk::Rect::from_ltrb(p[0].0 + p[1].0, p[0].1 + p[1].1, p[0].0, p[0].1))
}

fn rect_path(r: sk::Rect) -> sk::Path {
    sk::PathBuilder::from_rect(r)
}

/// The arc of the ellipse in `r` from `start` over `sweep` degrees (clockwise on the page).
fn arc(r: sk::Rect, start: f32, sweep: f32, pie: bool) -> Option<sk::Path> {
    let (cx, cy, rx, ry) = (
        (r.left() + r.right()) / 2.0,
        (r.top() + r.bottom()) / 2.0,
        r.width() / 2.0,
        r.height() / 2.0,
    );
    // Angles are true angles on the page; the curve wants the ellipse parameter.
    let param = |deg: f32| {
        let a = deg.to_radians();
        (rx * a.sin()).atan2(ry * a.cos())
    };
    let sweep = sweep.clamp(-360.0, 360.0);
    let t0 = param(start);
    let mut st = param(start + sweep) - t0;
    if sweep >= 0.0 {
        st = st.rem_euclid(std::f32::consts::TAU);
        if st < 1e-6 && sweep > 1.0 {
            st = std::f32::consts::TAU;
        }
    } else {
        st = -(-st).rem_euclid(std::f32::consts::TAU);
        if st > -1e-6 && sweep < -1.0 {
            st = -std::f32::consts::TAU;
        }
    }
    let mut pb = sk::PathBuilder::new();
    if pie {
        pb.move_to(cx, cy);
        pb.line_to(cx + rx * t0.cos(), cy + ry * t0.sin());
    } else {
        pb.move_to(cx + rx * t0.cos(), cy + ry * t0.sin());
    }
    arc_to(&mut pb, cx, cy, rx, ry, t0, st);
    if pie {
        pb.close();
    }
    pb.finish()
}

fn brush_obj(d: &[u8]) -> Option<(PBrush, usize)> {
    let typ = rd(d, 4);
    Some(match typ {
        0 => (PBrush::Solid(argb(u32le(d, 8)?)), 12),
        1 => {
            // Hatch: the mix of fore- and background.
            let (f, b) = (argb(u32le(d, 12)?), argb(u32le(d, 16)?));
            let mix = |x: f32, y: f32| (x + y) / 2.0;
            let c = sk::Color::from_rgba(
                mix(f.red(), b.red()),
                mix(f.green(), b.green()),
                mix(f.blue(), b.blue()),
                mix(f.alpha(), b.alpha()),
            )?;
            (PBrush::Solid(c), 20)
        }
        3 => (PBrush::Solid(argb(u32le(d, 16)?)), 20),
        4 => {
            let flags = rd(d, 8);
            let r = rect_at(d, 16, false)?;
            let (c0, c1) = (argb(u32le(d, 32)?), argb(u32le(d, 36)?));
            let mut xf = sk::Transform::identity();
            if flags & 2 != 0 {
                let f = |k: usize| f32le(d, 48 + 4 * k);
                xf = sk::Transform::from_row(f(0)?, f(1)?, f(2)?, f(3)?, f(4)?, f(5)?);
            }
            (
                PBrush::Linear {
                    rect: r,
                    c0,
                    c1,
                    xf,
                },
                48,
            )
        }
        // Texture brushes: a neutral grey.
        _ => (PBrush::Solid(sk::Color::from_rgba8(128, 128, 128, 255)), 12),
    })
}

fn pen_obj(d: &[u8]) -> Option<PPen> {
    let flags = rd(d, 8);
    let width = f32le(d, 16)?;
    let mut at = 20usize;
    let mut cap = sk::LineCap::Butt;
    let mut join = sk::LineJoin::Miter;
    let mut miter = 10.0;
    let mut style = 0;
    let mut custom: Option<Vec<f32>> = None;
    let skip_block = |at: &mut usize| -> Option<()> {
        let n = u32le(d, *at)? as usize;
        *at = at.checked_add(4)?.checked_add(n)?;
        Some(())
    };
    if flags & 0x1 != 0 {
        at += 24;
    }
    if flags & 0x2 != 0 {
        cap = match rd(d, at) {
            1 => sk::LineCap::Square,
            2 => sk::LineCap::Round,
            _ => sk::LineCap::Butt,
        };
        at += 4;
    }
    if flags & 0x4 != 0 {
        at += 4;
    }
    if flags & 0x8 != 0 {
        join = match rd(d, at) {
            1 => sk::LineJoin::Bevel,
            2 => sk::LineJoin::Round,
            _ => sk::LineJoin::Miter,
        };
        at += 4;
    }
    if flags & 0x10 != 0 {
        miter = f32le(d, at).unwrap_or(10.0);
        at += 4;
    }
    if flags & 0x20 != 0 {
        style = rd(d, at);
        at += 4;
    }
    if flags & 0x40 != 0 {
        at += 4;
    }
    if flags & 0x80 != 0 {
        at += 4;
    }
    if flags & 0x100 != 0 {
        let n = (rd(d, at) as usize).min(64);
        custom = (0..n).map(|k| f32le(d, at + 4 + 4 * k)).collect();
        at = at.checked_add(4 + 4 * rd(d, at) as usize)?;
    }
    if flags & 0x200 != 0 {
        at += 4;
    }
    if flags & 0x400 != 0 {
        at = at.checked_add(4 + 4 * rd(d, at) as usize)?;
    }
    if flags & 0x800 != 0 {
        skip_block(&mut at)?;
    }
    if flags & 0x1000 != 0 {
        skip_block(&mut at)?;
    }
    let (brush, _) = brush_obj(d.get(at..)?)?;
    let dash = match style {
        1 => Some(vec![3.0, 1.0]),
        2 => Some(vec![1.0, 1.0]),
        3 => Some(vec![3.0, 1.0, 1.0, 1.0]),
        4 => Some(vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
        5 => custom
            .filter(|c| c.len() >= 2 && c.len().is_multiple_of(2) && c.iter().all(|v| *v > 0.0)),
        _ => None,
    };
    Some(PPen {
        width: width.abs(),
        brush,
        cap,
        join,
        miter,
        dash,
    })
}

fn path_obj(d: &[u8]) -> Option<sk::Path> {
    let n = rd(d, 4) as usize;
    let flags = rd(d, 8);
    // Relative and run-length encoded points are not read.
    if flags & 0x1800 != 0 {
        return None;
    }
    let c = flags & 0x4000 != 0;
    let pts = points(d, 12, n as u32, c)?;
    let types = d.get(12 + n * if c { 4 } else { 8 }..)?.get(..n)?;
    let mut pb = sk::PathBuilder::new();
    let mut k = 0;
    while k < n {
        let (x, y) = pts[k];
        match types[k] & 7 {
            0 => pb.move_to(x, y),
            3 if k + 2 < n => {
                let (a, b) = (pts[k + 1], pts[k + 2]);
                pb.cubic_to(x, y, a.0, a.1, b.0, b.1);
                k += 2;
            }
            _ => pb.line_to(x, y),
        }
        if types[k] & 0x80 != 0 {
            pb.close();
        }
        k += 1;
    }
    pb.finish()
}

fn image_obj(d: &[u8], list: &mut List, cancel: &CancelToken) -> Option<Arc<sk::Pixmap>> {
    if rd(d, 4) != 1 {
        // Metafile images are not drawn.
        list.notes.skipped_images += 1;
        return None;
    }
    let (w, h) = (rd(d, 8) as i32, rd(d, 12) as i32);
    let (stride, fmt, kind) = (rd(d, 16) as i32, rd(d, 20), rd(d, 24));
    let data = d.get(28..)?;
    if kind == 1 {
        let img = if data.starts_with(b"\x89PNG") {
            decode_png(data, cancel).ok()
        } else if data.starts_with(&[0xff, 0xd8]) {
            decode_jpeg(data, cancel).ok()
        } else {
            None
        };
        let Some(img) = img else {
            list.notes.skipped_images += 1;
            return None;
        };
        return list.pixmap(img.w, img.h, &img.px);
    }
    let bpp = match fmt {
        0x0026_200a | 0x000e_200b | 0x0002_2009 => 4,
        0x0002_1808 => 3,
        _ => {
            list.notes.skipped_images += 1;
            return None;
        }
    };
    if w <= 0 || h <= 0 || w as u64 * h as u64 > super::MAX_IMAGE_PIXELS {
        list.notes.skipped_images += 1;
        return None;
    }
    let (w, h) = (w as usize, h as usize);
    let row = stride.unsigned_abs() as usize;
    if row < w * bpp || data.len() < row * h {
        list.notes.skipped_images += 1;
        return None;
    }
    let mut px = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let y = if stride < 0 { h - 1 - y } else { y };
        for p in data[y * row..][..w * bpp].chunks_exact(bpp) {
            let (b, g, r) = (p[0], p[1], p[2]);
            let a = if bpp == 4 && fmt != 0x0002_2009 {
                p[3]
            } else {
                255
            };
            let un = |c: u8| {
                if fmt == 0x000e_200b && a != 0 {
                    ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                } else {
                    c
                }
            };
            px.extend_from_slice(&[un(r), un(g), un(b), a]);
        }
    }
    list.pixmap(w as u32, h as u32, &px)
}

impl Plus {
    pub fn new(frame: Option<sk::Rect>) -> Plus {
        Plus {
            active: false,
            getdc: false,
            dpi: (96.0, 96.0),
            state: State {
                world: sk::Transform::identity(),
                unit: 2,
                scale: 1.0,
            },
            stack: Vec::new(),
            objects: vec![None; OBJECTS],
            pending: None,
            frame,
        }
    }

    /// World → op space.
    fn xf(&self) -> sk::Transform {
        let s = self.state.scale;
        let (fx, fy) = match self.state.unit {
            3 => (self.dpi.0 / 72.0, self.dpi.1 / 72.0),
            4 => self.dpi,
            5 => (self.dpi.0 / 300.0, self.dpi.1 / 300.0),
            6 => (self.dpi.0 / 25.4, self.dpi.1 / 25.4),
            _ => (1.0, 1.0),
        };
        sk::Transform::from_scale(fx * s, fy * s).pre_concat(self.state.world)
    }

    fn save(&mut self, idx: u32) {
        if self.stack.len() < MAX_SAVE {
            self.stack.push((idx, self.state));
        }
    }

    fn restore(&mut self, idx: u32) {
        if let Some(k) = self.stack.iter().rposition(|(i, _)| *i == idx) {
            self.state = self.stack[k].1;
            self.stack.truncate(k);
        }
    }

    fn obj(&self, id: u32) -> Option<&PObj> {
        self.objects.get(id as usize & 0xff)?.as_ref()
    }

    /// The brush of a fill record: an ARGB colour when the S flag is set, else an object.
    fn brush(&self, flags: u16, v: u32) -> Option<PBrush> {
        if flags & 0x8000 != 0 {
            return Some(PBrush::Solid(argb(v)));
        }
        match self.obj(v)? {
            PObj::Brush(b) => Some(b.clone()),
            _ => None,
        }
    }

    fn paint(b: &PBrush, xf: sk::Transform) -> Paint {
        match b {
            PBrush::Solid(c) => Paint::Solid(*c),
            PBrush::Linear {
                rect,
                c0,
                c1,
                xf: bx,
            } => {
                let t = xf.pre_concat(*bx);
                let cy = (rect.top() + rect.bottom()) / 2.0;
                let mut p = [
                    sk::Point::from_xy(rect.left(), cy),
                    sk::Point::from_xy(rect.right(), cy),
                ];
                t.map_points(&mut p);
                Paint::Linear {
                    p0: p[0],
                    p1: p[1],
                    c0: *c0,
                    c1: *c1,
                }
            }
        }
    }

    fn fill(&self, list: &mut List, path: Option<sk::Path>, brush: Option<PBrush>) {
        let xf = self.xf();
        if let (Some(p), Some(b)) = (path.and_then(|p| p.transform(xf)), brush) {
            list.fill(p, Self::paint(&b, xf), true);
        }
    }

    fn draw(&self, list: &mut List, path: Option<sk::Path>, pen: u32) {
        let Some(PObj::Pen(pen)) = self.obj(pen) else {
            return;
        };
        let xf = self.xf();
        let Some(p) = path.and_then(|p| p.transform(xf)) else {
            return;
        };
        let width = (pen.width * scale_of(&xf)).max(1.0);
        let color = match &pen.brush {
            PBrush::Solid(c) => *c,
            PBrush::Linear { c0, .. } => *c0,
        };
        list.stroke(
            p,
            Pen {
                color,
                width,
                cap: pen.cap,
                join: pen.join,
                miter: pen.miter,
                dash: pen
                    .dash
                    .as_ref()
                    .map(|d| d.iter().map(|v| v * width).collect()),
            },
        );
    }

    fn object(&mut self, flags: u16, d: &[u8], list: &mut List, cancel: &CancelToken) {
        let id = (flags & 0xff) as usize;
        if id >= OBJECTS {
            return;
        }
        let obj = match (flags >> 8) & 0x7f {
            1 => brush_obj(d).map(|(b, _)| PObj::Brush(b)),
            2 => pen_obj(d).map(PObj::Pen),
            3 => path_obj(d).map(PObj::Path),
            5 => image_obj(d, list, cancel).map(PObj::Image),
            _ => Some(PObj::Other),
        };
        self.objects[id] = obj;
    }

    /// Play the EMF+ records of one comment record (after its "EMF+" tag).
    pub fn play(&mut self, data: &[u8], list: &mut List, cancel: &CancelToken) {
        let mut at = 0usize;
        while let (Some(typ), Some(size), Some(dsize)) = (
            super::u16le(data, at),
            u32le(data, at + 4),
            u32le(data, at + 8),
        ) {
            let flags = super::u16le(data, at + 2).unwrap_or(0);
            let size = size as usize;
            let Some(rec) = data.get(at..at.saturating_add(size)).filter(|_| size >= 12) else {
                return;
            };
            at += size;
            if cancel.is_cancelled() {
                return;
            }
            let d = &rec[12..12 + (dsize as usize).min(size - 12)];
            if typ != 0x4004 {
                self.getdc = false;
            }
            self.record(typ, flags, d, list, cancel);
        }
    }

    fn record(&mut self, typ: u16, flags: u16, d: &[u8], list: &mut List, cancel: &CancelToken) {
        let c = flags & 0x4000 != 0;
        let pen = (flags & 0xff) as u32;
        match typ {
            // Header
            0x4001 => {
                self.active = true;
                list.notes.plus = Some(flags & 1 != 0);
                let (x, y) = (rd(d, 8) as f32, rd(d, 12) as f32);
                if x > 0.0 && y > 0.0 && x < 1e5 && y < 1e5 {
                    self.dpi = (x, y);
                }
            }
            0x4004 => self.getdc = true,
            // Object, possibly continued over several records.
            0x4008 => {
                let key = flags & 0x7fff;
                if flags & 0x8000 != 0 {
                    let total = rd(d, 0) as usize;
                    let chunk = d.get(4..).unwrap_or(&[]);
                    let pending = match self.pending.take() {
                        Some((k, t, buf)) if k == key => (k, t, buf),
                        _ => (key, total.min(MAX_OBJECT), Vec::new()),
                    };
                    let (k, t, mut buf) = pending;
                    buf.extend_from_slice(&chunk[..chunk.len().min(t.saturating_sub(buf.len()))]);
                    if buf.len() >= t {
                        self.object(k, &buf, list, cancel);
                    } else {
                        self.pending = Some((k, t, buf));
                    }
                } else if let Some((k, t, mut buf)) = self.pending.take().filter(|p| p.0 == key) {
                    buf.extend_from_slice(&d[..d.len().min(t.saturating_sub(buf.len()))]);
                    self.object(k, &buf, list, cancel);
                } else {
                    self.object(flags, d, list, cancel);
                }
            }
            // Clear
            0x4009 => {
                if let (Some(f), Some(v)) = (self.frame, u32le(d, 0)) {
                    list.fill(rect_path(f), Paint::Solid(argb(v)), false);
                }
            }
            // FillRects, DrawRects
            0x400a | 0x400b => {
                let (n, at) = if typ == 0x400a {
                    (rd(d, 4), 8)
                } else {
                    (rd(d, 0), 4)
                };
                let sz = if c { 8 } else { 16 };
                let n = (n as usize).min(d.len().saturating_sub(at) / sz);
                let mut pb = sk::PathBuilder::new();
                for k in 0..n {
                    if let Some(r) = rect_at(d, at + k * sz, c) {
                        pb.push_rect(r);
                    }
                }
                if typ == 0x400a {
                    self.fill(list, pb.finish(), self.brush(flags, rd(d, 0)));
                } else {
                    self.draw(list, pb.finish(), pen);
                }
            }
            // FillPolygon
            0x400c => {
                if flags & 0x0800 == 0
                    && let Some(pts) = points(d, 8, rd(d, 4), c)
                {
                    self.fill(
                        list,
                        super::polygon(&pts, true),
                        self.brush(flags, rd(d, 0)),
                    );
                }
            }
            // DrawLines
            0x400d => {
                if flags & 0x0800 == 0
                    && let Some(pts) = points(d, 4, rd(d, 0), c)
                {
                    self.draw(list, super::polygon(&pts, flags & 0x2000 != 0), pen);
                }
            }
            // FillEllipse, DrawEllipse
            0x400e => {
                let r = rect_at(d, 4, c).and_then(sk::PathBuilder::from_oval);
                self.fill(list, r, self.brush(flags, rd(d, 0)));
            }
            0x400f => {
                let r = rect_at(d, 0, c).and_then(sk::PathBuilder::from_oval);
                self.draw(list, r, pen);
            }
            // FillPie, DrawPie, DrawArc
            0x4010 => {
                if let (Some(s), Some(sw), Some(r)) = (f32le(d, 4), f32le(d, 8), rect_at(d, 12, c))
                {
                    self.fill(list, arc(r, s, sw, true), self.brush(flags, rd(d, 0)));
                }
            }
            0x4011 | 0x4012 => {
                if let (Some(s), Some(sw), Some(r)) = (f32le(d, 0), f32le(d, 4), rect_at(d, 8, c)) {
                    self.draw(list, arc(r, s, sw, typ == 0x4011), pen);
                }
            }
            // FillPath, DrawPath
            0x4014 | 0x4015 => {
                let Some(PObj::Path(path)) = self.obj(pen).cloned() else {
                    return;
                };
                if typ == 0x4014 {
                    self.fill(list, Some(path), self.brush(flags, rd(d, 0)));
                } else {
                    self.draw(list, Some(path), rd(d, 0));
                }
            }
            // DrawBeziers
            0x4019 => {
                if flags & 0x0800 == 0
                    && let Some(pts) = points(d, 4, rd(d, 0), c)
                {
                    self.draw(list, super::beziers(&pts), pen);
                }
            }
            // DrawImage, DrawImagePoints
            0x401a | 0x401b => {
                let Some(PObj::Image(pm)) = self.obj(pen).cloned() else {
                    return;
                };
                let Some(src) = rect_at(d, 8, false) else {
                    return;
                };
                let (iw, ih) = (pm.width() as f32, pm.height() as f32);
                let full = src.left() <= 0.0
                    && src.top() <= 0.0
                    && src.right() >= iw
                    && src.bottom() >= ih;
                let pm = if full {
                    pm
                } else {
                    let r = sk::IntRect::from_ltrb(
                        src.left().max(0.0) as i32,
                        src.top().max(0.0) as i32,
                        (src.right().min(iw)).ceil() as i32,
                        (src.bottom().min(ih)).ceil() as i32,
                    );
                    match r.and_then(|r| pm.clone_rect(r)) {
                        Some(p) => Arc::new(p),
                        None => return,
                    }
                };
                let (sw, sh) = (pm.width() as f32, pm.height() as f32);
                let place = if typ == 0x401a {
                    let Some(dst) = rect_at(d, 24, c) else { return };
                    sk::Transform::from_row(
                        dst.width() / sw,
                        0.0,
                        0.0,
                        dst.height() / sh,
                        dst.left(),
                        dst.top(),
                    )
                } else {
                    if flags & 0x0800 != 0 || rd(d, 24) != 3 {
                        return;
                    }
                    let Some(p) = points(d, 28, 3, c) else { return };
                    sk::Transform::from_row(
                        (p[1].0 - p[0].0) / sw,
                        (p[1].1 - p[0].1) / sw,
                        (p[2].0 - p[0].0) / sh,
                        (p[2].1 - p[0].1) / sh,
                        p[0].0,
                        p[0].1,
                    )
                };
                list.image(pm, self.xf().pre_concat(place), 1.0);
            }
            // DrawString, DrawDriverString
            0x401c | 0x4036 => list.notes.text = true,
            // Save, Restore, BeginContainer(NoParams), EndContainer
            0x4025 | 0x4028 => self.save(rd(d, 0)),
            0x4026 | 0x4029 => self.restore(rd(d, 0)),
            0x4027 => {
                self.save(rd(d, 32));
                if let (Some(dst), Some(src)) = (rect_at(d, 0, false), rect_at(d, 16, false)) {
                    let m = sk::Transform::from_row(
                        dst.width() / src.width(),
                        0.0,
                        0.0,
                        dst.height() / src.height(),
                        dst.left() - src.left() * dst.width() / src.width(),
                        dst.top() - src.top() * dst.height() / src.height(),
                    );
                    if m.is_finite() {
                        self.state.world = self.state.world.pre_concat(m);
                    }
                }
            }
            // World transform records; A (0x2000) appends.
            0x402a..=0x402f => {
                let f = |k: usize| f32le(d, 4 * k);
                let m = match typ {
                    0x402a | 0x402c => (|| {
                        Some(sk::Transform::from_row(
                            f(0)?,
                            f(1)?,
                            f(2)?,
                            f(3)?,
                            f(4)?,
                            f(5)?,
                        ))
                    })(),
                    0x402b => Some(sk::Transform::identity()),
                    0x402d => f(0)
                        .zip(f(1))
                        .map(|(x, y)| sk::Transform::from_translate(x, y)),
                    0x402e => f(0).zip(f(1)).map(|(x, y)| sk::Transform::from_scale(x, y)),
                    _ => f(0).map(sk::Transform::from_rotate),
                };
                let Some(m) = m.filter(|m| m.is_finite()) else {
                    return;
                };
                let w = self.state.world;
                self.state.world = match typ {
                    0x402a | 0x402b => m,
                    _ if flags & 0x2000 != 0 => w.post_concat(m),
                    _ => w.pre_concat(m),
                };
            }
            // SetPageTransform
            0x4030 => {
                if let Some(s) = f32le(d, 0).filter(|s| *s > 0.0 && *s < 1e6) {
                    self.state.unit = (flags & 0xff) as u32;
                    self.state.scale = s;
                }
            }
            // SetClipRect, SetClipPath
            0x4032 | 0x4033 => list.notes.clip = true,
            _ => {}
        }
    }
}

//! QuickDraw vector drawing: the shape, line, text, pen and pattern opcodes of a picture,
//! turned into a `specular-drawpage` display list in canvas pixels.
//!
//! Written from Inside Macintosh: Imaging With QuickDraw (the picture opcodes, appendix A, and
//! the drawing model). What it follows:
//!
//! - shapes (rectangles, rounded rectangles, ovals, arcs and wedges, polygons, regions) with
//!   the five verbs: frame (an outline the pen's size wide, inside the shape), paint (pen
//!   pattern), erase (background pattern), invert and fill (fill pattern), and the "same
//!   shape" opcodes;
//! - lines: the pen, a rectangle hanging below and right of the point, swept from one end to
//!   the other;
//! - patterns: 8 × 8 one-bit patterns in the foreground and background colours (the "or"
//!   modes leave the zero bits clear), pixel patterns (PixPat type 1, drawn tiled) and RGB
//!   patterns (type 2, solid);
//! - colours: RGB foreground and background and the eight old-style colours;
//! - text: LongText, DHText, DVText and DHDVText at the text origin, in the font named by
//!   FontName or the classic font numbers (Times, Helvetica, Courier, Geneva, …), its size and
//!   face (bold, italic, underline), Mac Roman coded; drawn with the drawing stack's fonts;
//! - the clip region and the picture origin.
//!
//! Transfer modes other than the "or" pattern modes draw as copy; the drawing is antialiased.

use std::collections::HashMap;
use std::sync::Arc;

use specular_drawpage::kurbo::{self, Affine, BezPath, Point, Shape, Vec2};
use specular_drawpage::{self as dp, Op, Paint};

use crate::{Error, Reader, Rect, Region, Result, read_map, read_row, row_rgba};

/// Curve flattening tolerance, pixels.
const TOL: f64 = 0.1;

/// A pattern as set by the pen, fill and background pattern opcodes.
#[derive(Clone, Debug)]
pub(crate) enum Pat {
    /// One-bit 8 × 8: bit 1 the foreground colour, bit 0 the background.
    Bits([u8; 8]),
    /// A pixel pattern, tiled from the local origin.
    Pix(Arc<dp::Image>),
    Rgb([u8; 3]),
}

/// The drawing verbs of the shape opcodes (the low three bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Frame,
    Paint,
    Erase,
    Invert,
    Fill,
}

impl Verb {
    fn of(op: u16) -> Option<Verb> {
        Some(match op & 7 {
            0 => Verb::Frame,
            1 => Verb::Paint,
            2 => Verb::Erase,
            3 => Verb::Invert,
            4 => Verb::Fill,
            _ => return None,
        })
    }
}

/// What a drawing opcode produced.
pub(crate) enum Drawn {
    /// Nothing visible (reserved verbs, empty shapes, opcodes that only set state).
    Nothing,
    Ops(Vec<Op>),
    /// Invert the canvas under this (canvas-pixel) path.
    Invert(BezPath),
}

/// The QuickDraw port state a picture changes as it plays.
pub(crate) struct State {
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pen: (i32, i32),
    pen_size: (i32, i32),
    pen_mode: u16,
    pen_pat: Pat,
    fill_pat: Pat,
    bk_pat: Pat,
    ov_size: (i32, i32),
    last_rect: Rect,
    last_rrect: Rect,
    last_oval: Rect,
    last_arc: Rect,
    last_poly: Vec<(i32, i32)>,
    last_rgn: BezPath,
    text_at: (i32, i32),
    font: u16,
    face: u8,
    text_size: u16,
    font_names: HashMap<u16, String>,
    /// Picture coordinates → canvas pixels.
    xf: Affine,
    clip: Option<BezPath>,
    /// Path elements emitted so far (bounded by [`dp::MAX_PATH_ELEMENTS`]).
    pub elements: usize,
}

impl State {
    pub fn new(canvas: Rect) -> State {
        State {
            fg: [0; 3],
            bg: [255; 3],
            pen: (0, 0),
            pen_size: (1, 1),
            pen_mode: 8,
            pen_pat: Pat::Bits([0xff; 8]),
            fill_pat: Pat::Bits([0xff; 8]),
            bk_pat: Pat::Bits([0; 8]),
            ov_size: (0, 0),
            last_rect: Rect::default(),
            last_rrect: Rect::default(),
            last_oval: Rect::default(),
            last_arc: Rect::default(),
            last_poly: Vec::new(),
            last_rgn: BezPath::new(),
            text_at: (0, 0),
            font: 0,
            face: 0,
            text_size: 12,
            font_names: HashMap::new(),
            xf: Affine::translate((-f64::from(canvas.left), -f64::from(canvas.top))),
            clip: None,
            elements: 0,
        }
    }

    /// Whether `op` is one of the opcodes this module plays.
    pub fn handles(op: u16) -> bool {
        matches!(
            op,
            0x01..=0x0f
                | 0x10
                | 0x20..=0x23
                | 0x28..=0x2b
                | 0x2c
                | 0x30..=0x34
                | 0x38..=0x3c
                | 0x40..=0x44
                | 0x48..=0x4c
                | 0x50..=0x54
                | 0x58..=0x5c
                | 0x60..=0x64
                | 0x68..=0x6c
                | 0x70..=0x74
                | 0x78..=0x7c
                | 0x80..=0x84
                | 0x88..=0x8c
        )
    }

    /// Plays opcode `op` (see [`State::handles`]), whose data `r` is at.
    pub fn play(&mut self, r: &mut Reader, op: u16) -> Result<Drawn> {
        let point = |r: &mut Reader| -> Result<(i32, i32)> {
            let v = i32::from(r.u16()? as i16);
            let h = i32::from(r.u16()? as i16);
            Ok((h, v))
        };
        match op {
            0x01 => {
                let rgn = Region::read(r)?;
                self.clip = Some(self.region_path(&rgn));
            }
            0x02 => self.bk_pat = Pat::Bits(pattern(r)?),
            0x03 => self.font = r.u16()?,
            0x04 => self.face = r.u8()?,
            0x05 | 0x10 => {
                r.skip(if op == 0x05 { 2 } else { 8 })?;
            }
            0x06 => r.skip(4)?, // SpExtra
            0x07 => {
                let (h, v) = point(r)?;
                self.pen_size = (h.max(0), v.max(0));
            }
            0x08 => self.pen_mode = r.u16()?,
            0x09 => self.pen_pat = Pat::Bits(pattern(r)?),
            0x0a => self.fill_pat = Pat::Bits(pattern(r)?),
            0x0b => {
                let (h, v) = point(r)?;
                self.ov_size = (h, v);
            }
            0x0c => {
                let (h, v) = point(r)?;
                self.xf *= Affine::translate((-f64::from(h), -f64::from(v)));
            }
            0x0d => self.text_size = r.u16()?,
            0x0e | 0x0f => {
                let c = old_colour(r.u32()?);
                if op == 0x0e {
                    self.fg = c;
                } else {
                    self.bg = c;
                }
            }
            0x20 => {
                let a = point(r)?;
                let b = point(r)?;
                self.pen = b;
                return Ok(self.line(a, b));
            }
            0x21 => {
                let b = point(r)?;
                let a = std::mem::replace(&mut self.pen, b);
                return Ok(self.line(a, b));
            }
            0x22 => {
                let a = point(r)?;
                let dh = i32::from(r.u8()? as i8);
                let dv = i32::from(r.u8()? as i8);
                let b = (a.0 + dh, a.1 + dv);
                self.pen = b;
                return Ok(self.line(a, b));
            }
            0x23 => {
                let dh = i32::from(r.u8()? as i8);
                let dv = i32::from(r.u8()? as i8);
                let a = self.pen;
                let b = (a.0 + dh, a.1 + dv);
                self.pen = b;
                return Ok(self.line(a, b));
            }
            0x28..=0x2b => {
                match op {
                    0x28 => self.text_at = point(r)?,
                    0x29 => self.text_at.0 += i32::from(r.u8()?),
                    0x2a => self.text_at.1 += i32::from(r.u8()?),
                    _ => {
                        self.text_at.0 += i32::from(r.u8()?);
                        self.text_at.1 += i32::from(r.u8()?);
                    }
                }
                let n = usize::from(r.u8()?);
                let text = mac_roman(r.take(n)?);
                return Ok(self.text(text));
            }
            0x2c => {
                let len = usize::from(r.u16()?);
                let body = r.take(len)?;
                if body.len() >= 3 {
                    let id = u16::from_be_bytes([body[0], body[1]]);
                    let n = usize::from(body[2]).min(body.len() - 3);
                    let name = mac_roman(&body[3..3 + n]);
                    if self.font_names.len() < 4096 {
                        self.font_names.insert(id, name);
                    }
                }
            }
            0x30..=0x34 | 0x38..=0x3c => {
                if op < 0x38 {
                    self.last_rect = r.rect()?;
                }
                let rect = self.last_rect;
                return Ok(self.shape(op, |s, inset| s.rect_path(rect, inset)));
            }
            0x40..=0x44 | 0x48..=0x4c => {
                if op < 0x48 {
                    self.last_rrect = r.rect()?;
                }
                let rect = self.last_rrect;
                let ov = self.ov_size;
                return Ok(self.shape(op, |s, inset| s.rrect_path(rect, ov, inset)));
            }
            0x50..=0x54 | 0x58..=0x5c => {
                if op < 0x58 {
                    self.last_oval = r.rect()?;
                }
                let rect = self.last_oval;
                return Ok(self.shape(op, |s, inset| s.oval_path(rect, inset)));
            }
            0x60..=0x64 | 0x68..=0x6c => {
                if op < 0x68 {
                    self.last_arc = r.rect()?;
                }
                let start = f64::from(r.u16()? as i16);
                let sweep = f64::from(r.u16()? as i16);
                return Ok(self.arc(op, self.last_arc, start, sweep));
            }
            0x70..=0x74 | 0x78..=0x7c => {
                if op < 0x78 {
                    let size = usize::from(r.u16()?);
                    r.skip(8)?; // bounding box
                    let n = size.saturating_sub(10) / 4;
                    let mut pts = Vec::with_capacity(n);
                    for _ in 0..n {
                        pts.push(point(r)?);
                    }
                    r.skip(size.saturating_sub(10) % 4)?;
                    self.last_poly = pts;
                }
                return Ok(self.poly(op));
            }
            0x80..=0x84 | 0x88..=0x8c => {
                if op < 0x88 {
                    let rgn = Region::read(r)?;
                    self.last_rgn = self.region_path(&rgn);
                }
                return Ok(self.region(op));
            }
            _ => return Err(Error::Malformed("QuickDraw opcode")),
        }
        Ok(Drawn::Nothing)
    }

    /// PnPixPat / FillPixPat / BkPixPat (0x13, 0x14, 0x12).
    pub fn set_pix_pat(&mut self, op: u16, pat: Pat) {
        match op {
            0x12 => self.bk_pat = pat,
            0x13 => self.pen_pat = pat,
            _ => self.fill_pat = pat,
        }
    }

    fn pt(&self, x: f64, y: f64) -> Point {
        self.xf * Point::new(x, y)
    }

    fn rect_path(&self, r: Rect, inset: (f64, f64)) -> Option<BezPath> {
        let (x0, y0) = (f64::from(r.left) + inset.0, f64::from(r.top) + inset.1);
        let (x1, y1) = (f64::from(r.right) - inset.0, f64::from(r.bottom) - inset.1);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let mut p = BezPath::new();
        p.move_to(self.pt(x0, y0));
        p.line_to(self.pt(x1, y0));
        p.line_to(self.pt(x1, y1));
        p.line_to(self.pt(x0, y1));
        p.close_path();
        Some(p)
    }

    fn oval_path(&self, r: Rect, inset: (f64, f64)) -> Option<BezPath> {
        let (x0, y0) = (f64::from(r.left) + inset.0, f64::from(r.top) + inset.1);
        let (x1, y1) = (f64::from(r.right) - inset.0, f64::from(r.bottom) - inset.1);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let e = kurbo::Ellipse::from_rect(kurbo::Rect::new(x0, y0, x1, y1));
        let mut p = e.to_path(TOL);
        p.apply_affine(self.xf);
        Some(p)
    }

    /// A rounded rectangle whose corners are quarters of an `ov`-sized oval.
    fn rrect_path(&self, r: Rect, ov: (i32, i32), inset: (f64, f64)) -> Option<BezPath> {
        let (x0, y0) = (f64::from(r.left) + inset.0, f64::from(r.top) + inset.1);
        let (x1, y1) = (f64::from(r.right) - inset.0, f64::from(r.bottom) - inset.1);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let rx = (f64::from(ov.0) / 2.0 - inset.0).clamp(0.0, (x1 - x0) / 2.0);
        let ry = (f64::from(ov.1) / 2.0 - inset.1).clamp(0.0, (y1 - y0) / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return self.rect_path(r, inset);
        }
        let q = std::f64::consts::FRAC_PI_2;
        let mut p = BezPath::new();
        p.move_to(Point::new(x0 + rx, y0));
        p.line_to(Point::new(x1 - rx, y0));
        corner(&mut p, Point::new(x1 - rx, y0 + ry), (rx, ry), -q);
        p.line_to(Point::new(x1, y1 - ry));
        corner(&mut p, Point::new(x1 - rx, y1 - ry), (rx, ry), 0.0);
        p.line_to(Point::new(x0 + rx, y1));
        corner(&mut p, Point::new(x0 + rx, y1 - ry), (rx, ry), q);
        p.line_to(Point::new(x0, y0 + ry));
        corner(&mut p, Point::new(x0 + rx, y0 + ry), (rx, ry), 2.0 * q);
        p.close_path();
        p.apply_affine(self.xf);
        Some(p)
    }

    /// A shape opcode: `path(state, inset)` gives the shape, inset by `inset` on every side.
    fn shape(&mut self, op: u16, path: impl Fn(&State, (f64, f64)) -> Option<BezPath>) -> Drawn {
        let Some(verb) = Verb::of(op) else {
            return Drawn::Nothing;
        };
        let Some(outer) = path(self, (0.0, 0.0)) else {
            return Drawn::Nothing;
        };
        if verb == Verb::Frame {
            let (pw, ph) = (f64::from(self.pen_size.0), f64::from(self.pen_size.1));
            if pw <= 0.0 || ph <= 0.0 {
                return Drawn::Nothing;
            }
            let mut ring = outer;
            if let Some(inner) = path(self, (pw, ph)) {
                ring.extend(inner);
            }
            return self.fill(verb, ring, true);
        }
        self.fill(verb, outer, false)
    }

    /// Arcs and wedges: angles in degrees clockwise from 12 o'clock, relative to the
    /// rectangle (45° points at its top right corner).
    fn arc(&mut self, op: u16, r: Rect, start: f64, sweep: f64) -> Drawn {
        let Some(verb) = Verb::of(op) else {
            return Drawn::Nothing;
        };
        if r.is_empty() || sweep == 0.0 {
            return Drawn::Nothing;
        }
        let sweep = sweep.clamp(-360.0, 360.0);
        let c = (
            (f64::from(r.left) + f64::from(r.right)) / 2.0,
            (f64::from(r.top) + f64::from(r.bottom)) / 2.0,
        );
        let radii = |inset: (f64, f64)| {
            Vec2::new(
                f64::from(r.width()) / 2.0 - inset.0,
                f64::from(r.height()) / 2.0 - inset.1,
            )
        };
        let (a0, da) = ((start - 90.0).to_radians(), sweep.to_radians());
        let arc = |rad: Vec2, a0: f64, da: f64| kurbo::Arc {
            center: Point::new(c.0, c.1),
            radii: rad,
            start_angle: a0,
            sweep_angle: da,
            x_rotation: 0.0,
        };
        let at = |rad: Vec2, a: f64| Point::new(c.0 + rad.x * a.cos(), c.1 + rad.y * a.sin());
        let mut p = BezPath::new();
        if verb == Verb::Frame {
            // The curved edge only, the pen's size wide, inside the oval.
            let (pw, ph) = (f64::from(self.pen_size.0), f64::from(self.pen_size.1));
            if pw <= 0.0 || ph <= 0.0 {
                return Drawn::Nothing;
            }
            let outer = radii((0.0, 0.0));
            let inner = radii((pw, ph));
            p.move_to(at(outer, a0));
            p.extend(arc(outer, a0, da).append_iter(TOL));
            if inner.x > 0.0 && inner.y > 0.0 {
                p.line_to(at(inner, a0 + da));
                p.extend(arc(inner, a0 + da, -da).append_iter(TOL));
            } else {
                p.line_to(Point::new(c.0, c.1));
            }
            p.close_path();
        } else {
            let rad = radii((0.0, 0.0));
            p.move_to(Point::new(c.0, c.1));
            p.line_to(at(rad, a0));
            p.extend(arc(rad, a0, da).append_iter(TOL));
            p.close_path();
        }
        p.apply_affine(self.xf);
        self.fill(verb, p, false)
    }

    fn poly(&mut self, op: u16) -> Drawn {
        let Some(verb) = Verb::of(op) else {
            return Drawn::Nothing;
        };
        let pts = std::mem::take(&mut self.last_poly);
        let out = if pts.len() < 2 {
            Drawn::Nothing
        } else if verb == Verb::Frame {
            // Frame: the pen along each edge (the polygon closes only if its points do).
            let mut ops = Vec::new();
            for w in pts.windows(2) {
                if let Drawn::Ops(o) = self.line(w[0], w[1]) {
                    ops.extend(o);
                }
            }
            Drawn::Ops(ops)
        } else {
            let mut p = BezPath::new();
            for (i, &(x, y)) in pts.iter().enumerate() {
                let q = self.pt(f64::from(x), f64::from(y));
                if i == 0 {
                    p.move_to(q);
                } else {
                    p.line_to(q);
                }
            }
            p.close_path();
            self.fill(verb, p, true)
        };
        self.last_poly = pts;
        out
    }

    fn region(&mut self, op: u16) -> Drawn {
        let Some(verb) = Verb::of(op) else {
            return Drawn::Nothing;
        };
        let p = self.last_rgn.clone();
        if p.elements().is_empty() {
            return Drawn::Nothing;
        }
        if verb == Verb::Frame {
            // The region's outline, stroked the pen's width (centred on the edge).
            let w = f64::from(self.pen_size.0.max(self.pen_size.1));
            if w <= 0.0 {
                return Drawn::Nothing;
            }
            let Some(paint) = self.paint(verb) else {
                return Drawn::Nothing;
            };
            self.elements += p.elements().len();
            return Drawn::Ops(self.clipped(Op::Stroke {
                path: p,
                stroke: kurbo::Stroke::new(w),
                paint,
            }));
        }
        self.fill(verb, p, false)
    }

    /// A region as a path: one rectangle per span between its inversion rows.
    fn region_path(&self, rgn: &Region) -> BezPath {
        let mut p = BezPath::new();
        let b = rgn.bbox;
        if rgn.data.is_empty() {
            return self.rect_path(b, (0.0, 0.0)).unwrap_or_default();
        }
        let mut cur: Vec<i32> = Vec::new();
        let mut rows: Vec<(i32, Vec<i32>)> = Vec::new();
        let mut i = 0;
        let d = &rgn.data;
        while let Some(&y) = d.get(i) {
            if y == 0x7fff {
                break;
            }
            i += 1;
            while let Some(&x) = d.get(i) {
                i += 1;
                if x == 0x7fff {
                    break;
                }
                match cur.binary_search(&x) {
                    Ok(k) => {
                        cur.remove(k);
                    }
                    Err(k) if cur.len() < 1 << 14 => cur.insert(k, x),
                    Err(_) => {}
                }
            }
            rows.push((y, cur.clone()));
            if rows.len() > 1 << 16 {
                break;
            }
        }
        for w in rows.windows(2) {
            let (y0, xs) = (&w[0].0, &w[0].1);
            let y1 = w[1].0;
            for s in xs.chunks_exact(2) {
                let r = Rect {
                    top: (*y0).max(b.top),
                    left: s[0],
                    bottom: y1.min(b.bottom),
                    right: s[1],
                };
                if let Some(q) = self.rect_path(r, (0.0, 0.0)) {
                    p.extend(q);
                }
            }
        }
        p
    }

    /// A line from `a` to `b`: the pen rectangle swept along it (the hull of its corners at
    /// both ends).
    fn line(&mut self, a: (i32, i32), b: (i32, i32)) -> Drawn {
        let (pw, ph) = self.pen_size;
        if pw <= 0 || ph <= 0 {
            return Drawn::Nothing;
        }
        let mut pts = Vec::with_capacity(8);
        for &(x, y) in &[a, b] {
            for (dx, dy) in [(0, 0), (pw, 0), (pw, ph), (0, ph)] {
                pts.push((f64::from(x + dx), f64::from(y + dy)));
            }
        }
        let hull = convex_hull(&mut pts);
        let mut p = BezPath::new();
        for (i, &(x, y)) in hull.iter().enumerate() {
            let q = self.pt(x, y);
            if i == 0 {
                p.move_to(q);
            } else {
                p.line_to(q);
            }
        }
        p.close_path();
        self.fill(Verb::Paint, p, false)
    }

    fn text(&mut self, text: String) -> Drawn {
        if text.trim().is_empty() {
            return Drawn::Nothing;
        }
        let size = if self.text_size == 0 {
            12.0
        } else {
            f64::from(self.text_size)
        };
        let family = self
            .font_names
            .get(&self.font)
            .cloned()
            .unwrap_or_else(|| classic_font(self.font).to_string());
        let mut run = dp::Run::new(text, family.clone(), size);
        run.bold = self.face & 1 != 0;
        run.italic = self.face & 2 != 0;
        run.underline = self.face & 4 != 0;
        run.color = [self.fg[0], self.fg[1], self.fg[2], 255];
        let f = dp::face(&family, run.bold, run.italic);
        let ascent = f64::from(f.ascent) * size / f64::from(f.units_per_em.max(1.0));
        let at = self.pt(
            f64::from(self.text_at.0),
            f64::from(self.text_at.1) - ascent,
        );
        let mut block = dp::TextBlock::new(Affine::translate(at.to_vec2()), 1e5, size * 2.0);
        block.wrap = false;
        block.paragraphs.push(dp::Paragraph {
            runs: vec![run],
            ..Default::default()
        });
        self.elements += 1;
        Drawn::Ops(self.clipped(Op::Text(Box::new(block))))
    }

    /// The paint a verb draws with, `None` when it draws nothing.
    fn paint(&self, verb: Verb) -> Option<Paint> {
        let pat = match verb {
            Verb::Frame | Verb::Paint => &self.pen_pat,
            Verb::Erase => &self.bk_pat,
            Verb::Fill => &self.fill_pat,
            Verb::Invert => return None,
        };
        // The "or" modes (srcOr, patOr) leave the background-coloured bits alone.
        let or = matches!(verb, Verb::Frame | Verb::Paint) && matches!(self.pen_mode & 0x3f, 1 | 9);
        let rgba = |c: [u8; 3]| [c[0], c[1], c[2], 255];
        Some(match pat {
            Pat::Rgb(c) => Paint::Solid(rgba(*c)),
            Pat::Bits(b) if b.iter().all(|&v| v == 0xff) => Paint::Solid(rgba(self.fg)),
            Pat::Bits(b) if b.iter().all(|&v| v == 0) => {
                if or {
                    return None;
                }
                Paint::Solid(rgba(self.bg))
            }
            Pat::Bits(b) => {
                let mut px = Vec::with_capacity(256);
                for row in b {
                    for x in 0..8 {
                        if row & (0x80 >> x) != 0 {
                            px.extend_from_slice(&rgba(self.fg));
                        } else if or {
                            px.extend_from_slice(&[0; 4]);
                        } else {
                            px.extend_from_slice(&rgba(self.bg));
                        }
                    }
                }
                let img = dp::Image::from_rgba(8, 8, px)?;
                Paint::Image {
                    image: Arc::new(img),
                    transform: self.xf,
                    repeat: true,
                }
            }
            Pat::Pix(img) => Paint::Image {
                image: img.clone(),
                transform: self.xf,
                repeat: true,
            },
        })
    }

    fn fill(&mut self, verb: Verb, path: BezPath, even_odd: bool) -> Drawn {
        if verb == Verb::Invert {
            return Drawn::Invert(path);
        }
        let Some(paint) = self.paint(verb) else {
            return Drawn::Nothing;
        };
        self.elements += path.elements().len();
        Drawn::Ops(self.clipped(Op::Fill {
            path,
            even_odd,
            paint,
        }))
    }

    /// `op` inside the clip region.
    fn clipped(&self, op: Op) -> Vec<Op> {
        match &self.clip {
            Some(c) => vec![
                Op::PushClip {
                    path: c.clone(),
                    even_odd: false,
                },
                op,
                Op::Pop,
            ],
            None => vec![op],
        }
    }
}

/// A quarter of an `r`-radii oval around `c` from angle `a` (radians, clockwise from 3
/// o'clock in y-down space), appended to `p`.
fn corner(p: &mut BezPath, c: Point, r: (f64, f64), a: f64) {
    let arc = kurbo::Arc {
        center: c,
        radii: Vec2::new(r.0, r.1),
        start_angle: a,
        sweep_angle: std::f64::consts::FRAC_PI_2,
        x_rotation: 0.0,
    };
    for el in arc.append_iter(TOL) {
        p.push(el);
    }
}

/// The convex hull of `pts` (Andrew's monotone chain), clockwise in y-down space.
fn convex_hull(pts: &mut [(f64, f64)]) -> Vec<(f64, f64)> {
    pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut h: Vec<(f64, f64)> = Vec::with_capacity(pts.len() * 2);
    for &p in pts.iter() {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
            h.pop();
        }
        h.push(p);
    }
    let lower = h.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
            h.pop();
        }
        h.push(p);
    }
    h.pop();
    h
}

fn pattern(r: &mut Reader) -> Result<[u8; 8]> {
    let mut p = [0u8; 8];
    p.copy_from_slice(r.take(8)?);
    Ok(p)
}

/// A PixPat (BkPixPat, PnPixPat, FillPixPat): type 1 a pixel map, tiled; type 2 an RGB
/// colour; else the one-bit pattern.
pub(crate) fn pix_pat(r: &mut Reader, fg: [u8; 3], bg: [u8; 3]) -> Result<Pat> {
    let kind = r.u16()?;
    let bits = pattern(r)?;
    match kind {
        1 => {
            let m = read_map(r, false)?;
            let (w, h) = (m.bounds.width() as usize, m.bounds.height() as usize);
            let mut buf = Vec::new();
            let mut px = vec![0u8; w * h * 4];
            for row in px.chunks_exact_mut(w * 4) {
                read_row(r, &m, true, false, &mut buf)?;
                row_rgba(&m, &buf, fg, bg, row);
            }
            Ok(match dp::Image::from_rgba(w as u32, h as u32, px) {
                Some(img) => Pat::Pix(Arc::new(img)),
                None => Pat::Bits(bits),
            })
        }
        2 => {
            let c = r.take(6)?;
            Ok(Pat::Rgb([c[0], c[2], c[4]]))
        }
        _ => Ok(Pat::Bits(bits)),
    }
}

/// The eight colours of the original QuickDraw (FgColor / BkColor).
fn old_colour(c: u32) -> [u8; 3] {
    match c {
        30 => [255, 255, 255],
        205 => [221, 8, 6],
        341 => [0, 128, 17],
        409 => [0, 0, 212],
        273 => [2, 171, 234],
        137 => [242, 8, 132],
        69 => [252, 243, 5],
        _ => [0, 0, 0],
    }
}

/// The family of a classic Macintosh font number.
fn classic_font(id: u16) -> &'static str {
    match id {
        2 | 20 => "Times",
        4 | 22 => "Courier",
        23 => "Symbol",
        _ => "Helvetica",
    }
}

/// Mac Roman text to a string.
fn mac_roman(b: &[u8]) -> String {
    const HIGH: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø\
        ¿¡¬√ƒ≈∆«»…\u{a0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{f8ff}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";
    b.iter()
        .map(|&c| match c {
            0x80.. => HIGH.chars().nth(usize::from(c - 0x80)).unwrap_or('?'),
            b'\r' | b'\n' | b'\t' => ' ',
            c if c < 0x20 => ' ',
            c => char::from(c),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_roman_high_half() {
        assert_eq!(mac_roman(&[0x80, 0xa5, 0xd0, 0xff, b'A']), "Ä•–ˇA");
        assert_eq!(
            "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»…\u{a0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{f8ff}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ"
                .chars()
                .count(),
            128
        );
    }

    #[test]
    fn hull_of_a_swept_pen() {
        let mut pts = vec![
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (10.0, 5.0),
            (12.0, 5.0),
            (12.0, 7.0),
            (10.0, 7.0),
        ];
        let h = convex_hull(&mut pts);
        assert_eq!(h.len(), 6);
    }

    #[test]
    fn verbs() {
        assert_eq!(Verb::of(0x31), Some(Verb::Paint));
        assert_eq!(Verb::of(0x84), Some(Verb::Fill));
        assert_eq!(Verb::of(0x35), None);
    }
}

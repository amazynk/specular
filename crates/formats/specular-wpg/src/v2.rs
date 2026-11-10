//! WPG version 2 records.

use std::collections::HashMap;
use std::sync::Arc;

use specular_drawpage::kurbo::{Arc as KArc, Ellipse, PathEl, Point, RoundedRect, Vec2};
use specular_drawpage::{
    Affine, BezPath, Cap, Image, Join, MAX_IMAGE_PIXELS, Op, Page, Paint, Rect, Rgba, Shape, Stop,
    Stroke,
};

use crate::bitmap::{rle2, to_rgba};
use crate::palette::default_palette;
use crate::reader::R;
use crate::{Error, Facts, MAX_POINTS, MAX_RECORDS, page_fit};

/// Deepest group nesting followed.
const MAX_DEPTH: usize = 256;

/// An object's characterisation: what is drawn, and its transform (WPG space).
#[derive(Clone, Copy)]
struct Ch {
    filled: bool,
    closed: bool,
    framed: bool,
    nonzero: bool,
    m: Affine,
}

enum Kind {
    Group,
    /// A compound polygon: its members' outlines collect into one path.
    Compound(Ch, BezPath),
    Other,
}

struct Frame {
    remaining: u32,
    /// This frame's transform, composed with its parents'.
    m: Affine,
    kind: Kind,
}

struct State {
    dp: bool,
    to_page: Affine,
    /// Page points per WPG unit (line widths).
    k: f64,
    palette: Vec<Rgba>,
    pen: Rgba,
    pen_style: u16,
    pen_width: f64,
    cap: Cap,
    join: Join,
    dashes: HashMap<u16, Vec<f64>>,
    brush: Rgba,
    gradient: Option<(Rgba, Rgba)>,
    gradient_angle: f64,
    stack: Vec<Frame>,
    /// The box and transform of the last Bitmap record, for its Bitmap Data.
    bitmap: Option<(Rect, Affine)>,
    points: usize,
    pixels: u64,
}

impl State {
    fn coord(&self, r: &mut R) -> Option<f64> {
        if self.dp {
            r.i32().map(|v| f64::from(v) / 65536.0)
        } else {
            r.i16().map(f64::from)
        }
    }

    fn point(&self, r: &mut R) -> Option<Point> {
        Some(Point::new(self.coord(r)?, self.coord(r)?))
    }

    fn points(&mut self, r: &mut R, n: usize, facts: &mut Facts) -> Option<Vec<Point>> {
        let each = if self.dp { 8 } else { 4 };
        if n.checked_mul(each)? > r.left() {
            facts.truncated = true;
            return None;
        }
        self.points += n;
        if self.points > MAX_POINTS {
            facts.limited = true;
            return None;
        }
        (0..n).map(|_| self.point(r)).collect()
    }

    /// The transform of the enclosing frames (WPG space).
    fn parent(&self) -> Affine {
        self.stack.last().map_or(Affine::IDENTITY, |f| f.m)
    }

    /// Characterisation: flags, then the object id, rotation, scale/skew and translation the
    /// flags announce.
    fn ch(&self, r: &mut R) -> Option<Ch> {
        let flags = r.u16()?;
        if flags & 0x20 != 0 {
            let id = r.u16()?;
            if id & 0x8000 != 0 {
                r.u16()?;
            }
        }
        let fixed = |v: i32| f64::from(v) / 65536.0;
        let (taper, translate, skew, scale, rotate) = (
            flags & 0x01 != 0,
            flags & 0x02 != 0,
            flags & 0x04 != 0,
            flags & 0x08 != 0,
            flags & 0x10 != 0,
        );
        let (mut m00, mut m01, mut m10, mut m11, mut tx, mut ty) = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        if rotate {
            r.i32()?;
        }
        if rotate || scale {
            m00 = fixed(r.i32()?);
            m11 = fixed(r.i32()?);
        }
        if rotate || skew {
            m10 = fixed(r.i32()?);
            m01 = fixed(r.i32()?);
        }
        if translate {
            if self.dp {
                tx = fixed(r.i32()?);
                ty = fixed(r.i32()?);
            } else {
                let fx = f64::from(r.u16()?) / 65536.0;
                tx = f64::from(r.i16()?) + fx;
                let fy = f64::from(r.u16()?) / 65536.0;
                ty = f64::from(r.i16()?) + fy;
            }
        }
        if taper {
            r.i32()?;
            r.i32()?;
        }
        let m = Affine::new([m00, m01, m10, m11, tx, ty]);
        let m = if m.determinant().is_finite() && m.determinant() != 0.0 {
            m
        } else {
            Affine::IDENTITY
        };
        Some(Ch {
            filled: flags & (1 << 13) != 0,
            closed: flags & (1 << 14) != 0,
            framed: flags & (1 << 15) != 0,
            nonzero: flags & (1 << 12) != 0,
            m,
        })
    }

    fn stroke(&self) -> Stroke {
        let mut s = Stroke::new(self.pen_width * self.k)
            .with_caps(self.cap)
            .with_join(self.join);
        if self.pen_style != 0 {
            let unit = (self.pen_width * self.k).max(1.0);
            let d: Vec<f64> = match self.dashes.get(&self.pen_style) {
                Some(d) => d.iter().map(|v| v * self.k).collect(),
                None => match self.pen_style % 6 {
                    1 => vec![12.0 * unit, 4.0 * unit],
                    2 => vec![unit, 3.0 * unit],
                    3 => vec![8.0 * unit, 3.0 * unit, unit, 3.0 * unit],
                    4 => vec![8.0 * unit, 4.0 * unit],
                    5 => vec![4.0 * unit, 4.0 * unit],
                    _ => Vec::new(),
                },
            };
            if d.len() >= 2
                && d.iter().all(|v| v.is_finite() && *v >= 0.0)
                && d.iter().sum::<f64>() > 0.0
            {
                s = s.with_dashes(0.0, d);
            }
        }
        s
    }

    fn brush_paint(&self, page_path: &BezPath) -> Paint {
        match self.gradient {
            Some((c0, c1)) => {
                let b = page_path.bounding_box();
                let c = b.center();
                // The angle is counter-clockwise in WPG space: clockwise on the page.
                let a = -self.gradient_angle.to_radians();
                let d = Vec2::new(a.cos(), a.sin());
                let half = (b.width() * d.x.abs() + b.height() * d.y.abs()) / 2.0;
                Paint::Linear {
                    start: c - d * half,
                    end: c + d * half,
                    stops: vec![
                        Stop {
                            offset: 0.0,
                            color: c0,
                        },
                        Stop {
                            offset: 1.0,
                            color: c1,
                        },
                    ],
                }
            }
            None => Paint::Solid(self.brush),
        }
    }

    /// Draws a path in the object's space, or adds it to the open compound polygon.
    fn draw(&mut self, ops: &mut Vec<Op>, path: BezPath, ch: Ch) {
        if let Some(Frame {
            kind: Kind::Compound(cch, acc),
            ..
        }) = self.stack.last_mut()
        {
            let mut p = path;
            p.apply_affine(ch.m);
            for e in p.elements() {
                acc.push(*e);
            }
            if cch.closed
                && !acc.elements().is_empty()
                && !matches!(acc.elements().last(), Some(PathEl::ClosePath))
            {
                acc.close_path();
            }
            return;
        }
        let t = self.to_page * self.parent() * ch.m;
        self.emit(ops, path, ch, t);
    }

    fn emit(&self, ops: &mut Vec<Op>, mut path: BezPath, ch: Ch, t: Affine) {
        path.apply_affine(t);
        if ch.filled {
            let paint = self.brush_paint(&path);
            ops.push(Op::Fill {
                path: path.clone(),
                even_odd: !ch.nonzero,
                paint,
            });
        }
        if ch.framed && self.pen[3] > 0 {
            ops.push(Op::Stroke {
                path,
                stroke: self.stroke(),
                paint: Paint::Solid(self.pen),
            });
        }
    }

    /// Closes frames whose members are all read; a compound polygon is drawn then.
    fn pop_done(&mut self, ops: &mut Vec<Op>) {
        while self.stack.last().is_some_and(|f| f.remaining == 0) {
            let Some(f) = self.stack.pop() else { break };
            if let Kind::Compound(ch, path) = f.kind {
                // Its members are already in the compound's space.
                let t = self.to_page * self.parent() * ch.m;
                self.emit(ops, path, ch, t);
            }
        }
    }
}

/// RGBA with WPG's transparency byte (0 opaque).
fn rgba(r: &mut R) -> Option<Rgba> {
    let (cr, cg, cb, ca) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
    Some([cr, cg, cb, 255 - ca])
}

/// The 16-bit-per-channel form.
fn rgba16(r: &mut R) -> Option<Rgba> {
    let (cr, cg, cb, ca) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
    Some([
        (cr >> 8) as u8,
        (cg >> 8) as u8,
        (cb >> 8) as u8,
        255 - (ca >> 8) as u8,
    ])
}

fn poly(pts: &[Point], close: bool) -> BezPath {
    let mut p = BezPath::new();
    for (i, q) in pts.iter().enumerate() {
        if i == 0 {
            p.move_to(*q);
        } else {
            p.line_to(*q);
        }
    }
    if close && pts.len() > 2 {
        p.close_path();
    }
    p
}

fn skip_name(t: u8) -> Option<&'static str> {
    Some(match t {
        0x0F | 0x1C | 0x1D | 0x1E => "text",
        0x10 | 0x11 | 0x1F => "charts",
        0x12 => "embedded objects",
        0x3A | 0x3B => "envelopes",
        0x3C | 0x3D | 0x3F => "textures",
        // Settings, layers, patterns, comments, fonts and other state that draws nothing.
        0x02..=0x0B | 0x22..=0x24 | 0x2A | 0x35..=0x39 | 0x3E => return None,
        _ => "unknown records",
    })
}

pub(crate) fn parse(data: &[u8], facts: &mut Facts) -> Result<Page, Error> {
    let mut r = R::new(data);
    let mut st = State {
        dp: false,
        to_page: Affine::IDENTITY,
        k: 1.0,
        palette: default_palette(),
        pen: [0, 0, 0, 255],
        pen_style: 0,
        pen_width: 0.0,
        cap: Cap::Butt,
        join: Join::Miter,
        dashes: HashMap::new(),
        brush: [255, 255, 255, 255],
        gradient: None,
        gradient_angle: 0.0,
        stack: Vec::new(),
        bitmap: None,
        points: 0,
        pixels: 0,
    };
    let mut ops: Vec<Op> = Vec::new();
    let mut size: Option<(f64, f64)> = None;
    let mut count = 0usize;
    while r.left() > 0 {
        count += 1;
        if count > MAX_RECORDS {
            facts.limited = true;
            break;
        }
        let (Some(_class), Some(t)) = (r.u8(), r.u8()) else {
            facts.truncated = true;
            break;
        };
        let (Some(ext), Some(len)) = (r.varint(), r.varint()) else {
            facts.truncated = true;
            break;
        };
        let body = match r.bytes(len as usize) {
            Some(b) => b,
            None => {
                facts.truncated = true;
                r.rest()
            }
        };
        facts.records += 1;
        if let Some(f) = st.stack.last_mut() {
            f.remaining = f.remaining.saturating_sub(1);
        }
        let depth_before = st.stack.len();
        let mut b = R::new(body);
        if t == 0x02 {
            break;
        }
        if size.is_none() && t != 0x01 {
            // Nothing is drawn before the Start WPG record sets the coordinate system.
            continue;
        }
        match t {
            0x01 => {
                if size.is_some() {
                    continue;
                }
                let (Some(xres), Some(yres), Some(prec)) = (b.u16(), b.u16(), b.u8()) else {
                    return Err(Error::Malformed("short Start WPG record".into()));
                };
                st.dp = prec == 1;
                facts.double_precision = st.dp;
                let vals: Vec<f64> = (0..6).map_while(|_| st.coord(&mut b)).collect();
                let get = |i: usize| vals.get(i).copied().unwrap_or(0.0);
                let (vx1, vy1, vx2, vy2) = (get(0), get(1), get(2), get(3));
                let (mut x0, mut y0, mut x1, mut y1) = (vx1, vy1, vx2, vy2);
                if !(x1 > x0 && y1 > y0) {
                    (x0, y0, x1, y1) = (0.0, 0.0, get(4), get(5));
                }
                if !(x1 > x0 && y1 > y0) {
                    return Err(Error::Malformed("a picture without a size".into()));
                }
                let xres = f64::from(if xres == 0 { 1200 } else { xres });
                let yres = f64::from(if yres == 0 { 1200 } else { yres });
                let (wpt, hpt) = ((x1 - x0) / xres * 72.0, (y1 - y0) / yres * 72.0);
                let fit = page_fit(wpt, hpt);
                let (kx, ky) = (72.0 / xres * fit, 72.0 / yres * fit);
                st.k = (kx * ky).sqrt();
                st.to_page = Affine::new([kx, 0.0, 0.0, -ky, -x0 * kx, y1 * ky]);
                size = Some((wpt * fit, hpt * fit));
            }
            0x0C | 0x0D => {
                let (Some(start), Some(n)) = (b.u16(), b.u16()) else {
                    continue;
                };
                for i in 0..usize::from(n) {
                    let c = if t == 0x0C {
                        rgba(&mut b)
                    } else {
                        rgba16(&mut b)
                    };
                    let Some(c) = c else { break };
                    let idx = usize::from(start) + i;
                    if idx < 256 {
                        st.palette[idx] = c;
                    }
                }
            }
            0x08 => {
                let (Some(style), Some(n)) = (b.u16(), b.u16()) else {
                    continue;
                };
                let d: Vec<f64> = (0..usize::from(n).min(64))
                    .map_while(|_| st.coord(&mut b))
                    .map(f64::abs)
                    .collect();
                st.dashes.insert(style, d);
            }
            0x25 => st.pen = rgba(&mut b).unwrap_or(st.pen),
            0x26 => st.pen = rgba16(&mut b).unwrap_or(st.pen),
            0x29 => st.pen_style = b.u16().unwrap_or(0),
            0x2B => st.pen_width = st.coord(&mut b).unwrap_or(0.0).abs(),
            0x2C => {
                st.pen_width = b.i32().map_or(0.0, |v| f64::from(v) / 65536.0).abs();
            }
            0x2D => {
                st.cap = match b.u8() {
                    Some(1) => Cap::Round,
                    Some(2) => Cap::Square,
                    _ => Cap::Butt,
                }
            }
            0x2E => {
                st.join = match b.u8() {
                    Some(1) => Join::Round,
                    Some(2) => Join::Bevel,
                    _ => Join::Miter,
                }
            }
            0x2F | 0x30 => {
                let frac = f64::from(b.u16().unwrap_or(0)) / 65536.0;
                st.gradient_angle = f64::from(b.i16().unwrap_or(0)) + frac;
            }
            0x31 | 0x32 => {
                let ty = b.u8().unwrap_or(0);
                let one = |b: &mut R| if t == 0x31 { rgba(b) } else { rgba16(b) };
                if ty == 0 {
                    st.brush = one(&mut b).unwrap_or(st.brush);
                    st.gradient = None;
                } else {
                    let n = usize::from(b.u16().unwrap_or(0)).min(256);
                    let cs: Vec<Rgba> = (0..n).map_while(|_| one(&mut b)).collect();
                    match cs.as_slice() {
                        [] => {}
                        [c] => {
                            st.brush = *c;
                            st.gradient = None;
                        }
                        [c0, .., c1] => {
                            st.brush = *c0;
                            st.gradient = Some((*c0, *c1));
                        }
                    }
                }
            }
            0x27 | 0x28 | 0x33 | 0x34 => {}
            0x15 | 0x16 => {
                let Some(ch) = st.ch(&mut b) else { continue };
                let n = usize::from(b.u16().unwrap_or(0));
                if let Some(pts) = st.points(&mut b, n, facts) {
                    st.draw(&mut ops, poly(&pts, ch.closed), ch);
                }
            }
            0x17 => {
                let Some(ch) = st.ch(&mut b) else { continue };
                let n = usize::from(b.u16().unwrap_or(0));
                let Some(pts) = st.points(&mut b, n.saturating_mul(3), facts) else {
                    continue;
                };
                let tri: Vec<&[Point]> = pts.chunks_exact(3).collect();
                if let Some(first) = tri.first() {
                    let mut p = BezPath::new();
                    p.move_to(first[1]);
                    for w in tri.windows(2) {
                        p.curve_to(w[0][2], w[1][0], w[1][1]);
                    }
                    if ch.closed && tri.len() > 1 {
                        let last = tri[tri.len() - 1];
                        p.curve_to(last[2], first[0], first[1]);
                        p.close_path();
                    }
                    st.draw(&mut ops, p, ch);
                }
            }
            0x18 => {
                let Some(ch) = st.ch(&mut b) else { continue };
                let vals: Vec<f64> = (0..6).map_while(|_| st.coord(&mut b)).collect();
                if vals.len() < 4 {
                    continue;
                }
                let rect = Rect::new(vals[0], vals[1], vals[2], vals[3]).abs();
                let (rx, ry) = (
                    vals.get(4).copied().unwrap_or(0.0).abs(),
                    vals.get(5).copied().unwrap_or(0.0).abs(),
                );
                let path = if rx > 0.0 && ry > 0.0 {
                    // Elliptical corners: a circular rounded rectangle scaled to them.
                    let sy = ry / rx;
                    let rr = RoundedRect::from_rect(
                        Rect::new(rect.x0, rect.y0 / sy, rect.x1, rect.y1 / sy),
                        rx.min(rect.width() / 2.0),
                    );
                    let mut p = rr.to_path(0.1);
                    p.apply_affine(Affine::scale_non_uniform(1.0, sy));
                    p
                } else {
                    rect.to_path(0.1)
                };
                let ch = Ch { closed: true, ..ch };
                st.draw(&mut ops, path, ch);
            }
            0x19 => {
                let Some(ch) = st.ch(&mut b) else { continue };
                let v: Vec<f64> = (0..8).map_while(|_| st.coord(&mut b)).collect();
                if v.len() < 8 {
                    continue;
                }
                let c = Point::new(v[0], v[1]);
                let radii = Vec2::new(v[2].abs(), v[3].abs());
                if radii.x == 0.0 || radii.y == 0.0 {
                    continue;
                }
                let (i, e) = (Point::new(v[4], v[5]), Point::new(v[6], v[7]));
                let tol = 0.05 / st.k.max(1e-9);
                if i == e {
                    let ch = Ch { closed: true, ..ch };
                    st.draw(&mut ops, Ellipse::new(c, radii, 0.0).to_path(tol), ch);
                } else {
                    let ang = |p: Point| ((p.y - c.y) / radii.y).atan2((p.x - c.x) / radii.x);
                    let a0 = ang(i);
                    let mut sweep = ang(e) - a0;
                    if sweep <= 0.0 {
                        sweep += std::f64::consts::TAU;
                    }
                    let arc = KArc::new(c, radii, a0, sweep, 0.0);
                    let mut p = BezPath::new();
                    if ch.closed {
                        p.move_to(c);
                        for el in arc.path_elements(tol) {
                            match el {
                                PathEl::MoveTo(q) => p.line_to(q),
                                el => p.push(el),
                            }
                        }
                        p.close_path();
                    } else {
                        p = arc.to_path(tol);
                    }
                    st.draw(&mut ops, p, ch);
                }
            }
            0x1A | 0x20 | 0x21 => {
                let ch = st.ch(&mut b).unwrap_or(Ch {
                    filled: false,
                    closed: false,
                    framed: false,
                    nonzero: false,
                    m: Affine::IDENTITY,
                });
                if ext > 0 && st.stack.len() < MAX_DEPTH {
                    let kind = if t == 0x1A {
                        Kind::Compound(
                            Ch {
                                m: Affine::IDENTITY,
                                ..ch
                            },
                            BezPath::new(),
                        )
                    } else {
                        Kind::Group
                    };
                    let m = st.parent() * ch.m;
                    st.stack.push(Frame {
                        remaining: ext,
                        m,
                        kind,
                    });
                }
            }
            0x1B => {
                let Some(ch) = st.ch(&mut b) else { continue };
                let v: Vec<f64> = (0..4).map_while(|_| st.coord(&mut b)).collect();
                if v.len() == 4 {
                    let t = st.to_page * st.parent() * ch.m;
                    st.bitmap = Some((Rect::new(v[0], v[1], v[2], v[3]).abs(), t));
                }
            }
            0x0E => {
                if bitmap_data(&mut b, &mut st, &mut ops, facts) {
                    facts.bitmaps += 1;
                }
            }
            _ => {
                if let Some(name) = skip_name(t) {
                    facts.skip(name);
                }
            }
        }
        // A record with members that is not a group still owns them (text data, palettes).
        if ext > 0 && st.stack.len() == depth_before && st.stack.len() < MAX_DEPTH {
            let m = st.parent();
            st.stack.push(Frame {
                remaining: ext,
                m,
                kind: Kind::Other,
            });
        }
        if st.stack.len() == depth_before {
            st.pop_done(&mut ops);
        }
    }
    // Compounds left open at the end are drawn.
    while let Some(f) = st.stack.last_mut() {
        f.remaining = 0;
        st.pop_done(&mut ops);
    }
    let size = size.ok_or_else(|| Error::Malformed("no Start WPG record".into()))?;
    let mut page = Page::new("", size);
    page.ops = ops;
    Ok(page)
}

/// Bitmap Data: width, height, depth code (1, 2, 3, 4, 8 for 1, 2, 4, 8, 24 bits per
/// pixel), compression (0 none, 1 run-length), pixels top-down; placed in the last Bitmap
/// record's box.
fn bitmap_data(b: &mut R, st: &mut State, ops: &mut Vec<Op>, facts: &mut Facts) -> bool {
    let (Some(w), Some(h), Some(depth), Some(comp)) = (b.u16(), b.u16(), b.u8(), b.u8()) else {
        return false;
    };
    let bits: u32 = match depth {
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 8,
        8 | 24 => 24,
        _ => {
            facts.skip("bitmaps of an unsupported depth");
            return false;
        }
    };
    let Some((rect, t)) = st.bitmap.take() else {
        facts.skip("bitmaps without a position");
        return false;
    };
    let (w, h) = (usize::from(w), usize::from(h));
    if w == 0 || h == 0 {
        return false;
    }
    st.pixels += (w * h) as u64;
    if st.pixels > MAX_IMAGE_PIXELS {
        facts.limited = true;
        return false;
    }
    let stride = (w * bits as usize).div_ceil(8);
    let mut data = b.rest();
    let raw: Vec<u8> = match comp {
        0 => {
            // Some writers start raw data with a sample-size token as well.
            if data.len() >= stride * h + 2 && data[0] == 0x7D {
                data = &data[2..];
            }
            data[..data.len().min(stride * h)].to_vec()
        }
        1 => match rle2(data, stride, h, if bits == 24 { 3 } else { 1 }) {
            Ok(raw) => raw,
            Err(n) => {
                facts.refused_memory(n);
                return false;
            }
        },
        _ => {
            facts.skip("bitmaps with an unknown compression");
            return false;
        }
    };
    let px = match to_rgba(&raw, stride, w, h, bits, &st.palette) {
        Ok(Some(px)) => px,
        Ok(None) => return false,
        Err(n) => {
            facts.refused_memory(n);
            return false;
        }
    };
    let Some(img) = Image::from_rgba(w as u32, h as u32, px) else {
        return false;
    };
    // Image pixels → WPG space (y up: the first row at the top of the box) → page.
    let m = Affine::new([
        rect.width() / w as f64,
        0.0,
        0.0,
        -rect.height() / h as f64,
        rect.x0,
        rect.y1,
    ]);
    let tf = t * m;
    if !(tf.determinant().is_finite() && tf.determinant() != 0.0) {
        return false;
    }
    ops.push(Op::Image {
        image: Arc::new(img),
        transform: tf,
        opacity: 1.0,
    });
    true
}

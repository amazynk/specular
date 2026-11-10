//! WPG version 1 records.

use std::sync::Arc;

use specular_drawpage::kurbo::{Arc as KArc, Ellipse, Point, Vec2};
use specular_drawpage::{
    Affine, BezPath, Cap, HAlign, Image, Join, MAX_IMAGE_PIXELS, Op, Page, Paint, Paragraph, Rgba,
    Run, Shape, Stroke, TextBlock, VAlign,
};

use crate::bitmap::{rle1, rle1_stride, to_rgba};
use crate::palette::default_palette;
use crate::reader::R;
use crate::{Error, Facts, MAX_POINTS, MAX_RECORDS, page_fit};

/// WPG 1 units per inch.
const UNITS: f64 = 1200.0;

struct State {
    palette: Vec<Rgba>,
    fill_style: u8,
    fill_color: u8,
    line_style: u8,
    line_color: u8,
    /// WPG units.
    line_width: f64,
    text_height: f64,
    text_color: u8,
    text_angle: f64,
    text_halign: u8,
    /// WPG units → page points.
    to_page: Affine,
    /// Points → page points (line widths, text sizes).
    k: f64,
    points: usize,
    pixels: u64,
}

impl State {
    fn color(&self, i: u8) -> Rgba {
        self.palette
            .get(usize::from(i))
            .copied()
            .unwrap_or([0, 0, 0, 255])
    }

    fn stroke(&self) -> Option<(Stroke, Paint)> {
        if self.line_style == 0 {
            return None;
        }
        let w = self.line_width * self.k;
        let mut s = Stroke::new(w).with_caps(Cap::Butt).with_join(Join::Miter);
        // Dash and gap lengths in line widths (as LibreOffice's import draws them).
        let unit = w.max(1.0);
        let dashes: &[f64] = match self.line_style {
            2 => &[1.18, 0.53],
            3 | 7 => &[0.53, 0.53],
            4 => &[0.71, 0.53, 0.53, 0.53],
            5 => &[0.71, 0.71],
            6 => &[0.71, 0.53, 0.53, 0.53, 0.53, 0.53],
            _ => &[],
        };
        if !dashes.is_empty() {
            s = s.with_dashes(0.0, dashes.iter().map(|d| d * unit));
        }
        Some((s, Paint::Solid(self.color(self.line_color))))
    }

    fn fill(&self) -> Option<Paint> {
        (self.fill_style != 0).then(|| Paint::Solid(self.color(self.fill_color)))
    }

    /// Fills (when `closed` and the fill style is not hollow) and strokes a path in WPG units.
    fn draw(&self, ops: &mut Vec<Op>, mut path: BezPath, closed: bool) {
        path.apply_affine(self.to_page);
        if closed && let Some(p) = self.fill() {
            ops.push(Op::Fill {
                path: path.clone(),
                even_odd: true,
                paint: p,
            });
        }
        if let Some((stroke, paint)) = self.stroke() {
            ops.push(Op::Stroke {
                path,
                stroke,
                paint,
            });
        }
    }

    fn take_points(&mut self, n: usize, facts: &mut Facts) -> bool {
        self.points += n;
        if self.points > MAX_POINTS {
            facts.limited = true;
            return false;
        }
        true
    }
}

fn pt(r: &mut R) -> Option<Point> {
    Some(Point::new(f64::from(r.i16()?), f64::from(r.i16()?)))
}

fn points(r: &mut R, n: usize) -> Option<Vec<Point>> {
    if n.checked_mul(4)? > r.left() {
        return None;
    }
    (0..n).map(|_| pt(r)).collect()
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

fn record_name(t: u8) -> &'static str {
    match t {
        0x04 => "polymarkers",
        0x11 | 0x1B => "PostScript data",
        0x16 => "charts",
        0x17 => "PlanPerfect data",
        0x18 => "text (type 2)",
        0x1A => "text (type 3)",
        _ => "unknown records",
    }
}

pub(crate) fn parse(data: &[u8], facts: &mut Facts) -> Result<Page, Error> {
    let mut r = R::new(data);
    let mut st = State {
        palette: default_palette(),
        fill_style: 1,
        fill_color: 0,
        line_style: 1,
        line_color: 0,
        line_width: 0.0,
        text_height: 240.0,
        text_color: 0,
        text_angle: 0.0,
        text_halign: 0,
        // Until a picture size says otherwise: 1/1200 inch, y up, origin at the top left.
        to_page: Affine::new([72.0 / UNITS, 0.0, 0.0, -72.0 / UNITS, 0.0, 0.0]),
        k: 72.0 / UNITS,
        points: 0,
        pixels: 0,
    };
    let mut ops: Vec<Op> = Vec::new();
    let mut size: Option<(f64, f64)> = None;
    let mut started = false;
    let mut count = 0usize;
    while r.left() > 0 {
        count += 1;
        if count > MAX_RECORDS {
            facts.limited = true;
            break;
        }
        let Some(t) = r.u8() else { break };
        let Some(len) = r.varint() else {
            facts.truncated = true;
            break;
        };
        let len = len as usize;
        let body = match r.bytes(len) {
            Some(b) => b,
            None => {
                facts.truncated = true;
                r.rest()
            }
        };
        facts.records += 1;
        let mut b = R::new(body);
        match t {
            0x0F => {
                // Start WPG: version, flags, width, height (1/1200 inch).
                let _ = (b.u8(), b.u8());
                let (w, h) = (b.u16().unwrap_or(0), b.u16().unwrap_or(0));
                if !started {
                    started = true;
                    if w > 0 && h > 0 {
                        let (w, h) = (f64::from(w), f64::from(h));
                        let k = 72.0 / UNITS * page_fit(w * 72.0 / UNITS, h * 72.0 / UNITS);
                        size = Some((w * k, h * k));
                        st.k = k;
                        st.to_page = Affine::new([k, 0.0, 0.0, -k, 0.0, h * k]);
                    }
                }
            }
            0x10 => break,
            0x01 => {
                st.fill_style = b.u8().unwrap_or(1);
                st.fill_color = b.u8().unwrap_or(0);
            }
            0x02 => {
                st.line_style = b.u8().unwrap_or(1);
                st.line_color = b.u8().unwrap_or(0);
                st.line_width = f64::from(b.u16().unwrap_or(0));
            }
            0x03 | 0x12 | 0x15 | 0x19 => {}
            0x0D => {
                // Graphics text attributes: cell width, height, …, font, …, alignment,
                // colour, angle.
                let _w = b.u16();
                if let Some(h) = b.u16().filter(|&h| h > 0) {
                    st.text_height = f64::from(h);
                }
                if body.len() > 17 {
                    st.text_halign = body[17];
                }
                if let Some(&c) = body.get(20) {
                    st.text_color = c;
                }
                if let Some(a) = body.get(21..23) {
                    st.text_angle = f64::from(u16::from_le_bytes([a[0], a[1]]) % 360);
                }
            }
            0x0E => {
                let (Some(start), Some(n)) = (b.u16(), b.u16()) else {
                    continue;
                };
                for i in 0..usize::from(n) {
                    let (Some(cr), Some(cg), Some(cb)) = (b.u8(), b.u8(), b.u8()) else {
                        break;
                    };
                    let idx = usize::from(start) + i;
                    if idx < 256 {
                        st.palette[idx] = [cr, cg, cb, 255];
                    }
                }
            }
            0x05 => {
                if let (Some(a), Some(c)) = (pt(&mut b), pt(&mut b))
                    && st.take_points(2, facts)
                {
                    st.draw(&mut ops, poly(&[a, c], false), false);
                }
            }
            0x06 | 0x08 => {
                let n = usize::from(b.u16().unwrap_or(0));
                if !st.take_points(n, facts) {
                    continue;
                }
                if let Some(pts) = points(&mut b, n) {
                    let closed = t == 0x08;
                    st.draw(&mut ops, poly(&pts, closed), closed);
                }
            }
            0x07 => {
                if let (Some(x), Some(y), Some(w), Some(h)) = (b.i16(), b.i16(), b.i16(), b.i16()) {
                    let (x, y, w, h) = (f64::from(x), f64::from(y), f64::from(w), f64::from(h));
                    let rect = specular_drawpage::Rect::new(x, y, x + w, y + h);
                    st.draw(&mut ops, rect.to_path(0.1), true);
                }
            }
            0x09 => ellipse(&mut b, &st, &mut ops),
            0x13 => {
                let _ = b.u32();
                let n = usize::from(b.u16().unwrap_or(0));
                if !st.take_points(n, facts) {
                    continue;
                }
                if let Some(pts) = points(&mut b, n)
                    && let Some(first) = pts.first()
                {
                    let mut p = BezPath::new();
                    p.move_to(*first);
                    for c in pts[1..].chunks_exact(3) {
                        p.curve_to(c[0], c[1], c[2]);
                    }
                    st.draw(&mut ops, p, false);
                }
            }
            0x0B | 0x14 => {
                if bitmap(t, &mut b, &mut st, size, &mut ops, facts) {
                    facts.bitmaps += 1;
                }
            }
            0x0C => text(&mut b, &st, &mut ops),
            0x11 | 0x1B => {
                facts.postscript = true;
                facts.skip(record_name(t));
            }
            _ => facts.skip(record_name(t)),
        }
    }
    if !started {
        return Err(Error::Malformed("no Start WPG record".into()));
    }
    let size = match size {
        Some(s) => s,
        None => {
            // No picture size: the drawing's extent.
            let b = specular_drawpage::content_bounds(&ops)
                .ok_or_else(|| Error::Malformed("an empty picture without a size".into()))?;
            specular_drawpage::translate_ops(&mut ops, Vec2::new(-b.x0, -b.y0));
            (b.width().max(1.0), b.height().max(1.0))
        }
    };
    let mut page = Page::new("", size);
    page.ops = ops;
    Ok(page)
}

fn ellipse(b: &mut R, st: &State, ops: &mut Vec<Op>) {
    let (Some(cx), Some(cy), Some(rx), Some(ry)) = (b.i16(), b.i16(), b.i16(), b.i16()) else {
        return;
    };
    let rot = f64::from(b.u16().unwrap_or(0) % 360).to_radians();
    let a0 = b.u16().unwrap_or(0) % 361;
    let a1 = b.u16().unwrap_or(0) % 361;
    let flags = b.u16().unwrap_or(0);
    let c = Point::new(f64::from(cx), f64::from(cy));
    let radii = Vec2::new(f64::from(rx), f64::from(ry));
    let full = a0 == a1 || a1.abs_diff(a0) == 360;
    if full {
        let path = Ellipse::new(c, radii, rot).to_path(0.25);
        st.draw(ops, path, true);
        return;
    }
    let start = f64::from(a0).to_radians();
    let mut sweep = f64::from(a1).to_radians() - start;
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    let arc = KArc::new(c, radii, start, sweep, rot);
    let mut path = BezPath::new();
    let closed = flags != 0;
    if closed {
        path.move_to(c);
        for e in arc.path_elements(0.25) {
            match e {
                specular_drawpage::kurbo::PathEl::MoveTo(p) => path.line_to(p),
                e => path.push(e),
            }
        }
        path.close_path();
    } else {
        path = arc.to_path(0.25);
    }
    st.draw(ops, path, closed);
}

/// Bitmap type 1 (at its resolution from the picture's top left) or type 2 (in its box).
fn bitmap(
    t: u8,
    b: &mut R,
    st: &mut State,
    size: Option<(f64, f64)>,
    ops: &mut Vec<Op>,
    facts: &mut Facts,
) -> bool {
    let rect = if t == 0x14 {
        let _rot = b.u16();
        let (Some(x1), Some(y1), Some(x2), Some(y2)) = (b.i16(), b.i16(), b.i16(), b.i16()) else {
            return false;
        };
        Some((f64::from(x1), f64::from(y1), f64::from(x2), f64::from(y2)))
    } else {
        None
    };
    let (Some(w), Some(h), Some(depth), Some(hres), Some(vres)) =
        (b.u16(), b.u16(), b.u16(), b.u16(), b.u16())
    else {
        return false;
    };
    let (w, h) = (usize::from(w), usize::from(h));
    if w == 0 || h == 0 || !matches!(depth, 1 | 2 | 4 | 8) {
        facts.skip("bitmaps of an unsupported depth");
        return false;
    }
    st.pixels += (w * h) as u64;
    if st.pixels > MAX_IMAGE_PIXELS {
        facts.limited = true;
        return false;
    }
    let row = (w * usize::from(depth)).div_ceil(8);
    let data = b.rest();
    let stride = rle1_stride(data, row, h);
    let raw = match rle1(data, stride, h) {
        Ok((raw, _)) => raw,
        Err(n) => {
            facts.refused_memory(n);
            return false;
        }
    };
    // One-bit bitmaps are black and white whatever the colour map says.
    let bw = [[0, 0, 0, 255], [255, 255, 255, 255]];
    let pal: &[Rgba] = if depth == 1 { &bw } else { &st.palette };
    let rgba = match to_rgba(&raw, stride, w, h, u32::from(depth), pal) {
        Ok(Some(rgba)) => rgba,
        Ok(None) => return false,
        Err(n) => {
            facts.refused_memory(n);
            return false;
        }
    };
    let Some(img) = Image::from_rgba(w as u32, h as u32, rgba) else {
        return false;
    };
    // The box in WPG units (y up).
    let (x1, y1, x2, y2) = rect.unwrap_or_else(|| {
        // Type 1: at its resolution, from the picture's top left (or the origin without a
        // picture size).
        let top = size.map_or(0.0, |(_, ph)| ph / st.k);
        let dw = w as f64 / f64::from(hres.max(1)) * UNITS;
        let dh = h as f64 / f64::from(vres.max(1)) * UNITS;
        (0.0, top - dh, dw, top)
    });
    let p0 = st.to_page * Point::new(x1.min(x2), y1.max(y2));
    let p1 = st.to_page * Point::new(x1.max(x2), y1.min(y2));
    let (bw_, bh_) = (p1.x - p0.x, p1.y - p0.y);
    if !(bw_.abs() > 0.0 && bh_.abs() > 0.0) {
        return false;
    }
    let m = Affine::new([bw_ / w as f64, 0.0, 0.0, bh_ / h as f64, p0.x, p0.y]);
    ops.push(Op::Image {
        image: Arc::new(img),
        transform: m,
        opacity: 1.0,
    });
    true
}

/// Graphics text (type 1): length, origin, characters (WordPerfect's extended
/// characters drawn as Latin-1).
fn text(b: &mut R, st: &State, ops: &mut Vec<Op>) {
    let (Some(len), Some(x), Some(y)) = (b.u16(), b.i16(), b.i16()) else {
        return;
    };
    let Some(bytes) = b.bytes(usize::from(len)).or_else(|| Some(b.rest())) else {
        return;
    };
    let s: String = bytes
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| char::from(c))
        .collect();
    if s.trim().is_empty() {
        return;
    }
    let size = (st.text_height * st.k).max(0.5);
    let p = st.to_page * Point::new(f64::from(x), f64::from(y));
    let width = size * s.chars().count() as f64 * 2.0 + 10.0;
    let align = match st.text_halign {
        1 => HAlign::Centre,
        2 => HAlign::Right,
        _ => HAlign::Left,
    };
    let dx = match align {
        HAlign::Centre => -width / 2.0,
        HAlign::Right => -width,
        _ => 0.0,
    };
    // The point is the text's top left (as LibreOffice's import places it).
    let rot = -st.text_angle.to_radians();
    let tf = Affine::translate((p.x, p.y)) * Affine::rotate(rot) * Affine::translate((dx, 0.0));
    let mut tb = TextBlock::new(tf, width, size * 1.2);
    tb.wrap = false;
    tb.valign = VAlign::Top;
    let mut run = Run::new(s, "Arial", size);
    run.color = st.color(st.text_color);
    tb.paragraphs.push(Paragraph {
        runs: vec![run],
        align,
        line_height: Some(size * 1.2),
        ..Paragraph::default()
    });
    ops.push(Op::Text(Box::new(tb)));
}

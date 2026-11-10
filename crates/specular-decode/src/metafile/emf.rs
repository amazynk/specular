//! EMF: enhanced metafiles, played into the GDI device context of [`super::Gdi`]; EMF+ records
//! in comment records go to [`super::plus`].
//!
//! Op space is the reference device's pixels (`szlDevice` / `szlMillimeters`); the picture is
//! the header's frame (0.01 mm), or its device bounds when the frame is empty.

use super::plus::Plus;
use super::{
    ArcKind, Brush, Gdi, GdiPen, Picture, arc_end, arc_to, average, beziers, colorref, dib,
    dib_bottom_up, ellipse, f32le, finish, gdi_arc, i32le, polygon, probe_emf, rect, round_rect,
    u16le, u32le,
};
use crate::{DecodeError, Result};
use resvg::tiny_skia as sk;
use specular_core::CancelToken;

/// Most entries of the object table the header may ask for.
const MAX_HANDLES: usize = 1 << 16;

enum Obj {
    Pen(GdiPen),
    Brush(Brush),
    Other,
}

fn i(p: &[u8], at: usize) -> f32 {
    i32le(p, at).unwrap_or(0) as f32
}

fn u(p: &[u8], at: usize) -> u32 {
    u32le(p, at).unwrap_or(0)
}

/// `n` points from byte `at`: 32-bit pairs, or 16-bit ones for the `*16` records.
fn points(p: &[u8], at: usize, n: u32, small: bool) -> Vec<(f32, f32)> {
    let sz = if small { 4 } else { 8 };
    let n = (n as usize).min(p.len().saturating_sub(at) / sz);
    (0..n)
        .map(|k| {
            let o = at + k * sz;
            if small {
                (
                    super::i16le(p, o).unwrap_or(0) as f32,
                    super::i16le(p, o + 2).unwrap_or(0) as f32,
                )
            } else {
                (i(p, o), i(p, o + 4))
            }
        })
        .collect()
}

fn xform(p: &[u8], at: usize) -> Option<sk::Transform> {
    let f = |k: usize| f32le(p, at + 4 * k);
    let t = sk::Transform::from_row(f(0)?, f(1)?, f(2)?, f(3)?, f(4)?, f(5)?);
    t.is_finite().then_some(t)
}

/// Page (logical) → device for the map mode, window and viewport, after the world transform.
fn update(gdi: &mut Gdi, dpm: (f32, f32)) {
    let dc = &mut gdi.dc;
    let fixed = |mm: f32| (dpm.0 * mm, -dpm.1 * mm);
    let (sx, sy) = match dc.map_mode {
        2 => fixed(0.1),
        3 => fixed(0.01),
        4 => fixed(0.254),
        5 => fixed(0.0254),
        6 => fixed(25.4 / 1440.0),
        7 | 8 => {
            let (mut sx, mut sy) = (dc.vp_ext.0 / dc.win_ext.0, dc.vp_ext.1 / dc.win_ext.1);
            if !(sx.is_finite() && sy.is_finite() && sx != 0.0 && sy != 0.0) {
                (sx, sy) = (1.0, 1.0);
            }
            if dc.map_mode == 7 {
                let m = sx.abs().min(sy.abs());
                (sx, sy) = (m.copysign(sx), m.copysign(sy));
            }
            (sx, sy)
        }
        _ => (1.0, 1.0),
    };
    let page = sk::Transform::from_row(
        sx,
        0.0,
        0.0,
        sy,
        dc.vp_org.0 - dc.win_org.0 * sx,
        dc.vp_org.1 - dc.win_org.1 * sy,
    );
    let xf = page.pre_concat(dc.world);
    if xf.is_finite() {
        dc.xf = xf;
    }
}

/// A DIB stored as BITMAPINFO and bits at record offsets.
fn record_dib(
    rec: &[u8],
    (off_bmi, cb_bmi, off_bits, cb_bits): (u32, u32, u32, u32),
) -> Option<(Vec<u8>, usize)> {
    let get = |o: u32, n: u32| rec.get(o as usize..(o as usize).checked_add(n as usize)?);
    let bmi = get(off_bmi, cb_bmi)?;
    let bits = get(off_bits, cb_bits)?;
    let mut buf = Vec::with_capacity(bmi.len() + bits.len());
    buf.extend_from_slice(bmi);
    buf.extend_from_slice(bits);
    Some((buf, bmi.len()))
}

pub(crate) fn parse(d: &[u8], cancel: &CancelToken) -> Result<(Picture, &'static str)> {
    if !probe_emf(d) {
        return Err(DecodeError::UnsupportedFormat);
    }
    let hsize = u(d, 4) as usize;
    if !(88..=d.len()).contains(&hsize) {
        return Err(DecodeError::Truncated("EMF header".into()));
    }
    let bounds = (i(d, 8), i(d, 12), i(d, 16), i(d, 20));
    let frame = (i(d, 24), i(d, 28), i(d, 32), i(d, 36));
    let handles = (u16le(d, 56).unwrap_or(0) as usize).clamp(1, MAX_HANDLES);
    let (dev, mm) = ((i(d, 72), i(d, 76)), (i(d, 80), i(d, 84)));
    let dpm = if dev.0 > 0.0 && dev.1 > 0.0 && mm.0 > 0.0 && mm.1 > 0.0 {
        (dev.0 / mm.0, dev.1 / mm.1)
    } else {
        (96.0 / 25.4, 96.0 / 25.4)
    };
    // The picture: the frame (0.01 mm) in device pixels, sized at 96 dpi.
    let px = 96.0 / 25.4;
    let pic = if frame.2 > frame.0 && frame.3 > frame.1 {
        sk::Rect::from_ltrb(
            frame.0 / 100.0 * dpm.0,
            frame.1 / 100.0 * dpm.1,
            frame.2 / 100.0 * dpm.0,
            frame.3 / 100.0 * dpm.1,
        )
        .map(|r| {
            (
                r,
                (frame.2 - frame.0) / 100.0 * px,
                (frame.3 - frame.1) / 100.0 * px,
            )
        })
    } else if bounds.2 >= bounds.0 && bounds.3 >= bounds.1 {
        sk::Rect::from_ltrb(bounds.0, bounds.1, bounds.2 + 1.0, bounds.3 + 1.0)
            .map(|r| (r, r.width() / dpm.0 * px, r.height() / dpm.1 * px))
    } else {
        None
    };

    let mut gdi = Gdi::new();
    update(&mut gdi, dpm);
    let mut objs: Vec<Option<Obj>> = Vec::new();
    objs.resize_with(handles.min(d.len() / 8 + 1), || None);
    let mut plus = Plus::new(pic.map(|p| p.0));
    let mut at = hsize;
    let mut n = 0usize;
    while let (Some(typ), Some(size)) = (u32le(d, at), u32le(d, at + 4)) {
        let size = size as usize;
        let Some(rec) = d.get(at..at.saturating_add(size)).filter(|_| size >= 8) else {
            break;
        };
        at += size;
        n += 1;
        if n.is_multiple_of(4096) && cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        if typ == 14 {
            break;
        }
        if typ == 70 {
            // GDICOMMENT: EMF+ records, or anything else (ignored).
            let cb = u(rec, 8) as usize;
            if let Some(data) = rec.get(12..12usize.saturating_add(cb))
                && data.starts_with(b"EMF+")
            {
                plus.play(&data[4..], &mut gdi.list, cancel);
                gdi.draw = !plus.active || plus.getdc;
            }
            continue;
        }
        record(&mut gdi, &mut objs, dpm, typ, rec);
    }
    if cancel.is_cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let kind = match gdi.list.notes.plus {
        Some(true) => "EMF+ (dual)",
        Some(false) => "EMF+",
        None => "EMF",
    };
    Ok((finish(std::mem::take(&mut gdi.list), pic)?, kind))
}

fn record(gdi: &mut Gdi, objs: &mut [Option<Obj>], dpm: (f32, f32), typ: u32, rec: &[u8]) {
    let p = &rec[8..];
    let ltrb = |at: usize| (i(p, at), i(p, at + 4), i(p, at + 8), i(p, at + 12));
    match typ {
        // POLYBEZIER, POLYGON, POLYLINE, POLYBEZIERTO, POLYLINETO (and their 16-bit forms)
        2..=6 | 85..=89 => {
            let pts = points(p, 20, u(p, 16), typ >= 85);
            match if typ >= 85 { typ - 83 } else { typ } {
                2 => gdi.shape(beziers(&pts), false),
                3 => gdi.shape(polygon(&pts, true), true),
                4 => gdi.shape(polygon(&pts, false), false),
                5 => gdi.line_to(&pts, true),
                _ => gdi.line_to(&pts, false),
            }
        }
        // POLYPOLYLINE, POLYPOLYGON (and 16-bit)
        7 | 8 | 90 | 91 => {
            let small = typ >= 90;
            let closed = matches!(typ, 8 | 91);
            let polys = (u(p, 16) as usize).min(p.len() / 4);
            let mut at = 24 + 4 * polys;
            let mut pb = sk::PathBuilder::new();
            for k in 0..polys {
                let count = u(p, 24 + 4 * k);
                let pts = points(p, at, count, small);
                at = at.saturating_add(count as usize * if small { 4 } else { 8 });
                if let Some(path) = polygon(&pts, closed) {
                    pb.push_path(&path);
                }
            }
            gdi.shape(pb.finish(), closed);
        }
        // SETWINDOWEXTEX, SETWINDOWORGEX, SETVIEWPORTEXTEX, SETVIEWPORTORGEX
        9 => {
            if i(p, 0) != 0.0 && i(p, 4) != 0.0 {
                gdi.dc.win_ext = (i(p, 0), i(p, 4));
            }
            update(gdi, dpm);
        }
        10 => {
            gdi.dc.win_org = (i(p, 0), i(p, 4));
            update(gdi, dpm);
        }
        11 => {
            if i(p, 0) != 0.0 && i(p, 4) != 0.0 {
                gdi.dc.vp_ext = (i(p, 0), i(p, 4));
            }
            update(gdi, dpm);
        }
        12 => {
            gdi.dc.vp_org = (i(p, 0), i(p, 4));
            update(gdi, dpm);
        }
        // SCALEVIEWPORTEXTEX, SCALEWINDOWEXTEX
        31 | 32 => {
            let (xn, xd, yn, yd) = (i(p, 0), i(p, 4), i(p, 8), i(p, 12));
            if xn != 0.0 && xd != 0.0 && yn != 0.0 && yd != 0.0 {
                let e = if typ == 31 {
                    &mut gdi.dc.vp_ext
                } else {
                    &mut gdi.dc.win_ext
                };
                *e = (e.0 * xn / xd, e.1 * yn / yd);
                update(gdi, dpm);
            }
        }
        // SETMAPMODE, SETPOLYFILLMODE, SETARCDIRECTION
        17 => {
            if (1..=8).contains(&u(p, 0)) {
                gdi.dc.map_mode = u(p, 0);
                update(gdi, dpm);
            }
        }
        19 => gdi.dc.even_odd = u(p, 0) == 1,
        57 => gdi.dc.clockwise = u(p, 0) == 2,
        // SAVEDC, RESTOREDC
        33 => gdi.save(),
        34 => gdi.restore(i32le(p, 0).unwrap_or(0)),
        // SETWORLDTRANSFORM, MODIFYWORLDTRANSFORM
        35 => {
            if let Some(x) = xform(p, 0) {
                gdi.dc.world = x;
                update(gdi, dpm);
            }
        }
        36 => {
            let x = xform(p, 0);
            let w = gdi.dc.world;
            gdi.dc.world = match (u(p, 24), x) {
                (1, _) => sk::Transform::identity(),
                (2, Some(x)) => w.pre_concat(x),
                (3, Some(x)) => w.post_concat(x),
                (4, Some(x)) => x,
                _ => w,
            };
            update(gdi, dpm);
        }
        // MOVETOEX, LINETO
        27 => gdi.move_to(i(p, 0), i(p, 4)),
        54 => gdi.line_to(&[(i(p, 0), i(p, 4))], false),
        // ELLIPSE, RECTANGLE, ROUNDRECT
        42 => {
            let (l, t, r, b) = ltrb(0);
            gdi.shape(ellipse(l, t, r, b), true);
        }
        43 => {
            let (l, t, r, b) = ltrb(0);
            gdi.shape(rect(l, t, r, b), true);
        }
        44 => {
            let (l, t, r, b) = ltrb(0);
            gdi.shape(round_rect(l, t, r, b, i(p, 16), i(p, 20)), true);
        }
        // ARC, CHORD, PIE, ARCTO
        45..=47 | 55 => {
            let kind = match typ {
                46 => ArcKind::Chord,
                47 => ArcKind::Pie,
                _ => ArcKind::Arc,
            };
            let b = ltrb(0);
            let (s, e) = ((i(p, 16), i(p, 20)), (i(p, 24), i(p, 28)));
            let lead = (typ == 55).then_some(gdi.dc.cur);
            let path = gdi_arc(kind, b, s, e, gdi.dc.clockwise, lead);
            gdi.shape(path, kind != ArcKind::Arc);
            if typ == 55 {
                gdi.dc.cur = arc_end(b, e);
            }
        }
        // ANGLEARC: a line from the current position, then the arc (degrees, counter-clockwise).
        41 => {
            let (cx, cy, r) = (i(p, 0), i(p, 4), u(p, 8) as f32);
            let (Some(a0), Some(sw)) = (f32le(p, 12), f32le(p, 16)) else {
                return;
            };
            let (t0, sweep) = (-a0.to_radians(), -sw.to_radians().clamp(-10.0, 10.0));
            let mut pb = sk::PathBuilder::new();
            pb.move_to(gdi.dc.cur.0, gdi.dc.cur.1);
            pb.line_to(cx + r * t0.cos(), cy + r * t0.sin());
            arc_to(&mut pb, cx, cy, r, r, t0, sweep);
            gdi.shape(pb.finish(), false);
            let t1 = t0 + sweep;
            gdi.dc.cur = (cx + r * t1.cos(), cy + r * t1.sin());
        }
        // SELECTOBJECT: a table entry or a stock object.
        37 => {
            let ih = u(p, 0);
            if ih & 0x8000_0000 != 0 {
                let grey = |v| Brush::Solid(sk::Color::from_rgba8(v, v, v, 255));
                match ih & 0x7fff_ffff {
                    0 | 18 => gdi.dc.brush = grey(255),
                    1 => gdi.dc.brush = grey(192),
                    2 => gdi.dc.brush = grey(128),
                    3 => gdi.dc.brush = grey(64),
                    4 => gdi.dc.brush = grey(0),
                    5 => gdi.dc.brush = Brush::Null,
                    6 => {
                        gdi.dc.pen = GdiPen {
                            color: sk::Color::WHITE,
                            ..GdiPen::black()
                        }
                    }
                    7 | 19 => gdi.dc.pen = GdiPen::black(),
                    8 => gdi.dc.pen = GdiPen::null(),
                    _ => {}
                }
            } else {
                match objs.get(ih as usize) {
                    Some(Some(Obj::Pen(pen))) => gdi.dc.pen = pen.clone(),
                    Some(Some(Obj::Brush(b))) => gdi.dc.brush = b.clone(),
                    _ => {}
                }
            }
        }
        // CREATEPEN, EXTCREATEPEN, CREATEBRUSHINDIRECT, the pattern brushes, other objects
        38 | 39 | 93 | 94 | 95 | 49 | 82 => {
            let obj = match typ {
                38 => Obj::Pen(GdiPen {
                    style: u(p, 4),
                    width: i(p, 8).abs(),
                    color: colorref(u(p, 16)),
                }),
                95 => {
                    if u(p, 28) == 1 {
                        Obj::Pen(GdiPen::null())
                    } else {
                        let style = u(p, 20);
                        Obj::Pen(GdiPen {
                            style,
                            width: if style & 0x10000 != 0 {
                                u(p, 24) as f32
                            } else {
                                0.0
                            },
                            color: colorref(u(p, 32)),
                        })
                    }
                }
                39 => {
                    let c = colorref(u(p, 8));
                    Obj::Brush(match u(p, 4) {
                        1 => Brush::Null,
                        2 => Brush::Hatch(c),
                        _ => Brush::Solid(c),
                    })
                }
                93 | 94 => {
                    let c = record_dib(rec, (u(p, 8), u(p, 12), u(p, 16), u(p, 20)))
                        .filter(|_| u(p, 4) != 1)
                        .and_then(|(b, at)| dib(&b, Some(at)))
                        .and_then(|img| average(&img))
                        .unwrap_or(sk::Color::from_rgba8(128, 128, 128, 255));
                    Obj::Brush(Brush::Solid(c))
                }
                _ => Obj::Other,
            };
            if let Some(slot) = objs.get_mut(u(p, 0) as usize) {
                *slot = Some(obj);
            }
        }
        // DELETEOBJECT
        40 => {
            if let Some(slot) = objs.get_mut(u(p, 0) as usize) {
                *slot = None;
            }
        }
        // Paths
        59 => gdi.begin_path(),
        60 => gdi.end_path(),
        61 => gdi.close_figure(),
        62 => gdi.draw_path(true, false),
        63 => gdi.draw_path(true, true),
        64 => gdi.draw_path(false, true),
        67 => {
            gdi.abort_path();
            gdi.list.notes.clip = true;
        }
        68 => gdi.abort_path(),
        // BITBLT, STRETCHBLT, ALPHABLEND, TRANSPARENTBLT
        76 | 77 | 114 | 116 => {
            let dest = (i(p, 16), i(p, 20), i(p, 24), i(p, 28));
            let rop = u(p, 32);
            let dib_at = (u(p, 76), u(p, 80), u(p, 84), u(p, 88));
            if dib_at.1 == 0 {
                if typ <= 77 {
                    gdi.pat_blt(dest, rop);
                }
                return;
            }
            if !gdi.draw || (typ <= 77 && matches!(rop, 0x00aa_0029 | 0x0055_0009)) {
                return;
            }
            let src_size = if typ == 76 {
                (dest.2, dest.3)
            } else {
                (i(p, 92), i(p, 96))
            };
            let src = (i(p, 36), i(p, 40), src_size.0, src_size.1);
            let opacity = if typ == 114 {
                ((rop >> 16) & 0xff) as f32 / 255.0
            } else {
                1.0
            };
            let key = (typ == 116).then(|| colorref(rop));
            match record_dib(rec, dib_at)
                .filter(|_| u(p, 72) != 1)
                .and_then(|(b, at)| dib(&b, Some(at)))
            {
                Some(img) => gdi.bitmap(&img, src, dest, opacity, key),
                None => gdi.list.notes.skipped_images += 1,
            }
        }
        // SETDIBITSTODEVICE, STRETCHDIBITS
        80 | 81 => {
            let rop = if typ == 81 { u(p, 60) } else { 0x00cc_0020 };
            if !gdi.draw || matches!(rop, 0x00aa_0029 | 0x0055_0009) {
                return;
            }
            let (sw, sh) = (i(p, 32), i(p, 36));
            let dest = if typ == 81 {
                (i(p, 16), i(p, 20), i(p, 64), i(p, 68))
            } else {
                (i(p, 16), i(p, 20), sw, sh)
            };
            let Some((buf, at)) =
                record_dib(rec, (u(p, 40), u(p, 44), u(p, 48), u(p, 52))).filter(|_| u(p, 56) != 1)
            else {
                gdi.list.notes.skipped_images += 1;
                return;
            };
            let Some(img) = dib(&buf, Some(at)) else {
                gdi.list.notes.skipped_images += 1;
                return;
            };
            let mut src = (i(p, 24), i(p, 28), sw, sh);
            if dib_bottom_up(&buf) {
                src.1 = img.height as f32 - src.1 - src.3;
            }
            gdi.bitmap(&img, src, dest, 1.0, None);
        }
        // Text
        83 | 84 | 96 | 97 | 108 => gdi.list.notes.text = true,
        // EXCLUDECLIPRECT, INTERSECTCLIPRECT, EXTSELECTCLIPRGN with a region
        29 | 30 => gdi.list.notes.clip = true,
        75 if u(p, 0) > 0 => gdi.list.notes.clip = true,
        _ => {}
    }
}

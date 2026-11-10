//! WMF: 16-bit Windows metafiles, with or without the "placeable" (Aldus) header, played into
//! the GDI device context of [`super::Gdi`].
//!
//! The picture is the placeable header's bounding box (its size from the header's units per
//! inch) or, without one, the first window extent the file sets; the current window maps onto
//! it. A file with neither is sized by what it draws.

use super::{
    ArcKind, Brush, Gdi, GdiPen, Picture, average, colorref, dib, dib_bottom_up, ellipse, finish,
    gdi_arc, i16le, polygon, rect, round_rect, u16le, u32le,
};
use crate::{DecodeError, Result};
use resvg::tiny_skia as sk;
use specular_core::CancelToken;

pub(crate) const PLACEABLE_KEY: [u8; 4] = [0xd7, 0xcd, 0xc6, 0x9a];
/// Most objects a WMF object table holds (its indices are 16-bit).
const MAX_OBJECTS: usize = 0xffff;

enum Obj {
    Pen(GdiPen),
    Brush(Brush),
    Other,
}

/// Signed parameter word `i`.
fn w(p: &[u8], i: usize) -> f32 {
    i16le(p, 2 * i).unwrap_or(0) as f32
}

fn uw(p: &[u8], i: usize) -> u16 {
    u16le(p, 2 * i).unwrap_or(0)
}

/// `n` points (x, y words) from byte `at`, as many as the record holds.
fn points(p: &[u8], at: usize, n: usize) -> Vec<(f32, f32)> {
    let n = n.min(p.len().saturating_sub(at) / 4);
    (0..n)
        .map(|i| {
            let o = at + 4 * i;
            (
                i16le(p, o).unwrap_or(0) as f32,
                i16le(p, o + 2).unwrap_or(0) as f32,
            )
        })
        .collect()
}

/// The records after the header: (size in words, function, parameters).
fn records(d: &[u8]) -> impl Iterator<Item = (u16, &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let size = u32le(d, at)? as usize;
        let func = u16le(d, at + 4)?;
        let len = size.checked_mul(2)?;
        if size < 3 || func == 0 {
            return None;
        }
        let p = d.get(at + 6..at.checked_add(len)?)?;
        at += len;
        Some((func, p))
    })
}

/// Logical → op space for the current window: onto the picture box when there is one, else
/// only the window origin.
fn update(gdi: &mut Gdi, size: Option<(f32, f32)>) {
    let dc = &mut gdi.dc;
    let (ox, oy) = dc.win_org;
    let xf = match size {
        Some((pw, ph)) => {
            let (ex, ey) = dc.win_ext;
            if ex == 0.0 || ey == 0.0 {
                return;
            }
            let (sx, sy) = (pw / ex, ph / ey);
            sk::Transform::from_row(sx, 0.0, 0.0, sy, -ox * sx, -oy * sy)
        }
        None => sk::Transform::from_translate(-ox, -oy),
    };
    if xf.is_finite() {
        dc.xf = xf;
    }
}

pub(crate) fn parse(d: &[u8], cancel: &CancelToken) -> Result<(Picture, &'static str)> {
    let mut at = 0;
    let mut placeable = None;
    if d.starts_with(&PLACEABLE_KEY) {
        if d.len() < 22 {
            return Err(DecodeError::Truncated("WMF placeable header".into()));
        }
        let v = |o| i16le(d, o).unwrap_or(0) as f32;
        let inch = match u16le(d, 14).unwrap_or(0) {
            0 => 1440.0,
            n => n as f32,
        };
        placeable = Some(((v(6), v(8), v(10), v(12)), inch));
        at = 22;
    }
    let h = d
        .get(at..at + 18)
        .ok_or_else(|| DecodeError::Truncated("WMF header".into()))?;
    if !(matches!(u16le(h, 0), Some(1 | 2)) && u16le(h, 2) == Some(9)) {
        return Err(DecodeError::Malformed("not a WMF header".into()));
    }
    let body = &d[at + 18..];

    let mut gdi = Gdi::new();
    // The picture box and the window that maps onto it.
    let size = match placeable {
        Some(((l, t, r, b), inch)) => {
            gdi.dc.win_org = (l, t);
            gdi.dc.win_ext = (r - l, b - t);
            Some(((r - l).abs() * 96.0 / inch, (b - t).abs() * 96.0 / inch))
        }
        None => records(body)
            .take(100_000)
            .find(|(f, _)| *f == 0x020c)
            .map(|(_, p)| {
                gdi.dc.win_ext = (w(p, 1), w(p, 0));
                (w(p, 1).abs(), w(p, 0).abs())
            }),
    }
    .filter(|&(pw, ph)| pw > 0.0 && ph > 0.0);
    update(&mut gdi, size);

    let mut objs: Vec<Option<Obj>> = Vec::new();
    for (n, (func, p)) in records(body).enumerate() {
        if n.is_multiple_of(4096) && cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        record(&mut gdi, &mut objs, size, func, p);
    }
    let frame = size.and_then(|(pw, ph)| Some((sk::Rect::from_xywh(0.0, 0.0, pw, ph)?, pw, ph)));
    let kind = if placeable.is_some() {
        "WMF (placeable)"
    } else {
        "WMF"
    };
    Ok((finish(std::mem::take(&mut gdi.list), frame)?, kind))
}

fn add(objs: &mut Vec<Option<Obj>>, o: Obj) {
    if let Some(s) = objs.iter_mut().find(|s| s.is_none()) {
        *s = Some(o);
    } else if objs.len() < MAX_OBJECTS {
        objs.push(Some(o));
    }
}

fn record(
    gdi: &mut Gdi,
    objs: &mut Vec<Option<Obj>>,
    size: Option<(f32, f32)>,
    func: u16,
    p: &[u8],
) {
    let words = |i: usize| w(p, i);
    match func {
        // SETPOLYFILLMODE
        0x0106 => gdi.dc.even_odd = uw(p, 0) == 1,
        // SETWINDOWORG, SETWINDOWEXT, OFFSETWINDOWORG, SCALEWINDOWEXT
        0x020b => {
            gdi.dc.win_org = (words(1), words(0));
            update(gdi, size);
        }
        0x020c => {
            if words(0) != 0.0 && words(1) != 0.0 {
                gdi.dc.win_ext = (words(1), words(0));
                update(gdi, size);
            }
        }
        0x020f => {
            gdi.dc.win_org.0 += words(1);
            gdi.dc.win_org.1 += words(0);
            update(gdi, size);
        }
        0x0410 => {
            let (yd, yn, xd, xn) = (words(0), words(1), words(2), words(3));
            if yd != 0.0 && xd != 0.0 && yn != 0.0 && xn != 0.0 {
                gdi.dc.win_ext.0 = gdi.dc.win_ext.0 * xn / xd;
                gdi.dc.win_ext.1 = gdi.dc.win_ext.1 * yn / yd;
                update(gdi, size);
            }
        }
        // MOVETO, LINETO
        0x0214 => gdi.move_to(words(1), words(0)),
        0x0213 => gdi.line_to(&[(words(1), words(0))], false),
        // POLYGON, POLYLINE
        0x0324 => gdi.shape(polygon(&points(p, 2, uw(p, 0) as usize), true), true),
        0x0325 => gdi.shape(polygon(&points(p, 2, uw(p, 0) as usize), false), false),
        // POLYPOLYGON
        0x0538 => {
            let n = uw(p, 0) as usize;
            let mut at = 2 + 2 * n;
            let mut pb = sk::PathBuilder::new();
            for i in 0..n.min(p.len() / 2) {
                let k = uw(p, 1 + i) as usize;
                let pts = points(p, at, k);
                at += 4 * k;
                if let Some(path) = polygon(&pts, true) {
                    pb.push_path(&path);
                }
            }
            gdi.shape(pb.finish(), true);
        }
        // RECTANGLE, ROUNDRECT, ELLIPSE
        0x041b => gdi.shape(rect(words(3), words(2), words(1), words(0)), true),
        0x061c => gdi.shape(
            round_rect(words(5), words(4), words(3), words(2), words(1), words(0)),
            true,
        ),
        0x0418 => gdi.shape(ellipse(words(3), words(2), words(1), words(0)), true),
        // ARC, PIE, CHORD
        0x0817 | 0x081a | 0x0830 => {
            let kind = match func {
                0x0817 => ArcKind::Arc,
                0x081a => ArcKind::Pie,
                _ => ArcKind::Chord,
            };
            let path = gdi_arc(
                kind,
                (words(7), words(6), words(5), words(4)),
                (words(3), words(2)),
                (words(1), words(0)),
                false,
                None,
            );
            gdi.shape(path, kind != ArcKind::Arc);
        }
        // CREATEPENINDIRECT
        0x02fa => add(
            objs,
            Obj::Pen(GdiPen {
                style: uw(p, 0) as u32,
                width: words(1).abs(),
                color: colorref(u32le(p, 6).unwrap_or(0)),
            }),
        ),
        // CREATEBRUSHINDIRECT
        0x02fc => {
            let c = colorref(u32le(p, 2).unwrap_or(0));
            add(
                objs,
                Obj::Brush(match uw(p, 0) {
                    1 => Brush::Null,
                    2 => Brush::Hatch(c),
                    _ => Brush::Solid(c),
                }),
            );
        }
        // DIBCREATEPATTERNBRUSH: the bitmap's average colour.
        0x0142 => {
            let c = p
                .get(4..)
                .filter(|_| uw(p, 0) != 3)
                .and_then(|b| dib(b, None))
                .and_then(|i| average(&i))
                .unwrap_or(sk::Color::from_rgba8(128, 128, 128, 255));
            add(objs, Obj::Brush(Brush::Solid(c)));
        }
        // CREATEPATTERNBRUSH (a device-dependent bitmap): grey.
        0x01f9 => add(
            objs,
            Obj::Brush(Brush::Solid(sk::Color::from_rgba8(128, 128, 128, 255))),
        ),
        // CREATEPALETTE, CREATEFONTINDIRECT, CREATEREGION
        0x00f7 | 0x02fb | 0x06ff => add(objs, Obj::Other),
        // SELECTOBJECT
        0x012d => match objs.get(uw(p, 0) as usize) {
            Some(Some(Obj::Pen(pen))) => gdi.dc.pen = pen.clone(),
            Some(Some(Obj::Brush(b))) => gdi.dc.brush = b.clone(),
            _ => {}
        },
        // DELETEOBJECT
        0x01f0 => {
            if let Some(s) = objs.get_mut(uw(p, 0) as usize) {
                *s = None;
            }
        }
        // SAVEDC, RESTOREDC
        0x001e => gdi.save(),
        0x0127 => gdi.restore(words(0) as i32),
        // TEXTOUT, EXTTEXTOUT
        0x0521 | 0x0a32 => gdi.list.notes.text = true,
        // INTERSECTCLIPRECT, EXCLUDECLIPRECT
        0x0416 | 0x0415 => gdi.list.notes.clip = true,
        // PATBLT
        0x061d => gdi.pat_blt(
            (words(5), words(4), words(3), words(2)),
            u32le(p, 0).unwrap_or(0),
        ),
        // STRETCHDIB
        0x0f43 => blit(
            gdi,
            p.get(22..),
            u32le(p, 0).unwrap_or(0),
            (words(6), words(5), words(4), words(3)),
            (words(10), words(9), words(8), words(7)),
            true,
        ),
        // DIBSTRETCHBLT, with or without a bitmap
        0x0b41 => {
            let rop = u32le(p, 0).unwrap_or(0);
            if p.len() == 2 * 11 {
                gdi.pat_blt((words(10), words(9), words(8), words(7)), rop);
            } else {
                blit(
                    gdi,
                    p.get(20..),
                    rop,
                    (words(5), words(4), words(3), words(2)),
                    (words(9), words(8), words(7), words(6)),
                    false,
                );
            }
        }
        // DIBBITBLT
        0x0940 => {
            let rop = u32le(p, 0).unwrap_or(0);
            if p.len() == 2 * 9 {
                gdi.pat_blt((words(8), words(7), words(6), words(5)), rop);
            } else {
                let (wd, ht) = (words(5), words(4));
                blit(
                    gdi,
                    p.get(16..),
                    rop,
                    (words(3), words(2), wd, ht),
                    (words(7), words(6), wd, ht),
                    false,
                );
            }
        }
        // SETDIBTODEV
        0x0d33 => {
            let (wd, ht) = (words(6), words(5));
            blit(
                gdi,
                p.get(18..),
                0x00cc_0020,
                (words(4), words(3), wd, ht),
                (words(8), words(7), wd, ht),
                true,
            );
        }
        // BITBLT, STRETCHBLT with device-dependent bitmaps
        0x0922 | 0x0b23 => gdi.list.notes.skipped_images += 1,
        // Everything else (palettes, escapes, modes, regions) does not change the drawing here.
        _ => {}
    }
}

/// A DIB blit: `src` is in DIB pixels (from the bottom for StretchDIB-style records when the
/// DIB is bottom-up), `dest` in logical units.
fn blit(
    gdi: &mut Gdi,
    bytes: Option<&[u8]>,
    rop: u32,
    mut src: (f32, f32, f32, f32),
    dest: (f32, f32, f32, f32),
    dib_origin: bool,
) {
    // NOP and DSTINVERT leave nothing to draw.
    if !gdi.draw || matches!(rop, 0x00aa_0029 | 0x0055_0009) {
        return;
    }
    let Some(bytes) = bytes else { return };
    let Some(img) = dib(bytes, None) else {
        gdi.list.notes.skipped_images += 1;
        return;
    };
    if dib_origin && dib_bottom_up(bytes) {
        src.1 = img.height as f32 - src.1 - src.3;
    }
    gdi.bitmap(&img, src, dest, 1.0, None);
}

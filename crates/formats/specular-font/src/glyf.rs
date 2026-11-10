//! TrueType `glyf` outlines: simple glyphs and composites (any depth up to a limit, with
//! their offsets, scales and 2×2 transforms).

use crate::outline::{Outline, PathEl};
use crate::read::{Reader, i16_at, u16_at, u32_at};

/// Deepest composite nesting followed.
const MAX_DEPTH: u32 = 16;
/// Most points one glyph may emit, components included.
const MAX_POINTS: usize = 1 << 18;
/// Most glyph visits and matching points for one glyph: components that each use the same
/// glyph many times over 16 levels would otherwise cost k^16 visits (an empty leaf emits
/// no point, so [`MAX_POINTS`] alone never stops them).
const MAX_WORK: usize = 1 << 20;

/// What drawing one glyph has cost so far.
#[derive(Default)]
struct Work {
    /// Points emitted.
    points: usize,
    /// Glyphs visited and points collected for point matching.
    steps: usize,
}

impl Work {
    fn step(&mut self, n: usize) -> Option<()> {
        self.steps += n;
        (self.steps <= MAX_WORK).then_some(())
    }
}

/// `glyf` and `loca` of a font.
pub(crate) struct Glyf<'a> {
    pub glyf: &'a [u8],
    pub loca: &'a [u8],
    pub long: bool,
    pub num_glyphs: u16,
}

#[derive(Clone, Copy)]
struct Ts {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl Ts {
    const ID: Ts = Ts {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };
    fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
    /// `self` after `o` (`o` first).
    fn then(&self, o: &Ts) -> Ts {
        Ts {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }
    fn is_default(&self) -> bool {
        self.a == 1.0 && self.b == 0.0 && self.c == 0.0 && self.d == 1.0
    }
}

/// Turns TrueType points (on and off the curve) into path commands, with the implied
/// on-curve midpoints between consecutive off-curve points.
#[derive(Default)]
struct Contour {
    path: Vec<PathEl>,
    first_on: Option<(f32, f32)>,
    first_off: Option<(f32, f32)>,
    last_off: Option<(f32, f32)>,
}

fn mid(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * 0.5, a.1 + (b.1 - a.1) * 0.5)
}

impl Contour {
    fn push(&mut self, p: (f32, f32), on: bool, last: bool) {
        if self.first_on.is_none() {
            if on {
                self.first_on = Some(p);
                self.path.push(PathEl::MoveTo(p.0, p.1));
            } else if let Some(off) = self.first_off {
                let m = mid(off, p);
                self.first_on = Some(m);
                self.last_off = Some(p);
                self.path.push(PathEl::MoveTo(m.0, m.1));
            } else {
                self.first_off = Some(p);
            }
        } else {
            match (self.last_off, on) {
                (Some(off), true) => {
                    self.last_off = None;
                    self.path.push(PathEl::QuadTo(off.0, off.1, p.0, p.1));
                }
                (Some(off), false) => {
                    self.last_off = Some(p);
                    let m = mid(off, p);
                    self.path.push(PathEl::QuadTo(off.0, off.1, m.0, m.1));
                }
                (None, true) => self.path.push(PathEl::LineTo(p.0, p.1)),
                (None, false) => self.last_off = Some(p),
            }
        }
        if last {
            self.finish();
        }
    }

    fn finish(&mut self) {
        if let (Some(o1), Some(o2)) = (self.first_off, self.last_off) {
            self.last_off = None;
            let m = mid(o2, o1);
            self.path.push(PathEl::QuadTo(o2.0, o2.1, m.0, m.1));
        }
        match (self.first_on, self.first_off, self.last_off) {
            (Some(p), Some(o1), _) => self.path.push(PathEl::QuadTo(o1.0, o1.1, p.0, p.1)),
            (Some(p), None, Some(o2)) => self.path.push(PathEl::QuadTo(o2.0, o2.1, p.0, p.1)),
            (Some(p), None, None) => self.path.push(PathEl::LineTo(p.0, p.1)),
            _ => {}
        }
        if self.first_on.is_some() {
            self.path.push(PathEl::Close);
        }
        self.first_on = None;
        self.first_off = None;
        self.last_off = None;
    }
}

impl<'a> Glyf<'a> {
    /// The bytes of glyph `gid` (empty for a glyph without outline).
    pub fn data(&self, gid: u16) -> Option<&'a [u8]> {
        if gid >= self.num_glyphs {
            return None;
        }
        let (s, e) = if self.long {
            (
                u32_at(self.loca, gid as usize * 4)? as usize,
                u32_at(self.loca, gid as usize * 4 + 4)? as usize,
            )
        } else {
            (
                u16_at(self.loca, gid as usize * 2)? as usize * 2,
                u16_at(self.loca, gid as usize * 2 + 2)? as usize * 2,
            )
        };
        if s >= e {
            return Some(&[]);
        }
        self.glyf.get(s..e)
    }

    pub fn outline(&self, gid: u16) -> Option<Outline> {
        let mut c = Contour::default();
        let mut work = Work::default();
        self.emit(gid, &Ts::ID, 0, &mut c, &mut work)?;
        Outline::finish(c.path)
    }

    /// The points of glyph `gid` (for point-matching composites), transformed.
    fn points(
        &self,
        gid: u16,
        ts: &Ts,
        depth: u32,
        out: &mut Vec<(f32, f32)>,
        work: &mut Work,
    ) -> Option<()> {
        work.step(1)?;
        let d = self.data(gid)?;
        if d.is_empty() {
            return Some(());
        }
        let n = i16_at(d, 0)?;
        if n >= 0 {
            let before = out.len();
            simple(d, n as usize, |p, _, _| {
                out.push(ts.apply(p.0, p.1));
            })?;
            work.step(out.len() - before)
        } else {
            if depth >= MAX_DEPTH {
                return None;
            }
            components(d, |g, t, _| {
                let t = ts.then(&t);
                self.points(g, &t, depth + 1, out, work)
            })
        }
    }

    fn emit(&self, gid: u16, ts: &Ts, depth: u32, c: &mut Contour, work: &mut Work) -> Option<()> {
        work.step(1)?;
        let d = self.data(gid)?;
        if d.is_empty() {
            return Some(());
        }
        let n = i16_at(d, 0)?;
        if n >= 0 {
            let mut count = 0usize;
            simple(d, n as usize, |p, on, last| {
                count += 1;
                c.push(ts.apply(p.0, p.1), on, last);
            })?;
            work.points += count;
            (work.points <= MAX_POINTS).then_some(())
        } else {
            if depth >= MAX_DEPTH {
                return None;
            }
            let mut so_far: Vec<(f32, f32)> = Vec::new();
            let matching = composite_matches(d);
            components(d, |g, mut t, matched| {
                if let Some((parent_pt, child_pt)) = matched {
                    // Point matching: move the component so its point lands on ours.
                    let mut child = Vec::new();
                    let tt = Ts {
                        e: 0.0,
                        f: 0.0,
                        ..t
                    };
                    self.points(g, &tt, depth + 1, &mut child, work)?;
                    let p = so_far.get(parent_pt as usize).copied();
                    let q = child.get(child_pt as usize).copied();
                    if let (Some(p), Some(q)) = (p, q) {
                        t.e = p.0 - q.0;
                        t.f = p.1 - q.1;
                    }
                }
                if matching {
                    self.points(g, &t, depth + 1, &mut so_far, work)?;
                }
                let t = ts.then(&t);
                self.emit(g, &t, depth + 1, c, work)
            })
        }
    }
}

/// Whether any component of composite glyph data positions itself by point matching.
fn composite_matches(d: &[u8]) -> bool {
    let mut any = false;
    let _ = components(d, |_, _, m| {
        any |= m.is_some();
        Some(())
    });
    any
}

/// Calls `f` with each point of a simple glyph: position, on curve, last of its contour.
fn simple(d: &[u8], contours: usize, mut f: impl FnMut((f32, f32), bool, bool)) -> Option<()> {
    let mut r = Reader::at(d, 10);
    let mut ends = Vec::with_capacity(contours);
    for _ in 0..contours {
        ends.push(r.u16()?);
    }
    let Some(&last) = ends.last() else {
        return Some(());
    };
    let n = last as usize + 1;
    let il = r.u16()? as usize;
    r.skip(il)?;
    let mut flags = Vec::with_capacity(n);
    while flags.len() < n {
        let fl = r.u8()?;
        flags.push(fl);
        if fl & 0x08 != 0 {
            let rep = r.u8()?;
            for _ in 0..rep {
                flags.push(fl);
            }
        }
    }
    flags.truncate(n);
    let mut xs = Vec::with_capacity(n);
    let mut x = 0i32;
    for &fl in &flags {
        if fl & 0x02 != 0 {
            let v = r.u8()? as i32;
            x += if fl & 0x10 != 0 { v } else { -v };
        } else if fl & 0x10 == 0 {
            x += r.i16()? as i32;
        }
        xs.push(x);
    }
    let mut y = 0i32;
    let mut end_i = 0usize;
    for (i, &fl) in flags.iter().enumerate() {
        if fl & 0x04 != 0 {
            let v = r.u8()? as i32;
            y += if fl & 0x20 != 0 { v } else { -v };
        } else if fl & 0x20 == 0 {
            y += r.i16()? as i32;
        }
        // Contour ends out of order: points past the last end go nowhere.
        while end_i < ends.len() && (ends[end_i] as usize) < i {
            end_i += 1;
        }
        let last = end_i < ends.len() && ends[end_i] as usize == i;
        f((xs[i] as f32, y as f32), fl & 0x01 != 0, last);
    }
    Some(())
}

/// Calls `f` with each component of composite glyph data: its glyph, its transform (offset
/// in `e`, `f` unless matched) and, when positioned by point matching, the two point numbers.
fn components(
    d: &[u8],
    mut f: impl FnMut(u16, Ts, Option<(u16, u16)>) -> Option<()>,
) -> Option<()> {
    let mut r = Reader::at(d, 10);
    loop {
        let flags = r.u16()?;
        let g = r.u16()?;
        let (a1, a2) = if flags & 0x01 != 0 {
            if flags & 0x02 != 0 {
                (r.i16()? as i32, r.i16()? as i32)
            } else {
                (r.u16()? as i32, r.u16()? as i32)
            }
        } else if flags & 0x02 != 0 {
            (r.u8()? as i8 as i32, r.u8()? as i8 as i32)
        } else {
            (r.u8()? as i32, r.u8()? as i32)
        };
        let mut t = Ts::ID;
        if flags & 0x08 != 0 {
            let s = f2dot14(&mut r)?;
            t.a = s;
            t.d = s;
        } else if flags & 0x40 != 0 {
            t.a = f2dot14(&mut r)?;
            t.d = f2dot14(&mut r)?;
        } else if flags & 0x80 != 0 {
            t.a = f2dot14(&mut r)?;
            t.b = f2dot14(&mut r)?;
            t.c = f2dot14(&mut r)?;
            t.d = f2dot14(&mut r)?;
        }
        let matched = if flags & 0x02 != 0 {
            let (mut dx, mut dy) = (a1 as f32, a2 as f32);
            // SCALED_COMPONENT_OFFSET: the offset is in the component's scaled space.
            if flags & 0x0800 != 0 && !t.is_default() {
                let (x, y) = (t.a * dx + t.c * dy, t.b * dx + t.d * dy);
                dx = x;
                dy = y;
            }
            t.e = dx;
            t.f = dy;
            None
        } else {
            Some((a1 as u16, a2 as u16))
        };
        f(g, t, matched)?;
        if flags & 0x20 == 0 {
            return Some(());
        }
    }
}

fn f2dot14(r: &mut Reader<'_>) -> Option<f32> {
    r.i16().map(|v| v as f32 / 16384.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyph i is 64 uses of glyph i + 1 (the last is empty): 64^16 visits before the work
    /// budget, now refused after a million.
    #[test]
    fn shared_components_are_bounded() {
        let n = 17u16;
        let mut glyf = Vec::new();
        let mut loca = Vec::new();
        for i in 0..n {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if i + 1 == n {
                continue;
            }
            glyf.extend_from_slice(&(-1i16).to_be_bytes());
            glyf.extend_from_slice(&[0; 8]);
            for k in 0..64 {
                let more = if k < 63 { 0x20u16 } else { 0 };
                glyf.extend_from_slice(&(more | 0x02).to_be_bytes());
                glyf.extend_from_slice(&(i + 1).to_be_bytes());
                glyf.extend_from_slice(&[0, 0]);
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let g = Glyf {
            glyf: &glyf,
            loca: &loca,
            long: true,
            num_glyphs: n,
        };
        let t = std::time::Instant::now();
        assert!(g.outline(0).is_none());
        assert!(t.elapsed().as_secs() < 10);
    }
}

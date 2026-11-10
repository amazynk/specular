//! Box-filter downscaling fused into decoding.
//!
//! Every output pixel is the average of an `f × f` block of input pixels (smaller at the right
//! and bottom edges), computed in linear light with premultiplied alpha so that edges against
//! transparency don't darken. Integer arithmetic: sRGB → 16-bit linear through a 256-entry
//! table, sums in `u32` (at most 64 pixels × 65535 × 255 < 2³²), back to sRGB through a
//! 65536-entry table.
//!
//! Two drivers share the arithmetic:
//! - [`render`] pulls rows from a random-access [`Rows`] source, only those the requested
//!   region needs, in parallel bands;
//! - [`RowScaler`] is pushed rows by a sequential decoder (top-down or bottom-up).
//!
//! [`render16`] is [`render`] for 16-bit samples (grey, grey + alpha or RGBA): the same
//! linear-light, alpha-weighted average, in floating point.

use crate::{Cancel, Error, GREY, Options, Result};
use rayon::prelude::*;
use std::sync::OnceLock;

fn lin_lut() -> &'static [u16; 256] {
    static T: OnceLock<[u16; 256]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u16; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f64 / 255.0;
            let l = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            *v = (l * 65535.0 + 0.5) as u16;
        }
        t
    })
}

fn srgb_lut() -> &'static [u8] {
    static T: OnceLock<Box<[u8]>> = OnceLock::new();
    T.get_or_init(|| {
        (0..65536u32)
            .map(|i| {
                let l = i as f64 / 65535.0;
                let c = if l <= 0.0031308 {
                    l * 12.92
                } else {
                    1.055 * l.powf(1.0 / 2.4) - 0.055
                };
                (c * 255.0 + 0.5).clamp(0.0, 255.0) as u8
            })
            .collect()
    })
}

/// Geometry of one decode: full size, factor, and the region in scaled pixels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Plan {
    pub w: u32,
    pub h: u32,
    pub f: u32,
    pub rx: u32,
    pub ry: u32,
    pub rw: u32,
    pub rh: u32,
}

impl Plan {
    pub fn new(w: u32, h: u32, opts: &Options) -> Result<Plan> {
        let f = opts.scale;
        if !(1..=8).contains(&f) {
            return Err(Error::BadRequest(format!("scale {f}")));
        }
        let (sw, sh) = (w.div_ceil(f), h.div_ceil(f));
        let (rx, ry, rw, rh) = match opts.region {
            None => (0, 0, sw, sh),
            Some(r) => {
                let x1 = r.x.saturating_add(r.w).min(sw);
                let y1 = r.y.saturating_add(r.h).min(sh);
                if r.x >= x1 || r.y >= y1 {
                    return Err(Error::BadRequest(format!(
                        "region {}×{}+{}+{} outside {sw}×{sh}",
                        r.w, r.h, r.x, r.y
                    )));
                }
                (r.x, r.y, x1 - r.x, y1 - r.y)
            }
        };
        Ok(Plan {
            w,
            h,
            f,
            rx,
            ry,
            rw,
            rh,
        })
    }
    /// Full-resolution columns the region reads.
    pub fn in_x(&self) -> (u32, u32) {
        (self.rx * self.f, ((self.rx + self.rw) * self.f).min(self.w))
    }
    /// Full-resolution rows the region reads.
    pub fn in_y(&self) -> (u32, u32) {
        (self.ry * self.f, ((self.ry + self.rh) * self.f).min(self.h))
    }
    pub fn out_len(&self, ch: usize) -> usize {
        self.rw as usize * self.rh as usize * ch
    }
}

/// A random-access row source.
pub(crate) trait Rows: Sync {
    /// Fill `out` with columns `x0..x1` of full-resolution row `y` (`ch` bytes per pixel).
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]);
}

/// Accumulates full-resolution rows into one output row.
struct Acc {
    ch: usize,
    opaque: bool,
    f: usize,
    sums: Vec<u32>,
    rows: u32,
}

impl Acc {
    fn new(rw: u32, ch: usize, opaque: bool, f: u32) -> Acc {
        Acc {
            ch,
            opaque,
            f: f as usize,
            sums: vec![0; rw as usize * ch],
            rows: 0,
        }
    }

    /// Add one input row (the region's columns only).
    fn add(&mut self, row: &[u8]) {
        let lin = lin_lut();
        let f = self.f;
        self.rows += 1;
        match (self.ch, self.opaque) {
            (1, _) => {
                for (s, blk) in self.sums.iter_mut().zip(row.chunks(f)) {
                    *s += blk.iter().map(|&v| lin[v as usize] as u32).sum::<u32>();
                }
            }
            (_, true) => {
                for (s, blk) in self.sums.chunks_exact_mut(4).zip(row.chunks(f * 4)) {
                    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                    for p in blk.chunks_exact(4) {
                        r += lin[p[0] as usize] as u32;
                        g += lin[p[1] as usize] as u32;
                        b += lin[p[2] as usize] as u32;
                    }
                    s[0] += r;
                    s[1] += g;
                    s[2] += b;
                }
            }
            _ => {
                for (s, blk) in self.sums.chunks_exact_mut(4).zip(row.chunks(f * 4)) {
                    let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
                    for p in blk.chunks_exact(4) {
                        let al = p[3] as u32;
                        r += lin[p[0] as usize] as u32 * al;
                        g += lin[p[1] as usize] as u32 * al;
                        b += lin[p[2] as usize] as u32 * al;
                        a += al;
                    }
                    s[0] += r;
                    s[1] += g;
                    s[2] += b;
                    s[3] += a;
                }
            }
        }
    }

    /// Write the averages to `out` and reset. `in_cols` = input columns of the region.
    fn finish(&mut self, in_cols: u32, out: &mut [u8]) {
        let srgb = srgb_lut();
        let f = self.f as u32;
        let rows = self.rows.max(1);
        let n_at = |i: usize| ((in_cols - i as u32 * f).min(f)) * rows;
        match (self.ch, self.opaque) {
            (1, _) => {
                for (i, (o, &s)) in out.iter_mut().zip(&self.sums).enumerate() {
                    *o = srgb[(s / n_at(i)) as usize];
                }
            }
            (_, true) => {
                for (i, (o, s)) in out
                    .chunks_exact_mut(4)
                    .zip(self.sums.chunks_exact(4))
                    .enumerate()
                {
                    let n = n_at(i);
                    o[0] = srgb[(s[0] / n) as usize];
                    o[1] = srgb[(s[1] / n) as usize];
                    o[2] = srgb[(s[2] / n) as usize];
                    o[3] = 255;
                }
            }
            _ => {
                for (i, (o, s)) in out
                    .chunks_exact_mut(4)
                    .zip(self.sums.chunks_exact(4))
                    .enumerate()
                {
                    let n = n_at(i);
                    let a = s[3];
                    o[3] = ((a + n / 2) / n) as u8;
                    match (
                        s[0].checked_div(a),
                        s[1].checked_div(a),
                        s[2].checked_div(a),
                    ) {
                        (Some(r), Some(g), Some(b)) => {
                            o[0] = srgb[r.min(65535) as usize];
                            o[1] = srgb[g.min(65535) as usize];
                            o[2] = srgb[b.min(65535) as usize];
                        }
                        _ => o[..3].fill(0),
                    }
                }
            }
        }
        self.sums.fill(0);
        self.rows = 0;
    }
}

/// Decode `plan` from a random-access source. `ch` is 1 (grey) or 4 (RGBA); `opaque` skips
/// alpha weighting.
pub(crate) fn render<R: Rows>(
    src: &R,
    ch: usize,
    opaque: bool,
    plan: &Plan,
    parallel: bool,
    cancel: Cancel,
) -> Result<Vec<u8>> {
    let mut out = vec![0u8; plan.out_len(ch)];
    let row_bytes = plan.rw as usize * ch;
    let (x0, x1) = plan.in_x();
    let in_cols = x1 - x0;
    // About 256 K input pixels per band.
    let band = ((256 * 1024) / (in_cols as usize * plan.f as usize * plan.f as usize).max(1))
        .clamp(1, plan.rh as usize);
    let work = |(bi, chunk): (usize, &mut [u8])| -> Result<()> {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let mut scratch = vec![0u8; in_cols as usize * ch];
        let mut acc = (plan.f > 1).then(|| Acc::new(plan.rw, ch, opaque, plan.f));
        for (i, orow) in chunk.chunks_exact_mut(row_bytes).enumerate() {
            let oy = plan.ry + (bi * band + i) as u32;
            match &mut acc {
                None => src.row(oy, x0, x1, orow),
                Some(acc) => {
                    let y0 = oy * plan.f;
                    let y1 = (y0 + plan.f).min(plan.h);
                    for y in y0..y1 {
                        src.row(y, x0, x1, &mut scratch);
                        acc.add(&scratch);
                    }
                    acc.finish(in_cols, orow);
                }
            }
        }
        Ok(())
    };
    let in_px = in_cols as u64 * (plan.in_y().1 - plan.in_y().0) as u64;
    if parallel && in_px > (1 << 20) && plan.rh as usize > band {
        out.par_chunks_mut(band * row_bytes)
            .enumerate()
            .try_for_each(work)?;
    } else {
        out.chunks_mut(band * row_bytes)
            .enumerate()
            .try_for_each(work)?;
    }
    Ok(out)
}

/// Receives full-width rows from a sequential decoder, in order (top-down or bottom-up).
pub(crate) struct RowScaler {
    plan: Plan,
    ch: usize,
    acc: Option<Acc>,
    out: Vec<u8>,
    top_down: bool,
    /// Next input row expected.
    next: i64,
    /// Output row (absolute, scaled) being accumulated.
    cur: Option<u32>,
}

impl RowScaler {
    pub fn new(plan: Plan, ch: usize, opaque: bool, top_down: bool) -> RowScaler {
        RowScaler {
            plan,
            ch,
            acc: (plan.f > 1).then(|| Acc::new(plan.rw, ch, opaque, plan.f)),
            out: vec![0u8; plan.out_len(ch)],
            top_down,
            next: if top_down { 0 } else { plan.h as i64 - 1 },
            cur: None,
        }
    }

    /// Whether row `y` contributes to the output (a decoder may skip converting others).
    pub fn wants(&self, y: u32) -> bool {
        let (y0, y1) = self.plan.in_y();
        (y0..y1).contains(&y)
    }

    /// Whether every needed row has arrived (a decoder can stop early).
    pub fn done(&self) -> bool {
        let (y0, y1) = self.plan.in_y();
        if self.top_down {
            self.next >= y1 as i64
        } else {
            self.next < y0 as i64
        }
    }

    /// Push full-resolution row `next` (`width × ch` bytes).
    pub fn push(&mut self, row: &[u8]) {
        let y = self.next;
        self.next += if self.top_down { 1 } else { -1 };
        if y < 0 || !self.wants(y as u32) {
            return;
        }
        let y = y as u32;
        let (x0, x1) = self.plan.in_x();
        let cols = &row[x0 as usize * self.ch..x1 as usize * self.ch];
        let oy = y / self.plan.f;
        let rb = self.plan.rw as usize * self.ch;
        let off = (oy - self.plan.ry) as usize * rb;
        match &mut self.acc {
            None => self.out[off..off + rb].copy_from_slice(cols),
            Some(acc) => {
                if let Some(c) = self.cur
                    && c != oy
                {
                    let o = (c - self.plan.ry) as usize * rb;
                    acc.finish(x1 - x0, &mut self.out[o..o + rb]);
                }
                self.cur = Some(oy);
                acc.add(cols);
            }
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        if let (Some(acc), Some(c)) = (&mut self.acc, self.cur) {
            let rb = self.plan.rw as usize * self.ch;
            let (x0, x1) = self.plan.in_x();
            let o = (c - self.plan.ry) as usize * rb;
            acc.finish(x1 - x0, &mut self.out[o..o + rb]);
        }
        self.out
    }
}

/// A random-access source of 16-bit rows.
pub(crate) trait Rows16: Sync {
    /// Fill `out` with columns `x0..x1` of full-resolution row `y` (`ch` samples per pixel).
    fn row16(&self, y: u32, x0: u32, x1: u32, out: &mut [u16]);
}

/// sRGB 16-bit code → linear light.
fn lin16_lut() -> &'static [f32] {
    static T: OnceLock<Box<[f32]>> = OnceLock::new();
    T.get_or_init(|| {
        (0..65536u32)
            .map(|i| {
                let c = i as f64 / 65535.0;
                (if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }) as f32
            })
            .collect()
    })
}

/// Linear light → sRGB 16-bit code.
fn enc16(l: f64) -> u16 {
    let c = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16
}

/// Write native-endian samples.
fn put16(src: &[u16], out: &mut [u8]) {
    for (o, v) in out.chunks_exact_mut(2).zip(src) {
        o.copy_from_slice(&v.to_ne_bytes());
    }
}

/// [`render`] for 16-bit samples: `ch` is 1 (grey), 2 (grey + alpha) or 4 (RGBA); the last
/// sample of 2 and 4 is straight alpha. Returns native-endian samples as bytes.
pub(crate) fn render16<R: Rows16>(
    src: &R,
    ch: usize,
    plan: &Plan,
    parallel: bool,
    cancel: Cancel,
) -> Result<Vec<u8>> {
    let mut out = vec![0u8; plan.out_len(ch * 2)];
    let row_bytes = plan.rw as usize * ch * 2;
    let (x0, x1) = plan.in_x();
    let in_cols = x1 - x0;
    let band = ((256 * 1024) / (in_cols as usize * plan.f as usize * plan.f as usize).max(1))
        .clamp(1, plan.rh as usize);
    let alpha = ch != 1;
    let nc = if alpha { ch - 1 } else { 1 };
    let f = plan.f as usize;
    let work = |(bi, chunk): (usize, &mut [u8])| -> Result<()> {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let lin = lin16_lut();
        let mut scratch = vec![0u16; in_cols as usize * ch];
        // Per output pixel: alpha-weighted linear colour, alpha sum, pixel count.
        let mut acc = vec![0f64; if f > 1 { plan.rw as usize * 5 } else { 0 }];
        let mut px = vec![0u16; plan.rw as usize * ch];
        for (i, orow) in chunk.chunks_exact_mut(row_bytes).enumerate() {
            let oy = plan.ry + (bi * band + i) as u32;
            if f == 1 {
                src.row16(oy, x0, x1, &mut scratch);
                put16(&scratch, orow);
                continue;
            }
            acc.fill(0.0);
            let y0 = oy * plan.f;
            let y1 = (y0 + plan.f).min(plan.h);
            for y in y0..y1 {
                src.row16(y, x0, x1, &mut scratch);
                for (x, p) in scratch.chunks_exact(ch).enumerate() {
                    let a = &mut acc[(x / f) * 5..(x / f) * 5 + 5];
                    let w = if alpha { p[nc] as f64 } else { 65535.0 };
                    if w != 0.0 {
                        for c in 0..nc {
                            a[c] += lin[p[c] as usize] as f64 * w;
                        }
                    }
                    a[3] += w;
                    a[4] += 1.0;
                }
            }
            for (a, o) in acc.chunks_exact(5).zip(px.chunks_exact_mut(ch)) {
                o.fill(0);
                if a[3] > 0.0 {
                    for c in 0..nc {
                        o[c] = enc16(a[c] / a[3]);
                    }
                    if alpha {
                        o[nc] = (a[3] / a[4].max(1.0) + 0.5) as u16;
                    }
                }
            }
            put16(&px, orow);
        }
        Ok(())
    };
    let in_px = in_cols as u64 * (plan.in_y().1 - plan.in_y().0) as u64;
    if parallel && in_px > (1 << 20) && plan.rh as usize > band {
        out.par_chunks_mut(band * row_bytes)
            .enumerate()
            .try_for_each(work)?;
    } else {
        out.chunks_mut(band * row_bytes)
            .enumerate()
            .try_for_each(work)?;
    }
    Ok(out)
}

/// The "missing" colour of 16-bit pixels of `ch` samples: mid grey, opaque.
pub(crate) fn fill_grey16(out: &mut [u16], ch: usize) {
    for p in out.chunks_exact_mut(ch) {
        p.fill(32768);
        if ch != 1 {
            p[ch - 1] = 65535;
        }
    }
}

/// Fill an RGBA or grey row with the "missing" colour.
pub(crate) fn fill_grey(out: &mut [u8], ch: usize) {
    if ch == 4 {
        for p in out.chunks_exact_mut(4) {
            p.copy_from_slice(&[GREY, GREY, GREY, 255]);
        }
    } else {
        out.fill(GREY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Solid(u32, [u8; 4]);
    impl Rows for Solid {
        fn row(&self, _y: u32, x0: u32, x1: u32, out: &mut [u8]) {
            for (x, p) in (x0..x1).zip(out.chunks_exact_mut(4)) {
                p.copy_from_slice(&if x < self.0 { self.1 } else { [0, 0, 0, 0] });
            }
        }
    }

    #[test]
    fn box_filter_premultiplied() {
        // Half the block red and opaque, half transparent black: red at half alpha, not dark.
        let s = Solid(1, [255, 0, 0, 255]);
        let plan = Plan::new(2, 2, &Options::scaled(2)).unwrap();
        let out = render(&s, 4, false, &plan, false, &|| false).unwrap();
        assert_eq!(out, vec![255, 0, 0, 128]);
    }

    struct Solid16(u32, [u16; 2]);
    impl Rows16 for Solid16 {
        fn row16(&self, _y: u32, x0: u32, x1: u32, out: &mut [u16]) {
            for (x, p) in (x0..x1).zip(out.chunks_exact_mut(2)) {
                p.copy_from_slice(&if x < self.0 { self.1 } else { [0, 0] });
            }
        }
    }

    #[test]
    fn box_filter16() {
        let ne = |v: &[u16]| v.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>();
        // Transparent pixels do not darken the average; alpha is the plain mean.
        let s = Solid16(1, [40000, 65535]);
        let plan = Plan::new(2, 2, &Options::scaled(2)).unwrap();
        let out = render16(&s, 2, &plan, false, &|| false).unwrap();
        assert_eq!(out, ne(&[40000, 32768]));
        // Scale 1 passes samples through.
        let plan = Plan::new(3, 1, &Options::scaled(1)).unwrap();
        let out = render16(&s, 2, &plan, false, &|| false).unwrap();
        assert_eq!(out, ne(&[40000, 65535, 0, 0, 0, 0]));
        // Black and white average in linear light: 0.5 → sRGB 0.73536.
        let s = Solid16(1, [65535, 65535]);
        let plan = Plan::new(2, 1, &Options::scaled(2)).unwrap();
        let out = render16(&Solid16Opaque(s), 2, &plan, false, &|| false).unwrap();
        assert_eq!(out, ne(&[48192, 65535]));
    }

    /// Opaque black past the edge of a [`Solid16`].
    struct Solid16Opaque(Solid16);
    impl Rows16 for Solid16Opaque {
        fn row16(&self, y: u32, x0: u32, x1: u32, out: &mut [u16]) {
            self.0.row16(y, x0, x1, out);
            for p in out.chunks_exact_mut(2) {
                p[1] = 65535;
            }
        }
    }

    #[test]
    fn edges_and_regions() {
        let s = Solid(100, [10, 20, 30, 255]);
        let plan = Plan::new(5, 3, &Options::scaled(2)).unwrap();
        assert_eq!((plan.rw, plan.rh), (3, 2));
        let out = render(&s, 4, true, &plan, false, &|| false).unwrap();
        assert!(out.chunks(4).all(|p| p == [10, 20, 30, 255]));
        let mut o = Options::scaled(2);
        o.region = Some(crate::Region {
            x: 2,
            y: 1,
            w: 10,
            h: 10,
        });
        let plan = Plan::new(5, 3, &o).unwrap();
        assert_eq!(
            (plan.rw, plan.rh, plan.in_x(), plan.in_y()),
            (1, 1, (4, 5), (2, 3))
        );
        o.region = Some(crate::Region {
            x: 3,
            y: 0,
            w: 1,
            h: 1,
        });
        assert!(Plan::new(5, 3, &o).is_err());
    }
}

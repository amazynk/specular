//! Blu-ray Presentation Graphics (PGS): `.sup` files and Matroska `S_HDMV/PGS` blocks.
//!
//! A stream is display sets of segments: a presentation composition (PCS) placing objects
//! on the screen, windows (WDS), palettes (PDS: Y, Cr, Cb, alpha) and run-length coded
//! objects (ODS, possibly split over several segments), closed by END. A composition with
//! no objects clears the screen; each shown picture lasts until the next composition.
//! Objects and palettes live for an epoch (a composition in state "epoch start" begins one).

use std::collections::HashMap;
use std::sync::Arc;

use crate::{Bitmap, Body, Cue, Error, Result, Track};

const PDS: u8 = 0x14;
const ODS: u8 = 0x15;
const PCS: u8 = 0x16;
const WDS: u8 = 0x17;
const END: u8 = 0x80;

/// How long the last picture shows when nothing ends it (seconds).
const LAST_HOLD_US: i64 = 5_000_000;

#[derive(Clone, Debug, Default)]
struct Object {
    w: u16,
    h: u16,
    rle: Vec<u8>,
}

#[derive(Clone, Debug)]
struct Placed {
    object: u16,
    x: u16,
    y: u16,
    crop: Option<(u16, u16, u16, u16)>,
}

#[derive(Clone, Debug, Default)]
struct Composition {
    w: u16,
    h: u16,
    palette: u8,
    objects: Vec<Placed>,
}

/// Decodes a PGS stream segment by segment.
#[derive(Default)]
pub struct Decoder {
    objects: HashMap<u16, Object>,
    palettes: HashMap<u8, Box<[[u8; 4]; 256]>>,
    pending: Option<(i64, Composition)>,
    /// Cues shown and not yet ended (by the next composition).
    open: Vec<Cue>,
    done: Vec<Cue>,
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder::default()
    }

    /// One segment: its type, payload and presentation time.
    pub fn segment(&mut self, kind: u8, data: &[u8], pts_us: i64) {
        match kind {
            PCS => self.pcs(data, pts_us),
            PDS => self.pds(data),
            ODS => self.ods(data),
            WDS => {}
            END => self.end(),
            _ => {}
        }
    }

    /// A Matroska block: segments without the `PG` header, timed by the block.
    pub fn block(&mut self, data: &[u8], pts_us: i64) {
        let mut p = 0;
        while p + 3 <= data.len() {
            let kind = data[p];
            let len = usize::from(u16::from_be_bytes([data[p + 1], data[p + 2]]));
            let body = data
                .get(p + 3..p + 3 + len)
                .unwrap_or(&data[(p + 3).min(data.len())..]);
            self.segment(kind, body, pts_us);
            p += 3 + len;
        }
    }

    fn pcs(&mut self, d: &[u8], pts: i64) {
        if d.len() < 11 {
            return;
        }
        let be = |i: usize| u16::from_be_bytes([d[i], d[i + 1]]);
        let state = d[7];
        if state & 0x80 != 0 {
            // Epoch start: earlier objects and palettes are gone.
            self.objects.clear();
            self.palettes.clear();
        }
        let mut c = Composition {
            w: be(0),
            h: be(2),
            palette: d[9],
            objects: Vec::new(),
        };
        let n = usize::from(d[10]);
        let mut p = 11;
        for _ in 0..n {
            if p + 8 > d.len() {
                break;
            }
            let cropped = d[p + 3] & 0x40 != 0;
            let mut o = Placed {
                object: be(p),
                x: be(p + 4),
                y: be(p + 6),
                crop: None,
            };
            p += 8;
            if cropped && p + 8 <= d.len() {
                o.crop = Some((be(p), be(p + 2), be(p + 4), be(p + 6)));
                p += 8;
            }
            c.objects.push(o);
        }
        // A new composition ends what shows.
        for mut cue in self.open.drain(..) {
            cue.end_us = pts;
            self.done.push(cue);
        }
        self.pending = Some((pts, c));
    }

    fn pds(&mut self, d: &[u8]) {
        if d.len() < 2 {
            return;
        }
        let id = d[0];
        let pal = self
            .palettes
            .entry(id)
            .or_insert_with(|| Box::new([[0, 0, 0, 0]; 256]));
        for e in d[2..].chunks_exact(5) {
            pal[usize::from(e[0])] = ycbcr(e[1], e[3], e[2], e[4]);
        }
    }

    fn ods(&mut self, d: &[u8]) {
        if d.len() < 4 {
            return;
        }
        let id = u16::from_be_bytes([d[0], d[1]]);
        let seq = d[3];
        if seq & 0x80 != 0 {
            if d.len() < 11 {
                return;
            }
            let o = Object {
                w: u16::from_be_bytes([d[7], d[8]]),
                h: u16::from_be_bytes([d[9], d[10]]),
                rle: d[11..].to_vec(),
            };
            self.objects.insert(id, o);
        } else if let Some(o) = self.objects.get_mut(&id) {
            o.rle.extend_from_slice(&d[4..]);
        }
    }

    fn end(&mut self) {
        let Some((pts, c)) = self.pending.take() else {
            return;
        };
        let Some(pal) = self.palettes.get(&c.palette) else {
            return;
        };
        for p in &c.objects {
            let Some(o) = self.objects.get(&p.object) else {
                continue;
            };
            let Some(mut rgba) = decode_rle(&o.rle, o.w, o.h, pal) else {
                continue;
            };
            let (x, y) = (p.x, p.y);
            let (mut w, mut h) = (o.w, o.h);
            if let Some((cx, cy, cw, ch)) = p.crop
                && cx.saturating_add(cw) <= o.w
                && cy.saturating_add(ch) <= o.h
                && cw > 0
                && ch > 0
            {
                rgba = crop(&rgba, o.w, cx, cy, cw, ch);
                (w, h) = (cw, ch);
            }
            if rgba.chunks_exact(4).all(|px| px[3] == 0) {
                continue;
            }
            self.open.push(Cue {
                start_us: pts,
                end_us: i64::MAX,
                body: Body::Bitmap(Bitmap {
                    x: u32::from(x),
                    y: u32::from(y),
                    w: u32::from(w),
                    h: u32::from(h),
                    canvas: (u32::from(c.w), u32::from(c.h)),
                    rgba: Arc::from(rgba.into_boxed_slice()),
                }),
            });
        }
    }

    /// The cues so far; open ones end `LAST_HOLD_US` after they started, or at `end_us`.
    pub fn finish(mut self, end_us: Option<i64>) -> Track {
        for mut cue in self.open.drain(..) {
            cue.end_us = end_us
                .unwrap_or(cue.start_us + LAST_HOLD_US)
                .max(cue.start_us + 1);
            self.done.push(cue);
        }
        Track::new(self.done)
    }
}

fn crop(rgba: &[u8], w: u16, cx: u16, cy: u16, cw: u16, ch: u16) -> Vec<u8> {
    let (w, cx, cy, cw, ch) = (
        usize::from(w),
        usize::from(cx),
        usize::from(cy),
        usize::from(cw),
        usize::from(ch),
    );
    let mut out = Vec::with_capacity(cw * ch * 4);
    for row in cy..cy + ch {
        let s = (row * w + cx) * 4;
        out.extend_from_slice(&rgba[s..s + cw * 4]);
    }
    out
}

/// BT.709 limited-range YCbCr and alpha to straight RGBA.
pub(crate) fn ycbcr(y: u8, cb: u8, cr: u8, a: u8) -> [u8; 4] {
    let y = (f32::from(y) - 16.0) * 1.164;
    let cb = f32::from(cb) - 128.0;
    let cr = f32::from(cr) - 128.0;
    let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [
        c(y + 1.793 * cr),
        c(y - 0.213 * cb - 0.533 * cr),
        c(y + 2.112 * cb),
        a,
    ]
}

/// PGS run-length data to RGBA.
fn decode_rle(d: &[u8], w: u16, h: u16, pal: &[[u8; 4]; 256]) -> Option<Vec<u8>> {
    let (w, h) = (usize::from(w), usize::from(h));
    if w == 0 || h == 0 || w * h > 16_000_000 {
        return None;
    }
    let mut idx = vec![0u8; w * h];
    let (mut x, mut y, mut p) = (0usize, 0usize, 0usize);
    let mut put = |x: &mut usize, y: usize, n: usize, c: u8| {
        if y >= h {
            return;
        }
        let n = n.min(w.saturating_sub(*x));
        let s = y * w + *x;
        idx[s..s + n].fill(c);
        *x += n;
    };
    while p < d.len() && y < h {
        let b = d[p];
        p += 1;
        if b != 0 {
            put(&mut x, y, 1, b);
            continue;
        }
        let Some(&f) = d.get(p) else { break };
        p += 1;
        if f == 0 {
            x = 0;
            y += 1;
            continue;
        }
        let long = f & 0x40 != 0;
        let coloured = f & 0x80 != 0;
        let mut n = usize::from(f & 0x3F);
        if long {
            n = (n << 8) | usize::from(*d.get(p)?);
            p += 1;
        }
        let c = if coloured {
            let c = *d.get(p)?;
            p += 1;
            c
        } else {
            0
        };
        put(&mut x, y, n, c);
    }
    let mut out = Vec::with_capacity(w * h * 4);
    for i in idx {
        out.extend_from_slice(&pal[usize::from(i)]);
    }
    Some(out)
}

/// A `.sup` file: segments each behind `PG`, PTS and DTS (90 kHz).
pub fn from_sup(data: &[u8]) -> Result<Track> {
    let mut dec = Decoder::new();
    let mut p = 0;
    let mut seen = false;
    while p + 13 <= data.len() {
        if &data[p..p + 2] != b"PG" {
            if !seen {
                return Err(Error::Malformed("not a PGS .sup file".into()));
            }
            break;
        }
        seen = true;
        let pts = u32::from_be_bytes([data[p + 2], data[p + 3], data[p + 4], data[p + 5]]);
        let kind = data[p + 10];
        let len = usize::from(u16::from_be_bytes([data[p + 11], data[p + 12]]));
        let body = data.get(p + 13..p + 13 + len).unwrap_or(&data[p + 13..]);
        dec.segment(kind, body, i64::from(pts) * 100 / 9);
        p += 13 + len;
    }
    if !seen {
        return Err(Error::Malformed("not a PGS .sup file".into()));
    }
    Ok(dec.finish(None))
}

/// Test streams: one display set showing a `w`×`h` object filled with palette entry 1
/// (opaque), and an empty composition clearing it.
#[doc(hidden)]
pub mod fixture {
    /// Run-length codes for a `w`×`h` object of colour 1.
    fn rle(w: u16, h: u16) -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..h {
            // A coloured run (long form when over 63 pixels), then end of line.
            if w < 64 {
                v.extend_from_slice(&[0x00, 0x80 | w as u8, 1]);
            } else {
                v.extend_from_slice(&[0x00, 0xC0 | (w >> 8) as u8, w as u8, 1]);
            }
            v.extend_from_slice(&[0x00, 0x00]);
        }
        v
    }

    /// Segments (type, payload) for one display set at a picture `cw`×`ch`.
    pub fn show(cw: u16, ch: u16, x: u16, y: u16, w: u16, h: u16) -> Vec<(u8, Vec<u8>)> {
        let mut pcs = Vec::new();
        pcs.extend_from_slice(&cw.to_be_bytes());
        pcs.extend_from_slice(&ch.to_be_bytes());
        pcs.extend_from_slice(&[0x10, 0, 1, 0x80, 0, 0, 1]);
        pcs.extend_from_slice(&[0, 0, 0, 0]);
        pcs.extend_from_slice(&x.to_be_bytes());
        pcs.extend_from_slice(&y.to_be_bytes());
        // Palette 0: entry 1 white (Y 235, Cb/Cr 128), opaque.
        let pds = vec![0, 0, 1, 235, 128, 128, 255];
        let data = rle(w, h);
        let len = data.len() + 4;
        let mut ods = vec![
            0,
            0,
            0,
            0xC0,
            (len >> 16) as u8,
            (len >> 8) as u8,
            len as u8,
        ];
        ods.extend_from_slice(&w.to_be_bytes());
        ods.extend_from_slice(&h.to_be_bytes());
        ods.extend_from_slice(&data);
        vec![
            (super::PCS, pcs),
            (super::PDS, pds),
            (super::ODS, ods),
            (super::END, vec![]),
        ]
    }

    /// An empty composition (clears the screen).
    pub fn clear(cw: u16, ch: u16) -> Vec<(u8, Vec<u8>)> {
        let mut pcs = Vec::new();
        pcs.extend_from_slice(&cw.to_be_bytes());
        pcs.extend_from_slice(&ch.to_be_bytes());
        pcs.extend_from_slice(&[0x10, 0, 2, 0x00, 0, 0, 0]);
        vec![(super::PCS, pcs), (super::END, vec![])]
    }

    /// The segments as a `.sup` file, at `pts` (90 kHz ticks).
    /// Segments: (type, payload).
    pub type Segments = Vec<(u8, Vec<u8>)>;

    pub fn sup(sets: &[(u32, Segments)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (pts, segs) in sets {
            for (k, d) in segs {
                out.extend_from_slice(b"PG");
                out.extend_from_slice(&pts.to_be_bytes());
                out.extend_from_slice(&0u32.to_be_bytes());
                out.push(*k);
                out.extend_from_slice(&(d.len() as u16).to_be_bytes());
                out.extend_from_slice(d);
            }
        }
        out
    }

    /// The segments as one Matroska block.
    pub fn block(segs: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (k, d) in segs {
            out.push(*k);
            out.extend_from_slice(&(d.len() as u16).to_be_bytes());
            out.extend_from_slice(d);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sup_shows_and_clears() {
        let data = fixture::sup(&[
            (90_000, fixture::show(1920, 1080, 100, 900, 200, 50)),
            (270_000, fixture::clear(1920, 1080)),
        ]);
        let t = from_sup(&data).unwrap();
        assert_eq!(t.cues.len(), 1);
        let c = &t.cues[0];
        assert_eq!((c.start_us, c.end_us), (1_000_000, 3_000_000));
        let Body::Bitmap(b) = &c.body else {
            panic!("bitmap")
        };
        assert_eq!(
            (b.x, b.y, b.w, b.h, b.canvas),
            (100, 900, 200, 50, (1920, 1080))
        );
        assert_eq!(&b.rgba[..4], &[255, 255, 255, 255]);
        assert_eq!(b.rgba.len(), 200 * 50 * 4);
    }

    #[test]
    fn matroska_blocks_and_open_end() {
        let mut d = Decoder::new();
        d.block(
            &fixture::block(&fixture::show(720, 480, 10, 20, 30, 4)),
            2_000_000,
        );
        let t = d.finish(Some(2_500_000));
        assert_eq!(t.cues.len(), 1);
        assert_eq!(
            (t.cues[0].start_us, t.cues[0].end_us),
            (2_000_000, 2_500_000)
        );
    }

    #[test]
    fn not_pgs() {
        assert!(from_sup(b"hello world, not pgs").is_err());
    }
}

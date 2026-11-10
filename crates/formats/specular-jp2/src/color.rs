//! Channel mapping (JP2 `pclr` / `cmap` / `cdef`), colour conversion and scaling to the output
//! depth.

use crate::boxes::{Boxes, ColorSpace};
use crate::geom::Rect;
use crate::markers::Siz;
use crate::tile::CompOut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Gray,
    Rgb,
    Sycc,
    Cmyk,
}

/// A channel: a component used directly (through a scaling LUT) or through a palette column
/// (already scaled to the output range).
pub(crate) struct Chan {
    pub comp: usize,
    /// Native value -> output scale (direct), or palette entry -> output scale.
    lut: Vec<u16>,
    palette: bool,
    /// The LUT maps every value to itself (8-bit samples to 8-bit output, 16 to 16).
    identity: bool,
}

pub(crate) struct Mapping {
    pub kind: Kind,
    pub chans: Vec<Chan>,
    /// Channel index per colour (1, 3 or 4 entries).
    pub colors: Vec<usize>,
    /// Alpha channel and whether it is premultiplied.
    pub alpha: Option<(usize, bool)>,
    /// 255 or 65535.
    pub max: u32,
}

/// Output sampling grid: the common component subsampling when every component shares it
/// (the image is then output at component resolution), else `(1, 1)`.
pub(crate) fn output_grid(siz: &Siz) -> (u32, u32) {
    let c0 = siz.comps[0];
    if siz.comps.iter().all(|c| c.dx == c0.dx && c.dy == c0.dy) {
        (c0.dx.max(1) as u32, c0.dy.max(1) as u32)
    } else {
        (1, 1)
    }
}

fn scale_lut(prec: u8, max: u32) -> Vec<u16> {
    let p = prec.clamp(1, 16) as u32;
    let vmax = (1u64 << p) - 1;
    (0..=vmax)
        .map(|v| ((v * max as u64 + vmax / 2) / vmax) as u16)
        .collect()
}

impl Mapping {
    pub fn new(boxes: Option<&Boxes>, siz: &Siz, sixteen: bool) -> Mapping {
        let max = if sixteen { 65535 } else { 255 };
        let nc = siz.comps.len();
        let comp_prec = |c: usize| siz.comps[c].prec.min(16);
        let mut chans = Vec::new();
        let pal = boxes.and_then(|b| b.palette.as_ref());
        let cmap = boxes.and_then(|b| b.cmap.as_ref());
        match (pal, cmap) {
            (Some(pal), Some(cmap)) => {
                for e in cmap {
                    let comp = (e.component as usize).min(nc - 1);
                    match e.palette_col {
                        Some(col) if (col as usize) < pal.columns.len() => {
                            let ncol = pal.columns.len();
                            let (pp, _) = pal.columns[col as usize];
                            let shift = pp.saturating_sub(16) as u32;
                            let pe = pp.clamp(1, 16);
                            let lut = scale_lut(pe, max);
                            let vals = (0..pal.entries)
                                .map(|i| {
                                    let v = pal.values[i * ncol + col as usize] >> shift;
                                    lut[(v as usize).min(lut.len() - 1)]
                                })
                                .collect();
                            chans.push(Chan {
                                comp,
                                lut: vals,
                                palette: true,
                                identity: false,
                            });
                        }
                        _ => chans.push(Chan {
                            comp,
                            lut: scale_lut(comp_prec(comp), max),
                            palette: false,
                            identity: false,
                        }),
                    }
                }
            }
            _ => {
                for c in 0..nc {
                    chans.push(Chan {
                        comp: c,
                        lut: scale_lut(comp_prec(c), max),
                        palette: false,
                        identity: false,
                    });
                }
            }
        }
        for ch in &mut chans {
            ch.identity = !ch.palette && ch.lut.iter().enumerate().all(|(i, &v)| v as usize == i);
        }
        let n = chans.len();
        let color = boxes.and_then(|b| b.color);
        let mut kind = match color {
            Some(ColorSpace::Gray) => Kind::Gray,
            Some(ColorSpace::Sycc) | Some(ColorSpace::Esycc) => Kind::Sycc,
            Some(ColorSpace::Cmyk) => Kind::Cmyk,
            Some(ColorSpace::Srgb) => Kind::Rgb,
            _ => {
                if n >= 3 {
                    Kind::Rgb
                } else {
                    Kind::Gray
                }
            }
        };
        let need = |k: Kind| match k {
            Kind::Gray => 1,
            Kind::Cmyk => 4,
            _ => 3,
        };
        if n < need(kind) {
            kind = if n >= 3 { Kind::Rgb } else { Kind::Gray };
        }
        let nk = need(kind);
        let mut colors: Vec<Option<usize>> = vec![None; nk];
        let mut alpha = None;
        if let Some(cdef) = boxes.and_then(|b| b.cdef.as_ref()) {
            for e in cdef {
                let ch = e.channel as usize;
                if ch >= n {
                    continue;
                }
                match e.typ {
                    0 if (1..=nk).contains(&(e.assoc as usize)) => {
                        colors[e.assoc as usize - 1].get_or_insert(ch);
                    }
                    1 | 2 if alpha.is_none() => alpha = Some((ch, e.typ == 2)),
                    _ => {}
                }
            }
        }
        // Fill unassigned colours with the remaining channels in order.
        let used: Vec<usize> = colors.iter().flatten().copied().collect();
        let alpha_ch = alpha.map(|a| a.0);
        let mut free = (0..n).filter(|&ch| !used.contains(&ch) && alpha_ch != Some(ch));
        for col in &mut colors {
            if col.is_none() {
                *col = free.next();
            }
        }
        let has_cdef = boxes.is_some_and(|b| b.cdef.is_some());
        if alpha.is_none() && !has_cdef {
            alpha = free.next().map(|ch| (ch, false));
        }
        let colors: Vec<usize> = colors.into_iter().map(|c| c.unwrap_or(0)).collect();
        Mapping {
            kind,
            chans,
            colors,
            alpha,
            max,
        }
    }

    /// Output is a single grey channel (no alpha).
    pub fn is_gray(&self) -> bool {
        self.kind == Kind::Gray && self.alpha.is_none()
    }

    /// Fill `out[..len]` with channel `ch` for output pixels `x0..x0 + len` of row `y`
    /// (reduced output-grid coordinates, see [`output_grid`]).
    #[allow(clippy::too_many_arguments)]
    pub fn fill(&self, ch: usize, comps: &[CompOut], siz: &Siz, x0: u32, y: u32, out: &mut [u16]) {
        let chan = &self.chans[ch];
        let co = &comps[chan.comp];
        if co.data.is_empty() {
            out.fill(0);
            return;
        }
        let cs = siz.comps[chan.comp];
        let (gx, gy) = output_grid(siz);
        let (dx, dy) = ((cs.dx as u32 / gx).max(1), (cs.dy as u32 / gy).max(1));
        let r: Rect = co.rect;
        let cy = (y / dy).clamp(r.y0, r.y1 - 1) - r.y0;
        let w = r.w() as usize;
        let row0 = cy as usize * w;
        let lut = &chan.lut[..];
        let lmax = lut.len() - 1;
        if dx == 1 && x0 >= r.x0 && x0 as u64 + out.len() as u64 <= r.x1 as u64 {
            co.data.native(row0 + (x0 - r.x0) as usize, out);
            if chan.palette {
                for o in out.iter_mut() {
                    *o = lut[(*o as usize).min(lmax)];
                }
            } else if !chan.identity {
                for o in out.iter_mut() {
                    *o = lut[(*o as usize) & lmax];
                }
            }
        } else {
            for (i, o) in out.iter_mut().enumerate() {
                let cx = ((x0 + i as u32) / dx).clamp(r.x0, r.x1 - 1) - r.x0;
                let v = co.data.get(row0 + cx as usize) as usize;
                *o = lut[v.min(lmax)];
            }
        }
    }

    /// Convert channel rows into output pixels: grey (1 sample) or RGBA (4 samples).
    pub fn pixels(
        &self,
        rows: &[Vec<u16>],
        len: usize,
        out8: Option<&mut [u8]>,
        out16: Option<&mut [u16]>,
    ) {
        let max = self.max;
        // Fast paths: plain grey or RGB with straight (or no) alpha.
        let straight = !matches!(self.alpha, Some((_, true)));
        if straight && matches!(self.kind, Kind::Gray | Kind::Rgb) {
            let g = |ch: usize| &rows[ch][..len];
            if self.is_gray() {
                let src = g(self.colors[0]);
                if let Some(o) = out8 {
                    for (o, &v) in o[..len].iter_mut().zip(src) {
                        *o = v as u8;
                    }
                }
                if let Some(o) = out16 {
                    o[..len].copy_from_slice(src);
                }
                return;
            }
            let (r, gg, b) = if self.kind == Kind::Gray {
                let s = g(self.colors[0]);
                (s, s, s)
            } else {
                (g(self.colors[0]), g(self.colors[1]), g(self.colors[2]))
            };
            let a = self.alpha.map(|(ch, _)| g(ch));
            let m = max as u16;
            if let Some(o) = out8 {
                let o = &mut o[..len * 4];
                match a {
                    None => {
                        for (i, px) in o.chunks_exact_mut(4).enumerate() {
                            px.copy_from_slice(&[r[i] as u8, gg[i] as u8, b[i] as u8, m as u8]);
                        }
                    }
                    Some(a) => {
                        for (i, px) in o.chunks_exact_mut(4).enumerate() {
                            px.copy_from_slice(&[r[i] as u8, gg[i] as u8, b[i] as u8, a[i] as u8]);
                        }
                    }
                }
            }
            if let Some(o) = out16 {
                for (i, px) in o[..len * 4].chunks_exact_mut(4).enumerate() {
                    px.copy_from_slice(&[r[i], gg[i], b[i], a.map_or(m, |a| a[i])]);
                }
            }
            return;
        }
        let get = |ch: usize, i: usize| rows[ch][i] as u32;
        let gray_out = self.is_gray();
        let mut o8 = out8;
        let mut o16 = out16;
        for i in 0..len {
            let a = self.alpha.map(|(ch, _)| get(ch, i)).unwrap_or(max);
            let mut c = match self.kind {
                Kind::Gray => {
                    let g = get(self.colors[0], i);
                    [g, g, g]
                }
                Kind::Rgb => [
                    get(self.colors[0], i),
                    get(self.colors[1], i),
                    get(self.colors[2], i),
                ],
                Kind::Sycc => {
                    let m = max as f32;
                    let half = (max / 2 + 1) as f32;
                    let y = get(self.colors[0], i) as f32;
                    let cb = get(self.colors[1], i) as f32 - half;
                    let cr = get(self.colors[2], i) as f32 - half;
                    let f = |v: f32| v.round().clamp(0.0, m) as u32;
                    [
                        f(y + 1.402 * cr),
                        f(y - 0.344_136 * cb - 0.714_136 * cr),
                        f(y + 1.772 * cb),
                    ]
                }
                Kind::Cmyk => {
                    let k = max - get(self.colors[3], i).min(max);
                    let f = |v: u32| ((max - v.min(max)) * k + max / 2) / max;
                    [
                        f(get(self.colors[0], i)),
                        f(get(self.colors[1], i)),
                        f(get(self.colors[2], i)),
                    ]
                }
            };
            if let Some((_, true)) = self.alpha {
                for v in &mut c {
                    // Fully transparent (a == 0) un-premultiplies to 0.
                    *v = (*v * max + a / 2).checked_div(a).map_or(0, |q| q.min(max));
                }
            }
            if gray_out {
                if let Some(o) = o8.as_deref_mut() {
                    o[i] = c[0] as u8;
                }
                if let Some(o) = o16.as_deref_mut() {
                    o[i] = c[0] as u16;
                }
            } else {
                if let Some(o) = o8.as_deref_mut() {
                    o[i * 4] = c[0] as u8;
                    o[i * 4 + 1] = c[1] as u8;
                    o[i * 4 + 2] = c[2] as u8;
                    o[i * 4 + 3] = a as u8;
                }
                if let Some(o) = o16.as_deref_mut() {
                    o[i * 4] = c[0] as u16;
                    o[i * 4 + 1] = c[1] as u16;
                    o[i * 4 + 2] = c[2] as u16;
                    o[i * 4 + 3] = a as u16;
                }
            }
        }
    }
}

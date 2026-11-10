//! One slice (RFC 9043 §4.5–4.8): its header, the contexts it keeps from frame to frame,
//! the median-predicted samples of every plane, and the reversible colour transform.

use crate::coder::{BitReader, RangeDecoder, States, Transitions, Vlc};
use crate::{ColourSpace, Error, Layout, Params, Picture, QuantSet, Result};

/// RFC 9043 §3.8.2.2.1: bits of the run length below each run index.
const LOG2_RUN: [u32; 41] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13,
    14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
];

/// The adaptive state of one context set (luma, chroma or transparency) of a slice.
#[derive(Default)]
struct PlaneCtx {
    states: Vec<States>,
    vlc: Vec<Vlc>,
}

/// What a slice keeps between frames: its contexts and working buffers.
#[derive(Default)]
pub struct SliceCtx {
    sets: Vec<usize>,
    planes: Vec<PlaneCtx>,
    bufs: Vec<Vec<i32>>,
}

/// A decoded slice: where it sits and its planes (final samples, in the output order).
pub struct SliceOut {
    pub ok: bool,
    /// Luma position and size.
    pub rect: (usize, usize, usize, usize),
    pub planes: Vec<Vec<u16>>,
    pub picture_structure: u32,
    pub sar: (u32, u32),
}

impl SliceOut {
    pub fn damaged() -> SliceOut {
        SliceOut {
            ok: false,
            rect: (0, 0, 0, 0),
            planes: Vec::new(),
            picture_structure: 0,
            sar: (0, 0),
        }
    }
}

#[inline(always)]
fn median(a: i32, b: i32, c: i32) -> i32 {
    a.min(b).max(a.max(b).min(c))
}

/// Where the planes' samples sit in a slice's working buffer: two zero rows above, two
/// columns to the left (zero, then the first sample of the row above) and one to the right.
struct Plane {
    w: usize,
    h: usize,
    stride: usize,
}

impl Plane {
    fn new(w: usize, h: usize) -> Plane {
        Plane {
            w,
            h,
            stride: w + 3,
        }
    }
    fn len(&self) -> usize {
        (self.h + 2) * self.stride
    }
}

/// The rows a line reads: `(row y-2, row y-1, row y)`.
#[inline(always)]
fn rows(buf: &mut [i32], stride: usize, y: usize) -> (&[i32], &[i32], &mut [i32]) {
    let (top, cur) = buf.split_at_mut((y + 2) * stride);
    let (above2, above) = top[y * stride..].split_at(stride);
    (above2, above, &mut cur[..stride])
}

/// The context of the sample at `xi` and its prediction.
#[inline(always)]
fn context(
    q: &QuantSet,
    above2: &[i32],
    above: &[i32],
    cur: &[i32],
    xi: usize,
) -> (i32, i32, i32, i32) {
    let l = cur[xi - 1];
    let t = above[xi];
    let tl = above[xi - 1];
    let tr = above[xi + 1];
    let tb = &q.tables;
    let mut ctx = tb[0][((l - tl) & 255) as usize]
        + tb[1][((tl - t) & 255) as usize]
        + tb[2][((t - tr) & 255) as usize];
    if q.far {
        let ll = cur[xi - 2];
        let tt = above2[xi];
        ctx += tb[3][((ll - l) & 255) as usize] + tb[4][((tt - t) & 255) as usize];
    }
    (ctx, l, t, tl)
}

#[inline(always)]
fn signed16(v: i32) -> i32 {
    if v >= 32768 { v - 65536 } else { v }
}

/// One line, range coded.
#[allow(clippy::too_many_arguments)]
fn line_range(
    c: &mut RangeDecoder<'_>,
    t: &Transitions,
    q: &QuantSet,
    states: &mut [States],
    buf: &mut [i32],
    pl: &Plane,
    y: usize,
    bits: u32,
    exception16: bool,
) {
    let mask = ((1u32 << bits) - 1) as i32;
    let (above2, above, cur) = rows(buf, pl.stride, y);
    cur[1] = above[2];
    for xi in 2..pl.w + 2 {
        let (ctx, l, tp, tl) = context(q, above2, above, cur, xi);
        let pred = if exception16 {
            let (l, tp, tl) = (signed16(l), signed16(tp), signed16(tl));
            median(l, tp, l + tp - tl)
        } else {
            median(l, tp, l + tp - tl)
        };
        let diff = if ctx < 0 {
            c.symbol(&mut states[(-ctx) as usize], t, true)
                .wrapping_neg()
        } else {
            c.symbol(&mut states[ctx as usize], t, true)
        };
        cur[xi] = pred.wrapping_add(diff) & mask;
    }
    cur[pl.w + 2] = cur[pl.w + 1];
}

/// One line, Golomb-Rice coded with run mode.
#[allow(clippy::too_many_arguments)]
fn line_golomb(
    r: &mut BitReader<'_>,
    q: &QuantSet,
    vlc: &mut [Vlc],
    buf: &mut [i32],
    pl: &Plane,
    y: usize,
    bits: u32,
    run_index: &mut usize,
) {
    let mask = ((1u32 << bits) - 1) as i32;
    let (above2, above, cur) = rows(buf, pl.stride, y);
    cur[1] = above[2];
    let w = pl.w;
    let mut run_mode = 0u8;
    let mut run_count: i64 = 0;
    for x in 0..w {
        let xi = x + 2;
        let (ctx, l, tp, tl) = context(q, above2, above, cur, xi);
        let (ctx, neg) = if ctx < 0 { (-ctx, true) } else { (ctx, false) };
        if ctx == 0 && run_mode == 0 {
            run_mode = 1;
        }
        let mut diff;
        if run_mode != 0 {
            if run_count == 0 && run_mode == 1 {
                let lg = LOG2_RUN[*run_index];
                if r.bit() {
                    run_count = 1i64 << lg;
                    if x as i64 + run_count <= w as i64 {
                        *run_index = (*run_index + 1).min(40);
                    }
                } else {
                    run_count = i64::from(r.bits(lg));
                    *run_index = run_index.saturating_sub(1);
                    run_mode = 2;
                }
            }
            run_count -= 1;
            if run_count < 0 {
                run_mode = 0;
                run_count = 0;
                diff = r.vlc(&mut vlc[ctx as usize], bits);
                if diff >= 0 {
                    diff += 1;
                }
            } else {
                diff = 0;
            }
        } else {
            diff = r.vlc(&mut vlc[ctx as usize], bits);
        }
        if neg {
            diff = diff.wrapping_neg();
        }
        cur[xi] = median(l, tp, l + tp - tl).wrapping_add(diff) & mask;
    }
    cur[w + 2] = cur[w + 1];
}

/// The context set of plane `p` (luma 0, chroma 1, transparency 2).
fn set_of(p: usize, params: &Params) -> usize {
    match (params.colour_space, p) {
        (_, 0) => 0,
        (ColourSpace::Rgb, 1 | 2) => 1,
        (ColourSpace::Rgb, _) => 2,
        (ColourSpace::Ycbcr, _) if !params.chroma_planes => 2,
        (ColourSpace::Ycbcr, 1 | 2) => 1,
        _ => 2,
    }
}

/// Decodes one slice of a frame. `c` is the slice's range decoder (for the first slice, past
/// the keyframe flag and any parameters); `bytes` its coded bytes.
pub fn decode(
    p: &Params,
    (fw, fh): (u32, u32),
    keyframe: bool,
    mut c: RangeDecoder<'_>,
    bytes: &[u8],
    ctx: &mut SliceCtx,
) -> SliceOut {
    match decode_inner(p, (fw as usize, fh as usize), keyframe, &mut c, bytes, ctx) {
        Ok(o) => o,
        Err(_) => {
            // The contexts are no longer in step with the encoder's.
            ctx.planes.clear();
            SliceOut::damaged()
        }
    }
}

fn decode_inner(
    p: &Params,
    (fw, fh): (usize, usize),
    keyframe: bool,
    c: &mut RangeDecoder<'_>,
    bytes: &[u8],
    ctx: &mut SliceCtx,
) -> Result<SliceOut> {
    let bad = |s: &str| Error::Malformed(s.into());
    let dt = Transitions::default();
    let nsets = p.context_sets();
    let (mut rect, mut sets) = ((0, 0, fw, fh), vec![0usize; nsets]);
    let (mut picture_structure, mut sar) = (0, (0, 0));
    if p.version >= 3 {
        let mut st: States = [128; 32];
        let t = &p.transitions;
        let sx = c.unsigned(&mut st, t);
        let sy = c.unsigned(&mut st, t);
        let sw = c.unsigned(&mut st, t).saturating_add(1);
        let sh = c.unsigned(&mut st, t).saturating_add(1);
        if sx.saturating_add(sw) > p.num_h_slices || sy.saturating_add(sh) > p.num_v_slices {
            return Err(bad("slice position"));
        }
        for s in sets.iter_mut() {
            *s = c.unsigned(&mut st, t) as usize;
            if *s >= p.quant.len() {
                return Err(bad("quantisation table set index"));
            }
        }
        picture_structure = c.unsigned(&mut st, t);
        sar = (c.unsigned(&mut st, t), c.unsigned(&mut st, t));
        if c.overflow {
            return Err(bad("slice header"));
        }
        let (nh, nv) = (p.num_h_slices as usize, p.num_v_slices as usize);
        let x0 = sx as usize * fw / nh;
        let x1 = (sx + sw) as usize * fw / nh;
        let y0 = sy as usize * fh / nv;
        let y1 = (sy + sh) as usize * fh / nv;
        rect = (x0, y0, x1 - x0, y1 - y0);
    }
    if keyframe || ctx.sets != sets || ctx.planes.len() != nsets {
        ctx.planes.clear();
        for &s in &sets {
            let q = &p.quant[s];
            let states = match &p.initial[s] {
                Some(init) => init.clone(),
                None => vec![[128u8; 32]; q.context_count],
            };
            ctx.planes.push(PlaneCtx {
                states,
                vlc: vec![Vlc::default(); q.context_count],
            });
        }
        ctx.sets = sets.clone();
    }
    let (_, _, w, h) = rect;
    let count = p.plane_count();
    let sizes: Vec<Plane> = (0..count)
        .map(|i| {
            if p.chroma_planes && (i == 1 || i == 2) {
                Plane::new(w.div_ceil(1 << p.log2_h), h.div_ceil(1 << p.log2_v))
            } else {
                Plane::new(w, h)
            }
        })
        .collect();
    ctx.bufs.resize_with(count, Vec::new);
    for (b, pl) in ctx.bufs.iter_mut().zip(&sizes) {
        b.clear();
        b.resize(pl.len(), 0);
    }
    let rgb = p.colour_space == ColourSpace::Rgb;
    let bits = if rgb { p.bits + 1 } else { p.bits };
    let exception16 = !rgb && p.bits == 16 && p.coder_type != 0;
    let golomb = p.coder_type == 0;
    let mut reader = if golomb {
        let at = c.sentinel(&dt);
        Some(BitReader::new(bytes.get(at..).unwrap_or_default()))
    } else {
        None
    };
    let t = &p.transitions;
    let mut run_index = 0usize;
    let mut line = |pi: usize, y: usize, ctx: &mut SliceCtx, run_index: &mut usize| {
        let set = set_of(pi, p);
        let q = &p.quant[ctx.sets[set]];
        let pc = &mut ctx.planes[set];
        let buf = &mut ctx.bufs[pi];
        match reader.as_mut() {
            Some(r) => line_golomb(r, q, &mut pc.vlc, buf, &sizes[pi], y, bits, run_index),
            None => line_range(
                c,
                t,
                q,
                &mut pc.states,
                buf,
                &sizes[pi],
                y,
                bits,
                exception16,
            ),
        }
    };
    if rgb {
        for y in 0..h {
            for pi in 0..count {
                line(pi, y, ctx, &mut run_index);
            }
        }
    } else {
        for (pi, pl) in sizes.iter().enumerate() {
            run_index = 0;
            for y in 0..pl.h {
                line(pi, y, ctx, &mut run_index);
            }
        }
    }
    if c.overflow {
        return Err(bad("sample"));
    }
    // Out of the working buffers, the colour transform undone.
    let crop = |b: &[i32], pl: &Plane| -> Vec<u16> {
        let mut o = Vec::with_capacity(pl.w * pl.h);
        for y in 0..pl.h {
            let r = (y + 2) * pl.stride + 2;
            o.extend(b[r..r + pl.w].iter().map(|&v| v as u16));
        }
        o
    };
    let planes = if rgb {
        let off = 1i32 << p.bits;
        let max = (1i32 << p.bits) - 1;
        let swapped = (9..=15).contains(&p.bits) && !p.extra_plane;
        let n = w * h;
        let (mut g, mut b, mut r) = (vec![0u16; n], vec![0u16; n], vec![0u16; n]);
        let pl = &sizes[0];
        for y in 0..h {
            let row = (y + 2) * pl.stride + 2;
            let (ys, cbs, crs) = (
                &ctx.bufs[0][row..row + w],
                &ctx.bufs[1][row..row + w],
                &ctx.bufs[2][row..row + w],
            );
            for x in 0..w {
                let (cb, cr) = (cbs[x] - off, crs[x] - off);
                let base = ys[x] - ((cb + cr) >> 2);
                let (gv, bv, rv) = if swapped {
                    (cb + base, base, cr + base)
                } else {
                    (base, cb + base, cr + base)
                };
                let i = y * w + x;
                g[i] = gv.clamp(0, max) as u16;
                b[i] = bv.clamp(0, max) as u16;
                r[i] = rv.clamp(0, max) as u16;
            }
        }
        let mut v = vec![g, b, r];
        if p.extra_plane {
            let max = max as u16;
            v.push(
                crop(&ctx.bufs[3], &sizes[3])
                    .into_iter()
                    .map(|a| a.min(max))
                    .collect(),
            );
        }
        v
    } else {
        ctx.bufs
            .iter()
            .zip(&sizes)
            .map(|(b, pl)| crop(b, pl))
            .collect()
    };
    Ok(SliceOut {
        ok: true,
        rect,
        planes,
        picture_structure,
        sar,
    })
}

/// Puts the slices together into the frame's planes.
pub fn assemble(
    p: &Params,
    (fw, fh): (u32, u32),
    keyframe: bool,
    outs: Vec<SliceOut>,
) -> Result<Picture> {
    let (w, h) = (fw as usize, fh as usize);
    let wide = p.bits > 8;
    let bps = if wide { 2 } else { 1 };
    let rgb = p.colour_space == ColourSpace::Rgb;
    let (cw, ch) = if rgb || !p.chroma_planes {
        (w, h)
    } else {
        (w.div_ceil(1 << p.log2_h), h.div_ceil(1 << p.log2_v))
    };
    let mut planes: [Vec<u8>; 3] = [vec![0u8; w * h * bps], Vec::new(), Vec::new()];
    if p.chroma_planes {
        planes[1] = vec![0u8; cw * ch * bps];
        planes[2] = vec![0u8; cw * ch * bps];
    }
    let mut alpha = p.extra_plane.then(|| vec![0u8; w * h * bps]);
    let mut damaged = 0;
    let (mut structure, mut sar) = (0, (0, 0));
    let mut first = true;
    for o in outs {
        if !o.ok {
            damaged += 1;
            continue;
        }
        if first {
            structure = o.picture_structure;
            sar = o.sar;
            first = false;
        }
        let (x0, y0, sw, _) = o.rect;
        for (i, src) in o.planes.iter().enumerate() {
            let chroma = !rgb && p.chroma_planes && (i == 1 || i == 2);
            let (dw, dh, px, py, pw) = if chroma {
                (
                    cw,
                    ch,
                    x0 >> p.log2_h,
                    y0 >> p.log2_v,
                    sw.div_ceil(1 << p.log2_h),
                )
            } else {
                (w, h, x0, y0, sw)
            };
            let dst: &mut Vec<u8> = if i < 3 && (rgb || p.chroma_planes || i == 0) {
                &mut planes[i]
            } else {
                match alpha.as_mut() {
                    Some(a) => a,
                    None => continue,
                }
            };
            if pw == 0 {
                continue;
            }
            for (sy, row) in src.chunks_exact(pw).enumerate() {
                let y = py + sy;
                if y >= dh || px >= dw {
                    break;
                }
                let n = pw.min(dw - px);
                let at = (y * dw + px) * bps;
                if wide {
                    for (d, &s) in dst[at..at + 2 * n].chunks_exact_mut(2).zip(row) {
                        d.copy_from_slice(&s.to_le_bytes());
                    }
                } else {
                    for (d, &s) in dst[at..at + n].iter_mut().zip(row) {
                        *d = s as u8;
                    }
                }
            }
        }
    }
    if !p.chroma_planes {
        let _ = (cw, ch);
    }
    Ok(Picture {
        width: fw,
        height: fh,
        bits: p.bits,
        layout: if rgb {
            Layout::Rgb
        } else if p.chroma_planes {
            Layout::Ycbcr {
                log2_h: p.log2_h,
                log2_v: p.log2_v,
            }
        } else {
            Layout::Grey
        },
        planes,
        alpha,
        keyframe,
        damaged_slices: damaged,
        picture_structure: structure,
        sar,
    })
}

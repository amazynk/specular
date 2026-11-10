//! The composite of a file without a merged image ("Maximize compatibility" off), built from
//! its layers (such files were refused).
//!
//! Layers are drawn bottom-most first onto a transparent canvas in the document's colour
//! space, each with its transparency, user mask, opacity and fill opacity
//! ([`crate::layer::planes`]) and its blend mode (the W3C compositing formulas: separable
//! modes per channel, hue, saturation, colour and luminosity on the first three channels).
//! Clipped layers paint only where their base layer is, inheriting its opacity. Groups are
//! isolated (drawn on their own canvas, then blended with the group's mode and opacity)
//! unless they pass through; a group's user and vector masks apply to what it draws. Fill
//! and shape layers draw their stored pixels, or else their solid colour, gradient or
//! pattern ([`crate::fill`]) through their masks; vector masks ([`crate::vector`]) clip pixel
//! layers too; a shape without stored pixels gets its stroke (`vstk`). Adjustment layers
//! ([`crate::adjust`]) change the colours below them through their masks; layer effects
//! ([`crate::effects`]) are drawn under and on their layer or group. A pass-through group
//! with a fill opacity, effects or clipped layers is composited on its own, its colours
//! still blended with the backdrop. Artboards are drawn on their background and clipped to
//! their rectangle. Knockout (`knko`) shows, where the layer or group is, the backdrop at the
//! start of the enclosing group (shallow) or of the nearest isolated group, else the
//! background layer (deep). Blend If ranges hide the layer by its own and the underlying
//! values; "Blend Clipped Layers as Group" off draws clipped layers straight onto the
//! backdrop; "Blend Interior Effects as Group" lets fill opacity fade the interior effects,
//! which otherwise blend with what is under the layer; the eight special modes take their
//! fill opacity in the blend. [`skipped`] counts the visible adjustment layers that are not
//! drawn and [`notes`] names everything left out, so the caller can say so.

use std::sync::atomic::AtomicBool;

use rayon::prelude::*;

use crate::layer;
use crate::{
    Artboard, ArtboardBackground, ColorMode, CompositeNotes, Error, Layer, LayerKind, Options,
    Planes, Psd, Result, Samples,
};

/// One node of the layer tree, siblings bottom-most first.
enum Node {
    Layer(usize),
    Group(Option<usize>, Vec<Node>),
}

/// The layer records as a tree: a group's divider ([`LayerKind::GroupEnd`]) opens it below
/// its children, its folder record ([`LayerKind::GroupStart`]) closes it above them.
fn tree(layers: &[Layer]) -> Vec<Node> {
    let mut stack: Vec<Vec<Node>> = vec![Vec::new()];
    for (i, l) in layers.iter().enumerate() {
        match l.kind {
            LayerKind::GroupEnd => stack.push(Vec::new()),
            LayerKind::GroupStart { .. } => {
                if stack.len() > 1 {
                    let children = stack.pop().unwrap_or_default();
                    if let Some(top) = stack.last_mut() {
                        top.push(Node::Group(Some(i), children));
                    }
                }
            }
            _ => {
                if let Some(top) = stack.last_mut() {
                    top.push(Node::Layer(i));
                }
            }
        }
    }
    // Unclosed groups: their children stay where they are.
    while stack.len() > 1 {
        let children = stack.pop().unwrap_or_default();
        if let Some(top) = stack.last_mut() {
            top.push(Node::Group(None, children));
        }
    }
    stack.pop().unwrap_or_default()
}

/// Visible adjustment layers that are not drawn (colour lookup, unknown settings, a colour
/// mode they are not drawn in) under visible groups.
pub(crate) fn skipped(psd: &Psd, data: &[u8]) -> usize {
    fn count(psd: &Psd, data: &[u8], nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|n| match n {
                Node::Layer(i) => {
                    let l = &psd.layers[*i];
                    usize::from(
                        l.visible
                            && is_adjustment(l)
                            && crate::adjust::parse(psd, data, l).is_none(),
                    )
                }
                Node::Group(g, ch) => {
                    if g.is_none_or(|g| psd.layers[g].visible) {
                        count(psd, data, ch)
                    } else {
                        0
                    }
                }
            })
            .sum()
    }
    count(psd, data, &tree(&psd.layers))
}

/// Whether the file's composite can be built from its layers: a colour mode whose layers
/// are decoded, and at least one layer record. Layers with nothing to draw (adjustment
/// layers alone, empty layers) give a transparent canvas, as Photoshop shows them.
pub(crate) fn possible(psd: &Psd) -> bool {
    layer::mode_supported(psd.info.mode) && !psd.layers.is_empty()
}

/// A straight-alpha canvas: `cc` colour planes then alpha, 0..1 (32-bit documents' linear
/// values as they are).
#[derive(Clone)]
pub(crate) struct Canvas {
    pub cc: usize,
    pub n: usize,
    pub v: Vec<f32>,
}

impl Canvas {
    pub(crate) fn new(cc: usize, n: usize) -> Canvas {
        Canvas {
            cc,
            n,
            v: vec![0.0; (cc + 1) * n],
        }
    }
}

struct Ctx<'a> {
    psd: &'a Psd,
    data: &'a [u8],
    opts: &'a Options,
    cancel: Option<&'a AtomicBool>,
    cc: usize,
    n: usize,
    /// Hue, saturation, colour and luminosity work on these three channels as RGB.
    rgb_like: bool,
    truncated: bool,
    /// Effects left out, by [`crate::effects::Kind`] bit (oracle diagnostics only).
    skip_fx: u32,
}

/// A layer's planes on the canvas as a straight-alpha [`Canvas`], its opacities left out.
fn layer_canvas(ctx: &mut Ctx, i: usize) -> Result<Canvas> {
    let p = layer::drawn_planes(ctx.psd, ctx.data, i, ctx.opts, ctx.cancel, false)?;
    ctx.truncated |= p.truncated;
    let mut c = Canvas::new(ctx.cc, ctx.n);
    let k = (ctx.cc + 1) * ctx.n;
    match &p.samples {
        Samples::U8(s) => {
            for (d, &v) in c.v.iter_mut().zip(&s[..k.min(s.len())]) {
                *d = f32::from(v) / 255.0;
            }
        }
        Samples::U16(s) => {
            for (d, &v) in c.v.iter_mut().zip(&s[..k.min(s.len())]) {
                *d = f32::from(v) / 65535.0;
            }
        }
        Samples::F32(s) => c.v[..k.min(s.len())].copy_from_slice(&s[..k.min(s.len())]),
    }
    Ok(c)
}

/// The coverage of layer `i`'s vector mask, when it has an enabled one.
fn vector_coverage(ctx: &Ctx, i: usize) -> Option<Vec<f32>> {
    let l = &ctx.psd.layers[i];
    // Its rendering is stored as the user mask, which is applied instead.
    if l.mask.is_some_and(|m| m.rendered && !m.disabled) {
        return None;
    }
    let vm = crate::vector::parse(ctx.data.get(l.vector_mask.clone()?)?)?;
    if vm.disabled {
        return None;
    }
    let info = &ctx.psd.info;
    let mut c = vm.raster(info.width as usize, info.height as usize);
    if l.vector_mask_density != 255 {
        let d = f32::from(l.vector_mask_density) / 255.0;
        c.iter_mut().for_each(|v| *v = 1.0 - (1.0 - *v) * d);
    }
    Some(c)
}

/// Multiplies a canvas's alpha by `m`.
fn mask_alpha(c: &mut Canvas, m: &[f32]) {
    for (a, &v) in c.v[c.cc * c.n..].iter_mut().zip(m) {
        *a *= v;
    }
}

/// Layer `i` as drawn, with its masks (not its opacities); `None` when it paints nothing (no
/// pixels, or a fill that cannot be drawn). Pixel layers and shape layers with stored
/// pixels draw those (a shape's stored pixels already follow its path); fill layers without
/// them are painted from their settings over the canvas, then masked.
fn layer_source(ctx: &mut Ctx, i: usize) -> Result<Option<Canvas>> {
    let l = &ctx.psd.layers[i];
    let fill_layer = l.fill.is_some();
    // A softened vector mask shows the fill beyond the stored pixels (which follow the
    // path): then it is painted.
    let soft = l.vector_mask.is_some()
        && (l.vector_mask_density != 255 || l.mask.is_some_and(|m| m.rendered && m.density != 255));
    let paint = fill_layer && soft && crate::fill::render(ctx.psd, ctx.data, l, 0).is_some();
    if !paint && (l.has_pixels() || (fill_layer && layer::stored_pixels(l))) {
        let mut c = layer_canvas(ctx, i)?;
        if !fill_layer && let Some(v) = vector_coverage(ctx, i) {
            mask_alpha(&mut c, &v);
        }
        return Ok(Some(c));
    }
    if !fill_layer {
        return Ok(None);
    }
    paint_fill(ctx, i)
}

/// Fill or shape layer `i` painted from its settings (fill, vector mask, stroke, user
/// mask), `None` when they cannot be drawn.
fn paint_fill(ctx: &Ctx, i: usize) -> Result<Option<Canvas>> {
    let l = &ctx.psd.layers[i];
    let Some(v) = crate::fill::render(ctx.psd, ctx.data, l, ctx.cc) else {
        return Ok(None);
    };
    let mut c = Canvas {
        cc: ctx.cc,
        n: ctx.n,
        v,
    };
    let stroke = crate::fill::shape_stroke(ctx.data, l);
    if let Some(v) = vector_coverage(ctx, i) {
        mask_alpha(&mut c, &v);
    }
    if let Some(st) = &stroke {
        if !st.fill_enabled {
            c.v[ctx.cc * ctx.n..].fill(0.0);
        }
        stroke_onto(ctx, l, st, &mut c);
    }
    if let Some(m) = layer::mask_plane(ctx.psd, ctx.data, i, ctx.cancel)? {
        mask_alpha(&mut c, &m);
    }
    Ok(Some(c))
}

/// Draws a shape layer's stroke (along its vector mask's paths) onto its fill `c`.
fn stroke_onto(ctx: &Ctx, l: &Layer, st: &crate::fill::ShapeStroke, c: &mut Canvas) {
    let Some(vm) = l
        .vector_mask
        .clone()
        .and_then(|r| ctx.data.get(r))
        .and_then(crate::vector::parse)
    else {
        return;
    };
    let info = &ctx.psd.info;
    let (w, h) = (info.width as usize, info.height as usize);
    let bounds = vm
        .bounds(w, h)
        .map_or((0.0, 0.0, w as f64, h as f64), |(l, t, r, b)| {
            (l, t, r - l, b - t)
        });
    let origin = crate::fill::reference_point(ctx.data, l);
    let Some(paint) = crate::fill::stroke_paint(ctx.psd, ctx.data, st, ctx.cc, bounds, origin)
    else {
        return;
    };
    let mut src = Canvas {
        cc: ctx.cc,
        n: ctx.n,
        v: paint,
    };
    let cov = vm.stroke_raster(w, h, &st.style);
    mask_alpha(&mut src, &cov);
    draw(c, &src, &st.mode, st.opacity, false, ctx.rgb_like);
}

/// A group's user and vector masks multiplied, when it has either.
fn group_mask(ctx: &Ctx, g: usize) -> Result<Option<Vec<f32>>> {
    let user = layer::mask_plane(ctx.psd, ctx.data, g, ctx.cancel)?;
    let vector = vector_coverage(ctx, g);
    Ok(match (user, vector) {
        (Some(mut u), Some(v)) => {
            u.iter_mut().zip(&v).for_each(|(a, b)| *a *= b);
            Some(u)
        }
        (u, v) => u.or(v),
    })
}

/// Separable blend `B(cb, cs)` of one channel, 0..1, for a Photoshop blend mode key
/// ([`specular_blend`]; normal, dissolve and unknown keys give the source).
#[inline]
pub(crate) fn blend1(mode: &[u8; 4], cb: f32, cs: f32) -> f32 {
    match specular_blend::Mode::from_photoshop(mode) {
        Some(m) => specular_blend::blend1(m, cb, cs),
        None => cs,
    }
}

/// Non-separable blend of three channels, `None` for separable modes.
#[inline]
pub(crate) fn blend3(mode: &[u8; 4], cb: [f32; 3], cs: [f32; 3]) -> Option<[f32; 3]> {
    let m = specular_blend::Mode::from_photoshop(mode).filter(|m| m.non_separable())?;
    Some(specular_blend::blend3(m, cb, cs))
}

/// Draws `src` over `dst` with blend `mode` and `src`'s alpha scaled by `k`; with `atop`,
/// only where `dst` is and keeping its alpha (a clipped layer on its base).
// `j` indexes the same pixel in every plane of the band.
#[allow(clippy::needless_range_loop)]
pub(crate) fn draw(
    dst: &mut Canvas,
    src: &Canvas,
    mode: &[u8; 4],
    k: f32,
    atop: bool,
    rgb_like: bool,
) {
    let (cc, n) = (dst.cc, dst.n);
    let three = rgb_like && cc >= 3 && blend3(mode, [0.0; 3], [0.0; 3]).is_some();
    // Rows of pixels in parallel: planes are split into bands of the same pixels.
    const BAND: usize = 4096;
    let bands = n.div_ceil(BAND);
    let dv = &mut dst.v;
    let sv = &src.v;
    // Per band: the band's slice of each plane.
    let mut planes: Vec<&mut [f32]> = dv.chunks_mut(n).collect();
    let mut per_band: Vec<Vec<&mut [f32]>> = (0..bands).map(|_| Vec::new()).collect();
    for p in planes.drain(..) {
        for (b, chunk) in p.chunks_mut(BAND).enumerate() {
            per_band[b].push(chunk);
        }
    }
    per_band.into_par_iter().enumerate().for_each(|(b, mut d)| {
        let base = b * BAND;
        let len = d[0].len();
        for j in 0..len {
            let i = base + j;
            let a_s = (sv[cc * n + i] * k).clamp(0.0, 1.0);
            if a_s <= 0.0 {
                continue;
            }
            let a_b = d[cc][j].clamp(0.0, 1.0);
            if atop && a_b <= 0.0 {
                continue;
            }
            let mut cr = [0f32; 16];
            if three {
                let cb3 = [d[0][j], d[1][j], d[2][j]];
                let cs3 = [sv[i], sv[n + i], sv[2 * n + i]];
                let b3 = blend3(mode, cb3, cs3).unwrap_or(cs3);
                cr[..3].copy_from_slice(&b3);
                for (c, v) in cr.iter_mut().enumerate().take(cc.min(16)).skip(3) {
                    *v = sv[c * n + i];
                }
            } else {
                for (c, v) in cr.iter_mut().enumerate().take(cc.min(16)) {
                    let cs = sv[c * n + i];
                    *v = blend1(mode, d[c][j], cs);
                }
            }
            if atop {
                // Source atop: the backdrop's alpha stays; colour mixes by the source's.
                for (c, &r) in cr.iter().enumerate().take(cc.min(16)) {
                    let mixed = (1.0 - a_b) * sv[c * n + i] + a_b * r;
                    d[c][j] += (mixed - d[c][j]) * a_s;
                }
                continue;
            }
            let a_o = a_s + a_b * (1.0 - a_s);
            for (c, &r) in cr.iter().enumerate().take(cc.min(16)) {
                let cs = sv[c * n + i];
                let cb = d[c][j];
                // W3C: the blended colour where both are, the source's where only it is.
                let mixed = (1.0 - a_b) * cs + a_b * r;
                d[c][j] = (a_s * mixed + (1.0 - a_s) * a_b * cb) / a_o;
            }
            d[cc][j] = a_o;
        }
    });
}

/// Whether layer `i` is an adjustment layer (not a fill).
fn is_adjustment(l: &Layer) -> bool {
    l.kind == LayerKind::Adjustment && l.fill.is_none()
}

/// Applies adjustment layer `i` to the colours of `dst` through its masks, opacity and blend
/// mode (transparency stays). False when it is not drawn.
fn adjust_onto(ctx: &Ctx, i: usize, dst: &mut Canvas) -> Result<bool> {
    let l = &ctx.psd.layers[i];
    let Some(adj) = crate::adjust::parse(ctx.psd, ctx.data, l) else {
        return Ok(false);
    };
    let mut mask = layer::mask_plane(ctx.psd, ctx.data, i, ctx.cancel)?;
    if let Some(v) = vector_coverage(ctx, i) {
        match mask.as_mut() {
            Some(m) => m.iter_mut().zip(&v).for_each(|(a, b)| *a *= b),
            None => mask = Some(v),
        }
    }
    let k = f32::from(l.opacity) / 255.0 * f32::from(l.fill_opacity) / 255.0;
    if k <= 0.0 {
        return Ok(true);
    }
    let (cc, n) = (dst.cc, dst.n);
    let mode = l.blend_mode;
    let normal = matches!(&mode, b"norm" | b"diss" | b"pass");
    let rgb = cc >= 3;
    let v = &dst.v;
    let new: Vec<[f32; 3]> = (0..n)
        .into_par_iter()
        .map(|j| {
            let m = mask.as_ref().map_or(1.0, |m| m[j]) * k;
            if m <= 0.0 || v[cc * n + j] <= 0.0 {
                return [f32::NAN; 3];
            }
            let c = if rgb {
                [v[j], v[n + j], v[2 * n + j]]
            } else {
                [v[j]; 3]
            };
            let a = adj.apply(c);
            let a = if rgb {
                a
            } else {
                // Grey: the tables' first channel, else the result's luma.
                let g = match &adj {
                    crate::adjust::Adjust::Tables(_) => a[0],
                    _ => 0.299 * a[0] + 0.587 * a[1] + 0.114 * a[2],
                };
                [g; 3]
            };
            let mixed = if normal {
                a
            } else if rgb && let Some(b3) = blend3(&mode, c, a) {
                b3
            } else {
                [0, 1, 2].map(|q| blend1(&mode, c[q], a[q]))
            };
            [0, 1, 2].map(|q| c[q] + (mixed[q] - c[q]) * m)
        })
        .collect();
    for (j, px) in new.iter().enumerate() {
        if px[0].is_nan() {
            continue;
        }
        for (q, &val) in px.iter().enumerate().take(cc.min(3)) {
            dst.v[q * n + j] = val;
        }
    }
    Ok(true)
}

/// The opacity and fill opacity of record `r` (a group without a record: none).
fn opacities(r: Option<&Layer>) -> (f32, f32) {
    r.map_or((1.0, 1.0), |r| {
        (
            f32::from(r.opacity) / 255.0,
            f32::from(r.fill_opacity) / 255.0,
        )
    })
}

/// Layer `l`'s knockout (`knko`): 0 none, 1 shallow, 2 deep.
fn knockout(data: &[u8], l: &Layer) -> u8 {
    l.block(b"knko")
        .and_then(|r| data.get(r))
        .and_then(|b| b.first().copied())
        .map_or(0, |v| v.min(2))
}

/// Whether a visible layer or group among `nodes` (at any depth) knocks out.
fn has_knockout(psd: &Psd, data: &[u8], nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Layer(i) => {
            let l = &psd.layers[*i];
            l.visible && knockout(data, l) != 0
        }
        Node::Group(g, ch) => {
            g.is_some_and(|g| {
                let l = &psd.layers[g];
                l.visible && knockout(data, l) != 0
            }) || g.is_none_or(|g| psd.layers[g].visible) && has_knockout(psd, data, ch)
        }
    })
}

/// What knockout layers in a sibling list show through to: the backdrop at the start of the
/// enclosing group (shallow) and at the start of the nearest isolated group, or the
/// document's background layer (deep). `None` when nothing below knocks out.
#[derive(Clone, Copy, Default)]
struct Knock<'a> {
    shallow: Option<&'a Canvas>,
    deep: Option<&'a Canvas>,
}

impl<'a> Knock<'a> {
    /// The backdrop record `l` knocks through to, if it knocks out.
    fn target(self, data: &[u8], l: Option<&Layer>) -> Option<&'a Canvas> {
        match l.map_or(0, |l| knockout(data, l)) {
            1 => self.shallow,
            2 => self.deep,
            _ => None,
        }
    }
}

/// Draws the sibling list `nodes` onto `dst`, bottom-most first.
fn render(ctx: &mut Ctx, nodes: &[Node], dst: &mut Canvas, kn: Knock) -> Result<()> {
    let layers = &ctx.psd.layers;
    let mut k = 0;
    while k < nodes.len() {
        if ctx
            .cancel
            .is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        {
            return Err(Error::Cancelled);
        }
        // The clipped layers right above this node (a clipping base).
        let mut end = k + 1;
        while let Some(Node::Layer(j)) = nodes.get(end) {
            if !layers[*j].clipping {
                break;
            }
            end += 1;
        }
        let (rec, visible) = match &nodes[k] {
            Node::Layer(i) => {
                let l = &layers[*i];
                if l.clipping {
                    // Clipped to nothing drawn (its base was skipped): hidden.
                    k += 1;
                    continue;
                }
                if is_adjustment(l) {
                    if l.visible {
                        adjust_onto(ctx, *i, dst)?;
                    }
                    k += 1;
                    continue;
                }
                (Some(*i), l.visible)
            }
            Node::Group(g, _) => (*g, g.is_none_or(|g| layers[g].visible)),
        };
        if !visible {
            // A hidden base hides its clipped layers.
            k = end;
            continue;
        }
        let r = rec.map(|i| &layers[i]);
        let (op, fill) = opacities(r);
        let fx =
            rec.and_then(|i| crate::effects::parse_skipping(ctx.data, &layers[i], ctx.skip_fx));
        let clipped = end > k + 1;
        let mut mode = r.map_or(*b"pass", |r| r.blend_mode);
        let knock = kn.target(ctx.data, r);
        if let Node::Group(g, children) = &nodes[k] {
            let gmask = match g {
                Some(g) => group_mask(ctx, *g)?,
                None => None,
            };
            // Knockouts inside: a snapshot of where this group starts.
            let inner = has_knockout(ctx.psd, ctx.data, children);
            let board = g.and_then(|g| artboard_of(ctx.psd, ctx.data, g));
            if let Some(a) = board {
                // An artboard: its background, its layers, clipped to its rectangle.
                let mut own = Canvas::new(dst.cc, dst.n);
                let info = &ctx.psd.info;
                let (w, h) = (info.width as usize, info.height as usize);
                let (xs, ys) = board_span(&a, w, h);
                if let Some(c) = board_colour(&a, info.mode, dst.cc, info.depth == 32) {
                    let n = dst.n;
                    for y in ys.clone() {
                        for x in xs.clone() {
                            for (q, v) in c.iter().enumerate() {
                                own.v[q * n + y * w + x] = *v;
                            }
                            own.v[dst.cc * n + y * w + x] = 1.0;
                        }
                    }
                }
                let start = inner.then(|| own.clone());
                let kn2 = Knock {
                    shallow: start.as_ref(),
                    deep: start.as_ref(),
                };
                render(ctx, children, &mut own, kn2)?;
                let n = dst.n;
                for y in 0..h {
                    for x in 0..w {
                        if !(ys.contains(&y) && xs.contains(&x)) {
                            own.v[dst.cc * n + y * w + x] = 0.0;
                        }
                    }
                }
                if let Some(m) = &gmask {
                    mask_alpha(&mut own, m);
                }
                draw(dst, &own, b"norm", op, false, ctx.rgb_like);
                k = end;
                continue;
            }
            if &mode == b"pass" && fill >= 1.0 && fx.is_none() && !clipped && knock.is_none() {
                let start = inner.then(|| dst.clone());
                let kn2 = Knock {
                    shallow: start.as_ref(),
                    deep: kn.deep,
                };
                if op >= 1.0 && gmask.is_none() {
                    render(ctx, children, dst, kn2)?;
                } else {
                    // Drawn on a copy, then mixed with the backdrop by the opacity and the
                    // group's masks.
                    let mut over = dst.clone();
                    render(ctx, children, &mut over, kn2)?;
                    // Mixed premultiplied, so a transparent backdrop does not darken it.
                    let (cc, n) = (dst.cc, dst.n);
                    for j in 0..n {
                        let t = gmask.as_ref().map_or(1.0, |m| m[j]) * op;
                        let (ad, ao) = (dst.v[cc * n + j], over.v[cc * n + j]);
                        let a = ad + (ao - ad) * t;
                        for q in 0..cc {
                            let (cd, co) = (dst.v[q * n + j] * ad, over.v[q * n + j] * ao);
                            let c = cd + (co - cd) * t;
                            dst.v[q * n + j] = if a > 1e-9 { c / a } else { 0.0 };
                        }
                        dst.v[cc * n + j] = a;
                    }
                }
                k += 1;
                continue;
            }
            let mut own = Canvas::new(dst.cc, dst.n);
            {
                let start = inner.then(|| own.clone());
                let kn2 = Knock {
                    shallow: start.as_ref(),
                    deep: start.as_ref(),
                };
                render(ctx, children, &mut own, kn2)?;
            }
            if &mode == b"pass" {
                // A pass-through group with a fill opacity, effects, clipped layers or a
                // knockout: its contents blended with the backdrop (the one it knocks through
                // to, if any), shown where they are (the transparency they have on their own).
                let back: &Canvas = knock.unwrap_or(dst);
                let mut over = back.clone();
                let start = inner.then(|| back.clone());
                let kn2 = Knock {
                    shallow: start.as_ref(),
                    deep: kn.deep,
                };
                render(ctx, children, &mut over, kn2)?;
                // The colour that, laid over the backdrop at the contents' own transparency,
                // gives the pass-through result.
                let (cc, n) = (dst.cc, dst.n);
                for j in 0..n {
                    let a = own.v[cc * n + j];
                    if a <= 1e-6 {
                        continue;
                    }
                    for q in 0..cc {
                        let (b, o) = (back.v[q * n + j], over.v[q * n + j]);
                        own.v[q * n + j] = (b + (o - b) / a).clamp(0.0, 1.0);
                    }
                }
                mode = *b"norm";
            }
            if let Some(m) = &gmask {
                mask_alpha(&mut own, m);
            }
            let job = Finish {
                rec: r,
                clipped: &nodes[k + 1..end],
                fx: fx.as_ref(),
                mode,
                op,
                fill,
                knock,
            };
            finish(ctx, &job, own, dst)?;
        } else if let Some(i) = rec
            && let Some(base) = layer_source(ctx, i)?
        {
            let job = Finish {
                rec: r,
                clipped: &nodes[k + 1..end],
                fx: fx.as_ref(),
                mode,
                op,
                fill,
                knock,
            };
            finish(ctx, &job, base, dst)?;
        }
        k = end;
    }
    Ok(())
}

/// How a layer or isolated group is finished onto its backdrop.
struct Finish<'a> {
    /// Its record (a group without one: none).
    rec: Option<&'a Layer>,
    /// The layers clipped to it.
    clipped: &'a [Node],
    fx: Option<&'a crate::effects::Effects>,
    mode: [u8; 4],
    op: f32,
    fill: f32,
    /// The backdrop it knocks through to, if it knocks out.
    knock: Option<&'a Canvas>,
}

/// The first byte of record `l`'s block `key`, `default` without one.
fn flag(data: &[u8], l: Option<&Layer>, key: &[u8; 4], default: u8) -> u8 {
    l.and_then(|l| l.block(key))
        .and_then(|r| data.get(r))
        .and_then(|b| b.first().copied())
        .unwrap_or(default)
}

/// The blend modes whose fill opacity works differently from their opacity (Photoshop's
/// "special eight").
fn fill_special(mode: &[u8; 4]) -> bool {
    matches!(
        mode,
        b"idiv" | b"lbrn" | b"div " | b"lddg" | b"vLit" | b"lLit" | b"hMix" | b"diff"
    )
}

/// Draws a layer or isolated group (its unfaded content `base`) onto `dst`: its effects
/// under it, its fill opacity, the clipped layers, its effects on it, then the whole with
/// its blend mode and opacity. "Blend Interior Effects as Group" draws the overlays, satin
/// and inner glow before the fill opacity fades the content; "Blend Clipped Layers as Group"
/// off draws the clipped layers straight onto the backdrop, each with its own blend mode,
/// where the base is.
fn finish(ctx: &mut Ctx, job: &Finish, mut base: Canvas, dst: &mut Canvas) -> Result<()> {
    use crate::effects::Inside;
    let info = &ctx.psd.info;
    let fxc = crate::effects::Fx {
        psd: ctx.psd,
        data: ctx.data,
        w: info.width as usize,
        h: info.height as usize,
        rgb_like: ctx.rgb_like,
        origin: job
            .rec
            .map_or((0.0, 0.0), |l| crate::fill::reference_point(ctx.data, l)),
        cover: job.fill * job.op,
    };
    let (cc, n) = (base.cc, base.n);
    let as_group = flag(ctx.data, job.rec, b"clbl", 1) != 0 || job.clipped.is_empty();
    let interior = flag(ctx.data, job.rec, b"infx", 0) != 0 && job.fill < 1.0;
    let weights = job.rec.and_then(|l| blend_if(l, &base, dst, ctx.rgb_like));
    let shape =
        (job.fx.is_some() || job.knock.is_some() || !as_group).then(|| base.v[cc * n..].to_vec());
    if let (Some(fx), Some(a)) = (job.fx, &shape) {
        fx.draw_behind(&fxc, a, dst, job.op);
    }
    if interior && let (Some(fx), Some(a)) = (job.fx, &shape) {
        fx.draw_inside(&fxc, a, &mut base, Inside::Interior);
    }
    // The special modes take their fill opacity in the blend itself.
    let special =
        job.fill < 1.0 && fill_special(&job.mode) && job.clipped.is_empty() && job.knock.is_none();
    if job.fill < 1.0 && !special {
        base.v[cc * n..].iter_mut().for_each(|a| *a *= job.fill);
    }
    if as_group {
        for node in job.clipped {
            clipped_onto(ctx, node, &mut base, None)?;
        }
    }
    // Effects with their own blend modes (or on a layer with one) blend with what is under
    // the layer too: drawn onto the backdrop after it.
    let on_backdrop = !interior
        && job.knock.is_none()
        && job.fx.is_some_and(|fx| {
            let (any, blends) = fx.interior_blends();
            any && (blends || !matches!(&job.mode, b"norm" | b"pass" | b"diss"))
        });
    if let (Some(fx), Some(a)) = (job.fx, &shape) {
        let which = if interior || on_backdrop {
            Inside::Rest
        } else {
            Inside::All
        };
        fx.draw_inside(&fxc, a, &mut base, which);
    }
    if let Some(w) = &weights {
        mask_alpha(&mut base, w);
    }
    match (job.knock, &shape) {
        (Some(back), Some(a)) => knock_onto(dst, back, &base, a, &job.mode, job.op, ctx.rgb_like),
        _ if special => draw_fill(dst, &base, &job.mode, job.op, job.fill),
        _ => draw(dst, &base, &job.mode, job.op, false, ctx.rgb_like),
    }
    if on_backdrop && let (Some(fx), Some(a)) = (job.fx, &shape) {
        fx.draw_interior_on_backdrop(&fxc, a, dst, job.op);
    }
    if !as_group && let Some(a) = &shape {
        let clip: Vec<f32> = a.iter().map(|v| v * job.op).collect();
        for node in job.clipped {
            clipped_onto(ctx, node, dst, Some(&clip))?;
        }
    }
    Ok(())
}

/// Draws clipped layer `node` with its effects onto `target`: on its base (`clip` `None`:
/// the base is `target`, its transparency kept), or straight onto the backdrop where the
/// base's shape `clip` is.
fn clipped_onto(
    ctx: &mut Ctx,
    node: &Node,
    target: &mut Canvas,
    clip: Option<&[f32]>,
) -> Result<()> {
    let Node::Layer(j) = node else {
        return Ok(());
    };
    let c = &ctx.psd.layers[*j];
    if !c.visible {
        return Ok(());
    }
    if is_adjustment(c) {
        adjust_onto(ctx, *j, target)?;
        return Ok(());
    }
    let Some(mut src) = layer_source(ctx, *j)? else {
        return Ok(());
    };
    let (cop, cfill) = opacities(Some(c));
    let (cc, n) = (src.cc, src.n);
    let fx = crate::effects::parse_skipping(ctx.data, c, ctx.skip_fx);
    let info = &ctx.psd.info;
    let fxc = crate::effects::Fx {
        psd: ctx.psd,
        data: ctx.data,
        w: info.width as usize,
        h: info.height as usize,
        rgb_like: ctx.rgb_like,
        origin: crate::fill::reference_point(ctx.data, c),
        cover: cfill * cop,
    };
    let shape = fx.as_ref().map(|_| src.v[cc * n..].to_vec());
    if let (Some(fx), Some(a)) = (&fx, &shape) {
        // Under it, on the base only.
        match clip {
            Some(m) => {
                let mut under = Canvas::new(cc, n);
                fx.draw_behind(&fxc, a, &mut under, cop);
                mask_alpha(&mut under, m);
                draw(target, &under, b"norm", 1.0, false, ctx.rgb_like);
            }
            None => {
                let kept = target.v[cc * n..].to_vec();
                fx.draw_behind(&fxc, a, target, cop);
                target.v[cc * n..].copy_from_slice(&kept);
            }
        }
    }
    if cfill < 1.0 {
        src.v[cc * n..].iter_mut().for_each(|a| *a *= cfill);
    }
    if let (Some(fx), Some(a)) = (&fx, &shape) {
        fx.draw_inside(&fxc, a, &mut src, crate::effects::Inside::All);
    }
    match clip {
        Some(m) => {
            mask_alpha(&mut src, m);
            draw(target, &src, &c.blend_mode, cop, false, ctx.rgb_like);
        }
        None => draw(target, &src, &c.blend_mode, cop, true, ctx.rgb_like),
    }
    Ok(())
}

/// Blend If: how much of layer `l` (its content `src` over the backdrop `dst`) its ranges
/// let through, `None` when they let everything through. Grey is the luminosity of RGB.
fn blend_if(l: &Layer, src: &Canvas, dst: &Canvas, rgb_like: bool) -> Option<Vec<f32>> {
    if l.blend_if.is_empty() {
        return None;
    }
    let (cc, n) = (src.cc, src.n);
    let open = |r: &[u8]| r == [0, 0, 255, 255];
    let grey = |c: &Canvas, j: usize| {
        if rgb_like && cc >= 3 {
            specular_blend::lum([c.v[j], c.v[n + j], c.v[2 * n + j]])
        } else {
            c.v[j]
        }
    };
    let w: Vec<f32> = (0..n)
        .into_par_iter()
        .map(|j| {
            let mut w = 1.0;
            for (q, r) in l.blend_if.iter().enumerate().take(cc + 1) {
                let (this, under) = (&r[..4], &r[4..]);
                if open(this) && open(under) {
                    continue;
                }
                let (vs, vb) = if q == 0 {
                    (grey(src, j), grey(dst, j))
                } else {
                    (src.v[(q - 1) * n + j], dst.v[(q - 1) * n + j])
                };
                // Nothing underneath counts as black.
                let ab = dst.v[cc * n + j].clamp(0.0, 1.0);
                w *= ramp(this, vs) * ramp(under, vb * ab);
                if w <= 0.0 {
                    break;
                }
            }
            w
        })
        .collect();
    Some(w)
}

/// How much a Blend If range (black low/high, white low/high, 0..255) lets through at value
/// `v` (0..1): nothing outside it, all between the inner sliders, linear between split ones.
fn ramp(r: &[u8], v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0) * 255.0;
    let (bl, bh) = (f32::from(r[0]), f32::from(r[1].max(r[0])));
    let (wl, wh) = (f32::from(r[2]), f32::from(r[3].max(r[2])));
    if v < bl || v > wh {
        0.0
    } else if v < bh {
        (v - bl) / (bh - bl)
    } else if v > wl {
        (wh - v) / (wh - wl)
    } else {
        1.0
    }
}

/// Draws `src` onto `dst` with one of the special modes ([`fill_special`]) at fill opacity
/// `fill` and opacity `op`: where the backdrop is, the fill moves the source towards the
/// mode's neutral colour instead of fading it (Hard Mix: its threshold softens).
fn draw_fill(dst: &mut Canvas, src: &Canvas, mode: &[u8; 4], op: f32, fill: f32) {
    let (cc, n) = (dst.cc, dst.n);
    let f = fill.clamp(0.0, 1.0);
    let blend = |cb: f32, cs: f32| -> f32 {
        match mode {
            b"idiv" | b"lbrn" => blend1(mode, cb, 1.0 - f * (1.0 - cs)),
            b"div " | b"lddg" | b"diff" => blend1(mode, cb, f * cs),
            b"vLit" | b"lLit" => blend1(mode, cb, 0.5 + f * (cs - 0.5)),
            b"hMix" if f < 1.0 => ((cb + f * cs - f) / (1.0 - f)).clamp(0.0, 1.0),
            _ => blend1(mode, cb, cs),
        }
    };
    let dv = &mut dst.v;
    for j in 0..n {
        let a_sh = (src.v[cc * n + j] * op).clamp(0.0, 1.0);
        if a_sh <= 0.0 {
            continue;
        }
        let a_s = a_sh * f;
        let a_b = dv[cc * n + j].clamp(0.0, 1.0);
        let a_o = a_s + a_b * (1.0 - a_s);
        if a_o <= 1e-9 {
            continue;
        }
        for q in 0..cc {
            let (cb, cs) = (dv[q * n + j], src.v[q * n + j]);
            let p = a_s * (1.0 - a_b) * cs + a_b * (a_sh * blend(cb, cs) + (1.0 - a_sh) * cb);
            dv[q * n + j] = p / a_o;
        }
        dv[cc * n + j] = a_o;
    }
}

/// Draws a knockout layer or group (`src`, its unfaded transparency `shape`) onto `dst`:
/// where it is, the backdrop is replaced by `back` (what it knocks through to) with `src`
/// drawn on it, all at opacity `op`.
fn knock_onto(
    dst: &mut Canvas,
    back: &Canvas,
    src: &Canvas,
    shape: &[f32],
    mode: &[u8; 4],
    op: f32,
    rgb_like: bool,
) {
    let (cc, n) = (dst.cc, dst.n);
    // The source at its fill opacity alone, its edges left to the mix below.
    let mut s = src.clone();
    for (a, &m) in s.v[cc * n..].iter_mut().zip(shape) {
        *a = if m > 1e-6 {
            (*a / m).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
    let mut knocked = back.clone();
    draw(&mut knocked, &s, mode, 1.0, false, rgb_like);
    for (j, &m) in shape.iter().enumerate() {
        let t = (m * op).clamp(0.0, 1.0);
        if t <= 0.0 {
            continue;
        }
        let (ad, ak) = (dst.v[cc * n + j], knocked.v[cc * n + j]);
        let a = ad + (ak - ad) * t;
        for q in 0..cc {
            let (cd, ck) = (dst.v[q * n + j] * ad, knocked.v[q * n + j] * ak);
            let c = cd + (ck - cd) * t;
            dst.v[q * n + j] = if a > 1e-9 { c / a } else { 0.0 };
        }
        dst.v[cc * n + j] = a;
    }
}

/// The artboard record `g` (a group) sets up, if any.
fn artboard_of(psd: &Psd, data: &[u8], g: usize) -> Option<Artboard> {
    let l = psd.layers.get(g)?;
    if !matches!(l.kind, LayerKind::GroupStart { .. }) {
        return None;
    }
    let d = [b"artb", b"artd", b"abdd"].iter().find_map(|k| {
        l.block(k)
            .and_then(|r| data.get(r))
            .and_then(crate::desc::versioned)
    })?;
    let r = d.obj(b"artboardRect")?;
    let n = |k: &[u8]| r.num(k).map(|v| v.round().clamp(-1e9, 1e9) as i32);
    let background = match d.num(b"artboardBackgroundType").map(|v| v as i64) {
        Some(2) => ArtboardBackground::Black,
        Some(3) => ArtboardBackground::Transparent,
        Some(4) => {
            let c = d
                .obj(b"Clr ")
                .and_then(crate::fill::desc_rgb)
                .unwrap_or([1.0; 3]);
            ArtboardBackground::Color(c.map(|v| (v * 255.0).round() as u8))
        }
        _ => ArtboardBackground::White,
    };
    Some(Artboard {
        name: l.name.clone(),
        top: n(b"Top ")?,
        left: n(b"Left")?,
        bottom: n(b"Btom")?,
        right: n(b"Rght")?,
        background,
        layer: g,
    })
}

/// The canvas columns and rows artboard `a` covers.
fn board_span(
    a: &Artboard,
    w: usize,
    h: usize,
) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
    let c = |v: i32, n: usize| (v.max(0) as usize).min(n);
    let (x0, x1) = (c(a.left, w), c(a.right, w));
    let (y0, y1) = (c(a.top, h), c(a.bottom, h));
    (x0..x1.max(x0), y0..y1.max(y0))
}

/// An artboard's background in the document's channels, `None` when transparent.
fn board_colour(a: &Artboard, mode: ColorMode, cc: usize, linear: bool) -> Option<Vec<f32>> {
    let rgb = match a.background {
        ArtboardBackground::Transparent => return None,
        ArtboardBackground::White => [255u8; 3],
        ArtboardBackground::Black => [0u8; 3],
        ArtboardBackground::Color(c) => c,
    };
    let d = crate::desc::Desc {
        class: b"RGBC".to_vec(),
        items: vec![
            (b"Rd  ".to_vec(), crate::desc::Value::Num(f64::from(rgb[0]))),
            (b"Grn ".to_vec(), crate::desc::Value::Num(f64::from(rgb[1]))),
            (b"Bl  ".to_vec(), crate::desc::Value::Num(f64::from(rgb[2]))),
        ],
    };
    crate::fill::desc_channels(&d, mode, cc, linear)
}

/// The artboards, top-most first.
pub(crate) fn artboards(psd: &Psd, data: &[u8]) -> Vec<Artboard> {
    (0..psd.layers.len())
        .rev()
        .filter_map(|i| artboard_of(psd, data, i))
        .collect()
}

/// `p` cropped to artboard `a`'s rectangle over the canvas (at least one pixel).
pub(crate) fn crop(p: &Planes, a: &Artboard) -> Planes {
    let (w, h) = (p.width as usize, p.height as usize);
    let (mut xs, mut ys) = board_span(a, w, h);
    if xs.is_empty() || ys.is_empty() {
        xs = 0..w.min(1);
        ys = 0..h.min(1);
    }
    let planes = p.color_channels + usize::from(p.alpha);
    fn cut<T: Copy>(
        s: &[T],
        planes: usize,
        w: usize,
        h: usize,
        xs: &std::ops::Range<usize>,
        ys: &std::ops::Range<usize>,
    ) -> Vec<T> {
        let mut out = Vec::with_capacity(planes * xs.len() * ys.len());
        for q in 0..planes {
            let base = q * w * h;
            for y in ys.clone() {
                if let Some(row) = s.get(base + y * w + xs.start..base + y * w + xs.end) {
                    out.extend_from_slice(row);
                }
            }
        }
        out
    }
    let samples = match &p.samples {
        Samples::U8(s) => Samples::U8(cut(s, planes, w, h, &xs, &ys)),
        Samples::U16(s) => Samples::U16(cut(s, planes, w, h, &xs, &ys)),
        Samples::F32(s) => Samples::F32(cut(s, planes, w, h, &xs, &ys)),
    };
    Planes {
        width: xs.len() as u32,
        height: ys.len() as u32,
        samples,
        palette: p.palette.clone(),
        ..*p
    }
}

/// What the composite built from the layers leaves out or approximates, among visible
/// layers under visible groups.
pub(crate) fn notes(psd: &Psd, data: &[u8]) -> CompositeNotes {
    fn add(v: &mut Vec<&'static str>, s: &'static str) {
        if !v.contains(&s) {
            v.push(s);
        }
    }
    fn walk(psd: &Psd, data: &[u8], nodes: &[Node], out: &mut CompositeNotes) {
        for n in nodes {
            let (i, children) = match n {
                Node::Layer(i) => (Some(*i), None),
                Node::Group(g, ch) => (*g, Some(ch)),
            };
            if let Some(i) = i {
                let l = &psd.layers[i];
                if !l.visible {
                    continue;
                }
                if is_adjustment(l) && crate::adjust::parse(psd, data, l).is_none() {
                    let key = crate::adjust::adjustment_key(l)
                        .copied()
                        .unwrap_or(*b"    ");
                    add(&mut out.not_drawn, crate::adjust::name(&key));
                }
                if crate::effects::legacy_only(data, l) {
                    add(&mut out.not_drawn, "Layer Style (legacy)");
                }
                for name in crate::effects::approximate(data, l) {
                    add(&mut out.approximate, name);
                }
                if crate::effects::has_noise_gradient(data, l)
                    || (crate::fill::noise_fill(data, l) && !layer::stored_pixels(l))
                {
                    add(&mut out.not_drawn, "Noise Gradient");
                }
                // Drawn from Photoshop's description, without one of its merged images to
                // check against.
                if !l.blend_if.is_empty() {
                    add(&mut out.approximate, "Blend If");
                }
            }
            if let Some(ch) = children {
                walk(psd, data, ch, out);
            }
        }
    }
    let mut out = CompositeNotes::default();
    walk(psd, data, &tree(&psd.layers), &mut out);
    out
}

/// Shape or fill layer `i` painted from its settings alone (even when it stores pixels), as
/// straight colour planes then transparency, 0..1.
pub(crate) fn from_settings(psd: &Psd, data: &[u8], i: usize) -> Result<Option<Vec<f32>>> {
    let info = &psd.info;
    if psd.layers.get(i).is_none_or(|l| l.fill.is_none()) {
        return Ok(None);
    }
    let ctx = Ctx {
        psd,
        data,
        opts: &Options { parallel: true },
        cancel: None,
        cc: info.mode.color_channels(info.channels),
        n: info.width as usize * info.height as usize,
        rgb_like: info.mode == ColorMode::Rgb,
        truncated: false,
        skip_fx: 0,
    };
    Ok(paint_fill(&ctx, i)?.map(|c| c.v))
}

/// The composite built from the layers, as [`Planes`] with transparency at the document's
/// depth.
pub(crate) fn composite(
    psd: &Psd,
    data: &[u8],
    opts: &Options,
    cancel: Option<&AtomicBool>,
) -> Result<Planes> {
    composite_skipping(psd, data, opts, cancel, 0)
}

/// [`composite`] with the effects whose [`crate::effects::Kind`] bits are in `skip_fx` left
/// out.
pub(crate) fn composite_skipping(
    psd: &Psd,
    data: &[u8],
    opts: &Options,
    cancel: Option<&AtomicBool>,
    skip_fx: u32,
) -> Result<Planes> {
    let info = &psd.info;
    if !possible(psd) {
        return Err(Error::NoComposite(info.composite));
    }
    let cc = info.mode.color_channels(info.channels);
    let n = info.width as usize * info.height as usize;
    let mut ctx = Ctx {
        psd,
        data,
        opts,
        cancel,
        cc,
        n,
        rgb_like: info.mode == ColorMode::Rgb,
        truncated: false,
        skip_fx,
    };
    let mut canvas = Canvas::new(cc, n);
    let nodes = tree(&psd.layers);
    // Deep knockouts at the top show the background layer, or transparency without one.
    let bottom = has_knockout(psd, data, &nodes).then(|| -> Result<Canvas> {
        let mut c = Canvas::new(cc, n);
        if let Some(first @ Node::Layer(0)) = nodes.first()
            && psd.layers[0].channels.iter().all(|c| c.id != -1)
        {
            render(
                &mut ctx,
                std::slice::from_ref(first),
                &mut c,
                Knock::default(),
            )?;
        }
        Ok(c)
    });
    let bottom = bottom.transpose()?;
    let kn = Knock {
        shallow: bottom.as_ref(),
        deep: bottom.as_ref(),
    };
    render(&mut ctx, &nodes, &mut canvas, kn)?;
    let v = canvas.v;
    let samples = match info.depth {
        8 => Samples::U8(
            v.iter()
                .map(|&x| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
                .collect(),
        ),
        16 => Samples::U16(
            v.iter()
                .map(|&x| (x.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
                .collect(),
        ),
        _ => Samples::F32(v),
    };
    Ok(Planes {
        width: info.width,
        height: info.height,
        mode: info.mode,
        color_channels: cc,
        alpha: true,
        samples,
        truncated: ctx.truncated,
        palette: None,
        transparent_index: None,
        matted_on_white: false,
    })
}

//! Compositing the visible layer stack, the way GIMP's projection does.
//!
//! The canvas is processed in bands of rows. Within a band the stack runs layer at a time
//! over a float RGBA accumulator: each layer's tiles for those rows are decoded (in
//! parallel), converted to the space its mode composites in, and composited row by row (in
//! parallel). Groups render their children into a band of their own (pass-through groups
//! start from a copy of the backdrop) and are then composited like a layer.
//!
//! As in GIMP, the bottom layer of a stack (no backdrop yet) is placed as is, and layers
//! whose mode leaves the backdrop alone where they are transparent are only processed over
//! their own area.

use crate::blend::{self, Composite, Kind, Mode, Space};
use crate::parse::{BaseType, Component, Doc, Node, Plane, PlaneKind};
use crate::pixel::{Loader, f16_to_f32, f32_to_f16, linear_to_srgb, srgb_to_linear};
use crate::{Error, Image, Info, Options, PixelFormat, Result, Xcf};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// Half-open canvas rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

impl Rect {
    fn new(x: i64, y: i64, w: i64, h: i64) -> Option<Rect> {
        (w > 0 && h > 0).then_some(Rect {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        })
    }

    fn of(p: &Plane) -> Option<Rect> {
        Rect::new(
            i64::from(p.x),
            i64::from(p.y),
            i64::from(p.width),
            i64::from(p.height),
        )
    }

    fn intersect(self, o: Rect) -> Option<Rect> {
        let r = Rect {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        };
        (r.x0 < r.x1 && r.y0 < r.y1).then_some(r)
    }

    fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Rect {
                x0: a.x0.min(b.x0),
                y0: a.y0.min(b.y0),
                x1: a.x1.max(b.x1),
                y1: a.y1.max(b.y1),
            }),
            (a, None) => a,
            (None, b) => b,
        }
    }
}

/// A visible layer or group in the render tree.
struct Item {
    node: usize,
    mode: Mode,
    /// A pass-through group.
    pass: bool,
    /// Children, bottom first.
    kids: Vec<Item>,
    /// Nothing is below it in its stack (GIMP places it without compositing).
    last: bool,
    /// Its pixels' extent on the canvas (`None`: nothing).
    src: Option<Rect>,
    /// It cannot change anything.
    skip: bool,
    /// The stack is empty after it.
    empty_after: bool,
}

/// The visible tree, each level bottom first.
fn build(nodes: &[Node]) -> Vec<Item> {
    fn level(nodes: &[Node], i: &mut usize, depth: u32) -> Vec<Item> {
        let mut out = Vec::new();
        while *i < nodes.len() && nodes[*i].depth >= depth {
            let idx = *i;
            *i += 1;
            let n = &nodes[idx];
            let kids = if n.group {
                level(nodes, i, depth + 1)
            } else {
                Vec::new()
            };
            if n.visible {
                out.push(Item {
                    node: idx,
                    mode: blend::resolve(
                        n.mode,
                        n.composite_mode,
                        n.composite_space,
                        n.blend_space,
                    ),
                    pass: n.group && n.mode == blend::PASS_THROUGH,
                    kids,
                    last: false,
                    src: None,
                    skip: false,
                    empty_after: false,
                });
            }
        }
        out.reverse();
        out
    }
    // `fix_depths` makes the first depth 0 and lets depths grow only below groups.
    level(nodes, &mut 0, 0)
}

/// Works out each item's extent, whether it is the bottom of its stack and what it leaves;
/// returns the stack's extent.
fn extents(items: &mut [Item], doc: &Doc, mut acc: Option<Rect>) -> Option<Rect> {
    let canvas = Rect::new(0, 0, i64::from(doc.width), i64::from(doc.height));
    for it in items.iter_mut() {
        let n = &doc.nodes[it.node];
        let last = acc.is_none();
        let src = if n.group {
            extents(&mut it.kids, doc, if it.pass { acc } else { None })
        } else {
            n.plane
                .as_ref()
                .and_then(Rect::of)
                .and_then(|r| canvas.and_then(|c| r.intersect(c)))
        };
        let shows = n.opacity > 0.0;
        it.last = last;
        it.src = src;
        let result = if last {
            it.skip = src.is_none() || !shows;
            if shows { src } else { None }
        } else {
            it.skip = src.is_none() && it.mode.identity_outside();
            let (inc_src, inc_dst) = it.mode.included();
            let mut r = src.and_then(|s| acc.and_then(|a| s.intersect(a)));
            if inc_src && shows {
                r = Rect::union(r, src);
            }
            if inc_dst {
                r = Rect::union(r, acc);
            }
            r
        };
        it.empty_after = result.is_none();
        acc = result;
    }
    acc
}

/// Whether the composite can have transparent pixels: false only when the bottom layer is
/// opaque and covers the canvas and nothing above can lower alpha.
pub(crate) fn may_have_alpha(doc: &Doc) -> bool {
    fn keeps_opaque(items: &[Item], doc: &Doc) -> bool {
        items.iter().all(|it| {
            if it.pass {
                return keeps_opaque(&it.kids, doc);
            }
            matches!(
                it.mode.composite,
                Composite::Union | Composite::ClipToBackdrop
            ) && !matches!(it.mode.kind, Kind::Erase | Kind::Split | Kind::Replace)
                && !it.mode.subtractive
                && !doc.nodes[it.node].broken
        })
    }
    let items = build(&doc.nodes);
    let Some(bottom) = items.first() else {
        return true;
    };
    let n = &doc.nodes[bottom.node];
    let opaque_bottom = !n.group
        && !n.broken
        && n.mask.is_none()
        && n.opacity >= 1.0
        && n.plane.as_ref().is_some_and(|p| {
            let full = (p.width.div_ceil(64) * p.height.div_ceil(64)) as usize;
            matches!(p.kind, PlaneKind::Layer(t) if !t.has_alpha())
                && p.tiles.len() == full
                && p.x <= 0
                && p.y <= 0
                && i64::from(p.x) + i64::from(p.width) >= i64::from(doc.width)
                && i64::from(p.y) + i64::from(p.height) >= i64::from(doc.height)
        });
    !(opaque_bottom && keeps_opaque(&items[1..], doc))
}

/// The pixel format [`render`] produces.
pub(crate) fn output_format(info: &Info, has_icc: bool, has_alpha: bool) -> PixelFormat {
    let c = info.precision.component;
    if info.base == BaseType::Indexed {
        return PixelFormat::Rgba8;
    }
    if c.is_float() && !has_icc {
        return PixelFormat::Rgba16F;
    }
    let eight = c == Component::U8 && !info.precision.linear;
    match (info.base == BaseType::Grey, has_alpha, eight) {
        (true, false, true) => PixelFormat::Gray8,
        (true, false, false) => PixelFormat::Gray16,
        (true, true, _) => PixelFormat::GrayAlpha16,
        (false, _, true) => PixelFormat::Rgba8,
        (false, _, false) => PixelFormat::Rgba16,
    }
}

/// Whether output colour is linear light (else coded with the sRGB curve, or the stored one).
fn output_linear(info: &Info, fmt: PixelFormat, has_icc: bool) -> bool {
    fmt == PixelFormat::Rgba16F
        || (has_icc && info.precision.linear && info.base != BaseType::Indexed)
}

struct Ctx<'a> {
    data: &'a [u8],
    doc: &'a Doc,
    parallel: bool,
    width: usize,
    /// First canvas row of the band.
    y0: usize,
    rows: usize,
    truncated: &'a AtomicBool,
    opts: &'a Options<'a>,
    /// Layers with layer effects, drawn with them over the whole canvas beforehand (straight
    /// R'G'B'A floats), by node.
    filtered: &'a HashMap<usize, Vec<f32>>,
}

/// A band of float RGBA (straight alpha) in one space.
#[derive(Clone)]
struct Acc {
    buf: Vec<f32>,
    space: Space,
}

/// A region within the band: canvas columns `x0..x1`, band rows `r0..r1`.
#[derive(Clone, Copy)]
struct Region {
    x0: usize,
    x1: usize,
    r0: usize,
    r1: usize,
}

impl Region {
    fn width(&self) -> usize {
        self.x1 - self.x0
    }
}

/// A read-only window of a float buffer laid out in rows.
#[derive(Clone, Copy)]
struct View<'a> {
    buf: &'a [f32],
    /// Pixels per row.
    stride: usize,
    x: usize,
    r: usize,
    channels: usize,
}

impl<'a> View<'a> {
    fn row(&self, i: usize, width: usize) -> &'a [f32] {
        let s = ((self.r + i) * self.stride + self.x) * self.channels;
        &self.buf[s..s + width * self.channels]
    }
}

/// Runs `f(row index, row)` over consecutive `stride`-long rows of `buf`.
fn for_rows<T: Send>(par: bool, buf: &mut [T], stride: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    if stride == 0 {
        return;
    }
    if par {
        buf.par_chunks_mut(stride)
            .enumerate()
            .for_each(|(i, r)| f(i, r));
    } else {
        buf.chunks_mut(stride)
            .enumerate()
            .for_each(|(i, r)| f(i, r));
    }
}

fn convert_space(par: bool, acc: &mut Acc, to: Space, width: usize) {
    if acc.space == to {
        return;
    }
    let f: fn(f32) -> f32 = match to {
        Space::Linear => srgb_to_linear,
        Space::NonLinear => linear_to_srgb,
    };
    for_rows(par, &mut acc.buf, width * 4, |_, row| {
        for p in row.chunks_exact_mut(4) {
            p[0] = f(p[0]);
            p[1] = f(p[1]);
            p[2] = f(p[2]);
        }
    });
    acc.space = to;
}

/// Decodes the part of `plane` inside `reg` to floats (`channels` per pixel, zero outside
/// the plane and where tiles are missing).
fn load(
    ctx: &Ctx,
    plane: &Plane,
    reg: Region,
    channels: usize,
    conv: impl Fn(&[u8], &mut [f32]) + Sync,
) -> Vec<f32> {
    let w = reg.width();
    let h = reg.r1 - reg.r0;
    let mut out = vec![0.0f32; w * h * channels];
    let cy0 = (ctx.y0 + reg.r0) as i64;
    let want = Rect {
        x0: reg.x0 as i64,
        y0: cy0,
        x1: reg.x1 as i64,
        y1: cy0 + h as i64,
    };
    let Some(r) = Rect::of(plane).and_then(|p| p.intersect(want)) else {
        return out;
    };
    let (px, py) = (i64::from(plane.x), i64::from(plane.y));
    let t = i64::from(crate::tile::TILE);
    let (tx0, tx1) = ((r.x0 - px) / t, (r.x1 - 1 - px) / t);
    let (ty0, ty1) = ((r.y0 - py) / t, (r.y1 - 1 - py) / t);
    let tiles_x = i64::from(plane.tiles_x());
    let list: Vec<usize> = (ty0..=ty1)
        .flat_map(|ty| (tx0..=tx1).map(move |tx| (ty * tiles_x + tx) as usize))
        .collect();
    let decode = |&idx: &usize| match crate::tile::decode(ctx.data, plane, idx, ctx.doc.compression)
    {
        Ok(Some(t)) => Some(t),
        _ => {
            ctx.truncated.store(true, Ordering::Relaxed);
            None
        }
    };
    let tiles: Vec<Option<Vec<u8>>> = if ctx.parallel && list.len() > 1 {
        list.par_iter().map(decode).collect()
    } else {
        list.iter().map(decode).collect()
    };
    let ncols = (tx1 - tx0 + 1) as usize;
    let bpp = plane.bpp;
    let first = (r.y0 - cy0) as usize;
    let rows = (r.y1 - r.y0) as usize;
    let out_rows = &mut out[first * w * channels..(first + rows) * w * channels];
    for_rows(ctx.parallel, out_rows, w * channels, |i, row| {
        let ly = r.y0 + i as i64 - py;
        let ty = ly / t;
        let in_row = (ly % t) as usize;
        for tx in tx0..=tx1 {
            let k = ((ty - ty0) as usize) * ncols + (tx - tx0) as usize;
            let Some(tile) = &tiles[k] else {
                continue;
            };
            let tw = i64::from(t as u32).min(i64::from(plane.width) - tx * t);
            let sx0 = (tx * t).max(r.x0 - px);
            let sx1 = (tx * t + tw).min(r.x1 - px);
            if sx0 >= sx1 {
                continue;
            }
            let s = ((in_row as i64 * tw + sx0 - tx * t) as usize) * bpp;
            let e = s + (sx1 - sx0) as usize * bpp;
            let d = ((sx0 + px) as usize - reg.x0) * channels;
            let de = d + (sx1 - sx0) as usize * channels;
            if let Some(src) = tile.get(s..e) {
                conv(src, &mut row[d..de]);
            }
        }
    });
    out
}

/// The canvas region a layer is processed over in this band.
fn region(ctx: &Ctx, it: &Item) -> Option<Region> {
    let full = Region {
        x0: 0,
        x1: ctx.width,
        r0: 0,
        r1: ctx.rows,
    };
    if it.last || !it.mode.identity_outside() {
        return Some(full);
    }
    let s = it.src?;
    let band = Rect::new(0, ctx.y0 as i64, ctx.width as i64, ctx.rows as i64)?;
    let r = s.intersect(band)?;
    Some(Region {
        x0: r.x0 as usize,
        x1: r.x1 as usize,
        r0: (r.y0 - ctx.y0 as i64) as usize,
        r1: (r.y1 - ctx.y0 as i64) as usize,
    })
}

#[allow(clippy::too_many_arguments)]
fn composite(
    ctx: &Ctx,
    acc: &mut Acc,
    it: &Item,
    space: Space,
    reg: Region,
    layer: View,
    mask: Option<View>,
    opacity: f32,
) {
    let w = ctx.width;
    let rw = reg.width();
    let mode = &it.mode;
    let rows = &mut acc.buf[reg.r0 * w * 4..reg.r1 * w * 4];
    for_rows(ctx.parallel, rows, w * 4, |i, row| {
        let run = blend::Run {
            mode,
            space,
            opacity,
            x: reg.x0 as i32,
            y: (ctx.y0 + reg.r0 + i) as i32,
            last: it.last,
        };
        blend::apply(
            &run,
            &mut row[reg.x0 * 4..reg.x1 * 4],
            layer.row(i, rw),
            mask.as_ref().map(|m| m.row(i, rw)),
        );
    });
    acc.space = space;
}

/// Channels are stored linear, except in 8-bit non-linear images, where GIMP keeps them
/// perceptual (`Y' u8`; layer masks stay linear).
fn channel_linear(doc: &Doc) -> bool {
    doc.precision.linear || doc.precision.component != Component::U8
}

fn load_mask(ctx: &Ctx, node: &Node, reg: Region) -> Option<Vec<f32>> {
    let m = node.mask.as_ref()?;
    let loader = Loader {
        kind: PlaneKind::Mask,
        component: ctx.doc.precision.component,
        big_endian: ctx.doc.big_endian,
        data_linear: true,
        colormap: &[],
    };
    Some(load(ctx, m, reg, 1, |s, d| loader.mask(s, d)))
}

fn mask_view(m: &Option<Vec<f32>>, reg: Region) -> Option<View<'_>> {
    m.as_ref().map(|buf| View {
        buf,
        stride: reg.width(),
        x: 0,
        r: 0,
        channels: 1,
    })
}

/// GIMP keeps a group's (and the image's) projection in the image's precision: brings a result
/// to that storage (its space, clamped and rounded for integer precisions, rounded to half
/// floats).
fn store_like_gimp(ctx: &Ctx, acc: &mut Acc) {
    let p = ctx.doc.precision;
    let scale = match p.component {
        Component::U8 => 255.0,
        Component::U16 => 65535.0,
        Component::U32 => 0.0,
        Component::Half => -1.0,
        Component::Float | Component::Double => return,
    };
    let space = if p.linear && ctx.doc.base != BaseType::Indexed {
        Space::Linear
    } else {
        Space::NonLinear
    };
    convert_space(ctx.parallel, acc, space, ctx.width);
    let q = move |v: f32| -> f32 {
        if scale < 0.0 {
            f16_to_f32(f32_to_f16(v))
        } else if scale == 0.0 {
            v.clamp(0.0, 1.0)
        } else {
            (v.clamp(0.0, 1.0) * scale).round() / scale
        }
    };
    for_rows(ctx.parallel, &mut acc.buf, ctx.width * 4, |_, row| {
        for v in row.iter_mut() {
            *v = q(*v);
        }
    });
}

/// How deeply drawn groups nest in `items` (at most the parser's nesting cap).
fn group_levels(doc: &Doc, items: &[Item]) -> usize {
    items
        .iter()
        .filter(|it| !it.skip && doc.nodes[it.node].group)
        .map(|it| 1 + group_levels(doc, &it.kids))
        .max()
        .unwrap_or(0)
}

/// Renders a stack (bottom first) onto `acc`. Returns false when cancelled.
fn stack(ctx: &Ctx, items: &[Item], acc: &mut Acc) -> bool {
    let doc = ctx.doc;
    // `acc` was cleared and nothing drew since: clearing it again for each of a run of empty
    // layers was layers × band.
    let mut cleared = false;
    for it in items {
        if ctx.opts.cancelled() {
            return false;
        }
        let n = &doc.nodes[it.node];
        if !it.skip {
            let Some(reg) = region(ctx, it) else {
                if it.empty_after && !cleared {
                    acc.buf.fill(0.0);
                    cleared = true;
                }
                continue;
            };
            cleared = false;
            if n.group {
                let mut inner = if it.pass {
                    acc.clone()
                } else {
                    Acc {
                        buf: vec![0.0; acc.buf.len()],
                        space: Space::NonLinear,
                    }
                };
                if !stack(ctx, &it.kids, &mut inner) {
                    return false;
                }
                if !it.pass {
                    store_like_gimp(ctx, &mut inner);
                }
                if it.pass && n.opacity >= 1.0 && n.mask.is_none() {
                    *acc = inner;
                } else {
                    let space =
                        it.mode
                            .space
                            .unwrap_or(if it.last { inner.space } else { acc.space });
                    convert_space(ctx.parallel, &mut inner, space, ctx.width);
                    if !it.last {
                        convert_space(ctx.parallel, acc, space, ctx.width);
                    }
                    let mask = load_mask(ctx, n, reg);
                    let layer = View {
                        buf: &inner.buf,
                        stride: ctx.width,
                        x: reg.x0,
                        r: reg.r0,
                        channels: 4,
                    };
                    composite(
                        ctx,
                        acc,
                        it,
                        space,
                        reg,
                        layer,
                        mask_view(&mask, reg),
                        n.opacity,
                    );
                }
            } else if let Some(plane) = &n.plane {
                let data_linear = doc.precision.linear && doc.base != BaseType::Indexed;
                let space = it.mode.space.unwrap_or(if it.last {
                    if data_linear {
                        Space::Linear
                    } else {
                        Space::NonLinear
                    }
                } else {
                    acc.space
                });
                if !it.last {
                    convert_space(ctx.parallel, acc, space, ctx.width);
                }
                let loader = Loader {
                    kind: plane.kind,
                    component: doc.precision.component,
                    big_endian: doc.big_endian,
                    data_linear,
                    colormap: &doc.colormap,
                };
                let want_linear = space == Space::Linear;
                let buf = match ctx.filtered.get(&it.node) {
                    Some(f) => from_filtered(ctx, f, reg, want_linear),
                    None => load(ctx, plane, reg, 4, |s, d| loader.rgba(s, want_linear, d)),
                };
                let mask = load_mask(ctx, n, reg);
                let layer = View {
                    buf: &buf,
                    stride: reg.width(),
                    x: 0,
                    r: 0,
                    channels: 4,
                };
                composite(
                    ctx,
                    acc,
                    it,
                    space,
                    reg,
                    layer,
                    mask_view(&mask, reg),
                    n.opacity,
                );
            }
        }
        if it.empty_after && !cleared {
            acc.buf.fill(0.0);
            cleared = true;
        }
    }
    true
}

/// Draws the visible channels over the band (bottom one first), each its colour at the
/// channel's opacity times its value (or one minus it, for "show masked"), normal over in
/// linear light.
fn overlays(ctx: &Ctx, acc: &mut Acc) {
    if ctx.doc.overlays.is_empty() {
        return;
    }
    convert_space(ctx.parallel, acc, Space::Linear, ctx.width);
    let reg = Region {
        x0: 0,
        x1: ctx.width,
        r0: 0,
        r1: ctx.rows,
    };
    let loader = Loader {
        kind: PlaneKind::Mask,
        component: ctx.doc.precision.component,
        big_endian: ctx.doc.big_endian,
        data_linear: channel_linear(ctx.doc),
        colormap: &[],
    };
    for o in ctx.doc.overlays.iter().rev() {
        let m = load(ctx, &o.plane, reg, 1, |s, d| loader.mask(s, d));
        let col = o.color.map(srgb_to_linear);
        for_rows(ctx.parallel, &mut acc.buf, ctx.width * 4, |i, row| {
            let mrow = &m[i * ctx.width..(i + 1) * ctx.width];
            for (p, &v) in row.chunks_exact_mut(4).zip(mrow) {
                let v = v.clamp(0.0, 1.0);
                let ca = o.opacity * if o.inverted { 1.0 - v } else { v };
                if ca <= 0.0 {
                    continue;
                }
                let a = p[3].clamp(0.0, 1.0);
                let out_a = ca + a * (1.0 - ca);
                let k = a * (1.0 - ca);
                for c in 0..3 {
                    p[c] = (col[c] * ca + p[c] * k) / out_a;
                }
                p[3] = out_a;
            }
        });
    }
}

/// Rows per band: up to 256, fewer for very wide canvases (about 64 MB of floats per band).
fn band_rows(width: usize) -> usize {
    (4_194_304 / width.max(1)).clamp(16, 256)
}

pub(crate) fn render(x: &Xcf, data: &[u8], opts: &Options) -> Result<Image> {
    let doc = &x.doc;
    let mut items = build(&doc.nodes);
    extents(&mut items, doc, None);
    let fmt = output_format(&x.info, x.icc.is_some(), x.info.has_alpha);
    bands(x, data, opts, &items, fmt, true, doc.broken)
}

/// The leaf layers with pixels, top of the stack first (indices into [`Xcf::layers`]).
pub(crate) fn pixel_layers(doc: &Doc) -> Vec<usize> {
    doc.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            !n.group
                && n.plane
                    .as_ref()
                    .is_some_and(|p| p.width > 0 && p.height > 0)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Layer `index` alone on the canvas at its offsets: its pixels, transparency, layer mask and
/// opacity, placed as the bottom of a stack (no blend mode, no visible channels drawn over).
/// Hidden layers render too.
pub(crate) fn render_layer(x: &Xcf, data: &[u8], index: usize, opts: &Options) -> Result<Image> {
    let doc = &x.doc;
    let n = doc
        .nodes
        .get(index)
        .filter(|n| !n.group)
        .ok_or_else(|| Error::Malformed("no such layer".into()))?;
    let canvas = Rect::new(0, 0, i64::from(doc.width), i64::from(doc.height));
    let src = n
        .plane
        .as_ref()
        .and_then(Rect::of)
        .and_then(|r| canvas.and_then(|c| r.intersect(c)));
    // Normal, in whatever space the layer's data is in (the bottom of a stack is placed as is).
    let mut mode = blend::resolve(blend::NORMAL, 0, 0, 0);
    mode.space = None;
    let items = [Item {
        node: index,
        mode,
        pass: false,
        kids: Vec::new(),
        last: true,
        src,
        skip: src.is_none() || n.opacity <= 0.0,
        empty_after: false,
    }];
    let fmt = output_format(&x.info, x.icc.is_some(), true);
    bands(x, data, opts, &items, fmt, false, n.broken)
}

/// Runs `items` (a stack, bottom first) over the canvas band by band into `fmt`, drawing the
/// visible channels over each band when `overlays`.
fn bands(
    x: &Xcf,
    data: &[u8],
    opts: &Options,
    items: &[Item],
    fmt: PixelFormat,
    with_overlays: bool,
    broken: bool,
) -> Result<Image> {
    let doc = &x.doc;
    let info = &x.info;
    let lin_out = output_linear(info, fmt, x.icc.is_some());
    let (w, h) = (doc.width as usize, doc.height as usize);
    let bpp = fmt.bytes_per_pixel();
    let size = w
        .checked_mul(h)
        .and_then(|p| p.checked_mul(bpp))
        .ok_or(Error::TooLarge)?;
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| Error::TooLarge)?;
    out.resize(size, 0);

    let truncated = AtomicBool::new(broken);
    let sb = band_rows(w);
    // Each group nesting level holds a band accumulator of its own while its children draw
    // (64 levels of 4 floats a pixel): reserved up front.
    let levels = group_levels(doc, items) as u64 + 1;
    let _accs = specular_core::budget::reserve_n(levels, (w * sb.min(h.max(1)) * 16) as u64)?;
    let filtered = filter_layers(x, data, opts, items, &truncated)?;
    let mut acc = Acc {
        buf: Vec::new(),
        space: Space::NonLinear,
    };
    for (bi, chunk) in out.chunks_mut(w * bpp * sb).enumerate() {
        if opts.cancelled() {
            return Err(Error::Cancelled);
        }
        let rows = chunk.len() / (w * bpp);
        acc.buf.clear();
        acc.buf.resize(w * rows * 4, 0.0);
        acc.space = Space::NonLinear;
        let ctx = Ctx {
            data,
            doc,
            parallel: opts.parallel,
            width: w,
            y0: bi * sb,
            rows,
            truncated: &truncated,
            opts,
            filtered: &filtered,
        };
        if !stack(&ctx, items, &mut acc) {
            return Err(Error::Cancelled);
        }
        if with_overlays {
            overlays(&ctx, &mut acc);
        }
        // The image's projection is in its precision too (this matters for 8-bit linear).
        store_like_gimp(&ctx, &mut acc);
        write_out(opts.parallel, &acc, fmt, lin_out, chunk, w);
    }
    Ok(Image {
        width: doc.width,
        height: doc.height,
        format: fmt,
        data: out,
        truncated: truncated.load(Ordering::Relaxed),
    })
}

/// The layers among `items` (and their groups) that have layer effects this crate draws, each
/// loaded over the whole canvas and run through its effects.
fn filter_layers(
    x: &Xcf,
    data: &[u8],
    opts: &Options,
    items: &[Item],
    truncated: &AtomicBool,
) -> Result<HashMap<usize, Vec<f32>>> {
    fn walk(items: &[Item], doc: &Doc, out: &mut Vec<usize>) {
        for it in items {
            let n = &doc.nodes[it.node];
            if !it.skip
                && !n.group
                && n.plane.is_some()
                && n.filters.iter().any(|f| f.visible && f.drawn())
            {
                out.push(it.node);
            }
            walk(&it.kids, doc, out);
        }
    }
    let doc = &x.doc;
    let mut list = Vec::new();
    walk(items, doc, &mut list);
    let mut map = HashMap::new();
    if list.is_empty() {
        return Ok(map);
    }
    let (w, h) = (doc.width as usize, doc.height as usize);
    let _hold = specular_core::budget::reserve_n(list.len() as u64, (w * h * 16) as u64)?;
    let empty = HashMap::new();
    let ctx = Ctx {
        data,
        doc,
        parallel: opts.parallel,
        width: w,
        y0: 0,
        rows: h,
        truncated,
        opts,
        filtered: &empty,
    };
    let reg = Region {
        x0: 0,
        x1: w,
        r0: 0,
        r1: h,
    };
    for i in list {
        let n = &doc.nodes[i];
        let Some(plane) = &n.plane else { continue };
        let loader = Loader {
            kind: plane.kind,
            component: doc.precision.component,
            big_endian: doc.big_endian,
            data_linear: doc.precision.linear && doc.base != BaseType::Indexed,
            colormap: &doc.colormap,
        };
        let mut buf = load(&ctx, plane, reg, 4, |s, d| loader.rgba(s, false, d));
        crate::filters::apply(
            &n.filters,
            &mut crate::filters::Pixels {
                buf: &mut buf,
                w,
                h,
            },
        );
        map.insert(i, buf);
    }
    Ok(map)
}

/// The part `reg` of a filtered layer (canvas-sized, coded) for this band, in linear light
/// when `linear`.
fn from_filtered(ctx: &Ctx, f: &[f32], reg: Region, linear: bool) -> Vec<f32> {
    let rw = reg.width();
    let mut out = Vec::with_capacity(rw * (reg.r1 - reg.r0) * 4);
    for r in reg.r0..reg.r1 {
        let y = ctx.y0 + r;
        let s = (y * ctx.width + reg.x0) * 4;
        out.extend_from_slice(&f[s..s + rw * 4]);
    }
    if linear {
        for p in out.chunks_exact_mut(4) {
            for v in &mut p[..3] {
                *v = srgb_to_linear(*v);
            }
        }
    }
    out
}

#[inline]
fn q8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[inline]
fn q16(v: f32) -> [u8; 2] {
    ((v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16).to_ne_bytes()
}

fn write_out(par: bool, acc: &Acc, fmt: PixelFormat, lin_out: bool, out: &mut [u8], w: usize) {
    let bpp = fmt.bytes_per_pixel();
    let conv: fn(f32) -> f32 = match (acc.space, lin_out) {
        (Space::Linear, false) => linear_to_srgb,
        (Space::NonLinear, true) => srgb_to_linear,
        _ => |v| v,
    };
    let body = |(o, a): (&mut [u8], &[f32])| {
        for (d, p) in o.chunks_exact_mut(bpp).zip(a.chunks_exact(4)) {
            let alpha = if p[3].is_nan() {
                0.0
            } else {
                p[3].clamp(0.0, 1.0)
            };
            let c = if alpha == 0.0 {
                [0.0; 3]
            } else {
                [conv(p[0]), conv(p[1]), conv(p[2])].map(|v| if v.is_nan() { 0.0 } else { v })
            };
            match fmt {
                PixelFormat::Gray8 => d[0] = q8(c[0]),
                PixelFormat::Gray16 => d.copy_from_slice(&q16(c[0])),
                PixelFormat::GrayAlpha16 => {
                    d[..2].copy_from_slice(&q16(c[0]));
                    d[2..].copy_from_slice(&q16(alpha));
                }
                PixelFormat::Rgba8 => {
                    d.copy_from_slice(&[q8(c[0]), q8(c[1]), q8(c[2]), q8(alpha)]);
                }
                PixelFormat::Rgba16 => {
                    for k in 0..3 {
                        d[2 * k..2 * k + 2].copy_from_slice(&q16(c[k]));
                    }
                    d[6..].copy_from_slice(&q16(alpha));
                }
                PixelFormat::Rgba16F => {
                    for k in 0..3 {
                        d[2 * k..2 * k + 2].copy_from_slice(&f32_to_f16(c[k]).to_ne_bytes());
                    }
                    d[6..].copy_from_slice(&f32_to_f16(alpha).to_ne_bytes());
                }
            }
        }
    };
    if par {
        out.par_chunks_mut(w * bpp)
            .zip(acc.buf.par_chunks(w * 4))
            .for_each(body);
    } else {
        out.chunks_mut(w * bpp)
            .zip(acc.buf.chunks(w * 4))
            .for_each(body);
    }
}

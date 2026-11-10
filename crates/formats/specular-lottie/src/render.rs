//! One frame of an animation as a page: layers bottom to top, each through its parents'
//! transforms; shape groups paint the paths above them in their lists (a fill or stroke
//! applies to every path before it in its group and the groups before it), the first items
//! on top.

use specular_drawpage::kurbo::{Ellipse, PathEl, RoundedRect};
use specular_drawpage::{
    Affine, BezPath, Cap, Join, Op, Page, Paint, Point, Rect, Shape, Stop, Stroke,
};

use crate::prop::Prop;
use crate::{Animation, Content, Gradient, Item, Layer, MAX_DEPTH, Transform, colour01};

/// A layer transform's matrix and opacity at `t`.
fn matrix(tr: &Transform, t: f64) -> (Affine, f64) {
    let a = tr.anchor.at(t);
    let p = match &tr.split {
        Some((x, y)) => vec![x.num(t), y.num(t)],
        None => tr.position.at(t),
    };
    let s = tr.scale.at(t);
    let g = |v: &[f64], i: usize, d: f64| v.get(i).copied().unwrap_or(d);
    let rot = tr.rotation.num(t).to_radians();
    let sk = tr.skew.num(t).to_radians();
    let skew = if sk.abs() > 1e-9 {
        let sa = tr.skew_axis.num(t).to_radians();
        Affine::rotate(sa)
            * Affine::new([1.0, 0.0, (-sk).tan(), 1.0, 0.0, 0.0])
            * Affine::rotate(-sa)
    } else {
        Affine::IDENTITY
    };
    let m = Affine::translate((g(&p, 0, 0.0), g(&p, 1, 0.0)))
        * Affine::rotate(rot)
        * skew
        * Affine::scale_non_uniform(g(&s, 0, 100.0) / 100.0, g(&s, 1, 100.0) / 100.0)
        * Affine::translate((-g(&a, 0, 0.0), -g(&a, 1, 0.0)));
    (m, (tr.opacity.num(t) / 100.0).clamp(0.0, 1.0))
}

/// The frame `f` (in the animation's frame numbers) as a page.
pub fn frame_page(anim: &Animation, f: f64) -> Page {
    let mut page = Page::new("frame", (anim.width, anim.height));
    let mut ops = Vec::new();
    draw_layers(anim, &anim.layers, f, &mut ops, 0);
    page.ops = ops;
    page
}

/// A layer's matrix through its parents.
fn layer_matrix(layers: &[Layer], l: &Layer, t: f64) -> Affine {
    // Layer properties are keyed in the composition's time; the layer's start time only
    // shifts what a precomposition shows.
    let mut m = matrix(&l.transform, t).0;
    let mut parent = l.parent;
    let mut hops = 0;
    while let Some(pi) = parent {
        let Some(p) = layers.iter().find(|x| x.index == Some(pi)) else {
            break;
        };
        m = matrix(&p.transform, t).0 * m;
        parent = p.parent;
        hops += 1;
        if hops > 64 {
            break;
        }
    }
    m
}

fn draw_layers(anim: &Animation, layers: &[Layer], t: f64, ops: &mut Vec<Op>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for i in (0..layers.len()).rev() {
        let l = &layers[i];
        if l.hidden || l.is_matte || t < l.in_point || t >= l.out_point {
            continue;
        }
        let local = (t - l.start) / l.stretch;
        let m = layer_matrix(layers, l, t);
        let opacity = matrix(&l.transform, t).1;
        if opacity <= 0.0 {
            continue;
        }
        let mut body = Vec::new();
        match &l.content {
            Content::Shapes(items) => {
                let mut seq = Vec::new();
                walk(items, m, 1.0, t, &mut seq, &[], &[], 0);
                for s in seq {
                    s.emit(&mut body);
                }
            }
            Content::Solid { colour, w, h } => body.push(Op::Fill {
                path: m * Rect::new(0.0, 0.0, *w, *h).to_path(0.1),
                even_odd: false,
                paint: Paint::Solid(*colour),
            }),
            Content::Image(id) => {
                if let Some(img) = anim.images.get(id) {
                    body.push(Op::Image {
                        image: img.clone(),
                        transform: m,
                        opacity: 1.0,
                    });
                }
            }
            Content::Precomp { id, w, h, remap } => {
                if let Some(sub) = anim.precomps.get(id) {
                    let pt = match remap {
                        Some(r) => r.num(t) * anim.frame_rate,
                        None => local,
                    };
                    let mut inner = Vec::new();
                    draw_layers(anim, sub, pt, &mut inner, depth + 1);
                    let mut nested = Vec::new();
                    for o in inner {
                        nested.push(transform_op(o, m));
                    }
                    if *w > 0.0 && *h > 0.0 {
                        body.push(Op::PushClip {
                            path: m * Rect::new(0.0, 0.0, *w, *h).to_path(0.1),
                            even_odd: false,
                        });
                        body.extend(nested);
                        body.push(Op::Pop);
                    } else {
                        body.extend(nested);
                    }
                }
            }
            Content::Null | Content::Unsupported => {}
        }
        if body.is_empty() {
            continue;
        }
        let mut pushes = 0;
        // Track matte: the layer above, when it is one.
        if l.matte > 0
            && i > 0
            && layers[i - 1].is_matte
            && let Some(clip) = matte_clip(anim, layers, i - 1, t, l.matte)
        {
            ops.push(clip);
            pushes += 1;
        }
        for c in mask_clips(l, m, t, anim) {
            ops.push(c);
            pushes += 1;
        }
        if opacity < 1.0 {
            ops.push(Op::PushOpacity(opacity as f32));
            pushes += 1;
        }
        ops.extend(body);
        for _ in 0..pushes {
            ops.push(Op::Pop);
        }
    }
}

/// Moves a precomposition's op into its layer's space.
fn transform_op(o: Op, m: Affine) -> Op {
    match o {
        Op::Fill {
            path,
            even_odd,
            paint,
        } => Op::Fill {
            path: m * path,
            even_odd,
            paint: transform_paint(paint, m),
        },
        Op::Stroke {
            path,
            mut stroke,
            paint,
        } => {
            let k = scale_of(m);
            stroke.width *= k;
            stroke.dash_pattern.iter_mut().for_each(|d| *d *= k);
            stroke.dash_offset *= k;
            Op::Stroke {
                path: m * path,
                stroke,
                paint: transform_paint(paint, m),
            }
        }
        Op::Image {
            image,
            transform,
            opacity,
        } => Op::Image {
            image,
            transform: m * transform,
            opacity,
        },
        Op::PushClip { path, even_odd } => Op::PushClip {
            path: m * path,
            even_odd,
        },
        o => o,
    }
}

fn transform_paint(p: Paint, m: Affine) -> Paint {
    match p {
        Paint::Linear { start, end, stops } => Paint::Linear {
            start: m * start,
            end: m * end,
            stops,
        },
        Paint::Radial { transform, stops } => Paint::Radial {
            transform: m * transform,
            stops,
        },
        p => p,
    }
}

/// The scale a matrix gives lengths (for stroke widths).
fn scale_of(m: Affine) -> f64 {
    m.determinant().abs().sqrt()
}

fn comp_rect(anim: &Animation) -> BezPath {
    Rect::new(
        -anim.width,
        -anim.height,
        2.0 * anim.width,
        2.0 * anim.height,
    )
    .to_path(0.1)
}

/// The clip of a track matte: the matte layer's filled shapes (or its box).
fn matte_clip(anim: &Animation, layers: &[Layer], mi: usize, t: f64, kind: u8) -> Option<Op> {
    let ml = &layers[mi];
    if t < ml.in_point || t >= ml.out_point {
        return None;
    }
    let mut path = BezPath::new();
    fill_outline(anim, layers, mi, t, Affine::IDENTITY, 0, &mut path)?;
    let inverted = matches!(kind, 2 | 4);
    if inverted {
        let mut p = comp_rect(anim);
        p.extend(path.elements().iter().copied());
        path = p;
    }
    Some(Op::PushClip {
        path,
        even_odd: inverted,
    })
}

/// The filled outline of layer `li` at `t` (a precomposition's: its layers'), under `outer`.
fn fill_outline(
    anim: &Animation,
    layers: &[Layer],
    li: usize,
    t: f64,
    outer: Affine,
    depth: usize,
    out: &mut BezPath,
) -> Option<()> {
    let ml = &layers[li];
    if depth > MAX_DEPTH || t < ml.in_point || t >= ml.out_point {
        return Some(());
    }
    let local = (t - ml.start) / ml.stretch;
    let m = outer * layer_matrix(layers, ml, t);
    let path = match &ml.content {
        Content::Shapes(items) => {
            let mut seq = Vec::new();
            walk(items, m, 1.0, t, &mut seq, &[], &[], 0);
            let mut all = BezPath::new();
            for s in seq {
                if let Slot::Paint {
                    paths, fill: true, ..
                } = s
                {
                    for p in paths {
                        all.extend(p.elements().iter().copied());
                    }
                }
            }
            all
        }
        Content::Solid { w, h, .. } => m * Rect::new(0.0, 0.0, *w, *h).to_path(0.1),
        Content::Precomp { id, remap, .. } => {
            let sub = anim.precomps.get(id)?;
            let pt = match remap {
                Some(r) => r.num(t) * anim.frame_rate,
                None => local,
            };
            let mut all = BezPath::new();
            for k in 0..sub.len() {
                if !sub[k].hidden && !sub[k].is_matte {
                    fill_outline(anim, sub, k, pt, m, depth + 1, &mut all);
                }
            }
            all
        }
        Content::Image(id) => {
            let img = anim.images.get(id)?;
            m * Rect::new(0.0, 0.0, f64::from(img.width), f64::from(img.height)).to_path(0.1)
        }
        _ => return Some(()),
    };
    out.extend(path.elements().iter().copied());
    Some(())
}

/// The clips of a layer's masks: additive masks together, subtracted ones cut out,
/// intersecting ones in turn.
fn mask_clips(l: &Layer, m: Affine, t: f64, anim: &Animation) -> Vec<Op> {
    let mut out = Vec::new();
    let mut add = BezPath::new();
    let mut any_add = false;
    for k in &l.masks {
        if k.mode == 'n' || k.opacity.num(t) <= 0.0 {
            continue;
        }
        let p = m * k.path.at(t).path();
        // Lighten and difference masks drawn as add, darken as intersect.
        let mode = match k.mode {
            'l' | 'f' => 'a',
            'd' => 'i',
            m => m,
        };
        match (mode, k.inverted) {
            ('a', false) => {
                any_add = true;
                add.extend(p.elements().iter().copied());
            }
            ('s', false) | ('a', true) => {
                let mut c = comp_rect(anim);
                c.extend(p.elements().iter().copied());
                out.push(Op::PushClip {
                    path: c,
                    even_odd: true,
                });
            }
            ('i', false) | ('s', true) => out.push(Op::PushClip {
                path: p,
                even_odd: false,
            }),
            _ => {}
        }
    }
    if any_add {
        out.insert(
            0,
            Op::PushClip {
                path: add,
                even_odd: false,
            },
        );
    }
    out
}

/// A slot in a group's drawing order: a paint and the paths it collects, or ops drawn by a
/// nested group at its place.
enum Slot {
    Paint {
        paint: Paint,
        fill: bool,
        even_odd: bool,
        stroke: Option<Stroke>,
        paths: Vec<BezPath>,
    },
}

impl Slot {
    fn emit(self, ops: &mut Vec<Op>) {
        match self {
            Slot::Paint {
                paint,
                fill,
                even_odd,
                stroke,
                paths,
            } => {
                if paths.is_empty() {
                    return;
                }
                let mut path = BezPath::new();
                for p in paths {
                    path.extend(p.elements().iter().copied());
                }
                if fill {
                    ops.push(Op::Fill {
                        path,
                        even_odd,
                        paint,
                    });
                } else if let Some(stroke) = stroke {
                    ops.push(Op::Stroke {
                        path,
                        stroke,
                        paint,
                    });
                }
            }
        }
    }
}

fn gradient_paint(g: &Gradient, m: Affine, t: f64, opacity: f64) -> Paint {
    let v = g.stops.at(t);
    let n = g.count.max(1);
    let mut stops: Vec<Stop> = (0..n)
        .filter_map(|k| {
            let s = v.get(4 * k..4 * k + 4)?;
            Some(Stop {
                offset: s[0] as f32,
                color: colour01(&[s[1], s[2], s[3], 1.0]),
            })
        })
        .collect();
    // Opacity stops after the colour stops: each colour stop takes the alpha at its offset.
    let alphas: Vec<(f64, f64)> = v[(4 * n).min(v.len())..]
        .chunks_exact(2)
        .map(|c| (c[0], c[1]))
        .collect();
    for s in &mut stops {
        let a = if alphas.is_empty() {
            1.0
        } else {
            let o = f64::from(s.offset);
            match alphas.iter().position(|a| a.0 >= o) {
                Some(0) => alphas[0].1,
                None => alphas[alphas.len() - 1].1,
                Some(i) => {
                    let (a0, a1) = (alphas[i - 1], alphas[i]);
                    let f = if a1.0 > a0.0 {
                        (o - a0.0) / (a1.0 - a0.0)
                    } else {
                        0.0
                    };
                    a0.1 + (a1.1 - a0.1) * f
                }
            }
        };
        s.color[3] = (a * opacity * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    if stops.is_empty() {
        stops.push(Stop {
            offset: 0.0,
            color: [0, 0, 0, 255],
        });
    }
    let (s, e) = (g.start.at(t), g.end.at(t));
    let pt = |v: &[f64]| {
        Point::new(
            v.first().copied().unwrap_or(0.0),
            v.get(1).copied().unwrap_or(0.0),
        )
    };
    let (s, e) = (pt(&s), pt(&e));
    if g.radial {
        let r = s.distance(e).max(1e-3);
        Paint::Radial {
            transform: m * Affine::translate(s.to_vec2()) * Affine::scale(r),
            stops,
        }
    } else {
        Paint::Linear {
            start: m * s,
            end: m * e,
            stops,
        }
    }
}

fn stroke_of(width: f64, cap: u8, join: u8, dashes: &[(char, Prop)], t: f64, k: f64) -> Stroke {
    let mut s = Stroke::new((width * k).max(0.01))
        .with_caps(match cap {
            1 => Cap::Butt,
            3 => Cap::Square,
            _ => Cap::Round,
        })
        .with_join(match join {
            1 => Join::Miter,
            3 => Join::Bevel,
            _ => Join::Round,
        });
    let pattern: Vec<f64> = dashes
        .iter()
        .filter(|(n, _)| *n != 'o')
        .map(|(_, v)| v.num(t).max(0.0) * k)
        .collect();
    if !pattern.is_empty() && pattern.iter().any(|v| *v > 0.0) {
        let offset = dashes
            .iter()
            .find(|(n, _)| *n == 'o')
            .map_or(0.0, |(_, v)| v.num(t) * k);
        s = s.with_dashes(offset, pattern);
    }
    s
}

/// A trim (start, end, offset as fractions) of one path, flattened.
fn trim(path: &BezPath, s: f64, e: f64, o: f64) -> BezPath {
    let (mut a, mut b) = (s + o, e + o);
    if (b - a).abs() >= 1.0 - 1e-9 {
        return path.clone();
    }
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    // Polylines of each subpath.
    let mut subs: Vec<Vec<Point>> = Vec::new();
    specular_drawpage::kurbo::flatten(path.iter(), 0.25, |el| match el {
        PathEl::MoveTo(p) => subs.push(vec![p]),
        PathEl::LineTo(p) => {
            if let Some(s) = subs.last_mut() {
                s.push(p);
            }
        }
        PathEl::ClosePath => {
            if let Some(s) = subs.last_mut()
                && let Some(&f) = s.first()
            {
                s.push(f);
            }
        }
        _ => {}
    });
    let mut out = BezPath::new();
    for pts in subs {
        let lens: Vec<f64> = pts.windows(2).map(|w| w[0].distance(w[1])).collect();
        let total: f64 = lens.iter().sum();
        if total <= 0.0 {
            continue;
        }
        // The pieces [a, b] wrapped into [0, 1].
        let fa = a.rem_euclid(1.0);
        let len = b - a;
        let ranges = if fa + len <= 1.0 {
            vec![(fa, fa + len)]
        } else {
            vec![(fa, 1.0), (0.0, fa + len - 1.0)]
        };
        for (r0, r1) in ranges {
            let (d0, d1) = (r0 * total, r1 * total);
            let mut acc = 0.0;
            let mut started = false;
            for (i, l) in lens.iter().enumerate() {
                let (p, q) = (pts[i], pts[i + 1]);
                let (s0, s1) = (acc, acc + l);
                acc = s1;
                if s1 < d0 || s0 > d1 || *l <= 0.0 {
                    continue;
                }
                let at = |d: f64| p.lerp(q, ((d - s0) / l).clamp(0.0, 1.0));
                if !started {
                    out.move_to(at(d0.max(s0)));
                    started = true;
                }
                out.line_to(at(d1.min(s1)));
            }
        }
    }
    out
}

/// Walks a group's items, last to first (bottom to top): paints collect the paths before
/// them; nested groups add their paths to the paints around them too.
#[allow(clippy::too_many_arguments)]
fn walk(
    items: &[Item],
    m: Affine,
    opacity: f64,
    t: f64,
    seq: &mut Vec<Slot>,
    active: &[(usize, Affine)],
    trims: &[(f64, f64, f64)],
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut active: Vec<(usize, Affine)> = active.to_vec();
    let mut trims: Vec<(f64, f64, f64)> = trims.to_vec();
    let k = scale_of(m);
    for it in items.iter().rev() {
        let pt = |p: &Prop| {
            let v = p.at(t);
            Point::new(
                v.first().copied().unwrap_or(0.0),
                v.get(1).copied().unwrap_or(0.0),
            )
        };
        let path: Option<BezPath> = match it {
            Item::Group { items, transform } => {
                let (gm, go) = transform
                    .as_ref()
                    .map_or((Affine::IDENTITY, 1.0), |tr| matrix(tr, t));
                if go > 0.0 {
                    walk(
                        items,
                        m * gm,
                        opacity * go,
                        t,
                        seq,
                        &active,
                        &trims,
                        depth + 1,
                    );
                }
                None
            }
            Item::Rect {
                position,
                size,
                round,
            } => {
                let (c, s) = (pt(position), pt(size));
                let r = Rect::new(
                    c.x - s.x / 2.0,
                    c.y - s.y / 2.0,
                    c.x + s.x / 2.0,
                    c.y + s.y / 2.0,
                );
                let rr = round
                    .num(t)
                    .min(s.x.abs() / 2.0)
                    .min(s.y.abs() / 2.0)
                    .max(0.0);
                Some(if rr > 0.0 {
                    RoundedRect::from_rect(r, rr).to_path(0.1)
                } else {
                    r.to_path(0.1)
                })
            }
            Item::Ellipse { position, size } => {
                let (c, s) = (pt(position), pt(size));
                Some(Ellipse::new(c, (s.x.abs() / 2.0, s.y.abs() / 2.0), 0.0).to_path(0.1))
            }
            Item::Star {
                star,
                points,
                position,
                rotation,
                inner,
                outer,
            } => {
                let n = points.num(t).round().clamp(3.0, 1000.0) as usize;
                let c = pt(position);
                let rot = (rotation.num(t) - 90.0).to_radians();
                let (ri, ro) = (inner.num(t), outer.num(t));
                let mut p = BezPath::new();
                let steps = if *star { 2 * n } else { n };
                for i in 0..steps {
                    let a = rot + std::f64::consts::TAU * i as f64 / steps as f64;
                    let r = if *star && i % 2 == 1 { ri } else { ro };
                    let q = Point::new(c.x + r * a.cos(), c.y + r * a.sin());
                    if i == 0 {
                        p.move_to(q);
                    } else {
                        p.line_to(q);
                    }
                }
                p.close_path();
                Some(p)
            }
            Item::Path(pp) => Some(pp.at(t).path()),
            Item::Fill {
                colour,
                opacity: o,
                even_odd,
            } => {
                let mut c = colour01(&colour.at(t));
                c[3] = (f64::from(c[3]) * o.num(t) / 100.0 * opacity)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                seq.push(Slot::Paint {
                    paint: Paint::Solid(c),
                    fill: true,
                    even_odd: *even_odd,
                    stroke: None,
                    paths: Vec::new(),
                });
                active.push((seq.len() - 1, Affine::IDENTITY));
                None
            }
            Item::GradientFill {
                g,
                opacity: o,
                even_odd,
            } => {
                seq.push(Slot::Paint {
                    paint: gradient_paint(g, m, t, o.num(t) / 100.0 * opacity),
                    fill: true,
                    even_odd: *even_odd,
                    stroke: None,
                    paths: Vec::new(),
                });
                active.push((seq.len() - 1, Affine::IDENTITY));
                None
            }
            Item::Stroke {
                colour,
                opacity: o,
                width,
                cap,
                join,
                dashes,
            } => {
                let mut c = colour01(&colour.at(t));
                c[3] = (f64::from(c[3]) * o.num(t) / 100.0 * opacity)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let w = width.num(t);
                if w > 0.0 {
                    seq.push(Slot::Paint {
                        paint: Paint::Solid(c),
                        fill: false,
                        even_odd: false,
                        stroke: Some(stroke_of(w, *cap, *join, dashes, t, k)),
                        paths: Vec::new(),
                    });
                    active.push((seq.len() - 1, Affine::IDENTITY));
                }
                None
            }
            Item::GradientStroke {
                g,
                opacity: o,
                width,
                cap,
                join,
            } => {
                let w = width.num(t);
                if w > 0.0 {
                    seq.push(Slot::Paint {
                        paint: gradient_paint(g, m, t, o.num(t) / 100.0 * opacity),
                        fill: false,
                        even_odd: false,
                        stroke: Some(stroke_of(w, *cap, *join, &[], t, k)),
                        paths: Vec::new(),
                    });
                    active.push((seq.len() - 1, Affine::IDENTITY));
                }
                None
            }
            Item::Trim { start, end, offset } => {
                trims.push((
                    start.num(t) / 100.0,
                    end.num(t) / 100.0,
                    offset.num(t) / 360.0,
                ));
                None
            }
            Item::Other => None,
        };
        if let Some(mut p) = path {
            for &(s, e, o) in &trims {
                p = trim(&p, s, e, o);
            }
            let p = m * p;
            for &(i, _) in &active {
                let Slot::Paint { paths, .. } = &mut seq[i];
                paths.push(p.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_halve_a_line() {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((100.0, 0.0));
        let t = trim(&p, 0.0, 0.5, 0.0);
        let b = t.bounding_box();
        assert!((b.x1 - 50.0).abs() < 1e-6, "{b:?}");
    }

    #[test]
    fn the_square_turns() {
        let a = crate::open_json(crate::tests::SQUARE.as_bytes(), None, &|_| None).unwrap();
        let box_at = |f: f64| {
            let p = frame_page(&a, f);
            match &p.ops[0] {
                Op::Fill { path, .. } => path.bounding_box(),
                o => panic!("{o:?}"),
            }
        };
        let b0 = box_at(0.0);
        assert!((b0.width() - 40.0).abs() < 1e-6);
        // At 45° the square's box is √2 wider.
        let b = box_at(15.0);
        assert!((b.width() - 40.0 * 2f64.sqrt()).abs() < 0.5, "{b:?}");
    }
}

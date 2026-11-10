//! A presentation state's graphic annotations on screen (PS3.3 C.10.5): graphics, compound
//! graphics and text of the layers that apply to the shown frame, in layer order, in their
//! Line / Fill / Text Style or else the layer's colour. PIXEL coordinates are the stored
//! image's (origin at the top-left pixel's top-left corner) and follow the view; DISPLAY
//! coordinates are fractions of the displayed area as shown.
//!
//! Text stays upright and readable whatever the state's rotation and flip: only its box
//! moves with the image. The standard leaves this open; we read Bounding Box Text
//! Horizontal Justification and Vertical Alignment as tied to the box's top-left hand
//! corner (TLHC), so when the transform puts the TLHC on the right (or at the bottom) of
//! the box on screen, LEFT and RIGHT (TOP and BOTTOM) swap.
//!
//! Compound graphics (C.10.5.1.3) are drawn from their own description
//! ([`Compound::draw`]): one DISPLAY unit is the displayed area's width, arrowheads and
//! ticks are 8 logical pixels long, infinite lines are clipped to the displayed area.

use specular_core::Orientation;
use specular_dicom::gsps::{
    Compound, CompoundKind, GraphicKind, Justify, PresentationState, Sizes, Style, Units, VAlign,
};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_render::View;
use specular_theme::Color;
use specular_theme::metrics::{FONT_BODY, RADIUS_SM};

/// The view orientation for a presentation state's rotation (quarter turns clockwise) and
/// flip: the state flips before it rotates (C.10.6.1.1), as an [`Orientation`] does.
pub(crate) fn gsps_orientation(q: u8, flip: bool) -> Orientation {
    Orientation::from_rotation_flip(q % 4, flip)
}

/// A graphic on screen (logical pixels).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mark {
    pub outline: Vec<(f32, f32)>,
    pub closed: bool,
    /// The fill inside a closed outline.
    pub fill: Option<Color>,
    /// Points, drawn as crosses.
    pub points: bool,
    pub colour: Color,
    /// Line width, logical pixels.
    pub width: f32,
    pub dashed: bool,
}

/// A text object on screen (logical pixels).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Note {
    pub lines: Vec<String>,
    /// x0, y0, x1, y1.
    pub bbox: Option<(f32, f32, f32, f32)>,
    /// As shown: after the swap for a box turned or flipped on screen.
    pub justify: Justify,
    pub valign: VAlign,
    pub anchor: Option<(f32, f32)>,
    pub anchor_line: bool,
    pub colour: Color,
}

/// A compound graphic's tick label on screen: the text centred `at`, pushed off along
/// `away` (a unit vector from the tick) so it clears the line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tag {
    pub at: (f32, f32),
    pub away: (f32, f32),
    pub text: String,
    pub colour: Color,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Scene {
    pub marks: Vec<Mark>,
    pub notes: Vec<Note>,
    pub tags: Vec<Tag>,
}

impl Scene {
    pub(crate) fn is_empty(&self) -> bool {
        self.marks.is_empty() && self.notes.is_empty() && self.tags.is_empty()
    }
}

/// Graphics in a layer without a recommended colour or grey.
const DEFAULT_INK: Color = Color::rgba(1.0, 1.0, 0.6, 1.0);
/// Line width without a Line Thickness, logical pixels.
const LINE: f32 = 1.5;
/// Fill opacity without a Fill Pattern On Opacity.
const FILL: f32 = 0.3;
/// Arrowhead and tick length, logical pixels.
const HEAD: f32 = 8.0;

fn colour([r, g, b]: [u8; 3]) -> Color {
    Color::rgba(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    )
}

/// Line / fill look of an object: its style over the layer's colour; `per_unit` is logical
/// pixels per unit of the object's coordinates (for Line Thickness).
fn look(style: &Style, layer: Color, per_unit: f32) -> (Color, Color, f32) {
    let line = style.colour.map_or(layer, colour);
    let fill = style
        .fill
        .map_or(line, colour)
        .with_alpha(style.opacity.map_or(FILL, |o| o as f32));
    let width = style
        .thickness
        .map_or(LINE, |t| (t as f32 * per_unit).clamp(0.5, 24.0));
    (line, fill, width)
}

/// Segment a→b clipped to the box (x0, y0, x1, y1) (Liang–Barsky).
fn clip(a: (f32, f32), b: (f32, f32), (x0, y0, x1, y1): (f32, f32, f32, f32)) -> Option<Seg> {
    let d = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.0, a.0 - x0),
        (d.0, x1 - a.0),
        (-d.1, a.1 - y0),
        (d.1, y1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then(|| {
        let at = |t: f32| (a.0 + d.0 * t, a.1 + d.1 * t);
        (at(t0), at(t1))
    })
}

/// The annotations of `ps` on frame `frame` of image `sop`, through `view`; `s` is the
/// display scale (physical per logical pixel).
pub(crate) fn scene(ps: &PresentationState, sop: &str, frame: u32, view: &View, s: f32) -> Scene {
    let to_screen = view.stored_from_screen().invert();
    let pixel = |p: (f64, f64)| {
        let q = to_screen.apply((p.0 as f32, p.1 as f32));
        (q.0 / s, q.1 / s)
    };
    let (w, h) = view.stored_size();
    let area = ps.area_for(sop, frame).map(|a| a.stored_rect()).unwrap_or((
        0.0,
        0.0,
        f64::from(w),
        f64::from(h),
    ));
    let corners = [
        pixel((area.0, area.1)),
        pixel((area.2, area.1)),
        pixel((area.0, area.3)),
        pixel((area.2, area.3)),
    ];
    let x0 = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
    let y0 = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
    let x1 = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
    let y1 = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);
    let map = |u: Units, p: (f64, f64)| match u {
        Units::Pixel => pixel(p),
        Units::Display => (x0 + p.0 as f32 * (x1 - x0), y0 + p.1 as f32 * (y1 - y0)),
    };
    // Logical pixels per stored pixel (columns; rows differ by the pixel aspect).
    let k = view.scale() / s;
    let per_unit = |u: Units| match u {
        Units::Pixel => k,
        Units::Display => x1 - x0,
    };
    let (aw, ah) = (area.2 - area.0, area.3 - area.1);
    let sizes = |u: Units| match u {
        Units::Pixel => Sizes {
            display: aw,
            reach: 16.0 * aw.hypot(ah),
            head: f64::from(HEAD / k.max(1e-6)),
        },
        Units::Display => Sizes {
            display: 1.0,
            reach: 16.0,
            head: f64::from(HEAD / (x1 - x0).max(1e-6)),
        },
    };
    let mut out = Scene::default();
    for a in ps.annotations_for(sop, frame) {
        let layer = ps
            .layer(&a.layer)
            .and_then(|l| l.srgb())
            .map_or(DEFAULT_INK, colour);
        for g in &a.graphics {
            let (pts, closed) = g.outline();
            let (line, fill, width) = look(&g.style, layer, per_unit(g.units));
            out.marks.push(Mark {
                outline: pts.into_iter().map(|p| map(g.units, p)).collect(),
                closed,
                fill: (g.filled && closed).then_some(fill),
                points: g.kind == GraphicKind::Point,
                colour: line,
                width,
                dashed: g.style.dashed,
            });
        }
        for c in &a.compounds {
            compound(
                &mut out,
                c,
                layer,
                &map,
                per_unit(c.units),
                sizes(c.units),
                (x0, y0, x1, y1),
            );
        }
        for t in &a.texts {
            out.notes.push(note(t, layer, &map));
        }
    }
    out
}

fn compound(
    out: &mut Scene,
    c: &Compound,
    layer: Color,
    map: &impl Fn(Units, (f64, f64)) -> (f32, f32),
    per_unit: f32,
    sizes: Sizes,
    area: (f32, f32, f32, f32),
) {
    let (line, fill, width) = look(&c.style, layer, per_unit);
    let (strokes, labels) = c.draw(&sizes);
    for st in strokes {
        let mut outline: Vec<_> = st.points.iter().map(|p| map(c.units, *p)).collect();
        if c.kind == CompoundKind::InfiniteLine {
            let [a, b] = outline[..] else { continue };
            let Some((a, b)) = clip(a, b, area) else {
                continue;
            };
            outline = vec![a, b];
        }
        // Filled arrowheads are solid; filled shapes take the fill style.
        let head = matches!(c.kind, CompoundKind::Arrow | CompoundKind::Cutline);
        out.marks.push(Mark {
            outline,
            closed: st.closed,
            fill: (st.filled && st.closed).then_some(if head { line } else { fill }),
            points: false,
            colour: line,
            width,
            dashed: c.style.dashed && !st.filled,
        });
    }
    for l in labels {
        let (at, tick) = (map(c.units, l.at), map(c.units, l.tick));
        let d = (at.0 - tick.0, at.1 - tick.1);
        let n = d.0.hypot(d.1);
        out.tags.push(Tag {
            at,
            away: if n > 0.0 {
                (d.0 / n, d.1 / n)
            } else {
                (0.0, -1.0)
            },
            text: l.text,
            colour: line,
        });
    }
}

fn note(
    t: &specular_dicom::gsps::TextObject,
    layer: Color,
    map: &impl Fn(Units, (f64, f64)) -> (f32, f32),
) -> Note {
    let (mut justify, mut valign) = (t.justify, t.valign);
    let bbox = t.bbox.map(|(u, a, b)| {
        let (p, q) = (map(u, a), map(u, b));
        // The TLHC shown right of (below) the BRHC: the sides swap with it.
        if p.0 > q.0 {
            justify = match justify {
                Justify::Left => Justify::Right,
                Justify::Right => Justify::Left,
                Justify::Center => Justify::Center,
            };
        }
        if p.1 > q.1 {
            valign = match valign {
                VAlign::Top => VAlign::Bottom,
                VAlign::Bottom => VAlign::Top,
                VAlign::Center => VAlign::Center,
            };
        }
        (p.0.min(q.0), p.1.min(q.1), p.0.max(q.0), p.1.max(q.1))
    });
    let (anchor, anchor_line) = match t.anchor {
        Some((u, p, line)) => (Some(map(u, p)), line),
        None => (None, false),
    };
    Note {
        lines: t.lines.clone(),
        bbox,
        justify,
        valign,
        anchor,
        anchor_line,
        colour: t.colour.map_or(layer, colour),
    }
}

/// Horizontal spans inside a closed polygon every `step` pixels (even–odd rule).
fn spans(poly: &[(f32, f32)], step: f32) -> Vec<Seg> {
    let mut out = Vec::new();
    if poly.len() < 3 {
        return out;
    }
    let ymin = poly
        .iter()
        .map(|p| p.1)
        .fold(f32::MAX, f32::min)
        .max(-16384.0);
    let ymax = poly
        .iter()
        .map(|p| p.1)
        .fold(f32::MIN, f32::max)
        .min(16384.0);
    let mut y = ymin + step / 2.0;
    let mut xs = Vec::new();
    while y < ymax {
        xs.clear();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a.1 <= y) != (b.1 <= y) {
                xs.push(a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(f32::total_cmp);
        for p in xs.chunks_exact(2) {
            out.push(((p[0], y), (p[1], y)));
        }
        y += step;
    }
    out
}

/// A line segment on screen.
type Seg = ((f32, f32), (f32, f32));

/// The dashes (`on` long, `off` apart) along a path of segments, the pattern running on
/// across corners.
fn dashes(segs: &[Seg], on: f32, off: f32) -> Vec<Seg> {
    let mut out = Vec::new();
    let (on, period) = (f64::from(on), f64::from(on + off));
    if period <= 0.0 {
        return segs.to_vec();
    }
    // Dash k covers [k·period, k·period + on] of the distance along the whole path.
    let mut start = 0.0f64;
    for &(a, b) in segs {
        let (dx, dy) = (f64::from(b.0 - a.0), f64::from(b.1 - a.1));
        let len = dx.hypot(dy);
        if len <= 0.0 || !len.is_finite() {
            continue;
        }
        let at = |t: f64| {
            let f = (t - start) / len;
            (
                (f64::from(a.0) + dx * f) as f32,
                (f64::from(a.1) + dy * f) as f32,
            )
        };
        let end = start + len;
        let first = (start / period).floor() as i64;
        let last = ((end / period).floor() as i64).min(first + 100_000);
        for k in first..=last {
            let d0 = (k as f64 * period).max(start);
            let d1 = (k as f64 * period + on).min(end);
            if d1 - d0 > 1e-4 {
                out.push((at(d0), at(d1)));
            }
        }
        start = end;
    }
    out
}

/// Draws a scene: fills, outlines over a dark halo (dashed when styled so), points as
/// crosses, tick labels, then text on a dark backdrop in its box (or beside its anchor)
/// with the anchor line.
pub(crate) fn draw(dl: &mut DrawList, text: &mut Text, scene: &Scene, ts: f32) {
    let halo = Color::rgba(0.0, 0.0, 0.0, 0.55);
    for m in &scene.marks {
        if m.points {
            let r = 5.0;
            for &(x, y) in &m.outline {
                for (a, b) in [((x - r, y), (x + r, y)), ((x, y - r), (x, y + r))] {
                    dl.line(a, b, m.width + 1.5, halo);
                    dl.line(a, b, m.width, m.colour);
                }
            }
            continue;
        }
        if let Some(f) = m.fill {
            for (a, b) in spans(&m.outline, 1.0) {
                dl.line(a, b, 1.0, f);
            }
        }
        let mut segs: Vec<_> = m
            .outline
            .windows(2)
            .map(|w| (w[0], w[1]))
            .chain(
                m.closed
                    .then(|| Some((*m.outline.last()?, *m.outline.first()?)))
                    .flatten(),
            )
            .collect();
        if m.dashed {
            let u = m.width.max(1.0);
            segs = dashes(&segs, 4.0 * u, 2.7 * u);
        }
        for &(a, b) in &segs {
            dl.line(a, b, m.width + 1.5, halo);
        }
        for &(a, b) in &segs {
            dl.line(a, b, m.width, m.colour);
        }
    }
    let st = TextStyle::new(FONT_BODY * ts);
    let pad = 3.0 * ts;
    for g in &scene.tags {
        let b = text.layout(&g.text, st, None, Align::Start);
        // The box's centre far enough along `away` that its near edge sits at `at`.
        let push = g.away.0.abs() * (b.width / 2.0 + pad) + g.away.1.abs() * (b.height / 2.0 + pad);
        let c = (g.at.0 + g.away.0 * push, g.at.1 + g.away.1 * push);
        let back = Rect::new(
            c.0 - b.width / 2.0 - pad,
            c.1 - b.height / 2.0 - pad,
            b.width + 2.0 * pad,
            b.height + 2.0 * pad,
        );
        dl.rect(back, RADIUS_SM, halo);
        text.draw(dl, &b, c.0 - b.width / 2.0, c.1 - b.height / 2.0, g.colour);
    }
    for n in &scene.notes {
        let blocks: Vec<_> = n
            .lines
            .iter()
            .map(|l| text.layout(l, st, None, Align::Start))
            .collect();
        let tw = blocks.iter().map(|b| b.width).fold(0.0, f32::max);
        let th: f32 = blocks.iter().map(|b| b.height).sum();
        let (x, y, w) = match (n.bbox, n.anchor) {
            (Some(b), _) => {
                let room = b.3 - b.1 - th;
                let y = match n.valign {
                    _ if room <= 0.0 => b.1,
                    VAlign::Top => b.1,
                    VAlign::Center => b.1 + room / 2.0,
                    VAlign::Bottom => b.1 + room,
                };
                (b.0, y, (b.2 - b.0).max(tw))
            }
            (None, Some(a)) => (a.0 + 8.0 * ts, a.1 + 8.0 * ts, tw),
            (None, None) => continue,
        };
        let back = Rect::new(x - pad, y - pad, w + 2.0 * pad, th + 2.0 * pad);
        if n.anchor_line
            && let Some(a) = n.anchor
        {
            // From the nearest point of the text's box.
            let p = (
                a.0.clamp(back.x, back.x + back.w),
                a.1.clamp(back.y, back.y + back.h),
            );
            dl.line(p, a, 3.0, halo);
            dl.line(p, a, LINE, n.colour);
        }
        dl.rect(back, RADIUS_SM, halo);
        let mut ty = y;
        for b in &blocks {
            let tx = match n.justify {
                Justify::Left => x,
                Justify::Right => x + w - b.width,
                Justify::Center => x + (w - b.width) / 2.0,
            };
            text.draw(dl, b, tx, ty, n.colour);
            ty += b.height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_dicom::gsps::{Annotation, ImageRef, Layer, TextObject, TickAlign};
    use specular_render::view::{PxRect, display_from_stored};

    /// The state's own order: flip x, then turn clockwise `q` times.
    fn gsps_apply(q: u8, flip: bool, (w, h): (f32, f32), p: (f32, f32)) -> (f32, f32) {
        let mut p = if flip { (w - p.0, p.1) } else { p };
        let (mut w, mut h) = (w, h);
        for _ in 0..q {
            p = (h - p.1, p.0);
            (w, h) = (h, w);
        }
        let _ = (w, h);
        p
    }

    #[test]
    fn orientation_matches_flip_then_rotate() {
        let size = (40.0, 30.0);
        for q in 0..4 {
            for flip in [false, true] {
                let o = gsps_orientation(q, flip);
                let t = display_from_stored(o, size.0, size.1);
                for p in [(0.0, 0.0), (40.0, 0.0), (0.0, 30.0), (12.0, 7.0)] {
                    let a = t.apply(p);
                    let b = gsps_apply(q, flip, size, p);
                    assert!(
                        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4,
                        "q {q} flip {flip} at {p:?}: {a:?} != {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn spans_fill_a_square() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let s = spans(&sq, 1.0);
        assert_eq!(s.len(), 10);
        assert!(s.iter().all(|(a, b)| a.0 == 0.0 && b.0 == 10.0));
    }

    #[test]
    fn dashes_run_on_across_corners() {
        let d = dashes(
            &[((0.0, 0.0), (5.0, 0.0)), ((5.0, 0.0), (5.0, 10.0))],
            4.0,
            2.0,
        );
        // 0..4 on, 4..6 off (round the corner), 6..10 on, 10..12 off, 12..15 on.
        let close =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4;
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(close(d[0].0, (0.0, 0.0)) && close(d[0].1, (4.0, 0.0)));
        assert!(close(d[1].0, (5.0, 1.0)) && close(d[1].1, (5.0, 5.0)));
        assert!(close(d[2].0, (5.0, 7.0)) && close(d[2].1, (5.0, 10.0)));
    }

    #[test]
    fn clip_keeps_the_inside() {
        let r = (0.0, 0.0, 10.0, 10.0);
        let (a, b) = clip((-10.0, 5.0), (20.0, 5.0), r).unwrap();
        assert_eq!((a, b), ((0.0, 5.0), (10.0, 5.0)));
        assert!(clip((-10.0, 20.0), (20.0, 20.0), r).is_none());
    }

    fn state(q: u16, flip: bool) -> PresentationState {
        let refs = vec![ImageRef {
            sop_uid: "1.2".into(),
            frames: Vec::new(),
        }];
        PresentationState {
            references: refs.clone(),
            rotation: q,
            flip,
            layers: vec![Layer {
                name: "L".into(),
                order: 1,
                colour: None,
                grey: Some(65535),
                description: None,
            }],
            annotations: vec![Annotation {
                layer: "L".into(),
                refs,
                graphics: Vec::new(),
                texts: vec![TextObject {
                    lines: vec!["T".into()],
                    bbox: Some((Units::Pixel, (10.0, 20.0), (30.0, 40.0))),
                    justify: Justify::Left,
                    anchor: None,
                    colour: Some([255, 0, 0]),
                    valign: VAlign::Top,
                    compound: None,
                }],
                compounds: vec![Compound {
                    units: Units::Pixel,
                    kind: CompoundKind::Axis,
                    points: vec![(0.0, 50.0), (100.0, 50.0)],
                    rotation: 0.0,
                    pivot: None,
                    gap: None,
                    visible: None,
                    ticks: vec![specular_dicom::gsps::Tick {
                        at: 0.5,
                        label: Some("5".into()),
                    }],
                    tick_align: TickAlign::Center,
                    show_labels: true,
                    labels_below: false,
                    filled: false,
                    style: Style {
                        thickness: Some(2.0),
                        dashed: true,
                        ..Style::default()
                    },
                    id: None,
                }],
            }],
            ..PresentationState::default()
        }
    }

    fn view(q: u16, flip: bool) -> View {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        });
        v.set_image(100, 100, gsps_orientation((q / 90) as u8, flip));
        v
    }

    /// Text boxes move with the image; their sides swap with the TLHC; the text keeps its
    /// own colour; compound lines take their thickness and dashing.
    #[test]
    fn text_stays_upright_and_compounds_draw() {
        let ps = state(0, false);
        let sc = scene(&ps, "1.2", 0, &view(0, false), 1.0);
        let n = &sc.notes[0];
        assert_eq!(n.bbox, Some((10.0, 20.0, 30.0, 40.0)));
        assert_eq!((n.justify, n.valign), (Justify::Left, VAlign::Top));
        assert_eq!(n.colour, Color::rgba(1.0, 0.0, 0.0, 1.0));
        // The axis and one tick; tick label above the line, pushed up.
        assert_eq!(sc.marks.len(), 2);
        assert!(sc.marks.iter().all(|m| m.width == 2.0 && m.dashed));
        assert_eq!(sc.tags.len(), 1);
        assert!(sc.tags[0].away.1 < -0.99, "{:?}", sc.tags[0]);

        // Flipped: the TLHC is on the right, LEFT becomes RIGHT.
        let ps = state(0, true);
        let n = &scene(&ps, "1.2", 0, &view(0, true), 1.0).notes[0];
        assert_eq!(n.bbox, Some((70.0, 20.0, 90.0, 40.0)));
        assert_eq!((n.justify, n.valign), (Justify::Right, VAlign::Top));

        // A half turn: right and bottom.
        let ps = state(180, false);
        let sc = scene(&ps, "1.2", 0, &view(180, false), 1.0);
        let n = &sc.notes[0];
        assert_eq!(n.bbox, Some((70.0, 60.0, 90.0, 80.0)));
        assert_eq!((n.justify, n.valign), (Justify::Right, VAlign::Bottom));
        // The tick label still on the image's "up" side of the axis: now below on screen.
        assert!(sc.tags[0].away.1 > 0.99);
    }

    #[test]
    fn infinite_lines_stay_in_the_area() {
        let mut ps = state(0, false);
        let c = &mut ps.annotations[0].compounds[0];
        c.kind = CompoundKind::InfiniteLine;
        c.points = vec![(40.0, 50.0), (60.0, 50.0)];
        let sc = scene(&ps, "1.2", 0, &view(0, false), 1.0);
        assert_eq!(sc.marks.len(), 1);
        let o = &sc.marks[0].outline;
        let close =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3;
        assert!(
            o.len() == 2 && close(o[0], (0.0, 50.0)) && close(o[1], (100.0, 50.0)),
            "{o:?}"
        );
    }
}

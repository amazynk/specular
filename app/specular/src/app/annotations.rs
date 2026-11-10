//! Slide annotations over the image: the shapes of an NDPA, Aperio ImageScope or ASAP
//! sidecar ([`specular_decode::science::AnnotationSet`]) placed on screen each frame and drawn
//! as outlines with a dark halo, markers, arrow heads, names and ruler lengths.
//! `Shift+a` ([`specular_input::Action::ToggleAnnotations`]) shows or hides them, kept across
//! files.
//!
//! Placing culls shapes outside the view and drops points closer than a pixel to the last
//! one kept, so a slide with thousands of detailed outlines draws a few points per shape
//! when zoomed out.

use specular_decode::science::annot::{Annotation, Annotations, Shape};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_theme::Color;
use specular_theme::metrics::FONT_SMALL;

/// Points of an ellipse's outline.
const ELLIPSE_SEGMENTS: usize = 64;
/// Most labels drawn at once.
const MAX_LABELS: usize = 200;
/// A named shape wider or taller than this on screen (logical px) gets its label.
const LABEL_MIN_PX: f32 = 24.0;
/// The ink of shapes without a colour.
const DEFAULT_INK: [u8; 3] = [255, 214, 0];

/// One shape as drawn, in logical px.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShapeDraw {
    pub pts: Vec<(f32, f32)>,
    pub closed: bool,
    /// Each point is a marker.
    pub markers: bool,
    /// An arrow head at the last point.
    pub arrow: bool,
    pub colour: [u8; 3],
    /// 1 for a shape on the shown plane, less for one on a plane next to it.
    pub alpha: f32,
    /// Its name, and for a ruler its length.
    pub label: Option<String>,
}

/// The shapes of `set` that show in `view` (logical px), `map` taking slide pixels to logical
/// px. `length` names a ruler's length from its points (slide pixels), when the slide's pixel
/// size is known.
pub(super) fn place(
    set: &Annotations,
    map: impl Fn((f32, f32)) -> (f32, f32),
    view: Rect,
    length: impl Fn(&Annotation) -> Option<String>,
) -> Vec<ShapeDraw> {
    place_with(set, map, view, length, |_| Some(1.0))
}

/// [`place`], with `opacity(k)` saying how strongly item `k` shows (`None`: not at all), for
/// shapes that belong to some planes only.
pub(super) fn place_with(
    set: &Annotations,
    map: impl Fn((f32, f32)) -> (f32, f32),
    view: Rect,
    length: impl Fn(&Annotation) -> Option<String>,
    opacity: impl Fn(usize) -> Option<f32>,
) -> Vec<ShapeDraw> {
    let near = view.inset(-8.0);
    let mut out = Vec::new();
    for (k, a) in set.items.iter().enumerate() {
        if a.points.is_empty() {
            continue;
        }
        let Some(alpha) = opacity(k) else {
            continue;
        };
        let (x0, y0, x1, y1) = a.bounds();
        let corners =
            [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|(x, y)| map((x as f32, y as f32)));
        let (sx0, sy0, sx1, sy1) = corners.iter().fold(
            (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ),
            |b, &(x, y)| (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y)),
        );
        if !(sx0.is_finite() && sy0.is_finite() && sx1.is_finite() && sy1.is_finite())
            || sx1 < near.x
            || sy1 < near.y
            || sx0 > near.right()
            || sy0 > near.bottom()
        {
            continue;
        }
        let markers = matches!(a.shape, Shape::Point | Shape::Points);
        let tiny = (sx1 - sx0).max(sy1 - sy0) < 2.0;
        let pts = if markers {
            a.points
                .iter()
                .map(|&(x, y)| map((x as f32, y as f32)))
                .filter(|&p| near.contains(p))
                .collect()
        } else if tiny {
            // Smaller than two pixels: a dot.
            vec![((sx0 + sx1) / 2.0, (sy0 + sy1) / 2.0)]
        } else {
            let outline = a.outline(ELLIPSE_SEGMENTS);
            let last = outline.len() - 1;
            let mut pts: Vec<(f32, f32)> = Vec::new();
            for (i, &(x, y)) in outline.iter().enumerate() {
                let p = map((x as f32, y as f32));
                // Keep the end points (rulers, arrows), drop sub-pixel steps.
                match pts.last() {
                    Some(&q) if i != last && (p.0 - q.0).abs() < 1.0 && (p.1 - q.1).abs() < 1.0 => {
                    }
                    _ => pts.push(p),
                }
            }
            pts
        };
        if pts.is_empty() {
            continue;
        }
        let measured = matches!(a.shape, Shape::Ruler).then(|| length(a)).flatten();
        let label = match (a.name.trim(), measured) {
            ("", m) => m,
            (n, Some(m)) => Some(specular_i18n::dot_join::<&str>(&[n, &m])),
            (n, None) => Some(n.to_owned()),
        }
        .filter(|_| markers || (sx1 - sx0).max(sy1 - sy0) >= LABEL_MIN_PX);
        out.push(ShapeDraw {
            closed: a.shape.closed() && !tiny,
            markers: markers || tiny,
            arrow: a.shape == Shape::Arrow && !tiny,
            colour: a.colour.unwrap_or(DEFAULT_INK),
            alpha,
            // Faded shapes go unnamed: their planes' own labels stay legible.
            label: label.filter(|_| alpha >= 1.0),
            pts,
        });
    }
    out
}

fn ink(c: [u8; 3], alpha: f32) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        alpha,
    )
}

/// Draws `shapes` clipped to `clip` (logical px), labels at `ts` text scale.
pub(super) fn draw(dl: &mut DrawList, text: &mut Text, shapes: &[ShapeDraw], clip: Rect, ts: f32) {
    let halo = Color::rgba(0.0, 0.0, 0.0, 0.6);
    dl.push_clip(clip);
    // Halos first, so one shape's halo never covers another's line.
    for pass in 0..2 {
        for s in shapes {
            let (w, c) = if pass == 0 {
                (3.5, Color::rgba(0.0, 0.0, 0.0, halo.a * s.alpha))
            } else {
                (1.5, ink(s.colour, s.alpha))
            };
            if s.markers {
                for &p in &s.pts {
                    dl.circle(p, if pass == 0 { 4.5 } else { 3.0 }, c);
                }
                continue;
            }
            for seg in s.pts.windows(2) {
                dl.line(seg[0], seg[1], w, c);
            }
            if s.closed
                && s.pts.len() > 2
                && let (Some(&a), Some(&b)) = (s.pts.last(), s.pts.first())
            {
                dl.line(a, b, w, c);
            }
            if s.arrow
                && let [.., tail, head] = s.pts[..]
            {
                let (dx, dy) = (head.0 - tail.0, head.1 - tail.1);
                let len = dx.hypot(dy);
                if len > 1.0 {
                    let (ux, uy) = (dx / len, dy / len);
                    let size = 10.0_f32.min(len / 2.0);
                    for side in [-1.0, 1.0] {
                        let p = (
                            head.0 - size * (ux * 0.87 - side * uy * 0.5),
                            head.1 - size * (uy * 0.87 + side * ux * 0.5),
                        );
                        dl.line(head, p, w, c);
                    }
                }
            }
        }
    }
    let st = TextStyle::new(FONT_SMALL * ts);
    let mut placed: Vec<Rect> = Vec::new();
    for s in shapes.iter().filter(|s| s.label.is_some()).take(MAX_LABELS) {
        let (Some(label), Some(&p)) = (&s.label, s.pts.iter().min_by(|a, b| a.1.total_cmp(&b.1)))
        else {
            continue;
        };
        let b = text.layout(label, st, None, Align::Start);
        let x = (p.0 - b.width / 2.0).clamp(
            clip.x + 2.0,
            (clip.right() - b.width - 6.0).max(clip.x + 2.0),
        );
        let y = (p.1 - b.height - 8.0).clamp(
            clip.y + 2.0,
            (clip.bottom() - b.height - 4.0).max(clip.y + 2.0),
        );
        // A dark chip with the shape's colour as a swatch on its left and white text, legible
        // over any stain.
        let r = Rect::new(x, y, b.width + 11.0, b.height + 2.0);
        if placed
            .iter()
            .any(|q| r.x < q.right() && q.x < r.right() && r.y < q.bottom() && q.y < r.bottom())
        {
            continue;
        }
        placed.push(r);
        dl.rect(r, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.78));
        dl.rect(
            Rect::new(x + 2.0, y + 2.0, 3.0, b.height - 2.0),
            1.0,
            ink(s.colour, 1.0),
        );
        text.draw(dl, &b, x + 8.0, y + 1.0, Color::rgba(1.0, 1.0, 1.0, 0.95));
    }
    dl.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::science::annot::Format;

    fn shape(shape: Shape, name: &str, points: Vec<(f64, f64)>) -> Annotation {
        Annotation {
            name: name.to_owned(),
            group: String::new(),
            shape,
            colour: Some([255, 0, 0]),
            points,
        }
    }

    #[test]
    fn shapes_are_culled_and_decimated() {
        let wiggle: Vec<(f64, f64)> = (0..1000).map(|i| (f64::from(i) * 0.01, 0.0)).collect();
        let set = Annotations {
            format: Format::Asap,
            items: vec![
                shape(
                    Shape::Polygon,
                    "in",
                    vec![(10.0, 10.0), (90.0, 10.0), (90.0, 90.0)],
                ),
                shape(
                    Shape::Polygon,
                    "out",
                    vec![(1000.0, 1000.0), (1100.0, 1000.0), (1100.0, 1100.0)],
                ),
                shape(Shape::Polyline, "", wiggle),
                shape(Shape::Ruler, "r", vec![(0.0, 50.0), (40.0, 50.0)]),
                shape(Shape::Ellipse, "e", vec![(20.0, 20.0), (60.0, 40.0)]),
                shape(Shape::Point, "p", vec![(5.0, 5.0)]),
            ],
            skipped: 0,
        };
        // Two logical px per slide pixel.
        let view = Rect::new(0.0, 0.0, 400.0, 400.0);
        let d = place(
            &set,
            |(x, y)| (x * 2.0, y * 2.0),
            view,
            |a| Some(format!("{} µm", a.length(0.5, 0.5))),
        );
        assert_eq!(d.len(), 5, "{d:?}");
        assert_eq!(d[0].pts, vec![(20.0, 20.0), (180.0, 20.0), (180.0, 180.0)]);
        assert!(d[0].closed);
        assert_eq!(d[0].label.as_deref(), Some("in"));
        // 10 slide px over 1000 points: one point per logical pixel, and the last.
        assert_eq!(d[1].pts.len(), 21, "{:?}", d[1].pts);
        assert_eq!(d[1].pts.last(), Some(&(19.98, 0.0)));
        assert_eq!(d[2].label.as_deref(), Some("r · 20 µm"));
        assert_eq!(d[3].pts.len(), ELLIPSE_SEGMENTS);
        assert!(d[4].markers);
        // Zoomed far out the triangle is a dot without its label.
        let d = place(&set, |(x, y)| (x * 0.01, y * 0.01), view, |_| None);
        assert!(d[0].markers && d[0].pts.len() == 1 && d[0].label.is_none());
    }
}

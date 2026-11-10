//! Measuring on the ellipsoid: a distance or path with its bearing, an area with its
//! perimeter (Karney's geodesics, `specular-geodesy`), and an elevation profile along a
//! drawn line over the map's heights, charted with its length, lowest and highest point
//! and total climb.
//!
//! Points are kept as latitude and longitude, so they stay put while the map zooms and
//! pans. A click adds one; the keyboard moves a crosshair (the arrows, Shift for larger
//! steps) and Space adds one there; Backspace takes the last back, Enter finishes and Esc
//! closes the tool.

use specular_geodesy::geodesic;
use specular_geodesy::profile::{Profile, profile};
use specular_geodesy::units::{self, Units, format_value};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};

/// Samples of an elevation profile.
pub(super) const PROFILE_SAMPLES: usize = 512;
/// The chart's size, logical px.
const CHART_W: f32 = 440.0;
const CHART_H: f32 = 170.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Distance,
    Area,
    Profile,
}

/// The measuring tool.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Tool {
    pub kind: Kind,
    /// (latitude, longitude).
    pub pts: Vec<(f64, f64)>,
    /// The keyboard's crosshair in stored px, once an arrow moved it.
    pub cross: Option<(f64, f64)>,
    /// Finished (Enter): further clicks start again.
    pub done: bool,
}

impl Tool {
    pub(super) fn new(kind: Kind) -> Tool {
        Tool {
            kind,
            pts: Vec::new(),
            cross: None,
            done: false,
        }
    }

    /// Adds a point (a finished tool starts again).
    pub(super) fn add(&mut self, p: (f64, f64)) {
        if self.done {
            self.pts.clear();
            self.done = false;
        }
        if self.pts.last() != Some(&p) {
            self.pts.push(p);
        }
    }

    /// Enter: done when there is something to measure.
    pub(super) fn finish(&mut self) -> bool {
        let need = if self.kind == Kind::Area { 3 } else { 2 };
        if self.pts.len() >= need {
            self.done = true;
        }
        self.done
    }
}

/// A length as text in `units`: "12.3 km".
pub(super) fn length_text(m: f64, u: Units) -> String {
    let (v, unit) = units::length(m, u);
    t!(
        "map-length",
        value = format_value(v),
        unit = specular_i18n::unit_symbol(unit.symbol())
    )
}

/// An area as text in `units`: "1.25 km²".
pub(super) fn area_text(m2: f64, u: Units) -> String {
    let (v, unit) = units::area(m2, u);
    t!(
        "map-length",
        value = format_value(v),
        unit = specular_i18n::unit_symbol(unit.symbol())
    )
}

/// A bearing as text: "045.2°".
pub(super) fn bearing_text(deg: f64) -> String {
    // Zero-padded to three whole digits, as compasses write bearings.
    let digits = specular_i18n::fixed(deg, 1);
    let whole = digits.chars().take_while(char::is_ascii_digit).count();
    let pad = "0".repeat(3usize.saturating_sub(whole));
    t!("map-bearing-value", degrees = format!("{pad}{digits}"))
}

/// The result lines of the tool, with `hover` (where the pointer is) as a next point
/// while it is not done.
pub(super) fn result_lines(tool: &Tool, hover: Option<(f64, f64)>, u: Units) -> Vec<String> {
    let mut pts = tool.pts.clone();
    if !tool.done
        && let Some(h) = hover
    {
        pts.push(h);
    }
    let mut lines = Vec::new();
    match tool.kind {
        Kind::Distance | Kind::Profile => {
            if pts.len() < 2 {
                lines.push(match tool.kind {
                    Kind::Profile => t!("map-measure-hint-profile"),
                    _ => t!("map-measure-hint-distance"),
                });
                return lines;
            }
            let total = geodesic::path_length(&pts);
            lines.push(t!("map-measure-distance", length = length_text(total, u)));
            let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
            let inv = geodesic::inverse(a.0, a.1, b.0, b.1);
            let bearing = geodesic::bearing(a.0, a.1, b.0, b.1);
            if pts.len() > 2 {
                lines.push(t!(
                    "map-measure-segment",
                    length = length_text(inv.s12, u),
                    bearing = bearing_text(bearing)
                ));
            } else {
                lines.push(t!("map-measure-bearing", bearing = bearing_text(bearing)));
            }
        }
        Kind::Area => {
            if pts.len() < 3 {
                lines.push(t!("map-measure-hint-area"));
                return lines;
            }
            let (area, perimeter) = geodesic::polygon_area(&pts);
            lines.push(t!("map-measure-area", area = area_text(area, u)));
            lines.push(t!(
                "map-measure-perimeter",
                length = length_text(perimeter, u)
            ));
        }
    }
    if !tool.done {
        lines.push(t!("map-measure-keys"));
    }
    lines
}

/// Points along the geodesics between `pts` (each edge cut into pieces of at most about
/// `step` metres, at most 64), for drawing; `close` adds the edge back to the start.
pub(super) fn densify(pts: &[(f64, f64)], close: bool, step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let n = pts.len();
    let edges = if close && n > 2 {
        n
    } else {
        n.saturating_sub(1)
    };
    for i in 0..edges {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let s = geodesic::inverse(a.0, a.1, b.0, b.1).s12;
        let k = ((s / step.max(1.0)).ceil() as usize).clamp(1, 64);
        for j in 0..k {
            out.push(geodesic::interpolate(
                a.0,
                a.1,
                b.0,
                b.1,
                j as f64 / k as f64,
            ));
        }
    }
    if let Some(&last) = if close && n > 2 {
        pts.first()
    } else {
        pts.last()
    } {
        out.push(last);
    }
    out
}

/// The elevation profile of the tool's line over `height(lat, lon)` (metres).
pub(super) fn elevation(pts: &[(f64, f64)], height: impl Fn(f64, f64) -> Option<f64>) -> Profile {
    profile(pts, PROFILE_SAMPLES, height)
}

/// Heights shown in `u`: metres or feet.
fn height_text(m: f64, u: Units) -> String {
    match u {
        Units::Metric => t!(
            "map-length",
            value = specular_i18n::fixed(m, 0),
            unit = specular_i18n::unit_symbol("m")
        ),
        Units::Imperial => t!(
            "map-length",
            value = specular_i18n::fixed(m / 0.3048, 0),
            unit = specular_i18n::unit_symbol("ft")
        ),
    }
}

/// The chart's summary line: length, lowest, highest, climb and descent.
pub(super) fn profile_summary(p: &Profile, u: Units) -> String {
    match (p.min, p.max) {
        (Some(lo), Some(hi)) => t!(
            "map-profile-summary",
            length = length_text(p.length, u),
            min = height_text(lo, u),
            max = height_text(hi, u),
            climb = height_text(p.climb, u),
            descent = height_text(p.descent, u)
        ),
        _ => t!("map-profile-no-heights", length = length_text(p.length, u)),
    }
}

/// The value at the chart's cursor: distance and height.
pub(super) fn cursor_text(p: &Profile, i: usize, u: Units) -> Option<String> {
    let s = p.samples.get(i)?;
    Some(match s.height {
        Some(h) => t!(
            "map-profile-at",
            distance = length_text(s.distance, u),
            height = height_text(h, u)
        ),
        None => t!("map-profile-at-none", distance = length_text(s.distance, u)),
    })
}

/// The chart as drawn: its panel and plot rectangles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct DrawnChart {
    pub panel: Option<Rect>,
    pub plot: Option<Rect>,
}

/// The sample nearest to x `x` (logical px) in the plot.
pub(super) fn sample_at(p: &Profile, plot: Rect, x: f32, rtl: bool) -> Option<usize> {
    if p.samples.is_empty() || p.length <= 0.0 || plot.w <= 0.0 {
        return None;
    }
    let mut t = f64::from(((x - plot.x) / plot.w).clamp(0.0, 1.0));
    if rtl {
        t = 1.0 - t;
    }
    let d = t * p.length;
    p.samples
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1.distance - d)
                .abs()
                .total_cmp(&(b.1.distance - d).abs())
        })
        .map(|(i, _)| i)
}

/// Draws the chart at the bottom centre of `area` (above `floor`, the table's top when it
/// is open). Distance runs with the reading direction.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_chart(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &Profile,
    cursor: Option<usize>,
    u: Units,
    area: (f32, f32, f32, f32),
    floor: Option<f32>,
    rtl: bool,
    ts: f32,
) -> DrawnChart {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    let w = (CHART_W * ts).min(aw - 2.0 * m);
    let h = (CHART_H * ts).min(ah * 0.5);
    let bottom = floor.map_or(ay + ah - m, |f| f - 8.0 * ts);
    let x = ax + (aw - w) / 2.0;
    let y = bottom - h;
    let panel = Rect::new(x, y, w, h);
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let title = text.layout(
        &t!("palette-map-elevation-profile"),
        st.weight(600.0),
        None,
        Align::Start,
    );
    let tx = if rtl {
        x + w - PAD - title.width
    } else {
        x + PAD
    };
    text.draw(dl, &title, tx, y + PAD, pal.text);
    let summary = text.layout(
        &profile_summary(p, u),
        st,
        Some(w - 2.0 * PAD),
        Align::Start,
    );
    let sx = if rtl {
        x + w - PAD - summary.width
    } else {
        x + PAD
    };
    let sy = y + h - PAD - summary.height;
    text.draw(dl, &summary, sx, sy, pal.text_muted);
    // Height labels on the start side.
    let (lo, hi) = match (p.min, p.max) {
        (Some(lo), Some(hi)) if hi > lo => (lo, hi),
        (Some(lo), Some(_)) => (lo - 1.0, lo + 1.0),
        _ => (0.0, 1.0),
    };
    let lab_hi = text.layout(&height_text(hi, u), st, None, Align::Start);
    let lab_lo = text.layout(&height_text(lo, u), st, None, Align::Start);
    let lab_w = lab_hi.width.max(lab_lo.width) + 6.0 * ts;
    let top = y + PAD + title.height + 6.0 * ts;
    let plot_h = (sy - 6.0 * ts - top).max(10.0);
    let (px0, px1) = if rtl {
        (x + PAD, x + w - PAD - lab_w)
    } else {
        (x + PAD + lab_w, x + w - PAD)
    };
    let plot = Rect::new(px0, top, px1 - px0, plot_h);
    let lx = if rtl { px1 + 6.0 * ts } else { x + PAD };
    text.draw(
        dl,
        &lab_hi,
        lx,
        top - lab_hi.height / 2.0 + 2.0 * ts,
        pal.text_muted,
    );
    text.draw(
        dl,
        &lab_lo,
        lx,
        top + plot_h - lab_lo.height / 2.0 - 2.0 * ts,
        pal.text_muted,
    );
    dl.line(
        (plot.x, plot.y),
        (plot.x + plot.w, plot.y),
        ts,
        pal.separator,
    );
    dl.line(
        (plot.x, plot.y + plot.h),
        (plot.x + plot.w, plot.y + plot.h),
        ts,
        pal.separator,
    );
    let to_xy = |d: f64, hgt: f64| {
        let mut t = if p.length > 0.0 {
            (d / p.length) as f32
        } else {
            0.0
        };
        if rtl {
            t = 1.0 - t;
        }
        let v = ((hgt - lo) / (hi - lo)) as f32;
        (
            plot.x + t * plot.w,
            plot.y + plot.h - v.clamp(0.0, 1.0) * plot.h,
        )
    };
    // The terrain: soft columns under the line, then the line.
    let fill = pal.accent.with_alpha(0.22);
    let mut prev: Option<(f32, f32)> = None;
    for s in &p.samples {
        let Some(hgt) = s.height else {
            prev = None;
            continue;
        };
        let q = to_xy(s.distance, hgt);
        if let Some(a) = prev {
            let w = (q.0 - a.0).abs().max(ts);
            let x0 = a.0.min(q.0);
            let ty = a.1.min(q.1);
            dl.rect(Rect::new(x0, ty, w, plot.y + plot.h - ty), 0.0, fill);
            dl.line(a, q, 1.6 * ts, pal.accent);
        }
        prev = Some(q);
    }
    if let Some(i) = cursor
        && let Some(s) = p.samples.get(i)
    {
        let (cx, cy) = to_xy(s.distance, s.height.unwrap_or(lo));
        dl.line((cx, plot.y), (cx, plot.y + plot.h), ts, pal.text);
        if s.height.is_some() {
            dl.circle((cx, cy), 3.5 * ts, pal.text);
        }
        if let Some(c) = cursor_text(p, i, u) {
            let b = text.layout(&c, st, None, Align::Start);
            let bx = (cx - b.width / 2.0).clamp(x + PAD, x + w - PAD - b.width);
            let br = Rect::new(
                bx - 4.0 * ts,
                y - b.height - 10.0 * ts,
                b.width + 8.0 * ts,
                b.height + 6.0 * ts,
            );
            dl.rect(br, RADIUS_MD, Color::rgba(0.0, 0.0, 0.0, 0.7));
            text.draw(dl, &b, bx, br.y + 3.0 * ts, Color::WHITE);
        }
    }
    DrawnChart {
        panel: Some(panel),
        plot: Some(plot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In Hebrew the map writes its lengths, areas, heights and scale bars with the Hebrew
    /// abbreviations (metric and imperial); in English with the SI and imperial symbols.
    #[test]
    fn map_units_are_written_as_the_language_writes_them() {
        let plain = |s: String| specular_i18n::strip_isolates(&s);
        let m = Units::Metric;
        let i = Units::Imperial;
        let bar = |mpp: f64, u: Units| super::super::furniture::scale_bar(mpp, u).unwrap().label;
        let he = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            [
                length_text(250.0, m),
                length_text(12_300.0, m),
                area_text(500.0, m),
                area_text(50_000.0, m),
                area_text(5.0e7, m),
                height_text(812.0, m),
                length_text(30.0, i),
                length_text(16_000.0, i),
                area_text(100.0, i),
                area_text(20_000.0, i),
                area_text(5.0e7, i),
                bar(1.0, m),
                bar(100.0, m),
                bar(1.0, i),
                bar(100.0, i),
            ]
            .map(plain)
        });
        let want = [
            "מ׳",
            "ק״מ",
            "מ״ר",
            "הקטר",
            "קמ״ר",
            "מ׳",
            "רגל",
            "מייל",
            "רגל רבוע",
            "אקר",
            "מייל רבוע",
            "מ׳",
            "ק״מ",
            "רגל",
            "מייל",
        ];
        for (got, want) in he.iter().zip(want) {
            assert!(got.ends_with(&format!(" {want}")), "{got:?} ≠ … {want}");
        }
        assert!(plain(length_text(12_300.0, m)).ends_with(" km"));
        assert!(plain(area_text(50_000.0, m)).ends_with(" ha"));
        assert!(plain(area_text(20_000.0, i)).ends_with(" ac"));
    }

    #[test]
    fn distance_bearing_and_area_on_the_ellipsoid() {
        let mut t = Tool::new(Kind::Distance);
        // One degree of latitude at the equator: 110 574.4 m on WGS 84.
        t.add((0.0, 0.0));
        let l = result_lines(&t, Some((1.0, 0.0)), Units::Metric);
        assert!(l[0].contains("111") && l[0].contains("km"), "{l:?}");
        assert!(l[1].contains("000.0"), "{l:?}");
        t.add((1.0, 0.0));
        t.add((1.0, 1.0));
        assert!(t.finish());
        let l = result_lines(&t, None, Units::Imperial);
        assert!(l[0].contains("mi"), "{l:?}");
        assert!(l[1].contains("090.0") || l[1].contains("089."), "{l:?}");
        assert_eq!(l.len(), 2, "done: no key hint");
        // A finished tool starts again on the next point.
        t.add((5.0, 5.0));
        assert_eq!(t.pts, [(5.0, 5.0)]);
        let mut a = Tool::new(Kind::Area);
        for p in [(0.0, 0.0), (0.0, 0.01), (0.01, 0.01)] {
            a.add(p);
        }
        assert!(!result_lines(&a, None, Units::Metric).is_empty());
        a.add((0.01, 0.0));
        let l = result_lines(&a, None, Units::Metric);
        // About 1.23 km² (1.106 km × 1.113 km).
        assert!(l[0].contains("1.23") && l[0].contains("km²"), "{l:?}");
        let mut b = Tool::new(Kind::Area);
        b.add((0.0, 0.0));
        b.add((0.0, 1.0));
        assert!(!b.finish(), "an area needs three corners");
    }

    #[test]
    fn densified_edges_follow_the_geodesic() {
        let pts = [(10.0, 0.0), (10.0, 60.0)];
        let d = densify(&pts, false, 100_000.0);
        assert!(d.len() > 10 && d.len() <= 66);
        // The geodesic bulges towards the pole between the two points.
        let mid = d[d.len() / 2];
        assert!(mid.0 > 10.5, "{mid:?}");
        assert_eq!(*d.last().unwrap(), (10.0, 60.0));
        let ring = densify(&[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)], true, 1e9);
        assert_eq!(ring.len(), 4);
        assert_eq!(ring[3], (0.0, 0.0));
    }

    #[test]
    fn profile_over_a_ramp() {
        // Heights rise 1 m per 0.001° of longitude.
        let p = elevation(&[(0.0, 0.0), (0.0, 0.1)], |_, lon| Some(lon * 1000.0));
        assert_eq!(p.min, Some(0.0));
        let max = p.max.unwrap();
        assert!((max - 100.0).abs() < 1e-6, "{max}");
        assert!((p.climb - 100.0).abs() < 1e-6 && p.descent.abs() < 1e-9);
        let s = profile_summary(&p, Units::Metric);
        assert!(s.contains("100") && s.contains("11.1"), "{s}");
        let plot = Rect::new(0.0, 0.0, 100.0, 50.0);
        let i = sample_at(&p, plot, 50.0, false).unwrap();
        assert!((p.samples[i].distance / p.length - 0.5).abs() < 0.01);
        let j = sample_at(&p, plot, 0.0, true).unwrap();
        assert_eq!(
            j,
            p.samples.len() - 1,
            "right to left: the end is on the left"
        );
        assert!(cursor_text(&p, i, Units::Metric).is_some());
    }
}

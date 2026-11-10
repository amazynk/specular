//! Plots (spectra) shown as drawings: the traces are the drawing's vectors on the GPU, and
//! this lays the chart's axes over them at the exact zoom: grid lines and tick labels for
//! the values in view, the axis titles, peak marks, a legend and the values under the
//! pointer. Zooming and panning are the drawing's (the wheel, a zoom rectangle, the keys);
//! the ticks follow.

use specular_decode::drawing::DrawingPage;
use specular_decode::drawing::specular_vector::kurbo::{Affine, Point, Rect as KRect};
use specular_decode::drawing::specular_vector::{Chart, ChartAxis};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_render::View;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD};
use specular_theme::{Color, Palette};

use super::App;
use super::cad::shown_name;

/// Logical px between x ticks and between y ticks, about.
const X_TICK_PX: f64 = 110.0;
const Y_TICK_PX: f64 = 64.0;
/// Peak labels drawn at most (the highest first).
const MAX_MARKS: usize = 40;

/// What the chart overlay draws this frame (logical px).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ChartView {
    /// The part of the plot area in view.
    pub frame: Rect,
    /// Screen x of each x tick and its label; screen y of each y tick and its label.
    pub x_ticks: Vec<(f32, String)>,
    pub y_ticks: Vec<(f32, String)>,
    pub x_title: String,
    pub y_title: String,
    /// Peak marks: where, and the label.
    pub marks: Vec<((f32, f32), String)>,
    /// Trace names and colours, when there are several.
    pub legend: Vec<(String, [u8; 3])>,
    /// The values under the pointer.
    pub readout: Option<String>,
    pub dark: bool,
}

fn title(a: &ChartAxis) -> String {
    match (a.title.is_empty(), a.unit.is_empty()) {
        (_, true) => a.title.clone(),
        (true, false) => a.unit.clone(),
        (false, false) => format!("{} ({})", a.title, a.unit),
    }
}

/// Decimals that tell ticks `step` apart.
fn decimals(step: f64) -> usize {
    if !(step.is_finite() && step > 0.0) {
        return 0;
    }
    (-step.log10().floor()).clamp(0.0, 9.0) as usize
}

/// A tick or readout value: plain with `dec` decimals, or in powers of ten when it is very
/// large or small.
pub(super) fn value_text(v: f64, dec: usize, big: bool) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if big {
        let s = format!("{v:.2e}");
        // "1.20e5" → "1.2e5".
        return match s.split_once('e') {
            Some((m, e)) => {
                let m = m.trim_end_matches('0').trim_end_matches('.');
                format!("{m}e{e}")
            }
            None => s,
        };
    }
    let s = format!("{v:.dec$}");
    if s.starts_with('-')
        && s.trim_start_matches('-')
            .chars()
            .all(|c| c == '0' || c == '.')
    {
        return "0".into();
    }
    s.replace('-', "−")
}

/// Whether values around `span` and `extent` read better in powers of ten.
fn wants_exponent(step: f64, extent: f64) -> bool {
    extent >= 1e6 || (step > 0.0 && step < 1e-4)
}

/// Ticks of `axis` between fractions `fa` and `fb` of the frame, about `n` of them: each
/// tick's fraction along the frame and its label.
fn ticks(axis: &ChartAxis, fa: f64, fb: f64, n: usize) -> (Vec<(f64, String)>, usize, bool) {
    let (va, vb) = (axis.value(fa), axis.value(fb));
    let (lo, hi) = (va.min(vb), va.max(vb));
    let vals = specular_widgets::nice_ticks(lo, hi, n.max(2));
    let step = if vals.len() >= 2 {
        (vals[1] - vals[0]).abs()
    } else {
        (hi - lo).abs()
    };
    let big = wants_exponent(step, lo.abs().max(hi.abs()));
    let dec = decimals(step);
    let out = vals
        .into_iter()
        .map(|v| (axis.fraction(v), value_text(v, dec, big)))
        .collect();
    (out, dec + 1, big)
}

impl App {
    /// The drawing shown, when it is a plot.
    fn chart_page(&self) -> Option<(&DrawingPage, &Chart)> {
        let page = self.cad_page()?;
        let sheet = page.prepared.scene.sheets.get(page.sheet)?;
        Some((page, sheet.chart.as_deref()?))
    }

    /// The pointer moved: a plot's readout follows it.
    pub(super) fn chart_motion(&mut self) {
        if !self.gallery_active() && self.chart_page().is_some() {
            self.redraw = true;
        }
    }

    /// What the chart overlay draws, when a plot is shown.
    pub(super) fn chart_view(&self, pal: &Palette) -> Option<ChartView> {
        if self.gallery_active() {
            return None;
        }
        let (page, _) = self.chart_page()?;
        let dark = self.cad_dark_paper(pal);
        view_of(page, &self.view, self.dl.scale, self.pointer, dark)
    }
}

/// What the chart overlay draws for `page` shown in `view` (`scale` physical px per logical
/// px, the pointer in physical px), when the page is a plot.
pub(super) fn view_of(
    page: &DrawingPage,
    view: &View,
    scale: f32,
    pointer: Option<(f32, f32)>,
    dark: bool,
) -> Option<ChartView> {
    let sheet = page.prepared.scene.sheets.get(page.sheet)?;
    let chart = sheet.chart.as_deref()?;
    let s = f64::from(scale.max(0.1));
    let m = view.screen_from_stored_f64();
    let screen = Affine::new([m[0], m[3], m[1], m[4], m[2], m[5]]);
    let to_logical = Affine::scale(1.0 / s) * screen * page.to_stored;
    if to_logical.determinant().abs() < 1e-300 {
        return None;
    }
    let from_logical = to_logical.inverse();
    let f = to_logical.transform_rect_bbox(chart.frame);
    let vp = view.viewport;
    let vp = KRect::new(
        f64::from(vp.x) / s,
        f64::from(vp.y) / s,
        f64::from(vp.x + vp.w) / s,
        f64::from(vp.y + vp.h) / s,
    );
    let vis = f.intersect(vp);
    if vis.width() <= 1.0 || vis.height() <= 1.0 {
        return None;
    }
    // Fractions along the frame of the visible part's edges (x left to right, y bottom
    // to top; the sheet is y up).
    let frac = |p: Point| {
        let q = from_logical * p;
        let fr = chart.frame;
        (
            (q.x - fr.x0) / fr.width().max(1e-300),
            (q.y - fr.y0) / fr.height().max(1e-300),
        )
    };
    let (fx0, fy0) = frac(Point::new(vis.x0, vis.y1));
    let (fx1, fy1) = frac(Point::new(vis.x1, vis.y0));
    let nx = (vis.width() / X_TICK_PX).round() as usize;
    let ny = (vis.height() / Y_TICK_PX).round() as usize;
    let (xt, xdec, xbig) = ticks(&chart.x, fx0, fx1, nx);
    let (yt, ydec, ybig) = ticks(&chart.y, fy0, fy1, ny);
    let at = |fxv: f64, fyv: f64| {
        let fr = chart.frame;
        to_logical * Point::new(fr.x0 + fxv * fr.width(), fr.y0 + fyv * fr.height())
    };
    let x_ticks = xt
        .into_iter()
        .map(|(fr, l)| (at(fr, 0.0).x as f32, l))
        .filter(|(x, _)| f64::from(*x) >= vis.x0 - 0.5 && f64::from(*x) <= vis.x1 + 0.5)
        .collect();
    let y_ticks = yt
        .into_iter()
        .map(|(fr, l)| (at(0.0, fr).y as f32, l))
        .filter(|(y, _)| f64::from(*y) >= vis.y0 - 0.5 && f64::from(*y) <= vis.y1 + 0.5)
        .collect();
    // Peak marks in view, the highest first.
    let mut marks: Vec<_> = chart
        .marks
        .iter()
        .map(|mk| (to_logical * chart.to_sheet(mk.x, mk.y), mk))
        .filter(|(p, _)| vis.contains(*p))
        .collect();
    marks.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
    let marks = marks
        .into_iter()
        .take(MAX_MARKS)
        .map(|(p, mk)| ((p.x as f32, p.y as f32), mk.label.clone()))
        .collect();
    let paper = specular_decode::drawing::paper_colour(dark);
    let layers = &page.prepared.scene.layers;
    let legend = chart
        .legend
        .iter()
        .filter_map(|&i| layers.get(i as usize))
        .map(|l| (shown_name(&l.name, l.generated), l.color.on(dark, paper)))
        .collect();
    let readout = pointer.and_then(|(px, py)| {
        let p = Point::new(f64::from(px) / s, f64::from(py) / s);
        if !vis.contains(p) {
            return None;
        }
        let (x, y) = chart.to_values(from_logical * p);
        let unit = |v: String, u: &str| {
            if u.is_empty() { v } else { format!("{v} {u}") }
        };
        Some(specular_i18n::dot_join(&[
            unit(value_text(x, xdec, xbig), &chart.x.unit),
            unit(value_text(y, ydec, ybig), &chart.y.unit),
        ]))
    });
    let r = |k: KRect| {
        Rect::new(
            k.x0 as f32,
            k.y0 as f32,
            k.width() as f32,
            k.height() as f32,
        )
    };
    Some(ChartView {
        frame: r(vis),
        x_ticks,
        y_ticks,
        x_title: title(&chart.x),
        y_title: title(&chart.y),
        marks,
        legend,
        readout,
        dark,
    })
}

fn srgb(c: [u8; 3], a: f32) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        a,
    )
}

/// Draws the chart overlay inside `area` (the viewport, logical px).
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &ChartView,
    area: (f32, f32, f32, f32),
    ts: f32,
) {
    let (ax, ay, aw, ah) = area;
    let (ink, grid) = if v.dark {
        (Color::hex(0xC9D1D9), Color::rgba(1.0, 1.0, 1.0, 0.10))
    } else {
        (Color::hex(0x33383D), Color::rgba(0.0, 0.0, 0.0, 0.09))
    };
    let f = v.frame;
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    // Grid lines across the plot area.
    dl.push_clip(f);
    for (x, _) in &v.x_ticks {
        dl.line((*x, f.y), (*x, f.bottom()), 1.0, grid);
    }
    for (y, _) in &v.y_ticks {
        dl.line((f.x, *y), (f.right(), *y), 1.0, grid);
    }
    dl.pop_clip();
    // Tick labels below the plot area and left of it, kept on screen.
    let gap = 4.0 * ts;
    let line_h = text.layout("0", st, None, Align::Start).height;
    let label_y = (f.bottom() + gap).min(ay + ah - 2.0 * line_h - gap);
    let tick = 5.0 * ts;
    for (x, l) in &v.x_ticks {
        dl.line((*x, f.bottom()), (*x, f.bottom() - tick), 1.0, ink);
        let b = text.layout(l, st, None, Align::Start);
        text.draw(dl, &b, x - b.width / 2.0, label_y, ink);
    }
    let left = f.x.max(ax + 4.0 * ts);
    for (y, l) in &v.y_ticks {
        dl.line((f.x, *y), (f.x + tick, *y), 1.0, ink);
        let b = text.layout(l, st, None, Align::Start);
        let x = (left - gap - b.width).max(ax + 2.0);
        text.draw(dl, &b, x, y - b.height / 2.0, ink);
    }
    let tst = TextStyle::new(FONT_SMALL * ts).weight(600.0);
    if !v.x_title.is_empty() {
        let b = text.layout(&v.x_title, tst, None, Align::Start);
        let y = label_y + line_h + gap * 0.5;
        text.draw(dl, &b, f.x + (f.w - b.width) / 2.0, y, ink);
    }
    if !v.y_title.is_empty() {
        let b = text.layout(&v.y_title, tst, None, Align::Start);
        let y = (f.y - b.height - gap).max(ay + 2.0);
        text.draw(dl, &b, f.x + gap, y, ink);
    }
    // Peak marks: a tick above the peak and its label.
    for ((x, y), l) in &v.marks {
        let top = y - 6.0 * ts;
        dl.line((*x, y - 2.0 * ts), (*x, top - 8.0 * ts), 1.2, pal.accent);
        if !l.is_empty() {
            let b = text.layout(l, st, None, Align::Start);
            let ly = (top - 8.0 * ts - b.height).max(f.y);
            text.draw(dl, &b, x - b.width / 2.0, ly, ink);
        }
    }
    // Legend and readout at the plot area's top end.
    let mut rows: Vec<(String, Option<[u8; 3]>)> = v
        .legend
        .iter()
        .map(|(n, c)| (n.clone(), Some(*c)))
        .collect();
    if let Some(r) = &v.readout {
        rows.push((r.clone(), None));
    }
    if rows.is_empty() {
        return;
    }
    let blocks: Vec<_> = rows
        .iter()
        .map(|(t, _)| text.layout(t, st, None, Align::Start))
        .collect();
    let sw = 12.0 * ts;
    let w = blocks
        .iter()
        .zip(&rows)
        .map(|(b, (_, c))| b.width + if c.is_some() { sw + 6.0 * ts } else { 0.0 })
        .fold(0f32, f32::max)
        + 2.0 * PAD;
    let h: f32 = blocks.iter().map(|b| b.height).sum::<f32>() + 2.0 * PAD;
    let x = (f.right() - w - 8.0 * ts).max(f.x).min(ax + aw - w - 4.0);
    let y = (f.y + 8.0 * ts).max(ay + 4.0);
    dl.glass(
        Rect::new(x, y, w, h),
        RADIUS_MD,
        pal.glass,
        pal.glass_border,
        1.0,
    );
    let mut ty = y + PAD;
    for (b, (_, c)) in blocks.iter().zip(&rows) {
        let mut tx = x + PAD;
        if let Some(c) = c {
            let cy = ty + b.height / 2.0;
            dl.rect(
                Rect::new(tx, cy - 2.0 * ts, sw, 4.0 * ts),
                2.0,
                srgb(*c, 1.0),
            );
            tx += sw + 6.0 * ts;
        }
        text.draw(dl, b, tx, ty, pal.text);
        ty += b.height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_well() {
        assert_eq!(value_text(1500.0, 0, false), "1500");
        assert_eq!(value_text(-0.25, 2, false), "−0.25");
        assert_eq!(value_text(-0.0001, 2, false), "0");
        assert_eq!(value_text(1.2e7, 0, true), "1.2e7");
        assert_eq!(decimals(0.05), 2);
        assert_eq!(decimals(200.0), 0);
    }

    #[test]
    fn ticks_follow_a_reversed_axis() {
        let a = ChartAxis {
            title: "Wavenumber".into(),
            unit: "cm⁻¹".into(),
            start: 4000.0,
            end: 400.0,
        };
        let (t, _, big) = ticks(&a, 0.0, 1.0, 8);
        assert!(!big);
        assert!(t.len() >= 6, "{t:?}");
        // 4000 is at the left edge.
        assert!(t.iter().any(|(f, l)| l == "4000" && f.abs() < 1e-9));
        assert!(
            t.windows(2).all(|w| w[0].0 > w[1].0),
            "values grow right to left"
        );
        assert_eq!(title(&a), "Wavenumber (cm⁻¹)");
    }
}

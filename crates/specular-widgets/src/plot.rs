//! Plots drawn on the GPU with the chrome's draw list: a dose volume histogram (curves over
//! labelled axes) or an ECG sheet (traces on mm paper). The application lays the plot out on
//! a page in its own units ([`PlotSheet`]: millimetres of paper, or any unit for curves); the
//! sheet scales the page to fit, draws the grid, the cells' traces and labels, a legend and a
//! table, and offers calipers: a drag across a cell reads the interval between its ends (and,
//! for time, the rate) or, for curves, each curve's value under the pointer.
//!
//! An ECG's speed and gain are the application's (it lays the page out again): `+` and `−`
//! ask for the next gain, `]` and `[` for the next speed ([`Command::PlotAdjust`]).

use specular_gfx::Rect;
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, metrics};

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response};

/// The grid behind the cells.
#[derive(Clone, Debug, PartialEq)]
pub enum PlotGrid {
    /// Paper ruled every `minor` page units, heavier every `major` (ECG: 1 and 5 mm).
    Paper { minor: f64, major: f64 },
    /// Lines at each cell's ticks only.
    Ticks,
}

/// What a caliper measures along x.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaliperKind {
    /// Seconds: the interval in ms and the rate it stands for per minute.
    Time,
    /// Values along x (the unit in [`PlotCell::x_unit`]): each curve's y under the pointer.
    Value,
}

/// One trace: a polyline in page units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlotTrace {
    pub points: Vec<[f32; 2]>,
    pub srgb: [u8; 3],
    /// What the trace is (the legend's name; read out under a caliper).
    pub name: String,
}

/// One cell of the page: its rectangle (page units), label, ticks and traces, and how its x
/// and y map to values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlotCell {
    pub rect: [f64; 4],
    pub label: String,
    pub traces: Vec<PlotTrace>,
    /// Ticks: position (page units) and label.
    pub x_ticks: Vec<(f64, String)>,
    pub y_ticks: Vec<(f64, String)>,
    /// Axis titles, under and beside the cell.
    pub x_title: String,
    pub y_title: String,
    /// The value at the cell's left edge and per page unit to the right.
    pub x_value: (f64, f64),
    /// The value at the cell's bottom edge and per page unit up.
    pub y_value: (f64, f64),
    pub x_unit: String,
    pub y_unit: String,
}

impl PlotCell {
    /// The x value at page x `px`.
    pub fn x_at(&self, px: f64) -> f64 {
        self.x_value.0 + (px - self.rect[0]) * self.x_value.1
    }

    /// Each trace's y value at page x `px` (linear between its points), `None` off its ends.
    pub fn traces_at(&self, px: f64) -> Vec<Option<f64>> {
        let bottom = self.rect[1] + self.rect[3];
        self.traces
            .iter()
            .map(|t| {
                let p = &t.points;
                let k = p.windows(2).position(|w| {
                    let (a, b) = (f64::from(w[0][0]), f64::from(w[1][0]));
                    (a <= px && px <= b) || (b <= px && px <= a)
                })?;
                let (a, b) = (p[k], p[k + 1]);
                let (ax, bx) = (f64::from(a[0]), f64::from(b[0]));
                let f = if bx == ax { 0.0 } else { (px - ax) / (bx - ax) };
                let y = f64::from(a[1]) + (f64::from(b[1]) - f64::from(a[1])) * f;
                Some(self.y_value.0 + (bottom - y) * self.y_value.1)
            })
            .collect()
    }
}

/// A plot as the application lays it out.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotSheet {
    pub title: String,
    /// Page size (page units).
    pub page: [f64; 2],
    pub grid: PlotGrid,
    pub cells: Vec<PlotCell>,
    pub caliper: CaliperKind,
    /// Under the title: settings shown ("25 mm/s · 10 mm/mV").
    pub subtitle: String,
    /// Legend entries: name and sRGB colour.
    pub legend: Vec<(String, [u8; 3])>,
    /// A table under the plot: the head row, then rows.
    pub table: Vec<Vec<String>>,
    /// The page is paper (light, whatever the theme), as an ECG is printed.
    pub paper: bool,
    /// `+`, `−`, `[` and `]` ask for another gain and speed.
    pub adjustable: bool,
    /// A footnote (what was left out, tolerances).
    pub note: Option<String>,
}

impl Default for PlotSheet {
    fn default() -> PlotSheet {
        PlotSheet {
            title: String::new(),
            page: [100.0, 100.0],
            grid: PlotGrid::Ticks,
            cells: Vec::new(),
            caliper: CaliperKind::Value,
            subtitle: String::new(),
            legend: Vec::new(),
            table: Vec::new(),
            paper: false,
            adjustable: false,
            note: None,
        }
    }
}

/// Page to screen: screen = origin + page × scale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Fit {
    pub origin: (f32, f32),
    pub scale: f32,
}

impl Fit {
    /// The largest scale that shows `page` whole in `r`, centred.
    pub fn new(page: [f64; 2], r: Rect) -> Fit {
        let s = (r.w / page[0].max(1e-9) as f32).min(r.h / page[1].max(1e-9) as f32);
        Fit {
            origin: (
                r.x + (r.w - page[0] as f32 * s) * 0.5,
                r.y + (r.h - page[1] as f32 * s) * 0.5,
            ),
            scale: s,
        }
    }

    pub fn to_screen(self, p: [f64; 2]) -> (f32, f32) {
        (
            self.origin.0 + p[0] as f32 * self.scale,
            self.origin.1 + p[1] as f32 * self.scale,
        )
    }

    pub fn to_page(self, p: (f32, f32)) -> [f64; 2] {
        [
            f64::from((p.0 - self.origin.0) / self.scale),
            f64::from((p.1 - self.origin.1) / self.scale),
        ]
    }

    pub fn rect(&self, r: [f64; 4]) -> Rect {
        let (x, y) = self.to_screen([r[0], r[1]]);
        Rect::new(x, y, r[2] as f32 * self.scale, r[3] as f32 * self.scale)
    }
}

/// A polyline's screen points with runs inside one pixel column folded to their first, lowest,
/// highest and last point (what a pixel shows of them), so a long record draws in a few
/// segments per pixel.
pub(crate) fn decimate(points: impl Iterator<Item = (f32, f32)>) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::new();
    let mut run: Vec<(f32, f32)> = Vec::new();
    let flush = |run: &mut Vec<(f32, f32)>, out: &mut Vec<(f32, f32)>| {
        if run.len() <= 4 {
            out.append(run);
            return;
        }
        let first = run[0];
        let last = *run.last().expect("non-empty");
        let lo = run
            .iter()
            .copied()
            .fold(first, |m, p| if p.1 < m.1 { p } else { m });
        let hi = run
            .iter()
            .copied()
            .fold(first, |m, p| if p.1 > m.1 { p } else { m });
        let (a, b) = if lo.0 <= hi.0 { (lo, hi) } else { (hi, lo) };
        out.extend([first, a, b, last]);
        run.clear();
    };
    for p in points {
        if let Some(r0) = run.first()
            && r0.0.floor() != p.0.floor()
        {
            flush(&mut run, &mut out);
        }
        run.push(p);
    }
    flush(&mut run, &mut out);
    out
}

/// The caliper's readout: the interval between page x `a` and `b` in `cell`.
pub(crate) fn caliper_text(kind: CaliperKind, cell: &PlotCell, a: f64, b: f64) -> String {
    let (va, vb) = (cell.x_at(a), cell.x_at(b));
    match kind {
        CaliperKind::Time => {
            let dt = (vb - va).abs();
            let ms = (dt * 1000.0).round() as i64;
            if dt > 0.0 {
                t!(
                    "plot-caliper-time",
                    ms = ms,
                    rate = specular_i18n::fixed(60.0 / dt, 0)
                )
            } else {
                t!("plot-caliper-time-zero")
            }
        }
        CaliperKind::Value => {
            let mut parts = vec![format!("{} {}", fmt_value(vb), cell.x_unit)];
            for (t, v) in cell.traces.iter().zip(cell.traces_at(b)) {
                if let Some(v) = v {
                    parts.push(format!("{} {} {}", t.name, fmt_value(v), cell.y_unit));
                }
            }
            specular_i18n::dot_join(&parts)
        }
    }
}

fn fmt_value(v: f64) -> String {
    if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Evenly spaced round ticks covering `lo..=hi`, about `n` of them (1, 2 or 5 × 10ⁿ apart).
pub fn nice_ticks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if hi.is_nan() || lo.is_nan() || hi <= lo || n == 0 {
        return vec![lo];
    }
    let raw = (hi - lo) / n as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let first = (lo / step).ceil() as i64;
    let last = (hi / step + 1e-9).floor() as i64;
    (first..=last).map(|i| i as f64 * step).collect()
}

/// The plot sheet's state.
#[derive(Default)]
pub(crate) struct PlotState {
    pub sheet: Option<PlotSheet>,
    /// The page's fit on screen at the last draw.
    pub fit: Fit,
    /// The caliper: cell, page x of its ends.
    pub caliper: Option<(usize, f64, f64)>,
    pub dragging: bool,
}

impl PlotState {
    /// The cell under screen point `p`, and the page point there.
    fn cell_at(&self, p: (f32, f32)) -> Option<(usize, [f64; 2])> {
        let sheet = self.sheet.as_ref()?;
        let q = self.fit.to_page(p);
        sheet
            .cells
            .iter()
            .position(|c| {
                q[0] >= c.rect[0]
                    && q[0] <= c.rect[0] + c.rect[2]
                    && q[1] >= c.rect[1]
                    && q[1] <= c.rect[1] + c.rect[3]
            })
            .map(|i| (i, q))
    }
}

impl Chrome {
    /// Opens the plot sheet with this content (no caliper).
    pub fn show_plot(&mut self, sheet: PlotSheet) {
        self.plot = PlotState {
            sheet: Some(sheet),
            ..PlotState::default()
        };
        self.open_modal(Modal::Plot);
    }

    /// Replaces the open plot's content (another gain or speed); the caliper stays when the
    /// page keeps its cells. Ignored when it is closed.
    pub fn set_plot(&mut self, sheet: PlotSheet) {
        if !self.plot_open() {
            return;
        }
        let keep = self
            .plot
            .sheet
            .as_ref()
            .is_some_and(|s| s.cells.len() == sheet.cells.len() && s.page == sheet.page);
        if !keep {
            self.plot.caliper = None;
        }
        self.plot.sheet = Some(sheet);
    }

    pub fn plot_open(&self) -> bool {
        self.modal == Some(Modal::Plot)
    }

    /// The caliper's ends (cell, page x), for tests and the application.
    pub fn plot_caliper(&self) -> Option<(usize, f64, f64)> {
        self.plot.caliper
    }

    /// A press on the plot: a caliper starts there.
    pub(crate) fn plot_press(&mut self, p: (f32, f32)) -> bool {
        if !self.plot_open() {
            return false;
        }
        match self.plot.cell_at(p) {
            Some((i, q)) => {
                self.plot.caliper = Some((i, q[0], q[0]));
                self.plot.dragging = true;
                true
            }
            None => false,
        }
    }

    /// The pointer moved: a caliper being drawn follows it (inside its cell).
    pub(crate) fn plot_drag(&mut self, p: (f32, f32)) -> bool {
        if !self.plot.dragging {
            return false;
        }
        let Some((i, a, _)) = self.plot.caliper else {
            return false;
        };
        let Some(cell) = self.plot.sheet.as_ref().and_then(|s| s.cells.get(i)) else {
            return false;
        };
        let x = self.plot.fit.to_page(p)[0].clamp(cell.rect[0], cell.rect[0] + cell.rect[2]);
        self.plot.caliper = Some((i, a, x));
        true
    }

    pub(crate) fn plot_release(&mut self) -> bool {
        std::mem::take(&mut self.plot.dragging)
    }

    /// Keys on the plot: gain and speed, Delete clears the caliper.
    pub(crate) fn plot_key(&mut self, combo: &KeyCombo, repeat: bool) -> Response {
        let adjustable = self.plot.sheet.as_ref().is_some_and(|s| s.adjustable);
        let adjust = |gain: i32, speed: i32| {
            if repeat {
                Response::used()
            } else {
                Response::cmd(Command::PlotAdjust { gain, speed })
            }
        };
        match combo.key {
            Key::Char('+' | '=') if adjustable => adjust(1, 0),
            Key::Char('-' | '−') if adjustable => adjust(-1, 0),
            Key::Char(']') if adjustable => adjust(0, 1),
            Key::Char('[') if adjustable => adjust(0, -1),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                self.plot.caliper = None;
                Response::used()
            }
            _ => Response::used(),
        }
    }
}

/// The screen-reader text of the plot: title, settings, legend, caliper and table.
pub(crate) fn describe(s: &PlotState) -> Vec<String> {
    let Some(sheet) = &s.sheet else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if !sheet.subtitle.is_empty() {
        out.push(sheet.subtitle.clone());
    }
    if let Some((i, a, b)) = s.caliper
        && let Some(c) = sheet.cells.get(i)
    {
        out.push(caliper_text(sheet.caliper, c, a, b));
    }
    for row in &sheet.table {
        out.push(row.join(", "));
    }
    out
}

fn srgb(c: [u8; 3]) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    )
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let Some(sheet) = ch.plot.sheet.clone() else {
        return;
    };
    let w = (cx.w - 48.0).min(cx.u(1200.0));
    let h = (cx.h - 64.0).max(200.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let inner = Rect::new(r.x + 20.0, r.y, r.w - 40.0, r.h);

    // Title, settings and Close.
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 12.0, 32.0, 32.0), r);
    cx.close_button(close);
    let tr = cx.m_in(
        Rect::new(inner.x, r.y + 12.0, inner.w - 48.0, cx.u(32.0).max(32.0)),
        r,
    );
    cx.label_fit(&sheet.title, title_s, tr, HAlign::Start, cx.pal.text, true);
    let small = cx.style(metrics::FONT_SMALL);
    let mut y = tr.bottom() + 2.0;
    let line_h = cx.u(22.0);
    let mut sub = sheet.subtitle.clone();
    if sheet.adjustable {
        sub = specular_i18n::dot_join(&[sub, t!("plot-adjust-hint")]);
    }
    if !sub.is_empty() {
        let sr = cx.m_in(Rect::new(inner.x, y, inner.w, line_h), r);
        cx.label_fit(&sub, small, sr, HAlign::Start, cx.pal.text_muted, false);
        y += line_h;
    }

    // Under the plot: caliper readout, legend, table, note.
    let mut below: Vec<(String, Option<[u8; 3]>)> = Vec::new();
    let caliper = ch.plot.caliper.and_then(|(i, a, b)| {
        let c = sheet.cells.get(i)?;
        Some(caliper_text(sheet.caliper, c, a, b))
    });
    below.push((caliper.unwrap_or_else(|| t!("plot-caliper-hint")), None));
    let rows = sheet.table.len() + usize::from(!sheet.legend.is_empty()) + 1;
    let table_h = rows as f32 * line_h + sheet.note.is_some() as u8 as f32 * line_h;
    let plot_r = Rect::new(
        inner.x,
        y + 6.0,
        inner.w,
        (r.bottom() - 16.0 - table_h - (y + 6.0)).max(80.0),
    );

    // The page.
    let fit = Fit::new(sheet.page, plot_r);
    ch.plot.fit = fit;
    let page_r = fit.rect([0.0, 0.0, sheet.page[0], sheet.page[1]]);
    let (ink, minor_c, major_c, axis_c) = if sheet.paper {
        (
            Color::hex(0x101010),
            Color::hex(0xF6C8C8),
            Color::hex(0xE58A8A),
            Color::hex(0x404040),
        )
    } else {
        (
            cx.pal.text,
            cx.pal.separator.fade(0.5),
            cx.pal.separator,
            cx.pal.text_muted,
        )
    };
    if sheet.paper {
        cx.dl.rect(page_r, 0.0, Color::hex(0xFFF8F5));
    }
    cx.dl.push_clip(page_r.inset(-1.0));
    if let PlotGrid::Paper { minor, major } = sheet.grid {
        let px = fit.scale * minor as f32;
        // Minor lines only when they are at least 3 px apart.
        for (step, colour, width) in [(minor, minor_c, 0.6f32), (major, major_c, 1.0)] {
            if step <= 0.0 || (step == minor && px < 3.0) {
                continue;
            }
            let nx = (sheet.page[0] / step).floor() as i64;
            let ny = (sheet.page[1] / step).floor() as i64;
            for i in 0..=nx {
                let (x, _) = fit.to_screen([i as f64 * step, 0.0]);
                cx.dl
                    .line((x, page_r.y), (x, page_r.bottom()), width, colour);
            }
            for j in 0..=ny {
                let (_, y) = fit.to_screen([0.0, j as f64 * step]);
                cx.dl
                    .line((page_r.x, y), (page_r.right(), y), width, colour);
            }
        }
    }
    let label_s = small.weight(600.0);
    for (ci, cell) in sheet.cells.iter().enumerate() {
        let cr = fit.rect(cell.rect);
        if sheet.grid == PlotGrid::Ticks {
            for (p, _) in &cell.x_ticks {
                let (x, _) = fit.to_screen([*p, 0.0]);
                cx.dl.line((x, cr.y), (x, cr.bottom()), 1.0, major_c);
            }
            for (p, _) in &cell.y_ticks {
                let (_, y) = fit.to_screen([0.0, *p]);
                cx.dl.line((cr.x, y), (cr.right(), y), 1.0, major_c);
            }
            cx.dl
                .line((cr.x, cr.bottom()), (cr.right(), cr.bottom()), 1.2, axis_c);
            cx.dl.line((cr.x, cr.y), (cr.x, cr.bottom()), 1.2, axis_c);
        }
        cx.dl.push_clip(cr.inset(-1.0));
        for t in &cell.traces {
            let pts = decimate(
                t.points
                    .iter()
                    .map(|p| fit.to_screen([f64::from(p[0]), f64::from(p[1])])),
            );
            let colour = if sheet.paper { ink } else { srgb(t.srgb) };
            let width = if sheet.paper { 1.1 } else { 1.8 };
            for w in pts.windows(2) {
                cx.dl.line(w[0], w[1], width, colour);
            }
        }
        if let Some((i, a, b)) = ch.plot.caliper
            && i == ci
        {
            let (xa, _) = fit.to_screen([a, 0.0]);
            let (xb, _) = fit.to_screen([b, 0.0]);
            let lo = xa.min(xb);
            cx.dl.rect(
                Rect::new(lo, cr.y, (xa - xb).abs(), cr.h),
                0.0,
                cx.pal.accent.with_alpha(0.12),
            );
            for x in [xa, xb] {
                cx.dl.line((x, cr.y), (x, cr.bottom()), 1.5, cx.pal.accent);
            }
        }
        cx.dl.pop_clip();
        if !cell.label.is_empty() {
            let lr = Rect::new(cr.x + 4.0, cr.y + 2.0, cr.w - 8.0, line_h);
            cx.label_fit(&cell.label, label_s, lr, HAlign::Start, ink, false);
        }
    }
    cx.dl.pop_clip();
    // Tick labels and axis titles outside the cells.
    for cell in &sheet.cells {
        let cr = fit.rect(cell.rect);
        for (p, l) in &cell.x_ticks {
            let (x, _) = fit.to_screen([*p, 0.0]);
            let lr = Rect::new(x - 40.0, cr.bottom() + 2.0, 80.0, line_h);
            cx.label_fit(l, small, lr, HAlign::Center, cx.pal.text_muted, false);
        }
        for (p, l) in &cell.y_ticks {
            let (_, y) = fit.to_screen([0.0, *p]);
            let lr = Rect::new(cr.x - 52.0, y - line_h * 0.5, 46.0, line_h);
            cx.label_fit(l, small, lr, HAlign::End, cx.pal.text_muted, false);
        }
        if !cell.x_title.is_empty() {
            let lr = Rect::new(cr.x, cr.bottom() + line_h, cr.w, line_h);
            cx.label_fit(&cell.x_title, small, lr, HAlign::Center, cx.pal.text, false);
        }
        if !cell.y_title.is_empty() {
            let lr = Rect::new(cr.x + 6.0, cr.y - line_h, cr.w * 0.5, line_h);
            cx.label_fit(&cell.y_title, small, lr, HAlign::Start, cx.pal.text, false);
        }
    }
    cx.hit(page_r, Target::PlotArea);

    // Readout, legend, table, note.
    let mut ty = plot_r.bottom() + 8.0;
    for (text, _) in &below {
        let lr = cx.m_in(Rect::new(inner.x, ty, inner.w, line_h), r);
        let colour = if ch.plot.caliper.is_some() {
            cx.pal.text
        } else {
            cx.pal.text_faint
        };
        cx.label_fit(text, small.tabular(), lr, HAlign::Start, colour, false);
        ty += line_h;
    }
    if !sheet.legend.is_empty() {
        let mut x = inner.x;
        for (name, c) in &sheet.legend {
            let tw = cx.text.measure(name, small);
            if x + 18.0 + tw > inner.right() {
                break;
            }
            let sw = cx.m_in(Rect::new(x, ty + line_h * 0.5 - 2.0, 12.0, 4.0), r);
            cx.dl.rect(sw, 2.0, srgb(*c));
            let lr = cx.m_in(Rect::new(x + 16.0, ty, tw + 2.0, line_h), r);
            cx.label(name, small, lr, HAlign::Start, cx.pal.text);
            x += 16.0 + tw + 18.0;
        }
        ty += line_h;
    }
    if let Some(head) = sheet.table.first() {
        let n = head.len().max(1);
        let first_w = (inner.w * 0.34).min(cx.u(260.0));
        let col_w = (inner.w - first_w) / (n.saturating_sub(1)).max(1) as f32;
        for (ri, row) in sheet.table.iter().enumerate() {
            let s = if ri == 0 {
                small.weight(600.0)
            } else {
                small.tabular()
            };
            let colour = if ri == 0 {
                cx.pal.text_muted
            } else {
                cx.pal.text
            };
            for (k, cell) in row.iter().enumerate().take(n) {
                let (x, cw) = if k == 0 {
                    (inner.x, first_w)
                } else {
                    (inner.x + first_w + (k - 1) as f32 * col_w, col_w)
                };
                let align = if k == 0 { HAlign::Start } else { HAlign::End };
                let lr = cx.m_in(Rect::new(x, ty, cw - 8.0, line_h), r);
                cx.label_fit(cell, s, lr, align, colour, false);
            }
            ty += line_h;
        }
    }
    if let Some(n) = &sheet.note {
        let lr = cx.m_in(Rect::new(inner.x, ty, inner.w, line_h), r);
        cx.label_fit(n, small, lr, HAlign::Start, cx.pal.text_faint, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell() -> PlotCell {
        PlotCell {
            rect: [10.0, 0.0, 100.0, 50.0],
            traces: vec![PlotTrace {
                points: vec![[10.0, 0.0], [60.0, 25.0], [110.0, 50.0]],
                srgb: [255, 0, 0],
                name: "PTV".into(),
            }],
            // 0.1 Gy per page unit; 2 % per unit up from 0 at the bottom.
            x_value: (0.0, 0.1),
            y_value: (0.0, 2.0),
            x_unit: "Gy".into(),
            y_unit: "%".into(),
            ..PlotCell::default()
        }
    }

    #[test]
    fn values_under_the_caliper() {
        let c = cell();
        assert!((c.x_at(60.0) - 5.0).abs() < 1e-12);
        let v = c.traces_at(35.0);
        // Halfway along the first segment: page y 12.5, 37.5 above the bottom: 75 %.
        assert!((v[0].unwrap() - 75.0).abs() < 1e-9);
        assert_eq!(c.traces_at(200.0), vec![None]);
    }

    #[test]
    fn fit_keeps_the_page_whole_and_round_trips() {
        let f = Fit::new([280.0, 120.0], Rect::new(0.0, 0.0, 560.0, 400.0));
        assert_eq!(f.scale, 2.0);
        assert_eq!(f.origin, (0.0, 80.0));
        let q = f.to_page(f.to_screen([25.0, 40.0]));
        assert!((q[0] - 25.0).abs() < 1e-4 && (q[1] - 40.0).abs() < 1e-4);
    }

    #[test]
    fn decimation_keeps_each_columns_extremes() {
        // 1000 points over 10 px: a spike in column 3 survives.
        let pts: Vec<(f32, f32)> = (0..1000)
            .map(|i| {
                let x = i as f32 / 100.0;
                (x, if i == 345 { -50.0 } else { (i % 7) as f32 })
            })
            .collect();
        let d = decimate(pts.iter().copied());
        assert!(d.len() <= 40, "{}", d.len());
        assert!(d.contains(&(3.45, -50.0)));
        assert_eq!(d.first(), pts.first());
        assert_eq!(d.last(), pts.last());
    }

    #[test]
    fn ticks_are_round() {
        assert_eq!(
            nice_ticks(0.0, 70.0, 7),
            vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0]
        );
        assert_eq!(nice_ticks(0.0, 1.0, 4), vec![0.0, 0.5, 1.0]);
        assert_eq!(nice_ticks(5.0, 5.0, 4), vec![5.0]);
    }
}

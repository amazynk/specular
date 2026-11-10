//! Thermal sequences (FLIR `.seq` and `.csq`) in science mode.
//!
//! - The frames are a frame axis: its slider is the timeline, the plane keys step it, and
//!   [`Action::ScienceThermalPlay`] plays it at the recorded frame times (each frame waits for
//!   its temperatures, so a slow frame holds the clock rather than being skipped).
//! - The readout and the measurement tool read the frame shown.
//! - A panel at the bottom end corner holds the play button, where the playback is, and a
//!   chart of every spot, line and area over time: the spot's temperature, the mean of a line
//!   or an area, with a cursor at the frame shown. The series are read on a background
//!   thread over every frame, again when a measurement, the unit or the object parameters
//!   change. [`Action::ScienceThermalSeriesCsv`] saves them as CSV (time, then each
//!   measurement's mean, minimum and maximum).
//!
//! [`Action::ScienceThermalPlay`]: specular_input::Action::ScienceThermalPlay
//! [`Action::ScienceThermalSeriesCsv`]: specular_input::Action::ScienceThermalSeriesCsv

#[cfg(test)]
mod shots;

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crossbeam_channel::Receiver;
use specular_decode::science::{
    AxisKind, NdPlane, TemperatureUnit, ThermalConditions, ThermalImage,
};
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_measure::geom::Shape;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::super::App;
use super::{A11Y_ID, trimmed};
use crate::loader::ImageId;

/// AccessKit ids of the panel and its buttons (in science mode's range).
const A11Y_PANEL: u64 = A11Y_ID + 0x10;
const A11Y_PLAY: u64 = A11Y_ID + 0x11;
const A11Y_CSV: u64 = A11Y_ID + 0x12;
const A11Y_CHART: u64 = A11Y_ID + 0x13;

/// Shortest time a frame shows while playing, seconds (a file with coinciding times).
const MIN_FRAME_S: f64 = 1.0 / 120.0;

/// Line colours of the measurements, by number (bright on the dark chart and on light
/// themes' panels alike).
pub(super) const INKS: [Color; 6] = [
    Color::rgba(0.30, 0.62, 1.0, 1.0),
    Color::rgba(0.98, 0.55, 0.15, 1.0),
    Color::rgba(0.35, 0.80, 0.40, 1.0),
    Color::rgba(0.90, 0.35, 0.75, 1.0),
    Color::rgba(0.95, 0.80, 0.20, 1.0),
    Color::rgba(0.40, 0.85, 0.90, 1.0),
];

/// The ink of measurement `id`.
pub(super) fn ink(id: u32) -> Color {
    INKS[(id.max(1) as usize - 1) % INKS.len()]
}

/// One measurement over time: per frame its mean, minimum and maximum (NaN where the frame
/// has no value there).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Line {
    pub id: u32,
    pub name: String,
    pub mean: Vec<f64>,
    pub min: Vec<f64>,
    pub max: Vec<f64>,
}

/// What a series was read for: the image, the measurements and the temperatures' terms.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    id: ImageId,
    shapes: Vec<(u32, Shape)>,
    conditions: [Option<u64>; 7],
    unit: TemperatureUnit,
}

fn condition_bits(c: &ThermalConditions) -> [Option<u64>; 7] {
    [
        c.emissivity,
        c.distance,
        c.reflected,
        c.atmospheric,
        c.window_temperature,
        c.window_transmission,
        c.humidity,
    ]
    .map(|v| v.map(f64::to_bits))
}

/// Every measurement over every frame.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Series {
    /// Seconds from the first frame.
    pub times: Vec<f64>,
    pub unit: TemperatureUnit,
    pub lines: Vec<Line>,
}

/// A series being read; dropping it stops the reading.
struct Job {
    key: Key,
    rx: Receiver<Result<Series, String>>,
    stop: Arc<AtomicBool>,
    done: Arc<AtomicU32>,
    total: u32,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Playback and the series of the current image's sequence.
#[derive(Default)]
pub(in crate::app) struct SeqState {
    pub playing: bool,
    /// When the next frame is due (the app's clock, seconds).
    next: f64,
    series: Option<(Key, Arc<Series>)>,
    job: Option<Job>,
    /// Why the series could not be read.
    failed: Option<(Key, String)>,
    /// The panel as last drawn.
    pub drawn: Drawn,
    saved: Option<Receiver<Result<String, String>>>,
}

/// The panel's controls as drawn (logical px).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::app) struct Drawn {
    pub panel: Option<Rect>,
    pub play: Option<Rect>,
    pub csv: Option<Rect>,
    pub chart: Option<Rect>,
}

/// What the panel shows.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Panel {
    pub playing: bool,
    /// "Frame 3 of 7 · 0.12 s".
    pub position: String,
    /// Seconds of the frame shown, for the chart's cursor.
    pub at: f64,
    pub series: Option<Arc<Series>>,
    /// Instead of the chart: why there is none, or how far the reading is.
    pub note: Option<String>,
}

/// The series of `lines` as CSV: a row per frame (its number and time in seconds), then each
/// measurement's mean, minimum and maximum in `unit`.
pub(super) fn csv(s: &Series) -> String {
    let unit = s.unit.symbol();
    let mut out = String::from("frame,time_s");
    for l in &s.lines {
        for what in ["mean", "min", "max"] {
            let _ = write!(out, ",{} {} {what} ({unit})", l.id, l.name);
        }
    }
    out.push('\n');
    let cell = |v: f64| {
        if v.is_finite() {
            format!("{v:.3}")
        } else {
            String::new()
        }
    };
    for (k, t) in s.times.iter().enumerate() {
        let _ = write!(out, "{},{t:.3}", k + 1);
        for l in &s.lines {
            let _ = write!(
                out,
                ",{},{},{}",
                cell(l.mean[k]),
                cell(l.min[k]),
                cell(l.max[k])
            );
        }
        out.push('\n');
    }
    out
}

/// Reads every frame of `t` and each shape's statistics in it.
fn read_series(
    t: &ThermalImage,
    shapes: &[(u32, String, Shape)],
    c: &ThermalConditions,
    unit: TemperatureUnit,
    stop: &AtomicBool,
    done: &AtomicU32,
) -> Result<Series, String> {
    let n = t.frame_count();
    let (w, h) = (t.thermal().width as u32, t.thermal().height as u32);
    let mut lines: Vec<Line> = shapes
        .iter()
        .map(|(id, name, _)| Line {
            id: *id,
            name: name.clone(),
            mean: Vec::with_capacity(n as usize),
            min: Vec::with_capacity(n as usize),
            max: Vec::with_capacity(n as usize),
        })
        .collect();
    let mut times = Vec::with_capacity(n as usize);
    for k in 0..n {
        if stop.load(Ordering::Relaxed) {
            return Err(String::new());
        }
        let values = t
            .frame_values(k, c, unit)
            .map_err(|e| crate::error_text::decode_reason(&e))?;
        let plane = NdPlane {
            width: w,
            height: h,
            values,
            truncated: false,
        };
        times.push(t.frame_seconds(k));
        for (l, (_, _, s)) in lines.iter_mut().zip(shapes) {
            let st = super::super::measure_tool::value_stats(s, &plane, unit.symbol())
                .and_then(|(st, _)| st.into_iter().next());
            let (mean, min, max) =
                st.map_or((f64::NAN, f64::NAN, f64::NAN), |s| (s.mean, s.min, s.max));
            l.mean.push(mean);
            l.min.push(min);
            l.max.push(max);
        }
        done.store(k + 1, Ordering::Relaxed);
    }
    Ok(Series { times, unit, lines })
}

impl App {
    /// The current image's thermal sequence, when science mode shows one.
    pub(super) fn thermal_seq(&self) -> Option<Arc<ThermalImage>> {
        if !self.science_shown() {
            return None;
        }
        let s = self.science.summary.as_ref()?;
        let t = s.thermal.as_ref()?;
        (s.axes.first()?.kind == AxisKind::Frame && t.frame_count() > 1).then(|| t.clone())
    }

    /// The frame shown.
    fn thermal_seq_frame(&self) -> u32 {
        self.science.index.first().copied().unwrap_or(0)
    }

    /// Plays or pauses the sequence.
    pub(super) fn thermal_seq_play(&mut self) {
        if self.thermal_seq().is_none() {
            return;
        }
        let now = self.now();
        let st = &mut self.science.seq;
        st.playing = !st.playing;
        st.next = now;
        let msg = if self.science.seq.playing {
            t!("thermal-seq-playing")
        } else {
            t!("thermal-seq-paused")
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// Each frame: the next frame when it is due, the series read when the measurements
    /// changed, a finished CSV save reported.
    pub(in crate::app) fn thermal_seq_sync(&mut self) {
        self.thermal_seq_saved();
        let Some(t) = self.thermal_seq() else {
            self.science.seq.playing = false;
            self.science.seq.job = None;
            return;
        };
        let now = self.now();
        if self.science.seq.playing {
            if now >= self.science.seq.next && self.science_settled() {
                let n = t.frame_count();
                let k = (self.thermal_seq_frame() + 1) % n;
                let dt = t.frame_duration(self.thermal_seq_frame()).max(MIN_FRAME_S);
                self.science.seq.next = now + dt;
                self.science_step(0, Some(k), 0);
            }
            let due = (self.science.seq.next - now).clamp(0.0, 2.0);
            self.wake_by(
                std::time::Instant::now() + std::time::Duration::from_secs_f64(due),
                crate::app::Cause::Science,
            );
        }
        self.thermal_seq_series(&t);
    }

    /// Starts reading the series when what it is read for changed; takes a finished one.
    fn thermal_seq_series(&mut self, t: &Arc<ThermalImage>) {
        let Some(id) = self.science.id else { return };
        let regions = self.measure_regions();
        let shapes: Vec<(u32, String, Shape)> =
            regions.into_iter().map(|(i, n, s, _)| (i, n, s)).collect();
        let Some(c) = self.science.conditions_now() else {
            return;
        };
        let unit = self.science.unit_now();
        let key = Key {
            id,
            shapes: shapes.iter().map(|(i, _, s)| (*i, s.clone())).collect(),
            conditions: condition_bits(&c),
            unit,
        };
        let st = &mut self.science.seq;
        if let Some(job) = &st.job {
            match job.rx.try_recv() {
                Ok(r) => {
                    let job = st.job.take().expect("job");
                    match r {
                        Ok(s) => st.series = Some((job.key.clone(), Arc::new(s))),
                        Err(e) if !e.is_empty() => st.failed = Some((job.key.clone(), e)),
                        Err(_) => {}
                    }
                    self.redraw = true;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => st.job = None,
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // Progress shows while it reads.
                    self.redraw = true;
                }
            }
        }
        let st = &mut self.science.seq;
        let have = st.series.as_ref().is_some_and(|(k, _)| *k == key)
            || st.job.as_ref().is_some_and(|j| j.key == key)
            || st.failed.as_ref().is_some_and(|(k, _)| *k == key);
        if have || shapes.is_empty() {
            if shapes.is_empty() {
                st.job = None;
                st.series = None;
            }
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicU32::new(0));
        st.job = Some(Job {
            key,
            rx,
            stop: stop.clone(),
            done: done.clone(),
            total: t.frame_count(),
        });
        let t = t.clone();
        self.loader.background("thermal-series", move |l| {
            let t0 = std::time::Instant::now();
            let r = read_series(&t, &shapes, &c, unit, &stop, &done);
            specular_log::debug!(target: specular_log::UI, "thermal series: {} frames, {} measurements in {:.1} ms", t.frame_count(), shapes.len(), t0.elapsed().as_secs_f64() * 1e3);
            let _ = tx.send(r);
            l.waker()();
        });
    }

    /// The panel for the sequence shown.
    pub(in crate::app) fn thermal_seq_panel(&self) -> Option<Panel> {
        let t = self.thermal_seq()?;
        let k = self.thermal_seq_frame();
        let at = t.frame_seconds(k);
        let st = &self.science.seq;
        let position = t!(
            "thermal-seq-frame",
            frame = i64::from(k) + 1,
            frames = i64::from(t.frame_count()),
            time = format!("{} s", trimmed(at, 3))
        );
        let series = st.series.as_ref().map(|(_, s)| s.clone());
        let note = if let Some(j) = &st.job {
            Some(t!(
                "thermal-seq-reading",
                done = i64::from(j.done.load(Ordering::Relaxed)),
                total = i64::from(j.total)
            ))
        } else if let Some((_, e)) = &st.failed {
            Some(t!("thermal-seq-failed", reason = e.clone()))
        } else if series.is_none() {
            Some(t!("thermal-seq-hint"))
        } else {
            None
        };
        Some(Panel {
            playing: st.playing,
            position,
            at,
            series: if st.job.is_some() { None } else { series },
            note,
        })
    }

    /// A press on the panel's buttons; true when taken.
    pub(in crate::app) fn thermal_seq_press(&mut self, lp: (f32, f32)) -> bool {
        let d = self.science.seq.drawn;
        if d.play.is_some_and(|r| r.contains(lp)) {
            self.thermal_seq_play();
            return true;
        }
        if d.csv.is_some_and(|r| r.contains(lp)) {
            self.thermal_seq_csv();
            return true;
        }
        d.panel.is_some_and(|r| r.contains(lp)) && self.thermal_seq().is_some()
    }

    /// Asks where to save the series as CSV and writes it on the dialog's thread.
    pub(super) fn thermal_seq_csv(&mut self) {
        if self.thermal_seq().is_none() {
            return;
        }
        let now = self.now();
        let Some((_, s)) = self.science.seq.series.clone() else {
            self.chrome.toast(t!("thermal-seq-no-series"), None, now);
            return;
        };
        let stem = self
            .science
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or_else(
                || "temperatures".to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        let text = csv(&s);
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-therm-seq", move |l| {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .set_file_name(format!("{stem}-over-time.csv"))
                .save_file()
            else {
                return;
            };
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let r = std::fs::write(&path, text)
                .map(|()| name)
                .map_err(|e| crate::error_text::io_reason(&e));
            let _ = tx.send(r);
            l.waker()();
        });
        self.science.seq.saved = Some(rx);
    }

    fn thermal_seq_saved(&mut self) {
        let Some(Ok(r)) = self.science.seq.saved.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.science.seq.saved = None;
        let msg = match r {
            Ok(name) => t!("science-thermal-export-done", name = name),
            Err(e) => t!("science-thermal-export-failed", reason = e),
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// A screen reader's press on the panel's buttons; true when the node is the panel's.
    pub(in crate::app) fn thermal_seq_a11y_request(
        &mut self,
        req: &accesskit::ActionRequest,
    ) -> bool {
        match req.target_node.0 {
            A11Y_PLAY if req.action == accesskit::Action::Click => self.thermal_seq_play(),
            A11Y_CSV if req.action == accesskit::Action::Click => self.thermal_seq_csv(),
            A11Y_PANEL..=A11Y_CHART => {}
            _ => return false,
        }
        true
    }
}

/// The panel's height at text scale `ts`.
fn panel_size(ts: f32, aw: f32, ah: f32) -> (f32, f32) {
    let m = TOOLBAR_MARGIN;
    (
        (340.0 * ts).min(aw - 2.0 * m).max(160.0),
        (190.0 * ts).min(ah / 2.0 - 2.0 * m).max(90.0),
    )
}

/// Draws the panel at the bottom end corner of `area` (logical), above `below` (the readout).
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &Panel,
    area: (f32, f32, f32, f32),
    below: Option<Rect>,
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let (w, h) = panel_size(ts, aw, ah);
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let bottom = below.map_or(ay + ah - m, |b| b.y - m);
    let y = (bottom - h).max(ay + m);
    let r = Rect::new(x, y, w, h);
    dl.rect_border(
        r,
        RADIUS_MD,
        pal.surface.with_alpha(0.92),
        pal.separator,
        1.0,
    );
    let pad = PAD * ts * 0.75;
    let small = TextStyle::new(FONT_SMALL * ts).tabular();
    let bold = TextStyle::new(FONT_SMALL * ts).weight(600.0);
    // The top row: play, where the playback is, and Save CSV at the end.
    let btn = 22.0 * ts;
    let play = Rect::new(
        if rtl { x + w - pad - btn } else { x + pad },
        y + pad,
        btn,
        btn,
    );
    dl.rect(play, btn / 2.0, pal.accent_fill);
    let icon = if p.playing { Icon::Pause } else { Icon::Play };
    // Media controls keep their direction in right-to-left languages.
    draw_icon(
        dl,
        icon,
        play.inset(5.0 * ts),
        pal.accent_text,
        1.6 * ts,
        false,
    );
    let csv_l = text.layout(&t!("thermal-seq-save-csv"), small, None, Align::Start);
    let csv_w = csv_l.width + 2.0 * pad;
    let csv = p.series.as_ref().map(|_| {
        let cr = Rect::new(
            if rtl { x + pad } else { x + w - pad - csv_w },
            y + pad,
            csv_w,
            btn,
        );
        dl.rect_border(cr, RADIUS_SM, pal.surface, pal.separator, 1.0);
        text.draw(
            dl,
            &csv_l,
            cr.x + pad,
            cr.y + (btn - csv_l.height) / 2.0,
            pal.text,
        );
        cr
    });
    let room = w - 3.0 * pad - btn - csv.map_or(0.0, |c| c.w + pad);
    let pos = text.layout(&p.position, bold, Some(room.max(20.0)), Align::Start);
    let px = if rtl {
        play.x - pad - pos.width
    } else {
        play.x + btn + pad
    };
    text.draw(dl, &pos, px, y + pad + (btn - pos.height) / 2.0, pal.text);
    let top = y + pad + btn + 4.0 * ts;
    let chart_r = Rect::new(x + pad, top, w - 2.0 * pad, y + h - pad - top);
    if let Some(note) = &p.note {
        let b = text.layout(note, small, Some(chart_r.w), Align::Start);
        let nx = if rtl {
            chart_r.x + chart_r.w - b.width
        } else {
            chart_r.x
        };
        text.draw(dl, &b, nx, chart_r.y + 4.0 * ts, pal.text_muted);
    }
    if let Some(s) = &p.series {
        draw_chart(dl, text, pal, s, p.at, chart_r, rtl, ts);
    }
    Drawn {
        panel: Some(r),
        play: Some(play),
        csv,
        chart: p.series.as_ref().map(|_| chart_r),
    }
}

/// A short number for the chart's labels.
fn label(v: f64) -> String {
    trimmed(v, if v.abs() >= 100.0 { 0 } else { 1 })
}

/// The chart: each measurement's mean (a spot's value) over time, the value labels on the
/// start side, the time below, a legend of the numbers on top and a cursor at `at` seconds.
#[allow(clippy::too_many_arguments)]
fn draw_chart(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    s: &Series,
    at: f64,
    r: Rect,
    rtl: bool,
    ts: f32,
) {
    let small = TextStyle::new(FONT_SMALL * ts).tabular();
    let (t0, t1) = (
        s.times.first().copied().unwrap_or(0.0),
        s.times.last().copied().unwrap_or(1.0),
    );
    let t1 = if t1 - t0 < 1e-9 { t0 + 1.0 } else { t1 };
    let finite = s
        .lines
        .iter()
        .flat_map(|l| l.mean.iter().copied())
        .filter(|v| v.is_finite());
    let (v0, v1) = finite.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
    if v0 > v1 {
        return;
    }
    let (v0, v1) = if v1 - v0 < 1e-6 {
        (v0 - 0.5, v1 + 0.5)
    } else {
        let pad = (v1 - v0) * 0.08;
        (v0 - pad, v1 + pad)
    };
    let unit = s.unit.symbol();
    let top = text.layout(&format!("{} {unit}", label(v1)), small, None, Align::Start);
    let bot = text.layout(&format!("{} {unit}", label(v0)), small, None, Align::Start);
    let start = text.layout("0 s", small, None, Align::Start);
    let end = text.layout(
        &format!("{} s", trimmed(t1 - t0, 2)),
        small,
        None,
        Align::Start,
    );
    // Legend: each measurement's number in its colour.
    let mut legend_h: f32 = 0.0;
    let mut lx = 0.0;
    for l in &s.lines {
        let b = text.layout(&format!("● {}", l.id), small, None, Align::Start);
        let bx = if rtl {
            r.x + r.w - lx - b.width
        } else {
            r.x + lx
        };
        text.draw(dl, &b, bx, r.y, ink(l.id));
        lx += b.width + 8.0 * ts;
        legend_h = legend_h.max(b.height);
    }
    let label_w = top.width.max(bot.width) + 4.0 * ts;
    let py0 = r.y + legend_h + 4.0 * ts + top.height / 2.0;
    let py1 = r.y + r.h - start.height - 2.0 * ts;
    let (px0, px1) = if rtl {
        (r.x, r.x + r.w - label_w)
    } else {
        (r.x + label_w, r.x + r.w)
    };
    if py1 - py0 < 10.0 || px1 - px0 < 10.0 {
        return;
    }
    let lab_x = if rtl { px1 + 4.0 * ts } else { r.x };
    text.draw(dl, &top, lab_x, py0 - top.height / 2.0, pal.text_muted);
    text.draw(dl, &bot, lab_x, py1 - bot.height / 2.0, pal.text_muted);
    // Time runs with the reading direction.
    let tx = |t: f64| {
        let f = ((t - t0) / (t1 - t0)) as f32;
        if rtl {
            px1 - f * (px1 - px0)
        } else {
            px0 + f * (px1 - px0)
        }
    };
    let vy = |v: f64| py1 - ((v - v0) / (v1 - v0)) as f32 * (py1 - py0);
    let (sx, ex) = if rtl {
        (px1 - start.width, px0)
    } else {
        (px0, px1 - end.width)
    };
    text.draw(dl, &start, sx, py1 + 2.0 * ts, pal.text_muted);
    text.draw(dl, &end, ex, py1 + 2.0 * ts, pal.text_muted);
    dl.line((px0, py1), (px1, py1), ts, pal.separator);
    // The value axis on the side of its labels.
    let axis_x = if rtl { px1 } else { px0 };
    dl.line((axis_x, py0), (axis_x, py1), ts, pal.separator);
    for l in &s.lines {
        let c = ink(l.id);
        let mut prev: Option<(f32, f32)> = None;
        for (k, &v) in l.mean.iter().enumerate() {
            if !v.is_finite() {
                prev = None;
                continue;
            }
            let p = (tx(s.times[k]), vy(v));
            if let Some(q) = prev {
                dl.line(q, p, 1.8 * ts, c);
            } else if l.mean.len() == 1 {
                dl.circle(p, 2.5 * ts, c);
            }
            prev = Some(p);
        }
    }
    let cx = tx(at.clamp(t0, t1));
    dl.line((cx, py0), (cx, py1), 1.2 * ts, pal.text);
}

/// The value of each measurement at frame `k`, as a sentence for screen readers.
fn chart_text(s: &Series, k: usize) -> String {
    let unit = s.unit.symbol();
    s.lines
        .iter()
        .filter_map(|l| {
            let v = *l.mean.get(k)?;
            v.is_finite()
                .then(|| format!("{} {}: {} {unit}", l.id, l.name, trimmed(v, 2)))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The panel's AccessKit nodes: a group with the play button, Save CSV and the chart's values
/// at the frame shown.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    p: &Panel,
    d: &Drawn,
    frame: usize,
    sf: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let Some(panel) = d.panel else { return };
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::Group);
    g.set_label(format!("{}: {}", t!("thermal-seq-title"), p.position));
    g.set_bounds(bounds(panel));
    let mut kids = Vec::new();
    if let Some(r) = d.play {
        let mut n = Node::new(Role::Button);
        n.set_label(if p.playing {
            t!("thermal-seq-pause")
        } else {
            t!("thermal-seq-play")
        });
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Click);
        kids.push((NodeId(A11Y_PLAY), n));
    }
    if let Some(r) = d.csv {
        let mut n = Node::new(Role::Button);
        n.set_label(t!("thermal-seq-save-csv"));
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Click);
        kids.push((NodeId(A11Y_CSV), n));
    }
    if let (Some(r), Some(s)) = (d.chart, &p.series) {
        let mut n = Node::new(Role::Image);
        n.set_label(t!("thermal-seq-chart", values = chart_text(s, frame)));
        n.set_bounds(bounds(r));
        kids.push((NodeId(A11Y_CHART), n));
    } else if let Some(note) = &p.note {
        let mut n = Node::new(Role::Label);
        n.set_label(note.clone());
        kids.push((NodeId(A11Y_CHART), n));
    }
    for (id, _) in &kids {
        g.push_child(*id);
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_PANEL));
    }
    tree.nodes.push((NodeId(A11Y_PANEL), g));
    tree.nodes.extend(kids);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series() -> Series {
        Series {
            times: vec![0.0, 0.04, 0.08],
            unit: TemperatureUnit::Celsius,
            lines: vec![
                Line {
                    id: 1,
                    name: "Spot".into(),
                    mean: vec![30.0, 31.25, f64::NAN],
                    min: vec![30.0, 31.25, f64::NAN],
                    max: vec![30.0, 31.25, f64::NAN],
                },
                Line {
                    id: 2,
                    name: "Rectangle".into(),
                    mean: vec![20.5, 21.0, 22.0],
                    min: vec![18.0, 19.0, 20.0],
                    max: vec![40.0, 41.0, 42.0],
                },
            ],
        }
    }

    #[test]
    fn csv_rows_per_frame() {
        let c = csv(&series());
        let rows: Vec<&str> = c.lines().collect();
        assert_eq!(
            rows[0],
            "frame,time_s,1 Spot mean (°C),1 Spot min (°C),1 Spot max (°C),\
             2 Rectangle mean (°C),2 Rectangle min (°C),2 Rectangle max (°C)"
        );
        assert_eq!(rows[1], "1,0.000,30.000,30.000,30.000,20.500,18.000,40.000");
        assert_eq!(rows[3], "3,0.080,,,,22.000,20.000,42.000");
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn chart_text_names_each_value() {
        let s = series();
        assert_eq!(chart_text(&s, 1), "1 Spot: 31.25 °C, 2 Rectangle: 21 °C");
        assert_eq!(chart_text(&s, 2), "2 Rectangle: 22 °C");
    }

    #[test]
    fn inks_cycle() {
        assert_eq!(ink(1), INKS[0]);
        assert_eq!(ink(7), INKS[0]);
        assert_eq!(ink(0), INKS[0]);
    }
}

//! The map's time slider: a weather grid's time steps (a NetCDF time dimension, a GRIB
//! grid's forecast times) or a GPS track's times, with play. A grid steps through science
//! mode's planes, waiting for each plane to arrive before the next; a track moves a marker
//! along it at a speed that plays it whole in about half a minute.
//!
//! Time runs with the reading direction: right to left in Hebrew and Arabic.

use std::sync::Arc;

use specular_decode::science::ScienceSummary;
use specular_geodesy::time;
use specular_geodesy::track::Timeline;
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_theme::Palette;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};

/// Seconds a grid's step shows while playing (at least: a slow plane holds it longer).
pub(super) const GRID_STEP_S: f64 = 0.7;
/// Seconds a whole track takes to play at the starting speed.
const TRACK_PLAY_S: f64 = 30.0;
/// Frames a second while a track plays.
const TRACK_FPS: f64 = 30.0;
/// The bar's track width at text scale 1, logical px.
const TRACK_W: f32 = 320.0;

/// What the slider moves through.
#[derive(Clone, Debug)]
pub(super) enum Source {
    /// Science mode's axis `axis` of `len` positions, with a date per position.
    Grid {
        axis: usize,
        len: u32,
        labels: Vec<String>,
    },
    /// A GPS track's fixes in time.
    Track(Arc<Timeline>),
}

/// The slider's state.
#[derive(Clone, Debug)]
pub(super) struct TimeBar {
    pub source: Source,
    /// A grid's position, or a track's time (seconds).
    pub pos: f64,
    pub playing: bool,
    /// When the next step is due (the app's clock, seconds).
    pub next: f64,
    /// A track plays this many seconds of its time per second.
    pub speed: f64,
}

/// The time axis of a grid: one named like time ("time", "valid_time", GRIB's "time")
/// whose positions are all dates, else any axis whose labels are all dates.
pub(super) fn time_axis(s: &ScienceSummary) -> Option<usize> {
    let dated = |a: usize| {
        let ax = &s.axes[a];
        ax.len >= 2
            && ax.labels.len() == ax.len as usize
            && ax.labels.iter().all(|l| time::parse(l).is_some())
    };
    let named = |a: usize| {
        let n = s.axes[a].name.to_ascii_lowercase();
        matches!(
            n.as_str(),
            "time" | "t" | "valid_time" | "times" | "date" | "step"
        )
    };
    (0..s.axes.len())
        .find(|&a| named(a) && dated(a))
        .or_else(|| (0..s.axes.len()).find(|&a| dated(a)))
}

/// A playing speed for a track: a round multiple of real time that plays it in about
/// [`TRACK_PLAY_S`].
pub(super) fn track_speed(span: f64) -> f64 {
    let raw = (span / TRACK_PLAY_S).max(1.0);
    let p = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * p)
        .find(|&s| s >= raw)
        .unwrap_or(raw)
}

impl TimeBar {
    pub(super) fn grid(axis: usize, len: u32, labels: Vec<String>, at: u32) -> TimeBar {
        TimeBar {
            source: Source::Grid { axis, len, labels },
            pos: f64::from(at),
            playing: false,
            next: 0.0,
            speed: 1.0,
        }
    }

    pub(super) fn track(t: Arc<Timeline>) -> TimeBar {
        let (t0, t1) = t.span().unwrap_or((0.0, 0.0));
        TimeBar {
            speed: track_speed(t1 - t0),
            source: Source::Track(t),
            pos: t0,
            playing: false,
            next: 0.0,
        }
    }

    /// The range of `pos`.
    pub(super) fn range(&self) -> (f64, f64) {
        match &self.source {
            Source::Grid { len, .. } => (0.0, f64::from(len.saturating_sub(1))),
            Source::Track(t) => t.span().unwrap_or((0.0, 0.0)),
        }
    }

    /// Where `pos` is along the range, 0..=1.
    pub(super) fn fraction(&self) -> f64 {
        let (a, b) = self.range();
        if b > a {
            ((self.pos - a) / (b - a)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// `pos` for a fraction of the range (a grid's rounded to a step).
    pub(super) fn at_fraction(&self, f: f64) -> f64 {
        let (a, b) = self.range();
        let p = a + f.clamp(0.0, 1.0) * (b - a);
        match self.source {
            Source::Grid { .. } => p.round(),
            Source::Track(_) => p,
        }
    }

    /// A step: one position of a grid, a hundredth of a track (`big`: ten of them).
    pub(super) fn step(&mut self, dir: f64, big: bool) {
        let (a, b) = self.range();
        let d = match self.source {
            Source::Grid { .. } => 1.0,
            Source::Track(_) => (b - a) / 100.0,
        } * if big { 10.0 } else { 1.0 };
        self.pos = (self.pos + dir * d).clamp(a, b);
    }

    /// Plays on from `now`: a grid's next step when due (and its plane in, `ready`), a
    /// track's time; at the end it starts again. True when `pos` moved.
    pub(super) fn tick(&mut self, now: f64, ready: bool) -> bool {
        if !self.playing || now < self.next {
            return false;
        }
        let (a, b) = self.range();
        match self.source {
            Source::Grid { .. } => {
                if !ready {
                    return false;
                }
                self.pos = if self.pos + 1.0 > b {
                    a
                } else {
                    self.pos + 1.0
                };
                self.next = now + GRID_STEP_S;
            }
            Source::Track(_) => {
                let dt = 1.0 / TRACK_FPS;
                self.pos = if self.pos >= b {
                    a
                } else {
                    (self.pos + self.speed * dt).min(b)
                };
                self.next = now + dt;
            }
        }
        true
    }

    /// When the next step is due, while playing.
    pub(super) fn due(&self) -> Option<f64> {
        self.playing.then_some(self.next)
    }

    /// The bar's text: the date (or time along the track) and where it is.
    pub(super) fn label(&self) -> String {
        match &self.source {
            Source::Grid { len, labels, .. } => {
                let i = self.pos.round().max(0.0) as usize;
                let when = labels.get(i).cloned().unwrap_or_default();
                t!(
                    "map-time-grid",
                    when = when,
                    step = (i + 1) as i64,
                    steps = i64::from(*len)
                )
            }
            Source::Track(tl) => {
                let (t0, _) = tl.span().unwrap_or((0.0, 0.0));
                t!(
                    "map-time-track",
                    when = time::format(self.pos),
                    elapsed = specular_i18n::isolate_ltr(&time::format_offset(self.pos - t0))
                        .trim_start_matches('+')
                        .to_owned()
                )
            }
        }
    }

    /// The playing speed's text for a track ("× 60").
    pub(super) fn speed_label(&self) -> Option<String> {
        matches!(self.source, Source::Track(_)).then(|| {
            t!(
                "map-time-speed",
                times = specular_i18n::fixed(self.speed, 0)
            )
        })
    }

    /// The grid position to show, for a grid.
    pub(super) fn grid_index(&self) -> Option<(usize, u32)> {
        match self.source {
            Source::Grid { axis, .. } => Some((axis, self.pos.round().max(0.0) as u32)),
            Source::Track(_) => None,
        }
    }

    /// The track's position at the slider's time, for a track.
    pub(super) fn track_at(&self) -> Option<(f64, f64)> {
        match &self.source {
            Source::Track(t) => t.at(self.pos).map(|(p, _)| p),
            Source::Grid { .. } => None,
        }
    }
}

/// The time bar as drawn: its panel, the play button and the track.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::app) struct DrawnBar {
    pub panel: Option<Rect>,
    pub play: Option<Rect>,
    pub track: Option<Rect>,
}

/// The bar's height at text scale `ts`.
pub(super) fn height(ts: f32) -> f32 {
    2.0 * PAD + FONT_SMALL * 1.4 * ts + 24.0 * ts
}

/// Draws the time bar at the bottom centre of `area`, above `floor`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    bar: &TimeBar,
    focused: bool,
    area: (f32, f32, f32, f32),
    floor: Option<f32>,
    rtl: bool,
    ts: f32,
) -> DrawnBar {
    let (ax, ay, aw, ah) = area;
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    let label = text.layout(&bar.label(), st, None, Align::Start);
    let speed = bar
        .speed_label()
        .map(|s| text.layout(&s, st, None, Align::Start));
    let btn = 24.0 * ts;
    let tw = (TRACK_W * ts)
        .min(aw - 2.0 * TOOLBAR_MARGIN - 3.0 * PAD - btn)
        .max(60.0);
    let w = (btn + PAD + tw).max(label.width) + 2.0 * PAD;
    let h = height(ts);
    let bottom = floor.unwrap_or(ay + ah) - TOOLBAR_MARGIN - 70.0 * ts;
    let x = ax + (aw - w) / 2.0;
    let r = Rect::new(x, bottom - h, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    if focused {
        dl.stroke_rect(r, RADIUS_MD, pal.accent, 1.5 * ts);
    }
    let ty = r.y + PAD;
    let lx = if rtl {
        r.x + r.w - PAD - label.width
    } else {
        r.x + PAD
    };
    text.draw(dl, &label, lx, ty, pal.text);
    if let Some(s) = &speed {
        let sx = if rtl {
            r.x + PAD
        } else {
            r.x + r.w - PAD - s.width
        };
        text.draw(dl, s, sx, ty, pal.text_muted);
    }
    let row = ty + label.height + 2.0 * ts;
    let (bx, tx) = if rtl {
        (r.x + r.w - PAD - btn, r.x + PAD)
    } else {
        (r.x + PAD, r.x + PAD + btn + PAD)
    };
    let play = Rect::new(bx, row, btn, btn);
    dl.rect(play, btn / 2.0, pal.surface);
    let icon = if bar.playing { Icon::Pause } else { Icon::Play };
    // Media controls keep their direction in right-to-left languages.
    draw_icon(dl, icon, play.inset(5.0 * ts), pal.text, 1.6 * ts, false);
    let cy = row + btn / 2.0;
    let track = Rect::new(tx, cy - 2.0 * ts, tw, 4.0 * ts);
    dl.rect(track, 2.0 * ts, pal.separator);
    let f = bar.fraction() as f32;
    let k = if rtl { 1.0 - f } else { f };
    let kx = track.x + track.w * k;
    let (fill_x, fill_w) = if rtl {
        (kx, track.x + track.w - kx)
    } else {
        (track.x, kx - track.x)
    };
    dl.rect(
        Rect::new(fill_x, track.y, fill_w, track.h),
        2.0 * ts,
        pal.accent,
    );
    dl.circle((kx, cy), 6.0 * ts, pal.accent);
    if let Source::Grid { len, .. } = bar.source
        && len <= 60
    {
        for i in 0..len {
            let f = i as f32 / (len - 1).max(1) as f32;
            let px = track.x + track.w * if rtl { 1.0 - f } else { f };
            dl.line((px, cy + 5.0 * ts), (px, cy + 8.0 * ts), ts, pal.text_muted);
        }
    }
    DrawnBar {
        panel: Some(r),
        play: Some(play),
        track: Some(Rect::new(track.x, cy - 8.0 * ts, track.w, 16.0 * ts)),
    }
}

/// The slider's fraction at screen x over the track drawn.
pub(super) fn fraction_at(track: Rect, x: f32, rtl: bool) -> f64 {
    let f = f64::from(((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0));
    if rtl { 1.0 - f } else { f }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::science::{AxisKind, NdAxis};
    use specular_geodesy::track::Fix;

    #[test]
    fn grid_time_axes_and_steps() {
        let axis = |name: &str, labels: Vec<&str>| NdAxis {
            name: name.into(),
            len: labels.len() as u32,
            unit: String::new(),
            labels: labels.into_iter().map(str::to_owned).collect(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Cube,
        };
        let s = ScienceSummary {
            axes: vec![
                axis("level", vec!["1000 hPa", "850 hPa"]),
                axis(
                    "time",
                    vec![
                        "2024-01-01 00:00 (+0 h)",
                        "2024-01-01 06:00 (+6 h)",
                        "2024-01-01 12:00 (+12 h)",
                    ],
                ),
            ],
            ..Default::default()
        };
        assert_eq!(time_axis(&s), Some(1));
        let mut b = TimeBar::grid(1, 3, s.axes[1].labels.clone(), 0);
        b.playing = true;
        assert!(!b.tick(0.0, false), "waits for the plane");
        assert!(b.tick(0.0, true));
        assert_eq!(b.grid_index(), Some((1, 1)));
        assert!(!b.tick(0.1, true), "not yet due");
        assert!(b.tick(GRID_STEP_S, true));
        assert!(b.tick(2.0 * GRID_STEP_S, true));
        assert_eq!(b.pos, 0.0, "wraps to the start");
        let l = b.label().replace(['\u{2068}', '\u{2069}'], "");
        assert!(l.contains("2024-01-01 00:00") && l.contains('3'), "{l}");
    }

    #[test]
    fn tracks_play_in_half_a_minute() {
        assert_eq!(track_speed(3600.0), 200.0);
        assert_eq!(track_speed(10.0), 1.0);
        let t = Arc::new(Timeline::new(vec![
            Fix {
                t: 0.0,
                lat: 47.0,
                lon: 8.0,
                ele: None,
            },
            Fix {
                t: 3000.0,
                lat: 47.1,
                lon: 8.0,
                ele: None,
            },
        ]));
        let mut b = TimeBar::track(t);
        assert_eq!(b.speed, 100.0);
        b.playing = true;
        assert!(b.tick(0.0, true));
        assert!((b.pos - 100.0 / TRACK_FPS).abs() < 1e-9);
        b.pos = 1500.0;
        let (lat, _) = b.track_at().unwrap();
        assert!((lat - 47.05).abs() < 1e-6);
        b.step(1.0, true);
        assert_eq!(b.pos, 1800.0);
        assert_eq!(
            fraction_at(Rect::new(0.0, 0.0, 100.0, 10.0), 25.0, true),
            0.75
        );
    }
}

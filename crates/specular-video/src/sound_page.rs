//! The sound page's arrangement: which visuals show ([`Visuals`]), where each goes
//! ([`PageLayout`]), and the part of the sound the time-based ones show ([`TimeView`]:
//! zoomed with the wheel around the pointer, panned by dragging, following the playhead).
//!
//! Time-based visuals (waveform, spectrogram, loudness history) are stacked across the
//! page and share one time axis; the live ones (spectrum, meters, stereo panel, loudness
//! numbers) sit in a row under them, with the cover. Without live visuals the cover is
//! above the graphs, as on the picture. Lyrics, when shown, take a column at the end side.

/// A rectangle (logical px).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Area {
        Area { x, y, w, h }
    }

    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 <= self.x + self.w && p.1 >= self.y && p.1 <= self.y + self.h
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// The visuals that can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visual {
    Waveform,
    Spectrogram,
    Spectrum,
    Meters,
    Loudness,
    /// The goniometer and the phase correlation.
    Stereo,
}

impl Visual {
    pub const ALL: [Visual; 6] = [
        Visual::Waveform,
        Visual::Spectrogram,
        Visual::Spectrum,
        Visual::Meters,
        Visual::Loudness,
        Visual::Stereo,
    ];

    /// Its name in settings.
    pub fn key(self) -> &'static str {
        match self {
            Visual::Waveform => "waveform",
            Visual::Spectrogram => "spectrogram",
            Visual::Spectrum => "spectrum",
            Visual::Meters => "meters",
            Visual::Loudness => "loudness",
            Visual::Stereo => "stereo",
        }
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Which visuals show, and how the spectrum is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Visuals {
    bits: u8,
    /// The spectrum as a curve of the spectrogram's bins (else third-octave bars).
    pub curve: bool,
}

impl Default for Visuals {
    fn default() -> Self {
        Visuals::PRESETS[0]
    }
}

impl Visuals {
    /// What the "next visuals" key goes through: the overview (as the picture), the meters,
    /// everything.
    pub const PRESETS: [Visuals; 3] = [
        Visuals {
            bits: 0b00011,
            curve: false,
        },
        Visuals {
            bits: 0b111100,
            curve: false,
        },
        Visuals {
            bits: 0b111111,
            curve: false,
        },
    ];

    pub fn has(self, v: Visual) -> bool {
        self.bits & v.bit() != 0
    }

    /// `v` turned on or off; the last one stays on.
    pub fn toggled(self, v: Visual) -> Visuals {
        let bits = self.bits ^ v.bit();
        if bits == 0 {
            self
        } else {
            Visuals { bits, ..self }
        }
    }

    /// The spectrum as a curve, or as bars.
    pub fn with_curve(self, curve: bool) -> Visuals {
        Visuals { curve, ..self }
    }

    /// The next preset (the first when these are none of them).
    pub fn next(self) -> Visuals {
        let i = Visuals::PRESETS
            .iter()
            .position(|p| p.bits == self.bits)
            .map_or(0, |i| (i + 1) % Visuals::PRESETS.len());
        Visuals {
            curve: self.curve,
            ..Visuals::PRESETS[i]
        }
    }

    /// From settings: names separated by commas, `curve` for the spectrum's curve.
    pub fn parse(s: &str) -> Visuals {
        let mut v = Visuals {
            bits: 0,
            curve: false,
        };
        for w in s.split(',').map(str::trim) {
            if w == "curve" {
                v.curve = true;
            } else if let Some(x) = Visual::ALL.iter().find(|x| x.key() == w) {
                v.bits |= x.bit();
            }
        }
        if v.bits == 0 {
            v.bits = Visuals::PRESETS[0].bits;
        }
        v
    }

    /// For settings.
    pub fn format(self) -> String {
        let mut words: Vec<&str> = Visual::ALL
            .iter()
            .filter(|&&x| self.has(x))
            .map(|x| x.key())
            .collect();
        if self.curve {
            words.push("curve");
        }
        words.join(",")
    }

    /// Any time-based visual shows.
    pub fn timeline(self) -> bool {
        self.has(Visual::Waveform) || self.has(Visual::Spectrogram) || self.has(Visual::Loudness)
    }

    /// Any live visual shows.
    pub fn live(self) -> bool {
        self.has(Visual::Spectrum)
            || self.has(Visual::Meters)
            || self.has(Visual::Loudness)
            || self.has(Visual::Stereo)
    }
}

/// Sizes (logical px).
pub const MARGIN: f32 = 16.0;
pub const GAP: f32 = 8.0;
/// The row of visual choices at the top.
pub const BAR_H: f32 = 30.0;
/// The labels beside the time-based visuals, and the time axis under them.
pub const GUTTER: f32 = 44.0;
pub const AXIS_H: f32 = 18.0;
/// A meter channel's width, and the meters' scale.
pub const METER_W: f32 = 22.0;
pub const METER_SCALE_W: f32 = 30.0;
/// The loudness numbers' width.
pub const NUMBERS_W: f32 = 210.0;
/// The stereo panel's width: the row's height, within these.
pub const STEREO_MIN_W: f32 = 120.0;
pub const STEREO_MAX_W: f32 = 240.0;
/// The lyrics column: a share of the page's width, within these; not on a page narrower
/// than [`LYRICS_MIN_PAGE`].
pub const LYRICS_SHARE: f32 = 0.3;
pub const LYRICS_MIN_W: f32 = 200.0;
pub const LYRICS_MAX_W: f32 = 380.0;
pub const LYRICS_MIN_PAGE: f32 = 520.0;

/// Where the parts of the sound page go.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageLayout {
    /// The row of visual choices.
    pub bar: Area,
    pub cover: Option<Area>,
    /// The plots of the time-based visuals (labels go in the gutter beside them).
    pub waveform: Option<Area>,
    pub spectrogram: Option<Area>,
    pub history: Option<Area>,
    /// The time axis under them.
    pub axis: Option<Area>,
    pub spectrum: Option<Area>,
    pub meters: Option<Area>,
    pub numbers: Option<Area>,
    pub stereo: Option<Area>,
    pub lyrics: Option<Area>,
}

impl PageLayout {
    /// The layout of `v` in `area`, `cover`: there is a cover to show; `channels`: meter
    /// channels (the stereo panel needs two); `rtl`: labels, the live row and the lyrics
    /// column mirror (time still runs left to right); `lyrics`: lyrics to show.
    pub fn new(
        area: Area,
        v: Visuals,
        cover: bool,
        channels: usize,
        rtl: bool,
        lyrics: bool,
    ) -> PageLayout {
        let mut l = PageLayout {
            bar: Area::new(
                area.x + MARGIN,
                area.y + MARGIN,
                (area.w - 2.0 * MARGIN).max(0.0),
                BAR_H,
            ),
            ..PageLayout::default()
        };
        let top = l.bar.bottom() + GAP;
        let mut inner = Area::new(
            area.x + MARGIN,
            top,
            (area.w - 2.0 * MARGIN).max(1.0),
            (area.bottom() - MARGIN - top).max(1.0),
        );
        if lyrics && inner.w >= LYRICS_MIN_PAGE {
            let w = (inner.w * LYRICS_SHARE).clamp(LYRICS_MIN_W, LYRICS_MAX_W);
            let x = if rtl { inner.x } else { inner.right() - w };
            l.lyrics = Some(Area::new(x, inner.y, w, inner.h));
            inner.w -= w + GAP;
            if rtl {
                inner.x += w + GAP;
            }
        }
        let stereo = v.has(Visual::Stereo) && channels >= 2;
        let timeline =
            v.has(Visual::Waveform) || v.has(Visual::Spectrogram) || v.has(Visual::Loudness);
        let live =
            v.has(Visual::Spectrum) || v.has(Visual::Meters) || v.has(Visual::Loudness) || stereo;
        // The live row's height, and the cover's place.
        let (mut tl, live_row, cover_top) = if timeline && live {
            let h = (inner.h * 0.42).clamp(120.0, 320.0).min(inner.h * 0.6);
            (
                Area::new(inner.x, inner.y, inner.w, inner.h - h - GAP),
                Some(Area::new(inner.x, inner.bottom() - h, inner.w, h)),
                false,
            )
        } else if live {
            (Area::default(), Some(inner), false)
        } else {
            (inner, None, cover)
        };
        if cover_top {
            let h = (inner.h * 0.38).min(300.0);
            if h >= 48.0 {
                l.cover = Some(Area::new(inner.x, inner.y, inner.w, h));
                tl = Area::new(inner.x, inner.y + h + GAP, inner.w, inner.h - h - GAP);
            }
        }
        // Time-based visuals: stacked, sharing the time axis under them.
        if timeline {
            let plot_x = if rtl { tl.x } else { tl.x + GUTTER };
            let plot_w = (tl.w - GUTTER).max(1.0);
            let parts: Vec<(Visual, f32)> = [
                (Visual::Waveform, 1.0),
                (Visual::Spectrogram, 1.2),
                (Visual::Loudness, 0.6),
            ]
            .into_iter()
            .filter(|(x, _)| v.has(*x))
            .collect();
            let gaps = GAP * (parts.len() as f32 - 1.0);
            let room = (tl.h - AXIS_H - gaps).max(parts.len() as f32);
            let total: f32 = parts.iter().map(|p| p.1).sum();
            let mut y = tl.y;
            for (x, wgt) in parts {
                let h = room * wgt / total;
                let a = Some(Area::new(plot_x, y, plot_w, h));
                match x {
                    Visual::Waveform => l.waveform = a,
                    Visual::Spectrogram => l.spectrogram = a,
                    _ => l.history = a,
                }
                y += h + GAP;
            }
            l.axis = Some(Area::new(plot_x, y - GAP, plot_w, AXIS_H));
        }
        // Live visuals: cover, spectrum (the rest), meters, numbers; mirrored right to left.
        if let Some(row) = live_row {
            let mut items: Vec<(u8, f32)> = Vec::new();
            let meters_w = METER_SCALE_W + METER_W * channels.max(1) as f32 + 8.0;
            if cover && row.h >= 48.0 {
                items.push((0, row.h.min(row.w * 0.25)));
            }
            if v.has(Visual::Spectrum) {
                items.push((1, 0.0));
            }
            if v.has(Visual::Meters) {
                items.push((2, meters_w));
            }
            if stereo {
                items.push((4, row.h.clamp(STEREO_MIN_W, STEREO_MAX_W)));
            }
            if v.has(Visual::Loudness) {
                items.push((3, NUMBERS_W));
            }
            let fixed: f32 =
                items.iter().map(|i| i.1).sum::<f32>() + GAP * (items.len() as f32 - 1.0).max(0.0);
            // A narrow page drops the cover first.
            if fixed > row.w && items.first().is_some_and(|i| i.0 == 0) {
                items.remove(0);
            }
            let fixed: f32 =
                items.iter().map(|i| i.1).sum::<f32>() + GAP * (items.len() as f32 - 1.0).max(0.0);
            let flex = (row.w - fixed).max(0.0);
            // The spectrum takes the rest; without it, the last part does.
            match items.iter_mut().find(|i| i.0 == 1) {
                Some(s) => s.1 = flex,
                None => {
                    if let Some(last) = items.last_mut() {
                        last.1 += flex;
                    }
                }
            }
            let mut x = 0.0;
            for (k, w) in items {
                let w = w.min(row.w - x).max(0.0);
                let ax = if rtl { row.right() - x - w } else { row.x + x };
                let a = Some(Area::new(ax, row.y, w, row.h));
                match k {
                    0 => l.cover = a,
                    1 => l.spectrum = a,
                    2 => l.meters = a,
                    4 => l.stereo = a,
                    _ => l.numbers = a,
                }
                x += w + GAP;
            }
        }
        l
    }

    /// The time-based plots shown.
    pub fn timelines(&self) -> impl Iterator<Item = Area> + '_ {
        [self.waveform, self.spectrogram, self.history]
            .into_iter()
            .flatten()
    }

    /// Where in the sound (0 start, 1 end) a point is, when it is on a time-based plot.
    pub fn time_at(&self, p: (f32, f32), view: &TimeView) -> Option<f64> {
        self.timelines()
            .find(|a| a.contains(p))
            .map(|a| view.at(f64::from((p.0 - a.x) / a.w.max(1.0))))
    }
}

/// The part of the sound the time-based visuals show (fractions of its length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeView {
    pub start: f64,
    pub span: f64,
    /// Keeps the playhead in view while playing (dragging stops it; a seek or play starts
    /// it again).
    pub follow: bool,
}

impl Default for TimeView {
    fn default() -> Self {
        TimeView {
            start: 0.0,
            span: 1.0,
            follow: true,
        }
    }
}

impl TimeView {
    /// The fraction of the sound at `x` across the plot (0…1).
    pub fn at(&self, x: f64) -> f64 {
        (self.start + x.clamp(0.0, 1.0) * self.span).clamp(0.0, 1.0)
    }

    /// Where fraction `f` of the sound is across the plot (outside 0…1 when not shown).
    pub fn x_of(&self, f: f64) -> f64 {
        (f - self.start) / self.span
    }

    /// Zoomed in by `factor` (> 1 closer) around `x` across the plot, no closer than
    /// `min_span`.
    pub fn zoom(&mut self, factor: f64, x: f64, min_span: f64) {
        let anchor = self.at(x);
        let span = (self.span / factor).clamp(min_span.min(1.0), 1.0);
        self.span = span;
        self.start = anchor - x.clamp(0.0, 1.0) * span;
        self.clamp();
    }

    /// Moved by `dx` plot widths (positive: later sound comes into view).
    pub fn pan(&mut self, dx: f64) {
        self.start += dx * self.span;
        self.follow = false;
        self.clamp();
    }

    fn clamp(&mut self) {
        self.start = self.start.clamp(0.0, (1.0 - self.span).max(0.0));
    }

    /// Keeps playhead `p` in view while playing: scrolling to keep it at the middle, or,
    /// with reduce motion, turning a page when it nears the edge.
    pub fn follow(&mut self, p: f64, reduce_motion: bool) {
        if !self.follow || self.span >= 1.0 {
            return;
        }
        let x = self.x_of(p);
        if reduce_motion {
            if !(0.0..0.95).contains(&x) {
                self.start = p - 0.05 * self.span;
            }
        } else if x > 0.5 || x < 0.0 {
            self.start = p - 0.5 * self.span;
        }
        self.clamp();
    }

    /// Zoomed in at all.
    pub fn zoomed(&self) -> bool {
        self.span < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visuals_round_trip_and_presets() {
        let v = Visuals::parse("spectrum, meters,curve,unknown");
        assert!(v.has(Visual::Spectrum) && v.has(Visual::Meters) && v.curve);
        assert!(!v.has(Visual::Waveform));
        assert_eq!(Visuals::parse(&v.format()), v);
        assert_eq!(Visuals::parse(""), Visuals::default());
        assert_eq!(Visuals::default().format(), "waveform,spectrogram");
        // The key goes round the presets; a set of one's own goes to the first.
        let a = Visuals::default().next();
        assert_eq!(a.format(), "spectrum,meters,loudness,stereo");
        assert_eq!(a.next().next(), Visuals::default());
        assert_eq!(v.next().format(), "waveform,spectrogram,curve");
        // The last one stays on.
        let one = Visuals::parse("meters");
        assert_eq!(one.toggled(Visual::Meters), one);
        assert!(one.toggled(Visual::Waveform).has(Visual::Waveform));
    }

    fn inside(a: Area, b: Area) -> bool {
        a.x >= b.x - 0.01
            && a.y >= b.y - 0.01
            && a.right() <= b.right() + 0.01
            && a.bottom() <= b.bottom() + 0.01
    }

    #[test]
    fn layouts_fit_and_do_not_overlap() {
        let page = Area::new(10.0, 20.0, 1200.0, 800.0);
        for bits in 1u8..64 {
            for (cover, rtl, lyrics) in [
                (false, false, false),
                (true, false, false),
                (true, true, false),
                (true, false, true),
                (false, true, true),
            ] {
                let v = Visuals { bits, curve: false };
                let l = PageLayout::new(page, v, cover, 2, rtl, lyrics);
                let all: Vec<Area> = [
                    Some(l.bar),
                    l.cover,
                    l.waveform,
                    l.spectrogram,
                    l.history,
                    l.axis,
                    l.spectrum,
                    l.meters,
                    l.numbers,
                    l.stereo,
                    l.lyrics,
                ]
                .into_iter()
                .flatten()
                .collect();
                for (i, a) in all.iter().enumerate() {
                    assert!(inside(*a, page), "{bits:05b} {i}: {a:?}");
                    for b in &all[i + 1..] {
                        let overlap = a.x < b.right() - 0.01
                            && b.x < a.right() - 0.01
                            && a.y < b.bottom() - 0.01
                            && b.y < a.bottom() - 0.01;
                        assert!(!overlap, "{bits:05b}: {a:?} {b:?}");
                    }
                }
                assert_eq!(l.waveform.is_some(), v.has(Visual::Waveform));
                assert_eq!(l.spectrogram.is_some(), v.has(Visual::Spectrogram));
                assert_eq!(l.spectrum.is_some(), v.has(Visual::Spectrum));
                assert_eq!(l.meters.is_some(), v.has(Visual::Meters));
                assert_eq!(l.numbers.is_some(), v.has(Visual::Loudness));
                assert_eq!(l.history.is_some(), v.has(Visual::Loudness));
                assert_eq!(l.stereo.is_some(), v.has(Visual::Stereo));
                assert_eq!(l.lyrics.is_some(), lyrics);
                // The lyrics column is at the end side.
                if let Some(ly) = l.lyrics {
                    let others = [l.waveform, l.spectrum, l.meters, l.cover];
                    for o in others.into_iter().flatten() {
                        assert!(if rtl {
                            o.x >= ly.right()
                        } else {
                            o.right() <= ly.x
                        });
                    }
                }
            }
        }
        // Mono: no stereo panel; a narrow page: no lyrics column.
        let l = PageLayout::new(
            page,
            Visuals::parse("stereo,meters"),
            false,
            1,
            false,
            false,
        );
        assert!(l.stereo.is_none() && l.meters.is_some());
        let narrow = Area::new(0.0, 0.0, 400.0, 600.0);
        assert!(
            PageLayout::new(narrow, Visuals::default(), false, 2, false, true)
                .lyrics
                .is_none()
        );
    }

    #[test]
    fn clicks_on_the_plots_give_their_time() {
        let page = Area::new(0.0, 0.0, 1000.0, 700.0);
        let l = PageLayout::new(page, Visuals::default(), true, 2, false, false);
        let w = l.waveform.unwrap();
        let s = l.spectrogram.unwrap();
        let mut view = TimeView::default();
        assert_eq!(l.time_at((w.x, w.y + 5.0), &view), Some(0.0));
        let t = l.time_at((w.x + w.w * 0.25, s.y + 5.0), &view).unwrap();
        assert!((t - 0.25).abs() < 1e-6);
        // The cover and the gutter are not the plots.
        assert_eq!(l.time_at((500.0, l.cover.unwrap().y + 5.0), &view), None);
        assert_eq!(l.time_at((w.x - 5.0, w.y + 5.0), &view), None);
        // Zoomed in on the second half.
        view.zoom(2.0, 1.0, 0.0);
        assert!((view.start - 0.5).abs() < 1e-9);
        let t = l.time_at((w.x + w.w * 0.5, w.y + 5.0), &view).unwrap();
        assert!((t - 0.75).abs() < 1e-6);
    }

    #[test]
    fn zoom_pan_and_follow() {
        let mut v = TimeView::default();
        // Around the pointer: what is under it stays under it.
        v.zoom(4.0, 0.3, 0.001);
        assert!((v.span - 0.25).abs() < 1e-12);
        assert!((v.at(0.3) - 0.3).abs() < 1e-12);
        // No closer than the least span; no further out than the whole.
        v.zoom(1e9, 0.5, 0.01);
        assert!((v.span - 0.01).abs() < 1e-12);
        v.zoom(1e-9, 0.5, 0.01);
        assert_eq!((v.start, v.span), (0.0, 1.0));
        // Panning stays inside and stops following.
        v.zoom(10.0, 0.0, 0.0);
        v.pan(-3.0);
        assert_eq!(v.start, 0.0);
        assert!(!v.follow);
        v.pan(100.0);
        assert!((v.start - 0.9).abs() < 1e-12);
        // Following: the playhead held at the middle; reduce motion turns pages.
        let mut f = TimeView {
            start: 0.0,
            span: 0.1,
            follow: true,
        };
        f.follow(0.03, false);
        assert_eq!(f.start, 0.0);
        f.follow(0.08, false);
        assert!((f.start - 0.03).abs() < 1e-12);
        let mut r = TimeView {
            start: 0.0,
            span: 0.1,
            follow: true,
        };
        r.follow(0.08, true);
        assert_eq!(r.start, 0.0);
        r.follow(0.0951, true);
        assert!((r.start - 0.0901).abs() < 1e-9);
    }
}

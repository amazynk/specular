//! A sound file's page in the viewer, drawn on the GPU over its CPU picture (which stays for
//! thumbnails, printing and copying): the cover, the waveform, the spectrogram and the
//! loudness history (time-based, zoomed with the wheel around the pointer and panned by
//! dragging when zoomed; a click plays from there), and the live displays at the playing
//! position: spectrum analyser with peak hold, level meters (peak, RMS, true peak, clip) and
//! loudness numbers (EBU R 128). A row of choices at the top says which show
//! (`viewing.sound_visuals`; the `w` key goes through three sets).
//!
//! - The analysis (`specular_video::sound_view::Analysis`) and the loudness are worked out
//!   on a worker when the file is shown, and cached by its session; the integrated loudness
//!   and range then join the Info panel.
//! - The time-based visuals read the analysis from textures in `sound.wgsl`; bars, meters,
//!   labels and the playhead are drawn with the UI's own shapes and text.
//! - The playhead moves every frame while the sound plays (and only then: nothing redraws
//!   when paused). Reduce motion keeps the meters live but slows their falls, and a zoomed
//!   view turns pages instead of scrolling.
//! - Data colours (waveform, played part, spectrogram ramp) are the same in both themes;
//!   the page, panels, grid and text follow the theme. Clips are lit boxes with the scale
//!   and numbers beside them, never colour alone.
//! - While the sound plays, the live displays read what is heard (the player's live tap,
//!   `specular_video::live_tap`); paused or seeking, the whole-file analysis.
//! - The stereo panel: a goniometer (fading picture, slow automatic gain) and the phase
//!   correlation with its slow average (`specular_video::stereo`).
//! - Cover colours: the page's chrome tinted from the cover, text kept at WCAG AA
//!   ([`cover`]); the blurred cover behind the page ([`backdrop`]), under a scrim chosen to
//!   keep that. Settings turn each off.
//! - Lyrics (a `.lrc` beside the file, else its tags): a column at the end side. Synced
//!   lyrics scroll with the current line highlighted (word by word for enhanced LRC); a
//!   click on a line plays from it. Plain lyrics scroll with the wheel. Reduce motion: the
//!   lyrics jump instead of scrolling, the goniometer fades slower and redraws less.

mod backdrop;
mod cover;
mod gpu;
#[cfg(test)]
mod keyboard;
#[cfg(test)]
mod long;
#[cfg(test)]
mod shots;

use std::sync::atomic::AtomicI64;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use specular_core::CancelToken;
use specular_decode::Session;
use specular_gfx::{Align, DrawList, Gpu, Rect, TexId, Text, TextStyle, UiRenderer, wgpu};
use specular_i18n::t;
use specular_input::{Action, Key, NamedKey};
use specular_settings::Origin;
use specular_theme::metrics::{FONT_SMALL, RADIUS_SM};
use specular_theme::{Color, Mode, Palette};
use specular_video::live_tap::Tap;
use specular_video::loudness::Report;
use specular_video::lyrics::Lyrics;
use specular_video::sound_feed::{
    AnalysisFeed, Ballistics, FLOOR_DB, Feed, LiveFeed, Rates, Snapshot,
};
use specular_video::sound_page::{Area, PageLayout, TimeView, Visual, Visuals};
use specular_video::sound_view::{Analysis, BANDS, BINS, EXACT_MAX_US, SoundSession, band_centre};
use specular_video::stereo::{self, Goniometer};
use specular_widgets::accesskit;

use self::gpu::{Kind, Look, SoundGpu};
use super::App;
use crate::loader::ImageId;

/// AccessKit ids: the page, its choices, its meters and its numbers.
// Not 0x6E…: Develop's mask handles use that.
const A11Y_ID: u64 = 0x6B00_0000_0000_0000;
const A11Y_CHIP: u64 = A11Y_ID + 0x10;
const A11Y_METER: u64 = A11Y_ID + 0x100;
const A11Y_NUMBERS: u64 = A11Y_ID + 0x200;
const A11Y_STEREO: u64 = A11Y_ID + 0x300;
const A11Y_LYRICS: u64 = A11Y_ID + 0x400;
const A11Y_STATUS: u64 = A11Y_ID + 0x3F0;

/// The goniometer's picture (texels a side) and its trace colours (data colours: on a
/// dark panel, on a light one).
const GONIO_PX: usize = 128;
const GONIO_TRACE: [u8; 3] = [0x8C, 0xB6, 0xE6];
const GONIO_TRACE_LIGHT: [u8; 3] = [0x2F, 0x5A, 0x92];

/// Data colours (the same in both themes): the waveform and its RMS, and the played part.
/// The outer colours keep 3:1 against both themes' panels; the inner ones sit on them.
const WAVE: Color = Color::hex(0x4F7BB0);
const WAVE_RMS: Color = Color::hex(0x8CB6E6);
const PLAYED: Color = Color::hex(0xC27B1C);
const PLAYED_RMS: Color = Color::hex(0xF2C46D);
/// The spectrum's and the meters' range (dBFS).
const SPEC_FLOOR: f32 = -90.0;
const METER_FLOOR: f32 = -60.0;
/// The analyser's frequency axis.
const F_LO: f64 = 20.0;
const F_HI: f64 = 20_000.0;
/// Zoomed in no closer than this many columns, or this long (seconds).
const MIN_COLS: f64 = 16.0;
const MIN_SECS: f64 = 0.25;
/// A sampled sound zooms to this span (s), its detail analysed again as it does.
const DETAIL_MIN_SECS: f64 = 2.0;
/// Most columns of a detail.
const DETAIL_MAX_COLS: usize = 4096;

/// What the worker found.
struct Loaded {
    id: ImageId,
    analysis: Option<Arc<Analysis>>,
    loudness: Option<Arc<Report>>,
    lyrics: Option<Arc<Lyrics>>,
    /// The loudness alone, measured after the analysis was sent (a long file's loudness
    /// reads every sample; the page doesn't wait for it).
    late: bool,
    /// The analysis, with its loudness to follow as a `late` message.
    follows: bool,
}

/// A choice in the row at the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Chip {
    Visual(Visual),
    Curve,
    Lyrics,
}

/// What was drawn last (logical px), for the pointer and the accessibility tree.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub area: Rect,
    pub layout: PageLayout,
    pub chips: Vec<(Chip, Rect, bool)>,
    /// The lyrics lines drawn (index, where).
    pub lines: Vec<(usize, Rect)>,
    /// "Reading the sound…" or why it can't be read, where it shows.
    pub status: Option<(String, Rect)>,
}

/// What has the keyboard on the page: a choice in the row at the top (by its place in
/// the row), or a line of synced lyrics (by its index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
    Chip(usize),
    Line(usize),
}

/// What a key did on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyOut {
    /// The focus moved (or entered the page).
    Moved,
    /// Enter or Space on what has the focus.
    Activate(Focus),
    /// The focus left the page; whether the key is used up (Esc, Shift+Tab) or goes on
    /// to the chrome (Tab).
    Left { taken: bool },
}

/// The lyrics column's scrolling (logical px from the top of the lyrics).
#[derive(Clone, Copy, Debug, Default)]
struct LyricsScroll {
    /// Where it is, and where it is going (the current line's place).
    y: f32,
    to: f32,
    /// The wheel's offset from there (plain lyrics: the whole scroll).
    wheel: f32,
    line: Option<usize>,
    /// Jump there at the next draw (a new song, a seek, reduce motion).
    jump: bool,
}

/// The cover's colours, and the page colours made from them for a theme.
#[derive(Default)]
struct CoverColours {
    dominant: Vec<(Color, f32)>,
    /// The cover averaged down (for the scrim over the blurred cover).
    samples: Vec<[f32; 3]>,
    /// For the theme they were made for: the page's palette, and the one over the blurred
    /// cover with the scrim's alpha.
    made: Option<(Mode, Palette, (Palette, f32))>,
}

/// A press on the page: where, and whether it moved (a drag pans; a click seeks).
#[derive(Clone, Copy, Debug)]
struct Press {
    at: (f32, f32),
    last_x: f32,
    plot_w: f32,
    timeline: bool,
    moved: bool,
}

#[derive(Default)]
pub(super) struct SoundPage {
    id: Option<ImageId>,
    rx: Option<mpsc::Receiver<Loaded>>,
    cancel: Option<CancelToken>,
    pub(super) analysis: Option<Arc<Analysis>>,
    pub(super) loudness: Option<Arc<Report>>,
    /// The worker is done (with or without an analysis).
    done: bool,
    /// The whole-file loudness of a long sound is still being measured.
    measuring: bool,
    /// The cover to upload, and once uploaded.
    cover: Option<(u32, u32, Vec<u8>)>,
    cover_tex: Option<(wgpu::Texture, TexId, (u32, u32))>,
    feed: Option<Box<dyn Feed>>,
    /// The player's live tap and its sound delay, while the sound plays.
    tap: Option<(Arc<Tap>, Arc<AtomicI64>)>,
    snap: Snapshot,
    gonio: Option<Goniometer>,
    gonio_px: Vec<u8>,
    gonio_tex: Option<(wgpu::Texture, TexId)>,
    /// The picture changed since uploaded; seconds since it was last redrawn.
    gonio_dirty: bool,
    gonio_wait: f32,
    /// The panel colour the picture is drawn on (sRGB bytes).
    gonio_bg: [u8; 3],
    pcm: Vec<f32>,
    pub(super) lyrics: Option<Arc<Lyrics>>,
    /// The lyrics column is wanted (`viewing.sound_lyrics`).
    pub(super) show_lyrics: bool,
    scroll: LyricsScroll,
    colours: CoverColours,
    /// Settings: the page's colours from the cover, and the blurred cover behind it.
    pub(super) cover_colours: bool,
    pub(super) backdrop_on: bool,
    backdrop: Option<backdrop::Backdrop>,
    backdrop_tex: Option<TexId>,
    pub(super) ball: Ballistics,
    pub(super) visuals: Visuals,
    /// The setting the visuals were read from.
    visuals_from: Option<String>,
    pub(super) view: TimeView,
    /// The playing position (seconds) and whether it plays.
    pub(super) t: f64,
    playing: bool,
    last_t: Option<f64>,
    last_at: Option<Instant>,
    reduce: bool,
    gpu: Option<SoundGpu>,
    tex: Option<TexId>,
    /// The analysis on the GPU is this one.
    uploaded: Option<usize>,
    retired: Vec<TexId>,
    pub(super) drawn: Option<Drawn>,
    press: Option<Press>,
    /// The sound shown and the worker's wake, for detail asked for as the view zooms.
    session: Option<Arc<dyn Session>>,
    wake: Option<crate::loader::Wake>,
    /// A sampled sound zoomed in: the part in view analysed again, one window a column.
    detail: Option<Detail>,
    detail_job: Option<DetailJob>,
    /// The detail on the GPU is this one.
    detail_uploaded: Option<usize>,
    /// The lyrics' line heights (regular weight), for the lyrics, width and text size.
    lyric_heights: Option<((usize, u32, u32), Vec<f32>)>,
    /// What has the keyboard on the page (Tab from the picture enters the choices).
    pub(super) focus: Option<Focus>,
}

/// A part of a sampled sound analysed in more detail: from `from` for `span` (fractions of
/// its length).
struct Detail {
    from: f64,
    span: f64,
    analysis: Arc<Analysis>,
}

/// A detail being worked out.
struct DetailJob {
    from: f64,
    span: f64,
    cols: usize,
    rx: mpsc::Receiver<Option<Analysis>>,
    cancel: CancelToken,
}

/// Whether a part from `from` for `span` in `cols` columns shows the view `v` with at
/// least one column for every two of the `plot` px.
fn holds(from: f64, span: f64, cols: usize, v: &TimeView, plot: f64) -> bool {
    from <= v.start + 1e-9
        && v.start + v.span <= from + span + 1e-9
        && cols as f64 * v.span / span.max(1e-12) >= plot / 2.0
}

impl SoundPage {
    /// Showing the page of image `id`.
    pub(super) fn active(&self) -> bool {
        self.id.is_some()
    }

    pub(super) fn shows(&self, id: ImageId) -> bool {
        self.id == Some(id)
    }

    /// The loudness of image `id`, once measured.
    pub(super) fn loudness_of(&self, id: ImageId) -> Option<&Report> {
        self.loudness.as_deref().filter(|_| self.id == Some(id))
    }

    /// A key while the page shows (`entering`: nothing else has the keyboard, so Tab may
    /// enter the page). Tab enters the choices, then the synced lyrics, then leaves;
    /// Shift+Tab goes back; arrows move along the choices (as they read) and the lines;
    /// Home and End go to the ends; Enter or Space activates; Esc leaves. `None`: not
    /// the page's key.
    pub(super) fn key(
        &mut self,
        key: &Key,
        shift: bool,
        rtl: bool,
        entering: bool,
    ) -> Option<KeyOut> {
        let d = self.drawn.as_ref()?;
        let chips = d.chips.len();
        if chips == 0 {
            self.focus = None;
            return None;
        }
        // The lines a line focus walks: synced lyrics shown, the lines with words.
        let lines: Vec<usize> = match &self.lyrics {
            Some(l) if l.synced && self.lyrics_shown() => l
                .lines
                .iter()
                .enumerate()
                .filter(|(_, x)| !x.text.is_empty())
                .map(|(i, _)| i)
                .collect(),
            _ => Vec::new(),
        };
        // Lines by their place among `lines`; the first one at or after the sung line.
        let place = |i: usize| {
            lines
                .iter()
                .position(|&x| x >= i)
                .unwrap_or(lines.len().saturating_sub(1))
        };
        let first_line = place(self.scroll.line.unwrap_or(0));
        let tab = matches!(key, Key::Named(NamedKey::Tab));
        let Some(f) = self.focus else {
            if tab && !shift && entering && chips > 0 {
                self.focus = Some(Focus::Chip(0));
                return Some(KeyOut::Moved);
            }
            return None;
        };
        // What was focused may be gone (the row changed, the lyrics went).
        let f = match f {
            Focus::Chip(i) if i >= chips => Focus::Chip(chips.saturating_sub(1)),
            Focus::Line(i) if !lines.contains(&i) => Focus::Chip(0),
            f => f,
        };
        let (next, prev) = if rtl {
            (NamedKey::ArrowLeft, NamedKey::ArrowRight)
        } else {
            (NamedKey::ArrowRight, NamedKey::ArrowLeft)
        };
        let to = match (key, f) {
            (Key::Named(NamedKey::Tab), Focus::Chip(_)) if !shift => {
                if lines.is_empty() {
                    None
                } else {
                    Some(Focus::Line(lines[first_line]))
                }
            }
            (Key::Named(NamedKey::Tab), Focus::Line(_)) if shift => Some(Focus::Chip(0)),
            (Key::Named(NamedKey::Tab), Focus::Line(_)) => None,
            (Key::Named(NamedKey::Tab), Focus::Chip(_)) => {
                self.focus = None;
                return Some(KeyOut::Left { taken: true });
            }
            (Key::Named(NamedKey::Escape), _) => {
                self.focus = None;
                return Some(KeyOut::Left { taken: true });
            }
            (Key::Named(NamedKey::Enter | NamedKey::Space), f) => {
                self.focus = Some(f);
                return Some(KeyOut::Activate(f));
            }
            (Key::Named(k), Focus::Chip(i)) if *k == next => {
                Some(Focus::Chip((i + 1).min(chips - 1)))
            }
            (Key::Named(k), Focus::Chip(i)) if *k == prev => Some(Focus::Chip(i.saturating_sub(1))),
            (Key::Named(NamedKey::Home), Focus::Chip(_)) => Some(Focus::Chip(0)),
            (Key::Named(NamedKey::End), Focus::Chip(_)) => Some(Focus::Chip(chips - 1)),
            (Key::Named(NamedKey::ArrowDown), Focus::Line(i)) => {
                Some(Focus::Line(lines[(place(i) + 1).min(lines.len() - 1)]))
            }
            (Key::Named(NamedKey::ArrowUp), Focus::Line(i)) => {
                Some(Focus::Line(lines[place(i).saturating_sub(1)]))
            }
            (Key::Named(NamedKey::Home), Focus::Line(_)) => Some(Focus::Line(lines[0])),
            (Key::Named(NamedKey::End), Focus::Line(_)) => {
                Some(Focus::Line(lines[lines.len() - 1]))
            }
            _ => {
                self.focus = Some(f);
                return None;
            }
        };
        match to {
            Some(t) => {
                self.focus = Some(t);
                if matches!(t, Focus::Line(_)) {
                    // The lyrics scroll to keep the focused line in view.
                    self.scroll.wheel = 0.0;
                }
                Some(KeyOut::Moved)
            }
            None => {
                self.focus = None;
                Some(KeyOut::Left { taken: false })
            }
        }
    }

    /// The name of what has the keyboard, for the screen reader's status.
    pub(super) fn focus_name(&self) -> Option<String> {
        let d = self.drawn.as_ref()?;
        match self.focus? {
            Focus::Chip(i) => d.chips.get(i).map(|(c, _, _)| chip_name(*c)),
            Focus::Line(i) => self
                .lyrics
                .as_ref()
                .and_then(|l| l.lines.get(i))
                .map(|l| l.text.clone()),
        }
    }

    fn leave(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.cancel();
        }
        if let Some(j) = self.detail_job.take() {
            j.cancel.cancel();
        }
        let mut retired = std::mem::take(&mut self.retired);
        retired.extend(self.cover_tex.take().map(|c| c.1));
        retired.extend(self.tex.take());
        retired.extend(self.gonio_tex.take().map(|g| g.1));
        retired.extend(self.backdrop_tex.take());
        let gpu = self.gpu.take();
        let backdrop = self.backdrop.take();
        let (visuals, visuals_from) = (self.visuals, self.visuals_from.take());
        *self = SoundPage {
            retired,
            visuals,
            visuals_from,
            backdrop,
            show_lyrics: self.show_lyrics,
            cover_colours: self.cover_colours,
            backdrop_on: self.backdrop_on,
            ..SoundPage::default()
        };
        // The pipeline is kept; its data goes.
        self.gpu = gpu.map(|mut g| {
            g.clear();
            g
        });
    }

    /// The app opened a new GPU device: what was made on the old one goes and the page is
    /// entered again on the next frame (the session keeps its analysis and loudness).
    pub(super) fn gpu_reset(&mut self) {
        self.gpu = None;
        self.backdrop = None;
        self.cover_tex = None;
        self.gonio_tex = None;
        self.backdrop_tex = None;
        self.tex = None;
        self.uploaded = None;
        self.retired.clear();
        self.leave();
    }

    /// Starts the page of image `id`: its cover now, its analysis on a worker.
    fn enter(&mut self, id: ImageId, session: &Arc<dyn Session>, wake: crate::loader::Wake) {
        self.leave();
        self.id = Some(id);
        self.session = Some(Arc::clone(session));
        self.wake = Some(Arc::clone(&wake));
        let sound = session
            .as_any()
            .and_then(|a| a.downcast_ref::<SoundSession>());
        self.cover = sound
            .and_then(|s| s.cover())
            .map(|(w, h, px)| (w, h, px.to_vec()));
        let cancel = CancelToken::new();
        self.cancel = Some(cancel.clone());
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let s = Arc::clone(session);
        let spawned = specular_thread::builder("sound-page").spawn(move || {
            let Some(snd) = s.as_any().and_then(|a| a.downcast_ref::<SoundSession>()) else {
                let _ = tx.send(Loaded {
                    id,
                    analysis: None,
                    loudness: None,
                    lyrics: None,
                    late: false,
                    follows: false,
                });
                wake();
                return;
            };
            // The lyrics first: a sidecar is a file read.
            let lyrics = snd.lyrics();
            let analysis = snd.analysis(&cancel).ok().flatten();
            if cancel.is_cancelled() {
                return;
            }
            // Short sound: the loudness came with the analysis. Long sound: shown first,
            // its loudness follows.
            let ready = snd.duration_us() <= EXACT_MAX_US;
            let loudness = if ready {
                snd.loudness(&cancel).ok().flatten()
            } else {
                None
            };
            let _ = tx.send(Loaded {
                id,
                analysis,
                loudness,
                lyrics,
                late: false,
                follows: !ready,
            });
            wake();
            if ready {
                return;
            }
            let loudness = snd.loudness(&cancel).ok().flatten();
            if cancel.is_cancelled() {
                return;
            }
            let _ = tx.send(Loaded {
                id,
                analysis: None,
                loudness,
                lyrics: None,
                late: true,
                follows: false,
            });
            wake();
        });
        if spawned.is_err() {
            self.done = true;
        }
    }

    /// Takes the worker's result; whether it came.
    fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok(l) if Some(l.id) == self.id && l.late => {
                self.rx = None;
                self.measuring = false;
                self.loudness = l.loudness;
                // The live numbers and the loudness history take it.
                self.make_feed();
                self.uploaded = None;
                true
            }
            Ok(l) if Some(l.id) == self.id => {
                self.done = true;
                self.lyrics = l.lyrics;
                self.scroll = LyricsScroll {
                    jump: true,
                    ..LyricsScroll::default()
                };
                self.set_data(l.analysis, l.loudness);
                self.measuring = l.follows;
                true
            }
            Ok(_) => false,
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                self.done = true;
                self.measuring = false;
                true
            }
        }
    }

    /// The data shown (also for screenshots).
    pub(super) fn set_data(
        &mut self,
        analysis: Option<Arc<Analysis>>,
        loudness: Option<Arc<Report>>,
    ) {
        self.analysis = analysis;
        self.loudness = loudness;
        self.make_feed();
        self.done = true;
        self.last_t = None;
        self.ball = Ballistics::default();
    }

    /// The live displays' input: the live tap while there is one (the analysis behind it),
    /// else the analysis.
    fn make_feed(&mut self) {
        self.feed = self.analysis.as_ref().map(|a| {
            let fallback = AnalysisFeed {
                analysis: Arc::clone(a),
                loudness: self.loudness.clone(),
            };
            match &self.tap {
                Some((tap, delay)) => {
                    let mut f = LiveFeed::new(Arc::clone(tap), fallback);
                    f.delay_us = Some(Arc::clone(delay));
                    Box::new(f) as Box<dyn Feed>
                }
                None => Box::new(fallback) as Box<dyn Feed>,
            }
        });
    }

    /// The player's live tap (and its sound delay), or none.
    pub(super) fn set_tap(&mut self, tap: Option<(Arc<Tap>, Arc<AtomicI64>)>) {
        let same = match (&self.tap, &tap) {
            (Some(a), Some(b)) => Arc::ptr_eq(&a.0, &b.0) && Arc::ptr_eq(&a.1, &b.1),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.tap = tap;
            self.make_feed();
        }
    }

    /// The cover's dominant colours and its averaged samples (once a cover).
    fn set_cover_colours(&mut self) {
        self.colours = match &self.cover {
            Some((w, h, px)) => CoverColours {
                dominant: cover::dominant(*w, *h, px),
                samples: cover::samples(*w, *h, px, 16),
                made: None,
            },
            None => CoverColours::default(),
        };
    }

    /// The page's palette for theme `base`, and the scrim's alpha over the backdrop
    /// (`None`: no backdrop drawn).
    fn palette(&mut self, base: &Palette) -> (Palette, Option<f32>) {
        let backdrop = self.backdrop_on && self.backdrop_tex.is_some();
        if (!self.cover_colours && !backdrop) || self.colours.dominant.is_empty() {
            return (base.clone(), None);
        }
        let stale = self.colours.made.as_ref().is_none_or(|m| m.0 != base.mode);
        if stale {
            let p = if self.cover_colours {
                cover::themed(base, &self.colours.dominant)
            } else {
                base.clone()
            };
            // Over the backdrop: the scrim and text colours that keep it readable.
            let (q, a) = cover::over_backdrop(&p, &self.colours.samples);
            self.colours.made = Some((base.mode, p, (q, a)));
        }
        let Some((_, p, (q, a))) = &self.colours.made else {
            return (base.clone(), None);
        };
        if backdrop {
            (q.clone(), Some(*a))
        } else {
            (p.clone(), None)
        }
    }

    /// Settings that change the page's looks.
    pub(super) fn set_looks(&mut self, cover_colours: bool, backdrop: bool, lyrics: bool) {
        if (cover_colours, backdrop) != (self.cover_colours, self.backdrop_on) {
            self.colours.made = None;
        }
        self.cover_colours = cover_colours;
        self.backdrop_on = backdrop;
        if lyrics != self.show_lyrics {
            self.scroll.jump = true;
        }
        self.show_lyrics = lyrics;
    }

    /// The lyrics column shows.
    fn lyrics_shown(&self) -> bool {
        self.show_lyrics && self.lyrics.is_some()
    }

    fn duration(&self) -> f64 {
        self.analysis.as_ref().map_or(0.0, |a| a.duration_s)
    }

    /// The playing position as a fraction of the length.
    fn played(&self) -> f64 {
        let d = self.duration();
        if d > 0.0 {
            (self.t / d).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Moves on to position `t` (seconds) at `now`: the view follows the playhead, the live
    /// displays take the sound there.
    pub(super) fn tick(&mut self, t: f64, playing: bool, now: Instant, reduce: bool) {
        self.reduce = reduce;
        self.playing = playing;
        self.t = t;
        if playing {
            let p = self.played();
            self.view.follow(p, reduce);
        }
        if let Some(feed) = &self.feed
            && feed.snapshot(t, &mut self.snap)
        {
            let dt = self
                .last_at
                .map_or(0.0, |a| now.saturating_duration_since(a).as_secs_f32());
            let jumped = self
                .last_t
                .is_none_or(|lt| (t - lt).abs() > 0.5 || (!playing && t != lt));
            if jumped {
                self.ball.jump(&self.snap);
                self.scroll.jump = true;
            } else if playing {
                self.ball.update(&self.snap, dt, &Rates::new(reduce));
                self.tick_gonio(t, dt, reduce);
            }
        }
        // The lyrics follow the line sung.
        if let Some(l) = &self.lyrics {
            let line = l.line_at(t);
            if line != self.scroll.line {
                self.scroll.line = line;
                self.scroll.wheel = 0.0;
                if !playing || reduce {
                    self.scroll.jump = true;
                }
            }
        }
        let dt = self
            .last_at
            .map_or(0.0, |a| now.saturating_duration_since(a).as_secs_f32());
        if !self.scroll.jump {
            let k = 1.0 - (-dt / 0.15).exp();
            self.scroll.y += (self.scroll.to - self.scroll.y) * k;
        }
        self.last_t = Some(t);
        self.last_at = Some(now);
    }

    /// The goniometer takes the sound heard since the last frame.
    fn tick_gonio(&mut self, t: f64, dt: f32, reduce: bool) {
        let shown = self.visuals.has(Visual::Stereo)
            && self.analysis.as_ref().is_some_and(|a| a.lanes == 2);
        let Some(feed) = self.feed.as_ref().filter(|_| shown) else {
            return;
        };
        let g = self.gonio.get_or_insert_with(|| Goniometer::new(GONIO_PX));
        self.gonio_wait += dt;
        if self.gonio_wait < stereo::redraw_every(reduce) {
            return;
        }
        let span = self.gonio_wait;
        self.gonio_wait = 0.0;
        let frames = ((f64::from(span) * 48_000.0) as usize).clamp(256, 8192);
        g.fade(span, stereo::persistence(reduce));
        if let Some(ch) = feed.samples(t, frames, &mut self.pcm) {
            g.add(&self.pcm, ch, span);
        }
        self.gonio_dirty = true;
    }

    /// Releases what went, uploads what came, and sizes the target for `area_px`
    /// (physical px).
    pub(super) fn prepare(&mut self, gpu: &Gpu, ui: &mut UiRenderer, area_px: (u32, u32)) {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        if self.id.is_none() {
            return;
        }
        if self.cover.is_some() {
            self.set_cover_colours();
        }
        if let Some((w, h, px)) = self.cover.take() {
            let (tex, id) = upload_rgba(gpu, ui, w, h, &px);
            // The backdrop: the cover blurred, once a cover.
            let b = self
                .backdrop
                .get_or_insert_with(|| backdrop::Backdrop::new(gpu));
            let view = b.set_cover(gpu, &tex.create_view(&Default::default()));
            match self.backdrop_tex {
                Some(t) => ui.update_texture(gpu, t, view),
                None => self.backdrop_tex = Some(ui.register_texture(gpu, view)),
            }
            self.cover_tex = Some((tex, id, (w, h)));
        }
        // The goniometer's picture, when it changed.
        if let Some(g) = self.gonio.as_ref().filter(|_| self.gonio_dirty) {
            self.gonio_dirty = false;
            let light = self.gonio_bg.iter().map(|&c| u32::from(c)).sum::<u32>() > 3 * 128;
            let trace = if light {
                GONIO_TRACE_LIGHT
            } else {
                GONIO_TRACE
            };
            g.rgba(self.gonio_bg, trace, &mut self.gonio_px);
            let n = g.size() as u32;
            match &self.gonio_tex {
                Some((tex, _)) => write_rgba(gpu, tex, n, n, &self.gonio_px),
                None => self.gonio_tex = Some(upload_rgba(gpu, ui, n, n, &self.gonio_px)),
            }
        }
        let Some(a) = self.analysis.clone() else {
            return;
        };
        let g = self.gpu.get_or_insert_with(|| SoundGpu::new(gpu));
        let key = Arc::as_ptr(&a) as usize;
        if self.uploaded != Some(key) {
            g.upload(gpu, &a, self.loudness.as_deref());
            self.uploaded = Some(key);
            self.detail_uploaded = None;
        }
        let detail = self
            .detail
            .as_ref()
            .map(|d| Arc::as_ptr(&d.analysis) as usize);
        if self.detail_uploaded != detail {
            g.upload_detail(
                gpu,
                self.detail.as_ref().map(|d| (&*d.analysis, d.from, d.span)),
            );
            self.detail_uploaded = detail;
        }
        // The plot's width (physical px), for the detail a zoomed sampled sound needs.
        let plot = self.drawn.as_ref().and_then(|d| {
            let r = d.layout.spectrogram.or(d.layout.waveform)?;
            Some(f64::from(r.w * area_px.0 as f32 / d.area.w.max(1.0)))
        });
        if let Some(plot) = plot {
            self.want_detail(plot);
        }
        let Some(g) = self.gpu.as_mut() else { return };
        if g.ensure_target(gpu, area_px)
            && let Some(view) = g.view()
        {
            match self.tex {
                Some(t) => ui.update_texture(gpu, t, view),
                None => self.tex = Some(ui.register_texture(gpu, view)),
            }
        }
    }

    /// Draws this frame's time-based panels (after [`draw`], before the UI pass).
    pub(super) fn render(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder) {
        if let Some(b) = &mut self.backdrop {
            b.render(enc);
        }
        if let Some(g) = &mut self.gpu {
            g.render(gpu, enc);
        }
    }

    /// The least span the time view zooms to.
    fn min_span(&self) -> f64 {
        let Some(a) = &self.analysis else { return 1.0 };
        if !a.exact {
            // Sampled: the part in view is analysed again as it zooms ([`Self::want_detail`]).
            return (DETAIL_MIN_SECS / a.duration_s.max(1e-3)).min(1.0);
        }
        (MIN_COLS / a.cols.max(1) as f64).max(MIN_SECS / a.duration_s.max(1e-3))
    }

    /// A sampled sound zoomed in past its columns: asks for the part in view (and half a
    /// view each side) analysed again with a column for every two of the `plot` px. The
    /// whole sound's columns show until it comes.
    fn want_detail(&mut self, plot: f64) {
        let Some(a) = self.analysis.as_ref().filter(|a| !a.exact) else {
            return;
        };
        let v = self.view;
        let enough = v.span * a.cols as f64 >= plot / 2.0
            || self
                .detail
                .as_ref()
                .is_some_and(|d| holds(d.from, d.span, d.analysis.cols, &v, plot));
        if enough {
            if let Some(j) = self.detail_job.take() {
                j.cancel.cancel();
            }
            return;
        }
        if self
            .detail_job
            .as_ref()
            .is_some_and(|j| holds(j.from, j.span, j.cols, &v, plot))
        {
            return;
        }
        let (Some(s), Some(wake)) = (self.session.clone(), self.wake.clone()) else {
            return;
        };
        if let Some(j) = self.detail_job.take() {
            j.cancel.cancel();
        }
        let span = (v.span * 2.0).min(1.0);
        let from = (v.start - v.span / 2.0).clamp(0.0, 1.0 - span);
        let cols = ((plot * 2.0) as usize).clamp(64, DETAIL_MAX_COLS);
        let length_us = a.duration_s * 1e6;
        let (tx, rx) = mpsc::channel();
        let cancel = CancelToken::new();
        let c = cancel.clone();
        let spawned = specular_thread::builder("sound-detail").spawn(move || {
            let Some(snd) = s.as_any().and_then(|a| a.downcast_ref::<SoundSession>()) else {
                return;
            };
            let a = snd
                .analysis_of(
                    (from * length_us) as u64,
                    (span * length_us) as u64,
                    cols,
                    &c,
                )
                .ok()
                .flatten();
            if !c.is_cancelled() {
                let _ = tx.send(a);
                wake();
            }
        });
        if spawned.is_ok() {
            self.detail_job = Some(DetailJob {
                from,
                span,
                cols,
                rx,
                cancel,
            });
        }
    }

    /// Takes a detail that came; whether one did (to draw again).
    pub(super) fn poll_detail(&mut self) -> bool {
        let Some(j) = &self.detail_job else {
            return false;
        };
        match j.rx.try_recv() {
            Ok(a) => {
                let (from, span) = (j.from, j.span);
                self.detail_job = None;
                if let Some(a) = a {
                    self.detail = Some(Detail {
                        from,
                        span,
                        analysis: Arc::new(a),
                    });
                }
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.detail_job = None;
                false
            }
        }
    }

    fn set_visuals(&mut self, v: Visuals) -> String {
        self.visuals = v;
        let s = v.format();
        self.visuals_from = Some(s.clone());
        s
    }
}

impl App {
    /// The sound page follows the current image and the playing position (each frame,
    /// before drawing).
    pub(super) fn sound_sync(&mut self) {
        let cur = self.current_id().filter(|_| !self.gallery_active());
        let session = cur
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone())
            .filter(|s| s.video().is_some_and(|v| v.sound.is_some()));
        let (Some(id), Some(session)) = (cur, session) else {
            if self.sound_page.active() {
                self.sound_page.leave();
            }
            return;
        };
        if !self.sound_page.shows(id) {
            let wake = self.loader.waker();
            self.sound_page.enter(id, &session, wake);
        }
        if self.sound_page.visuals_from.as_deref() != Some(&self.settings.viewing.sound_visuals) {
            let v = Visuals::parse(&self.settings.viewing.sound_visuals);
            self.sound_page.visuals = v;
            self.sound_page.visuals_from = Some(self.settings.viewing.sound_visuals.clone());
        }
        if self.sound_page.poll() {
            // The loudness joins the Info panel.
            self.video_patch(id);
            self.redraw = true;
        }
        if self.sound_page.poll_detail() {
            self.redraw = true;
        }
        let now = Instant::now();
        let (t, playing) = self
            .anim
            .as_ref()
            .filter(|a| a.id == id)
            .map_or((0.0, false), |a| {
                (a.time(now).map_or(0.0, |d| d.as_secs_f64()), a.is_playing())
            });
        // What is heard, from the player's live tap (with the sound's delay).
        let tap = self
            .anim
            .as_ref()
            .filter(|a| a.id == id)
            .and_then(|a| a.movie.as_ref())
            .and_then(|m| m.sound.as_ref().map(|s| (s.tap(), Arc::clone(&m.delay_us))));
        self.sound_page.set_tap(tap);
        let v = &self.settings.viewing;
        self.sound_page
            .set_looks(v.sound_cover_colours, v.sound_backdrop, v.sound_lyrics);
        let reduce = self.reduce_motion();
        self.sound_page.tick(t, playing, now, reduce);
        // The playhead and the live displays move every frame while it plays.
        if playing && self.sound_page.analysis.is_some() {
            self.redraw = true;
        }
    }

    pub(super) fn sound_page_active(&self) -> bool {
        self.sound_page.active() && !self.gallery_active()
    }

    fn sound_store_visuals(&mut self, v: Visuals) {
        let s = self.sound_page.set_visuals(v);
        self.store
            .update(Origin::Program, |st| st.viewing.sound_visuals = s);
        self.redraw = true;
    }

    /// The lyrics column on or off (the setting).
    fn sound_toggle_lyrics(&mut self) {
        let on = !self.settings.viewing.sound_lyrics;
        self.store
            .update(Origin::Program, |st| st.viewing.sound_lyrics = on);
        self.sound_page.show_lyrics = on;
        self.sound_page.scroll.jump = true;
        self.redraw = true;
    }

    /// The "next sound visuals" and "lyrics" keys.
    pub(super) fn sound_action(&mut self, action: Action) -> bool {
        if !self.sound_page_active() {
            return false;
        }
        match action {
            Action::CycleSoundVisuals => {
                let v = self.sound_page.visuals.next();
                self.sound_store_visuals(v);
            }
            Action::ToggleLyrics => self.sound_toggle_lyrics(),
            _ => return false,
        }
        true
    }

    /// Plays from line `i` of the lyrics (synced ones).
    fn sound_seek_line(&mut self, i: usize) {
        let at = self
            .sound_page
            .lyrics
            .as_ref()
            .filter(|l| l.synced)
            .and_then(|l| l.lines.get(i))
            .and_then(|l| l.at);
        let d = self.sound_page.duration();
        let Some(at) = at.filter(|_| d > 0.0) else {
            return;
        };
        self.sound_page.view.follow = true;
        self.sound_page.scroll.wheel = 0.0;
        self.video_seek((at / d).clamp(0.0, 1.0), true);
        if let Some(a) = self.anim.as_mut()
            && !a.is_playing()
        {
            a.toggle(Instant::now());
        }
        self.tick_animation();
        self.redraw = true;
    }

    /// A key on the page: Tab from the picture enters the choices and the synced lyrics
    /// (see [`SoundPage::key`]); Enter or Space toggles a choice or plays from a line.
    /// Whether the page took the key.
    pub(super) fn sound_key(&mut self, c: &specular_input::KeyCombo) -> bool {
        if !self.sound_page_active() {
            self.sound_page.focus = None;
            return false;
        }
        if c.mods.ctrl || c.mods.alt || c.mods.meta {
            return false;
        }
        let rtl = self.rtl();
        let entering = !self.chrome.has_keyboard_focus() && self.keymap.lookup(c).is_none();
        let Some(out) = self.sound_page.key(&c.key, c.mods.shift, rtl, entering) else {
            return false;
        };
        self.redraw = true;
        match out {
            KeyOut::Moved => {
                if let Some(name) = self.sound_page.focus_name() {
                    self.chrome.announce_status(name);
                }
                true
            }
            KeyOut::Activate(Focus::Line(i)) => {
                self.sound_seek_line(i);
                true
            }
            KeyOut::Activate(Focus::Chip(i)) => {
                let req = accesskit::ActionRequest {
                    action: accesskit::Action::Click,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: accesskit::NodeId(A11Y_CHIP + i as u64),
                    data: None,
                };
                self.sound_a11y_request(&req);
                true
            }
            KeyOut::Left { taken } => taken,
        }
    }

    /// A press at `lp` (logical px) on the page: a choice toggles, the meters' clip lights
    /// clear, a time-based plot starts a click or a drag. Whether the page took it.
    pub(super) fn sound_press(&mut self, lp: (f32, f32)) -> bool {
        if !self.sound_page_active() {
            return false;
        }
        let Some(d) = self.sound_page.drawn.clone() else {
            return false;
        };
        if !d.area.contains(lp) {
            return false;
        }
        if let Some((chip, _, _)) = d.chips.iter().find(|c| c.1.contains(lp)) {
            let v = self.sound_page.visuals;
            let v = match chip {
                Chip::Visual(x) => v.toggled(*x),
                Chip::Curve => v.with_curve(!v.curve),
                Chip::Lyrics => {
                    self.sound_toggle_lyrics();
                    return true;
                }
            };
            self.sound_store_visuals(v);
            return true;
        }
        if d.layout.lyrics.is_some_and(|r| r.contains(lp)) {
            if let Some(&(i, _)) = d.lines.iter().find(|(_, r)| r.contains(lp)) {
                self.sound_seek_line(i);
            }
            return true;
        }
        if d.layout.meters.is_some_and(|m| m.contains(lp)) {
            self.sound_page.ball.clear_clips();
            self.redraw = true;
            return true;
        }
        let plot = d.layout.timelines().find(|a| a.contains(lp));
        self.sound_page.press = Some(Press {
            at: lp,
            last_x: lp.0,
            plot_w: plot.map_or(1.0, |a| a.w),
            timeline: plot.is_some(),
            moved: false,
        });
        true
    }

    /// The pointer moved to `p` (physical px): a drag on a zoomed plot pans it.
    pub(super) fn sound_motion(&mut self, p: (f32, f32)) -> bool {
        let lp = self.logical(p);
        let page = &mut self.sound_page;
        let Some(press) = &mut page.press else {
            return false;
        };
        if (lp.0 - press.at.0).abs() > 4.0 || (lp.1 - press.at.1).abs() > 4.0 {
            press.moved = true;
        }
        if press.moved && press.timeline && page.view.zoomed() {
            let dx = lp.0 - press.last_x;
            page.view.pan(-f64::from(dx / press.plot_w.max(1.0)));
            self.redraw = true;
        }
        press.last_x = lp.0;
        true
    }

    /// Let go at `lp`: a click on a time-based plot plays from there.
    pub(super) fn sound_release(&mut self, lp: (f32, f32)) -> bool {
        let Some(press) = self.sound_page.press.take() else {
            return false;
        };
        if press.moved || !press.timeline {
            return true;
        }
        let Some(d) = &self.sound_page.drawn else {
            return true;
        };
        if let Some(f) = d.layout.time_at(press.at, &self.sound_page.view) {
            self.sound_page.view.follow = true;
            self.video_seek(f, true);
            if let Some(a) = self.anim.as_mut()
                && !a.is_playing()
            {
                a.toggle(Instant::now());
            }
            self.tick_animation();
        }
        let _ = lp;
        self.redraw = true;
        true
    }

    /// The wheel over the page: over a time-based plot it zooms in time around the pointer.
    /// Whether the page took it.
    pub(super) fn sound_wheel(&mut self, dy: f32, pixels: bool, lp: (f32, f32)) -> bool {
        if !self.sound_page_active() {
            return false;
        }
        let Some(d) = &self.sound_page.drawn else {
            return false;
        };
        if !d.area.contains(lp) {
            return false;
        }
        if d.layout.lyrics.is_some_and(|r| r.contains(lp)) {
            let px = if pixels { dy } else { dy * 40.0 };
            self.sound_page.scroll.wheel -= px;
            self.redraw = true;
            return true;
        }
        if let Some(a) = d.layout.timelines().find(|a| a.contains(lp)) {
            let x = f64::from((lp.0 - a.x) / a.w.max(1.0));
            let step = if pixels {
                f64::from(dy) / 3.0
            } else {
                f64::from(dy)
            };
            let min = self.sound_page.min_span();
            self.sound_page.view.zoom(1.25f64.powf(step), x, min);
            self.redraw = true;
        }
        true
    }

    /// A screen reader pressed one of the choices, or a line of the lyrics.
    pub(super) fn sound_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let n = req.target_node.0;
        if !self.sound_page_active() {
            return false;
        }
        let focus = req.action == accesskit::Action::Focus;
        if (A11Y_LYRICS + 1..A11Y_LYRICS + 0x10000).contains(&n) {
            let i = (n - A11Y_LYRICS - 1) as usize;
            if focus {
                self.sound_page.focus = Some(Focus::Line(i));
                self.redraw = true;
            } else {
                self.sound_seek_line(i);
            }
            return true;
        }
        if !(A11Y_CHIP..A11Y_CHIP + 8).contains(&n) {
            return false;
        }
        if focus {
            self.sound_page.focus = Some(Focus::Chip((n - A11Y_CHIP) as usize));
            self.redraw = true;
            return true;
        }
        let Some((chip, _, _)) = self
            .sound_page
            .drawn
            .as_ref()
            .and_then(|d| d.chips.get((n - A11Y_CHIP) as usize).cloned())
        else {
            return false;
        };
        let v = self.sound_page.visuals;
        let v = match chip {
            Chip::Visual(x) => v.toggled(x),
            Chip::Curve => v.with_curve(!v.curve),
            Chip::Lyrics => {
                self.sound_toggle_lyrics();
                return true;
            }
        };
        self.sound_store_visuals(v);
        true
    }
}

fn rect(a: Area) -> Rect {
    Rect::new(a.x, a.y, a.w, a.h)
}

/// A choice's name.
fn chip_name(c: Chip) -> String {
    match c {
        Chip::Visual(v) => visual_name(v),
        Chip::Curve => t!("sound-visual-curve"),
        Chip::Lyrics => t!("sound-visual-lyrics"),
    }
}

/// A visual's name.
fn visual_name(v: Visual) -> String {
    match v {
        Visual::Waveform => t!("sound-visual-waveform"),
        Visual::Spectrogram => t!("sound-visual-spectrogram"),
        Visual::Spectrum => t!("sound-visual-spectrum"),
        Visual::Meters => t!("sound-visual-meters"),
        Visual::Loudness => t!("sound-visual-loudness"),
        Visual::Stereo => t!("sound-visual-stereo"),
    }
}

/// Channel `i` of `n`'s short name, by its speaker (the WAVE order of
/// `specular_video::speakers::wave_layout`, the table playback's fold reads too); past
/// eight channels, its number.
fn channel_name(i: usize, n: usize) -> String {
    use specular_video::speakers::{Speaker, wave_layout};
    if n == 1 {
        return t!("sound-channel-mono");
    }
    match wave_layout(n).and_then(|l| l.get(i)) {
        Some(Speaker::L) => t!("sound-channel-left"),
        Some(Speaker::R) => t!("sound-channel-right"),
        Some(Speaker::C) => t!("sound-channel-centre"),
        Some(Speaker::Lfe) => t!("sound-channel-lfe"),
        Some(Speaker::Ls) => t!("sound-channel-left-surround"),
        Some(Speaker::Rs) => t!("sound-channel-right-surround"),
        Some(Speaker::Lb) => t!("sound-channel-left-back"),
        Some(Speaker::Rb) => t!("sound-channel-right-back"),
        Some(Speaker::Cb) => t!("sound-channel-back-centre"),
        None => specular_i18n::I18n::global()
            .localizer()
            .number_fixed((i + 1) as f64, 0),
    }
}

/// A number with `digits` decimals in the UI language and its unit, kept together; a
/// negative one (levels in dB, LUFS) with a true minus.
fn quantity(v: f64, digits: u8, unit: &str) -> String {
    let n = specular_i18n::I18n::global()
        .localizer()
        .number_fixed(v.abs(), digits);
    let zero = n.chars().all(|c| !c.is_ascii_digit() || c == '0');
    let n = specular_i18n::signed_number(&n, v < 0.0 && !zero, false);
    specular_i18n::quantity(&n, unit)
}

/// A level for the numbers: its value, or a dash when there is none (silence).
fn reading(v: Option<f64>, unit: &str) -> String {
    match v.filter(|v| v.is_finite()) {
        Some(v) => quantity(v, 1, unit),
        None => t!("sound-no-value"),
    }
}

/// A frequency label: "50", "500", "5k".
fn hz_label(f: f64) -> String {
    if f >= 1000.0 {
        let k = specular_i18n::I18n::global()
            .localizer()
            .number(f / 1000.0, 1);
        t!("sound-axis-khz", value = k)
    } else {
        specular_i18n::I18n::global().localizer().number(f, 0)
    }
}

/// Which axis labels to show, given each one's (frequency, left edge, width): the two ends
/// first, then powers of ten, then the rest, each only when it keeps `gap` clear of every
/// label already shown.
fn fit_labels(spans: &[(f64, f32, f32)], gap: f32) -> Vec<bool> {
    let rank = |i: usize| -> u8 {
        let f = spans[i].0;
        if i == 0 || i + 1 == spans.len() {
            0
        } else if (f.log10() - f.log10().round()).abs() < 1e-9 {
            1
        } else if (f / 10f64.powf(f.log10().floor()) - 5.0).abs() < 1e-9 {
            2
        } else {
            3
        }
    };
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| rank(i));
    let mut show = vec![false; spans.len()];
    for i in order {
        let (_, x, w) = spans[i];
        let clear = spans
            .iter()
            .zip(&show)
            .filter(|(_, s)| **s)
            .all(|((_, ox, ow), _)| x + w + gap <= *ox || ox + ow + gap <= x);
        show[i] = clear;
    }
    show
}

/// A time label for a step of `step` seconds.
fn time_label(t: f64, step: f64) -> String {
    let c = super::playback::clock(t);
    if step < 1.0 {
        let tenths = ((t.fract() * 10.0).round() as u32).min(9);
        format!("{c}.{tenths}")
    } else {
        c
    }
}

/// The position of `f` Hz across the analyser (0…1).
fn freq_x(f: f64) -> f32 {
    ((f.max(1.0) / F_LO).log10() / (F_HI / F_LO).log10()) as f32
}

fn lin(c: Color) -> [f32; 4] {
    c.to_linear_premul()
}

/// Draws the page of `page` into `area` (logical px; `scale` physical px a logical px).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    base: &Palette,
    page: &mut SoundPage,
    area: Rect,
    rtl: bool,
    ts: f32,
    scale: f32,
) {
    // The page's colours (from the cover, when it has one and that is on), and the blurred
    // cover behind it under a scrim of the canvas colour.
    let (themed, scrim) = page.palette(base);
    let pal = &themed;
    dl.rect(area, 0.0, pal.canvas);
    if let (Some(a), Some(id), Some((_, _, (cw, ch)))) = (scrim, page.backdrop_tex, &page.cover_tex)
    {
        dl.image(area, fill_uv(area, *cw, *ch), id, 0.0);
        dl.rect(area, 0.0, pal.canvas.with_alpha(a));
    }
    let bg = [pal.surface.r, pal.surface.g, pal.surface.b].map(|c| (c * 255.0).round() as u8);
    if bg != page.gonio_bg {
        page.gonio_bg = bg;
        page.gonio_dirty = page.gonio.is_some();
    }
    let small = TextStyle::new(FONT_SMALL * ts);
    let channels = page.analysis.as_ref().map_or(2, |a| a.channels);
    let stereo_sound = page.analysis.as_ref().is_none_or(|a| a.lanes == 2);
    let v = page.visuals;
    let layout = PageLayout::new(
        Area::new(area.x, area.y, area.w, area.h),
        v,
        page.cover_tex.is_some() || page.cover.is_some(),
        if stereo_sound { channels.max(2) } else { 1 },
        rtl,
        page.lyrics_shown(),
    );
    dl.push_clip(area);
    // The choices (the stereo panel only for stereo sound; lyrics when there are some).
    let mut chips = Vec::new();
    let mut items: Vec<(Chip, String, bool)> = Visual::ALL
        .iter()
        .filter(|&&x| x != Visual::Stereo || stereo_sound)
        .map(|&x| (Chip::Visual(x), visual_name(x), v.has(x)))
        .collect();
    if v.has(Visual::Spectrum) {
        items.push((Chip::Curve, t!("sound-visual-curve"), v.curve));
    }
    if page.lyrics.is_some() {
        items.push((Chip::Lyrics, t!("sound-visual-lyrics"), page.show_lyrics));
    }
    let bar = layout.bar;
    let mut x = 0.0;
    for (chip, label, on) in items {
        let b = text.layout(&label, small, None, Align::Start);
        let w = b.width + 20.0 * ts;
        let h = (b.height + 10.0 * ts).min(bar.h);
        let cx = if rtl { bar.right() - x - w } else { bar.x + x };
        let r = Rect::new(cx, bar.y + (bar.h - h) / 2.0, w, h);
        if on {
            dl.rect_border(r, h / 2.0, pal.accent_soft, pal.accent, 1.0);
        } else {
            dl.stroke_rect(r, h / 2.0, pal.separator, 1.0);
        }
        let colour = if on { pal.text } else { pal.text_muted };
        text.draw(
            dl,
            &b,
            r.x + (w - b.width) / 2.0,
            r.y + (h - b.height) / 2.0,
            colour,
        );
        if page.focus == Some(Focus::Chip(chips.len())) {
            let g = r.inset(-3.0);
            dl.stroke_rect(g, g.h / 2.0, pal.focus_ring, 2.0);
        }
        chips.push((chip, r, on));
        x += w + 6.0 * ts;
    }
    // The cover.
    if let (Some(c), Some((_, id, (cw, ch)))) = (layout.cover, &page.cover_tex) {
        let s = (c.w / *cw as f32).min(c.h / *ch as f32);
        let (w, h) = (*cw as f32 * s, *ch as f32 * s);
        let r = Rect::new(c.x + (c.w - w) / 2.0, c.y + (c.h - h) / 2.0, w, h);
        dl.image(r, [0.0, 0.0, 1.0, 1.0], *id, RADIUS_SM);
    }
    let lines = match layout.lyrics {
        Some(r) => draw_lyrics(dl, text, pal, page, r, ts),
        None => Vec::new(),
    };
    page.drawn = Some(Drawn {
        area,
        layout,
        chips,
        lines,
        status: None,
    });
    let Some(a) = page.analysis.clone() else {
        // Reading the sound, or it can't be read.
        let msg = if page.done {
            t!("sound-page-unreadable")
        } else {
            t!("sound-page-reading")
        };
        let b = text.layout(
            &msg,
            TextStyle::new(FONT_SMALL * 1.1 * ts),
            Some(area.w * 0.8),
            Align::Center,
        );
        let under = layout
            .cover
            .map_or(area.y + area.h / 2.0, |c| c.bottom() + 24.0);
        let at = Rect::new(area.x + (area.w - b.width) / 2.0, under, b.width, b.height);
        text.draw(dl, &b, at.x, at.y, pal.text_muted);
        if let Some(d) = page.drawn.as_mut() {
            d.status = Some((msg, at));
        }
        dl.pop_clip();
        return;
    };
    // The time-based panels: queued for the GPU, drawn into the page's texture.
    let played = page.played();
    let look = Look {
        bg: lin(pal.surface),
        axis: lin(pal.separator),
        wave: lin(WAVE),
        rms: lin(WAVE_RMS),
        played: lin(PLAYED),
        played_rms: lin(PLAYED_RMS),
        mark: lin(pal.text),
        start: page.view.start,
        span: page.view.span,
        played_to: (played > 0.0).then_some(played),
    };
    let to_px = |r: Area| {
        [
            ((r.x - area.x) * scale).round().max(0.0) as u32,
            ((r.y - area.y) * scale).round().max(0.0) as u32,
            (r.w * scale).round().max(0.0) as u32,
            (r.h * scale).round().max(0.0) as u32,
        ]
    };
    let lane_gap = 6.0 * scale;
    if let (Some(g), Some(tex)) = (&mut page.gpu, page.tex)
        && g.has_data()
    {
        for (r, kind) in [
            (layout.waveform, Kind::Waveform),
            (layout.spectrogram, Kind::Spectrogram),
            (layout.history, Kind::History),
        ] {
            if let Some(r) = r {
                g.panel(kind, to_px(r), &look, lane_gap);
            }
        }
        if layout.timelines().next().is_some() {
            dl.image(area, [0.0, 0.0, 1.0, 1.0], tex, 0.0);
        }
    }
    let gutter_x = |r: Area, w: f32| if rtl { r.right() + 6.0 } else { r.x - 6.0 - w };
    // Labels beside the plots.
    if let Some(r) = layout.waveform {
        let lanes = a.lanes;
        let lane_h = (r.h - 6.0 * (lanes as f32 - 1.0)) / lanes as f32;
        for l in 0..lanes {
            let name = if lanes == 1 {
                t!("sound-channel-mono")
            } else {
                channel_name(l, 2)
            };
            let b = text.layout(&name, small, None, Align::Start);
            let y = r.y + l as f32 * (lane_h + 6.0) + (lane_h - b.height) / 2.0;
            text.draw(dl, &b, gutter_x(r, b.width), y, pal.text_muted);
        }
    }
    if let Some(r) = layout.spectrogram {
        let (lo, hi) = (f64::from(a.f_lo), f64::from(a.f_hi));
        for f in [100.0, 1000.0, 10_000.0] {
            if f <= lo || f >= hi {
                continue;
            }
            let y = r.bottom() - ((f / lo).log10() / (hi / lo).log10()) as f32 * r.h;
            let b = text.layout(&hz_label(f), small, None, Align::Start);
            text.draw(
                dl,
                &b,
                gutter_x(r, b.width),
                y - b.height / 2.0,
                pal.text_muted,
            );
            let tick = if rtl {
                (r.right(), r.right() + 4.0)
            } else {
                (r.x - 4.0, r.x)
            };
            dl.line((tick.0, y), (tick.1, y), 1.0, pal.text_faint);
        }
    }
    if let Some(r) = layout.history {
        if page.measuring {
            let b = text.layout(
                &t!("sound-loudness-measuring"),
                small,
                Some(r.w - 16.0),
                Align::Center,
            );
            text.draw(
                dl,
                &b,
                r.x + (r.w - b.width) / 2.0,
                r.y + (r.h - b.height) / 2.0,
                pal.text_muted,
            );
        }
        for l in [-20.0f32, -40.0] {
            let y = r.y + (-l / 60.0) * r.h;
            let b = text.layout(&quantity(f64::from(l), 0, ""), small, None, Align::Start);
            text.draw(
                dl,
                &b,
                gutter_x(r, b.width),
                y - b.height / 2.0,
                pal.text_muted,
            );
        }
        if r.h >= 90.0 * ts {
            let b = text.layout(&t!("sound-unit-lufs"), small, None, Align::Start);
            text.draw(dl, &b, gutter_x(r, b.width), r.y, pal.text_muted);
        }
    }
    // The time axis.
    if let Some(ax) = layout.axis {
        let secs = a.duration_s * page.view.span;
        let px_per_s = f64::from(ax.w) / secs.max(1e-6);
        let step = [
            0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
            3600.0,
        ]
        .into_iter()
        .find(|s| s * px_per_s >= f64::from(80.0 * ts))
        .unwrap_or(7200.0);
        let t0 = a.duration_s * page.view.start;
        let mut tk = (t0 / step).ceil() * step;
        while tk <= t0 + secs + 1e-9 {
            let x = ax.x + ((tk - t0) * px_per_s) as f32;
            dl.line((x, ax.y), (x, ax.y + 4.0), 1.0, pal.text_faint);
            let b = text.layout(&time_label(tk, step), small, None, Align::Start);
            let bx = (x - b.width / 2.0).clamp(ax.x, ax.right() - b.width);
            text.draw(dl, &b, bx, ax.y + 4.0, pal.text_muted);
            tk += step;
        }
    }
    // The playhead across the time-based plots.
    let px = page.view.x_of(played);
    if (0.0..=1.0).contains(&px) {
        for r in layout.timelines() {
            let x = r.x + px as f32 * r.w;
            dl.line((x, r.y), (x, r.bottom()), 2.0, pal.text);
        }
    }
    if let Some(r) = layout.spectrum {
        draw_spectrum(dl, text, pal, page, &a, r, rtl, ts);
    }
    if let Some(r) = layout.meters {
        draw_meters(dl, text, pal, page, r, rtl, ts);
    }
    if let Some(r) = layout.numbers {
        draw_numbers(dl, text, pal, page, r, rtl, ts);
    }
    if let Some(r) = layout.stereo {
        draw_stereo(dl, text, pal, page, r, ts);
    }
    dl.pop_clip();
}

/// The part of a `cw` × `ch` picture that fills `r` (centred, cropped).
fn fill_uv(r: Rect, cw: u32, ch: u32) -> [f32; 4] {
    let pa = r.w / r.h.max(1.0);
    let ca = cw.max(1) as f32 / ch.max(1) as f32;
    if ca > pa {
        let w = pa / ca;
        [(1.0 - w) / 2.0, 0.0, (1.0 + w) / 2.0, 1.0]
    } else {
        let h = ca / pa;
        [0.0, (1.0 - h) / 2.0, 1.0, (1.0 + h) / 2.0]
    }
}

/// A correlation as shown: signed, two decimals.
fn correlation_text(v: Option<f32>) -> String {
    match v {
        Some(v) => {
            let n = specular_i18n::I18n::global()
                .localizer()
                .number_fixed(f64::from(v.abs()), 2);
            let sign = if v >= 0.005 {
                "+"
            } else if v <= -0.005 {
                "\u{2212}"
            } else {
                ""
            };
            format!("{sign}{n}")
        }
        None => t!("sound-no-value"),
    }
}

/// The stereo panel: the goniometer's picture (mid up, left and right on the diagonals)
/// over the phase correlation bar (−1 left, +1 right, a mark for the slow average) and its
/// number.
fn draw_stereo(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    page: &SoundPage,
    r: Area,
    ts: f32,
) {
    dl.rect(rect(r), RADIUS_SM, pal.surface);
    let small = TextStyle::new(FONT_SMALL * ts);
    let foot = 40.0 * ts;
    let side = (r.w - 16.0).min(r.h - foot - 12.0).max(0.0);
    let sq = Rect::new(r.x + (r.w - side) / 2.0, r.y + 8.0, side, side);
    if side >= 24.0 {
        if let Some((_, id)) = &page.gonio_tex {
            dl.image(sq, [0.0, 0.0, 1.0, 1.0], *id, 0.0);
        }
        let (cx, cy) = (sq.x + side / 2.0, sq.y + side / 2.0);
        dl.line((cx, sq.y), (cx, sq.y + side), 1.0, pal.separator);
        dl.line((sq.x, cy), (sq.x + side, cy), 1.0, pal.separator);
        let d = side / 2.0 * 0.92;
        dl.line((cx - d, cy - d), (cx + d, cy + d), 1.0, pal.separator);
        dl.line((cx + d, cy - d), (cx - d, cy + d), 1.0, pal.separator);
        dl.stroke_rect(sq, 0.0, pal.separator, 1.0);
        for (label, x) in [
            (t!("sound-channel-left"), sq.x + 4.0),
            (t!("sound-channel-right"), sq.right() - 4.0),
        ] {
            let b = text.layout(&label, small, None, Align::Start);
            let bx = if x > cx { x - b.width } else { x };
            text.draw(dl, &b, bx, sq.y + 2.0, pal.text_muted);
        }
        let m = text.layout(&t!("sound-channel-mono"), small, None, Align::Start);
        text.draw(dl, &m, cx + 4.0, sq.y + 2.0, pal.text_muted);
    }
    // The correlation: a bar from 0 to the value, the average as a mark, the scale.
    let b = &page.ball;
    let bar = Rect::new(r.x + 12.0, r.bottom() - foot + 6.0, r.w - 24.0, 6.0);
    let x_of = |v: f32| bar.x + (v.clamp(-1.0, 1.0) + 1.0) / 2.0 * bar.w;
    dl.rect(bar, 3.0, pal.surface_raised);
    let zero = x_of(0.0);
    if let Some(c) = b.correlation.filter(|_| b.live) {
        let x = x_of(c);
        let colour = if c >= 0.0 { WAVE } else { PLAYED };
        dl.rect(
            Rect::new(zero.min(x), bar.y, (x - zero).abs().max(2.0), bar.h),
            2.0,
            colour,
        );
    }
    if let Some(avg) = b.correlation_avg.filter(|_| b.live) {
        let x = x_of(avg);
        dl.rect(
            Rect::new(x - 1.0, bar.y - 3.0, 2.0, bar.h + 6.0),
            0.0,
            pal.text,
        );
    }
    for v in [-1.0f32, 0.0, 1.0] {
        let x = x_of(v);
        dl.line(
            (x, bar.y + bar.h),
            (x, bar.y + bar.h + 3.0),
            1.0,
            pal.text_faint,
        );
        let label = match v {
            v if v < 0.0 => "\u{2212}1".to_owned(),
            v if v > 0.0 => "+1".to_owned(),
            _ => "0".to_owned(),
        };
        let lb = text.layout(&label, small, None, Align::Start);
        let lx = (x - lb.width / 2.0).clamp(r.x + 4.0, r.right() - 4.0 - lb.width);
        text.draw(dl, &lb, lx, bar.y + bar.h + 3.0, pal.text_muted);
    }
    let value = text.layout(
        &correlation_text(b.correlation_avg.filter(|_| b.live)),
        TextStyle::new(FONT_SMALL * ts).weight(600.0).tabular(),
        None,
        Align::Start,
    );
    text.draw(
        dl,
        &value,
        r.x + (r.w - value.width) / 2.0,
        bar.y - value.height - 2.0,
        pal.text,
    );
}

/// Whether `s` is written right to left (its first strong letter is Hebrew or Arabic).
fn rtl_text(s: &str) -> bool {
    s.chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| ('\u{0590}'..='\u{08FF}').contains(&c))
}

/// The lyrics column: synced lyrics with the current line in bold and the text colour,
/// its sung words filled (enhanced LRC), scrolled to keep it a little above the middle;
/// plain lyrics as they are, scrolled by the wheel. The lines drawn, for clicks.
fn draw_lyrics(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    page: &mut SoundPage,
    r: Area,
    ts: f32,
) -> Vec<(usize, Rect)> {
    let mut drawn = Vec::new();
    dl.rect(rect(r), RADIUS_SM, pal.surface);
    let Some(l) = page.lyrics.clone() else {
        return drawn;
    };
    let size = FONT_SMALL * 1.15 * ts;
    let plain = TextStyle::new(size);
    let bold = TextStyle::new(size).weight(650.0);
    let pad = 12.0;
    let w = (r.w - 2.0 * pad).max(1.0);
    let cur = page.scroll.line.filter(|_| l.synced);
    let gap = 6.0 * ts;
    // Where each line goes (from the top of the lyrics). The lines' heights are laid out
    // once for the lyrics, width and size (a long recording's lyrics have thousands of
    // lines); the current line's again, in bold.
    let key = (Arc::as_ptr(&l) as usize, w.to_bits(), size.to_bits());
    if page.lyric_heights.as_ref().is_none_or(|(k, _)| *k != key) {
        let heights = l
            .lines
            .iter()
            .map(|line| {
                if line.text.is_empty() {
                    size * 0.8
                } else {
                    text.layout(&line.text, plain, Some(w), Align::Center)
                        .height
                }
            })
            .collect();
        page.lyric_heights = Some((key, heights));
    }
    let heights = page.lyric_heights.as_ref().map_or(&[][..], |h| &h.1[..]);
    let mut y = 0.0;
    let mut places = Vec::with_capacity(l.lines.len());
    for (i, line) in l.lines.iter().enumerate() {
        let h = if Some(i) == cur && !line.text.is_empty() {
            text.layout(&line.text, bold, Some(w), Align::Center).height
        } else {
            heights.get(i).copied().unwrap_or(size)
        };
        places.push((y, h));
        y += h + gap;
    }
    let total = y;
    let view_h = (r.h - 2.0 * pad).max(1.0);
    // The line with the keyboard is kept in view while it has it.
    let anchor = match page.focus {
        Some(Focus::Line(i)) if l.synced && i < places.len() => Some(i),
        _ => cur,
    };
    let to = match anchor {
        Some(i) => places[i].0 + places[i].1 / 2.0 - view_h * 0.4,
        None => 0.0,
    };
    page.scroll.to = to;
    if page.scroll.jump {
        page.scroll.y = to;
        page.scroll.jump = false;
    }
    // The wheel moves from there, no further than the ends.
    let max = (total - view_h).max(0.0);
    let base = if l.synced { page.scroll.y } else { 0.0 };
    let (lo, hi) = if l.synced {
        (-view_h * 0.5, max + view_h * 0.5)
    } else {
        (0.0, max)
    };
    let offset = (base + page.scroll.wheel).clamp(lo.min(base), hi.max(base));
    page.scroll.wheel = offset - base;
    let inner = Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0);
    dl.push_clip(inner);
    for (i, line) in l.lines.iter().enumerate() {
        let (ly, h) = places[i];
        let top = r.y + pad + ly - offset;
        if top + h < r.y || top > r.bottom() {
            continue;
        }
        if line.text.is_empty() {
            continue;
        }
        let is_cur = Some(i) == cur;
        let style = if is_cur { bold } else { plain };
        let b = text.layout(&line.text, style, Some(w), Align::Center);
        let x = r.x + pad + (w - b.width) / 2.0;
        let colour = if !l.synced || is_cur {
            pal.text
        } else {
            pal.text_muted
        };
        let sung = if is_cur && b.lines == 1 {
            l.sung(i, page.t)
        } else {
            None
        };
        match sung {
            Some(f) if f < 1.0 => {
                // Sung part in the text colour, the rest muted.
                text.draw(dl, &b, x, top, pal.text_muted);
                let fw = b.width * f;
                let clip = if rtl_text(&line.text) {
                    Rect::new(x + b.width - fw, top, fw, b.height)
                } else {
                    Rect::new(x, top, fw, b.height)
                };
                dl.push_clip(clip);
                text.draw(dl, &b, x, top, pal.text);
                dl.pop_clip();
            }
            _ => text.draw(dl, &b, x, top, colour),
        }
        // Lines wholly in view can be clicked (and are read out).
        if top >= r.y && top + b.height <= r.bottom() {
            drawn.push((i, Rect::new(r.x, top, r.w, b.height)));
        }
        if page.focus == Some(Focus::Line(i)) {
            let g = Rect::new(x - 6.0, top - 3.0, b.width + 12.0, b.height + 6.0);
            dl.stroke_rect(g, RADIUS_SM, pal.focus_ring, 2.0);
        }
    }
    dl.pop_clip();
    drawn
}

/// The spectrum analyser: third-octave bars (or the bins as a curve) with held peaks, on a
/// log frequency axis and a dB scale.
#[allow(clippy::too_many_arguments)]
fn draw_spectrum(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    page: &SoundPage,
    a: &Analysis,
    r: Area,
    rtl: bool,
    ts: f32,
) {
    let small = TextStyle::new(FONT_SMALL * ts);
    dl.rect(rect(r), RADIUS_SM, pal.surface);
    let scale_w = 30.0 * ts;
    let axis_h = 18.0 * ts;
    let plot = Area::new(
        if rtl { r.x + 6.0 } else { r.x + scale_w },
        r.y + 8.0,
        (r.w - scale_w - 6.0).max(1.0),
        (r.h - axis_h - 8.0).max(1.0),
    );
    let y_of = |db: f32| plot.y + (db.clamp(SPEC_FLOOR, 0.0) / SPEC_FLOOR) * plot.h;
    // dB grid.
    for db in [0.0f32, -20.0, -40.0, -60.0, -80.0] {
        let y = y_of(db);
        dl.line((plot.x, y), (plot.right(), y), 1.0, pal.separator);
        let b = text.layout(&quantity(f64::from(db), 0, ""), small, None, Align::Start);
        let x = if rtl {
            plot.right() + 4.0
        } else {
            plot.x - 4.0 - b.width
        };
        text.draw(dl, &b, x, y - b.height / 2.0, pal.text_muted);
    }
    // Frequency labels: as many as fit without touching (ends and powers of ten first).
    const TICKS: [f64; 9] = [
        50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0,
    ];
    let mut labels = Vec::with_capacity(TICKS.len());
    for f in TICKS {
        let x = plot.x + freq_x(f) * plot.w;
        dl.line(
            (x, plot.bottom()),
            (x, plot.bottom() + 3.0),
            1.0,
            pal.text_faint,
        );
        let b = text.layout(&hz_label(f), small, None, Align::Start);
        let bx = (x - b.width / 2.0).clamp(plot.x, (plot.right() - b.width).max(plot.x));
        labels.push((f, bx, b));
    }
    let spans: Vec<(f64, f32, f32)> = labels.iter().map(|(f, x, b)| (*f, *x, b.width)).collect();
    for ((_, bx, b), show) in labels.iter().zip(fit_labels(&spans, 6.0 * ts)) {
        if show {
            text.draw(dl, b, *bx, plot.bottom() + 3.0, pal.text_muted);
        }
    }
    let ball = &page.ball;
    if !ball.live {
        return;
    }
    dl.push_clip(rect(plot));
    if page.visuals.curve {
        // The spectrogram's bins at their frequencies.
        let (lo, hi) = (f64::from(a.f_lo), f64::from(a.f_hi));
        let x_of = |b: usize| {
            let f = lo * (hi / lo).powf((b as f64 + 0.5) / BINS as f64);
            plot.x + freq_x(f) * plot.w
        };
        for b in 1..BINS {
            let (x0, x1) = (x_of(b - 1), x_of(b));
            if x1 < plot.x || x0 > plot.right() {
                continue;
            }
            let p0 = (x0, y_of(ball.bins[b - 1].peak));
            let p1 = (x1, y_of(ball.bins[b].peak));
            dl.line(p0, p1, 1.0, PLAYED);
            let l0 = (x0, y_of(ball.bins[b - 1].level));
            let l1 = (x1, y_of(ball.bins[b].level));
            dl.line(l0, l1, 2.0, WAVE);
        }
    } else {
        let edge = |k: f64| plot.x + freq_x(band_centre(0) * 2f64.powf(k / 3.0)) * plot.w;
        for (k, h) in ball.bands.iter().enumerate().take(BANDS) {
            let (x0, x1) = (edge(k as f64 - 0.5), edge(k as f64 + 0.5));
            let w = (x1 - x0 - 2.0).max(1.0);
            if h.level > SPEC_FLOOR {
                let y = y_of(h.level);
                dl.rect(Rect::new(x0 + 1.0, y, w, plot.bottom() - y), 1.0, WAVE);
            }
            if h.peak > SPEC_FLOOR {
                let y = y_of(h.peak);
                dl.rect(Rect::new(x0 + 1.0, y - 1.0, w, 2.0), 0.0, PLAYED);
            }
        }
    }
    dl.pop_clip();
}

/// The level meters: per channel the RMS level (solid) under the peak level, the held peak
/// and the true peak as marks, a clip light on top and the dB scale beside.
#[allow(clippy::too_many_arguments)]
fn draw_meters(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    page: &SoundPage,
    r: Area,
    rtl: bool,
    ts: f32,
) {
    use specular_video::sound_page::{METER_SCALE_W, METER_W};
    let small = TextStyle::new(FONT_SMALL * ts);
    dl.rect(rect(r), RADIUS_SM, pal.surface);
    let ball = &page.ball;
    let n = ball
        .channels
        .max(page.analysis.as_ref().map_or(1, |a| a.channels))
        .max(1);
    let light_h = 10.0;
    let label_h = 18.0 * ts;
    let top = r.y + 8.0 + light_h + 4.0;
    let bottom = r.bottom() - label_h;
    let h = (bottom - top).max(1.0);
    let y_of = |db: f32| top + (db.clamp(METER_FLOOR, 0.0) / METER_FLOOR) * h;
    let cols_w = METER_W * n as f32;
    let x0 = if rtl { r.x + 4.0 } else { r.x + METER_SCALE_W };
    // Scale.
    for db in [0.0f32, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0, -60.0] {
        let y = y_of(db);
        dl.line((x0, y), (x0 + cols_w, y), 1.0, pal.separator);
        if h < 140.0 && [-6.0, -18.0, -36.0].contains(&db) {
            continue;
        }
        let b = text.layout(&quantity(f64::from(db), 0, ""), small, None, Align::Start);
        let x = if rtl {
            x0 + cols_w + 4.0
        } else {
            x0 - 4.0 - b.width
        };
        text.draw(dl, &b, x, y - b.height / 2.0, pal.text_muted);
    }
    for i in 0..n {
        let cx = x0 + METER_W * i as f32;
        let bar = Rect::new(cx + 4.0, top, METER_W - 8.0, h);
        dl.rect(bar, 2.0, pal.surface_raised);
        if ball.live {
            let peak = ball.peak[i].level;
            if peak > METER_FLOOR {
                let y = y_of(peak);
                dl.rect(Rect::new(bar.x, y, bar.w, bottom - y), 2.0, WAVE_RMS);
            }
            let rms = ball.rms[i];
            if rms > METER_FLOOR {
                let y = y_of(rms);
                dl.rect(Rect::new(bar.x, y, bar.w, bottom - y), 2.0, WAVE);
            }
            let hold = ball.peak[i].peak;
            if hold > METER_FLOOR {
                let y = y_of(hold);
                dl.rect(Rect::new(bar.x, y - 1.0, bar.w, 2.0), 0.0, pal.text);
            }
            let tp = ball.true_peak[i].peak;
            if tp > METER_FLOOR {
                let y = y_of(tp);
                dl.rect(
                    Rect::new(bar.x - 2.0, y - 1.0, bar.w + 4.0, 2.0),
                    0.0,
                    PLAYED,
                );
            }
        }
        // The clip light.
        let light = Rect::new(bar.x, r.y + 8.0, bar.w, light_h);
        if ball.clip[i] {
            dl.rect(light, 2.0, pal.danger);
        } else {
            dl.stroke_rect(light, 2.0, pal.separator, 1.0);
        }
        let b = text.layout(&channel_name(i, n), small, None, Align::Start);
        text.draw(
            dl,
            &b,
            cx + (METER_W - b.width) / 2.0,
            bottom + 2.0,
            pal.text_muted,
        );
    }
}

/// The loudness numbers: momentary and short-term at the playing position, integrated,
/// range and the highest true peak of the whole sound.
fn draw_numbers(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    page: &SoundPage,
    r: Area,
    rtl: bool,
    ts: f32,
) {
    dl.rect(rect(r), RADIUS_SM, pal.surface);
    let label_style = TextStyle::new(FONT_SMALL * ts);
    let value_style = TextStyle::new(FONT_SMALL * 1.25 * ts)
        .weight(600.0)
        .tabular();
    let rows = numbers(page);
    let laid: Vec<_> = rows
        .iter()
        .map(|(l, v)| {
            (
                text.layout(l, label_style, Some(r.w - 20.0), Align::Start),
                text.layout(v, value_style, None, Align::Start),
            )
        })
        .collect();
    let gap = 6.0 * ts;
    // A row without a value ("Measuring loudness…") is its label alone.
    let value_h = |v: &specular_gfx::TextBlock| if v.width > 0.0 { v.height } else { 0.0 };
    let two_lines: f32 = laid.iter().map(|(l, v)| l.height + value_h(v) + gap).sum();
    let x = |w: f32, end: bool| {
        if rtl != end {
            r.right() - 10.0 - w
        } else {
            r.x + 10.0
        }
    };
    let mut y = r.y + 10.0;
    if two_lines <= r.h - 16.0 {
        // Label over value.
        for (l, v) in &laid {
            text.draw(dl, l, x(l.width, false), y, pal.text_muted);
            y += l.height;
            text.draw(dl, v, x(v.width, false), y, pal.text);
            y += value_h(v) + gap;
        }
    } else {
        // Short: label and value on one line, as many as fit.
        for (l, v) in &laid {
            let h = l.height.max(v.height);
            if y + h > r.bottom() - 4.0 {
                break;
            }
            text.draw(
                dl,
                l,
                x(l.width, false),
                y + (h - l.height) / 2.0,
                pal.text_muted,
            );
            text.draw(dl, v, x(v.width, true), y + (h - v.height) / 2.0, pal.text);
            y += h + gap / 2.0;
        }
    }
}

/// The loudness numbers as label and value.
fn numbers(page: &SoundPage) -> Vec<(String, String)> {
    let b = &page.ball;
    let l = page.loudness.as_deref();
    let lufs = t!("sound-unit-lufs");
    let at = |v: Option<f32>| reading(v.map(f64::from).filter(|v| *v > f64::from(FLOOR_DB)), &lufs);
    if page.measuring {
        // A long sound's whole-file loudness is still being read: said where it will show.
        return vec![
            (t!("sound-momentary"), at(b.momentary)),
            (t!("sound-short-term"), at(b.short_term)),
            (t!("sound-loudness-measuring"), String::new()),
        ];
    }
    vec![
        (t!("sound-momentary"), at(b.momentary)),
        (t!("sound-short-term"), at(b.short_term)),
        (
            t!("sound-integrated"),
            reading(l.and_then(|l| l.integrated), &lufs),
        ),
        (
            t!("sound-loudness-range"),
            reading(l.and_then(|l| l.range), &t!("sound-unit-lu")),
        ),
        (
            t!("sound-true-peak"),
            reading(l.map(Report::max_true_peak_db), &t!("sound-unit-dbtp")),
        ),
    ]
}

/// The Info panel's loudness rows: integrated loudness, loudness range, true peak.
pub(super) fn info_items(l: &Report) -> Vec<specular_metadata::SummaryItem> {
    let item = |key: &'static str, label: String, value: String| specular_metadata::SummaryItem {
        key,
        label,
        value,
        detail: None,
    };
    let mut out = Vec::new();
    if let Some(i) = l.integrated {
        out.push(item(
            "sound-loudness",
            t!("info-sound-loudness"),
            quantity(i, 1, &t!("sound-unit-lufs")),
        ));
    }
    if let Some(r) = l.range {
        out.push(item(
            "sound-loudness-range",
            t!("info-sound-loudness-range"),
            quantity(r, 1, &t!("sound-unit-lu")),
        ));
    }
    let tp = l.max_true_peak_db();
    if tp.is_finite() {
        out.push(item(
            "sound-true-peak",
            t!("info-sound-true-peak"),
            quantity(tp, 1, &t!("sound-unit-dbtp")),
        ));
    }
    out
}

/// The page's accessibility nodes: the choices (toggle buttons), each meter's levels and
/// the loudness numbers, read on demand (not announced as they change).
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, page: &SoundPage, sf: f64) {
    use accesskit::{Node, NodeId, Role, Toggled};
    let Some(d) = &page.drawn else { return };
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::Group);
    g.set_label(t!("sound-page"));
    g.set_bounds(bounds(d.area));
    let mut children = Vec::new();
    for (i, (chip, r, on)) in d.chips.iter().enumerate() {
        let id = NodeId(A11Y_CHIP + i as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(chip_name(*chip));
        n.set_toggled(if *on { Toggled::True } else { Toggled::False });
        n.set_bounds(bounds(*r));
        n.add_action(accesskit::Action::Click);
        n.add_action(accesskit::Action::Focus);
        g.push_child(id);
        children.push((id, n));
    }
    if let Some(m) = d.layout.meters {
        let b = &page.ball;
        let ch = b.channels.max(1);
        for i in 0..ch {
            let id = NodeId(A11Y_METER + i as u64);
            let mut n = Node::new(Role::Meter);
            n.set_label(channel_name(i, ch));
            let db = |v: f32| {
                reading(
                    Some(f64::from(v)).filter(|v| *v > f64::from(FLOOR_DB)),
                    &t!("sound-unit-db"),
                )
            };
            n.set_value(t!(
                "sound-meter-value",
                peak = db(b.peak[i].level),
                rms = db(b.rms[i]),
                true_peak = db(b.true_peak[i].peak),
                clip = if b.clip[i] {
                    t!("sound-clipped")
                } else {
                    String::new()
                }
            ));
            n.set_numeric_value(f64::from(b.peak[i].level));
            n.set_min_numeric_value(f64::from(METER_FLOOR));
            n.set_max_numeric_value(0.0);
            n.set_bounds(bounds(rect(m)));
            g.push_child(id);
            children.push((id, n));
        }
    }
    if let Some(r) = d.layout.numbers {
        for (i, (label, value)) in numbers(page).into_iter().enumerate() {
            let id = NodeId(A11Y_NUMBERS + i as u64);
            // "Measuring loudness…" has no value: a status, read out when it shows.
            let mut n = if value.is_empty() {
                let mut n = Node::new(Role::Status);
                n.set_live(accesskit::Live::Polite);
                n.set_label(label);
                n
            } else {
                let mut n = Node::new(Role::Label);
                n.set_label(format!("{label}: {value}"));
                n
            };
            n.set_bounds(bounds(rect(r)));
            g.push_child(id);
            children.push((id, n));
        }
    }
    if let Some(r) = d.layout.stereo {
        let b = &page.ball;
        let id = NodeId(A11Y_STEREO);
        let mut n = Node::new(Role::Meter);
        n.set_label(t!("sound-correlation"));
        let live = |v: Option<f32>| v.filter(|_| b.live);
        n.set_value(t!(
            "sound-correlation-value",
            value = correlation_text(live(b.correlation)),
            average = correlation_text(live(b.correlation_avg))
        ));
        if let Some(c) = live(b.correlation_avg) {
            n.set_numeric_value(f64::from(c));
        }
        n.set_min_numeric_value(-1.0);
        n.set_max_numeric_value(1.0);
        n.set_bounds(bounds(rect(r)));
        g.push_child(id);
        children.push((id, n));
    }
    if let (Some(r), Some(l)) = (d.layout.lyrics, &page.lyrics) {
        let id = NodeId(A11Y_LYRICS);
        let mut n = Node::new(Role::Group);
        n.set_label(t!("sound-lyrics"));
        n.set_bounds(bounds(rect(r)));
        for (i, lr) in &d.lines {
            let Some(line) = l.lines.get(*i) else {
                continue;
            };
            let lid = NodeId(A11Y_LYRICS + 1 + *i as u64);
            let mut ln = Node::new(if l.synced { Role::Button } else { Role::Label });
            ln.set_label(line.text.clone());
            if l.synced {
                ln.add_action(accesskit::Action::Click);
                ln.add_action(accesskit::Action::Focus);
                if Some(*i) == page.scroll.line {
                    ln.set_selected(true);
                }
            }
            ln.set_bounds(bounds(*lr));
            n.push_child(lid);
            children.push((lid, ln));
        }
        g.push_child(id);
        children.push((id, n));
    }
    if let Some((msg, r)) = &d.status {
        let id = NodeId(A11Y_STATUS);
        let mut n = Node::new(Role::Status);
        n.set_live(accesskit::Live::Polite);
        n.set_label(msg.as_str());
        n.set_bounds(bounds(*r));
        g.push_child(id);
        children.push((id, n));
    }
    let root = tree.tree.as_ref().map_or(NodeId(1), |t| t.root);
    if let Some((_, w)) = tree.nodes.iter_mut().find(|(id, _)| *id == root) {
        w.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), g));
    tree.nodes.extend(children);
    // The keyboard's place on the page, when it is drawn.
    let focus = match page.focus {
        Some(Focus::Chip(i)) if i < d.chips.len() => Some(NodeId(A11Y_CHIP + i as u64)),
        Some(Focus::Line(i)) if d.lines.iter().any(|(x, _)| *x == i) => {
            Some(NodeId(A11Y_LYRICS + 1 + i as u64))
        }
        _ => None,
    };
    if let Some(f) = focus {
        tree.focus = f;
    }
}

/// Writes an RGBA8 picture into `tex` (made by [`upload_rgba`] at that size).
fn write_rgba(gpu: &Gpu, tex: &wgpu::Texture, w: u32, h: u32, rgba: &[u8]) {
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w.max(1)),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
    );
}

/// Uploads an RGBA8 picture into a new sRGB texture the UI can draw.
fn upload_rgba(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w.max(1),
        height: h.max(1),
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sound cover"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w.max(1)),
            rows_per_image: None,
        },
        size,
    );
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_names_follow_the_wave_order() {
        assert_eq!(channel_name(0, 1), t!("sound-channel-mono"));
        assert_eq!(channel_name(3, 6), t!("sound-channel-lfe"));
        assert_eq!(channel_name(3, 5), t!("sound-channel-left-surround"));
        assert_eq!(channel_name(2, 4), t!("sound-channel-left-surround"));
        // 7.1 in the WAVE order: L R C LFE Lb Rb Ls Rs.
        assert_eq!(channel_name(4, 8), t!("sound-channel-left-back"));
        assert_eq!(channel_name(7, 8), t!("sound-channel-right-surround"));
        assert_eq!(channel_name(4, 7), t!("sound-channel-back-centre"));
    }

    #[test]
    fn frequency_axis_ends() {
        assert!(freq_x(F_LO).abs() < 1e-6);
        assert!((freq_x(F_HI) - 1.0).abs() < 1e-6);
        assert!((freq_x(632.4555) - 0.5).abs() < 1e-3);
    }

    /// Crowded axis labels drop out without overlapping: the ends and powers of ten stay
    /// longest; with room, every label shows.
    #[test]
    fn frequency_labels_never_overlap() {
        let ticks = [
            50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0,
        ];
        for w in [120.0f32, 200.0, 300.0, 450.0, 900.0] {
            let spans: Vec<(f64, f32, f32)> = ticks
                .iter()
                .map(|&f| {
                    let x = freq_x(f) * w;
                    let lw = 26.0;
                    (f, (x - lw / 2.0).clamp(0.0, w - lw), lw)
                })
                .collect();
            let show = fit_labels(&spans, 6.0);
            let shown: Vec<_> = spans.iter().zip(&show).filter(|(_, s)| **s).collect();
            for (i, (a, _)) in shown.iter().enumerate() {
                for (b, _) in &shown[i + 1..] {
                    assert!(
                        a.1 + a.2 + 6.0 <= b.1 || b.1 + b.2 + 6.0 <= a.1,
                        "{w}: {a:?} {b:?}"
                    );
                }
            }
            assert!(show[0] && show[8], "{w}: ends");
            if w >= 450.0 {
                assert!(show[1] && show[4] && show[7], "{w}: powers of ten");
            }
            if w >= 900.0 {
                assert!(show.iter().all(|s| *s), "{w}");
            }
        }
    }
}

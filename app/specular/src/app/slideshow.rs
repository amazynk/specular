//! The slideshow (DESIGN.md §6.5): `s` starts and stops it, Space pauses, ← and → step and
//! restart the timer, Esc stops at the current image. Full screen with the chrome hidden; the
//! next slides are prefetched and a slide only comes on once it is sharp, so a transition never
//! shows a half-loaded image; the timer counts only while the slide on screen is ready, no
//! dialog is open and the show is not paused.
//!
//! The order is a [`Playlist`]: the folder's order starting at the current image, or shuffled
//! (deterministic per run, every image once before any comes back). Without loop the show
//! stops after every image has been on screen once.

use std::path::PathBuf;

use specular_gfx::{
    Align, Compositor, Crossfade, DrawList, Gpu, Icon, Rect, Text, TextStyle, draw_icon, wgpu,
};
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo, Mods, NamedKey};
use specular_metadata::MetadataReport;
use specular_render::{PxRect, View};
use specular_settings::{SlideshowCaption, SlideshowTransition};
use specular_theme::Color;
use specular_theme::metrics::{FONT_BODY, FONT_TITLE, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_widgets::Playback;

use super::App;
use crate::loader::{ImageId, ListOptions};

/// How long the cross-fade takes (s).
pub(super) const FADE_S: f64 = 0.4;
/// An animation plays through once, but never keeps the show on one slide for longer than this
/// (or the interval, when that is longer).
pub(super) const ANIM_CAP_S: f64 = 60.0;
/// The pointer hides after this long without moving (s).
pub(super) const POINTER_IDLE_S: f64 = 2.0;
/// A next slide that is not sharp yet comes on anyway after this long, if it has any pixels.
pub(super) const PATIENCE_S: f64 = 5.0;
/// How many upcoming slides are prefetched first.
pub(super) const AHEAD: usize = 2;
/// Ken Burns: how much a slide zooms over its time on screen (subtle).
const KB_ZOOM: f32 = 0.08;

// ----- Order --------------------------------------------------------------------------------

/// splitmix64: small, fast and good enough to shuffle a folder.
fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn below(state: &mut u64, n: usize) -> usize {
    (next_u64(state) % n as u64) as usize
}

/// Fisher–Yates.
fn shuffle(v: &mut [usize], rng: &mut u64) {
    for i in (1..v.len()).rev() {
        let j = below(rng, i + 1);
        v.swap(i, j);
    }
}

/// The order of the slides: indices into the folder's listing, in cycles (every image once per
/// cycle). `order[..pos]` is what was on screen before, kept for ← (the current cycle and the
/// one before it, across reshuffles and new listings); `order[pos]` is the current slide and
/// the rest the plan. Without repeat there is one cycle; with repeat the next is appended as it
/// is needed (shuffled anew, never starting with the image the last one ended on; in order
/// from where the show began).
#[derive(Clone, Debug)]
pub(super) struct Playlist {
    order: Vec<usize>,
    pos: usize,
    /// Where the cycles start in `order`, ascending.
    cycles: Vec<usize>,
    n: usize,
    /// The image an in-order cycle starts at.
    start: usize,
    shuffle: bool,
    pub repeat: bool,
    rng: u64,
    /// Entries the show passes over (text files in a show of pictures, or the other way
    /// round); empty: none.
    pub skip: Vec<bool>,
}

impl Playlist {
    /// `n` images, starting at `start`.
    pub fn new(n: usize, start: usize, shuffle: bool, repeat: bool, seed: u64) -> Playlist {
        let mut p = Playlist {
            order: Vec::new(),
            pos: 0,
            cycles: Vec::new(),
            n,
            start,
            shuffle,
            repeat,
            rng: seed,
            skip: Vec::new(),
        };
        p.relist(n, start, start, |_| None);
        p
    }

    /// Passes over the entries `skip` marks from now on (the current slide stays).
    pub fn set_skip(&mut self, skip: Vec<bool>) {
        self.skip = skip;
        let Some(cur) = self.current() else { return };
        self.relist(self.n, cur, self.start, Some);
    }

    fn skipped(&self, i: usize) -> bool {
        self.skip.get(i).copied().unwrap_or(false)
    }

    pub fn shuffled(&self) -> bool {
        self.shuffle
    }

    /// Shuffle switched on or off mid-show: the current slide and what was on before it stay;
    /// the images not on yet this cycle are shuffled, or (off) follow in the folder's order from
    /// the current one, where later cycles start too.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        let Some(cur) = self.current() else { return };
        let start = if on { self.start } else { cur };
        self.relist(self.n, cur, start, Some);
    }

    /// Plans anew for a listing of `n` images with `current` on screen; `remap` takes an index
    /// of the listing planned for so far to the new one (`None`: gone). What was on screen
    /// before stays behind for ← and, from this cycle, does not come again before the cycle
    /// ends; the rest of the cycle is shuffled anew, or follows in the listing's order from
    /// `current` (later cycles in order start at `start`, where the show began).
    pub fn relist(
        &mut self,
        n: usize,
        current: usize,
        start: usize,
        remap: impl Fn(usize) -> Option<usize>,
    ) {
        let cycle = self.cycle_start();
        let mut past = Vec::new();
        let mut seen = 0;
        for (k, &i) in self.order.iter().enumerate().take(self.pos) {
            if let Some(j) = remap(i).filter(|&j| j < n) {
                past.push(j);
                seen += usize::from(k >= cycle);
            }
        }
        // The slide on screen until now, when another image was chosen.
        if let Some(j) = self
            .current()
            .and_then(&remap)
            .filter(|&j| j < n && j != current)
        {
            past.push(j);
            seen += 1;
        }
        self.n = n;
        self.pos = 0;
        self.order.clear();
        self.cycles.clear();
        if n == 0 {
            return;
        }
        let current = current.min(n - 1);
        self.start = if start < n { start } else { current };
        let mut taken = vec![false; n];
        for &i in &past[past.len() - seen..] {
            taken[i] = true;
        }
        taken[current] = true;
        self.pos = past.len();
        self.cycles.push(self.pos - seen);
        self.order = past;
        self.order.push(current);
        if self.shuffle {
            let mut rest: Vec<usize> = (0..n).filter(|&i| !taken[i] && !self.skipped(i)).collect();
            shuffle(&mut rest, &mut self.rng);
            self.order.extend(rest);
        } else {
            let rest: Vec<usize> = (1..n)
                .map(|k| (current + k) % n)
                .filter(|&i| !taken[i] && !self.skipped(i))
                .collect();
            self.order.extend(rest);
        }
        self.trim();
    }

    pub fn current(&self) -> Option<usize> {
        self.order.get(self.pos).copied()
    }

    /// Where the current slide's cycle starts in `order`.
    fn cycle_start(&self) -> usize {
        self.cycles
            .iter()
            .rev()
            .copied()
            .find(|&c| c <= self.pos)
            .unwrap_or(0)
    }

    /// Appends cycles until `len` slides are planned (with repeat).
    fn grow(&mut self, len: usize) {
        if !self.repeat || self.n == 0 {
            return;
        }
        while self.order.len() < len {
            let n = self.n;
            let next: Vec<usize> = if self.shuffle {
                let mut v: Vec<usize> = (0..n).filter(|&i| !self.skipped(i)).collect();
                shuffle(&mut v, &mut self.rng);
                if v.len() > 1 && self.order.last() == v.first() {
                    let j = 1 + below(&mut self.rng, v.len() - 1);
                    v.swap(0, j);
                }
                v
            } else {
                (0..n)
                    .map(|k| (self.start + k) % n)
                    .filter(|&i| !self.skipped(i))
                    .collect()
            };
            if next.is_empty() {
                return;
            }
            self.cycles.push(self.order.len());
            self.order.extend(next);
        }
    }

    /// Keeps at most a cycle's worth of slides before the current cycle for ←, no more.
    fn trim(&mut self) {
        let c = self.cycle_start();
        if c <= self.n {
            return;
        }
        let d = c - self.n;
        self.order.drain(..d);
        self.pos -= d;
        self.cycles.retain(|&b| b >= d);
        for b in &mut self.cycles {
            *b -= d;
        }
    }

    /// The next slide (after ←, the one that was on next again); `None` at the end (without
    /// repeat).
    pub fn advance(&mut self) -> Option<usize> {
        self.grow(self.pos + 2);
        if self.pos + 1 >= self.order.len() {
            return None;
        }
        self.pos += 1;
        self.trim();
        self.current()
    }

    /// The slide on before, back through the last cycle; `None` at the start (in order with
    /// repeat, the image before the one the show began at).
    pub fn back(&mut self) -> Option<usize> {
        if self.pos > 0 {
            self.pos -= 1;
        } else if self.repeat && !self.shuffle && self.n > 1 {
            self.grow(self.order.len() + 1);
            let end = self.cycles.iter().copied().find(|&c| c > 0);
            self.pos = end.unwrap_or(self.order.len()) - 1;
        } else {
            return None;
        }
        self.current()
    }

    /// The next `k` slides (fewer at the end).
    pub fn peek(&mut self, k: usize) -> Vec<usize> {
        self.grow(self.pos + 1 + k);
        let end = (self.pos + 1 + k).min(self.order.len());
        self.order
            .get(self.pos + 1..end)
            .unwrap_or_default()
            .to_vec()
    }
}

// ----- Time ---------------------------------------------------------------------------------

/// Time on screen of the current slide: counts only while `counting` (the slide is ready, no
/// dialog, not paused).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Timer {
    elapsed: f64,
    last: Option<f64>,
}

impl Timer {
    pub fn tick(&mut self, now: f64, counting: bool) {
        if counting {
            if let Some(l) = self.last {
                self.elapsed += (now - l).max(0.0);
            }
            self.last = Some(now);
        } else {
            self.last = None;
        }
    }

    /// Time on screen as of `now` (between ticks too, for smooth motion).
    pub fn at(&self, now: f64) -> f64 {
        self.elapsed + self.last.map_or(0.0, |l| (now - l).max(0.0))
    }

    pub fn restart(&mut self) {
        *self = Timer::default();
    }
}

/// Whether the slide has been on long enough: the interval, and for an animation also one
/// play-through (at most [`ANIM_CAP_S`], or the interval when that is longer).
pub(super) fn due(elapsed: f64, interval: f64, animated: bool, played_once: bool) -> bool {
    elapsed >= interval && (!animated || played_once || elapsed >= interval.max(ANIM_CAP_S))
}

/// Watches an animation for one play-through.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AnimWatch {
    last: Option<usize>,
    pub done: bool,
}

impl AnimWatch {
    pub fn observe(&mut self, p: Option<Playback>) {
        let Some(p) = p else { return };
        let wrapped = self.last.is_some_and(|l| p.frame < l);
        // Paused or stopped (played its loops), a single frame, or back at the start.
        if !p.playing || p.count.is_some_and(|c| c <= 1) || wrapped {
            self.done = true;
        }
        self.last = Some(p.frame);
    }
}

/// Ease in and out (the outgoing slide's weight is `1 − smoothstep(t)`).
pub(super) fn fade_weight(t: f64) -> f32 {
    let t = t.clamp(0.0, 1.0);
    (1.0 - t * t * (3.0 - 2.0 * t)) as f32
}

/// The listing of a show with subfolders: the folder's sort order, recursive.
pub(super) fn recursive_listing(
    order: specular_settings::SortOrder,
    descending: bool,
) -> ListOptions {
    ListOptions {
        order,
        descending,
        recursive: true,
    }
}

/// How a running show's folder is listed again when the sort order or a subfolder setting
/// changes: with its subfolders, in the folder's (new) order, while the show `wants` them;
/// the folder's own listing when the show `had` listed them and no longer does; `None` when
/// the show never listed its own (the folder is listed as when browsing).
pub(super) fn show_relisting(
    wants: bool,
    had: bool,
    b: &specular_settings::Browsing,
) -> Option<ListOptions> {
    if wants {
        Some(recursive_listing(b.sort_order, b.sort_descending))
    } else {
        had.then_some(ListOptions {
            order: b.sort_order,
            descending: b.sort_descending,
            recursive: b.include_subfolders,
        })
    }
}

// ----- Caption ------------------------------------------------------------------------------

/// The caption's lines: the file name, and with EXIF the date taken, camera and exposure on a
/// second line (what of them the file has).
pub(super) fn caption_lines(
    name: &str,
    report: Option<&MetadataReport>,
    mode: SlideshowCaption,
) -> Vec<String> {
    if mode == SlideshowCaption::Off {
        return Vec::new();
    }
    let mut lines = vec![name.to_owned()];
    if mode == SlideshowCaption::NameExif
        && let Some(r) = report
    {
        let item = |group: &str, key: &str| {
            r.summary
                .iter()
                .filter(|g| g.key == group)
                .flat_map(|g| &g.items)
                .find(|i| i.key == key)
                .map(|i| i.value.clone())
        };
        let parts: Vec<String> = [
            item("date", "taken"),
            item("camera", "camera"),
            r.exposure_line.clone(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
        if !parts.is_empty() {
            lines.push(specular_i18n::dot_join(&parts));
        }
    }
    lines
}

/// The slideshow's overlay in the image `area` (logical x, y, w, h): the caption in a
/// translucent box at the bottom-start corner, and a pause badge at the top-end corner while
/// paused.
pub(super) fn draw_overlay(
    dl: &mut DrawList,
    text: &mut Text,
    lines: &[String],
    paused: bool,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let back = Color::rgba(0.0, 0.0, 0.0, 0.55);
    if paused {
        let s = 40.0 * ts;
        let x = if rtl { ax + m } else { ax + aw - m - s };
        let r = Rect::new(x, ay + m, s, s);
        dl.rect(r, s * 0.5, back);
        let i = s * 0.25;
        draw_icon(
            dl,
            Icon::Pause,
            Rect::new(r.x + i, r.y + i, s - 2.0 * i, s - 2.0 * i),
            Color::WHITE,
            2.0 * ts,
            false,
        );
    }
    if lines.is_empty() {
        return;
    }
    let max_w = ((aw * 0.6).min(aw - 2.0 * m) - 2.0 * PAD).max(40.0);
    let styles = [
        (TextStyle::new(FONT_TITLE * ts).weight(600.0), Color::WHITE),
        (
            TextStyle::new(FONT_BODY * ts),
            Color::WHITE.with_alpha(0.82),
        ),
    ];
    let blocks: Vec<_> = lines
        .iter()
        .take(2)
        .zip(styles)
        .map(|(l, (st, c))| (text.layout(l, st, Some(max_w), Align::Start), c))
        .collect();
    let gap = 4.0 * ts;
    let w = blocks.iter().map(|b| b.0.width).fold(0.0, f32::max) + 2.0 * PAD;
    let h = blocks.iter().map(|b| b.0.height).sum::<f32>()
        + gap * (blocks.len() - 1) as f32
        + 2.0 * PAD;
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = ay + ah - m - h;
    dl.rect(Rect::new(x, y, w, h), RADIUS_MD, back);
    let mut ty = y + PAD;
    for (b, c) in &blocks {
        let tx = if rtl { x + w - PAD - b.width } else { x + PAD };
        text.draw(dl, b, tx, ty, *c);
        ty += b.height + gap;
    }
}

// ----- The show in the viewer ---------------------------------------------------------------

/// What a slide is like on the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Slide {
    /// Sharp at the window's size.
    Ready,
    /// Has some pixels (an overview), not sharp yet.
    Partial,
    Loading,
    Failed,
}

struct Run {
    playlist: Playlist,
    timer: Timer,
    paused: bool,
    /// Whether the timer counted at the last tick.
    counting: bool,
    entered_fullscreen: bool,
    /// The folder listed again with its subfolders for the show (the show's own listing,
    /// listed anew when the sort order changes).
    rescanned: Option<PathBuf>,
    /// Where the show started (the cycle's start in order).
    start: PathBuf,
    /// The current slide's path.
    shown: Option<PathBuf>,
    anim: AnimWatch,
    /// When the current slide was due but the next was not ready yet.
    waiting: Option<f64>,
    seed: u64,
}

/// A cross-fade to `to`; starts when its scene is first drawn.
#[derive(Clone, Copy, Debug)]
pub(super) struct Fade {
    pub to: ImageId,
    pub start: Option<f64>,
}

#[derive(Default)]
pub(super) struct Slideshow {
    run: Option<Run>,
    crossfade: Option<Crossfade>,
    fade: Option<Fade>,
    pointer_moved: f64,
    /// Ken Burns moved the view (fit again when the show stops).
    moved_view: bool,
    /// Ken Burns's last move of the view on the current slide.
    ken_burns: Option<KenBurns>,
    /// The listing the playlist was planned for, when a new one replaced it since the last
    /// tick (to carry the playlist over by path).
    old_listing: Option<Vec<PathBuf>>,
}

/// Ken Burns on one slide: its last move of the view, to tell the user's pan or zoom from its
/// own. Once the user moves the view the drift stops for the rest of that slide, so what they
/// zoomed in on stays put; the next slide drifts again.
#[derive(Clone, Copy, Debug)]
pub(super) struct KenBurns {
    slide: ImageId,
    revision: u64,
    viewport: PxRect,
    suspended: bool,
}

impl KenBurns {
    /// Moves `view` along the drift of `slide` (`seed` gives its direction) at `p` (0–1) of
    /// its time on screen, unless the user has taken the view over on this slide (dragging,
    /// or any change of the view since the last move other than a new viewport). Returns
    /// whether the view moved.
    pub fn drive(
        state: &mut Option<KenBurns>,
        slide: ImageId,
        view: &mut View,
        p: f32,
        seed: u64,
    ) -> bool {
        if let Some(k) = state.as_mut().filter(|k| k.slide == slide) {
            let user =
                view.is_dragging() || (view.revision != k.revision && view.viewport == k.viewport);
            k.suspended |= user;
            if k.suspended {
                return false;
            }
        }
        let (s, c) = ken_burns_target(view, p, seed);
        let (cx, cy) = view.center();
        let moved =
            (view.scale() / s - 1.0).abs() >= 1e-4 || (cx - c.0).abs() + (cy - c.1).abs() >= 0.05;
        if moved {
            view.zoom_direct_by(s / view.scale(), None);
            view.center_on(c, false);
        }
        *state = Some(KenBurns {
            slide,
            revision: view.revision,
            viewport: view.viewport,
            suspended: false,
        });
        moved
    }

    /// Whether the drift on `slide` stopped for the user.
    pub fn suspended(state: Option<KenBurns>, slide: Option<ImageId>) -> bool {
        state.is_some_and(|k| Some(k.slide) == slide && k.suspended)
    }
}

/// Ken Burns at `p` (0–1) of a slide's time on screen: a slow zoom (in or out by [`KB_ZOOM`])
/// and a pan across the slide, in a direction of its own (from `seed`). The scale and the
/// display point in the middle of the view.
pub(super) fn ken_burns_target(view: &View, p: f32, seed: u64) -> (f32, (f32, f32)) {
    let mut h = seed;
    let r = next_u64(&mut h);
    let zoom_in = r & 1 == 0;
    let dx = ((r >> 8) & 0xff) as f32 / 127.5 - 1.0;
    let dy = ((r >> 16) & 0xff) as f32 / 127.5 - 1.0;
    let z = if zoom_in {
        1.0 + KB_ZOOM * p
    } else {
        1.0 + KB_ZOOM * (1.0 - p)
    };
    let s = view.fit_scale() * z;
    let (dw, dh) = view.display_size();
    let vp = view.viewport;
    let ox = ((dw - vp.w / s) * 0.5).max(0.0);
    let oy = ((dh - vp.h / s) * 0.5).max(0.0);
    let t = 2.0 * p - 1.0;
    (s, (dw * 0.5 + dx * t * ox, dh * 0.5 + dy * t * oy))
}

impl Slideshow {
    /// The GPU device was replaced: the crossfade is made again (a fade under way ends).
    pub(super) fn gpu_reset(&mut self) {
        self.crossfade = None;
        self.fade = None;
    }

    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    /// Whether prefetch wraps around the folder's ends (looping in order).
    pub fn wraps(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.playlist.repeat)
    }

    /// The next slides to prefetch first, without `cur`.
    pub fn upcoming(&mut self, cur: usize) -> Vec<usize> {
        let Some(r) = self.run.as_mut() else {
            return Vec::new();
        };
        let mut v = r.playlist.peek(AHEAD);
        v.retain(|&i| i != cur);
        v.dedup();
        v
    }

    /// The folder's listing was replaced by a new one: while the show runs the first replaced
    /// one is kept until the next tick, which carries the playlist over to the new listing.
    pub fn listing_replaced(&mut self, old: Vec<PathBuf>) {
        if self.run.is_some() && self.old_listing.is_none() {
            self.old_listing = Some(old);
        }
    }

    /// Whether the pointer should be hidden now.
    pub fn hides_pointer(&self, now: f64) -> bool {
        self.run.is_some() && now - self.pointer_moved >= POINTER_IDLE_S
    }

    /// Keeps the outgoing scene before the incoming slide's scene is drawn: when the fade is
    /// armed for `to`, the scene changes from another image to it, and both have pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn before_scene(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        comp: &Compositor,
        from: Option<ImageId>,
        to: Option<ImageId>,
        both_have_pixels: bool,
        now: f64,
    ) {
        let Some(f) = &mut self.fade else { return };
        if f.start.is_some() || to != Some(f.to) || from == to {
            return;
        }
        if from.is_none() || !both_have_pixels {
            self.fade = None;
            return;
        }
        self.crossfade
            .get_or_insert_with(|| Crossfade::new(gpu))
            .snapshot(gpu, enc, comp);
        f.start = Some(now);
    }

    /// Whether [`Slideshow::draw_fade`] may draw this frame (a fade has started).
    pub fn fade_started(&self) -> bool {
        matches!(self.fade, Some(Fade { start: Some(_), .. })) && self.crossfade.is_some()
    }

    /// Blends the outgoing slide over the composite; `true` while the fade runs.
    pub fn draw_fade(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        comp: &Compositor,
        now: f64,
    ) -> bool {
        let Some(Fade { start: Some(s), .. }) = self.fade else {
            if self.fade.is_none() {
                self.release_snapshot();
            }
            return false;
        };
        let t = (now - s) / FADE_S;
        if t >= 1.0 {
            self.fade = None;
            self.release_snapshot();
            return false;
        }
        if let Some(c) = &self.crossfade {
            c.draw(enc, comp, fade_weight(t));
        }
        true
    }

    /// Frees the outgoing slide's snapshot once its fade is over: a scene-sized half-float
    /// texture (66 MB at 4K) is worth more to the image cache for the seconds a slide is on
    /// than its allocation costs when the next fade starts.
    fn release_snapshot(&mut self) {
        if let Some(c) = &mut self.crossfade {
            c.release();
        }
    }
}

impl App {
    fn slide(&self, idx: usize) -> Slide {
        self.slide_of(
            self.entries
                .get(idx)
                .and_then(|p| self.by_path.get(p))
                .copied(),
        )
    }

    /// What image `id` is like on the GPU (sharp at the size of every window it shows in).
    pub(super) fn slide_of(&self, id: Option<ImageId>) -> Slide {
        let Some(id) = id else {
            return Slide::Loading;
        };
        let Some(img) = self.images.get(&id) else {
            return Slide::Loading;
        };
        if img.error.is_some() {
            return Slide::Failed;
        }
        if !self.gpu_images.contains(id) || img.gpu_rank.is_none() {
            return Slide::Loading;
        }
        match (self.sharp_scale(id), img.gpu_rank) {
            (Some(s), Some(r)) if r <= s => Slide::Ready,
            _ => Slide::Partial,
        }
    }

    fn slideshow_start(&mut self) {
        let Some(cur) = self.current.filter(|&c| c < self.entries.len()) else {
            return;
        };
        self.gallery_dismiss();
        let now = self.now();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x5EED, |d| d.as_nanos() as u64);
        let s = &self.settings.slideshow;
        let mut playlist = Playlist::new(self.entries.len(), cur, s.shuffle, s.repeat, seed);
        // A show of pictures passes over text files (and one started on a text file shows
        // only text files).
        playlist.set_skip(
            (0..self.entries.len())
                .map(|i| self.steps_over(cur, i))
                .collect(),
        );
        // Subfolders: the folder listed again, recursively, in its sort order; the playlist
        // follows when the listing arrives.
        let rescanned = self.show_folder();
        if let Some(dir) = &rescanned {
            let b = &self.settings.browsing;
            self.loader.scan_folder(
                dir.clone(),
                recursive_listing(b.sort_order, b.sort_descending),
            );
        }
        let entered_fullscreen = !self.fullscreen;
        if entered_fullscreen {
            self.toggle_fullscreen();
        }
        let path = self.entries[cur].clone();
        self.slideshow.run = Some(Run {
            playlist,
            timer: Timer::default(),
            paused: false,
            counting: false,
            entered_fullscreen,
            rescanned,
            start: path.clone(),
            shown: Some(path),
            anim: AnimWatch::default(),
            waiting: None,
            seed,
        });
        self.slideshow.pointer_moved = now;
        self.chrome.set_presenting(true);
        self.chrome.announce_status(t!("slideshow-started"));
        self.schedule();
        self.redraw = true;
    }

    pub(super) fn slideshow_stop(&mut self) {
        let Some(run) = self.slideshow.run.take() else {
            return;
        };
        self.slideshow.fade = None;
        self.slideshow.crossfade = None;
        self.slideshow.ken_burns = None;
        self.slideshow.old_listing = None;
        if std::mem::take(&mut self.slideshow.moved_view) {
            self.view.zoom_fit();
            self.view.settle();
        }
        if run.entered_fullscreen && self.fullscreen {
            self.toggle_fullscreen();
        }
        self.chrome.set_presenting(false);
        if run.rescanned.is_some() {
            // Back to the folder's own listing: the folder of the image the show stopped at.
            let dir = self
                .current
                .and_then(|i| self.entries.get(i))
                .and_then(|p| p.parent())
                .map(PathBuf::from)
                .or_else(|| self.folder.clone());
            if let Some(dir) = dir {
                self.scan_folder(dir);
            }
        }
        self.chrome.announce_status(t!("slideshow-stopped"));
        self.scene = None;
        self.redraw = true;
    }

    /// The folder a show lists again with its subfolders (`slideshow.include_subfolders`, when
    /// browsing does not list them anyway).
    pub(super) fn show_folder(&self) -> Option<PathBuf> {
        let s = &self.settings;
        if !s.slideshow.include_subfolders || s.browsing.include_subfolders || self.in_book() {
            return None;
        }
        self.folder.clone().filter(|d| d.is_dir())
    }

    /// The sort order or a subfolder setting changed while the show runs: its own listing is
    /// listed again in the new order (or, subfolders switched off, the folder's own listing
    /// comes back); the playlist follows by path when it arrives. Returns whether the show
    /// took care of the listing.
    pub(super) fn slideshow_relist(&mut self) -> bool {
        if !self.slideshow.running() {
            return false;
        }
        let want = self.show_folder();
        let b = &self.settings.browsing;
        let Some(run) = self.slideshow.run.as_mut() else {
            return false;
        };
        let had = std::mem::replace(&mut run.rescanned, want.clone());
        let Some(opts) = show_relisting(want.is_some(), had.is_some(), b) else {
            return false;
        };
        if let Some(dir) = want.or_else(|| self.folder.clone()) {
            self.loader.scan_folder(dir, opts);
        }
        true
    }

    /// Goes to the playlist's current slide (after a step), with the transition.
    fn slideshow_show(&mut self, forward: bool) {
        let Some(run) = self.slideshow.run.as_mut() else {
            return;
        };
        let Some(idx) = run.playlist.current() else {
            return;
        };
        run.timer.restart();
        run.anim = AnimWatch::default();
        run.waiting = None;
        run.shown = self.entries.get(idx).cloned();
        self.nav_dir = if forward { 1 } else { -1 };
        let before = self.current_id();
        self.go_to(idx);
        let to = self.current_id();
        self.slideshow.fade = match (self.settings.slideshow.transition, to) {
            (SlideshowTransition::None, _) | (_, None) => None,
            (_, Some(to)) if Some(to) == before => None,
            (_, Some(to)) => Some(Fade { to, start: None }),
        };
    }

    /// Moves the playlist `steps` forward (or one back, through what was on before).
    fn slideshow_step(&mut self, forward: bool, steps: usize) -> bool {
        let Some(run) = self.slideshow.run.as_mut() else {
            return false;
        };
        let mut moved = false;
        if forward {
            for _ in 0..steps {
                if run.playlist.advance().is_none() {
                    break;
                }
                moved = true;
            }
        } else {
            moved = run.playlist.back().is_some();
        }
        if moved {
            self.slideshow_show(forward);
        }
        moved
    }

    /// `s`, and ←, → and Esc while the show runs. Returns whether the action was taken.
    pub(super) fn slideshow_action(&mut self, a: Action) -> bool {
        match a {
            Action::Slideshow => {
                if self.slideshow.running() {
                    self.slideshow_stop();
                } else {
                    self.slideshow_start();
                }
                true
            }
            _ if !self.slideshow.running() => false,
            Action::NextImage | Action::PrevImage => {
                let forward = a == Action::NextImage;
                if !self.slideshow_step(forward, 1)
                    && let Some(r) = self.slideshow.run.as_mut()
                {
                    // At an end: the timer starts over all the same.
                    r.timer.restart();
                }
                true
            }
            Action::Back => {
                if !(self.chrome.back() || self.cancel_held_tool()) {
                    self.slideshow_stop();
                }
                true
            }
            // Jumps would leave the playlist's order.
            Action::FirstImage | Action::LastImage => true,
            _ => false,
        }
    }

    /// Space pauses and resumes; held ← and → do not flip through the show. Returns whether
    /// the key was taken.
    pub(super) fn slideshow_key(&mut self, combo: &KeyCombo, repeat: bool) -> bool {
        if !self.slideshow.running() {
            return false;
        }
        if combo.key == Key::Named(NamedKey::Space) && combo.mods == Mods::NONE {
            if !repeat && let Some(r) = self.slideshow.run.as_mut() {
                r.paused = !r.paused;
                let msg = if r.paused {
                    t!("slideshow-paused")
                } else {
                    t!("slideshow-resumed")
                };
                self.chrome.announce_status(msg);
                self.redraw = true;
            }
            return true;
        }
        repeat
            && matches!(
                self.keymap.lookup(combo),
                Some(Action::NextImage | Action::PrevImage)
            )
    }

    pub(super) fn slideshow_pointer(&mut self, p: (f32, f32)) {
        if self
            .pointer
            .is_some_and(|q| (q.0 - p.0).abs() + (q.1 - p.1).abs() > 2.0)
        {
            self.slideshow.pointer_moved = self.now();
            if self.slideshow.running() {
                self.redraw = true;
            }
        }
    }

    /// The show's clock, from `about_to_wait`: follows listing changes, counts the timer and
    /// moves on when the slide is due and the next one is ready.
    pub(super) fn slideshow_tick(&mut self) {
        if self.slideshow.run.is_none() {
            return;
        }
        let now = self.now();
        let Some(cur) = self.current.filter(|&c| c < self.entries.len()) else {
            self.slideshow_stop();
            return;
        };
        if self.gallery_active() {
            self.slideshow_stop();
            return;
        }
        // A new listing (subfolders arrived, a new sort order, files came or went) or another
        // image chosen: the playlist carries over by path. Shuffle switched on or off takes
        // effect at once.
        let old = self.slideshow.old_listing.take();
        let cur_path = self.entries.get(cur).cloned();
        let mut relisted = false;
        let skip: Vec<bool> = (0..self.entries.len())
            .map(|i| self.steps_over(cur, i))
            .collect();
        if let Some(run) = self.slideshow.run.as_mut() {
            let s = &self.settings.slideshow;
            run.playlist.repeat = s.repeat;
            if old.is_some() || run.shown != cur_path || run.playlist.current() != Some(cur) {
                run.playlist.skip = skip;
                let start = self
                    .path_index
                    .get(&self.entries, &run.start)
                    .unwrap_or(cur);
                let n = self.entries.len();
                match &old {
                    Some(old) => run.playlist.relist(n, cur, start, |i| {
                        old.get(i)
                            .and_then(|p| self.path_index.get(&self.entries, p))
                    }),
                    None => run.playlist.relist(n, cur, start, Some),
                }
                if run.shown != cur_path {
                    run.timer.restart();
                    run.anim = AnimWatch::default();
                    run.shown = cur_path.clone();
                }
                relisted = true;
            }
            if run.playlist.shuffled() != s.shuffle {
                if !s.shuffle
                    && let Some(p) = &cur_path
                {
                    // In order from here: the next cycles start at this image.
                    run.start = p.clone();
                }
                run.playlist.set_shuffle(s.shuffle);
                relisted = true;
            }
        }
        if relisted {
            self.schedule();
        }
        // A fade that never started (the image changed under it) is dropped.
        if let Some(f) = self.slideshow.fade
            && f.start.is_none()
            && self.current_id() != Some(f.to)
        {
            self.slideshow.fade = None;
        }
        let state = self.slide(cur);
        let playback = self.playback();
        let modal = self.chrome.modal().is_some();
        let interval = self.settings.slideshow.interval.clamp(1, 60) as f64;
        let Some(run) = self.slideshow.run.as_mut() else {
            return;
        };
        let counting = !run.paused && !modal && matches!(state, Slide::Ready | Slide::Failed);
        run.counting = counting;
        run.timer.tick(now, counting);
        run.anim.observe(playback);
        let elapsed = run.timer.at(now);
        // A slide that failed after all is passed over at once.
        if state != Slide::Failed && !due(elapsed, interval, playback.is_some(), run.anim.done) {
            return;
        }
        let waiting_since = *run.waiting.get_or_insert(now);
        let n = self.entries.len();
        let ahead = run.playlist.peek(n.min(64));
        let mut steps = None;
        for (k, &idx) in ahead.iter().enumerate() {
            match self.slide(idx) {
                Slide::Failed => continue,
                Slide::Ready => steps = Some(k + 1),
                Slide::Partial if now - waiting_since >= PATIENCE_S => steps = Some(k + 1),
                _ => {}
            }
            break;
        }
        match steps {
            Some(k) => {
                self.slideshow_step(true, k);
            }
            // The end of the show: every image has been on.
            None if ahead.len() < n.min(64)
                && ahead.iter().all(|&i| self.slide(i) == Slide::Failed) =>
            {
                self.slideshow_stop();
            }
            None => {}
        }
    }

    /// When the show needs the next frame (s from now): the timer, a slide that is waited
    /// for, the pointer to hide, or a fade or Ken Burns running (0).
    pub(super) fn slideshow_wake(&self, now: f64) -> Option<f64> {
        let run = self.slideshow.run.as_ref()?;
        let mut wake = f64::INFINITY;
        if self.slideshow.fade.is_some() || (run.counting && self.ken_burns_drifting()) {
            wake = 0.0;
        }
        if run.counting {
            let interval = self.settings.slideshow.interval.clamp(1, 60) as f64;
            let left = interval - run.timer.at(now);
            // Animations are watched for their play-through.
            wake = wake.min(if self.anim.is_some() {
                left.clamp(0.0, 0.1)
            } else {
                left.max(0.0)
            });
        }
        if run.waiting.is_some() {
            wake = wake.min(0.25);
        }
        let idle = now - self.slideshow.pointer_moved;
        if idle < POINTER_IDLE_S {
            wake = wake.min(POINTER_IDLE_S - idle);
        }
        wake.is_finite().then_some(wake)
    }

    pub(super) fn ken_burns(&self) -> bool {
        self.settings.slideshow.transition == SlideshowTransition::KenBurns
            && !self.chrome.reduce_motion
    }

    /// Ken Burns: a slow zoom and pan across the slide over its time on screen, in a direction
    /// of its own ([`KenBurns`]); a pan or zoom of the user's stops it for the rest of the
    /// slide. Before the scene is drawn.
    pub(super) fn slideshow_ken_burns(&mut self, now: f64) {
        let Some(run) = self.slideshow.run.as_ref() else {
            return;
        };
        if !self.ken_burns() {
            return;
        }
        let (Some(cur), Some(id)) = (self.current, self.current_id()) else {
            return;
        };
        if self.shown != Some(id) || self.slide(cur) != Slide::Ready {
            return;
        }
        let interval = self.settings.slideshow.interval.clamp(1, 60) as f64;
        let p = (run.timer.at(now) / interval).clamp(0.0, 1.0) as f32;
        // The slide's own direction, from the run's seed and the image.
        let seed = run.seed ^ (cur as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if KenBurns::drive(&mut self.slideshow.ken_burns, id, &mut self.view, p, seed) {
            self.slideshow.moved_view = true;
        }
    }

    /// Whether Ken Burns drifts the current slide (not stopped for the user's pan or zoom).
    fn ken_burns_drifting(&self) -> bool {
        self.ken_burns() && !KenBurns::suspended(self.slideshow.ken_burns, self.current_id())
    }

    /// The caption's lines and whether the show is paused, while it runs.
    pub(super) fn slideshow_overlay(&self) -> Option<(Vec<String>, bool)> {
        let run = self.slideshow.run.as_ref()?;
        let mode = self.settings.slideshow.caption;
        let lines = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .map(|img| caption_lines(&img.name(), img.metadata.as_deref(), mode))
            .unwrap_or_default();
        Some((lines, run.paused))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A show passes over the entries marked to skip (text files among pictures), in order,
    /// shuffled and on repeat; one started on a text file shows only the text files.
    #[test]
    fn skips_the_other_kind_of_file() {
        let text = vec![false, true, false, false, true, false];
        let mut p = Playlist::new(6, 0, false, false, 1);
        p.set_skip(text.clone());
        assert_eq!(run_through(&mut p, 10), [0, 2, 3, 5]);
        let mut p = Playlist::new(6, 3, false, true, 1);
        p.set_skip(text.clone());
        assert_eq!(run_through(&mut p, 6), [3, 5, 0, 2, 3, 5, 0]);
        for seed in 1..10 {
            let mut p = Playlist::new(6, 0, true, true, seed);
            p.set_skip(text.clone());
            let v = run_through(&mut p, 12);
            assert!(v.iter().all(|&i| !text[i]), "seed {seed}: {v:?}");
        }
        let mut p = Playlist::new(6, 1, false, true, 1);
        p.set_skip(text.iter().map(|t| !t).collect());
        assert_eq!(run_through(&mut p, 4), [1, 4, 1, 4, 1]);
    }

    fn run_through(p: &mut Playlist, steps: usize) -> Vec<usize> {
        let mut v: Vec<usize> = p.current().into_iter().collect();
        for _ in 0..steps {
            match p.advance() {
                Some(i) => v.push(i),
                None => break,
            }
        }
        v
    }

    #[test]
    fn in_order_from_the_current_image_and_stops_after_all() {
        let mut p = Playlist::new(5, 2, false, false, 1);
        assert_eq!(run_through(&mut p, 10), [2, 3, 4, 0, 1]);
        assert_eq!(p.advance(), None);
        assert_eq!(p.current(), Some(1));
        assert_eq!(p.back(), Some(0));
    }

    #[test]
    fn loop_starts_over_in_order() {
        let mut p = Playlist::new(3, 0, false, true, 1);
        assert_eq!(run_through(&mut p, 7), [0, 1, 2, 0, 1, 2, 0, 1]);
        // ← from the start of the folder goes round too.
        let mut q = Playlist::new(3, 0, false, true, 1);
        assert_eq!(q.back(), Some(2));
        let mut r = Playlist::new(3, 0, false, false, 1);
        assert_eq!(r.back(), None);
    }

    #[test]
    fn shuffle_has_no_repeats_until_all_were_shown() {
        for seed in 0..50 {
            let n = 17;
            let mut p = Playlist::new(n, 4, true, true, seed);
            let all = run_through(&mut p, 5 * n - 1);
            assert_eq!(all[0], 4, "starts at the current image");
            for (c, cycle) in all.chunks(n).enumerate() {
                let mut s = cycle.to_vec();
                s.sort_unstable();
                assert_eq!(s, (0..n).collect::<Vec<_>>(), "seed {seed} cycle {c}");
            }
            // No image twice in a row across a cycle boundary.
            assert!(all.windows(2).all(|w| w[0] != w[1]), "seed {seed}");
        }
    }

    #[test]
    fn shuffle_is_deterministic_per_seed_and_ends_without_loop() {
        let a = run_through(&mut Playlist::new(30, 0, true, false, 7), 100);
        let b = run_through(&mut Playlist::new(30, 0, true, false, 7), 100);
        let c = run_through(&mut Playlist::new(30, 0, true, false, 8), 100);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 30, "every image once, then the end");
    }

    #[test]
    fn peek_looks_across_the_loop() {
        let mut p = Playlist::new(3, 1, false, true, 1);
        assert_eq!(p.peek(4), [2, 0, 1, 2]);
        let mut q = Playlist::new(3, 1, false, false, 1);
        assert_eq!(q.peek(4), [2, 0]);
        assert_eq!(p.current(), Some(1), "peeking does not move");
    }

    #[test]
    fn relist_keeps_what_was_seen_behind() {
        let mut p = Playlist::new(4, 0, true, false, 3);
        let seen = run_through(&mut p, 2);
        // The listing grows to 8 (subfolders): the old images are now at 1, 3, 5 and 7.
        let map = [1, 3, 5, 7];
        let remap = |i: usize| map.get(i).copied();
        let cur = map[seen[2]];
        p.relist(8, cur, map[0], remap);
        assert_eq!(p.current(), Some(cur));
        let rest = run_through(&mut p, 20);
        let mut all: Vec<usize> = seen[..2].iter().map(|&i| map[i]).chain(rest).collect();
        all.sort_unstable();
        assert_eq!(
            all,
            (0..8).collect::<Vec<_>>(),
            "each image once this cycle"
        );
        // ← still goes back to what was on before the new listing.
        let mut p = Playlist::new(4, 0, true, false, 3);
        run_through(&mut p, 2);
        p.relist(8, cur, map[0], remap);
        assert_eq!(p.back(), Some(map[seen[1]]));
        assert_eq!(p.back(), Some(map[seen[0]]));
        assert_eq!(p.back(), None);
        // In order: on from the current image through the images not on yet.
        let mut q = Playlist::new(8, 6, false, false, 1);
        q.advance();
        q.relist(10, 7, 6, Some);
        assert_eq!(run_through(&mut q, 20), [7, 8, 9, 0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_new_sort_order_carries_the_show_over() {
        // In order from image 1 of 6; after 1, 2, 3 the listing is sorted the other way round
        // (old i is new 5 − i): the show goes on from 3 (now 2) through the three not on yet
        // (old 0, 5 and 4), in the new order, and ← walks back through 2 and 1 (now 3 and 4).
        let mut p = Playlist::new(6, 1, false, true, 1);
        run_through(&mut p, 2);
        let remap = |i: usize| (i < 6).then(|| 5 - i);
        p.relist(6, 2, 4, remap);
        assert_eq!(p.current(), Some(2));
        let mut q = p.clone();
        assert_eq!(run_through(&mut q, 3), [2, 5, 0, 1]);
        assert_eq!(
            q.advance(),
            Some(4),
            "the next cycle starts where the show began"
        );
        assert_eq!(p.back(), Some(3));
        assert_eq!(p.back(), Some(4));
        // Shuffled: every image once this cycle, across the new order.
        for seed in 0..20 {
            let mut p = Playlist::new(9, 4, true, false, seed);
            let before = run_through(&mut p, 3);
            let remap = |i: usize| (i < 9).then(|| 8 - i);
            p.relist(9, 8 - before[3], 0, remap);
            let mut all: Vec<usize> = before[..3].iter().map(|&i| 8 - i).collect();
            all.extend(run_through(&mut p, 20));
            all.sort_unstable();
            assert_eq!(all, (0..9).collect::<Vec<_>>(), "seed {seed}");
        }
    }

    #[test]
    fn a_show_with_subfolders_keeps_its_listing_when_the_sort_changes() {
        use specular_settings::{Browsing, SortOrder};
        let b = Browsing {
            sort_order: SortOrder::Modified,
            sort_descending: true,
            ..Browsing::default()
        };
        // The show's own listing: with subfolders, in the new order.
        assert_eq!(
            show_relisting(true, true, &b),
            Some(ListOptions {
                order: SortOrder::Modified,
                descending: true,
                recursive: true,
            })
        );
        // Subfolders switched on mid-show.
        assert_eq!(
            show_relisting(true, false, &b),
            show_relisting(true, true, &b)
        );
        // Switched off: the folder's own listing comes back.
        assert_eq!(
            show_relisting(false, true, &b),
            Some(ListOptions {
                order: SortOrder::Modified,
                descending: true,
                recursive: false,
            })
        );
        // A show of the folder's own listing leaves it to browsing.
        assert_eq!(show_relisting(false, false, &b), None);
    }

    #[test]
    fn shuffle_switched_mid_show_takes_effect_at_once() {
        let mut shuffled_somewhere = false;
        for seed in 0..20 {
            let n = 10;
            let mut p = Playlist::new(n, 0, false, false, seed);
            assert_eq!(run_through(&mut p, 3), [0, 1, 2, 3]);
            p.set_shuffle(true);
            assert_eq!(p.current(), Some(3), "the current image stays");
            let rest = run_through(&mut p.clone(), 20);
            assert_eq!(rest[0], 3);
            let mut s = rest[1..].to_vec();
            shuffled_somewhere |= s != [4, 5, 6, 7, 8, 9];
            s.sort_unstable();
            assert_eq!(s, [4, 5, 6, 7, 8, 9], "the images not on yet, once each");
            // ← still goes back in the order they were on.
            let mut b = p.clone();
            assert_eq!([b.back(), b.back(), b.back()], [Some(2), Some(1), Some(0)]);
            // Off again two slides on: in the folder's order after the current image, through
            // the images not on yet.
            p.advance();
            p.advance();
            let on: Vec<usize> = [0, 1, 2, 3, rest[1], rest[2]].into();
            p.set_shuffle(false);
            assert_eq!(p.current(), Some(rest[2]));
            let after = run_through(&mut p, 20);
            let want: Vec<usize> = (0..n)
                .map(|k| (rest[2] + k) % n)
                .filter(|i| !on.contains(i) || *i == rest[2])
                .collect();
            assert_eq!(after, want, "seed {seed}");
        }
        assert!(shuffled_somewhere);
    }

    #[test]
    fn back_walks_through_the_last_cycle_too() {
        for seed in 0..20 {
            let n = 5;
            let mut p = Playlist::new(n, 2, true, true, seed);
            // Two whole cycles and three slides of the third.
            let shown = run_through(&mut p, 2 * n + 2);
            let mut back = Vec::new();
            while let Some(i) = p.back() {
                back.push(i);
            }
            // The current cycle and the one before it; no further.
            let want: Vec<usize> = shown[n..2 * n + 2].iter().rev().copied().collect();
            assert_eq!(back, want, "seed {seed}");
            // → after ← shows the same slides again, then goes on.
            let again = run_through(&mut p, n + 2);
            assert_eq!(again, shown[n..], "seed {seed}");
            // A new listing (one more image) keeps the history for ←.
            let mut q = Playlist::new(n, 0, true, true, seed);
            let shown = run_through(&mut q, n + 1);
            q.relist(n + 1, shown[n + 1], 0, Some);
            let mut back = Vec::new();
            while let Some(i) = q.back() {
                back.push(i);
            }
            let want: Vec<usize> = shown[..=n].iter().rev().copied().collect();
            assert_eq!(back, want, "seed {seed}");
        }
        // History stays bounded on a long show.
        let mut p = Playlist::new(4, 0, true, true, 9);
        run_through(&mut p, 1000);
        assert!(p.order.len() <= 4 * 3 + 2, "{}", p.order.len());
    }

    #[test]
    fn ken_burns_stops_for_the_users_pan_or_zoom() {
        let mut view = View::new();
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        view.set_viewport(vp);
        view.set_image(3000, 2000, specular_core::Orientation::NORMAL);
        let mut kb = None;
        let seed = 0x1234;
        assert!(KenBurns::drive(&mut kb, 1, &mut view, 0.1, seed));
        assert!(KenBurns::drive(&mut kb, 1, &mut view, 0.2, seed));
        // A new viewport (the window went full screen) is not the user's: the drift goes on.
        view.set_viewport(PxRect { w: 1000.0, ..vp });
        assert!(KenBurns::drive(&mut kb, 1, &mut view, 0.3, seed));
        assert!(!KenBurns::suspended(kb, Some(1)));
        // The user pans: the view stays where they put it for the rest of the slide.
        view.pan_by(40.0, 0.0, false);
        let (scale, center) = (view.scale(), view.center());
        for p in [0.4, 0.6, 1.0] {
            assert!(!KenBurns::drive(&mut kb, 1, &mut view, p, seed));
        }
        assert_eq!((view.scale(), view.center()), (scale, center));
        assert!(KenBurns::suspended(kb, Some(1)));
        // The next slide drifts again.
        view.set_image(2000, 3000, specular_core::Orientation::NORMAL);
        assert!(KenBurns::drive(&mut kb, 2, &mut view, 0.1, seed));
        assert!(!KenBurns::suspended(kb, Some(2)));
        // A zoom, or a drag in progress, stops it too.
        view.zoom_direct_by(1.5, Some((100.0, 100.0)));
        assert!(!KenBurns::drive(&mut kb, 2, &mut view, 0.2, seed));
        view.set_image(2000, 3000, specular_core::Orientation::NORMAL);
        assert!(KenBurns::drive(&mut kb, 3, &mut view, 0.1, seed));
        view.begin_drag(0.0);
        assert!(!KenBurns::drive(&mut kb, 3, &mut view, 0.2, seed));
    }

    #[test]
    fn timer_counts_only_while_running_with_a_fake_clock() {
        let mut t = Timer::default();
        t.tick(10.0, false); // not ready yet
        t.tick(11.0, true);
        t.tick(13.0, true);
        assert_eq!(t.at(13.0), 2.0);
        assert_eq!(t.at(13.5), 2.5, "between ticks");
        t.tick(14.0, false); // paused: the second up to here is lost, as nothing counted it
        t.tick(20.0, false);
        assert_eq!(t.at(20.0), 2.0, "paused time does not count");
        t.tick(21.0, true); // resumed
        t.tick(24.0, true);
        assert_eq!(t.at(24.0), 5.0);
        assert!(due(t.at(24.0), 5.0, false, false));
        assert!(!due(t.at(24.0), 6.0, false, false));
        t.restart();
        assert_eq!(t.at(30.0), 0.0);
    }

    #[test]
    fn animations_play_through_up_to_a_cap() {
        assert!(!due(5.0, 5.0, true, false));
        assert!(due(5.0, 5.0, true, true));
        assert!(!due(4.0, 5.0, true, true), "the interval at least");
        assert!(due(ANIM_CAP_S, 5.0, true, false), "capped");
        let mut w = AnimWatch::default();
        let pb = |frame, playing| {
            Some(Playback {
                frame,
                count: Some(3),
                playing,
                speed: 1.0,
                video: None,
            })
        };
        for f in [0, 1, 2] {
            w.observe(pb(f, true));
            assert!(!w.done);
        }
        w.observe(pb(0, true));
        assert!(w.done, "wrapped round");
        let mut once = AnimWatch::default();
        once.observe(pb(2, false));
        assert!(once.done, "stopped after its loops");
    }

    #[test]
    fn fade_weight_eases_through_the_midpoint() {
        assert_eq!(fade_weight(0.0), 1.0);
        assert_eq!(fade_weight(0.5), 0.5);
        assert_eq!(fade_weight(1.0), 0.0);
        assert!(fade_weight(0.1) > 0.9);
    }

    #[test]
    fn caption_lines_follow_the_setting() {
        let r = MetadataReport {
            exposure_line: Some("1/250 s · f/8 · ISO 100".into()),
            summary: vec![specular_metadata::SummaryGroup {
                key: "camera",
                title: "Camera".into(),
                items: vec![specular_metadata::SummaryItem {
                    key: "camera",
                    label: "Camera".into(),
                    value: "Fujifilm X-T5".into(),
                    detail: None,
                }],
            }],
            ..MetadataReport::default()
        };
        assert!(caption_lines("a.jpg", Some(&r), SlideshowCaption::Off).is_empty());
        assert_eq!(
            caption_lines("a.jpg", Some(&r), SlideshowCaption::Name),
            ["a.jpg"]
        );
        assert_eq!(
            caption_lines("a.jpg", Some(&r), SlideshowCaption::NameExif),
            ["a.jpg", "Fujifilm X-T5 · 1/250 s · f/8 · ISO 100"]
        );
        assert_eq!(
            caption_lines("a.jpg", None, SlideshowCaption::NameExif),
            ["a.jpg"]
        );
    }

    #[test]
    fn subfolders_are_listed_recursively_in_the_folder_order() {
        let root = std::env::temp_dir().join(format!("specular-slideshow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in [
            "b.jpg",
            "a.jpg",
            "trip/2.jpg",
            "trip/10.jpg",
            "trip/day/1.png",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        let reg = specular_decode::Registry::new();
        let opts = recursive_listing(specular_settings::SortOrder::Natural, false);
        let files: Vec<String> = crate::loader::list_folder(&root, &reg, opts)
            .iter()
            .map(|p| {
                // `/`-separated on every platform, to compare with the lists below.
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            files,
            [
                "a.jpg",
                "b.jpg",
                "trip/2.jpg",
                "trip/10.jpg",
                "trip/day/1.png"
            ]
        );
        // A show started at b.jpg goes through the subfolders and stops before a.jpg.
        let mut p = Playlist::new(files.len(), 1, false, false, 0);
        assert_eq!(run_through(&mut p, 10), [1, 2, 3, 4, 0]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[cfg(test)]
mod shots;

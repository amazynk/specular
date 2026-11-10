//! Multi-monitor (DESIGN.md §6.5): a slideshow in a full-screen presentation window on another
//! display, and the viewer on another display following the main window's selection.
//!
//! The presentation window has its own surface, compositor, UI renderer and text, but shares
//! the GPU device, the image renderer and the GPU image cache with the main window, and its
//! images are the viewer's own (`App::images`): an image both windows show is decoded once.
//! The main window stays fully usable; it shows a "Presenting on …" control with Pause and
//! Stop. Which display: `[slideshow] monitor` ("auto": one other than the main window's).
//! With one display the slideshow runs in the main window as before (`slideshow.rs`).
//!
//! The presentation window shows whole-image overviews only (sharp at fit for its size; no
//! detail tiles), so a slide is ready once its overview at that size is on the GPU.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::Priority;
use specular_gfx::{Backdrop, Compositor, Crossfade, DrawList, Frame, Text, UiRenderer, wgpu};
use specular_gfx::{WindowSurface, winit};
use specular_i18n::{I18n, isolate_ltr, t};
use specular_input::{Action, Key, Mods, NamedKey};
use specular_render::{PxRect, RenderOptions, View};
use specular_settings::{HdrOutput, MonitorChoice, SlideshowCaption, SlideshowTransition};
use specular_widgets::Presenting;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window};

use super::slideshow::{
    AHEAD, FADE_S, Fade, KenBurns, PATIENCE_S, POINTER_IDLE_S, Playlist, Slide, Timer,
    caption_lines, draw_overlay, due, fade_weight, recursive_listing,
};
use super::{App, HDR_FALLBACK_HEADROOM, Img, settings_color, stands_in, with_app_id};
use crate::loader::ImageId;
use specular_log::{GPU, INPUT, SETTINGS, debug, info};

/// How often the displays are listed (for unplugged ones and the Settings list).
const MONITOR_POLL: Duration = Duration::from_secs(1);
/// How often the presentation window's HDR state is checked.
const HDR_POLL: Duration = Duration::from_millis(500);

// ----- Choosing the display -----------------------------------------------------------------

/// The display to present on, as an index into `monitors` (the connected displays' names):
/// the one `choice` names when it is connected and is not the main window's, else (and for
/// "auto") the first that is not the main window's. `None` when there is no other display, or
/// the main window's display is not known (`main`): then the slideshow runs in the main
/// window, as with one display.
pub(super) fn pick_monitor(
    choice: &MonitorChoice,
    monitors: &[String],
    main: Option<&str>,
) -> Option<usize> {
    let main = main?;
    if let MonitorChoice::Named(n) = choice
        && n != main
        && let Some(i) = monitors.iter().position(|m| m == n)
    {
        return Some(i);
    }
    monitors.iter().position(|m| m != main)
}

/// Whether the presentation window keeps its last picture while the image to show (`target`,
/// its entry `img`) has no pixels (`shown_pixels`). `kept`: the scene holds an image's
/// pixels drawn at the current size and canvas. As the main window's stand-in
/// ([`stands_in`]), with no time limit (as while a key is held): this window shows no
/// spinner, so a slide or a page switched to ([`Img::page_hold`]) keeps the old picture
/// until it has pixels. An error ends it.
fn keeps_picture(
    kept: bool,
    shown_pixels: bool,
    target: Option<ImageId>,
    img: Option<&Img>,
) -> bool {
    !shown_pixels && kept && target.is_some() && img.is_none_or(|i| stands_in(i, true))
}

/// Where a presentation goes when its display is unplugged: another display that is not the
/// main window's (`None`: close it).
pub(super) fn after_unplug(monitors: &[String], main: Option<&str>) -> Option<usize> {
    pick_monitor(&MonitorChoice::Auto, monitors, main)
}

/// The connected displays, listed once a second.
#[derive(Default)]
pub(super) struct Monitors {
    checked: Option<Instant>,
    names: Vec<String>,
    /// Displays found unplugged: winit on Wayland can keep listing a removed output, so
    /// these are left out until a display is plugged in (the raw list grows).
    lost: Vec<String>,
    /// The raw list's length at the last poll.
    raw: usize,
}

// ----- The presentation window --------------------------------------------------------------

/// A slideshow of its own in the presentation window.
struct Show {
    listing: Vec<PathBuf>,
    playlist: Playlist,
    timer: Timer,
    paused: bool,
    /// Whether the timer counted at the last tick.
    counting: bool,
    /// When the current slide was due but the next was not ready yet.
    waiting: Option<f64>,
    seed: u64,
    /// Where the show started (the cycle's start in order).
    start: PathBuf,
    /// The folder listed again with its subfolders for the show, until the listing arrives.
    scanning: Option<PathBuf>,
}

enum Mode {
    Show(Box<Show>),
    /// The viewer on another display: the main window's image (or the gallery's focus).
    Follow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SceneKey {
    id: Option<ImageId>,
    image_rev: u64,
    view_rev: u64,
    size: (u32, u32),
    canvas: [u32; 4],
    /// The movie's picture adjustments (bits), when it is the one shown.
    picture: Option<[u32; 5]>,
}

/// The picture adjustments for the presentation's image `target`: the playing movie's
/// (Alt+p), when it is that movie.
fn picture_for(
    target: Option<ImageId>,
    playing: Option<(ImageId, specular_render::PictureAdjust)>,
) -> Option<specular_render::PictureAdjust> {
    playing
        .filter(|(id, _)| Some(*id) == target)
        .map(|(_, p)| p)
}

pub(super) struct Presenter {
    window: Arc<Window>,
    surface: WindowSurface,
    comp: Compositor,
    ui: UiRenderer,
    text: Text,
    dl: DrawList,
    view: View,
    /// The display's name.
    monitor: String,
    mode: Mode,
    /// The image to show, and the one the view is set up for.
    target: Option<ImageId>,
    shown: Option<ImageId>,
    /// Images kept decoded for this window: the one on screen and the next slides.
    wants: Vec<ImageId>,
    scene: Option<SceneKey>,
    /// The scene texture holds an image's pixels, drawn at this size and canvas (it can stand
    /// in for one still loading; [`keeps_picture`]).
    scene_pixels: Option<((u32, u32), [u32; 4])>,
    crossfade: Option<Crossfade>,
    fade: Option<Fade>,
    ken_burns: Option<KenBurns>,
    display_hash: u64,
    /// Output peak (units of SDR white): 1 for SDR.
    peak: f32,
    hdr_checked: Option<Instant>,
    /// Polls in a row the window was found on the main window's display.
    moved_off: u8,
    pointer: Option<(f32, f32)>,
    pointer_moved: f64,
    cursor_hidden: bool,
    redraw: bool,
    wake_at: Option<Instant>,
}

impl Presenter {
    fn show(&self) -> Option<&Show> {
        match &self.mode {
            Mode::Show(s) => Some(s),
            Mode::Follow => None,
        }
    }

    fn show_mut(&mut self) -> Option<&mut Show> {
        match &mut self.mode {
            Mode::Show(s) => Some(s),
            Mode::Follow => None,
        }
    }

    fn set_target(&mut self, id: Option<ImageId>, fade: bool) {
        if self.target == id {
            return;
        }
        self.fade = match id {
            Some(to) if fade && self.target.is_some() => Some(Fade { to, start: None }),
            _ => None,
        };
        self.target = id;
        self.redraw = true;
    }
}

/// Why a presentation ends (what is announced).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Close {
    /// Stopped by the user or at the end of the show.
    Stopped,
    /// Its display was unplugged and there is no other.
    Unplugged,
    Quiet,
}

impl App {
    /// The connected displays' names, their handles, and the main window's display.
    fn monitors_now(&self) -> (Vec<String>, Vec<MonitorHandle>, Option<String>) {
        let Some(g) = self.gfx.as_ref() else {
            return (Vec::new(), Vec::new(), None);
        };
        let mut names = Vec::new();
        let mut handles = Vec::new();
        for m in g.window.available_monitors() {
            // Some platforms list a display more than once (Wayland: once per output global).
            if let Some(n) = m.name()
                && !names.contains(&n)
                && !self.monitors.lost.contains(&n)
            {
                names.push(n);
                handles.push(m);
            }
        }
        let main = g.window.current_monitor().and_then(|m| m.name());
        (names, handles, main)
    }

    /// The presentation window's id (its events are routed to [`present_event`](Self::present_event)).
    pub(super) fn presenter_window(&self) -> Option<winit::window::WindowId> {
        self.presenter.as_ref().map(|p| p.window.id())
    }

    /// Whether a slideshow runs on another display.
    pub(super) fn present_show_running(&self) -> bool {
        self.presenter.as_ref().is_some_and(|p| p.show().is_some())
    }

    /// The presentation window's images (kept decoded, at its size).
    pub(super) fn presenter_wants(&self, id: ImageId) -> bool {
        self.presenter
            .as_ref()
            .is_some_and(|p| p.wants.contains(&id))
    }

    pub(super) fn presenter_ids(&self) -> Vec<ImageId> {
        self.presenter
            .as_ref()
            .map(|p| p.wants.clone())
            .unwrap_or_default()
    }

    /// The image the presentation window shows.
    pub(super) fn presenter_current(&self) -> Option<ImageId> {
        self.presenter.as_ref().and_then(|p| p.target)
    }

    /// An image's pixels changed on the GPU: the presentation window draws again if it shows it.
    pub(super) fn presenter_uploaded(&mut self, id: ImageId) {
        if let Some(p) = self.presenter.as_mut()
            && p.target == Some(id)
        {
            p.redraw = true;
        }
    }

    /// The size an image is decoded for (physical px): the main window's, or for an image the
    /// presentation window shows, the larger of both.
    pub(super) fn viewport_for(&self, id: ImageId) -> Option<(u32, u32)> {
        let main = self.viewport_px();
        let pres = self
            .presenter
            .as_ref()
            .filter(|p| p.wants.contains(&id))
            .map(|p| p.surface.size())
            .filter(|s| s.0 > 1 && s.1 > 1);
        match (main, pres) {
            (Some(a), Some(b)) => Some((a.0.max(b.0), a.1.max(b.1))),
            (a, b) => a.or(b),
        }
    }

    /// The image for `path`, created when it is not touched yet.
    pub(super) fn id_for_path(&mut self, path: &Path) -> ImageId {
        if let Some(id) = self.by_path.get(path) {
            return *id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.images.insert(id, Img::new(path.to_path_buf()));
        self.by_path.insert(path.to_path_buf(), id);
        id
    }

    /// The main window's "Presenting on …" control.
    pub(super) fn presenting_status(&self) -> Option<Presenting> {
        let p = self.presenter.as_ref()?;
        Some(Presenting {
            display: p.monitor.clone(),
            paused: p.show().map(|s| s.paused),
        })
    }

    /// `s` and the "viewer on another display" command. `s` starts the slideshow on another
    /// display when there is one (and stops it); with one display it is left to the viewer's
    /// slideshow. Returns whether the action was taken.
    pub(super) fn present_action(&mut self, a: Action, el: &ActiveEventLoop) -> bool {
        match a {
            Action::Slideshow => {
                if self.present_show_running() {
                    self.present_close(Close::Stopped);
                    return true;
                }
                if self.slideshow.running() || self.headless() {
                    return false;
                }
                let Some(show) = self.new_show() else {
                    return false;
                };
                if let Some(p) = self.presenter.as_mut() {
                    // The viewer on the other display becomes the show.
                    p.mode = Mode::Show(Box::new(show));
                    p.redraw = true;
                } else {
                    let (names, handles, main) = self.monitors_now();
                    let pick =
                        pick_monitor(&self.settings.slideshow.monitor, &names, main.as_deref());
                    debug!(target: SETTINGS, "slideshow: displays {names:?}, main window on {main:?}: {pick:?}");
                    let Some(i) = pick else {
                        return false;
                    };
                    if !self.present_open(el, &handles[i], &names[i], Mode::Show(Box::new(show))) {
                        return false;
                    }
                }
                let display = self
                    .presenter
                    .as_ref()
                    .map(|p| isolate_ltr(&p.monitor))
                    .unwrap_or_default();
                self.chrome
                    .announce_status(t!("slideshow-started-on", display = display));
                self.present_tick();
                self.redraw = true;
                true
            }
            Action::ViewerOnOtherMonitor => {
                if let Some(p) = self.presenter.as_mut() {
                    if p.show().is_some() {
                        // The show stops; the viewer stays on that display.
                        p.mode = Mode::Follow;
                        p.fade = None;
                        p.redraw = true;
                        self.chrome.announce_status(t!("slideshow-stopped"));
                        self.present_tick();
                    } else {
                        self.present_close(Close::Quiet);
                        self.chrome.announce_status(t!("viewer-on-display-closed"));
                    }
                    self.redraw = true;
                    return true;
                }
                let now = self.now();
                let (names, handles, main) = self.monitors_now();
                let choice = &self.settings.slideshow.monitor;
                match pick_monitor(choice, &names, main.as_deref()) {
                    Some(i) if !self.headless() => {
                        if self.present_open(el, &handles[i], &names[i], Mode::Follow) {
                            let display = isolate_ltr(&names[i]);
                            self.chrome
                                .announce_status(t!("viewer-on-display", display = display));
                            self.present_tick();
                        }
                    }
                    _ => {
                        self.chrome.toast(t!("toast-no-other-display"), None, now);
                    }
                }
                self.redraw = true;
                true
            }
            _ => false,
        }
    }

    /// A show from the current image through the folder (its own listing; with
    /// `include_subfolders` listed again with its subfolders).
    fn new_show(&mut self) -> Option<Show> {
        let cur = self.current.filter(|&c| c < self.entries.len())?;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x5EED, |d| d.as_nanos() as u64);
        let s = &self.settings.slideshow;
        let listing = self.entries.clone();
        let playlist = Playlist::new(listing.len(), cur, s.shuffle, s.repeat, seed);
        let scanning = self.show_folder();
        if let Some(dir) = &scanning {
            let b = &self.settings.browsing;
            self.loader.scan_show(
                dir.clone(),
                recursive_listing(b.sort_order, b.sort_descending),
            );
        }
        Some(Show {
            start: listing[cur].clone(),
            listing,
            playlist,
            timer: Timer::default(),
            paused: false,
            counting: false,
            waiting: None,
            seed,
            scanning,
        })
    }

    /// Opens the presentation window full screen on `monitor`.
    fn present_open(
        &mut self,
        el: &ActiveEventLoop,
        monitor: &MonitorHandle,
        name: &str,
        mode: Mode,
    ) -> bool {
        let Some(g) = self.gfx.as_ref() else {
            return false;
        };
        let attrs = Window::default_attributes()
            .with_title(t!("presenter-window-title"))
            .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor.clone()))))
            // The main window keeps the keyboard where the platform allows.
            .with_active(false);
        let window = match el.create_window(with_app_id(attrs)) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                specular_log::warn!(target: GPU, "presentation window: {e}");
                return false;
            }
        };
        let surface = match WindowSurface::new(&g.gpu, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                specular_log::warn!(target: GPU, "presentation window surface: {e}");
                return false;
            }
        };
        let (w, h) = surface.size();
        let comp = Compositor::new(&g.gpu, surface.format(), surface.bits, w, h);
        comp.set_bits(
            &g.gpu,
            if self.settings.colour.dithering {
                surface.bits
            } else {
                16
            },
        );
        let ui = UiRenderer::new(&g.gpu, &comp);
        let mut text = if self.args.screenshot.is_some() {
            Text::bundled_only()
        } else {
            Text::new()
        };
        text.set_locale(Some(I18n::global().language().to_string()));
        info!(target: SETTINGS, "presenting on {name} ({w}×{h})");
        let now = self.now();
        self.presenter = Some(Presenter {
            window,
            surface,
            comp,
            ui,
            text,
            dl: DrawList::new(1.0),
            view: View::new(),
            monitor: name.to_owned(),
            mode,
            target: None,
            shown: None,
            wants: Vec::new(),
            scene: None,
            scene_pixels: None,
            crossfade: None,
            fade: None,
            ken_burns: None,
            display_hash: 0,
            peak: 1.0,
            hdr_checked: None,
            moved_off: 0,
            pointer: None,
            pointer_moved: now,
            cursor_hidden: false,
            redraw: true,
            wake_at: None,
        });
        true
    }

    /// Closes the presentation window.
    fn present_close(&mut self, why: Close) {
        let Some(p) = self.presenter.take() else {
            return;
        };
        let now = self.now();
        match why {
            Close::Stopped if p.show().is_some() => {
                self.chrome.announce_status(t!("slideshow-stopped"));
            }
            Close::Stopped => {
                self.chrome.announce_status(t!("viewer-on-display-closed"));
            }
            Close::Unplugged => {
                let display = isolate_ltr(&p.monitor);
                self.chrome.toast(
                    t!("toast-display-disconnected", display = display),
                    None,
                    now,
                );
            }
            Close::Quiet => {}
        }
        drop(p);
        // Its images go back to the viewer's prefetch window (or are let go).
        self.schedule();
        self.redraw = true;
    }

    /// The show's listing with subfolders arrived: the playlist follows by path.
    pub(super) fn present_show_listed(&mut self, dir: PathBuf, mut files: Vec<PathBuf>) {
        let Some(show) = self.presenter.as_mut().and_then(Presenter::show_mut) else {
            return;
        };
        if show.scanning.as_ref() != Some(&dir) {
            return;
        }
        show.scanning = None;
        let Some(cur_path) = show
            .playlist
            .current()
            .and_then(|i| show.listing.get(i))
            .cloned()
        else {
            return;
        };
        if !files.contains(&cur_path) {
            files.insert(0, cur_path.clone());
        }
        let index: HashMap<&PathBuf, usize> =
            files.iter().enumerate().map(|(i, p)| (p, i)).collect();
        let cur = index[&cur_path];
        let start = index.get(&show.start).copied().unwrap_or(cur);
        let old = &show.listing;
        show.playlist.relist(files.len(), cur, start, |i| {
            old.get(i).and_then(|p| index.get(p)).copied()
        });
        drop(index);
        show.listing = files;
        debug!(target: SETTINGS, "presentation: {} slides with subfolders", show.listing.len());
        self.present_tick();
    }

    /// The presentation's clock, from `about_to_wait`: displays unplugged, the image to show
    /// (the show's timer and steps, or the main window's selection), the pointer.
    pub(super) fn present_tick(&mut self) {
        self.poll_monitors();
        let Some(p) = self.presenter.as_ref() else {
            return;
        };
        let now = self.now();
        let follow = p.show().is_none();
        if follow {
            let path = if self.gallery_active() {
                self.gallery.focus_file()
            } else {
                self.current.and_then(|i| self.entries.get(i)).cloned()
            };
            let id = path.map(|p| self.id_for_path(&p));
            if let Some(p) = self.presenter.as_mut() {
                p.set_target(id, false);
            }
            self.present_wants(id.into_iter().collect());
        } else {
            self.present_show_tick(now);
        }
        let Some(p) = self.presenter.as_mut() else {
            return;
        };
        if p.wake_at.is_some_and(|t| Instant::now() >= t) {
            p.wake_at = None;
            p.redraw = true;
        }
        let hide = now - p.pointer_moved >= POINTER_IDLE_S;
        if hide != p.cursor_hidden {
            p.window.set_cursor_visible(!hide);
            p.cursor_hidden = hide;
        }
        if std::mem::take(&mut p.redraw) {
            p.window.request_redraw();
        }
    }

    /// Keeps `ids` (the first: on screen) decoded for the presentation window.
    fn present_wants(&mut self, ids: Vec<ImageId>) {
        let Some(p) = self.presenter.as_mut() else {
            return;
        };
        let changed = p.wants != ids;
        p.wants = ids;
        if let Some(&id) = p.wants.first() {
            self.ensure_open(id, Priority::Immediate);
            self.request_levels(id, Priority::Immediate);
            self.upload(id);
            if self.caption_wants_metadata() {
                self.request_metadata_for(id);
            }
        }
        if changed {
            self.schedule();
        }
    }

    fn caption_wants_metadata(&self) -> bool {
        self.settings.slideshow.caption == SlideshowCaption::NameExif && self.present_show_running()
    }

    fn present_show_tick(&mut self, now: f64) {
        let s = self.settings.slideshow.clone();
        let interval = s.interval.clamp(1, 60) as f64;
        let fade = s.transition != SlideshowTransition::None;
        let Some(show) = self.presenter.as_mut().and_then(Presenter::show_mut) else {
            return;
        };
        show.playlist.repeat = s.repeat;
        if show.playlist.shuffled() != s.shuffle {
            if !s.shuffle
                && let Some(p) = show.playlist.current().and_then(|i| show.listing.get(i))
            {
                // In order from here: the next cycles start at this image.
                show.start = p.clone();
            }
            show.playlist.set_shuffle(s.shuffle);
        }
        let paths: Vec<PathBuf> = show
            .playlist
            .current()
            .into_iter()
            .chain(show.playlist.peek(AHEAD))
            .filter_map(|i| show.listing.get(i).cloned())
            .collect();
        let ids: Vec<ImageId> = paths.iter().map(|p| self.id_for_path(p)).collect();
        let cur = ids.first().copied();
        if let Some(p) = self.presenter.as_mut() {
            p.set_target(cur, fade);
        }
        self.present_wants(ids);
        let state = self.slide_of(cur);
        let Some(show) = self.presenter.as_mut().and_then(Presenter::show_mut) else {
            return;
        };
        let counting = !show.paused && matches!(state, Slide::Ready | Slide::Failed);
        show.counting = counting;
        show.timer.tick(now, counting);
        let elapsed = show.timer.at(now);
        // A slide that failed is passed over at once. Animations show as stills here.
        if state != Slide::Failed && !due(elapsed, interval, false, true) {
            show.waiting = None;
            return;
        }
        let waiting_since = *show.waiting.get_or_insert(now);
        let n = show.listing.len();
        let ahead: Vec<PathBuf> = show
            .playlist
            .peek(n.min(64))
            .into_iter()
            .filter_map(|i| show.listing.get(i).cloned())
            .collect();
        let mut steps = None;
        for (k, path) in ahead.iter().enumerate() {
            let slide = self.slide_of(self.by_path.get(path).copied());
            match slide {
                Slide::Failed => continue,
                Slide::Ready => steps = Some(k + 1),
                Slide::Partial if now - waiting_since >= PATIENCE_S => steps = Some(k + 1),
                _ => {}
            }
            break;
        }
        match steps {
            Some(k) => self.present_step(true, k),
            // The end of the show: every image has been on.
            None if ahead.len() < n.min(64)
                && ahead
                    .iter()
                    .all(|p| self.slide_of(self.by_path.get(p).copied()) == Slide::Failed) =>
            {
                self.present_close(Close::Stopped);
            }
            None => {}
        }
    }

    /// Moves the show `steps` forward (or one back); the timer starts over.
    fn present_step(&mut self, forward: bool, steps: usize) {
        let Some(show) = self.presenter.as_mut().and_then(Presenter::show_mut) else {
            return;
        };
        if forward {
            for _ in 0..steps {
                if show.playlist.advance().is_none() {
                    break;
                }
            }
        } else {
            show.playlist.back();
        }
        show.timer.restart();
        show.waiting = None;
        if let Some(p) = self.presenter.as_mut() {
            p.ken_burns = None;
        }
        self.present_show_tick(self.now());
        if let Some(p) = self.presenter.as_mut() {
            p.redraw = true;
        }
    }

    /// Pause and resume (Space in the presentation window, or the main window's control).
    pub(super) fn present_pause(&mut self) {
        let Some(show) = self.presenter.as_mut().and_then(Presenter::show_mut) else {
            return;
        };
        show.paused = !show.paused;
        let msg = if show.paused {
            t!("slideshow-paused")
        } else {
            t!("slideshow-resumed")
        };
        if let Some(p) = self.presenter.as_mut() {
            p.redraw = true;
        }
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// The main window's Stop.
    pub(super) fn present_stop(&mut self) {
        self.present_close(Close::Stopped);
    }

    /// The GPU device was replaced: the presentation window's surface, compositor and UI
    /// renderer are made again on the new one (the presentation stops when that fails).
    /// The old surface is let go first: a window takes one surface at a time on some
    /// platforms (Vulkan's "native window in use").
    pub(super) fn present_gpu_reset(&mut self) {
        let Some(g) = self.gfx.as_ref() else {
            return;
        };
        let Some(p) = self.presenter.take() else {
            return;
        };
        let Presenter {
            window,
            surface,
            comp,
            ui,
            text,
            dl,
            view,
            monitor,
            mode,
            target,
            shown,
            wants,
            scene: _,
            scene_pixels: _,
            crossfade: _,
            fade,
            ken_burns,
            display_hash,
            peak,
            hdr_checked,
            moved_off,
            pointer,
            pointer_moved,
            cursor_hidden,
            redraw: _,
            wake_at,
        } = p;
        drop((ui, comp, surface));
        let surface = match WindowSurface::new(&g.gpu, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                specular_log::warn!(target: GPU, "presentation window surface: {e}");
                self.chrome.announce_status(match mode {
                    Mode::Show(_) => t!("slideshow-stopped"),
                    Mode::Follow => t!("viewer-on-display-closed"),
                });
                drop(window);
                self.schedule();
                self.redraw = true;
                return;
            }
        };
        let (w, h) = surface.size();
        let comp = Compositor::new(&g.gpu, surface.format(), surface.bits, w, h);
        comp.set_bits(
            &g.gpu,
            if self.settings.colour.dithering {
                surface.bits
            } else {
                16
            },
        );
        let ui = UiRenderer::new(&g.gpu, &comp);
        self.presenter = Some(Presenter {
            window,
            surface,
            comp,
            ui,
            text,
            dl,
            view,
            monitor,
            mode,
            target,
            shown,
            wants,
            scene: None,
            scene_pixels: None,
            crossfade: None,
            fade,
            ken_burns,
            display_hash,
            peak,
            hdr_checked,
            moved_off,
            pointer,
            pointer_moved,
            cursor_hidden,
            redraw: true,
            wake_at,
        });
    }

    /// Lists the displays once a second: the Settings list, and an unplugged presentation
    /// display (the window moves to another free display, or closes).
    fn poll_monitors(&mut self) {
        if self
            .monitors
            .checked
            .is_some_and(|t| t.elapsed() < MONITOR_POLL)
        {
            return;
        }
        self.monitors.checked = Some(Instant::now());
        let Some(g) = self.gfx.as_ref() else {
            return;
        };
        let raw = g.window.available_monitors().count();
        if raw > self.monitors.raw && !self.monitors.lost.is_empty() {
            // A display was plugged in: it may be one found lost before.
            self.monitors.lost.clear();
        }
        self.monitors.raw = raw;
        let (names, handles, main) = self.monitors_now();
        if names != self.monitors.names {
            debug!(target: SETTINGS, "displays: {names:?} (main window on {main:?})");
            self.chrome.set_monitors(names.clone());
            self.monitors.names = names.clone();
            self.redraw = true;
        }
        let Some(p) = self.presenter.as_mut() else {
            return;
        };
        // The display is gone from the list, or (the list can keep a removed display: winit on
        // Wayland) the compositor moved the window onto the main window's display, seen at two
        // polls in a row. An empty list says nothing (some platforms list no names).
        let on = p.window.current_monitor().and_then(|m| m.name());
        let moved_off = on.is_some() && on != Some(p.monitor.clone()) && on == main;
        p.moved_off = if moved_off { p.moved_off + 1 } else { 0 };
        let gone = !names.is_empty() && !names.contains(&p.monitor);
        if !gone && p.moved_off < 2 {
            return;
        }
        p.moved_off = 0;
        // Never back onto the display just lost (winit on Wayland can keep listing it).
        self.monitors.lost.push(p.monitor.clone());
        let others: Vec<usize> = (0..names.len())
            .filter(|&i| names[i] != p.monitor)
            .collect();
        let other_names: Vec<String> = others.iter().map(|&i| names[i].clone()).collect();
        match after_unplug(&other_names, main.as_deref()).map(|k| others[k]) {
            Some(i) => {
                info!(target: SETTINGS, "display {} unplugged: presenting on {}", p.monitor, names[i]);
                let old = std::mem::replace(&mut p.monitor, names[i].clone());
                p.window
                    .set_fullscreen(Some(Fullscreen::Borderless(Some(handles[i].clone()))));
                p.scene = None;
                p.redraw = true;
                let now = self.now();
                self.chrome.toast(
                    t!(
                        "toast-display-moved",
                        old = isolate_ltr(&old),
                        display = isolate_ltr(&names[i])
                    ),
                    None,
                    now,
                );
                self.redraw = true;
            }
            None => {
                info!(target: SETTINGS, "display {} unplugged: presentation closed", p.monitor);
                self.present_close(Close::Unplugged);
            }
        }
    }

    /// An event of the presentation window.
    pub(super) fn present_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                self.present_close(Close::Stopped);
            }
            WindowEvent::Resized(s) => {
                let Some(g) = self.gfx.as_ref() else { return };
                let Some(p) = self.presenter.as_mut() else {
                    return;
                };
                if s.width == 0 || s.height == 0 || (s.width, s.height) == p.surface.size() {
                    return;
                }
                p.surface.resize(&g.gpu, s.width, s.height);
                let (w, h) = p.surface.size();
                p.comp.resize(&g.gpu, w, h);
                p.ui.compositor_resized(&g.gpu, &p.comp);
                p.scene = None;
                p.redraw = true;
                debug!(target: GPU, "presentation window {w}×{h}");
                // Sharp levels for the new size.
                if let Some(id) = p.target {
                    self.request_levels(id, Priority::Immediate);
                }
                self.schedule();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(p) = self.presenter.as_mut() {
                    p.scene = None;
                    p.redraw = true;
                }
            }
            WindowEvent::RedrawRequested => {
                self.drain_events();
                self.present_frame();
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::Focused(f) => {
                debug!(target: INPUT, "presentation window focused: {f}");
                if f {
                    self.mods = winit::keyboard::ModifiersState::empty();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.present_key(&event, el),
            WindowEvent::CursorMoved { position, .. } => {
                let now = self.now();
                let Some(p) = self.presenter.as_mut() else {
                    return;
                };
                let q = (position.x as f32, position.y as f32);
                if p.pointer
                    .is_none_or(|o| (o.0 - q.0).abs() + (o.1 - q.1).abs() > 2.0)
                {
                    p.pointer_moved = now;
                    if p.cursor_hidden {
                        p.window.set_cursor_visible(true);
                        p.cursor_hidden = false;
                    }
                }
                p.pointer = Some(q);
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(p) = self.presenter.as_mut() {
                    p.pointer = None;
                }
            }
            _ => {}
        }
    }

    /// Keys in the presentation window: Space pauses, ← and → step (in the viewer on another
    /// display: move through the main window's folder), Esc stops.
    fn present_key(&mut self, ev: &KeyEvent, _el: &ActiveEventLoop) {
        if ev.state != ElementState::Pressed {
            return;
        }
        let Some(combo) = self.key_combo(ev) else {
            return;
        };
        debug!(target: INPUT, "presentation window: key {combo}");
        let show = self.present_show_running();
        if combo.mods == Mods::NONE {
            match combo.key {
                Key::Named(NamedKey::Space) => {
                    if show && !ev.repeat {
                        self.present_pause();
                    }
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    if !ev.repeat {
                        self.present_close(Close::Stopped);
                    }
                    return;
                }
                _ => {}
            }
        }
        match self.keymap.lookup(&combo) {
            Some(a @ (Action::NextImage | Action::PrevImage)) => {
                let forward = a == Action::NextImage;
                if show {
                    if !ev.repeat {
                        self.present_step(forward, 1);
                    }
                } else {
                    self.navigate(if forward { 1 } else { -1 });
                    self.present_tick();
                }
                self.redraw = true;
            }
            Some(Action::Back | Action::Slideshow | Action::ViewerOnOtherMonitor) if !ev.repeat => {
                self.present_close(Close::Stopped);
            }
            _ => {}
        }
    }

    /// Draws the presentation window.
    pub(super) fn present_frame(&mut self) {
        let now = self.now();
        let s = &self.settings;
        let interval = s.slideshow.interval.clamp(1, 60) as f64;
        let ken_burns = self.ken_burns();
        let pal = self.palette();
        let canvas = settings_color(s.viewing.fullscreen_background);
        let backdrop = self.transparency(&pal, Some(canvas));
        let want_hdr = self.args.screenshot.is_none()
            && match s.colour.hdr_output {
                HdrOutput::Off => false,
                HdrOutput::On => true,
                HdrOutput::Auto => true,
            };
        let hdr_auto = s.colour.hdr_output == HdrOutput::Auto;
        let caption = s.slideshow.caption;
        let ui_scale = self.ui_scale;
        let ts = self.chrome.text_scale;
        let rtl = self.rtl();
        let playing = self
            .player_render_picture()
            .zip(self.anim.as_ref().map(|a| a.id))
            .map(|(p, id)| (id, p));
        let Some(p) = self.presenter.as_mut() else {
            return;
        };
        let Some(g) = self.gfx.as_mut() else { return };
        // HDR per surface: this display's own headroom.
        if p.hdr_checked.is_none_or(|t| t.elapsed() >= HDR_POLL) {
            p.hdr_checked = Some(Instant::now());
            let d = p.surface.hdr_display(&g.gpu);
            let want = want_hdr && (!hdr_auto || d.capable);
            let was = p.surface.hdr;
            let hdr = p.surface.set_hdr(&g.gpu, want);
            let peak = if hdr {
                d.headroom.unwrap_or(HDR_FALLBACK_HEADROOM).clamp(1.0, 64.0)
            } else {
                1.0
            };
            if hdr != was || (peak - p.peak).abs() > 0.01 {
                let format = p.surface.format();
                p.comp.set_output(&g.gpu, format, hdr.then_some(peak));
                p.peak = peak;
                p.scene = None;
                info!(target: GPU, "presentation HDR output {}: {format:?}, peak {peak:.2}× SDR white", if hdr { "on" } else { "off" });
            }
        }
        // The display transform (shared with the main window: one storage space).
        if let Some(d) = &self.colour.display
            && d.hash != p.display_hash
        {
            p.comp.set_display(&g.gpu, d);
            p.display_hash = d.hash;
            p.scene = None;
        }
        let size = p.surface.size();
        if size.0 < 2 || size.1 < 2 {
            return;
        }
        let scale = p.window.scale_factor() as f32 * ui_scale;
        p.view.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f32,
            h: size.1 as f32,
        });
        // The view, set up for the image once its size is known.
        let img = p
            .target
            .and_then(|id| self.images.get(&id).map(|i| (id, i)));
        if let Some((id, i)) = img
            && p.shown != Some(id)
            && let Some(info) = &i.info
        {
            p.view.upscale_small = true;
            p.view.set_image(
                info.width,
                info.height,
                i.orient.unwrap_or(info.orientation),
            );
            p.view.zoom_fit();
            p.view.settle();
            p.shown = Some(id);
            p.ken_burns = None;
        }
        let ready = img.is_some_and(|(id, i)| {
            p.shown == Some(id) && i.gpu_rank.is_some() && self.gpu_images.contains(id)
        });
        let lines = match (&p.mode, img) {
            (Mode::Show(_), Some((_, i))) => {
                caption_lines(&i.name(), i.metadata.as_deref(), caption)
            }
            _ => Vec::new(),
        };
        let (paused, counting, elapsed, seed) = match &p.mode {
            Mode::Show(sh) => (sh.paused, sh.counting, sh.timer.at(now), sh.seed),
            Mode::Follow => (false, false, 0.0, 0),
        };
        // Ken Burns on a show's slide while it is on.
        let mut drifting = false;
        if ken_burns
            && ready
            && p.show().is_some()
            && let Some(id) = p.target
        {
            let t = (elapsed / interval).clamp(0.0, 1.0) as f32;
            let seed = seed ^ id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            KenBurns::drive(&mut p.ken_burns, id, &mut p.view, t, seed);
            drifting = counting && !KenBurns::suspended(p.ken_burns, Some(id));
        } else if !ken_burns && p.ken_burns.take().is_some() {
            p.view.zoom_fit();
            p.view.settle();
        }

        // Overlay: the caption and the pause badge.
        p.dl.clear(scale);
        p.text.begin_frame(scale);
        if p.show().is_some() {
            let area = (0.0, 0.0, size.0 as f32 / scale, size.1 as f32 / scale);
            draw_overlay(&mut p.dl, &mut p.text, &lines, paused, area, rtl, ts);
        }

        let picture = picture_for(p.target, playing);
        let mut opts = RenderOptions {
            hq: true,
            canvas: canvas.to_linear_premul(),
            picture,
            ..RenderOptions::default()
        };
        let lin = opts.canvas;
        (opts.checker_a, opts.checker_b, opts.checker_cell) = match backdrop {
            Backdrop::Checker { a, b, cell } => (
                a.to_linear_premul(),
                b.to_linear_premul(),
                (cell * scale).round().max(1.0),
            ),
            Backdrop::Solid(c) => (c.to_linear_premul(), c.to_linear_premul(), 1.0e6),
            Backdrop::None => (lin, lin, 1.0e6),
        };
        let gimg = p
            .target
            .filter(|_| ready)
            .and_then(|id| self.gpu_images.get(id));
        let key = SceneKey {
            id: gimg.as_ref().map(|g| g.id),
            image_rev: gimg.as_ref().map_or(0, |g| g.revision),
            view_rev: p.view.revision,
            size,
            canvas: lin.map(f32::to_bits),
            picture: picture.map(|a| {
                [a.brightness, a.contrast, a.saturation, a.gamma, a.hue].map(f32::to_bits)
            }),
        };
        let shown_pixels = gimg.as_ref().is_some_and(|g| g.overview.is_some());
        // Never a blank frame: the last image (or page) stays until the next has pixels.
        let kept = p.scene_pixels == Some((size, key.canvas));
        let stand_in = keeps_picture(kept, shown_pixels, p.target, img.map(|(_, i)| i));
        let frame = match p.surface.acquire(&g.gpu) {
            Frame::Ready(f) => f,
            Frame::Skip => {
                p.redraw = true;
                return;
            }
        };
        let target = frame.texture.create_view(&Default::default());
        let mut enc = g
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("presentation"),
            });
        if p.scene != Some(key) && !stand_in {
            // The outgoing slide is kept for the cross-fade.
            if let Some(f) = &mut p.fade
                && f.start.is_none()
                && key.id == Some(f.to)
            {
                let from = p.scene.and_then(|s| s.id);
                if from.is_some() && from != key.id && shown_pixels && kept {
                    p.crossfade
                        .get_or_insert_with(|| Crossfade::new(&g.gpu))
                        .snapshot(&g.gpu, &mut enc, &p.comp);
                    f.start = Some(now);
                } else {
                    p.fade = None;
                }
            }
            // The renderer is shared: this display's peak for this draw only.
            let main_peak = g.renderer.display_peak();
            g.renderer.set_display_peak(p.peak);
            g.renderer.render(
                &g.gpu,
                &mut enc,
                &p.comp.scene.view,
                size,
                gimg.as_deref(),
                &p.view,
                &opts,
            );
            g.renderer.set_display_peak(main_peak);
            p.scene = Some(key);
            p.scene_pixels = shown_pixels.then_some((size, key.canvas));
        }
        p.comp.begin(&mut enc);
        let mut fading = false;
        if let Some(Fade { start: Some(s), .. }) = p.fade {
            let t = (now - s) / FADE_S;
            if t >= 1.0 {
                p.fade = None;
                if let Some(c) = &mut p.crossfade {
                    c.release();
                }
            } else if let Some(c) = &p.crossfade {
                c.draw(&mut enc, &p.comp, fade_weight(t));
                fading = true;
            }
        }
        p.ui.render(
            &g.gpu,
            &mut enc,
            &p.comp.comp.view,
            size,
            &p.dl,
            &mut p.text,
        );
        p.comp.finish(&mut enc, &target);
        g.gpu.queue.submit([enc.finish()]);
        p.window.pre_present_notify();
        g.gpu.queue.present(frame);
        p.text.end_frame();

        // When to draw next.
        let mut wake = f64::INFINITY;
        if fading || drifting || p.text.needs_redraw {
            wake = 0.0;
        }
        if p.text.fonts_pending() {
            wake = wake.min(0.05);
        }
        if let Mode::Show(sh) = &p.mode {
            if sh.counting {
                wake = wake.min((interval - elapsed).max(0.0));
            }
            if sh.waiting.is_some() {
                wake = wake.min(0.25);
            }
        }
        let idle = now - p.pointer_moved;
        if idle < POINTER_IDLE_S {
            wake = wake.min(POINTER_IDLE_S - idle);
        }
        p.wake_at = None;
        if wake <= 0.0 {
            p.redraw = true;
        } else if wake.is_finite() {
            p.wake_at = Some(Instant::now() + Duration::from_secs_f64(wake));
        }
    }

    /// Wakes the event loop for the presentation window's next frame or tick.
    pub(super) fn present_control_flow(&mut self, el: &ActiveEventLoop) {
        let Some(p) = self.presenter.as_mut() else {
            return;
        };
        if std::mem::take(&mut p.redraw) {
            p.window.request_redraw();
        }
        let mut t = p.wake_at;
        // The show ticks (a slide waited for) even when nothing is drawn.
        if p.show().is_some_and(|s| s.waiting.is_some() || s.counting) {
            let tick = Instant::now() + Duration::from_millis(250);
            t = Some(t.map_or(tick, |w| w.min(tick)));
        }
        // Unplugged displays are noticed.
        let poll = Instant::now() + MONITOR_POLL;
        t = Some(t.map_or(poll, |w| w.min(poll)));
        let Some(t) = t else { return };
        match el.control_flow() {
            ControlFlow::Wait => el.set_control_flow(ControlFlow::WaitUntil(t)),
            ControlFlow::WaitUntil(w) if w > t => el.set_control_flow(ControlFlow::WaitUntil(t)),
            _ => {}
        }
    }

    /// Drops the presentation window (the app is exiting).
    pub(super) fn present_drop(&mut self) {
        self.presenter = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The playing movie's picture adjustments reach the presentation when it shows that
    /// movie, and only then.
    #[test]
    fn the_presentation_shows_the_movies_picture_adjustments() {
        let warm = specular_render::PictureAdjust {
            brightness: 0.1,
            hue: 12.0,
            ..specular_render::PictureAdjust::NEUTRAL
        };
        assert_eq!(picture_for(Some(3), Some((3, warm))), Some(warm));
        assert_eq!(picture_for(Some(4), Some((3, warm))), None);
        assert_eq!(picture_for(None, Some((3, warm))), None);
        assert_eq!(picture_for(Some(3), None), None);
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn auto_picks_another_display_than_the_main_windows() {
        let m = names(&["eDP-1", "HDMI-A-1", "DP-2"]);
        assert_eq!(
            pick_monitor(&MonitorChoice::Auto, &m, Some("eDP-1")),
            Some(1)
        );
        assert_eq!(
            pick_monitor(&MonitorChoice::Auto, &m, Some("HDMI-A-1")),
            Some(0)
        );
    }

    #[test]
    fn one_display_or_an_unknown_main_display_keeps_the_show_in_the_main_window() {
        let one = names(&["eDP-1"]);
        assert_eq!(
            pick_monitor(&MonitorChoice::Auto, &one, Some("eDP-1")),
            None
        );
        let named = MonitorChoice::Named("eDP-1".into());
        assert_eq!(pick_monitor(&named, &one, Some("eDP-1")), None);
        assert_eq!(pick_monitor(&MonitorChoice::Auto, &[], None), None);
        let two = names(&["eDP-1", "HDMI-A-1"]);
        assert_eq!(pick_monitor(&MonitorChoice::Auto, &two, None), None);
    }

    #[test]
    fn a_named_display_is_used_when_connected_and_free() {
        let m = names(&["eDP-1", "HDMI-A-1", "DP-2"]);
        let dp = MonitorChoice::Named("DP-2".into());
        assert_eq!(pick_monitor(&dp, &m, Some("eDP-1")), Some(2));
        // Not connected: the first other display.
        let gone = MonitorChoice::Named("DP-9".into());
        assert_eq!(pick_monitor(&gone, &m, Some("eDP-1")), Some(1));
        // The main window's own display: another one.
        assert_eq!(pick_monitor(&dp, &m, Some("DP-2")), Some(0));
    }

    /// A page switched to (or a slide still loading) keeps the old picture until it has
    /// pixels, past the main window's stand-in time; an error, a picture drawn at another
    /// size, or nothing to show ends it.
    #[test]
    fn the_old_page_stays_until_the_new_one_has_pixels() {
        let mut page = Img::new("scan.tif".into());
        page.page = 1;
        page.pages = 3;
        page.page_hold = true;
        page.shown_at = Some(Instant::now() - Duration::from_secs(5));
        assert!(stands_in(&page, false));
        assert!(keeps_picture(true, false, Some(2), Some(&page)));
        // The main window cleared its hold once it drew the page: this window still waits.
        page.page_hold = false;
        assert!(!stands_in(&page, false));
        assert!(keeps_picture(true, false, Some(2), Some(&page)));
        // The entry is gone (a page that never opened, replaced): still the old picture.
        assert!(keeps_picture(true, false, Some(2), None));
        // The new page has pixels; nothing kept (or kept at another size); nothing to show.
        assert!(!keeps_picture(true, true, Some(2), Some(&page)));
        assert!(!keeps_picture(false, false, Some(2), Some(&page)));
        assert!(!keeps_picture(true, false, None, None));
        page.error = Some("bad page".into());
        assert!(!keeps_picture(true, false, Some(2), Some(&page)));
    }

    #[test]
    fn an_unplugged_display_moves_the_presentation_or_closes_it() {
        let m = names(&["eDP-1", "DP-2"]);
        assert_eq!(after_unplug(&m, Some("eDP-1")), Some(1));
        assert_eq!(after_unplug(&names(&["eDP-1"]), Some("eDP-1")), None);
    }
}

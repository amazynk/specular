//! The start screen's side in the application: the recent folders (a small list in the config
//! folder, their cards' thumbnails made off the event loop), what its controls ask for (pick
//! a folder, the Library and the offer to add the Pictures folder to it), "Show welcome", the
//! startup choice (`general.on_start`) and the one-time hints.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};
use specular_i18n::t;
use specular_input::{Action, ImageState, Profile};
use specular_settings::{OnStart, Origin};
use specular_thumbs::generate::GenOptions;
use specular_widgets::{RecentFolder, Welcome, WelcomeCommand};

use crate::thumbnail::default_cache_dir;

use super::App;
use super::studies::upload_rgba;

/// Undo key of a hint's "Turn off tips" (after the profile offers' keys).
pub(super) const HINT_KEY: u64 = 1 << 59;
/// The confirmation "Add your Pictures folder to the library?".
pub(super) const PICTURES_ITEM: u64 = super::items::WELCOME_PICTURES;
/// The recent folders, one path a line, most recent first, in the config folder.
const RECENTS_FILE: &str = "recent-folders.txt";
/// Recent folders kept (the start screen shows as many as fit).
const MAX_RECENTS: usize = 8;
/// Long side of a recent folder's thumbnail.
const THUMB: u32 = 256;
/// Files looked at for a folder's cover image.
const COVER_SCAN: usize = 4000;

/// A message from a worker.
enum Msg {
    /// A recent folder's cover (`None`: no image in it, or it is gone).
    Cover(PathBuf, Option<(u32, u32, Vec<u8>)>),
    /// The folder chooser's answer.
    Picked(Option<PathBuf>),
}

struct Shown {
    _tex: wgpu::Texture,
    id: TexId,
    w: u32,
    h: u32,
}

#[derive(Default)]
pub(super) struct WelcomeState {
    /// Something to show was given at the start (files, a URL, `--library`, `--slideshow`):
    /// the startup choice does not apply.
    pub given: bool,
    recents: Vec<PathBuf>,
    loaded: bool,
    /// The folder last recorded.
    noted: Option<PathBuf>,
    covers: Vec<(PathBuf, Shown)>,
    /// Covers asked for (made or on the way).
    asked: HashSet<PathBuf>,
    /// Covers made, to upload at the next frame.
    made: Vec<(PathBuf, u32, u32, Vec<u8>)>,
    retired: Vec<TexId>,
    channel: Option<(Sender<Msg>, Receiver<Msg>)>,
    /// The chrome's copy is out of date.
    dirty: bool,
    /// "Your pictures" entered the Library: once it is read, an empty one offers the
    /// Pictures folder.
    pictures_pending: bool,
    /// The name of the Pictures folder while the Library has none ("Set up your library").
    pictures_offer: Option<String>,
    /// A hint was shown this session (at most one).
    hinted: bool,
    /// Screenshots: the profile the start screen is drawn for.
    pub shot_profile: Option<Profile>,
}

impl WelcomeState {
    /// The GPU device was replaced: the covers are made and uploaded again.
    pub(super) fn gpu_reset(&mut self) {
        self.covers.clear();
        self.asked.clear();
        self.retired.clear();
        self.dirty = true;
    }

    fn sender(&mut self) -> Sender<Msg> {
        self.channel
            .get_or_insert_with(crossbeam_channel::unbounded)
            .0
            .clone()
    }

    /// Uploads the covers made since the last frame and releases retired ones. True when the
    /// start screen changed.
    pub(super) fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) -> bool {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        if self.made.is_empty() {
            return false;
        }
        for (path, w, h, rgba) in self.made.drain(..) {
            let (tex, id) = upload_rgba(gpu, ui, w, h, &rgba);
            self.covers.push((
                path,
                Shown {
                    _tex: tex,
                    id,
                    w,
                    h,
                },
            ));
        }
        self.dirty = true;
        true
    }

    /// No covers on the way (screenshots wait for them).
    pub(super) fn settled(&self) -> bool {
        self.made.is_empty()
            && self
                .recents
                .iter()
                .all(|p| !self.asked.contains(p) || self.covers.iter().any(|(c, _)| c == p))
    }
}

/// What the window shows at a start without anything given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StartView {
    /// Something was given (a file from the file manager or the command line): the viewer.
    Given,
    StartScreen,
    Gallery(PathBuf),
    Library,
}

/// The startup choice: `on_start` unless something was `given`; the gallery of the last
/// folder falls back to the start screen when there is none.
pub(super) fn start_view(on_start: OnStart, given: bool, last: Option<&Path>) -> StartView {
    if given {
        return StartView::Given;
    }
    match (on_start, last) {
        (OnStart::Gallery, Some(p)) => StartView::Gallery(p.to_path_buf()),
        (OnStart::Library, _) => StartView::Library,
        _ => StartView::StartScreen,
    }
}

/// `list` with `dir` first (once), at most [`MAX_RECENTS`] long.
fn noted(list: &[PathBuf], dir: &Path) -> Vec<PathBuf> {
    std::iter::once(dir.to_path_buf())
        .chain(list.iter().filter(|p| *p != dir).cloned())
        .take(MAX_RECENTS)
        .collect()
}

fn parse_recents(text: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for l in text.lines().map(str::trim_end).filter(|l| !l.is_empty()) {
        let p = PathBuf::from(l);
        if p.is_absolute() && !out.contains(&p) {
            out.push(p);
        }
    }
    out.truncate(MAX_RECENTS);
    out
}

/// A one-time hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hint {
    /// The first image from a folder with others: the gallery's key.
    Gallery,
    /// The first time in the gallery: compare and cull.
    GalleryFirst,
    /// The first folder of raw photos: cull.
    Raw,
    /// The first file with several pages: the page keys.
    Pages,
}

impl Hint {
    /// Its id in `general.tips_shown`.
    pub(super) fn id(self) -> &'static str {
        match self {
            Hint::Gallery => "gallery",
            Hint::GalleryFirst => "gallery-first",
            Hint::Raw => "raw",
            Hint::Pages => "pages",
        }
    }
}

/// What the hints are decided on.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct HintFacts<'a> {
    /// `general.tips`.
    pub tips: bool,
    /// `general.tips_shown`: the ids shown before, comma separated.
    pub shown: &'a str,
    /// A hint was shown this session.
    pub this_session: bool,
    /// A slideshow, a presentation, fullscreen, a dialog, or a run without a user (tests,
    /// screenshots, benchmarks).
    pub quiet: bool,
    /// The gallery shows a folder.
    pub in_gallery: bool,
    /// An image is on screen.
    pub image: bool,
    /// Images in its folder.
    pub images: usize,
    /// It is a raw photo.
    pub raw: bool,
    /// Its pages.
    pub pages: u32,
}

/// The hint to show now, if any: never twice, at most one a session, never while it would
/// get in the way; `keys` says whether a hint's keys are bound (one without is not shown).
pub(super) fn next_hint(f: &HintFacts, keys: impl Fn(Hint) -> bool) -> Option<Hint> {
    if !f.tips || f.this_session || f.quiet {
        return None;
    }
    let mut candidates = Vec::new();
    if f.in_gallery {
        candidates.push(Hint::GalleryFirst);
    } else if f.image {
        if f.pages > 1 {
            candidates.push(Hint::Pages);
        }
        if f.raw && f.images > 1 {
            candidates.push(Hint::Raw);
        }
        if f.images > 1 {
            candidates.push(Hint::Gallery);
        }
    }
    let shown: Vec<&str> = f.shown.split(',').map(str::trim).collect();
    candidates
        .into_iter()
        .find(|h| !shown.contains(&h.id()) && keys(*h))
}

/// `shown` with `id` added.
fn record(shown: &str, id: &str) -> String {
    if shown.split(',').any(|s| s.trim() == id) {
        shown.to_owned()
    } else if shown.is_empty() {
        id.to_owned()
    } else {
        format!("{shown},{id}")
    }
}

/// The first image in `dir` (by name, among the first [`COVER_SCAN`] files).
fn cover_of(dir: &Path, supported: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .take(COVER_SCAN)
        .map(|e| e.path())
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                && p.is_file()
                && supported(p)
        })
        .min()
}

/// A folder's place as people know it: `~/Pictures` rather than `/home/me/Pictures`. Windows
/// has no `~`: there the path is shown as File Explorer's address bar shows it.
fn place(dir: Option<&Path>) -> String {
    let Some(dir) = dir else {
        return String::new();
    };
    if !cfg!(windows)
        && let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && let Ok(rest) = dir.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".to_owned()
        } else {
            format!("~/{}", rest.display())
        };
    }
    dir.display().to_string()
}

fn folder_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

impl App {
    fn recents_path(&self) -> Option<PathBuf> {
        if self.headless() {
            return None;
        }
        Some(self.store.path()?.parent()?.join(RECENTS_FILE))
    }

    /// Reads the recent folders, once.
    fn welcome_load(&mut self) {
        if self.welcome.loaded {
            return;
        }
        self.welcome.loaded = true;
        if let Some(p) = self.recents_path()
            && let Ok(text) = std::fs::read_to_string(p)
        {
            self.welcome.recents = parse_recents(&text);
        }
        self.welcome.pictures_offer = self.pictures_offer();
        self.welcome.dirty = true;
    }

    /// The name of the Pictures folder while the Library has never been set up.
    fn pictures_offer(&self) -> Option<String> {
        if self.headless() {
            return None;
        }
        if let Some(has) = self.library_has_folders() {
            return (!has)
                .then(|| folder_name(&specular_platform::integration::user_pictures_dir()));
        }
        let data = specular_settings::Locations::detect().data_dir;
        let catalogue = specular_library::default_path(&data);
        (!catalogue.exists())
            .then(|| folder_name(&specular_platform::integration::user_pictures_dir()))
    }

    /// Records the folder on screen as the most recent.
    fn welcome_note(&mut self) {
        let dir = match self.gallery_folder() {
            Some(d) => Some(d.to_path_buf()),
            None if !self.gallery.active && self.current.is_some() => self.folder.clone(),
            None => None,
        };
        let Some(dir) = dir else { return };
        if self.welcome.noted.as_ref() == Some(&dir) {
            return;
        }
        self.welcome.noted = Some(dir.clone());
        if self.welcome.recents.first() == Some(&dir) {
            return;
        }
        self.welcome.recents = noted(&self.welcome.recents, &dir);
        self.welcome.dirty = true;
        if let Some(path) = self.recents_path() {
            let mut text = String::new();
            for p in &self.welcome.recents {
                let s = p.to_string_lossy();
                if !s.contains('\n') {
                    text.push_str(&s);
                    text.push('\n');
                }
            }
            self.loader.background("recent-folders", move |_| {
                if let Err(e) = specular_settings::write_atomic(&path, &text) {
                    specular_log::warn!(target: specular_log::SETTINGS, "recent folders: {e}");
                }
            });
        }
    }

    /// Once per event-loop turn.
    pub(super) fn welcome_poll(&mut self) {
        self.welcome_load();
        self.welcome_note();
        let msgs: Vec<Msg> = match &self.welcome.channel {
            Some((_, rx)) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        for m in msgs {
            match m {
                Msg::Cover(p, Some((w, h, rgba))) if w > 0 && h > 0 => {
                    self.welcome.made.push((p, w, h, rgba));
                    self.redraw = true;
                }
                Msg::Cover(p, _) => {
                    // A folder that is gone leaves the list; one without images keeps its
                    // folder icon.
                    if !p.is_dir() {
                        self.welcome.recents.retain(|r| *r != p);
                        self.welcome.dirty = true;
                    }
                }
                Msg::Picked(Some(dir)) => self.gallery_enter_at(Some(dir), None),
                Msg::Picked(None) => {}
            }
        }
        if self.welcome.pictures_pending {
            match self.library_has_folders() {
                Some(true) => self.welcome.pictures_pending = false,
                Some(false) => {
                    self.welcome.pictures_pending = false;
                    self.pictures_ask();
                }
                None if self.library_failed() => self.welcome.pictures_pending = false,
                None => {}
            }
        }
        if self.chrome.welcome_shown() {
            self.welcome_covers();
        }
        self.welcome_hint();
        if self.welcome.dirty {
            self.welcome.dirty = false;
            let offer = self.pictures_offer();
            self.welcome.pictures_offer = offer;
            self.welcome_sync();
        }
    }

    /// Hands the start screen its recent folders.
    fn welcome_sync(&mut self) {
        let w = &self.welcome;
        let recents = w
            .recents
            .iter()
            .map(|p| RecentFolder {
                path: p.clone(),
                name: folder_name(p),
                parent: place(p.parent()),
                thumb: w
                    .covers
                    .iter()
                    .find(|(c, _)| c == p)
                    .map(|(_, s)| (s.id, s.w, s.h)),
            })
            .collect();
        self.chrome.set_welcome(Welcome {
            recents,
            pictures_offer: w.pictures_offer.clone(),
        });
        self.redraw = true;
    }

    /// Makes the covers of the recent folders not asked for yet, on a worker.
    fn welcome_covers(&mut self) {
        let todo: Vec<PathBuf> = self
            .welcome
            .recents
            .iter()
            .filter(|p| !self.welcome.asked.contains(*p))
            .cloned()
            .collect();
        if todo.is_empty() {
            return;
        }
        self.welcome.asked.extend(todo.iter().cloned());
        let tx = self.welcome.sender();
        let wake = self.loader.waker();
        let registry = self.loader.registry();
        let g = &self.settings.gallery;
        // Screenshots never read the user's cache.
        let cache = (!self.headless())
            .then(|| g.cache_location.clone().or_else(default_cache_dir))
            .flatten()
            .filter(|d| d.is_dir());
        let opts = crate::thumbnail::Options {
            cache,
            generate: GenOptions {
                prefer_embedded: g.prefer_embedded,
                parallel: false,
                ..GenOptions::default()
            },
        };
        self.loader.background("welcome-covers", move |_| {
            for dir in todo {
                let cover = cover_of(&dir, |p| registry.supports_file(p))
                    .and_then(|f| crate::thumbnail::rgba(&registry, &f, THUMB, &opts).ok())
                    .map(|(_, px, _)| px);
                let _ = tx.send(Msg::Cover(dir, cover));
                wake();
            }
        });
    }

    /// A control of the start screen.
    pub(super) fn welcome_command(&mut self, c: WelcomeCommand) {
        match c {
            WelcomeCommand::Browse => {
                let tx = self.welcome.sender();
                let wake = self.loader.waker();
                let title = t!("welcome-browse");
                self.loader.background("dlg-folder", move |_| {
                    let dir = rfd::FileDialog::new().set_title(&title).pick_folder();
                    let _ = tx.send(Msg::Picked(dir));
                    wake();
                });
            }
            WelcomeCommand::Pictures => match self.library_has_folders() {
                Some(true) => self.library_from_desktop(),
                Some(false) => self.pictures_ask(),
                // Never set up: ask before anything is added.
                None if self.welcome.pictures_offer.is_some() => self.pictures_ask(),
                None => {
                    self.library_from_desktop();
                    self.welcome.pictures_pending = true;
                }
            },
            WelcomeCommand::Folder(p) => self.gallery_enter_at(Some(p), None),
        }
        self.redraw = true;
    }

    /// "Add your Pictures folder to the library?"
    fn pictures_ask(&mut self) {
        let name = folder_name(&specular_platform::integration::user_pictures_dir());
        self.chrome.confirm(
            t!("welcome-pictures-ask", folder = name),
            t!("welcome-pictures-add"),
            PICTURES_ITEM,
        );
        self.redraw = true;
    }

    /// The confirmation's accept. True when `id` was ours.
    pub(super) fn welcome_item(&mut self, id: u64) -> bool {
        if id != PICTURES_ITEM {
            return false;
        }
        self.library_enter_adding(specular_platform::integration::user_pictures_dir());
        self.welcome.pictures_offer = None;
        self.welcome.dirty = true;
        true
    }

    /// "Show welcome": the tips come back, and the window shows the start screen.
    pub(super) fn welcome_show(&mut self) {
        if !self.settings.general.start_tips {
            self.store
                .update(Origin::Program, |s| s.general.start_tips = true);
            self.apply_settings(false);
        }
        self.welcome_close_all();
    }

    /// Closes the gallery and the image: the start screen shows.
    pub(super) fn welcome_close_all(&mut self) {
        // Develop goes with the photo (its edit saved), not left over the start screen.
        self.develop_leave();
        self.gallery_dismiss();
        self.close_book();
        self.series_close();
        self.set_entries(Vec::new());
        self.current = None;
        self.shown = None;
        self.folder = None;
        self.welcome.noted = None;
        self.redraw = true;
    }

    /// The window's close button (and Alt+F4) and Close (Ctrl+W): with an image or the gallery
    /// open it goes back to the start screen, like closing a document; on the start screen it
    /// quits. Returns whether it went back (false: quit). Quit (Ctrl+Q) always quits.
    pub(super) fn welcome_close_requested(&mut self) -> bool {
        if self.headless() || (self.current.is_none() && !self.gallery_active()) {
            return false;
        }
        self.welcome_close_all();
        true
    }

    /// The startup choice, on the first turn of an interactive start.
    pub(super) fn welcome_on_start(&mut self, given: bool) {
        if self.headless() {
            return;
        }
        self.welcome_load();
        let given = given || self.welcome.given || !self.entries.is_empty();
        let last = self.welcome.recents.iter().find(|p| p.is_dir()).cloned();
        match start_view(self.settings.general.on_start, given, last.as_deref()) {
            StartView::Given | StartView::StartScreen => {}
            StartView::Gallery(dir) => self.gallery_enter_at(Some(dir), None),
            StartView::Library => self.library_from_desktop(),
        }
    }

    /// Shows a one-time hint when one is due.
    fn welcome_hint(&mut self) {
        let st = self.action_state();
        let cur = self.current.and_then(|i| self.entries.get(i));
        let headless =
            self.headless() && std::env::var_os("SPECULAR_HINTS").is_none_or(|v| v != "1");
        let shown = self.settings.general.tips_shown.clone();
        let facts = HintFacts {
            tips: self.settings.general.tips,
            shown: &shown,
            this_session: self.welcome.hinted,
            quiet: headless
                || self.fullscreen
                || self.slideshow.running()
                || self.presenter.is_some()
                || self.chrome.modal().is_some(),
            in_gallery: self.gallery_folder().is_some(),
            image: st.image == ImageState::Ready,
            images: st.count,
            raw: cur.is_some_and(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(crate::app::gallery::is_raw_ext)
            }),
            pages: st.pages,
        };
        let Some(h) = next_hint(&facts, |h| self.hint_text(h).is_some()) else {
            return;
        };
        let Some(text) = self.hint_text(h) else {
            return;
        };
        self.welcome.hinted = true;
        let now = self.now();
        self.chrome.hint(text, HINT_KEY, now);
        let id = h.id();
        self.store.update(Origin::Program, |s| {
            s.general.tips_shown = record(&s.general.tips_shown, id);
        });
        self.apply_settings(false);
        self.redraw = true;
    }

    /// A hint's text with the user's keys; `None` when a key it names is not bound.
    fn hint_text(&self, h: Hint) -> Option<String> {
        let key = |a: Action| self.keymap.primary_key(a).map(|k| k.to_string());
        Some(match h {
            Hint::Gallery => t!("hint-gallery", key = key(Action::ToggleGallery)?),
            Hint::GalleryFirst => t!(
                "hint-gallery-first",
                compare = specular_widgets::compare_key()?,
                cull = key(Action::ToggleCull)?
            ),
            Hint::Raw => t!("hint-raw", key = key(Action::ToggleCull)?),
            Hint::Pages => t!(
                "hint-pages",
                count = self.action_state().pages as i64,
                next = key(Action::NextPage)?,
                prev = key(Action::PrevPage)?
            ),
        })
    }

    /// A hint's "Turn off tips". True when `key` was a hint's.
    pub(super) fn hint_turn_off(&mut self, key: u64) -> bool {
        if key != HINT_KEY {
            return false;
        }
        self.chrome.dismiss_undo(key);
        if self.last_undo == Some(key) {
            self.last_undo = None;
        }
        self.store
            .update(Origin::Program, |s| s.general.tips = false);
        self.apply_settings(false);
        let now = self.now();
        self.chrome.toast(t!("hint-turned-off"), None, now);
        self.redraw = true;
        true
    }

    /// `--show` scenes of the start screen. True when `s` was one.
    pub(super) fn welcome_show_scene(&mut self, s: &str) -> bool {
        match s {
            // The start screen (after the image given, if any, is closed).
            "welcome" => {
                self.welcome_close_all();
                self.welcome.recents.clear();
                self.welcome.dirty = true;
            }
            // With the folder of the image given and the folders beside it as recent ones.
            "welcome-recents" => {
                let dir = self.folder.clone();
                self.welcome_close_all();
                if let Some(dir) = dir {
                    let mut list = vec![dir.clone()];
                    if let Some(parent) = dir.parent()
                        && let Ok(rd) = std::fs::read_dir(parent)
                    {
                        let mut sib: Vec<PathBuf> = rd
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| p.is_dir() && *p != dir)
                            .collect();
                        sib.sort();
                        list.extend(sib);
                    }
                    list.truncate(MAX_RECENTS);
                    self.welcome.recents = list;
                    self.welcome.dirty = true;
                }
            }
            // As someone with the Medical tools on sees it.
            "welcome-medical" => {
                use specular_input::{Capabilities, Capability};
                self.welcome.shot_profile =
                    Some(Profile::of(Capabilities::GENERAL.with(Capability::Medical)));
                self.welcome_close_all();
                self.welcome.recents.clear();
                self.welcome.dirty = true;
            }
            "first-run" => {
                self.chrome
                    .set_profile_start(self.settings.general.on_start);
                self.chrome
                    .ask_profile(specular_input::Capabilities::GENERAL);
            }
            "hint" => {
                let now = self.now();
                if let Some(text) = self.hint_text(Hint::Gallery) {
                    // Born a moment ago, so it is drawn settled.
                    self.chrome.hint(text, HINT_KEY, now - 1.0);
                }
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file from the file manager or the command line always shows in the viewer; else the
    /// choice applies, the gallery falling back to the start screen without a last folder.
    #[test]
    fn startup_choice_and_a_file_given() {
        let last = Path::new("/photos/2026");
        for o in [OnStart::StartScreen, OnStart::Gallery, OnStart::Library] {
            assert_eq!(start_view(o, true, Some(last)), StartView::Given);
        }
        assert_eq!(
            start_view(OnStart::StartScreen, false, Some(last)),
            StartView::StartScreen
        );
        assert_eq!(
            start_view(OnStart::Gallery, false, Some(last)),
            StartView::Gallery(last.to_path_buf())
        );
        assert_eq!(
            start_view(OnStart::Gallery, false, None),
            StartView::StartScreen
        );
        assert_eq!(
            start_view(OnStart::Library, false, None),
            StartView::Library
        );
    }

    fn viewer<'a>(shown: &'a str) -> HintFacts<'a> {
        HintFacts {
            tips: true,
            shown,
            image: true,
            images: 12,
            ..HintFacts::default()
        }
    }

    /// Each hint is shown once: recorded, it is not shown again; one a session; never when
    /// quiet or with tips off; a hint whose key is unbound is skipped.
    #[test]
    fn hints_once_only() {
        let all = |_| true;
        let mut shown = String::new();
        assert_eq!(next_hint(&viewer(&shown), all), Some(Hint::Gallery));
        shown = record(&shown, Hint::Gallery.id());
        assert_eq!(next_hint(&viewer(&shown), all), None);
        assert_eq!(record(&shown, "gallery"), "gallery");

        let mut f = viewer("");
        f.this_session = true;
        assert_eq!(next_hint(&f, all), None);
        let mut f = viewer("");
        f.quiet = true;
        assert_eq!(next_hint(&f, all), None);
        let mut f = viewer("");
        f.tips = false;
        assert_eq!(next_hint(&f, all), None);
        // A single image: nothing to say.
        let mut f = viewer("");
        f.images = 1;
        assert_eq!(next_hint(&f, all), None);

        // Raw folders and pages come first; the gallery has its own.
        let mut f = viewer("");
        f.raw = true;
        assert_eq!(next_hint(&f, all), Some(Hint::Raw));
        f.pages = 3;
        assert_eq!(next_hint(&f, all), Some(Hint::Pages));
        assert_eq!(next_hint(&f, |h| h != Hint::Pages), Some(Hint::Raw));
        let shown = record(&record("", "pages"), "raw");
        assert_eq!(shown, "pages,raw");
        f.shown = &shown;
        assert_eq!(next_hint(&f, all), Some(Hint::Gallery));
        let f = HintFacts {
            tips: true,
            in_gallery: true,
            ..HintFacts::default()
        };
        assert_eq!(next_hint(&f, all), Some(Hint::GalleryFirst));
    }

    #[test]
    fn recent_folders_list() {
        // Absolute on this platform (`/a` has no drive on Windows).
        let abs = |n: &str| {
            PathBuf::from(if cfg!(windows) {
                format!(r"C:\{n}")
            } else {
                format!("/{n}")
            })
        };
        let a = abs("a");
        let b = abs("b");
        assert_eq!(noted(&[a.clone(), b.clone()], &b), [b.clone(), a.clone()]);
        let many: Vec<PathBuf> = (0..20).map(|i| abs(&i.to_string())).collect();
        assert_eq!(noted(&many, &a).len(), MAX_RECENTS);
        let text = format!("{0}\nrelative\n{0}\n\n{1}\n", a.display(), b.display());
        assert_eq!(parse_recents(&text), [a, b]);
    }
}

//! Develop mode (DESIGN.md §6.10): non-destructive editing of
//! the photo shown, entered from the viewer ([`Action::ToggleDevelop`], Shift+D) and left
//! the same way, with Esc at fit, or with the panel's Close.
//!
//! - **Layout:** the image keeps the window; the panel of module cards sits on the right (the
//!   left in right-to-left languages) and the filmstrip of the folder along the bottom.
//! - **Edits:** each image gets an [`Edit`](specular_develop::model::Edit) and its
//!   [`History`] ([`controls::Session`]), saved a second after the last change and when the
//!   image is left, to the catalogue and the `.specular.xmp` sidecar ([`store`]), and read
//!   back on open. Ctrl+Z and Ctrl+Shift+Z undo and redo in the edit while Develop is open.
//! - **Before and after:** `\` shows the original, `|` the original and the edit split
//!   down the middle of the image.
//! - **Crop:** [`Action::DevelopCrop`] (R) opens the crop and straighten tool over the
//!   whole photo ([`crop`]); Enter keeps, Esc goes back.
//! - **Speed edit:** hold a speed-edit key (E exposure, C contrast…) and scroll or press
//!   arrows anywhere; a readout over the image shows the control and its value.
//! - **Pixels:** [`preview`] develops the image on the GPU (`specular_develop`) at the size
//!   it shows and draws it over the image.
//! - **History, styles and batch** ([`batch`]): snapshots (compared split with the edit),
//!   compress, styles with hover preview, module presets, auto-apply rules, versions,
//!   copy and paste, sync in cull mode, match to reference ([`matcher`]) and the next
//!   capture style; their sheets ([`sheet`]) show in the file-operations sheet.
//! - **Profiles:** hidden while the Medical or Science profile is on
//!   ([`specular_input::develop_off`]).
//! - **Masks:** each module card has a Masks section and an instance list ([`masks`]); the
//!   mask row being edited shows handles on the picture, a brush row paints there.
//! - **Darkroom aids** ([`aids`]): scopes docked at the top of the panel, clipping overlays
//!   and soft-proofing; printing uses the developed photo (and the proof profile).

#[cfg(test)]
mod acts;
#[cfg(test)]
mod ahead_tests;
mod ai_denoise;
#[cfg(test)]
mod ai_denoise_shots;
mod ai_masks;
mod ai_remove;
#[cfg(test)]
mod ai_tools_shots;
mod aids;
#[cfg(test)]
mod aids_shots;
mod batch;
#[cfg(test)]
mod batch_shots;
mod chart_profile;
#[cfg(test)]
mod chart_shots;
mod controls;
mod crop;
mod depth_tools;
mod diffuse;
#[cfg(test)]
mod diffuse_shots;
pub(super) mod edited;
mod effects;
#[cfg(test)]
mod effects_shots;
pub(super) mod export;
mod filmstrip;
mod fusion;
mod ghosts;
mod interop;
#[cfg(test)]
pub(in crate::app) mod interop_shots;
mod lensfun;
mod masks;
mod matcher;
pub(super) mod merge;
#[cfg(test)]
mod merge_shots;
pub(super) mod merges;
mod pano;
mod pano_show;
#[cfg(test)]
mod persp_shots;
mod perspective;
mod prereq;
mod preview;
mod print_source;
mod retouch;
mod sheet;
#[cfg(test)]
mod shots;
mod stack;
pub(in crate::app) mod store;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tone_batch_shots;
mod zones;

pub(super) use self::print_source::{DevelopSource, developed_size, proof_filter, saved_edit};
pub(super) use self::sheet::DevelopSheet;

/// Readies an export of developed photos made outside Develop (client galleries): AI
/// denoise's runner, as Develop's own export does.
pub(super) fn ready_export(gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>) {
    let _ = ai_denoise::install_runner(gpu);
}

use std::collections::HashMap;
use std::path::PathBuf;

use specular_develop::model::{Edit, History};
use specular_gfx::{Align, DrawList, Gpu, Rect, Text, TextStyle, UiRenderer};
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo, NamedKey};
use specular_theme::{Palette, metrics};
use specular_widgets::accesskit;
use specular_widgets::develop_panel::{self, DevelopPanel, Event};
use specular_widgets::scopes::Press;

use self::controls::{Request, Session, id};
use self::preview::{Orient, Preview, upload as preview_upload};
use super::App;
use crate::loader::Loader;

/// Screen-reader ids of the panel and of the filmstrip.
const A11Y_PANEL: u64 = 0x6C00_0000_0000_0000;
const A11Y_STRIP: u64 = 0x6D00_0000_0000_0000;
const A11Y_HANDLES: u64 = 0x6E00_0000_0000_0000;
const A11Y_DOCK: u64 = 0x6F00_0000_0000_0000;
/// The selected retouch spot's handles.
const A11Y_RT_HANDLES: u64 = 0x6F40_0000_0000_0000;
/// The labels over the picture (before/after, "Developing…", "Refining…").
const A11Y_LABELS: u64 = 0x6F80_0000_0000_0000;
/// How long the speed-edit readout stays after the last change (s).
const HUD_S: f64 = 1.2;

/// Develop's state.
#[derive(Default)]
pub(super) struct Develop {
    session: Option<Session>,
    /// Edits made this run, by file: the edit and history of the version that was open, its
    /// number and name.
    kept: HashMap<PathBuf, (Edit, History, u32, String)>,
    /// History, styles, presets, versions and batch work (`batch.rs`).
    batch: batch::State,
    /// Merge to HDR (`merge.rs`).
    merge: merge::State,
    /// Panoramas, focus stacks and non-raw HDR (`merges.rs`, `pano.rs`, `stack.rs`,
    /// `fusion.rs`).
    merges: merges::State,
    preview: Preview,
    strip: filmstrip::Strip,
    /// The filmstrip has the keyboard (F6): arrows step through the folder, Esc goes back
    /// to the picture.
    strip_focus: bool,
    /// Where the keyboard was in the panel when F6 moved on (it comes back there).
    panel_last: Option<develop_panel::Item>,
    /// The original is shown.
    before: bool,
    /// The original and the edit split down the middle.
    split: bool,
    /// Where the split's divider sits: the fraction of the picture's width from the reading
    /// direction's start (0: not moved, the middle). Dragged; kept while Develop is open.
    split_at: f32,
    /// The divider is being dragged.
    split_drag: bool,
    /// The divider as last drawn: its x and the picture's rectangle (logical px).
    split_line: Option<(f32, Rect)>,
    /// The speed-edit readout: the control, until when.
    hud: Option<(u32, f64)>,
    store: store::Store,
    /// When the open edit last changed, while it is not saved.
    unsaved: Option<f64>,
    /// The next click on the picture picks point colour's colour.
    picking: bool,
    /// Where the developed picture was drawn (logical px).
    pic: Rect,
    /// The labels over the picture at the last draw (text, where, whether it is a status
    /// such as "Refining…" rather than a fixed label), for the screen reader.
    labels: Vec<(String, Rect, bool)>,
    /// A LUT file the file dialog chose.
    lut_rx: Option<std::sync::mpsc::Receiver<PathBuf>>,
    /// A flat (false) or dark (true) frame the file dialog chose.
    frame_rx: Option<std::sync::mpsc::Receiver<(bool, PathBuf)>>,
    /// A watermark SVG the file dialog chose.
    mark_rx: Option<std::sync::mpsc::Receiver<PathBuf>>,
    /// The watermark drawn for the frame preview: what it was drawn from, and its texture.
    mark_tex: effects::MarkSlot,
    /// The crop tool, while it is open.
    crop: Option<crop::CropTool>,
    /// Where the straightened photo sits on screen (from the last frame drawn).
    layout: Option<crop::Layout>,
    /// Perspective guides on screen while the crop tool draws them (last frame drawn).
    guide_map: Option<crop::GuideMap>,
    /// The display's peak (units of SDR white; 1 on an SDR swapchain).
    display_peak: f32,
    /// AI masks: the networks' worker and what it is computing.
    ai: ai_masks::Worker,
    /// AI denoise: the view's worker and what it runs.
    denoise: ai_denoise::Worker,
    /// AI remove and depth: their worker and states.
    ai2: ai_remove::Worker,
    /// The mask row being edited: its handles on the picture.
    handles: specular_widgets::mask_handles::MaskHandles,
    /// The picture and the warp, for mask handles (from the last frame drawn).
    map: Option<masks::Map>,
    /// Retouch spots' outlines and the selected spot's handles.
    rt_handles: specular_widgets::mask_handles::MaskHandles,
    /// The pointer over the window (logical px), for the brush cursor.
    pointer: Option<(f32, f32)>,
    /// A pen (or finger) is down on the picture: mouse presses the system makes from it
    /// are left alone.
    pen_down: bool,
    /// Other tools' edits of the open photo.
    others: interop::Others,
    /// A sidecar conflict being asked about: the photo, its version, the sidecar's edit.
    conflict: Option<(PathBuf, u32, store::Conflict)>,
    /// The open version's sidecar cannot be fully read: nothing is saved.
    locked: bool,
    /// Scopes, clipping overlays and soft-proofing (`aids.rs`).
    aids: aids::Aids,
    /// The photo shown (a JPEG) and the RAW of its pair developed instead.
    pair: Option<(PathBuf, PathBuf)>,
    /// The folder position last shown, and whether the user last moved back (the raw
    /// developed ahead is the one that way).
    last_index: Option<usize>,
    went_back: bool,
    /// "Make profile from chart…", while its panel is open, and its channels.
    chart: Option<chart_profile::Flow>,
    chart_io: chart_profile::Channels,
}

impl std::fmt::Debug for Develop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Develop")
            .field("open", &self.session.as_ref().map(|s| &s.path))
            .finish_non_exhaustive()
    }
}

impl Develop {
    pub fn active(&self) -> bool {
        self.session.is_some()
    }

    /// The display's peak, from the renderer (HDR editing when above 1).
    pub fn set_display_peak(&mut self, peak: f32) {
        self.display_peak = peak;
    }

    /// Room the panel (width) and the filmstrip (height) take from the image, in logical px
    /// for a window `w` wide at text scale `ts`.
    pub fn inset(&self, w: f32, ts: f32) -> (f32, f32) {
        if !self.active() {
            return (0.0, 0.0);
        }
        (
            DevelopPanel::width(w, ts),
            (filmstrip::HEIGHT * ts.max(1.0)).round(),
        )
    }
}

/// The slider a speed-edit action drives.
fn speed_control(a: Action) -> Option<u32> {
    Some(match a {
        Action::DevelopSpeedExposure => id::EXPOSURE,
        Action::DevelopSpeedContrast => id::CONTRAST,
        Action::DevelopSpeedHighlights => id::HIGHLIGHTS,
        Action::DevelopSpeedShadows => id::SHADOWS,
        Action::DevelopSpeedWhites => id::WHITES,
        Action::DevelopSpeedBlacks => id::BLACKS,
        Action::DevelopSpeedTemperature => id::TEMPERATURE,
        Action::DevelopSpeedVibrance => id::VIBRANCE,
        _ => return None,
    })
}

impl App {
    pub(super) fn develop_active(&self) -> bool {
        self.develop.active()
    }

    /// "Preview while dragging" as it stands (debug builds' `ui-state`).
    #[cfg(debug_assertions)]
    pub(super) fn develop_drag_state(&self) -> serde_json::Value {
        self.develop.preview.drag_state()
    }

    /// The photo open in Develop with its edit as it stands (while Develop shows).
    pub(super) fn develop_open_edit(&self) -> Option<(PathBuf, Edit)> {
        self.develop
            .session
            .as_ref()
            .filter(|_| self.develop_active())
            .map(|s| (s.path.clone(), s.edit.clone()))
    }

    /// Reads and develops ahead the raw the user is likely to open next: the neighbour in
    /// the way they last moved through the folder, with the edit it opens with.
    fn develop_ahead(&mut self) {
        let Some(i) = self.current else {
            self.develop.preview.want_next(None, &self.loader);
            return;
        };
        if self.develop.last_index != Some(i) {
            if let Some(l) = self.develop.last_index {
                self.develop.went_back = i < l;
            }
            self.develop.last_index = Some(i);
        }
        let j = if self.develop.went_back {
            i.checked_sub(1)
        } else {
            Some(i + 1)
        };
        let next = j
            .and_then(|j| self.entries.get(j))
            .filter(|p| controls::is_raw(p))
            .cloned();
        if next.as_deref() == self.develop.preview.next_path() {
            return;
        }
        let edit = next.as_ref().map(|p| match self.develop.kept.get(p) {
            Some(k) => k.0.clone(),
            None => controls::new_edit(p),
        });
        let want = next.as_deref().zip(edit);
        self.develop.preview.want_next(want, &self.loader);
    }

    /// No developed picture is pending (screenshots wait for it).
    pub(super) fn develop_settled(&self) -> bool {
        !self.develop.active() || !self.develop.preview.reading() && !self.develop.preview.busy()
    }

    fn develop_ms(&self) -> i64 {
        (self.now() * 1000.0) as i64
    }

    /// The speed-edit action whose key is held.
    fn develop_held(&self) -> Option<Action> {
        self.hold
            .map(|(a, _)| a)
            .filter(|a| a.is_develop_speed() && self.develop.active())
    }

    fn develop_enter(&mut self) {
        if let Some(why) = Action::ToggleDevelop.disabled(&self.action_state()) {
            let now = self.now();
            self.chrome.toast(
                specular_i18n::I18n::global().tr(why.message_id()),
                None,
                now,
            );
            return;
        }
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let path = self.develop_source_of(path);
        self.develop_open(path);
        self.chrome.announce_status(t!("develop-entered"));
        self.redraw = true;
    }

    /// What Develop develops for the photo at `path`: the RAW of a RAW+JPEG pair, else the
    /// photo itself.
    fn develop_source_of(&mut self, path: PathBuf) -> PathBuf {
        match controls::raw_partner(&path) {
            Some(raw) => {
                let name = raw
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.chrome
                    .announce_status(t!("develop-pair-raw", file = name));
                self.develop.pair = Some((path, raw.clone()));
                raw
            }
            None => {
                self.develop.pair = None;
                path
            }
        }
    }

    fn develop_open(&mut self, path: PathBuf) {
        self.develop.crop = None;
        self.develop.layout = None;
        let lib = self.gallery.library_handle();
        let (edit, history, version, name) = match self.develop.kept.remove(&path) {
            Some(k) => k,
            None => (
                controls::new_edit(&path),
                History::default(),
                0,
                String::new(),
            ),
        };
        // The saved edit (unless kept this run) and the photo's versions.
        self.develop.store.load(&path, version, lib, &self.loader);
        let b = &mut self.develop.batch;
        b.version = version;
        b.version_name = name.clone();
        b.versions = vec![(version, name)];
        b.compare = None;
        let focus = self.develop.session.as_ref().and_then(|s| s.panel.focus());
        let mut s = Session::new(path.clone(), edit, history);
        if focus.is_some() {
            s.panel.set_focus(focus);
        }
        self.develop.session = Some(s);
        self.develop.locked = false;
        self.develop.batch.shelf.load();
        self.develop_cards();
        self.develop_aids_start();
        self.develop.aids.set_ghosts(ghosts::load(&path));
        self.develop_aids_cards();
        self.develop.preview.open(&path, &self.loader);
    }

    /// Puts the open image's edit away (kept for the run), saving it when it changed.
    fn develop_put_away(&mut self) {
        self.develop_save();
        // A chart profile belongs to the photo it was made on.
        self.develop.chart = None;
        self.develop.chart_io.reference = None;
        self.develop.chart_io.measured = None;
        if let Some(s) = self.develop.session.take() {
            let b = &self.develop.batch;
            self.develop.kept.insert(
                s.path,
                (s.edit, s.history, b.version, b.version_name.clone()),
            );
        }
        self.develop.preview.set_other(None);
    }

    /// Saves the open edit if it changed since the last save.
    fn develop_save(&mut self) {
        if self.develop.unsaved.take().is_none() || self.develop.locked {
            return;
        }
        if let Some(s) = self.develop.session.as_ref() {
            let lib = self.gallery.library_handle();
            let on = self.settings.metadata.develop_sidecar;
            let b = &self.develop.batch;
            self.develop.store.save(
                &s.path,
                b.version,
                &b.version_name,
                &s.edit,
                &s.history,
                lib,
                on,
                &self.loader,
            );
        }
    }

    /// A saved edit read back: it replaces the starting edit unless the user changed it. A
    /// photo opened for the first time gets its auto-apply style.
    fn develop_loaded(&mut self) {
        while let Some(l) = self.develop.store.take() {
            let now = self.now();
            let Some(s) = self.develop.session.as_mut() else {
                continue;
            };
            if s.path != l.path || self.develop.batch.version != l.version {
                continue;
            }
            self.develop.batch.versions = l.versions;
            self.develop.others = l.others;
            self.develop.locked = l.locked.is_some();
            if let Some(id) = l.locked {
                let msg = if id == store::NOTICE_PARTIAL {
                    t!("toast-develop-edit-partial")
                } else {
                    t!("toast-develop-sidecar-unreadable")
                };
                self.chrome.toast(msg, None, now);
            }
            let fresh = s.history.steps.is_empty();
            // A sidecar kept as it is is not asked about either: nothing is written.
            if let Some(c) = l.conflict.filter(|_| l.locked.is_none()) {
                self.develop_ask_conflict(&l.path, c);
            }
            let Some(found) = l.found else {
                if fresh && l.version == 0 {
                    // Lightroom settings by setting, only where there is no Develop edit.
                    if self.settings.metadata.develop_import_lightroom
                        && self.develop.others.crs.is_some()
                    {
                        self.develop_import_lightroom();
                    } else {
                        self.develop_auto_apply();
                    }
                }
                self.develop_cards();
                continue;
            };
            if !fresh {
                self.develop_cards();
                continue;
            }
            let focus = self.develop.session.as_ref().and_then(|s| s.panel.focus());
            let mut n = Session::new(l.path.clone(), found.edit, found.history);
            if focus.is_some() {
                n.panel.set_focus(focus);
            }
            self.develop.session = Some(n);
            self.develop.batch.version_name = found.name;
            self.develop_cards();
            self.gallery_set_versions(&l.path, self.develop.batch.versions.len() as u32);
            self.redraw = true;
            let now = self.now();
            for (id, count) in found.notices {
                let msg = match id {
                    specular_develop::model::migrate::NOTICE_NEWER => {
                        t!("toast-develop-edit-newer", count = count)
                    }
                    store::NOTICE_NEWER_PROCESS => t!("toast-develop-edit-newer-process"),
                    _ => t!("toast-develop-edit-migrated", count = count),
                };
                self.chrome.toast(msg, None, now);
            }
        }
    }

    /// The app opened a new GPU device after a driver reset: Develop drops what it made on
    /// the old one and develops the photo again.
    pub(super) fn develop_gpu_reset(&mut self) {
        self.develop.preview.gpu_reset();
        self.develop.strip.gpu_reset();
        self.develop.mark_tex = None;
        self.redraw = true;
    }

    /// The system is short of memory: Develop's engine gives back its caches and spare
    /// buffers (the picture on screen stays).
    pub(super) fn develop_trim_memory(&mut self) {
        self.develop.preview.trim_memory();
    }

    pub(super) fn develop_leave(&mut self) {
        if !self.develop.active() {
            return;
        }
        self.develop_put_away();
        self.develop.aids.set_ghosts(None);
        self.develop.preview.close();
        self.develop.strip.clear();
        self.develop.strip_focus = false;
        self.develop.panel_last = None;
        self.develop.before = false;
        self.develop.split = false;
        self.develop.split_at = 0.0;
        self.develop.split_drag = false;
        self.develop.hud = None;
        self.develop.crop = None;
        self.chrome.announce_status(t!("develop-left"));
        self.redraw = true;
    }

    /// Develop's share of an action; whether it took it.
    pub(super) fn develop_action(&mut self, a: Action) -> bool {
        if self.develop_batch_action(a) {
            return true;
        }
        if a == Action::ToggleDevelop {
            if self.develop.active() {
                self.develop_leave();
            } else {
                self.develop_enter();
            }
            return true;
        }
        if !self.develop.active() {
            return false;
        }
        if self.develop_aids_action(a) {
            self.redraw = true;
            return true;
        }
        match a {
            Action::DevelopBeforeAfter => self.develop_before_after(),
            Action::DevelopSplit => {
                let d = &mut self.develop;
                d.split = !d.split;
                d.before = false;
                let msg = if d.split {
                    t!("develop-split-on")
                } else {
                    t!("develop-split-off")
                };
                self.chrome.announce_status(msg);
            }
            Action::DevelopFocusPanel => {
                self.develop.strip_focus = false;
                if let Some(s) = self.develop.session.as_mut() {
                    s.panel.set_focus(None);
                }
            }
            Action::DevelopNextRegion => self.develop_region(true),
            Action::DevelopPrevRegion => self.develop_region(false),
            Action::DevelopExport => self.develop_export_open(),
            Action::DevelopExportRecipes => self.recipes_open_develop(),
            Action::DevelopCrop => self.develop_crop_toggle(),
            Action::DevelopSpotCrosshair => self.develop_spot_crosshair(),
            Action::DevelopSpotOutlines => {
                if let Some(s) = self.develop.session.as_mut() {
                    let msg = if s.retouch_toggle_outlines() {
                        t!("develop-retouch-outlines-shown")
                    } else {
                        t!("develop-retouch-outlines-hidden")
                    };
                    self.chrome.announce_status(msg);
                }
            }
            Action::Undo | Action::Redo => {
                let r = match self.develop.session.as_mut() {
                    Some(s) if a == Action::Undo => s.undo(),
                    Some(s) => s.redo(),
                    None => Vec::new(),
                };
                self.develop_requests(r);
            }
            // Esc at fit leaves Develop (zoomed in, it fits first).
            Action::Back if self.view.is_at_fit() && self.chrome.modal().is_none() => {
                self.develop_leave();
            }
            a if a.is_develop_speed() => {
                let now = self.now();
                self.develop.hud = speed_control(a).map(|c| (c, now + HUD_S));
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// Opens the crop tool, or keeps the crop and closes it.
    fn develop_crop_toggle(&mut self) {
        let Some(s) = self.develop.session.as_ref() else {
            return;
        };
        if self.develop.crop.is_some() {
            self.develop_crop_outcome(crop::Outcome::Done);
            return;
        }
        self.develop.crop = Some(crop::CropTool::new(s.history.position));
        self.develop.picking = false;
        self.develop.before = false;
        self.develop.split = false;
        self.chrome.announce_status(t!("develop-crop-open"));
    }

    /// A row of the Crop card: the aspect, a typed ratio (asked in a sheet), portrait or
    /// landscape, turn or flip, as the crop tool's keys do them; announced.
    fn develop_crop_card(&mut self, control: u32) {
        let (Some(s), Some(lay)) = (self.develop.session.as_ref(), self.develop.layout) else {
            return;
        };
        let p = s.crop();
        let (n, say) = match control {
            id::CROP_ASPECT => {
                let Some(a) = s.panel.control(control).and_then(crop::card::chosen) else {
                    return;
                };
                crop::with_aspect(&p, &lay, a)
            }
            id::CROP_TYPE => {
                let now = match p.aspect.as_str() {
                    "free" | "original" => String::new(),
                    a => a.to_owned(),
                };
                let sheet = sheet::DevelopSheet::name(
                    sheet::Purpose::CropRatio,
                    t!("develop-crop-ratio-title"),
                    t!("develop-crop-ratio-field"),
                    &now,
                    t!("develop-crop-ratio-ok"),
                );
                self.fileops_open_develop(sheet);
                return;
            }
            id::CROP_SWAP => crop::swapped(&p, &lay),
            id::CROP_TURN_LEFT | id::CROP_TURN_RIGHT => {
                crop::turned_by(&p, &lay, control == id::CROP_TURN_RIGHT)
            }
            id::CROP_FLIP_H | id::CROP_FLIP_V => crop::flipped(&p, control == id::CROP_FLIP_H),
            _ => return,
        };
        self.develop_crop_outcome(crop::Outcome::Set(n, Some(say)));
    }

    /// The typed ratio of the Crop card's sheet: set, or the sheet's way of typing one.
    pub(super) fn develop_crop_ratio(&mut self, typed: &str) {
        let (Some(s), Some(lay)) = (self.develop.session.as_ref(), self.develop.layout) else {
            return;
        };
        let Some(r) = crop::parse_ratio(typed) else {
            self.chrome.announce_status(t!("develop-crop-ratio-bad"));
            let now = self.now();
            self.chrome.toast(t!("develop-crop-ratio-bad"), None, now);
            return;
        };
        let (n, say) = crop::with_aspect(&s.crop(), &lay, &r);
        self.develop_crop_outcome(crop::Outcome::Set(n, Some(say)));
    }

    /// Opens the crop tool on the perspective guides (or switches the open one to them).
    fn develop_crop_guides(&mut self) {
        let Some(s) = self.develop.session.as_ref() else {
            return;
        };
        match self.develop.crop.as_mut() {
            Some(t) => t.guides_on = true,
            None => {
                self.develop.crop = Some(crop::CropTool::new(s.history.position).with_guides());
                self.develop.picking = false;
                self.develop.before = false;
                self.develop.split = false;
            }
        }
        self.chrome.announce_status(t!("develop-persp-guides-on"));
        self.redraw = true;
    }

    /// Applies what the crop tool asked.
    fn develop_crop_outcome(&mut self, o: crop::Outcome) {
        let ms = self.develop_ms();
        match o {
            crop::Outcome::Guides(g, say) => {
                if let Some(s) = self.develop.session.as_mut()
                    && s.set_guides(g, ms)
                {
                    self.develop.unsaved = Some(self.now());
                }
                if let Some(m) = say {
                    self.chrome.announce_status(m);
                }
            }
            crop::Outcome::RemoveGuide(i) => {
                let r = self
                    .develop
                    .session
                    .as_mut()
                    .map(|s| s.persp_ask_remove_guide(i))
                    .unwrap_or_default();
                self.develop_requests(r);
            }
            crop::Outcome::Keystone(dv, dh) => {
                let say = self
                    .develop
                    .session
                    .as_mut()
                    .and_then(|s| s.nudge_keystone(dv, dh, ms));
                if let Some(m) = say {
                    self.develop.unsaved = Some(self.now());
                    self.chrome.announce_status(m);
                }
            }
            crop::Outcome::Set(p, say) => {
                if let Some(s) = self.develop.session.as_mut()
                    && s.set_crop(&p, ms)
                {
                    self.develop.unsaved = Some(self.now());
                }
                if let Some(m) = say {
                    self.chrome.announce_status(m);
                }
            }
            crop::Outcome::Redraw(say) => {
                if let Some(m) = say {
                    self.chrome.announce_status(m);
                }
            }
            crop::Outcome::Done => {
                self.develop.crop = None;
                self.chrome.announce_status(t!("develop-crop-kept"));
            }
            crop::Outcome::Cancel => {
                if let Some(t) = self.develop.crop.take()
                    && let Some(s) = self.develop.session.as_mut()
                    && s.go_to(t.start)
                {
                    self.develop.unsaved = Some(self.now());
                }
                self.chrome.announce_status(t!("develop-crop-cancelled"));
            }
        }
        self.redraw = true;
    }

    fn develop_before_after(&mut self) {
        let d = &mut self.develop;
        d.before = !d.before;
        d.split = false;
        if let Some(s) = d.session.as_mut() {
            s.panel.before = d.before;
        }
        let msg = if d.before {
            t!("develop-showing-before")
        } else {
            t!("develop-showing-after")
        };
        self.chrome.announce_status(msg);
    }

    fn develop_requests(&mut self, r: Vec<Request>) {
        for q in r {
            match q {
                Request::Render => self.develop.unsaved = Some(self.now()),
                Request::Leave => self.develop_leave(),
                Request::BeforeAfter => self.develop_before_after(),
                Request::Export => self.develop_export_open(),
                Request::Say(s) => self.chrome.announce_status(s),
                Request::Confirm(title, ok) => {
                    self.chrome
                        .confirm(title, ok, super::items::DEVELOP_MASK_REMOVE);
                }
                Request::PickFor(kind) => {
                    self.develop.preview.pick_before(kind);
                    self.develop.picking = true;
                    self.develop.crop = None;
                }
                Request::Pick => {
                    self.develop
                        .preview
                        .pick_before(specular_develop::model::ModuleKind::POINT_COLOUR);
                    if let Some(s) = self.develop.session.as_mut() {
                        s.masks.picking = None;
                    }
                    self.develop.picking = true;
                    // A pick is on the cropped picture: the crop tool closes, keeping the crop.
                    self.develop.crop = None;
                    self.chrome.announce_status(t!("develop-point-picking"));
                }
                Request::ChooseLut => self.develop_lut_dialog(),
                Request::ChooseInputProfile => self.develop_input_profile_dialog(),
                Request::ChartProfile => self.develop_chart_start(),
                Request::ChooseFrame(dark) => self.develop_frame_dialog(dark),
                Request::WatermarkText(t) => self.develop_watermark_text(&t),
                Request::ChooseWatermark => self.develop_mark_dialog(),
                Request::OpenLocalAi(at) => {
                    self.chrome.open_settings_at(at);
                    self.redraw = true;
                }
                Request::DenoiseAction => self.develop_denoise_action(),
                Request::Upright(mode) => self.develop.preview.want_upright(mode),
                Request::CropGuides => self.develop_crop_guides(),
                Request::CropCard(c) => self.develop_crop_card(c),
                Request::Menu(item, at) => {
                    self.develop_menu(item, at);
                }
                q @ (Request::SaveMaskPreset(..)
                | Request::ApplyMaskPreset(..)
                | Request::DeleteMaskPreset(_)) => self.develop_mask_preset(q),
            }
        }
        self.redraw = true;
    }

    /// "Choose a LUT file…": the platform's file dialog, off the UI thread.
    fn develop_lut_dialog(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.develop.lut_rx = Some(rx);
        let dir = self
            .develop
            .session
            .as_ref()
            .and_then(|s| s.lut_dir().or_else(|| s.path.parent().map(PathBuf::from)));
        let title = t!("develop-lut-dialog");
        let name = t!("develop-lut-kind");
        self.loader.background("dlg-lut", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&name, &["cube", "3dl", "spi3d", "spi1d", "clf", "ctf"]);
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
    }

    /// "Choose a flat frame…" or "Choose a dark frame…": the platform's file dialog, off the
    /// UI thread.
    /// "Choose an SVG…" for the watermark: the platform's file dialog, off the UI thread.
    fn develop_mark_dialog(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.develop.mark_rx = Some(rx);
        let dir = self
            .develop
            .session
            .as_ref()
            .and_then(|s| s.path.parent().map(PathBuf::from));
        let title = t!("develop-frame-svg-dialog");
        let name = t!("develop-frame-svg-kind");
        self.loader.background("dlg-mark", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&name, &["svg", "svgz"]);
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
    }

    fn develop_frame_dialog(&mut self, dark: bool) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.develop.frame_rx = Some(rx);
        let dir = self.develop.session.as_ref().and_then(|s| {
            s.frame_dir(dark)
                .or_else(|| self.settings.metadata.develop_calibration.clone())
                .or_else(|| s.path.parent().map(PathBuf::from))
        });
        let title = if dark {
            t!("develop-dark-dialog")
        } else {
            t!("develop-flat-dialog")
        };
        let name = t!("develop-raw-kind");
        self.loader.background("dlg-frame", move |l| {
            let mut d = rfd::FileDialog::new().set_title(&title).add_filter(
                &name,
                &[
                    "3fr", "arw", "cr2", "cr3", "dng", "erf", "iiq", "nef", "nrw", "orf", "pef",
                    "raf", "rw2", "rwl", "sr2", "srw",
                ],
            );
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                let _ = tx.send((dark, p));
                l.waker()();
            }
        });
    }

    /// A LUT file chosen in the dialog, and a colour picked on the GPU, into the edit.
    fn develop_results(&mut self) {
        let ms = self.develop_ms();
        let now = self.now();
        let frame = self
            .develop
            .frame_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some((dark, p)) = frame
            && let Some(s) = self.develop.session.as_mut()
        {
            self.develop.frame_rx = None;
            if s.set_frame_path(dark, &p, ms) {
                self.develop.unsaved = Some(now);
            }
            self.redraw = true;
        }
        let mark = self
            .develop
            .mark_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(p) = mark
            && let Some(s) = self.develop.session.as_mut()
        {
            self.develop.mark_rx = None;
            if s.set_watermark_svg(&p, ms) {
                self.develop.unsaved = Some(now);
            }
            self.redraw = true;
        }
        if let Some(why) = self.develop.preview.notice.take() {
            self.chrome.announce_status(why);
        }
        let chosen = self
            .develop
            .lut_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(p) = chosen
            && let Some(s) = self.develop.session.as_mut()
        {
            self.develop.lut_rx = None;
            if s.set_lut_path(&p, ms) {
                self.develop.unsaved = Some(now);
            }
            if let Some(why) = s.lut_problem() {
                self.chrome.announce_status(why);
            }
            self.redraw = true;
        }
        if let Some((mode, found)) = self.develop.preview.take_upright()
            && let Some(s) = self.develop.session.as_mut()
        {
            let r = s.upright_done(mode, found, ms);
            self.develop_requests(r);
        }
        if let Some((i, src)) = self.develop.preview.take_found()
            && let Some(s) = self.develop.session.as_mut()
            && s.retouch_found(i, src, ms)
        {
            self.develop.unsaved = Some(now);
            self.redraw = true;
        }
        if let Some(r) = self.develop.preview.take_picked() {
            match (r, self.develop.session.as_mut()) {
                (Ok(rgb), Some(s)) if s.zone_picking() => {
                    let pct = self.develop.preview.take_picked_percentiles();
                    if let Some(msg) = s.zone_picked(rgb, pct) {
                        self.chrome.announce_status(msg);
                    }
                }
                (Ok(rgb), Some(s)) if s.picking_base => {
                    if s.set_negative_base(rgb, ms) {
                        self.develop.unsaved = Some(now);
                    }
                    self.chrome.announce_status(t!("develop-negative-picked"));
                }
                (Ok(rgb), Some(s)) if s.masks.picking.is_some() => {
                    if s.mask_picked(rgb, ms) {
                        self.develop.unsaved = Some(now);
                    }
                    self.chrome.announce_status(t!("develop-mask-picked"));
                }
                (Ok(rgb), Some(s)) => {
                    if s.set_point_target(rgb, ms) {
                        self.develop.unsaved = Some(now);
                    }
                    self.chrome.announce_status(t!("develop-point-picked"));
                }
                (Err(e), _) => {
                    specular_log::warn!(target: specular_log::GPU, "develop pick: {e}");
                }
                _ => {}
            }
            self.redraw = true;
        }
    }

    fn develop_events(&mut self, ev: Vec<Event>) {
        let ms = self.develop_ms();
        // The styles, snapshots and versions cards are `batch.rs`'s.
        let (aid, ev): (Vec<Event>, Vec<Event>) = ev.into_iter().partition(aids::is_ours);
        if !aid.is_empty() {
            self.develop_aids_events(aid);
        }
        let (ours, ev): (Vec<Event>, Vec<Event>) = ev.into_iter().partition(|e| {
            matches!(e, Event::Value { control, .. }
                    if batch::is_ours(*control) || interop::is_ours(*control))
        });
        let r = match self.develop.session.as_mut() {
            Some(s) => s.handle(&ev, ms),
            None => return,
        };
        // A heavy module's slider being dragged renders drafts, refined on release;
        // so does any other module's slider (half-scale drafts).
        let dragging = ev.iter().rev().find_map(|e| match e {
            Event::Value { control, done, .. } if diffuse::is_diffuse(*control) => {
                Some((!*done).then(|| diffuse::module_of(*control)).flatten())
            }
            Event::Value { control, done, .. }
                if !masks::is_mask(*control) && !retouch::is_retouch(*control) =>
            {
                Some((!*done).then(|| controls::module_of(*control)))
            }
            _ => None,
        });
        if let Some(kind) = dragging {
            // "Preview while dragging": a slider's frame budget is the display's refresh.
            if kind.is_some() && self.develop.preview.draft_module().is_none() {
                let hz = self
                    .gfx
                    .as_ref()
                    .and_then(|g| g.window.current_monitor())
                    .and_then(|m| m.refresh_rate_millihertz())
                    .map(|mhz| f64::from(mhz) / 1000.0);
                self.develop.preview.set_refresh(hz);
            }
            self.develop.preview.set_draft_module(kind);
        }
        let changed = r.contains(&controls::Request::Render);
        self.develop_requests(r);
        for e in ours {
            if let Event::Value { control, .. } = e {
                if interop::is_ours(control) {
                    self.develop_others_control(control);
                } else {
                    self.develop_batch_control(control);
                }
            }
        }
        // Undo, redo or a step back past a snapshot's change: the cards follow.
        if changed && self.develop.batch.compare.is_some() {
            self.develop_cards();
        }
    }

    /// A right-click at `lp` (logical px) on the panel: the menu of what is under it.
    pub(super) fn develop_secondary(&mut self, lp: (f32, f32)) -> bool {
        // On the picture: the aids menu (clipping, soft proof, scopes, print).
        if self
            .develop
            .session
            .as_ref()
            .is_some_and(|s| !s.panel.contains(lp))
            && self.develop.pic.contains(lp)
            && self.develop.crop.is_none()
        {
            return self.develop_aids_menu(lp);
        }
        let Some(item) = self
            .develop
            .session
            .as_ref()
            .filter(|s| s.panel.contains(lp))
            .and_then(|s| s.panel.item_at(lp))
        else {
            return false;
        };
        self.develop_menu(item, lp);
        true
    }

    /// Speed edit: `steps` on the held action's control.
    fn develop_speed(&mut self, a: Action, steps: f32) {
        let Some(c) = speed_control(a) else { return };
        let ms = self.develop_ms();
        let now = self.now();
        if let Some(s) = self.develop.session.as_mut() {
            s.speed(c, steps, ms);
        }
        self.develop.hud = Some((c, now + HUD_S));
        self.redraw = true;
    }

    /// A key while Develop is open, before the keymap: arrows while a speed-edit key is
    /// held, then the panel when it has the keyboard. Whether it was taken.
    pub(super) fn develop_key(&mut self, combo: KeyCombo) -> bool {
        if !self.develop.active() {
            return false;
        }
        if combo.key == Key::Named(NamedKey::Escape) && self.develop_chart_escape() {
            return true;
        }
        if self.develop.picking && combo.key == Key::Named(NamedKey::Escape) {
            self.develop.picking = false;
            self.chrome.announce_status(t!("develop-point-cancelled"));
            self.redraw = true;
            return true;
        }
        // Esc in the filmstrip: back to the picture (not out of Develop).
        if combo.key == Key::Named(NamedKey::Escape)
            && !combo.mods.any()
            && self.develop.strip_focus
            && self
                .develop
                .session
                .as_ref()
                .is_some_and(|s| s.panel.focus().is_none())
        {
            self.develop.strip_focus = false;
            self.chrome.announce_status(t!("develop-region-picture"));
            self.redraw = true;
            return true;
        }
        if combo.key == Key::Named(NamedKey::Escape)
            && let Some(s) = self.develop.session.as_mut()
            && s.zone_edit
            && s.panel.focus().is_none()
        {
            let Request::Say(msg) = s.toggle_zone_edit() else {
                return true;
            };
            self.chrome.announce_status(msg);
            self.redraw = true;
            return true;
        }
        if self.develop.crop.is_some()
            && !self
                .develop
                .session
                .as_ref()
                .is_some_and(|s| s.panel.focus().is_some())
        {
            let out = match (
                self.develop.crop.as_mut(),
                self.develop.layout,
                self.develop.session.as_ref(),
            ) {
                (Some(t), Some(lay), Some(s)) => t.key(combo, &lay, &s.crop()),
                _ => None,
            };
            if let Some(o) = out {
                self.develop_crop_outcome(o);
                return true;
            }
        }
        if self.develop_cross_key(combo) {
            return true;
        }
        if self.develop_retouch_handle_key(combo) || self.develop_retouch_key(combo) {
            return true;
        }
        if self.develop_mask_key(combo) {
            return true;
        }
        if let Some(a) = self.develop_held() {
            let dir = match combo.key {
                Key::Named(NamedKey::ArrowUp | NamedKey::ArrowRight) => 1.0,
                Key::Named(NamedKey::ArrowDown | NamedKey::ArrowLeft) => -1.0,
                _ => 0.0,
            };
            if dir != 0.0 {
                let k = if combo.mods.shift { 0.2 } else { 1.0 };
                self.develop_speed(a, dir * k);
                return true;
            }
        }
        // Shift+F10 on a card or a row: its menu (presets, a style's…).
        let menu_key = combo.key == Key::Named(NamedKey::F(10))
            && combo.mods.shift
            && !combo.mods.ctrl
            && !combo.mods.alt;
        if menu_key
            && let Some(s) = self.develop.session.as_ref()
            && let Some(item) = s.panel.focus()
        {
            let at = s
                .panel
                .item_rect(item)
                .map_or((0.0, 0.0), |r| (r.x + r.w * 0.5, r.y + r.h * 0.5));
            return self.develop_menu(item, at);
        }
        let Some(ev) = self
            .develop
            .session
            .as_mut()
            .and_then(|s| s.panel.key(combo))
        else {
            return false;
        };
        self.develop_events(ev);
        self.redraw = true;
        true
    }

    /// F6 / Shift+F6: the keyboard on to the next region (picture → panel → filmstrip), or
    /// back, announced. The panel takes the keyboard where it left it.
    fn develop_region(&mut self, forward: bool) {
        let d = &mut self.develop;
        let Some(s) = d.session.as_mut() else { return };
        let at = if s.panel.focus().is_some() {
            1
        } else if d.strip_focus {
            2
        } else {
            0
        };
        let to = region_step(at, forward);
        if let Some(f) = s.panel.focus() {
            d.panel_last = Some(f);
        }
        let msg = match to {
            1 => {
                d.strip_focus = false;
                let back = d.panel_last.filter(|f| s.panel.order().contains(f));
                s.panel.set_focus(back);
                t!("develop-region-panel")
            }
            2 => {
                s.panel.blur();
                d.strip_focus = true;
                t!("develop-filmstrip")
            }
            _ => {
                s.panel.blur();
                d.strip_focus = false;
                t!("develop-region-picture")
            }
        };
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// The primary button at `lp` (logical px); whether Develop took it.
    pub(super) fn develop_button(&mut self, lp: (f32, f32), down: bool) -> bool {
        if !self.develop.active() {
            return false;
        }
        let shift = self.mods.shift_key();
        let now = self.now();
        if down {
            // A click anywhere puts the keyboard where it lands, not in the filmstrip.
            self.develop.strip_focus = false;
            // The split's divider: grabbed within a few pixels of it.
            if let Some((x, r)) = self.develop.split_line
                && (lp.0 - x).abs() <= SPLIT_GRAB * self.chrome.text_scale.max(1.0)
                && (r.y..=r.bottom()).contains(&lp.1)
            {
                self.develop.split_drag = true;
                return true;
            }
            if self.develop.strip.rect.contains(lp) {
                if let Some(i) = self.develop.strip.hit(lp) {
                    self.go_to(i);
                }
                return true;
            }
            // The scope dock: its picker and mode chip.
            let p = self.develop.aids.dock.press(lp);
            if p != Press::Outside {
                self.develop_dock_pressed(p);
                return true;
            }
            if self.develop_chart_press(lp) {
                return true;
            }
            let Some(s) = self.develop.session.as_mut() else {
                return false;
            };
            let inside = s.panel.contains(lp);
            let pic = self.develop.pic;
            if self.develop.picking && !inside && pic.contains(lp) && pic.w > 0.0 && pic.h > 0.0 {
                self.develop.picking = false;
                self.develop
                    .preview
                    .pick_at((lp.0 - pic.x) / pic.w, (lp.1 - pic.y) / pic.h);
                self.redraw = true;
                return true;
            }
            if !inside && pic.contains(lp) && self.develop_zone_press(lp) {
                self.redraw = true;
                return true;
            }
            if !inside
                && let (Some(t), Some(map)) =
                    (self.develop.crop.as_mut(), self.develop.guide_map.as_ref())
            {
                let (took, o) = t.guide_press(lp, map);
                if let Some(o) = o {
                    self.develop_crop_outcome(o);
                }
                if took {
                    self.redraw = true;
                    return true;
                }
            }
            let Some(s) = self.develop.session.as_mut() else {
                return false;
            };
            if !inside
                && let (Some(t), Some(lay)) = (self.develop.crop.as_mut(), self.develop.layout)
            {
                let ctrl = self.mods.control_key();
                let p = s.crop();
                if t.press(lp, ctrl, &lay, &p) {
                    self.redraw = true;
                    return true;
                }
            }
            if !inside && pic.contains(lp) && self.develop_mask_press(lp) {
                self.redraw = true;
                return true;
            }
            if !inside && pic.contains(lp) && self.develop_retouch_press(lp) {
                self.redraw = true;
                return true;
            }
            let Some(s) = self.develop.session.as_mut() else {
                return false;
            };
            if let Some(ev) = s.panel.press(lp, shift, now) {
                self.develop_events(ev);
            }
            self.redraw |= inside;
            inside
        } else {
            if self.develop.split_drag {
                self.develop.split_drag = false;
                let f = self.develop.split_at;
                self.chrome.announce_status(t!(
                    "develop-split-moved",
                    percent = (f64::from(f) * 100.0).round() as i64
                ));
                self.redraw = true;
                return true;
            }
            if self.develop_zone_release() {
                self.redraw = true;
                return true;
            }
            if self.develop_mask_release() {
                self.redraw = true;
                return true;
            }
            if self.develop_retouch_release() {
                self.redraw = true;
                return true;
            }
            if let (Some(t), Some(lay), Some(s)) = (
                self.develop.crop.as_mut(),
                self.develop.layout,
                self.develop.session.as_ref(),
            ) && t.dragging()
            {
                let o = t.release(&lay, &s.crop(), self.develop.guide_map.as_ref());
                if let Some(o) = o {
                    self.develop_crop_outcome(o);
                }
                self.redraw = true;
                return true;
            }
            let Some(ev) = self
                .develop
                .session
                .as_mut()
                .and_then(|s| s.panel.release())
            else {
                return false;
            };
            self.develop_events(ev);
            true
        }
    }

    /// The pointer moved; whether a drag in the panel is going on.
    pub(super) fn develop_pointer_move(&mut self, lp: Option<(f32, f32)>) -> bool {
        let shift = self.mods.shift_key();
        if self.develop.split_drag
            && let (Some(lp), Some((_, r))) = (lp, self.develop.split_line)
        {
            let rtl = self.rtl();
            self.develop.split_at = split_fraction(lp.0, r, rtl);
            self.redraw = true;
            return true;
        }
        if self.develop_zone_move(lp) {
            return true;
        }
        if self.develop_mask_move(lp) {
            return true;
        }
        if self.develop_retouch_move(lp) {
            return true;
        }
        if let (Some(lp), Some(t), Some(lay), Some(s)) = (
            lp,
            self.develop.crop.as_mut(),
            self.develop.layout,
            self.develop.session.as_ref(),
        ) && t.dragging()
        {
            if let Some(p) = t.moved(lp, &lay, &s.crop()) {
                self.develop_crop_outcome(crop::Outcome::Set(p, None));
            }
            self.redraw = true;
            return true;
        }
        if self.develop.aids.dock.pointer_move(lp) {
            self.redraw = true;
        }
        let Some(s) = self.develop.session.as_mut() else {
            return false;
        };
        let dragging = s.panel.dragging();
        let hot = s.panel.bar_hot();
        let over = s.panel.hovered();
        let ev = s.panel.pointer_move(lp, shift);
        // The scrollbar widens under the pointer; a new control under it starts the wait
        // for its tooltip (a dimmed control's reason).
        let rehot = s.panel.bar_hot() != hot || s.panel.hovered() != over;
        if !ev.is_empty() {
            self.develop_events(ev);
        }
        if dragging || rehot {
            self.redraw = true;
        }
        dragging
    }

    /// A wheel turn of `dx`, `dy` lines at `lp`; whether Develop took it.
    pub(super) fn develop_wheel(&mut self, lp: (f32, f32), dx: f32, dy: f32) -> bool {
        if !self.develop.active() || (dx == 0.0 && dy == 0.0) {
            return false;
        }
        if dy != 0.0
            && let Some(a) = self.develop_held()
        {
            let k = if self.mods.shift_key() { 0.2 } else { 1.0 };
            self.develop_speed(a, dy * k);
            return true;
        }
        // The filmstrip scrolls (either direction: touchpads swipe sideways).
        if self.develop.strip.rect.contains(lp) {
            let rtl = self.rtl();
            if self
                .develop
                .strip
                .scroll(dx, dy, self.current, self.entries.len(), rtl)
            {
                self.redraw = true;
            }
            return true;
        }
        if dy == 0.0 {
            return false;
        }
        if self
            .develop
            .session
            .as_mut()
            .is_some_and(|s| s.panel.wheel(lp, dy))
        {
            self.redraw = true;
            return true;
        }
        false
    }

    /// Background results, and another image opened while in Develop.
    pub(super) fn develop_tick(&mut self) {
        // Pastes, syncs and matches finish whether Develop is open or not.
        self.develop_batch_tick();
        self.develop_merge_tick();
        self.develop_merges_tick();
        if self.develop_matching() {
            self.redraw = true;
        }
        if !self.develop.active() {
            return;
        }
        let path = self.current.and_then(|i| self.entries.get(i)).cloned();
        // The gallery, or the start screen (no photo, nothing listed and no folder on its
        // way): Develop's panel would stay over them.
        if self.gallery_active()
            || (path.is_none() && self.entries.is_empty() && self.folder.is_none())
        {
            self.develop_leave();
            return;
        }
        let open = self.develop.session.as_ref().map(|s| s.path.clone());
        let paired = |p: &PathBuf| {
            self.develop
                .pair
                .as_ref()
                .is_some_and(|(j, r)| j == p && open.as_ref() == Some(r))
        };
        if let Some(p) = path
            && open.as_ref() != Some(&p)
            && !paired(&p)
        {
            let off = matches!(
                Action::ToggleDevelop.disabled(&self.action_state()),
                Some(specular_input::Disabled::NotPhoto | specular_input::Disabled::DevelopOff)
            );
            if off {
                self.develop_leave();
                return;
            }
            self.develop_put_away();
            let p = self.develop_source_of(p);
            self.develop_open(p);
            self.redraw = true;
        }
        specular_develop::ops::correct::calib::set_folder(
            self.settings.metadata.develop_calibration.clone(),
        );
        self.develop
            .preview
            .set_drag_mode(match self.settings.performance.drag_preview {
                specular_settings::schema::DragPreview::Automatic => {
                    specular_develop::drag::Mode::Automatic
                }
                specular_settings::schema::DragPreview::Sharp => {
                    specular_develop::drag::Mode::Sharp
                }
                specular_settings::schema::DragPreview::Fast => specular_develop::drag::Mode::Fast,
            });
        if self.develop.preview.poll(&self.loader) {
            self.redraw = true;
        }
        self.develop_ahead();
        if let Some(b) = self.develop.preview.take_bins() {
            self.develop.aids.set_bins(b);
            self.redraw = true;
        }
        self.develop_aids_tick();
        self.develop_ai_tick();
        self.develop_denoise_tick();
        self.develop_ai2_tick();
        self.develop_results();
        self.develop_chart_tick();
        self.develop_loaded();
        if self
            .develop
            .unsaved
            .is_some_and(|t| self.now() - t >= store::SAVE_AFTER_S)
        {
            self.develop_save();
        }
        if let Some(specular_develop::source::Note::Cmyk) = self.develop.preview.take_note() {
            let now = self.now();
            self.chrome.toast(t!("develop-source-cmyk"), None, now);
        }
        if let Some(white) = self.develop.preview.take_as_shot()
            && let Some(s) = self.develop.session.as_mut()
        {
            s.set_as_shot(white);
        }
        if let Some(shot) = self.develop.preview.take_lensfun()
            && let Some(s) = self.develop.session.as_mut()
        {
            s.set_lensfun(shot);
        }
        let cur = self.current;
        if self.develop.strip.tick(&self.entries, cur, &self.loader) {
            self.redraw = true;
        }
    }

    /// How long from `now` (seconds) until Develop needs a frame: the speed-edit readout
    /// going, the sidecar save, a dimmed control's reason showing, or match to reference.
    pub(super) fn develop_wake(&self, now: f64) -> Option<f64> {
        // Match to reference measures a photo per frame.
        if self.develop_matching() {
            return Some(0.016);
        }
        let hud = self.develop.hud.map(|h| h.1).filter(|&u| u > now);
        let save = self.develop.unsaved.map(|t| t + store::SAVE_AFTER_S);
        // A dimmed control's reason, once the tooltip wait is over.
        let tip = self
            .develop
            .session
            .as_ref()
            .and_then(|s| s.panel.tip_wake());
        wake_in(now, [hud, save, tip])
    }

    /// A screen reader's request on one of Develop's nodes; whether it was one.
    pub(super) fn develop_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        if !self.develop.active() {
            return false;
        }
        let id = req.target_node.0;
        if (A11Y_DOCK..A11Y_DOCK + 16).contains(&id) {
            if req.action == accesskit::Action::Click
                && let Some(p) = self.develop.aids.dock.a11y_click(A11Y_DOCK, id)
            {
                self.develop_dock_pressed(p);
            }
            return true;
        }
        if (A11Y_STRIP..A11Y_STRIP + (1 << 40)).contains(&id) {
            if req.action == accesskit::Action::Click && id > A11Y_STRIP {
                self.go_to((id - A11Y_STRIP - 1) as usize);
            }
            return true;
        }
        let value = match req.data {
            Some(accesskit::ActionData::NumericValue(v)) => Some(v),
            _ => None,
        };
        let Some(ev) = self
            .develop
            .session
            .as_mut()
            .and_then(|s| s.panel.a11y_action(A11Y_PANEL, id, req.action, value))
        else {
            return false;
        };
        self.develop_events(ev);
        self.redraw = true;
        true
    }
}

impl Develop {
    /// No menu is open: neither a card's ⋯ button nor the scope picker says its menu is.
    pub(super) fn menu_closed(&mut self) {
        self.aids.dock.menu_open = false;
        if let Some(s) = self.session.as_mut() {
            s.panel.menu_open = None;
        }
    }
}

/// Develop's screen-reader nodes (after [`draw`]).
pub(super) fn a11y(
    dev: &Develop,
    tree: &mut accesskit::TreeUpdate,
    entries: &[PathBuf],
    current: Option<usize>,
    scale: f64,
) {
    let Some(s) = dev.session.as_ref() else {
        return;
    };
    s.panel.a11y(tree, A11Y_PANEL, scale);
    dev.aids.dock.a11y(tree, A11Y_DOCK, scale);
    dev.strip
        .a11y(tree, A11Y_STRIP, entries, current, dev.strip_focus, scale);
    dev.handles
        .a11y(tree, A11Y_HANDLES, &t!("develop-mask-handles"), scale);
    dev.rt_handles
        .a11y(tree, A11Y_RT_HANDLES, &t!("develop-retouch-handles"), scale);
    labels_a11y(tree, &dev.labels, scale);
}

/// How near the split's divider a press grabs it (logical px at text scale 1).
const SPLIT_GRAB: f32 = 10.0;

/// The divider's x for fraction `at` of `image` from the reading direction's start (0: the
/// middle).
fn split_x(at: f32, image: Rect, rtl: bool) -> f32 {
    let f = if at > 0.0 { at } else { 0.5 };
    if rtl {
        image.right() - image.w * f
    } else {
        image.x + image.w * f
    }
}

/// The fraction (from the reading direction's start) for the divider dragged to `x`, kept
/// a little inside the picture.
fn split_fraction(x: f32, image: Rect, rtl: bool) -> f32 {
    let f = (x - image.x) / image.w.max(1.0);
    let f = if rtl { 1.0 - f } else { f };
    f.clamp(0.02, 0.98)
}

/// How long from `now` until the soonest of `due` (times on the same clock), at least 0:
/// the frame loop's wake is a delay, not a time.
fn wake_in(now: f64, due: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    due.into_iter()
        .flatten()
        .reduce(f64::min)
        .map(|t| (t - now).max(0.0))
}

/// The region F6 (`forward`) or Shift+F6 moves to from `at`: 0 the picture, 1 the panel,
/// 2 the filmstrip.
fn region_step(at: u8, forward: bool) -> u8 {
    if forward { (at + 1) % 3 } else { (at + 2) % 3 }
}

/// The labels over the picture: "Before" and "After" as text, and the picture's state
/// ("Developing…", "Refining…", "Preview failed") as a status read out when it changes.
fn labels_a11y(tree: &mut accesskit::TreeUpdate, labels: &[(String, Rect, bool)], scale: f64) {
    use accesskit::{Live, Node, NodeId, Role};
    let root = tree.tree.as_ref().map_or(NodeId(1), |t| t.root);
    for (i, (s, r, status)) in labels.iter().enumerate() {
        let id = NodeId(A11Y_LABELS + i as u64);
        let mut n = Node::new(if *status { Role::Status } else { Role::Label });
        n.set_label(s.as_str());
        if *status {
            n.set_live(Live::Polite);
        }
        n.set_bounds(accesskit::Rect::new(
            f64::from(r.x) * scale,
            f64::from(r.y) * scale,
            f64::from(r.right()) * scale,
            f64::from(r.bottom()) * scale,
        ));
        if let Some((_, w)) = tree.nodes.iter_mut().find(|(i, _)| *i == root) {
            w.push_child(id);
        }
        tree.nodes.push((id, n));
    }
}

/// How the view shows the stored image: the picture is made turned the same way.
pub(in crate::app) fn orient_of(view: &specular_render::View) -> Orient {
    let m = view.stored_from_screen().invert().m;
    if m[0].abs() < m[1].abs() {
        Orient {
            transpose: true,
            flip_x: m[1] < 0.0,
            flip_y: m[3] < 0.0,
        }
    } else {
        Orient {
            transpose: false,
            flip_x: m[0] < 0.0,
            flip_y: m[4] < 0.0,
        }
    }
}

/// A small label over the image.
fn chip(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    s: &str,
    at: (f32, f32),
    ts: f32,
    end: bool,
) -> Rect {
    let style = TextStyle::new(metrics::FONT_SMALL * ts).weight(600.0);
    let b = text.layout(s, style, None, Align::Start);
    let g = ts.max(1.0);
    let w = b.width + 16.0 * g;
    let h = b.height + 8.0 * g;
    let x = if end { at.0 - w } else { at.0 };
    let r = Rect::new(x, at.1, w, h);
    dl.rect(r, metrics::RADIUS_SM, pal.glass);
    dl.stroke_rect(r, metrics::RADIUS_SM, pal.glass_border, 1.0);
    text.draw(dl, &b, r.x + 8.0 * g, r.y + 4.0 * g, pal.text);
    r
}

/// Everything Develop draws: the developed picture over the image (`image`, logical px),
/// before/after labels, the filmstrip, the panel and the speed-edit readout. `window` is
/// the whole window (logical px).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dev: &mut Develop,
    dl: &mut DrawList,
    text: &mut Text,
    gpu: &Gpu,
    ui: &mut UiRenderer,
    pal: &Palette,
    view: &specular_render::View,
    image: Rect,
    window: Rect,
    entries: &[PathBuf],
    current: Option<usize>,
    loader: &Loader,
    held: Option<Action>,
    rtl: bool,
    ts: f32,
    now: f64,
) {
    // Match to reference measures a photo per frame.
    if dev.batch.matcher.busy() {
        dev.batch.matcher.step(gpu);
    }
    let Some(s) = dev.session.as_mut() else {
        // Textures of a session just closed still go.
        dev.preview.upload(gpu, ui);
        dev.strip.upload(gpu, ui);
        return;
    };
    dev.preview.upload(gpu, ui);
    dev.strip.upload(gpu, ui);
    let orient = orient_of(view);
    s.panel.before = dev.before;
    let g = ts.max(1.0);
    // The developed picture where the image is.
    let a = view.stored_from_screen().invert();
    let (w, h) = view.stored_size();
    let k = dl.scale.max(0.1);
    let p0 = a.apply((0.0, 0.0));
    let p1 = a.apply((w as f32, h as f32));
    let pic = Rect::from_min_max(
        p0.0.min(p1.0) / k,
        p0.1.min(p1.1) / k,
        p0.0.max(p1.0) / k,
        p0.1.max(p1.1) / k,
    );
    // The straightened photo on screen, and where the crop puts the developed picture.
    let cp = s.crop();
    let plan_now = dev.preview.plan();
    let lay = crop::Layout::new(pic, (w, h), orient, &cp).with_plan(plan_now.as_ref());
    s.frame = Some((lay.w, lay.h));
    s.persp.keystone = lay.keystone;
    dev.layout = Some(lay);
    dev.guide_map = match (&dev.crop, plan_now) {
        (Some(t), Some(pl)) if t.guides_on => {
            Some(crop::GuideMap::new(pl, lay.frame, s.persp().guides))
        }
        _ => None,
    };
    let crop_on = dev.crop.is_none();
    let shown = if crop_on {
        lay.rect(cp.crop)
    } else {
        lay.frame
    };
    // Borders and watermark shown on the picture (asked for): the photo shrinks into
    // the canvas the export gets.
    let framed = s
        .frame_shown()
        .filter(|_| crop_on && !dev.before)
        .map(|fr| {
            // The photo's size in its own pixels, so pixel borders keep their proportion.
            let px = (lay.w / f64::from(lay.frame.w.max(1.0))) as f32;
            let (pw, ph) = (
                (shown.w * px).round().max(1.0) as u32,
                (shown.h * px).round().max(1.0) as u32,
            );
            let mark = effects::mark_texture(&mut dev.mark_tex, gpu, ui, &fr, &s.path, pw);
            let l = fr.layout(pw, ph, mark.map(|m| m.1));
            let f = (shown.w / l.canvas.0 as f32).min(shown.h / l.canvas.1 as f32);
            let (cw, ch) = (l.canvas.0 as f32 * f, l.canvas.1 as f32 * f);
            let o = (
                shown.x + (shown.w - cw) / 2.0,
                shown.y + (shown.h - ch) / 2.0,
            );
            let r = |v: [u32; 4]| {
                Rect::new(
                    o.0 + v[0] as f32 * f,
                    o.1 + v[1] as f32 * f,
                    v[2] as f32 * f,
                    v[3] as f32 * f,
                )
            };
            let c = fr.colour;
            (
                Rect::new(o.0, o.1, cw, ch),
                r(l.image),
                specular_theme::Color::rgba(c[0], c[1], c[2], 1.0),
                mark.zip(l.mark).map(|(m, at)| (m.0, r(at))),
            )
        });
    let shown = framed.as_ref().map_or(shown, |f| f.1);
    // Developed at the size it shows (physical px), never above the image's own.
    let long = (shown.w.max(shown.h) * k).ceil() as u32;
    // A style hovered (or focused) in the Styles card shows on the picture.
    let edit = match batch::previewed_style(&s.panel, &dev.batch) {
        Some(st) => {
            std::borrow::Cow::Owned(st.with_grain_seed(effects::grain_seed(&s.path)).applied(
                &s.shown_edit(),
                specular_develop::model::Registry::builtin(),
                specular_develop::model::ApplyMode::Append,
            ))
        }
        None => s.shown_edit(),
    };
    dev.preview
        .request(&edit, orient, long.min(w.max(h)), crop_on);
    // A snapshot compared: developed too, shown split with the edit.
    let snap = dev
        .batch
        .compare
        .and_then(|i| s.edit.snapshots.get(i))
        .map(|sn| (sn.name.clone(), sn.edit_from(&s.edit)));
    dev.preview.set_other(snap.as_ref().map(|x| &x.1));
    let split = dev.split || snap.is_some();
    dev.preview
        .set_peak(if s.sdr_preview { 1.0 } else { dev.display_peak });
    dev.preview.set_overlay(s.overlay());
    dev.preview.set_aids(dev.aids.request());
    // Colour picks are fractions of the picture as developed (after the warp).
    dev.pic = shown;
    let _ = loader;
    // The divider: where it was dragged, else the middle (from the reading direction's start).
    let mid = split_x(dev.split_at, image, rtl).clamp(pic.x, pic.right());
    // A merge's ghost mask, through the picture's warp.
    let ghost = {
        let plan = dev.preview.plan();
        let px = (
            (shown.w * k).round().max(1.0) as u32,
            (shown.h * k).round().max(1.0) as u32,
        );
        dev.aids.ghost_texture(gpu, ui, plan.as_ref(), px)
    };
    if !dev.before
        && let Some(tex) = dev.preview.texture(orient, crop_on)
    {
        // Split: the original (or the snapshot) on the reading direction's start side.
        let start_side = if rtl {
            Rect::new(mid, image.y, image.right() - mid, image.h)
        } else {
            Rect::new(image.x, image.y, mid - image.x, image.h)
        };
        let clip = if split {
            if rtl {
                Rect::new(image.x, image.y, mid - image.x, image.h)
            } else {
                Rect::new(mid, image.y, image.right() - mid, image.h)
            }
        } else {
            image
        };
        dl.push_clip(clip.intersect(&image));
        // The original underneath does not show around a crop or a turned photo.
        dl.rect(image, 0.0, pal.canvas);
        if let Some((canvas, _, colour, _)) = &framed {
            dl.rect(*canvas, 0.0, *colour);
        }
        dl.image(shown, [0.0, 0.0, 1.0, 1.0], tex, 0.0);
        if let Some((_, _, _, Some((mark, at)))) = &framed {
            dl.image(*at, [0.0, 0.0, 1.0, 1.0], *mark, 0.0);
        }
        // Clipping: over the edit only, never over the original or a snapshot.
        if let Some(ov) = dev.preview.clip_texture(orient, crop_on) {
            dl.image(shown, [0.0, 0.0, 1.0, 1.0], ov, 0.0);
        }
        if let Some(ov) = ghost {
            dl.image(shown, [0.0, 0.0, 1.0, 1.0], ov, 0.0);
        }
        dl.pop_clip();
        if snap.is_some() {
            dl.push_clip(start_side.intersect(&image));
            dl.rect(image, 0.0, pal.canvas);
            if let Some(other) = dev.preview.other_texture(orient, crop_on) {
                dl.image(shown, [0.0, 0.0, 1.0, 1.0], other, 0.0);
            }
            dl.pop_clip();
        }
    }
    // The mask row being edited: outline, handles, brush cursor.
    dev.map = dev.preview.plan().map(|plan| masks::Map { plan, shown });
    // The frame on screen at 100 % (AI denoise runs there).
    dev.denoise.view = dev.map.as_ref().and_then(|m| {
        let vis = image.intersect(&shown);
        let crop_px = m.plan.crop_rect()[2];
        (ai_denoise::at_full_size(shown.w, k, crop_px) && vis.w > 0.0 && vis.h > 0.0)
            .then(|| ai_denoise::view_rect(m, vis))
    });
    let (hs, ol) = match (&dev.map, dev.crop.is_none() && !dev.before) {
        (Some(m), true) => s.handles(m),
        _ => (Vec::new(), Vec::new()),
    };
    dev.handles.set(hs, ol);
    if !dev.handles.handles.is_empty() || s.brush_active() {
        dl.push_clip(image);
        dev.handles.draw(dl, pal, ts);
        if s.brush_active()
            && let (Some(p), Some(m)) = (dev.pointer, dev.map.as_ref())
            && shown.contains(p)
        {
            let b = s.masks.brush;
            specular_widgets::mask_handles::brush_cursor(
                dl,
                p,
                b.size * m.px_per_long(),
                b.hardness,
                ts,
            );
        }
        dl.pop_clip();
    }
    // Retouch spots, while the Retouch card is open and no mask row is being edited.
    let (hs, ol) = match &dev.map {
        Some(m)
            if dev.crop.is_none()
                && !dev.before
                && dev.handles.handles.is_empty()
                && s.retouch_on() =>
        {
            s.retouch_handles(m)
        }
        _ => (Vec::new(), Vec::new()),
    };
    dev.rt_handles.set(hs, ol);
    if !dev.rt_handles.outlines.is_empty() || !dev.rt_handles.handles.is_empty() {
        dl.push_clip(image);
        dev.rt_handles.draw(dl, pal, ts);
        dl.pop_clip();
    }
    // The spot crosshair: the next spot's size around it.
    if dev.crop.is_none()
        && !dev.before
        && let Some(m) = dev.map.as_ref()
        && let Some(c) = s.retouch_cross_screen(m)
    {
        dl.push_clip(image);
        specular_widgets::mask_handles::brush_cursor(
            dl,
            c,
            s.retouch_state.size * m.px_per_long(),
            1.0,
            ts,
        );
        dl.pop_clip();
        super::cursor::draw_crosshair(dl, pal, c, image);
    }
    if s.retouch_on()
        && s.retouch_state.brush
        && dev.handles.handles.is_empty()
        && let (Some(p), Some(m)) = (dev.pointer, dev.map.as_ref())
        && shown.contains(p)
    {
        dl.push_clip(image);
        specular_widgets::mask_handles::brush_cursor(
            dl,
            p,
            s.retouch_state.size * m.px_per_long(),
            1.0,
            ts,
        );
        dl.pop_clip();
    }
    // A chart profile: the corners marked so far, and its panel.
    if let Some(f) = dev.chart.as_mut() {
        if let Some(m) = dev.map.as_ref() {
            dl.push_clip(image);
            for (i, c) in f.clicks.iter().enumerate() {
                if let Some(p) = m.to_screen(*c) {
                    let r = 6.0 * g;
                    dl.circle(p, r + 1.5 * g, pal.canvas);
                    dl.circle(p, r, pal.accent);
                    let b = text.layout(
                        &(i + 1).to_string(),
                        specular_gfx::TextStyle::new(specular_theme::metrics::FONT_SMALL * g),
                        None,
                        specular_gfx::Align::Start,
                    );
                    text.draw(dl, &b, p.0 + r + 3.0 * g, p.1 - b.height / 2.0, pal.text);
                }
            }
            dl.pop_clip();
        }
        f.draw(dl, text, pal, image, rtl, ts);
    }
    if !dev.before
        && let Some(t) = dev.crop.as_ref()
    {
        dl.push_clip(image);
        let tex = dev.preview.texture(orient, false);
        t.draw(
            dl,
            text,
            pal,
            &lay,
            &cp,
            rtl,
            ts,
            dev.guide_map.as_ref(),
            tex,
        );
        dl.pop_clip();
    }
    dev.labels.clear();
    let top = image.y + 12.0 * g;
    let (start_x, end_x) = if rtl {
        (image.right() - 12.0 * g, image.x + 12.0 * g)
    } else {
        (image.x + 12.0 * g, image.right() - 12.0 * g)
    };
    dev.split_line = (split && !dev.before).then_some((mid, image));
    if split && !dev.before {
        dl.line((mid, image.y), (mid, image.bottom()), 2.0, pal.text);
        // A grip in its middle says it can be dragged.
        let grip = Rect::new(
            mid - 7.0 * g,
            image.center().1 - 18.0 * g,
            14.0 * g,
            36.0 * g,
        );
        dl.rect_border(grip, 7.0 * g, pal.surface_raised, pal.text, 1.5);
        for dy in [-6.0, 0.0, 6.0] {
            let y = image.center().1 + dy * g;
            dl.line((mid - 3.0 * g, y), (mid + 3.0 * g, y), 1.0, pal.text_muted);
        }
        let (bx, ax) = if rtl {
            (mid + 8.0 * g, mid - 8.0 * g)
        } else {
            (mid - 8.0 * g, mid + 8.0 * g)
        };
        let start = snap
            .as_ref()
            .map_or_else(|| t!("develop-before"), |x| x.0.clone());
        let r = chip(dl, text, pal, &start, (bx, top), ts, !rtl);
        dev.labels.push((start, r, false));
        let after = t!("develop-after");
        let r = chip(dl, text, pal, &after, (ax, top), ts, rtl);
        dev.labels.push((after, r, false));
    } else if dev.before {
        let before = t!("develop-before");
        let r = chip(dl, text, pal, &before, (start_x, top), ts, rtl);
        dev.labels.push((before, r, false));
    }
    if let Some(e) = dev.preview.error.as_ref() {
        let _ = e;
        let status = t!("develop-preview-failed");
        let r = chip(dl, text, pal, &status, (end_x, top), ts, !rtl);
        dev.labels.push((status, r, true));
    } else if dev.preview.refining_shown() {
        // A quick draft is shown; the full picture follows.
        let status = t!("develop-refining");
        let r = chip(dl, text, pal, &status, (end_x, top), ts, !rtl);
        dev.labels.push((status, r, true));
    } else if dev.preview.reading() || dev.preview.busy() {
        let status = t!("develop-working");
        let r = chip(dl, text, pal, &status, (end_x, top), ts, !rtl);
        dev.labels.push((status, r, true));
    }
    // The filmstrip under the image, the panel beside both.
    let pw = DevelopPanel::width(window.w, ts);
    let sh = (filmstrip::HEIGHT * g).round();
    let strip = Rect::new(
        if rtl { window.x + pw } else { window.x },
        window.bottom() - sh,
        window.w - pw,
        sh,
    );
    dev.strip
        .draw(dl, pal, strip, entries, current, rtl, ts, dev.strip_focus);
    s.panel.dock = dev.aids.dock.height(text, pw, ts);
    s.panel.clock = now;
    s.panel.draw(dl, text, pal, window, rtl, ts);
    // The scopes in the panel's top area, at the size they show (physical px).
    if dev.aids.dock.shown {
        let (sw, sh) = dev.aids.dock.picture_size(pw, ts);
        let px = ((sw * k).round() as u32, (sh * k).round() as u32);
        let tex = dev.aids.scope_texture(gpu, ui, px);
        let marks = dev.aids.marks();
        let area = s.panel.dock_rect();
        dev.aids
            .dock
            .draw(dl, text, pal, area, tex, &marks, rtl, ts);
    }
    // Speed edit: while its key is held, and a moment after.
    let hud = held
        .and_then(speed_control)
        .or_else(|| dev.hud.filter(|h| h.1 > now).map(|h| h.0));
    if let Some(c) = hud
        && let Some(sl) = s.panel.slider_mut(c)
    {
        let sl = sl.clone();
        develop_panel::speed_hud(dl, text, pal, image, &sl, rtl, ts);
    }
}

impl App {
    /// The picture has the keyboard and a mask row shows handles: Tab walks them, arrows
    /// move them. Whether the key was taken.
    fn develop_mask_key(&mut self, combo: KeyCombo) -> bool {
        use specular_widgets::mask_handles::KeyOutcome;
        let panel_has_keys = self
            .develop
            .session
            .as_ref()
            .is_none_or(|s| s.panel.focus().is_some());
        if panel_has_keys || self.develop.handles.handles.is_empty() {
            return false;
        }
        let ms = self.develop_ms();
        // A chosen polygon point: Delete removes it, Insert adds one after it.
        if !combo.mods.ctrl && !combo.mods.alt {
            let said = match &combo.key {
                Key::Named(NamedKey::Delete | NamedKey::Backspace) => self
                    .develop
                    .session
                    .as_mut()
                    .and_then(|s| s.remove_point(ms)),
                Key::Named(NamedKey::Insert) => self
                    .develop
                    .session
                    .as_mut()
                    .and_then(|s| s.insert_point(ms)),
                _ => None,
            };
            if let Some(said) = said {
                self.develop.handles.focus = None;
                self.chrome.announce_status(said);
                self.develop.unsaved = Some(self.now());
                self.redraw = true;
                return true;
            }
        }
        let Some(o) = self.develop.handles.key(combo) else {
            return false;
        };
        match o {
            KeyOutcome::Focus(i) => {
                if let Some(s) = self.develop.session.as_mut() {
                    s.choose_handle(Some(i));
                }
                let name = self.develop.handles.handles[i].name.clone();
                self.chrome.announce_status(name);
            }
            KeyOutcome::Move { handle, dx, dy } => {
                let at = self.develop.handles.handles[handle].at;
                if let (Some(s), Some(m)) =
                    (self.develop.session.as_mut(), self.develop.map.as_ref())
                    && s.move_handle(m, handle, (at.0 + dx, at.1 + dy))
                {
                    s.commit(ms);
                    self.develop.unsaved = Some(self.now());
                }
            }
            KeyOutcome::Leave => {
                if let Some(s) = self.develop.session.as_mut() {
                    s.masks.active = None;
                    // The graduated filter's handles go too.
                    if s.nd_edit {
                        s.nd_edit = false;
                        s.sync_panel();
                    }
                }
                self.chrome.announce_status(t!("develop-mask-done"));
            }
        }
        self.redraw = true;
        true
    }

    /// A pen or finger on the window while a brush row is being painted (winit gives
    /// Windows pens as touches with a force): a stroke with its pressure. Whether Develop
    /// took it. Without a force the pressure is full.
    pub(super) fn develop_touch(&mut self, t: &specular_gfx::winit::event::Touch) -> bool {
        use specular_gfx::winit::event::TouchPhase;
        let brush = self.develop.active()
            && self
                .develop
                .session
                .as_ref()
                .is_some_and(|s| s.brush_active());
        if !brush {
            return false;
        }
        let lp = self.logical((t.location.x as f32, t.location.y as f32));
        if let Some(s) = self.develop.session.as_mut() {
            s.masks.pressure = t.force.map(|f| f.normalized() as f32);
        }
        match t.phase {
            TouchPhase::Started => {
                self.develop.pen_down = true;
                self.develop_button(lp, true)
            }
            TouchPhase::Moved => self.develop_pointer_move(Some(lp)),
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let took = self.develop_button(lp, false);
                self.develop.pen_down = false;
                took
            }
        }
    }

    /// A pressure reading (macOS: a pen or Force Touch; X11: a pen) for the stroke being
    /// painted.
    pub(super) fn develop_pressure(&mut self, pressure: f32) {
        if let Some(s) = self.develop.session.as_mut()
            && s.masks.painting
            && pressure.is_finite()
        {
            s.masks.pressure = Some(pressure.clamp(0.0, 1.0));
        }
    }

    /// A press on the picture: a handle of the row being edited, or the brush.
    fn develop_mask_press(&mut self, lp: (f32, f32)) -> bool {
        if self.develop_focus_press(lp) || self.develop_ai_press(lp) {
            return true;
        }
        let ms = self.develop_ms();
        // X11 pens give their pressure beside winit's press (`gesture::PenReading`).
        let pen = self.pen_pressure_now();
        let (Some(s), Some(m)) = (self.develop.session.as_mut(), self.develop.map.as_ref()) else {
            return false;
        };
        if s.brush_active() {
            if s.masks.painting {
                return true;
            }
            if !self.develop.pen_down {
                s.masks.pressure = pen;
            }
            s.masks.painting = true;
            s.paint(m, lp, true);
            return true;
        }
        let ts = self.chrome.text_scale;
        let alt = self.mods.alt_key();
        match self.develop.handles.hit(lp, ts) {
            // Alt+click on a polygon's point removes it.
            Some(i) if alt && let Some(said) = s.remove_point_at(i, ms) => {
                self.chrome.announce_status(said);
                self.develop.unsaved = Some(self.now());
                self.develop.handles.focus = None;
                true
            }
            Some(i) => {
                s.choose_handle(Some(i));
                s.masks.dragging = Some(i);
                self.develop.handles.drag = Some(i);
                self.develop.handles.focus = Some(i);
                true
            }
            None => {
                // On a polygon's edge: a new point there.
                let said = s.add_point_at(m, lp, 8.0 * ts, ms);
                match said {
                    Some(said) => {
                        self.chrome.announce_status(said);
                        self.develop.unsaved = Some(self.now());
                        true
                    }
                    None => false,
                }
            }
        }
    }

    /// The pointer moved: a handle drag or a brush stroke goes on. Whether one did.
    fn develop_mask_move(&mut self, lp: Option<(f32, f32)>) -> bool {
        self.develop.pointer = lp;
        let brush = self
            .develop
            .session
            .as_ref()
            .is_some_and(|s| s.brush_active());
        let (Some(lp), Some(s), Some(m)) =
            (lp, self.develop.session.as_mut(), self.develop.map.as_ref())
        else {
            return false;
        };
        if s.masks.painting {
            s.paint(m, lp, false);
            let kind = s.painted_module();
            self.develop.preview.set_draft_module(kind);
            self.redraw = true;
            return true;
        }
        if let Some(i) = s.masks.dragging {
            s.move_handle(m, i, lp);
            self.redraw = true;
            return true;
        }
        let hover = self.develop.handles.hit(lp, self.chrome.text_scale);
        if hover != self.develop.handles.hover || brush {
            self.develop.handles.hover = hover;
            self.redraw = true;
        }
        false
    }

    /// The button went up: the drag or stroke becomes one history step.
    fn develop_mask_release(&mut self) -> bool {
        if self.develop_ai_release() {
            return true;
        }
        let ms = self.develop_ms();
        let Some(s) = self.develop.session.as_mut() else {
            return false;
        };
        if !s.masks.painting && s.masks.dragging.is_none() {
            return false;
        }
        s.masks.painting = false;
        s.masks.dragging = None;
        self.develop.handles.drag = None;
        s.commit(ms);
        self.develop.preview.set_draft_module(None);
        self.develop.unsaved = Some(self.now());
        true
    }

    /// The picture has the keyboard and the selected spot shows handles (and no mask row
    /// does): Tab walks them, arrows move them (Shift: ten times as far), Esc leaves them.
    /// Each move is one history step. Whether the key was taken.
    fn develop_retouch_handle_key(&mut self, combo: KeyCombo) -> bool {
        use specular_widgets::mask_handles::KeyOutcome;
        let ok = self.develop.session.as_ref().is_some_and(|s| {
            s.panel.focus().is_none() && s.retouch_on() && s.retouch_state.selected.is_some()
        });
        if !ok || !self.develop.handles.handles.is_empty() {
            return false;
        }
        let Some(o) = self.develop.rt_handles.key(combo) else {
            return false;
        };
        match o {
            KeyOutcome::Focus(i) => {
                let name = self.develop.rt_handles.handles[i].name.clone();
                self.chrome.announce_status(name);
            }
            KeyOutcome::Move { handle, dx, dy } => {
                let at = self.develop.rt_handles.handles[handle].at;
                let ms = self.develop_ms();
                if let (Some(s), Some(m)) =
                    (self.develop.session.as_mut(), self.develop.map.as_ref())
                    && s.retouch_press(m, at, Some(handle))
                {
                    s.retouch_move(m, (at.0 + dx, at.1 + dy));
                    if s.retouch_release(m, ms) {
                        if let Some(i) = s.retouch_state.search {
                            let r = s.retouch();
                            self.develop.preview.find_source(r, i);
                        }
                        self.develop.unsaved = Some(self.now());
                    }
                }
            }
            KeyOutcome::Leave => {}
        }
        self.redraw = true;
        true
    }

    /// The spot crosshair key: on (where the pointer is over the picture, else the middle of
    /// what shows of it) or off; says so, or why it can't.
    fn develop_spot_crosshair(&mut self) {
        let k = self.dl.scale.max(0.1);
        let vp = self.view.viewport;
        let seen = Rect::new(vp.x / k, vp.y / k, vp.w / k, vp.h / k);
        let pointer = self.develop.pointer;
        let busy = self.develop.crop.is_some() || self.develop.before;
        let (Some(s), Some(m)) = (self.develop.session.as_mut(), self.develop.map.as_ref()) else {
            return;
        };
        let vis = seen.intersect(&m.shown);
        let at = pointer.filter(|p| m.shown.contains(*p)).unwrap_or_else(|| {
            if vis.w > 0.0 && vis.h > 0.0 {
                vis.center()
            } else {
                m.shown.center()
            }
        });
        let msg = if busy || !self.develop.handles.handles.is_empty() {
            None
        } else {
            s.retouch_cross_toggle(m, at)
        };
        let msg = msg.unwrap_or_else(|| t!("develop-retouch-cross-closed"));
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// A key while the spot crosshair is on: arrows move it (Shift ten times as far),
    /// Enter or Space places a spot, Esc puts it away. Whether it was taken.
    fn develop_cross_key(&mut self, combo: KeyCombo) -> bool {
        let m = combo.mods;
        if m.ctrl || m.alt || m.meta {
            return false;
        }
        let ms = self.develop_ms();
        let now = self.now();
        let k = self.dl.scale.max(0.1);
        let (Some(s), Some(map)) = (self.develop.session.as_mut(), self.develop.map.as_ref())
        else {
            return false;
        };
        if s.retouch_state.cross.is_none() {
            return false;
        }
        if !s.retouch_on() || s.panel.focus().is_some() {
            if !s.retouch_on() {
                s.retouch_state.cross = None;
            }
            return false;
        }
        let dir = match combo.key {
            Key::Named(NamedKey::ArrowLeft) => Some((-1.0, 0.0)),
            Key::Named(NamedKey::ArrowRight) => Some((1.0, 0.0)),
            Key::Named(NamedKey::ArrowUp) => Some((0.0, -1.0)),
            Key::Named(NamedKey::ArrowDown) => Some((0.0, 1.0)),
            _ => None,
        };
        let said = if let Some((dx, dy)) = dir {
            let step = retouch::cross_step(map, k, m.shift);
            let said = s.retouch_cross_move(map, dx * step, dy * step);
            // Kept in view: the view pans when the crosshair nears its edge.
            if let Some(q) = s.retouch_cross_screen(map) {
                let vp = self.view.viewport;
                let seen = Rect::new(vp.x / k, vp.y / k, vp.w / k, vp.h / k);
                let (px, py) = super::cursor::pan_into_view(q, seen, 24.0);
                if px != 0.0 || py != 0.0 {
                    self.view.pan_by(px * k, py * k, false);
                }
            }
            said
        } else {
            match combo.key {
                Key::Named(NamedKey::Enter | NamedKey::Space) if !m.shift => {
                    let said = s.retouch_cross_place(map, ms);
                    if let Some(i) = s.retouch_state.search {
                        let r = s.retouch();
                        self.develop.preview.find_source(r, i);
                    }
                    self.develop.unsaved = Some(now);
                    said
                }
                Key::Named(NamedKey::Escape) if !m.shift => {
                    s.retouch_cross_cancel();
                    Some(t!("develop-retouch-cross-off"))
                }
                _ => return false,
            }
        };
        if let Some(msg) = said {
            self.chrome.announce_status(msg);
        }
        self.redraw = true;
        true
    }

    /// Delete or Esc while the picture has the keyboard and a spot is selected.
    fn develop_retouch_key(&mut self, combo: KeyCombo) -> bool {
        let Some(s) = self.develop.session.as_mut() else {
            return false;
        };
        // A mask row being edited keeps its keys (Delete removes a polygon point).
        if s.panel.focus().is_some()
            || !s.retouch_on()
            || s.retouch_state.selected.is_none()
            || !self.develop.handles.handles.is_empty()
        {
            return false;
        }
        let plain = !combo.mods.ctrl && !combo.mods.alt && !combo.mods.shift;
        let r = match combo.key {
            Key::Named(NamedKey::Delete | NamedKey::Backspace) if plain => s.retouch_delete(),
            Key::Named(NamedKey::Escape) => {
                s.retouch_deselect();
                Vec::new()
            }
            _ => return false,
        };
        self.develop_requests(r);
        self.redraw = true;
        true
    }

    /// A press on the picture while retouching: a spot grabbed, placed or painted.
    fn develop_retouch_press(&mut self, lp: (f32, f32)) -> bool {
        let ts = self.chrome.text_scale;
        let handle = self.develop.rt_handles.hit(lp, ts);
        let (Some(s), Some(m)) = (self.develop.session.as_mut(), self.develop.map.as_ref()) else {
            return false;
        };
        if !s.retouch_on() || !self.develop.handles.handles.is_empty() {
            return false;
        }
        let took = s.retouch_press(m, lp, handle);
        self.develop.rt_handles.drag = handle;
        self.develop
            .preview
            .set_draft(s.retouch_state.drag.is_some());
        if took {
            self.develop.unsaved = Some(self.now());
        }
        took
    }

    /// The pointer moved during a retouch drag; whether one goes on.
    fn develop_retouch_move(&mut self, lp: Option<(f32, f32)>) -> bool {
        let (Some(lp), Some(s), Some(m)) =
            (lp, self.develop.session.as_mut(), self.develop.map.as_ref())
        else {
            return false;
        };
        if s.retouch_move(m, lp) {
            self.redraw = true;
            return true;
        }
        if s.retouch_on() && s.retouch_state.brush {
            self.redraw = true;
        }
        let hover = self.develop.rt_handles.hit(lp, self.chrome.text_scale);
        if hover != self.develop.rt_handles.hover {
            self.develop.rt_handles.hover = hover;
            self.redraw = true;
        }
        false
    }

    /// The button went up after a retouch drag: one history step; a new spot's source is
    /// searched for.
    fn develop_retouch_release(&mut self) -> bool {
        let ms = self.develop_ms();
        let (Some(s), Some(m)) = (self.develop.session.as_mut(), self.develop.map.as_ref()) else {
            return false;
        };
        if !s.retouch_release(m, ms) {
            return false;
        }
        self.develop.rt_handles.drag = None;
        self.develop.preview.set_draft(false);
        if let Some(i) = s.retouch_state.search {
            let r = s.retouch();
            self.develop.preview.find_source(r, i);
        }
        self.develop.unsaved = Some(self.now());
        true
    }

    /// The user accepted a removal in the Masks section.
    pub(super) fn develop_mask_confirmed(&mut self) {
        let ms = self.develop_ms();
        let r = match self.develop.session.as_mut() {
            Some(s) => {
                let mut r = s.mask_confirmed(ms);
                r.extend(s.retouch_confirmed(ms));
                r.extend(s.persp_confirmed(ms));
                r
            }
            None => return,
        };
        self.develop_requests(r);
    }
}

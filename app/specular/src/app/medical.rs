//! Medical mode (DICOM grey images): interactive window / level on the GPU, the value under
//! the pointer, CT presets and frame stepping.
//!
//! When the current image is a grey DICOM frame its modality values (HU for CT) are fetched
//! off the UI thread ([`specular_decode::ModalityFrame::values_f32`]) and uploaded as an f32
//! texture ([`ImageRenderer::set_values`]); a window then draws them through the VOI function
//! in the image shader, so dragging never decodes or uploads again. The keymap's medical mode
//! ([`Mode::Medical`], `[medical]` in `keymap.toml`, over the viewer's keys) is turned on:
//! by default `w` toggles the W/L tool (left drag), `0`–`9` are presets, `Ctrl+I` inverts,
//! `.` / `,` step frames, `p` plays them (cine, at the file's frame times), `m` `a` `e` `r`
//! pick the length, angle, ellipse and rectangle measuring tools ([`measure`]), `Shift+M`
//! clears the measurements and `o` shows or hides overlay planes. A length's or an angle's
//! points are dragged to move them; dropped on another of its points, the measurement is
//! removed. Right drag always adjusts
//! the window: horizontal changes the width, vertical the centre, at a rate scaled to the
//! value range (a drag across [`DRAG_SPAN_PX`] covers the whole range).
//!
//! Display shutters (in their grey or colour) and overlay planes (white, or in a presentation
//! state's layer colours) are drawn on the GPU from the frame's mask
//! ([`ImageRenderer::set_graphics`]) after the window, so they stay put while it changes.
//! Esc steps back: a measurement being placed, then cine, then the measuring tool, then
//! the W/L tool.
//!
//! The window stays across the frames of one file and the slices of one series (in series
//! mode each slice is its own file); it resets to the file's default in another series
//! unless `medical.keep_window` is on. Measurements belong to their file: stepping away
//! and back within the series brings them back ([`Medical::moved`]). With no window set and a VOI LUT as the file's
//! default, the CPU raster (the LUT applied by the decoder) is shown.
//!
//! Orientation labels ([`labels`]) mark the patient's directions at the image's edges
//! (`medical.orientation_labels`, `l` for the session). Grayscale Softcopy Presentation
//! States found with the file that refer to the shown frame ([`present`]) are applied, the
//! newest first (`Shift+P` cycles them, then none): their modality, VOI and Presentation
//! LUT shape the values uploaded (display fractions when a table is involved), their
//! rotation, flip and displayed area (with its pixel aspect ratio, again on each frame) set
//! the view, their shutter and activated overlays replace the image's, and their
//! annotations (graphics, compound graphics, upright text) are drawn over the
//! image. Files are never written. The wheel steps frames in medical mode
//! (`medical.wheel_steps_frames`; Ctrl+wheel zooms).
//!
//! [`ImageRenderer::set_values`]: specular_render::ImageRenderer::set_values
//! [`ImageRenderer::set_graphics`]: specular_render::ImageRenderer::set_graphics
//! [`Mode::Medical`]: specular_input::Mode::Medical

pub(super) mod analysis;
#[cfg(test)]
mod cine_shots;
pub(super) mod labels;
pub(super) mod measure;
pub(super) mod present;
#[cfg(test)]
mod shots;
#[cfg(test)]
pub(super) mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, Orientation};
use specular_decode::{ModalityFrame, Voi, VoiFunction};
use specular_dicom::gsps::{DisplayedArea, PresentationState, SizeMode};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::Action;
use specular_render::{GpuImage, Gsdf, VoiShape, Window as WlWindow};
use specular_settings::schema::MedicalDisplay;
use specular_theme::Color;
use specular_theme::metrics::{FONT_BODY, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_widgets::accesskit;

use super::App;
use crate::loader::{Event, ImageId};
use measure::{Measurement, Tool};

/// A drag over this many logical pixels changes the width (or centre) by the whole value
/// range.
pub(super) const DRAG_SPAN_PX: f64 = 600.0;
/// The Undo of the "measurements cleared" toast in 2D (MPR's is the key before).
pub(super) const CLEARED_KEY: u64 = 0x0ED1_0000_0000_0002;
/// AccessKit id of the readout (cull uses 0x5000…, the gallery 0x4000…).
const A11Y_ID: u64 = 0x5800_0000_0000_0000;

/// CT presets for keys 1–8: message id suffix, width, level (HU). Radiopaedia, "Windowing
/// (CT)" (radiopaedia.org/articles/windowing-ct): lungs W1500 L−600, mediastinum W350 L50,
/// bone W1800 L400, brain W80 L40, stroke W40 L40, abdominal soft tissue W400 L50, liver
/// W150 L30, subdural W200 L75 (the middle of its W130–300 / L50–100).
pub(super) const CT_PRESETS: [(&str, f64, f64); 8] = [
    ("lung", 1500.0, -600.0),
    ("mediastinum", 350.0, 50.0),
    ("bone", 1800.0, 400.0),
    ("brain", 80.0, 40.0),
    ("stroke", 40.0, 40.0),
    ("soft-tissue", 400.0, 50.0),
    ("liver", 150.0, 30.0),
    ("subdural", 200.0, 75.0),
];

/// Medical-mode state for the current image.
#[derive(Default)]
pub(super) struct Medical {
    /// The image the state below belongs to.
    id: Option<ImageId>,
    /// Frame fetch in flight, and the frame wanted next (key repeat while one is in flight).
    pending: Option<u32>,
    wanted: Option<u32>,
    /// The shown frame and its values (CPU copy, for the readout).
    frame: Option<Arc<ModalityFrame>>,
    index: u32,
    frames: u32,
    /// The frame the CPU raster shows (the session's page: the middle slice of a research
    /// volume, else the first).
    first: u32,
    /// Values waiting for the image's GPU texture.
    upload: Option<Vec<f32>>,
    /// The fetch failed or the frame is colour: no medical mode for this image.
    off: bool,
    /// The user's window (`None`: the file's default).
    window: Option<WlWindow>,
    /// The file's default as a window (`None` when it is a VOI LUT).
    default: Option<WlWindow>,
    file_invert: bool,
    range: (f64, f64),
    unit: Option<String>,
    /// `w`: left drag adjusts the window.
    pub(super) tool: bool,
    /// Pointer and window at the start of a W/L drag.
    drag: Option<((f32, f32), WlWindow)>,
    /// The readout's lines (value under the pointer, window, frame).
    readout: Vec<String>,
    /// How many of the readout's first lines are about the point under the pointer (read
    /// out to screen readers).
    readout_at: usize,
    /// The measuring tool picked (left clicks and drags place points).
    pub(super) measure: Option<Tool>,
    /// Points of the measurement being placed, and where its last press was (screen).
    draft: Vec<measure::P>,
    draft_press: Option<(f32, f32)>,
    /// This image's measurements, on every frame.
    measurements: Vec<Measurement>,
    /// The measurements `Shift+M` cleared from this image, for the toast's Undo.
    cleared: Vec<Measurement>,
    /// The image's file and its series (series mode), which the state is kept for.
    path: Option<PathBuf>,
    series: Option<String>,
    /// The measurements of the series' other files, by file.
    kept: HashMap<PathBuf, Vec<Measurement>>,
    /// The point of a length or angle being dragged: (measurement, point).
    moving: Option<(usize, usize)>,
    /// The user hid the overlay planes (kept across files).
    hide_overlays: bool,
    /// Files whose values could not be read and the user was told so (kept across files).
    told: std::collections::HashSet<PathBuf>,
    /// The shown frame has shutters or overlays.
    graphics: bool,
    /// The file's frame times (cine), when it states them.
    timing: Option<Vec<Duration>>,
    /// Cine is playing: when the next frame is due.
    cine: Option<Instant>,
    /// Cine steps the files of a series of single-frame images (kept across them).
    cine_series: bool,
    /// Presentation states referring to this image: path and label, newest first.
    states: Vec<(PathBuf, String)>,
    /// The state picked (index into `states`), and whether the user picked it.
    choice: Option<usize>,
    chosen: bool,
    /// The picked state once read.
    ps: Option<Arc<PresentationState>>,
    /// The orientation before a presentation state set one.
    orient_before: Option<Orientation>,
    /// `l`: orientation labels the other way from the setting (kept across files).
    labels_toggled: bool,
    /// The GPU values are display fractions (a presentation state's VOI LUT or
    /// Presentation LUT), not modality values.
    baked: bool,
    /// Segmentations, SUV and time-intensity curves (kept across images).
    pub(super) an: analysis::State,
}

/// The window for a VOI (a VOI LUT has none).
pub(super) fn window_of(voi: &Voi, invert: bool) -> Option<WlWindow> {
    match voi {
        Voi::Window {
            center,
            width,
            function,
        } => Some(WlWindow {
            center: *center,
            width: *width,
            shape: match function {
                VoiFunction::Linear => VoiShape::Linear,
                VoiFunction::LinearExact => VoiShape::LinearExact,
                VoiFunction::Sigmoid => VoiShape::Sigmoid,
            },
            invert,
        }),
        Voi::Lut(_) => None,
    }
}

/// Whether a frame goes to the GPU as display fractions: a presentation state applies a
/// table (a VOI LUT or a Presentation LUT) the window uniform cannot express.
fn baked(frame: &ModalityFrame) -> bool {
    frame.presentation().is_some()
        && (frame.has_presentation_lut() || matches!(frame.default_voi().0, Voi::Lut(_)))
}

/// LINEAR_EXACT over `range`.
pub(super) fn range_window(range: (f64, f64), invert: bool) -> WlWindow {
    WlWindow {
        center: (range.0 + range.1) / 2.0,
        width: (range.1 - range.0).max(1e-6),
        shape: VoiShape::LinearExact,
        invert,
    }
}

/// The window after a drag of `(dx, dy)` logical pixels from `start`: right widens, down
/// raises the centre (the image darkens), a drag of [`DRAG_SPAN_PX`] moving by `span`.
pub(super) fn dragged(start: WlWindow, dx: f64, dy: f64, span: f64) -> WlWindow {
    let k = span.max(1.0) / DRAG_SPAN_PX;
    let min = if start.shape == VoiShape::Linear {
        1.0
    } else {
        span.max(1.0) * 1e-6
    };
    WlWindow {
        center: start.center + dy * k,
        width: (start.width + dx * k).max(min),
        ..start
    }
}

/// A value for the readout: integers as such, others with up to two decimals; a true minus.
pub(super) fn number(x: f64) -> String {
    let s = if (x - x.round()).abs() < 1e-6 {
        format!("{}", x.round() as i64)
    } else {
        specular_i18n::number(x, 2)
    };
    s.replace('-', "\u{2212}")
}

impl Medical {
    /// The window drawn on the GPU: the user's, or the default once another frame than the
    /// CPU raster's is shown.
    fn effective(&self) -> Option<WlWindow> {
        self.window.or_else(|| {
            // The CPU raster has the overlays burnt in: hiding them takes the GPU path.
            // So is a presentation state's rendering.
            let presented = self
                .frame
                .as_ref()
                .is_some_and(|f| f.presentation().is_some());
            (self.index != self.first || self.hide_overlays && self.graphics || presented).then(
                || {
                    self.default
                        .unwrap_or_else(|| range_window(self.range, self.file_invert))
                },
            )
        })
    }

    /// The window to start a change from.
    fn current(&self) -> WlWindow {
        self.window
            .or(self.default)
            .unwrap_or_else(|| range_window(self.range, self.file_invert))
    }

    /// Medical mode is on: a grey frame's values are in.
    fn active(&self) -> bool {
        self.frame.is_some() && !self.off
    }

    /// The state for image `id` (file `path`, of series `series` in series mode) after
    /// this one's. The W/L and measuring tools, hidden overlays, the label side and the
    /// analysis carry over; the user's window within a series, and across series with
    /// `keep_window`. Each file's measurements are kept while its series is browsed and
    /// come back with the file; another series starts without any.
    fn moved(
        &mut self,
        id: Option<ImageId>,
        path: Option<PathBuf>,
        series: Option<String>,
        keep_window: bool,
    ) -> Medical {
        let same = series.is_some() && series == self.series;
        let window = (self.active() && (same || keep_window))
            .then_some(self.window)
            .flatten();
        let mut kept = if same {
            std::mem::take(&mut self.kept)
        } else {
            HashMap::new()
        };
        if same
            && let Some(p) = self.path.take()
            && !self.measurements.is_empty()
        {
            kept.insert(p, std::mem::take(&mut self.measurements));
        }
        let measurements = path
            .as_deref()
            .and_then(|p| kept.remove(p))
            .unwrap_or_default();
        let cine_series = same && self.cine_series && self.cine.is_some();
        Medical {
            cine: self.cine.filter(|_| cine_series),
            cine_series,
            id,
            window,
            tool: self.tool,
            measure: self.measure,
            hide_overlays: self.hide_overlays,
            told: std::mem::take(&mut self.told),
            labels_toggled: self.labels_toggled,
            an: std::mem::take(&mut self.an),
            measurements,
            path,
            series,
            kept,
            ..Default::default()
        }
    }

    /// How long frame `index` shows in cine.
    fn frame_time(&self, index: u32) -> Duration {
        self.timing
            .as_ref()
            .and_then(|t| t.get(index as usize).copied())
            .filter(|d| !d.is_zero())
            .unwrap_or(specular_decode::DEFAULT_CINE_FRAME)
    }

    fn readout_window(&self) -> String {
        let w = self.current();
        t!(
            "medical-window",
            center = number(w.center),
            width = number(w.width)
        )
    }
}

/// The window (centre, width), its unit and the value range, for scripting.
pub(super) type ScriptWindow = ((f64, f64), Option<String>, (f64, f64));

impl App {
    /// Scripting (`view-get`): the window (centre, width), its unit and the value range,
    /// `None` outside medical mode.
    pub(super) fn medical_script_window(&self) -> Option<ScriptWindow> {
        if !self.medical.active() || self.medical.id != self.current_id() {
            return None;
        }
        let w = self.medical.current();
        Some((
            (w.center, w.width),
            self.medical.unit.clone(),
            self.medical.range,
        ))
    }

    /// Scripting (`view-set`): sets the window (`None`: the file's default). False outside
    /// medical mode.
    pub(super) fn medical_script_set_window(&mut self, cw: Option<(f64, f64)>) -> bool {
        if !self.medical.active() || self.medical.id != self.current_id() {
            return false;
        }
        self.medical.window = cw.map(|(center, width)| {
            let cur = self.medical.current();
            WlWindow {
                center,
                width: width.max(1e-6),
                shape: VoiShape::Linear,
                invert: cur.invert,
            }
        });
        self.medical_sync();
        self.redraw = true;
        true
    }

    /// Keeps medical mode in step with the current image: fetches its values, uploads them,
    /// sets the window and the keymap's layer. Cheap when nothing changed.
    pub(super) fn medical_sync(&mut self) {
        let cur = self.current_id();
        if cur != self.medical.id {
            if let Some(old) = self.medical.id
                && let (Some(gfx), Some(g)) = (self.gfx.as_mut(), self.gpu_images.get(old))
            {
                gfx.renderer.clear_values(g);
            }
            let path = cur
                .and_then(|id| self.images.get(&id))
                .map(|i| i.path.clone());
            let series = path.as_deref().and_then(|p| self.series_uid_of(p));
            let keep = self.settings.medical.keep_window;
            self.medical = self.medical.moved(cur, path, series, keep);
        }
        self.medical_analysis_sync();
        let Some(id) = cur else {
            self.set_key_mode(false);
            return;
        };
        let session = self
            .images
            .get(&id)
            .and_then(|i| i.session.clone())
            .filter(|s| s.medical().is_some());
        if let Some(s) = &session {
            self.medical.frames = s.page_count().max(1);
            self.medical.first = s.page_index().min(self.medical.frames - 1);
            if self.medical.timing.is_none() {
                self.medical.timing = s.cine_timing().map(<[Duration]>::to_vec);
            }
            if self.medical.frame.is_none() && self.medical.pending.is_none() && !self.medical.off {
                let first = self.medical.first;
                self.medical_fetch(id, first);
            }
        }
        let on = session.is_some() && self.medical.active();
        self.set_key_mode(on && !self.gallery_active());
        if !on {
            return;
        }
        let Some(gfx) = self.gfx.as_mut() else { return };
        let Some(g) = self.gpu_images.get(id) else {
            return;
        };
        if !g.has_values() {
            // Evicted and recreated since: fetch again (the session keeps the frame).
            match (self.medical.upload.take(), &self.medical.frame) {
                (Some(v), Some(f)) => upload(gfx, g, &v, f),
                _ => {
                    if self.medical.pending.is_none() {
                        let i = self.medical.index;
                        self.medical_fetch(id, i);
                    }
                    return;
                }
            }
        }
        let Some(g) = self.gpu_images.get(id) else {
            return;
        };
        let w = self.medical.effective();
        if g.window() != w {
            g.set_window(w);
            self.redraw = true;
        }
        let r = g.revision;
        g.set_overlays(!self.medical.hide_overlays);
        // HiLo marks the stored bit depth's limits, through the modality LUT.
        let limits = (!self.medical.baked)
            .then_some(self.medical.frame.as_deref())
            .flatten()
            .and_then(hilo_limits);
        g.set_value_limits(limits);
        if g.revision != r {
            self.redraw = true;
        }
    }

    /// The colour bar's range (map start, map end) and unit while medical mode shows the
    /// current image: the window's ends in modality values (HU for CT), swapped when the grey
    /// is inverted, else the values' range. `None` outside medical mode.
    pub(super) fn medical_legend(&self) -> Option<(f64, f64, String)> {
        self.medical_state()?;
        if self.medical.baked || self.medical.off {
            return None;
        }
        let (lo, hi) = match self.medical.effective().or(self.medical.default) {
            Some(w) if w.invert => (w.center + w.width / 2.0, w.center - w.width / 2.0),
            Some(w) => (w.center - w.width / 2.0, w.center + w.width / 2.0),
            None => self.medical.range,
        };
        Some((lo, hi, self.medical.unit.clone().unwrap_or_default()))
    }

    /// Turns the keymap's medical mode on or off (over the viewer, under cull mode).
    fn set_key_mode(&mut self, medical: bool) {
        self.keys.medical = medical;
        self.keys_sync_modes();
    }

    /// The frame count for [`specular_input::ActionState::medical`].
    pub(super) fn medical_state(&self) -> Option<u32> {
        (self.medical.active() && self.medical.id == self.current_id())
            .then_some(self.medical.frames.max(1))
    }

    /// The current image's shown frame with its stored and modality values (the inspection
    /// probes read radiograph grey values and densities from it).
    pub(super) fn medical_frame_values(&self) -> Option<Arc<ModalityFrame>> {
        (self.medical.id == self.current_id())
            .then(|| self.medical.frame.clone())
            .flatten()
    }

    /// The frame shown for the current image (its first when medical mode is off).
    pub(super) fn medical_index(&self) -> u32 {
        if self.medical.id == self.current_id() && self.medical.frame.is_some() {
            self.medical.index
        } else {
            self.medical.first
        }
    }

    /// Shows frame `index` of the current image once it is in medical mode with no frame in
    /// flight (a report's jump); `false` while it is not ready yet.
    pub(super) fn medical_jump_to(&mut self, index: u32) -> bool {
        let Some(id) = self.current_id() else {
            return false;
        };
        if self.medical_state().is_none()
            || self.medical.frame.is_none()
            || self.medical.pending.is_some()
        {
            return false;
        }
        if index < self.medical.frames && index != self.medical.index {
            self.medical.cine = None;
            self.medical_fetch(id, index);
        }
        true
    }

    /// Reads frame `index`'s values on a worker.
    fn medical_fetch(&mut self, id: ImageId, index: u32) {
        let Some(session) = self.images.get(&id).and_then(|i| i.session.clone()) else {
            return;
        };
        self.medical.pending = Some(index);
        let ps = self.medical.ps.clone();
        self.loader.background("dcm-values", move |l| {
            let cancel = CancelToken::new();
            let read = || -> Result<_, String> {
                let mut m = session
                    .modality_frame_at(index, &cancel)
                    // No modality values (not a grey frame): nothing to tell the user.
                    .ok_or_else(String::new)?
                    .map_err(|e| crate::error_text::reason(&e))?;
                if let Some(ps) = &ps
                    && m.sop_instance_uid().is_some_and(|s| ps.refers_to(s, index))
                {
                    m = Arc::new((*m).clone().presented(ps, index));
                }
                let v = if baked(&m) {
                    m.display_values_f32(&m.default_voi().0)
                } else {
                    m.values_f32()
                };
                Ok((m, v))
            };
            l.post(Event::MedicalValues {
                id,
                index,
                values: read(),
            });
        });
    }

    /// A frame's values arrived.
    pub(super) fn medical_values(
        &mut self,
        id: ImageId,
        index: u32,
        values: Result<(Arc<ModalityFrame>, Option<Vec<f32>>), String>,
    ) {
        if self.medical.id != Some(id) {
            return;
        }
        self.medical.pending = None;
        match values {
            Ok((frame, Some(v))) => {
                // The first frame, or another presentation state: the defaults again.
                let first = self
                    .medical
                    .frame
                    .as_ref()
                    .is_none_or(|f| f.presentation() != frame.presentation());
                let had = self.medical.frame.is_some();
                if first {
                    let (voi, _) = frame.default_voi();
                    self.medical.file_invert = frame.inverted();
                    self.medical.baked = baked(&frame);
                    if self.medical.baked {
                        self.medical.default = Some(WlWindow {
                            center: 0.5,
                            width: 1.0,
                            shape: VoiShape::LinearExact,
                            invert: frame.inverted(),
                        });
                        self.medical.range = (0.0, 1.0);
                    } else {
                        self.medical.default = window_of(&voi, frame.inverted());
                        self.medical.range = frame.value_range().unwrap_or((0.0, 1.0));
                    }
                    self.medical.unit = frame.unit().map(str::to_owned);
                }
                self.medical.graphics = frame.graphics().is_some();
                if had && frame.presentation().is_some() {
                    let from = self.medical.index;
                    self.medical_frame_area(from, index, frame.sop_instance_uid());
                }
                self.medical.index = index;
                if let (Some(gfx), Some(g)) = (self.gfx.as_mut(), self.gpu_images.get(id)) {
                    upload(gfx, g, &v, &frame);
                } else {
                    self.medical.upload = Some(v);
                }
                self.medical.frame = Some(frame);
                self.redraw = true;
                if !had {
                    // Its SOP Instance UID picks the presentation states.
                    self.medical_gsps_refresh();
                }
            }
            Ok((_, None)) => self.medical.off = true,
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "img={id} medical values: {e}");
                self.medical.off = true;
                // Said once per file: stepping a damaged series does not repeat it.
                if !e.is_empty()
                    && let Some(p) = self.medical.path.clone()
                    && self.medical.told.insert(p)
                {
                    let now = self.now();
                    self.chrome.toast(t!("medical-off", reason = e), None, now);
                }
            }
        }
        if let Some(w) = self.medical.wanted.take()
            && w != self.medical.index
        {
            self.medical_fetch(id, w);
        } else if self.medical.pending.is_none() && self.medical_stale() {
            // The presentation state changed while this frame was read.
            let i = self.medical.index;
            self.medical_fetch(id, i);
        }
        self.medical_sync();
        self.medical_readout();
    }

    /// The shown frame was made without the presentation state now picked (or with one no
    /// longer picked).
    fn medical_stale(&self) -> bool {
        let Some(f) = self.medical.frame.as_ref().filter(|_| !self.medical.off) else {
            return false;
        };
        let want = self.medical.ps.as_ref().filter(|ps| {
            f.sop_instance_uid()
                .is_some_and(|s| ps.refers_to(s, self.medical.index))
        });
        want.map(|p| p.sop_uid.as_str()) != f.presentation()
    }

    /// Presentation states that refer to the shown image (when medical mode is on).
    pub(super) fn medical_presentation_count(&self) -> usize {
        self.medical_state()
            .map_or(0, |_| self.medical.states.len())
    }

    /// Finds the presentation states found with the file that refer to the shown image
    /// (once both the scan and the first frame are in) and applies the newest unless the
    /// user picked one.
    pub(super) fn medical_gsps_refresh(&mut self) {
        let Some(sop) = self
            .medical
            .frame
            .as_ref()
            .and_then(|f| f.sop_instance_uid())
            .map(str::to_owned)
        else {
            return;
        };
        let mut found: Vec<_> = self
            .series_presentations()
            .iter()
            .filter(|(_, i)| i.references.contains(&sop))
            .map(|(p, i)| {
                let label = if i.presentation_label.is_empty() {
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    i.presentation_label.clone()
                };
                (i.presentation_created.clone(), p.clone(), label)
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let states: Vec<_> = found.into_iter().map(|(_, p, l)| (p, l)).collect();
        if states == self.medical.states {
            return;
        }
        self.medical.states = states;
        if !self.medical.chosen
            || self
                .medical
                .choice
                .is_some_and(|c| c >= self.medical.states.len())
        {
            let c = (!self.medical.states.is_empty()).then_some(0);
            self.medical_gsps_choose(c);
        }
    }

    /// Picks presentation state `c` (none: the file's own rendering): reads it on a worker.
    fn medical_gsps_choose(&mut self, c: Option<usize>) {
        self.medical.choice = c;
        let Some(id) = self.medical.id else { return };
        match c.and_then(|i| self.medical.states.get(i)) {
            Some((path, _)) => {
                let path = path.clone();
                self.loader.background("dcm-gsps", move |l| {
                    let ps = std::fs::read(&path)
                        .map_err(|e| crate::error_text::reason(&e))
                        .and_then(|d| {
                            PresentationState::parse(&d).map_err(|e| crate::error_text::reason(&e))
                        })
                        .map(Arc::new);
                    l.post(Event::MedicalPresentation { id, path, ps });
                });
            }
            None => self.medical_gsps_apply(None),
        }
    }

    /// A presentation state was read.
    pub(super) fn medical_presentation(
        &mut self,
        id: ImageId,
        path: PathBuf,
        ps: Result<Arc<PresentationState>, String>,
    ) {
        let picked = self
            .medical
            .choice
            .and_then(|c| self.medical.states.get(c))
            .is_some_and(|(p, _)| *p == path);
        if self.medical.id != Some(id) || !picked {
            return;
        }
        match ps {
            Ok(ps) => self.medical_gsps_apply(Some(ps)),
            Err(e) => {
                specular_log::debug!(
                    target: specular_log::DECODE,
                    "img={id} presentation state {}: {e}",
                    path.display()
                );
                self.medical_gsps_apply(None);
            }
        }
    }

    /// Applies presentation state `ps` (or none): the view's rotation, flip and displayed
    /// area, then the frame again through its LUTs. The user's window gives way to it.
    fn medical_gsps_apply(&mut self, ps: Option<Arc<PresentationState>>) {
        let Some(id) = self.medical.id else { return };
        let had = self.medical.ps.is_some();
        if ps.is_none() && !had {
            return;
        }
        self.medical.window = None;
        let sop = self
            .medical
            .frame
            .as_ref()
            .and_then(|f| f.sop_instance_uid())
            .map(str::to_owned)
            .unwrap_or_default();
        let index = self.medical.index;
        match &ps {
            Some(p) => {
                if !had {
                    self.medical.orient_before = self
                        .images
                        .get(&id)
                        .and_then(|i| i.orient)
                        .or(Some(Orientation::NORMAL));
                }
                let (q, flip) = p.rotation_flip();
                let o = present::gsps_orientation(q, flip);
                self.set_orientation(|_| o);
                self.medical_gsps_area(p.area_for(&sop, index));
            }
            None => {
                let o = self
                    .medical
                    .orient_before
                    .take()
                    .unwrap_or(Orientation::NORMAL);
                self.set_orientation(|_| o);
                let aspect = self.dicom_pixel_aspect();
                self.view.set_pixel_aspect(aspect);
                self.view.zoom_fit();
            }
        }
        self.after_view_change();
        self.medical.ps = ps;
        if self.medical.pending.is_none() {
            self.medical_fetch(id, index);
        }
        self.medical_readout();
        self.redraw = true;
    }

    /// How many times as tall as wide the current DICOM image's stored pixels are shown:
    /// row over column spacing, else its Pixel Aspect Ratio (1 for other images).
    pub(super) fn dicom_pixel_aspect(&self) -> f32 {
        self.current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.medical())
            .map_or(1.0, |m| m.pixel_aspect() as f32)
    }

    /// Shows a presentation state's displayed area (the whole image without one): its
    /// Presentation Pixel Aspect Ratio, then the area fitted or magnified.
    fn medical_gsps_area(&mut self, area: Option<&DisplayedArea>) {
        let aspect = area.map_or_else(|| self.dicom_pixel_aspect(), |a| a.aspect as f32);
        self.view.set_pixel_aspect(aspect);
        match area {
            Some(a) => {
                let scale = match a.mode {
                    SizeMode::Magnify(m) => Some(m as f32),
                    // True size needs the screen's physical pixel size: fitted.
                    SizeMode::ScaleToFit | SizeMode::TrueSize(_) => None,
                };
                let r = a.stored_rect();
                let r = (r.0 as f32, r.1 as f32, r.2 as f32, r.3 as f32);
                self.view.show_stored_rect(r, scale);
            }
            None => self.view.zoom_fit(),
        }
    }

    /// A new frame under a presentation state whose displayed area differs from the last
    /// frame's (Displayed Area Selection items that reference frames): shows its area.
    fn medical_frame_area(&mut self, from: u32, to: u32, sop: Option<&str>) {
        let (Some(ps), Some(sop)) = (self.medical.ps.clone(), sop) else {
            return;
        };
        if from == to || !ps.refers_to(sop, to) {
            return;
        }
        let (a, b) = (ps.area_for(sop, from), ps.area_for(sop, to));
        if a != b {
            self.medical_gsps_area(b);
            self.after_view_change();
        }
    }

    /// The page actions on a multi-frame image in medical mode: step the frames (next,
    /// previous, first, last), stopping at the ends like the slice keys.
    pub(super) fn medical_page(&mut self, a: Action) {
        self.medical.cine = None;
        let n = self.medical.frames.max(1);
        let base = self.medical.wanted.unwrap_or(self.medical.index);
        if let Some(next) = super::pages::target_page(a, base, n)
            && let Some(id) = self.medical.id
        {
            if self.medical.pending.is_some() {
                self.medical.wanted = Some(next);
            } else {
                self.medical_fetch(id, next);
            }
        }
        self.medical_sync();
        self.medical_readout();
        self.redraw = true;
    }

    /// Medical actions; true when `a` was one.
    pub(super) fn medical_action(&mut self, a: Action) -> bool {
        if self.medical.active()
            && let Some(msg) = self.medical_analysis_action(a)
        {
            if !msg.is_empty() {
                let now = self.now();
                self.chrome.toast(msg, None, now);
            }
            self.medical_readout();
            self.redraw = true;
            return true;
        }
        if !matches!(
            a,
            Action::WindowLevelTool
                | Action::WindowPreset(_)
                | Action::InvertGrey
                | Action::NextSlice
                | Action::PrevSlice
                | Action::Cine
                | Action::MeasureLength
                | Action::MeasureAngle
                | Action::MeasureEllipse
                | Action::MeasureRectangle
                | Action::ClearMeasurements
                | Action::ToggleOverlays
                | Action::ToggleOrientationLabels
                | Action::NextPresentationState
        ) {
            return false;
        }
        if !self.medical.active() {
            return true;
        }
        let now = self.now();
        match a {
            Action::WindowLevelTool => {
                self.medical.tool = !self.medical.tool;
                if self.medical.tool {
                    self.medical.measure = None;
                    self.medical.draft.clear();
                }
                let msg = if self.medical.tool {
                    t!("medical-tool-on")
                } else {
                    t!("medical-tool-off")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::InvertGrey => {
                let mut w = self.medical.current();
                w.invert = !w.invert;
                self.medical.window = Some(w);
            }
            Action::WindowPreset(0) => {
                self.medical.window = None;
                self.chrome.toast(t!("palette-window-default"), None, now);
            }
            Action::WindowPreset(9) => {
                let invert = self.medical.current().invert;
                self.medical.window = Some(range_window(self.medical.range, invert));
                self.chrome
                    .toast(t!("palette-window-full-range"), None, now);
            }
            Action::WindowPreset(n) => {
                let Some(&(_, width, level)) = CT_PRESETS.get(n as usize - 1) else {
                    return true;
                };
                if self.medical.baked {
                    self.chrome
                        .toast(t!("medical-preset-presentation"), None, now);
                    return true;
                }
                if self.medical.unit.as_deref() != Some("HU") {
                    // Not CT: the digits pick the windows the file suggests.
                    let msg = self.medical_file_window(n as usize - 1);
                    self.chrome.toast(msg, None, now);
                    self.medical_sync();
                    self.medical_readout();
                    self.redraw = true;
                    return true;
                }
                let invert = self.medical.current().invert;
                self.medical.window = Some(WlWindow {
                    center: level,
                    width,
                    shape: VoiShape::Linear,
                    invert,
                });
                let msg = a.name();
                self.chrome.toast(msg, None, now);
            }
            Action::Cine => {
                // A series of single-frame files plays file by file, at the file's frame
                // time or 10 frames/s.
                let series = self.medical.frames <= 1 && self.series_active();
                if self.medical.cine.take().is_none() && (self.medical.frames > 1 || series) {
                    let d = self.medical.frame_time(self.medical.index);
                    self.medical.cine = Some(Instant::now() + d);
                    self.medical.cine_series = series;
                }
            }
            Action::MeasureLength
            | Action::MeasureAngle
            | Action::MeasureEllipse
            | Action::MeasureRectangle => {
                let t = Tool::of(a);
                self.medical.draft.clear();
                self.medical.draft_press = None;
                if self.medical.measure == t {
                    self.medical.measure = None;
                    self.chrome.toast(t!("measure-off"), None, now);
                } else {
                    self.medical.measure = t;
                    self.medical.tool = false;
                    if let Some(t) = t {
                        let msg = specular_i18n::I18n::global().tr(t.hint_id());
                        self.chrome.toast(msg, None, now);
                    }
                }
            }
            Action::ClearMeasurements => {
                self.medical.draft.clear();
                if !self.medical.measurements.is_empty() {
                    self.medical.cleared = std::mem::take(&mut self.medical.measurements);
                    self.medical.moving = None;
                    self.chrome
                        .toast(t!("measure-cleared"), Some(CLEARED_KEY), now);
                    self.last_undo = Some(CLEARED_KEY);
                }
            }
            Action::ToggleOverlays => {
                self.medical.hide_overlays = !self.medical.hide_overlays;
                let msg = if self.medical.hide_overlays {
                    t!("medical-overlays-hidden")
                } else {
                    t!("medical-overlays-shown")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::ToggleOrientationLabels => {
                self.medical.labels_toggled = !self.medical.labels_toggled;
                let msg = if self.medical_labels_on() {
                    t!("medical-labels-shown")
                } else {
                    t!("medical-labels-hidden")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::NextPresentationState => {
                let n = self.medical.states.len();
                if n == 0 {
                    return true;
                }
                let c = match self.medical.choice {
                    None => Some(0),
                    Some(i) if i + 1 < n => Some(i + 1),
                    Some(_) => None,
                };
                self.medical.chosen = true;
                self.medical_gsps_choose(c);
                let msg = match c {
                    Some(i) => t!(
                        "medical-presentation",
                        label = self.medical.states[i].1.clone(),
                        index = i as i64 + 1,
                        count = n as i64
                    ),
                    None => t!("medical-presentation-none"),
                };
                self.chrome.toast(msg, None, now);
            }
            Action::NextSlice | Action::PrevSlice if self.medical.frames <= 1 => {
                // One frame per file: the slice keys step the series' files.
                self.medical.cine = None;
                if self.series_active() {
                    self.navigate(if a == Action::NextSlice { 1 } else { -1 });
                    return true;
                }
            }
            Action::NextSlice | Action::PrevSlice => {
                // Stepping by hand stops cine.
                self.medical.cine = None;
                let n = self.medical.frames.max(1);
                let base = self.medical.wanted.unwrap_or(self.medical.index);
                let next = if a == Action::NextSlice {
                    (base + 1).min(n - 1)
                } else {
                    base.saturating_sub(1)
                };
                if next != base
                    && let Some(id) = self.medical.id
                {
                    if self.medical.pending.is_some() {
                        self.medical.wanted = Some(next);
                    } else {
                        self.medical_fetch(id, next);
                    }
                }
            }
            _ => {}
        }
        self.medical_sync();
        self.medical_readout();
        self.redraw = true;
        true
    }

    /// A press that starts a W/L drag (the right button, or the left one with the tool on)
    /// or places a measurement's point (the left one with a measuring tool). True when it was
    /// taken.
    pub(super) fn medical_press(&mut self, p: (f32, f32), primary: bool) -> bool {
        if !self.medical.active()
            || self.shown.is_none()
            || !super::rect_contains(self.view.viewport, p)
        {
            return false;
        }
        // A length's or an angle's point drawn under the pointer is moved (not while
        // placing one).
        if primary
            && self.medical.draft.is_empty()
            && let Some(at) = self.medical_end_at(p)
        {
            self.medical.moving = Some(at);
            self.redraw = true;
            return true;
        }
        if primary && !self.medical.tool && self.medical.measure.is_none() {
            return false;
        }
        if primary && self.medical.measure.is_some() {
            let s = self.medical_stored(p);
            self.medical.draft.push(s);
            self.medical.draft_press = Some(p);
            self.medical_place();
            self.redraw = true;
            return true;
        }
        self.medical.drag = Some((p, self.medical.current()));
        true
    }

    /// Ends a W/L drag or a measuring press: a drag of a few pixels places the release
    /// point too (press–drag–release draws a length or a region in one go). True when there
    /// was one.
    pub(super) fn medical_release(&mut self, primary: bool) -> bool {
        if primary && let Some((i, e)) = self.medical.moving.take() {
            // A point dropped on another of its measurement's removes the measurement.
            let to_screen = self.medical_to_screen();
            // A region's corner dropped on another corner leaves it no width or height.
            if let Some(m) = self.medical.measurements.get(i)
                && measure::dropped_on_another(
                    &measure::handles(m.tool, &m.points),
                    e,
                    to_screen,
                    MEASURE_GONE_PX,
                )
            {
                self.medical.measurements.remove(i);
                let now = self.now();
                self.chrome.toast(t!("mpr-measure-removed"), None, now);
            }
            self.medical_readout();
            self.redraw = true;
            return true;
        }
        if primary && let Some(start) = self.medical.draft_press.take() {
            let s = f64::from(self.dl.scale.max(0.1));
            if let Some(p) = self.pointer
                && !self.medical.draft.is_empty()
                && f64::from((p.0 - start.0).hypot(p.1 - start.1)) / s > MEASURE_DRAG_PX
            {
                let q = self.medical_stored(p);
                self.medical.draft.push(q);
                self.medical_place();
            }
            self.redraw = true;
            return true;
        }
        self.medical.drag.take().is_some()
    }

    /// Stored coordinates to logical screen pixels, as measurements are drawn.
    fn medical_to_screen(&self) -> impl Fn(measure::P) -> measure::P + use<> {
        let a = self.view.stored_from_screen().invert();
        let s = f64::from(self.dl.scale.max(0.1));
        move |p: measure::P| {
            let q = a.apply((p.0 as f32, p.1 as f32));
            (f64::from(q.0) / s, f64::from(q.1) / s)
        }
    }

    /// The length's or angle's point drawn under physical point `p` on the shown frame.
    fn medical_end_at(&self, p: (f32, f32)) -> Option<(usize, usize)> {
        let s = f64::from(self.dl.scale.max(0.1));
        let lp = (f64::from(p.0) / s, f64::from(p.1) / s);
        let hit = HANDLE_HIT * f64::from(self.chrome.text_scale);
        let m = &self.medical;
        measure::end_at(&m.measurements, m.index, self.medical_to_screen(), lp, hit)
    }

    /// A screen point in stored coordinates, inside the image.
    fn medical_stored(&self, p: (f32, f32)) -> measure::P {
        let (x, y) = self.view.stored_from_screen().apply(p);
        let (w, h) = self.view.stored_size();
        (
            f64::from(x).clamp(0.0, f64::from(w)),
            f64::from(y).clamp(0.0, f64::from(h)),
        )
    }

    /// Finishes the measurement being placed once it has its points.
    fn medical_place(&mut self) {
        let Some(tool) = self.medical.measure else {
            return;
        };
        if self.medical.draft.len() < tool.points() {
            return;
        }
        let points = std::mem::take(&mut self.medical.draft);
        self.medical.draft_press = None;
        let Some(frame) = self.medical.frame.clone() else {
            return;
        };
        let lines = measure::label(tool, &points, &frame, self.medical.unit.as_deref());
        self.medical.measurements.push(Measurement {
            tool,
            points,
            frame: self.medical.index,
            lines,
        });
    }

    /// Esc in medical mode, one step: the measurement being placed, cine, the measuring
    /// tool, the W/L tool. True when something was undone.
    pub(super) fn medical_back(&mut self) -> bool {
        if self.medical_state().is_none() {
            return false;
        }
        let m = &mut self.medical;
        let done = if !m.draft.is_empty() {
            m.draft.clear();
            m.draft_press = None;
            true
        } else {
            m.cine.take().is_some() || m.measure.take().is_some() || std::mem::take(&mut m.tool)
        };
        if done {
            self.medical_readout();
            self.redraw = true;
        }
        done
    }

    /// Cine: steps to the next frame when it is due (after the shown one arrived) and asks
    /// to be woken for the one after.
    pub(super) fn medical_tick(&mut self) {
        let Some(due) = self.medical.cine else {
            return;
        };
        if self.medical.cine_series {
            self.medical_series_tick(due);
            return;
        }
        if self.medical_state().is_none_or(|n| n < 2) || self.gallery_active() {
            self.medical.cine = None;
            return;
        }
        let now = Instant::now();
        if now >= due && self.medical.pending.is_none() && self.medical.wanted.is_none() {
            let next = (self.medical.index + 1) % self.medical.frames;
            if let Some(id) = self.medical.id {
                self.medical_fetch(id, next);
            }
            // Keep the rate; a decode slower than the frame time shows frames late, not
            // in a burst.
            let at = (due + self.medical.frame_time(next)).max(now);
            self.medical.cine = Some(at);
        }
        if let Some(t) = self.medical.cine.filter(|&t| t > now) {
            self.wake_by(t, super::Cause::Medical);
        }
    }

    /// Sets the file's window `k` (its `k + 1`th Window Center and Width) and says which it
    /// is; says so when the file has no such window.
    fn medical_file_window(&mut self, k: usize) -> String {
        let Some(f) = self.medical.frame.clone() else {
            return t!("medical-preset-not-ct");
        };
        let count = f.windows().len();
        let Some(&(center, width)) = f.windows().get(k) else {
            return if count == 0 {
                t!("medical-preset-not-ct")
            } else {
                t!("medical-file-window-none", index = k as i64 + 1)
            };
        };
        let invert = self.medical.current().invert;
        let voi = Voi::Window {
            center,
            width,
            function: f.voi_function(),
        };
        self.medical.window = window_of(&voi, invert);
        let (index, count) = (k as i64 + 1, count as i64);
        match f.window_explanation(k) {
            Some(name) => t!(
                "medical-file-window",
                index = index,
                count = count,
                name = name.to_owned()
            ),
            None => t!("medical-file-window-unnamed", index = index, count = count),
        }
    }

    /// Undo of `Shift+M`: the cleared measurements back on the image they were cleared
    /// from (gone once another image is shown).
    pub(super) fn medical_restore_cleared(&mut self) {
        let back = std::mem::take(&mut self.medical.cleared);
        if back.is_empty() {
            return;
        }
        self.medical.measurements.extend(back);
        self.medical_readout();
    }

    /// Cine over a series of single-frame files: the next file when it is due and the shown
    /// one is in (or a second late, so a file that is not a grey image does not stop it),
    /// back to the first after the last.
    fn medical_series_tick(&mut self, due: Instant) {
        if !self.series_active() || self.gallery_active() {
            self.medical.cine = None;
            return;
        }
        let now = Instant::now();
        let ready = self.medical_state().is_some() && self.medical.pending.is_none();
        if now >= due && (ready || now >= due + Duration::from_secs(1)) {
            let d = self.medical.frame_time(0);
            let n = self.entries.len();
            if let Some(cur) = self.current
                && n >= 2
            {
                self.go_to((cur + 1) % n);
            }
            self.medical.cine = Some((due + d).max(now));
        }
        // Waiting for a late file: looked at again shortly, not every frame.
        if let Some(t) = self.medical.cine {
            let soon = now + Duration::from_millis(15);
            self.wake_by(t.max(soon), super::Cause::Medical);
        }
    }

    /// The measurements of the shown frame (and the one being placed) on screen, in logical
    /// pixels.
    pub(super) fn medical_measurements(&self) -> Option<Vec<measure::Drawn>> {
        let m = &self.medical;
        self.medical_state()?;
        if m.measurements.is_empty() && m.draft.is_empty() {
            return None;
        }
        let a = self.view.stored_from_screen().invert();
        let s = self.dl.scale.max(0.1);
        let map = |p: &measure::P| {
            let q = a.apply((p.0 as f32, p.1 as f32));
            (q.0 / s, q.1 / s)
        };
        let shown = |tool: Tool, pts: &[measure::P], lines: Vec<String>, draft: bool| {
            let (outline, closed) = measure::outline(tool, pts);
            measure::Drawn {
                outline: outline.iter().map(map).collect(),
                closed,
                // A region shows its four corners, which can be dragged.
                handles: if draft {
                    pts.iter().map(map).collect()
                } else {
                    measure::handles(tool, pts).iter().map(map).collect()
                },
                lines,
                draft,
            }
        };
        let mut out: Vec<_> = m
            .measurements
            .iter()
            .filter(|x| x.frame == m.index)
            .map(|x| shown(x.tool, &x.points, x.lines.clone(), false))
            .collect();
        if let Some(tool) = m.measure
            && !m.draft.is_empty()
        {
            let mut pts = m.draft.clone();
            if let Some(p) = self
                .pointer
                .filter(|p| super::rect_contains(self.view.viewport, *p))
            {
                pts.push(self.medical_stored(p));
            }
            out.push(shown(tool, &pts, Vec::new(), true));
        }
        Some(out)
    }

    /// Pointer motion: drives a W/L drag and updates the readout. True while dragging.
    pub(super) fn medical_motion(&mut self, p: (f32, f32)) -> bool {
        let mut dragging = false;
        if let Some((i, e)) = self.medical.moving {
            // The point follows the pointer; the label is measured again.
            let q = self.medical_stored(p);
            let unit = self.medical.unit.clone();
            if let Some(frame) = self.medical.frame.clone()
                && let Some(m) = self.medical.measurements.get_mut(i)
            {
                measure::move_handle(m.tool, &mut m.points, e, q);
                m.lines = measure::label(m.tool, &m.points, &frame, unit.as_deref());
            }
            self.redraw = true;
            dragging = true;
        }
        if !self.medical.draft.is_empty() {
            // The rubber band follows the pointer.
            self.redraw = true;
        }
        if let Some((start, w)) = self.medical.drag {
            let s = f64::from(self.dl.scale.max(0.1));
            let dx = f64::from(p.0 - start.0) / s;
            let dy = f64::from(p.1 - start.1) / s;
            let span = self.medical.range.1 - self.medical.range.0;
            self.medical.window = Some(dragged(w, dx, dy, span));
            self.medical_sync();
            dragging = true;
        }
        if self.medical.active() {
            self.medical_readout_at(Some(p));
        }
        dragging
    }

    /// The readout read out to screen readers: the lines about the point under the pointer,
    /// or every line when `all`. `None` outside medical mode.
    pub(super) fn medical_cursor_lines(&self, all: bool) -> Option<&[String]> {
        let m = &self.medical;
        let r = &m.readout;
        m.active().then(|| {
            if all {
                &r[..]
            } else {
                &r[..m.readout_at.min(r.len())]
            }
        })
    }

    fn medical_readout(&mut self) {
        let p = self.pointer;
        self.medical_readout_at(p);
    }

    /// Rebuilds the readout from the CPU values (one lookup), redrawing when it changed.
    fn medical_readout_at(&mut self, p: Option<(f32, f32)>) {
        let m = &self.medical;
        let mut lines = Vec::new();
        let mut at_lines = 0;
        if let Some(f) = m.frame.as_ref().filter(|_| m.active()) {
            let at = p
                .filter(|p| super::rect_contains(self.view.viewport, *p))
                .map(|p| self.view.stored_from_screen().apply(p))
                .filter(|&(x, y)| x >= 0.0 && y >= 0.0)
                .and_then(|(x, y)| {
                    let v = f.value_at(x as u32, y as u32)?;
                    Some((f64::from(x), f64::from(y), v))
                });
            if let Some((x, y, v)) = at {
                lines.push(match &m.unit {
                    Some(u) => format!("{} {u}", number(v)),
                    None => number(v),
                });
                // Real World Value Mapping: the quantity in its own unit ("ADC 0.00123 mm2/s").
                if let Some(rw) = f.real_world_at(x as u32, y as u32) {
                    let mut s = number(rw.value);
                    if let Some(u) = &rw.unit {
                        s = format!("{s} {u}");
                    }
                    if let Some(l) = &rw.label {
                        s = format!("{l} {s}");
                    }
                    lines.push(s);
                }
            }
            lines.extend(self.medical_analysis_readout(at));
            at_lines = lines.len();
            let mut wl = m.readout_window();
            if m.current().invert {
                wl = specular_i18n::dot_join(&[wl, t!("medical-inverted")]);
            }
            lines.push(wl);
            if f.presentation().is_some()
                && let Some((_, label)) = m.choice.and_then(|c| m.states.get(c))
            {
                lines.push(t!("medical-presentation-readout", label = label.clone()));
            }
            if m.frames > 1 {
                lines.push(t!(
                    "medical-frame",
                    frame = i64::from(m.index) + 1,
                    count = i64::from(m.frames)
                ));
            }
            if m.cine.is_some() {
                let fps = 1.0 / m.frame_time(m.index).as_secs_f64();
                lines.push(t!(
                    "medical-cine",
                    rate = number((fps * 10.0).round() / 10.0)
                ));
            }
            if m.tool {
                lines.push(t!("medical-tool"));
            }
            if let Some(tool) = m.measure {
                lines.push(specular_i18n::I18n::global().tr(tool.hint_id()));
                if m.measurements
                    .iter()
                    .any(|x| x.frame == m.index && matches!(x.tool, Tool::Length | Tool::Angle))
                {
                    lines.push(t!("mpr-measure-hint-edit"));
                }
            }
        }
        self.medical.readout_at = at_lines;
        if lines != self.medical.readout {
            self.medical.readout = lines;
            self.redraw = true;
        }
    }

    /// Whether orientation labels show (the setting, flipped by `l`).
    fn medical_labels_on(&self) -> bool {
        self.settings.medical.orientation_labels != self.medical.labels_toggled
    }

    /// What is drawn over a medical image besides measurements, in logical pixels: the
    /// presentation state's annotations and the orientation labels.
    pub(super) fn medical_marks(&self) -> Option<(present::Scene, Vec<labels::Edge>)> {
        self.medical_state()?;
        let m = &self.medical;
        let f = m.frame.as_ref()?;
        if m.hide_overlays {
            return None;
        }
        let s = self.dl.scale.max(0.1);
        let scene = match (&m.ps, f.sop_instance_uid()) {
            (Some(ps), Some(sop)) if f.presentation().is_some() => {
                present::scene(ps, sop, m.index, &self.view, s)
            }
            _ => present::Scene::default(),
        };
        let edges = match f.patient_axes() {
            Some(axes) if self.medical_labels_on() => labels::edges(axes, &self.view, s),
            _ => Vec::new(),
        };
        (!scene.is_empty() || !edges.is_empty()).then_some((scene, edges))
    }

    /// The shown frame and its measurements in stored pixels, for figures and key images
    /// (`None` outside medical mode).
    pub(super) fn medical_for_figure(&self) -> Option<(u32, Vec<super::figure::Measured>)> {
        self.medical_state()?;
        let m = &self.medical;
        let items = m
            .measurements
            .iter()
            .filter(|x| x.frame == m.index)
            .map(|x| {
                let (outline, closed) = measure::outline(x.tool, &x.points);
                super::figure::Measured {
                    outline,
                    closed,
                    label: x.lines.first().cloned().unwrap_or_default(),
                    lines: x.lines.clone(),
                }
            })
            .collect();
        Some((m.index, items))
    }

    /// The readout's lines when it shows.
    pub(super) fn medical_lines(&self) -> Option<Vec<String>> {
        (self.medical_state().is_some() && !self.medical.readout.is_empty())
            .then(|| self.medical.readout.clone())
    }
}

/// A press and release further apart than this (logical pixels) place two points.
const MEASURE_DRAG_PX: f64 = 4.0;

/// A measurement whose point is dropped closer than this (logical px) to another of its
/// points is removed (as in MPR).
const MEASURE_GONE_PX: f64 = 4.0;

/// How near (logical px, times the text size) a press grabs a measurement's point (as in
/// MPR).
const HANDLE_HIT: f64 = 12.0;

/// The modality values of the stored bit depth's smallest and largest sample (BitsStored,
/// signed or not), for HiLo; `None` for float samples.
fn hilo_limits(f: &ModalityFrame) -> Option<[f32; 2]> {
    if let specular_dicom::Pixels::Grey(g) = &f.frame.pixels
        && g.is_float()
    {
        return None;
    }
    let bits = specular_inspect::BitRange::new(f.frame.bits.clamp(1, 32) as u8, f.frame.signed);
    let a = f.modality().apply(bits.min());
    let b = f.modality().apply(bits.max());
    Some([a.min(b) as f32, a.max(b) as f32])
}

/// Uploads a frame's values and its shutter / overlay mask to the image's texture.
fn upload(gfx: &mut super::Gfx, g: &mut GpuImage, values: &[f32], frame: &ModalityFrame) {
    gfx.renderer.set_values(&gfx.gpu, g, values);
    let m = frame
        .graphics()
        .filter(|m| (m.width, m.height) == (g.width, g.height));
    let (shutter, inks) = m.map(mask_colours).unwrap_or_default();
    let mask = m.map(|m| specular_render::ValueMask {
        mask: &m.mask,
        shutter,
        inks: &inks,
    });
    gfx.renderer.set_graphics(&gfx.gpu, g, mask);
}

/// A frame's shutter colour (its Shutter Presentation Color, else its grey) and overlay
/// inks, display-coded 0..=1 for the GPU.
pub(super) fn mask_colours(m: &specular_dicom::Graphics) -> ([f32; 3], Vec<[f32; 3]>) {
    let c = |c: [u8; 3]| c.map(|x| f32::from(x) / 255.0);
    let shutter = m
        .shutter_rgb
        .map_or([f32::from(m.shutter_grey) / 65535.0; 3], c);
    (shutter, m.inks.iter().map(|&i| c(i)).collect())
}

/// The readout box at the bottom-end corner of the image `area` (logical x, y, w, h);
/// returns its rectangle.
pub(super) fn draw_readout(
    dl: &mut DrawList,
    text: &mut Text,
    lines: &[String],
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> Rect {
    draw_readout_clear(dl, text, lines, area, rtl, ts, None)
}

/// A box at `r` moved up clear of the toolbar `bar` when they would overlap (the viewer
/// narrowed by a side panel brings its corner under the window-centred bar).
pub(super) fn clear_of_bar(r: Rect, bar: Option<Rect>) -> Rect {
    match bar {
        Some(b) if r.x < b.x + b.w && r.x + r.w > b.x && r.y + r.h > b.y => {
            Rect::new(r.x, b.y - TOOLBAR_MARGIN - r.h, r.w, r.h)
        }
        _ => r,
    }
}

/// [`draw_readout`], kept clear of the toolbar `bar` (logical px, [`Chrome::bottom_bar`]).
///
/// [`Chrome::bottom_bar`]: specular_widgets::Chrome::bottom_bar
pub(super) fn draw_readout_clear(
    dl: &mut DrawList,
    text: &mut Text,
    lines: &[String],
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
    bar: Option<Rect>,
) -> Rect {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_BODY * ts).tabular();
    // A line wider than the area (a long file name) is cut, never drawn past its edge.
    let max_w = (aw - 2.0 * m - 2.0 * PAD).max(0.0);
    let blocks: Vec<_> = lines
        .iter()
        .map(|l| {
            let l = super::convert::ellipsize(text, l, st, max_w);
            text.layout(&l, st, None, Align::Start)
        })
        .collect();
    let gap = 2.0 * ts;
    let w = blocks.iter().map(|b| b.width).fold(0.0, f32::max) + 2.0 * PAD;
    let h = blocks.iter().map(|b| b.height).sum::<f32>()
        + gap * blocks.len().saturating_sub(1) as f32
        + 2.0 * PAD;
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let r = clear_of_bar(Rect::new(x, ay + ah - m - h, w, h), bar);
    let y = r.y;
    dl.rect(r, RADIUS_MD, Color::rgba(0.0, 0.0, 0.0, 0.6));
    let mut ty = y + PAD;
    for (i, b) in blocks.iter().enumerate() {
        // Lines end at the box's right edge in either direction: towards the corner in a
        // left-to-right UI, on the reading start side in a right-to-left one.
        let tx = x + w - PAD - b.width;
        let c = if i == 0 {
            Color::WHITE
        } else {
            Color::WHITE.with_alpha(0.82)
        };
        text.draw(dl, b, tx, ty, c);
        ty += b.height + gap;
    }
    r
}

/// The readout's AccessKit node: a status (not live: the value follows the pointer) with
/// every line.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, lines: &[String], r: Rect, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let mut n = Node::new(Role::Status);
    n.set_label(format!("{}: {}", t!("medical-readout"), lines.join(", ")));
    n.set_bounds(accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    });
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), n));
}

/// The display medical greys are drawn for (`medical.display_function`): `None` for sRGB
/// greys, else the GSDF between the configured black and white luminance. A white
/// at or below the black is raised to just above it.
pub(super) fn gsdf_display(m: &specular_settings::schema::Medical) -> Option<Gsdf> {
    if m.display_function != MedicalDisplay::Gsdf {
        return None;
    }
    let l_min = m.display_black.max(0.01);
    let l_max = m.display_white.max(l_min * 1.01);
    Some(Gsdf {
        j_min: specular_dicom::gsdf::jnd_index(l_min),
        j_max: specular_dicom::gsdf::jnd_index(l_max),
        l_min,
        l_max,
    })
}

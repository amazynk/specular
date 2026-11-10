//! Multiplanar reformatting (MPR): the volume shown, in three orthogonal views with linked
//! crosshairs. Any volume, in every profile (tier 1): a DICOM series or multi-frame file, a
//! research or industrial CT volume (NIfTI, Analyze, NRRD, MetaImage, raw, Nikon, Zeiss,
//! Bruker, Volume Graphics), a seismic cube, and a scientific image's z-stack or cube
//! (OME-TIFF, CZI, LIF, ND2, ImageJ hyperstacks, FITS, HDF5, Zarr; [`science_vol`]) at its
//! channel and time point shown. Views are axial, coronal and sagittal (with orientation
//! letters) when the volume's axes have patient directions, XY, XZ and YZ otherwise. The
//! medical workflows over it (CT and SUV presets, measuring, fusion, TMTV) keep their tier:
//! their keys and lines show only as the profile shows them.
//!
//! `Shift+R` (or "Multiplanar reformat" in the palette) opens it over the image viewer; Esc
//! or the same key closes it. While it is open medical mode's key layer is on, for any
//! volume. `Shift+Z` maximises the active view (the controls stay beside it), `Shift+E`
//! saves the three planes through the crosshair as PNG images. The volume loads off the UI thread ([`load`]): it shows as soon
//! as its first slices are in (every n-th slice first; the GPU and the readouts bridge the
//! gaps by blending the loaded neighbours) and sharpens as the rest arrive. Series that do not
//! make a faithful volume (gaps, several images at one position, slices that are not
//! parallel) are refused with the reason; over the GPU's 3D texture limit or the memory
//! budget the volume is downsampled, and the panel says so.
//!
//! In each view: left drag moves the crosshair (the other two planes follow), dragging a
//! line's end handle rotates the other two planes about this view's normal (oblique MPR),
//! the wheel steps this view's plane through the volume (`.` and `,` too), Ctrl+wheel zooms,
//! Shift+left or middle drag pans, right drag sets the shared window (presets and inversion
//! as in medical mode). `m` measures distances in patient millimetres; `Shift+S` (or the
//! panel) cycles thin, MIP, MinIP and average slabs. The fourth slot is a live 3D view of the
//! volume ([`volume3d`]) under the controls. Views follow the crosshair; the value under the
//! pointer is sampled from the volume on the CPU as the GPU does
//! ([`specular_render_volume::reference`]).
//!
//! Read-only analysis ([`analysis`]): segmentations (SEG, RTSTRUCT) of the same frame of
//! reference drawn over the planes with a segment list (`Shift+O`, `Alt+O`), SUV readouts
//! and presets on PET, fusion of a PET/SPECT and a CT/MR series (`Shift+U`, colour map
//! `Alt+U`, opacity `Shift+B`, manual nudge `Shift+N`) and TMTV in a drawn sphere or box
//! (`Shift+T`, threshold `Alt+T`).

pub(super) mod analysis;
mod curve_app;
#[cfg(test)]
mod curve_shots;
mod curved;
mod dense;
mod dose;
mod draw;
mod load;
mod planes;
mod science_vol;
mod segments;
#[cfg(test)]
mod shots;
#[cfg(test)]
mod tests;
mod tracts;
mod views;
mod volume3d;
#[cfg(test)]
mod volume_shots;
#[cfg(test)]
mod volume_tests;

use specular_gfx::{Gpu, wgpu};
use specular_i18n::t;
use specular_input::Action;
use specular_render::{PxRect, VoiShape, Window as WlWindow};
use specular_render_volume::{
    Bricks, MprRenderer, VolumeTexture, VrRenderer, isosurface, max_dimension, reference,
};
use specular_theme::Palette;
use specular_widgets::accesskit;

use super::App;
use super::medical::{CT_PRESETS, dragged, measure, number, range_window};
use crate::loader::ImageId;
use draw::{Control, Layout, Placed, Scene};
use load::{Loading, Source, lock};
use views::{Grab, HANDLE_HIT, KINDS, R, V3, Views};
use volume3d::{ORBIT_KEY, ORBIT_PER_PX, Series, Vr3};

pub(super) use curve_app::DELETE_ITEM as CPR_DELETE_ITEM;
pub(super) use load::Length;
pub(super) use science_vol::{
    Stack as ScienceStack, stack as science_stack, z_axis as science_z_axis,
};

/// What MPR opens on: the facts [`volume_kind`] decides from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct VolumeFacts {
    /// Medical mode's frames of the current image.
    pub medical_frames: Option<u32>,
    /// The current image belongs to the series shown (series mode).
    pub in_series: bool,
    /// A research volume's first three dimensions (NIfTI, Analyze, NRRD, MetaImage,
    /// industrial CT, density maps).
    pub research: Option<[usize; 3]>,
    /// A 3-D seismic volume.
    pub seismic: bool,
    /// A scientific image with a z axis ([`science_vol::z_axis`]).
    pub science: bool,
}

/// The kind of volume MPR opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VolumeKind {
    Research,
    Seismic,
    Science,
    Series,
    MultiFrame,
}

/// What MPR would open on, in the order [`App::mpr_open`] tries them; `None` when the
/// image is not a volume. Profiles play no part.
pub(super) fn volume_kind(f: &VolumeFacts) -> Option<VolumeKind> {
    if let Some(d) = f.research {
        return d.iter().all(|&n| n >= 3).then_some(VolumeKind::Research);
    }
    if f.seismic {
        return Some(VolumeKind::Seismic);
    }
    if f.science {
        return Some(VolumeKind::Science);
    }
    match f.medical_frames {
        Some(_) if f.in_series => Some(VolumeKind::Series),
        Some(n) if n >= 3 => Some(VolumeKind::MultiFrame),
        _ => None,
    }
}

/// MPR state: the open session, if any, and the renderer (kept across sessions).
#[derive(Default)]
pub(super) struct Mpr {
    s: Option<Box<Session>>,
    renderer: Option<MprRenderer>,
    /// The display medical greys are drawn for (`medical.display_function`), as in 2D.
    pub(super) gsdf: Option<specular_render::Gsdf>,
    vr: Option<VrRenderer>,
    /// Bumped whenever the planes change (the scene redraws).
    rev: u64,
    /// `--show mpr`: opens once the series scan is in (or after a moment without one).
    show: Option<std::time::Instant>,
}

impl Mpr {
    /// The GPU device was replaced: the volumes read so far are uploaded to `gpu` again and
    /// the renderers made again when next drawn.
    pub(super) fn gpu_reset(&mut self, gpu: &Gpu) {
        self.renderer = None;
        self.vr = None;
        self.rev += 1;
        let Some(s) = self.s.as_mut() else { return };
        let remake = |load: &Loading, tex: &mut Option<VolumeTexture>| -> Result<(), String> {
            if tex.take().is_none() {
                return Ok(());
            }
            let vol = lock(&load.volume);
            let mut t = VolumeTexture::new(gpu, &vol)?;
            t.sync(gpu, &vol)?;
            *tex = Some(t);
            Ok(())
        };
        if let Err(e) = remake(&s.load, &mut s.tex) {
            s.gpu_error = Some(e);
        }
        for ch in &mut s.channels {
            if let Err(e) = remake(&ch.load, &mut ch.tex) {
                s.gpu_error = Some(e);
            }
        }
        s.analysis.gpu_reset(gpu);
    }
}

/// A pointer drag in a view.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Move(usize),
    /// The view, and the pointer's last angle about the crosshair.
    Rotate(usize, f64),
    /// The view, and the last pointer position.
    Pan(usize, (f64, f64)),
    /// Where the drag started and the window then.
    Window((f64, f64), WlWindow),
    /// In the 3D view: orbiting or panning from the last pointer position, or shifting the
    /// transfer window from where the drag started and the window then.
    Orbit((f64, f64)),
    Pan3d((f64, f64)),
    Transfer((f64, f64), (f64, f64)),
    /// Moving an end of a measurement: the view, the measurement and its end (0 or 1).
    Measure(usize, usize, usize),
    /// Moving a curve's control point, or panning the curved reformat from the last
    /// pointer position.
    CurvePoint(usize),
    CprPan((f64, f64)),
}

/// A further channel of a colour composite: its own volume, texture and colour.
struct Channel {
    load: Loading,
    tex: Option<VolumeTexture>,
    tint: [f32; 3],
    /// Its name in the value readout.
    name: String,
    /// The histogram equalisation curve for its window.
    tone: ToneCurve,
    /// The automatic stretch's midtones balance, as the 2D view draws the channel.
    mid: f32,
    /// It could not be read: it leaves the composite.
    failed: bool,
}

/// A composite's values under the pointer: each channel's name and value, four a line, as
/// the 2D view's readout writes them.
fn channel_lines(vals: &[(String, f32)]) -> Vec<String> {
    vals.chunks(4)
        .map(|chunk| {
            let text = chunk
                .iter()
                .map(|(name, x)| format!("{name} {}", number(f64::from(*x).round())))
                .collect::<Vec<_>>()
                .join("  ");
            t!("science-values", values = text)
        })
        .collect()
}

/// The histogram equalisation curve of `vol`'s values in window `w` (about a million of
/// them sampled evenly), as the science view equalises a plane between its limits.
#[cfg(test)]
fn histeq_curve(vol: &specular_dicom::volume::Volume, w: WlWindow) -> Vec<f32> {
    curve_of(&sample_values(vol), w)
}

/// About a million of `vol`'s values, sampled evenly (what its curves are made from).
fn sample_values(vol: &specular_dicom::volume::Volume) -> Vec<f32> {
    let n = vol.voxels.len();
    let stride = (n >> 20).max(1);
    (0..n).step_by(stride).map(|i| vol.voxels.get(i)).collect()
}

/// The histogram equalisation curve of `values` in window `w`.
fn curve_of(values: &[f32], w: WlWindow) -> Vec<f32> {
    let (lo, hi) = (w.center - w.width / 2.0, w.center + w.width / 2.0);
    let lut = specular_core::nd::histeq_lut(values, lo as f32, hi as f32);
    specular_render_volume::tone_curve(&lut)
}

/// A composite layer as drawn: its texture, planes, tint, stretch curve and Auto's
/// midtones balance.
type LayerSet<'a> = (
    &'a VolumeTexture,
    Vec<specular_render_volume::Plane>,
    [f32; 3],
    Vec<f32>,
    f32,
);

/// A finished curve: its window, the curve and the values it was made from.
type CurveDone = (WlWindow, Vec<f32>, std::sync::Arc<Vec<f32>>);

/// Runs a curve's job off the UI thread.
type CurveRunner<'a> = dyn FnMut(Box<dyn FnOnce() + Send>) + 'a;

/// A volume's histogram equalisation curve for the window shown, made off the UI thread:
/// its values sampled once (about a million), then a table per window. While the window is
/// dragged the curve made last stays drawn and one job runs at a time; when the drag stops
/// the curve for where it stopped follows.
#[derive(Default)]
struct ToneCurve {
    /// The curve and the window it was made for.
    made: Option<(WlWindow, Vec<f32>)>,
    samples: Option<std::sync::Arc<Vec<f32>>>,
    job: Option<crossbeam_channel::Receiver<CurveDone>>,
}

impl ToneCurve {
    /// The curve drawn now (empty: linear).
    fn curve(&self) -> Vec<f32> {
        self.made.as_ref().map_or_else(Vec::new, |(_, c)| c.clone())
    }

    /// Takes a finished curve; then, when none is being made and the curve is for another
    /// window than `w`, asks `run` to make one of `vol` for `w`. True when the curve changed.
    fn refresh(
        &mut self,
        vol: &std::sync::Arc<std::sync::Mutex<specular_dicom::volume::Volume>>,
        w: WlWindow,
        run: &mut CurveRunner<'_>,
    ) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok((k, c, s)) => {
                    self.made = Some((k, c));
                    self.samples = Some(s);
                    self.job = None;
                    changed = true;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => return false,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    // The job failed: linear for this window rather than asking again.
                    self.made = Some((w, Vec::new()));
                    self.job = None;
                    return true;
                }
            }
        }
        if self.made.as_ref().is_some_and(|(k, _)| *k == w) {
            return changed;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let vol = vol.clone();
        let samples = self.samples.clone();
        run(Box::new(move || {
            let s = samples.unwrap_or_else(|| std::sync::Arc::new(sample_values(&lock(&vol))));
            let c = curve_of(&s, w);
            let _ = tx.send((w, c, s));
        }));
        self.job = Some(rx);
        changed
    }
}

/// Whether `n` channels of `bytes` each fit the voxels' `budget` together; else what they
/// need and what there is, in free-RAM terms (the budget is 80 % of what is free).
pub(super) fn composite_fits(
    bytes: u64,
    n: usize,
    budget: u64,
) -> Result<(), specular_core::NeedsMemory> {
    let total = bytes.saturating_mul(n as u64);
    if total <= budget {
        return Ok(());
    }
    Err(specular_core::NeedsMemory {
        needed: total.saturating_mul(5) / 4,
        free: budget.saturating_mul(5) / 4,
    })
}

/// A channel's window as the user's window moves the first one's: the same shift (in
/// widths) and the same scale applied to the channel's own default.
pub(super) fn layer_window(first: WlWindow, user: Option<WlWindow>, own: WlWindow) -> WlWindow {
    let Some(u) = user else { return own };
    let w0 = first.width.max(f64::MIN_POSITIVE);
    WlWindow {
        center: own.center + (u.center - first.center) / w0 * own.width,
        width: (own.width * u.width / w0).max(f64::MIN_POSITIVE),
        shape: u.shape,
        invert: u.invert,
    }
}

struct Session {
    /// The image MPR was opened on (another one closes it).
    id: Option<ImageId>,
    load: Loading,
    tex: Option<VolumeTexture>,
    /// A colour composite of a microscopy stack's channels: the first channel's colour
    /// (white: grey, no composite) and the others.
    tint: [f32; 3],
    channels: Vec<Channel>,
    /// The first channel's name, and its histogram equalisation curve (a composite).
    name: String,
    tone: ToneCurve,
    /// The first channel's automatic stretch midtones balance (a composite).
    mid: f32,
    /// A composite's channels are stretched as the 2D view stretches them.
    stretch: specular_core::nd::Stretch,
    views: Option<Views>,
    /// The user's window (`None`: the file's default).
    window: Option<WlWindow>,
    layout: Option<Layout>,
    drag: Option<Drag>,
    /// The view last pointed at, and the pointer (logical).
    active: usize,
    pointer: Option<(f64, f64)>,
    /// Measuring: the first point placed, and where its press was.
    measuring: bool,
    draft: Option<V3>,
    draft_press: Option<(f64, f64)>,
    measures: Vec<[V3; 2]>,
    /// The lengths `Shift+M` cleared, for the toast's Undo.
    cleared: Option<Vec<[V3; 2]>>,
    /// `l` flips the orientation letters.
    labels_toggled: bool,
    /// The first of the slot's buttons shown when they do not all fit (More pages on).
    buttons_first: usize,
    placed: Placed,
    /// The texture could not be made: shown in the panel.
    gpu_error: Option<String>,
    /// Wheel turns not yet a whole slice.
    wheel: f64,
    vr: Vr3,
    /// Segmentations, SUV, fusion and TMTV.
    analysis: analysis::Analysis,
    /// Streamlines found beside a research volume, drawn near each plane.
    tracts: tracts::Tracts,
    /// A dense connectivity file: the seed whose row is shown, picked with the crosshair.
    dense: Option<dense::Dense>,
    /// Another seed's row is loading in place of the volume: the views stay as they are.
    reloading: bool,
    /// The view maximised (the others hidden, the controls beside it).
    maximised: Option<usize>,
    /// The folder chosen for the planes' export, then the export's result.
    planes_rx: Option<crossbeam_channel::Receiver<std::path::PathBuf>>,
    planes_done: Option<crossbeam_channel::Receiver<Result<(usize, String), String>>>,
    /// Curved MPR's curve and the dental layout's arch (one shown at a time, both kept).
    curved: Option<curved::Curved>,
    dental: Option<curved::Curved>,
    /// The volume's other centrelines (curved MPR) and the chosen one's place among them.
    lines: curve_app::Centrelines,
    curve_io: curve_app::Io,
    /// The profile shows Medical's workflow tools (the dental layout).
    medical_tools: bool,
}

impl Session {
    fn current_window(&self) -> WlWindow {
        self.window.unwrap_or_else(|| self.default_window())
    }

    /// Channel `ch`'s window: its own default moved as the user moves the first one's.
    fn channel_window(&self, ch: &Channel) -> Option<WlWindow> {
        let info = ch.load.info.as_ref()?;
        let own = info
            .window
            .unwrap_or_else(|| range_window(info.range, info.inverted));
        Some(layer_window(self.default_window(), self.window, own))
    }

    /// Makes the composite's histogram equalisation curves for the windows shown, once a
    /// channel's volume is read; true when one changed.
    fn refresh_curves(&mut self, run: &mut CurveRunner<'_>) -> bool {
        let mut changed = false;
        let w = self.current_window();
        if self.load.finished() {
            changed |= self.tone.refresh(&self.load.volume, w, run);
        }
        let windows: Vec<Option<WlWindow>> = self
            .channels
            .iter()
            .map(|ch| self.channel_window(ch))
            .collect();
        for (ch, w) in self.channels.iter_mut().zip(windows) {
            let Some(w) = w else { continue };
            if ch.load.finished() {
                changed |= ch.tone.refresh(&ch.load.volume, w, run);
            }
        }
        changed
    }

    /// Every shown channel's value under logical point `p` in view `v` (a composite): its
    /// name and value, the first channel first.
    fn channel_values(&self, v: usize, r: R, p: (f64, f64), scale: f64) -> Vec<(String, f32)> {
        let Some(views) = self.views.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(x) = self.value_at(v, r, p, scale) {
            out.push((self.name.clone(), x));
        }
        for ch in &self.channels {
            let Some(w) = self.channel_window(ch) else {
                continue;
            };
            let plane = views.plane(v, r, scale, w);
            let vol = lock(&ch.load.volume);
            if let Some(x) = reference(&vol, &plane, p.0 * scale, p.1 * scale) {
                out.push((ch.name.clone(), x));
            }
        }
        out
    }

    /// The file's window, else one over its values.
    fn default_window(&self) -> WlWindow {
        let info = self.load.info.as_ref();
        info.and_then(|i| i.window).unwrap_or_else(|| {
            let (r, inv) = info.map_or(((0.0, 1.0), false), |i| (i.range, i.inverted));
            range_window(r, inv)
        })
    }

    /// The view under logical point `p`.
    fn view_at(&self, p: (f64, f64)) -> Option<(usize, R)> {
        let l = self.layout?;
        (0..3)
            .find(|&v| draw::contains(l.views[v], p))
            .map(|v| (v, l.views[v]))
    }

    /// The value under logical point `p` in view `v`, as drawn (slab included).
    fn value_at(&self, v: usize, r: R, p: (f64, f64), scale: f64) -> Option<f32> {
        let views = self.views.as_ref()?;
        let plane = views.plane(v, r, scale, self.current_window());
        let vol = lock(&self.load.volume);
        reference(&vol, &plane, p.0 * scale, p.1 * scale)
    }

    fn unit(&self) -> Option<&str> {
        self.load.info.as_ref().and_then(|i| i.unit.as_deref())
    }

    /// The unit of lengths (mm; Ångström on density maps and cube files, µm or nm on
    /// microscopy stacks, voxels when the file gives no size).
    fn length(&self) -> Length {
        self.load.info.as_ref().map_or(Length::Mm, |i| i.length)
    }

    /// The volume's axes have patient directions (axial, coronal, sagittal and their
    /// letters); else the views are XY, XZ and YZ. Assumed while the volume opens.
    fn oriented(&self) -> bool {
        self.load.info.as_ref().is_none_or(|i| i.oriented)
    }

    /// A seismic volume's sections, when the volume is one.
    fn seismic(&self) -> Option<&specular_decode::science::Seismic> {
        self.load.info.as_ref()?.seismic.as_deref()
    }

    /// What the 3D view needs of the series: CT or not, and its default window.
    fn series(&self) -> Series {
        let info = self.load.info.as_ref();
        let w = info.and_then(|i| i.window).unwrap_or_else(|| {
            let (r, inv) = info.map_or(((0.0, 1.0), false), |i| (i.range, i.inverted));
            range_window(r, inv)
        });
        Series {
            hounsfield: self.unit() == Some("HU"),
            window: (w.center, w.width),
        }
    }

    /// The slot holds logical point `p`.
    fn in_slot(&self, p: (f64, f64)) -> bool {
        self.layout.is_some_and(|l| draw::contains(l.slot, p))
    }
}

/// Mm per voxel edge the crosshair moves with one key press.
const KEY_STEPS: f64 = 1.0;

/// A measurement whose ends are dragged closer than this (logical px) is removed.
const MEASURE_GONE_PX: f64 = 4.0;

/// The Undo of the "measurements cleared" toast (2D medical mode uses the next key).
pub(super) const MPR_CLEARED_KEY: u64 = 0x0ED1_0000_0000_0001;

impl App {
    /// The Undo of a "measurements cleared" toast: MPR's lengths, or the 2D image's
    /// measurements when it is still shown.
    pub(super) fn measurements_cleared_undo(&mut self, key: u64) -> bool {
        if key != MPR_CLEARED_KEY && key != super::medical::CLEARED_KEY {
            return false;
        }
        self.chrome.dismiss_undo(key);
        if self.last_undo == Some(key) {
            self.last_undo = None;
        }
        if key == MPR_CLEARED_KEY {
            if let Some(s) = self.mpr.s.as_mut()
                && let Some(m) = s.cleared.take()
            {
                s.measures.extend(m);
                self.mpr.rev += 1;
            }
        } else {
            self.medical_restore_cleared();
        }
        self.redraw = true;
        true
    }

    /// MPR is open.
    pub(super) fn mpr_active(&self) -> bool {
        self.mpr.s.is_some()
    }

    /// A navigation device turned or zoomed the 3D view (`app/devices.rs`): `zoom` a factor,
    /// `orbit` in radians. False when MPR is not open.
    pub(super) fn mpr_device_nav(&mut self, pan: (f64, f64), zoom: f64, orbit: (f64, f64)) -> bool {
        let scale = f64::from(self.dl.scale.max(0.1));
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        // Physical pixels to logical; the content moves the other way, as in the image view.
        let (dx, dy) = (pan.0 / scale, pan.1 / scale);
        let layout = s.layout;
        if s.active < 3 {
            // A 2D pane pans and zooms (about its centre) like the image view.
            let v = s.active;
            if let (Some(views), Some(l)) = (s.views.as_mut(), layout) {
                device_pane(views, v, l.views[v], (dx, dy), zoom);
            }
        } else {
            if orbit != (0.0, 0.0) {
                s.vr.orbit.orbit(orbit.0, orbit.1);
            }
            if (dx, dy) != (0.0, 0.0)
                && let (Some(views), Some(l)) = (s.views.as_ref(), layout)
            {
                let mm = volume3d::mm_per_px(views, &s.vr.orbit, s.placed.view3d.unwrap_or(l.slot));
                s.vr.orbit.pan[0] += dx * mm;
                s.vr.orbit.pan[1] += dy * mm;
            }
            if zoom != 1.0 {
                s.vr.orbit.zoom_by(zoom);
            }
            s.vr.nudge();
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// The device sticks drive MPR's 3D view (orbit) rather than a 2D pane (pan): the
    /// pane last pointed at is the 3D one.
    pub(super) fn mpr_device_3d(&self) -> bool {
        self.mpr.s.as_ref().is_some_and(|s| s.active >= 3)
    }

    /// Lays a label volume over the volume (any volume: MPR's tier 1); false when it does
    /// not overlap the volume.
    pub(super) fn mpr_add_labels(
        &mut self,
        labels: specular_dicom::labels::Labels,
    ) -> Result<bool, String> {
        let Some(s) = self.mpr.s.as_mut() else {
            return Ok(false);
        };
        let Some(gfx) = self.gfx.as_ref() else {
            return Err(String::new());
        };
        let fits = specular_labelmap::overlaps(&labels, &lock(&s.load.volume).geometry);
        if !fits {
            return Ok(false);
        }
        let textures = analysis::upload(&gfx.gpu, &labels)?;
        let n = labels.segments.len();
        s.analysis.sets.push(analysis::Set {
            labels,
            textures,
            hidden: vec![false; n],
        });
        s.analysis.hide = false;
        self.mpr.rev += 1;
        Ok(true)
    }

    /// The scene revision for the redraw key (0 when closed).
    pub(super) fn mpr_rev(&self) -> u64 {
        if self.mpr.s.is_some() {
            self.mpr.rev.max(1)
        } else {
            0
        }
    }

    /// Opens MPR on the current image's series (or its frames).
    fn mpr_open(&mut self) {
        let now = self.now();
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let Some(gfx) = self.gfx.as_ref() else { return };
        // A research volume (NIfTI, Analyze, NRRD, MetaImage): the volume (time point) the
        // shown slice belongs to.
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone());
        let research = session.as_ref().and_then(|s| {
            let volumes = s.research_volume()?.volumes.max(1) as u32;
            let slices = (s.page_count() / volumes).max(1);
            Some((self.medical_index() / slices) as usize)
        });
        // A CIFTI file's volume (made by its reader, not in the file as a plain volume),
        // and of a dense connectivity file the rows of the other seeds.
        let prepared = session
            .as_ref()
            .and_then(|s| s.prepared_volume())
            .map(|v| load::Prepared::Volume(std::sync::Arc::new(v)));
        let dense = session
            .as_ref()
            .and_then(|s| s.neuro()?.connectivity.clone())
            .filter(|c| c.0.has_voxel_seeds())
            .map(|c| dense::Dense::new(c.0, path.clone(), 0));
        // A 3-D seismic volume (SEG-Y, Seismic Unix): time, inline and crossline sections.
        let seismic = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone())
            .and_then(|s| s.science()?.seismic.clone())
            .filter(|s| s.is_volume());
        // A microscopy z-stack or a science cube, at its channel and time point shown; with
        // channels composited, every channel shown (a volume each) when they fit the memory
        // budget together, else the first alone and the panel says why.
        let budget = self.mpr_budget();
        let mut composite = self.science_volume_channels();
        let mut tint = [1.0f32; 3];
        let mut first_name = String::new();
        let mut first_mid = 0.5f32;
        let science;
        let mut extra = Vec::new();
        if composite.len() >= 2
            && let Some(sess) = session.clone()
        {
            let (st, ..) = &composite[0];
            let bytes = u64::from(st.width) * u64::from(st.height) * st.depth as u64 * 4;
            let n = composite.len();
            match composite_fits(bytes, n, budget) {
                Ok(()) => {
                    let (mut first, t, name) = composite.remove(0);
                    first.notes.push(t!("mpr-composite", count = n as i64));
                    tint = t;
                    first_name = name;
                    first_mid = first.mid;
                    science = Some((sess.clone(), std::sync::Arc::new(first)));
                    extra = composite
                        .into_iter()
                        .map(|(st, t, name)| {
                            let mid = st.mid;
                            (
                                Source::Science(sess.clone(), std::sync::Arc::new(st)),
                                t,
                                name,
                                mid,
                            )
                        })
                        .collect();
                }
                Err(need) => {
                    science = self.science_volume().map(|mut st| {
                        st.notes.push(t!(
                            "mpr-composite-memory",
                            reason = crate::error_text::needs_memory_reason(need)
                        ));
                        (sess.clone(), std::sync::Arc::new(st))
                    });
                }
            }
        } else {
            science = session
                .clone()
                .zip(self.science_volume())
                .map(|(s, st)| (s, std::sync::Arc::new(st)));
        }
        // Each channel's share of the budget.
        let budget = budget / (1 + extra.len() as u64);
        let source = match (research, seismic, science, self.series_set()) {
            (Some(volume), ..) => Source::Research(path, volume, prepared),
            (None, Some(s), ..) => Source::Seismic(path, s),
            (None, None, Some((session, stack)), _) => Source::Science(session, stack),
            (None, None, None, Some(s)) if s.items.iter().any(|i| i.key == path) => {
                Source::Series(s.clone())
            }
            _ => Source::File(path),
        };
        // Tractograms beside a brain volume, read in the background.
        let tracts = match &source {
            Source::Research(p, ..) => {
                let loader = self.loader.clone();
                let wake = self.loader.waker();
                tracts::Tracts::start(
                    p,
                    move |job| loader.background("mpr-tracts", move |_| job()),
                    move || wake(),
                )
            }
            _ => tracts::Tracts::default(),
        };
        let (first, times) = match &source {
            Source::Series(s) => (
                s.items.first().map(|i| i.key.clone()),
                s.items
                    .iter()
                    .filter_map(|i| i.instance.acquisition_time)
                    .collect(),
            ),
            Source::File(p) => (Some(p.clone()), Vec::new()),
            // Segmentations and curves come with DICOM series, not research volumes.
            Source::Research(..) | Source::Seismic(..) | Source::Science(..) => (None, Vec::new()),
        };
        let analysis = match first {
            Some(f) => analysis::Analysis::start(f, times, self.loader.waker()),
            None => analysis::Analysis::default(),
        };
        let load = Loading::start(
            source,
            self.loader.pool(),
            self.loader.waker(),
            max_dimension(&gfx.gpu),
            budget,
        );
        let channels = extra
            .into_iter()
            .map(|(src, t, name, mid)| Channel {
                load: Loading::start(
                    src,
                    self.loader.pool(),
                    self.loader.waker(),
                    max_dimension(&gfx.gpu),
                    budget,
                ),
                tex: None,
                tint: t,
                name,
                tone: ToneCurve::default(),
                mid,
                failed: false,
            })
            .collect();
        // Centrelines saved beside the volume (`<name>-curve….json`), read off the UI
        // thread; added once the volume's views are there.
        let mut curve_io = curve_app::Io::default();
        if let Some(p) = self.current.and_then(|i| self.entries.get(i)).cloned() {
            let (tx, rx) = crossbeam_channel::bounded(1);
            self.loader.background("mpr-sidecars", move |l| {
                let _ = tx.send(curve_app::read_sidecars(&p));
                l.waker()();
            });
            curve_io.sidecars_rx = Some(rx);
        }
        self.mpr.s = Some(Box::new(Session {
            id: self.current_id(),
            load,
            tex: None,
            tint,
            channels,
            name: first_name,
            tone: ToneCurve::default(),
            mid: first_mid,
            stretch: specular_core::nd::Stretch::Linear,
            views: None,
            window: None,
            layout: None,
            drag: None,
            active: 0,
            pointer: None,
            measuring: false,
            draft: None,
            draft_press: None,
            measures: Vec::new(),
            cleared: None,
            labels_toggled: false,
            buttons_first: 0,
            placed: Placed::default(),
            gpu_error: None,
            wheel: 0.0,
            vr: Vr3::default(),
            analysis,
            tracts,
            dense,
            reloading: false,
            maximised: None,
            planes_rx: None,
            planes_done: None,
            curved: None,
            dental: None,
            lines: Default::default(),
            curve_io,
            medical_tools: false,
        }));
        self.mpr.rev += 1;
        self.chrome.toast(t!("mpr-opening"), None, now);
        self.redraw = true;
    }

    /// Loads dense connectivity seed `seed`'s row in place of the shown volume, keeping the
    /// views and starting from the new row's own window.
    fn mpr_show_seed(&mut self, seed: u32) {
        let budget = self.mpr_budget();
        let Some(gfx) = self.gfx.as_ref() else { return };
        let max_dim = max_dimension(&gfx.gpu);
        let (pool, wake) = (self.loader.pool(), self.loader.waker());
        let Some(s) = self.mpr.s.as_mut() else { return };
        let Some(d) = s.dense.as_mut() else { return };
        let source = d.show(seed);
        s.load.cancel();
        s.load = Loading::start(source, pool, wake, max_dim, budget);
        s.window = None;
        s.reloading = true;
        self.mpr.rev += 1;
        self.redraw = true;
    }

    /// Bytes the voxels may take: they live in RAM and on the GPU, so at most the RAM
    /// cache's budget, half of the memory free now and what the shared memory budget (80 %
    /// of the free RAM) has left. The opener reserves what it plans from that budget.
    fn mpr_budget(&self) -> u64 {
        let ram = match self.settings.performance.ram_cache {
            0 => self.hw.ram_cache_budget(),
            mib => (mib as u64) << 20,
        };
        let ram = specular_core::available_memory().map_or(ram, |a| ram.min(a / 2));
        let ram = specular_core::budget::free().map_or(ram, |f| ram.min(f));
        ram.max(64 << 20)
    }

    /// The volume MPR would open on (the action's state), whatever the profile.
    pub(super) fn mpr_volume_kind(&self) -> Option<VolumeKind> {
        if self.gallery_active() {
            return None;
        }
        let path = self.current.and_then(|i| self.entries.get(i))?;
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone());
        let facts = VolumeFacts {
            medical_frames: self.medical_state(),
            in_series: self
                .series_set()
                .is_some_and(|s| s.items.iter().any(|i| &i.key == path)),
            research: session
                .as_ref()
                .and_then(|s| s.research_volume())
                .map(|f| [0, 1, 2].map(|k| f.dims.get(k).copied().unwrap_or(1))),
            seismic: session
                .as_ref()
                .and_then(|s| s.science()?.seismic.as_ref().map(|s| s.is_volume()))
                .unwrap_or(false),
            science: self.science_is_volume(),
        };
        volume_kind(&facts)
    }

    /// Closes MPR (cancels the loading and frees the volume).
    pub(super) fn mpr_close(&mut self) {
        if let Some(s) = self.mpr.s.take() {
            s.load.cancel();
            if let Some(f) = &s.analysis.fusion {
                f.load.cancel();
            }
            self.mpr.rev += 1;
            self.scene = None;
            self.redraw = true;
        }
    }

    /// Takes the loaders' news, uploads new slices, follows the current image.
    pub(super) fn mpr_tick(&mut self) {
        if let Some(t) = self.mpr.show
            && (self.series_set().is_some() || t.elapsed().as_secs_f64() > 3.0)
        {
            self.mpr.show = None;
            self.mpr_open();
        }
        if self.mpr.s.is_none() {
            return;
        }
        self.mpr_tick_3d();
        self.mpr_curve_tick();
        let now = self.now();
        let id = self.current_id();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if s.id != id || self.gallery.active {
            self.mpr_close();
            return;
        }
        if s.tracts.poll() {
            self.mpr.rev += 1;
            self.redraw = true;
        }
        let polled = s.load.poll();
        if let Some(e) = polled.refused {
            self.mpr_close();
            if !e.is_empty() {
                self.chrome.toast(t!("mpr-refused", reason = e), None, now);
            }
            return;
        }
        let Some(gfx) = self.gfx.as_ref() else { return };
        if s.analysis.poll(&gfx.gpu) {
            self.mpr.rev += 1;
            self.redraw = true;
            // `--show dvh`: the dose's histograms once the search found them.
            if std::mem::take(&mut self.report.dvh_on_dose)
                && let Some(sheet) = s
                    .analysis
                    .dose
                    .as_ref()
                    .and_then(|d| dose::dvh_sheet(&d.found))
            {
                self.chrome.show_plot(sheet);
            }
        }
        let (changed, msg) = s.analysis.tick_fusion(&gfx.gpu);
        if changed {
            self.mpr.rev += 1;
            self.redraw = true;
        }
        if let Some(m) = msg {
            self.chrome.toast(m, None, now);
        }
        if polled.opened {
            let vol = lock(&s.load.volume);
            // Another seed's row has the same geometry: the views stay.
            if !std::mem::take(&mut s.reloading) || s.views.is_none() {
                let mut views = Views::new(&vol.geometry);
                views.anatomy = s
                    .load
                    .info
                    .as_ref()
                    .map_or_else(Default::default, |i| i.anatomy);
                s.views = Some(views);
            }
            match VolumeTexture::new(&gfx.gpu, &vol) {
                Ok(t) => s.tex = Some(t),
                Err(e) => {
                    drop(vol);
                    self.mpr_close();
                    self.chrome.toast(
                        t!("mpr-no-gpu", reason = crate::error_text::text_reason(&e)),
                        None,
                        now,
                    );
                    return;
                }
            }
        }
        for ch in &mut s.channels {
            let p = ch.load.poll();
            ch.failed |= p.refused.is_some();
            if p.opened {
                match VolumeTexture::new(&gfx.gpu, &lock(&ch.load.volume)) {
                    Ok(t) => ch.tex = Some(t),
                    Err(e) => s.gpu_error = Some(e),
                }
            }
            if ch.load.dirty
                && let Some(tex) = ch.tex.as_mut()
            {
                ch.load.dirty = false;
                match tex.sync(&gfx.gpu, &lock(&ch.load.volume)) {
                    Ok(true) => {
                        self.mpr.rev += 1;
                        self.redraw = true;
                    }
                    Ok(false) => {}
                    Err(e) => s.gpu_error = Some(e),
                }
            }
        }
        // A channel that could not be read leaves the composite.
        s.channels.retain(|ch| !ch.failed);
        // A composite follows the 2D view's stretch, each channel through its own curve.
        let stretch = if s.channels.is_empty() {
            specular_core::nd::Stretch::Linear
        } else {
            self.science.stretch_now()
        };
        if s.stretch != stretch {
            s.stretch = stretch;
            self.mpr.rev += 1;
            self.redraw = true;
        }
        // The curves are made off the UI thread (a million values a channel).
        let loader = &self.loader;
        let mut run = |job: Box<dyn FnOnce() + Send>| {
            loader.background("mpr-histeq", move |l| {
                job();
                l.waker()();
            });
        };
        if stretch == specular_core::nd::Stretch::HistEq && s.refresh_curves(&mut run) {
            self.mpr.rev += 1;
            self.redraw = true;
        }
        if s.load.dirty
            && let Some(tex) = s.tex.as_mut()
        {
            s.load.dirty = false;
            let vol = lock(&s.load.volume);
            match tex.sync(&gfx.gpu, &vol) {
                Ok(true) => {
                    self.mpr.rev += 1;
                    self.redraw = true;
                }
                Ok(false) => {}
                Err(e) => s.gpu_error = Some(e),
            }
            if s.load.finished() && s.load.complete.is_none() {
                let t = s.load.started.elapsed();
                s.load.complete = Some(t);
                specular_log::info!(
                    target: specular_log::DECODE,
                    "mpr: {} slices loaded in {:.0} ms ({} failed), {} MiB on the GPU",
                    vol.geometry.slices(),
                    t.as_secs_f64() * 1e3,
                    s.load.failed,
                    tex.bytes() >> 20
                );
                if let Some(e) = s.load.error.clone() {
                    self.chrome.toast(t!("mpr-failed", reason = e), None, now);
                }
                // The empty-space grid, off the UI thread.
                let volume = s.load.volume.clone();
                let (tx, rx) = crossbeam_channel::bounded(1);
                self.loader.background("mpr-bricks", move |l| {
                    let b = Bricks::compute(&lock(&volume));
                    let _ = tx.send(b);
                    l.waker()();
                });
                s.vr.bricks_rx = Some(rx);
            }
            self.redraw = true;
        }
    }

    /// MPR's actions, and while it is open the viewer's and medical mode's that would act
    /// on the image under it. True when `a` was taken.
    pub(super) fn mpr_action(&mut self, a: Action) -> bool {
        if a == Action::Mpr {
            if self.mpr_active() {
                self.mpr_close();
            } else {
                self.mpr_open();
            }
            return true;
        }
        let now = self.now();
        let Some(_) = self.mpr.s.as_ref() else {
            return matches!(
                a,
                Action::MprSlab
                    | Action::MprTractSlab
                    | Action::MprPreset
                    | Action::MprRender
                    | Action::MprClip
                    | Action::MprExportStl
                    | Action::MprMaximise
                    | Action::MprExportPlanes
                    | Action::MprCurve
                    | Action::MprCurveLayout
                    | Action::MprCurveSave
                    | Action::MprCurveLoad
                    | Action::MprExportSegments
                    | Action::MprDental
                    | Action::MprNerve
            );
        };
        if self.mpr_curve_action(a) {
            return true;
        }
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        if a == Action::MprExportStl {
            self.mpr_export_stl();
            return true;
        }
        if a == Action::MprExportPlanes {
            self.mpr_export_planes();
            return true;
        }
        if a == Action::MprMaximise {
            if let Some(c) = curve_app::shown(&mut s.curved, &mut s.dental) {
                c.shown = false;
            }
            s.maximised = match s.maximised {
                Some(_) => None,
                None => Some(s.active.min(2)),
            };
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        if s.active == 3 && matches!(a, Action::ZoomIn | Action::ZoomOut | Action::ZoomFit) {
            match a {
                Action::ZoomIn => s.vr.orbit.zoom_by(1.25),
                Action::ZoomOut => s.vr.orbit.zoom_by(0.8),
                _ => s.vr.orbit = Default::default(),
            }
            s.vr.nudge();
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        let v = s.active.min(2);
        if s.analysis.nudge
            && let Some(views) = s.views.as_ref()
        {
            // The fused series moves by a millimetre, or turns by a degree.
            let (d, turn) = match a {
                Action::NextImage | Action::PanRight => ([1.0, 0.0, 0.0], 0.0),
                Action::PrevImage | Action::PanLeft => ([-1.0, 0.0, 0.0], 0.0),
                Action::PanDown => ([0.0, 1.0, 0.0], 0.0),
                Action::PanUp => ([0.0, -1.0, 0.0], 0.0),
                Action::PanPageUp => ([0.0, 0.0, 1.0], 0.0),
                Action::PanPageDown => ([0.0, 0.0, -1.0], 0.0),
                Action::PanPageLeft | Action::RotateLeft => ([0.0; 3], -1.0),
                Action::PanPageRight | Action::RotateRight => ([0.0; 3], 1.0),
                _ => ([0.0; 3], f64::NAN),
            };
            if !turn.is_nan() {
                s.analysis.nudge(views, v, d, turn);
                self.mpr.rev += 1;
                self.redraw = true;
                return true;
            }
        }
        if let Some(msg) = self.mpr_analysis_action(a) {
            if let Some(m) = msg {
                self.chrome.toast(m, None, now);
            }
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        let r = s.layout.map(|l| l.views[v]);
        let series = s.series();
        match a {
            Action::MprPreset => {
                let p = s.vr.preset(series.hounsfield).next(series.hounsfield);
                s.vr.preset = Some(p);
                s.vr.tf_window = None;
                self.chrome
                    .toast(t!("vr-preset", preset = p.id()), None, now);
            }
            Action::MprRender => {
                s.vr.mode = s.vr.mode.next();
                let m = s.vr.mode.id();
                self.chrome.toast(t!("vr-mode", mode = m), None, now);
            }
            Action::MprClip => {
                s.vr.clip = s.vr.clip.next();
                let c = s.vr.clip.id();
                self.chrome.toast(t!("vr-clip", clip = c), None, now);
            }
            // The arrows orbit the 3D view (the image under MPR stays).
            Action::PanPageLeft
            | Action::PanPageRight
            | Action::PanPageUp
            | Action::PanPageDown
            | Action::PanUp
            | Action::PanDown => {
                let (dx, dy) = match a {
                    Action::PanPageLeft => (-ORBIT_KEY, 0.0),
                    Action::PanPageRight => (ORBIT_KEY, 0.0),
                    Action::PanPageUp | Action::PanUp => (0.0, -ORBIT_KEY),
                    _ => (0.0, ORBIT_KEY),
                };
                s.vr.orbit.orbit(dx, dy);
                s.vr.nudge();
                s.active = 3;
            }
            Action::MprTractSlab => {
                let msg = match s.tracts.files.is_empty() {
                    true => t!("mpr-tracts-none"),
                    false => {
                        s.tracts.step();
                        s.tracts.text().unwrap_or_default()
                    }
                };
                self.chrome.toast(msg, None, now);
            }
            Action::MprSlab => {
                let length = s.length();
                if let Some(views) = s.views.as_mut() {
                    views.next_slab();
                    let msg = draw::slab_text(views, length);
                    self.chrome.toast(msg, None, now);
                }
            }
            Action::NextSlice | Action::PrevSlice => {
                let d = if a == Action::NextSlice { 1.0 } else { -1.0 };
                if let Some(views) = s.views.as_mut() {
                    views.scroll(v, d * KEY_STEPS);
                }
            }
            Action::ZoomIn | Action::ZoomOut | Action::ZoomFit => {
                if let (Some(views), Some(r)) = (s.views.as_mut(), r) {
                    let at = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                    match a {
                        Action::ZoomIn => views.zoom_at(v, r, 1.25, at),
                        Action::ZoomOut => views.zoom_at(v, r, 0.8, at),
                        _ => {
                            views.frames[v].zoom = 1.0;
                            views.frames[v].pan = [0.0; 2];
                        }
                    }
                }
            }
            Action::InvertGrey => {
                let mut w = s.current_window();
                w.invert = !w.invert;
                s.window = Some(w);
            }
            Action::WindowPreset(0) => {
                s.window = None;
                self.chrome.toast(t!("palette-window-default"), None, now);
            }
            Action::WindowPreset(9) => {
                let inv = s.current_window().invert;
                let range = s.load.info.as_ref().map_or((0.0, 1.0), |i| i.range);
                s.window = Some(range_window(range, inv));
                self.chrome
                    .toast(t!("palette-window-full-range"), None, now);
            }
            Action::WindowPreset(n @ 1..=3) if s.analysis.preset_suv().is_some() => {
                // PET: SUV 0–5, 0–10, 0–20 (on the fused series when it is the PET).
                let (f, fused) = s.analysis.preset_suv().expect("checked");
                let upper = analysis::SUV_PRESETS[n as usize - 1];
                if fused {
                    if let Some(fu) = s.analysis.fusion.as_mut() {
                        fu.window = Some(analysis::suv_window(upper, f, false));
                    }
                } else {
                    let invert = s.current_window().invert;
                    s.window = Some(analysis::suv_window(upper, f, invert));
                }
                self.chrome
                    .toast(t!("suv-preset", upper = number(upper)), None, now);
            }
            Action::WindowPreset(n) => {
                let Some(&(_, width, level)) = CT_PRESETS.get(n as usize - 1) else {
                    return true;
                };
                if s.unit() != Some("HU") {
                    self.chrome.toast(t!("medical-preset-not-ct"), None, now);
                    return true;
                }
                let invert = s.current_window().invert;
                s.window = Some(WlWindow {
                    center: level,
                    width,
                    shape: VoiShape::Linear,
                    invert,
                });
                let msg = a.name();
                self.chrome.toast(msg, None, now);
            }
            Action::MeasureLength => {
                s.measuring = !s.measuring;
                s.draft = None;
                let msg = if s.measuring {
                    t!("measure-hint-length")
                } else {
                    t!("measure-off")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::ClearMeasurements => {
                s.draft = None;
                if !s.measures.is_empty() {
                    s.cleared = Some(std::mem::take(&mut s.measures));
                    self.chrome
                        .toast(t!("measure-cleared"), Some(MPR_CLEARED_KEY), now);
                    self.last_undo = Some(MPR_CLEARED_KEY);
                }
            }
            // Only lengths are measured on the planes: say so rather than do nothing.
            Action::MeasureAngle | Action::MeasureEllipse | Action::MeasureRectangle => {
                self.chrome.toast(t!("mpr-measure-lengths-only"), None, now);
                return true;
            }
            Action::ToggleOrientationLabels => {
                s.labels_toggled = !s.labels_toggled;
            }
            // What would act on the image under MPR, or leave it.
            Action::WindowLevelTool
            | Action::Cine
            | Action::ToggleOverlays
            | Action::NextPresentationState
            | Action::ZoomFill
            | Action::ZoomFitWidth
            | Action::ZoomFitHeight
            | Action::ZoomTo(_)
            | Action::RotateLeft
            | Action::RotateRight
            | Action::FlipHorizontal
            | Action::FlipVertical
            | Action::PanLeft
            | Action::PanRight => {}
            _ => return false,
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// Esc while MPR is open: the measurement being placed, the measuring tool, TMTV's
    /// tool, nudging, then MPR.
    pub(super) fn mpr_back(&mut self) -> bool {
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        if s.draft.take().is_some()
            || std::mem::take(&mut s.measuring)
            || s.analysis.tmtv.drag.take().is_some()
            || s.analysis.tmtv.shape.take().is_some()
            || std::mem::take(&mut s.analysis.nudge)
        {
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        if self.mpr_curve_back() {
            return true;
        }
        self.mpr_close();
        true
    }

    /// A button press or release at physical point `p`; true when MPR took it.
    pub(super) fn mpr_button(
        &mut self,
        p: (f32, f32),
        down: bool,
        b: specular_widgets::Button,
    ) -> bool {
        use specular_widgets::Button;
        let scale = f64::from(self.dl.scale.max(0.1));
        let shift = self.mods.shift_key();
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        if let Some(taken) = self.mpr_curve_button(lp, down, b) {
            return taken;
        }
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        if !down {
            if b == Button::Primary
                && let Some((v, a, _)) = s.analysis.tmtv.drag.take()
            {
                if let (Some(views), Some(l)) = (s.views.as_ref(), s.layout)
                    && let Some(region) = s.analysis.tmtv_region(views, v, l.views[v], a, lp)
                    && let Some(gfx) = self.gfx.as_ref()
                {
                    let vol = lock(&s.load.volume);
                    s.analysis.tmtv_measure(&gfx.gpu, &vol, region);
                }
                self.mpr.rev += 1;
                self.redraw = true;
                return true;
            }
            let was = s.drag.take();
            if let (Some(Drag::Measure(v, i, _)), Some(views), Some(l)) =
                (was, s.views.as_ref(), s.layout)
                && let Some(m) = s.measures.get(i)
            {
                // An end dropped on the other one removes the measurement.
                let (a, b) = (
                    views.to_screen(v, l.views[v], m[0]),
                    views.to_screen(v, l.views[v], m[1]),
                );
                if (a.0 - b.0).hypot(a.1 - b.1) < MEASURE_GONE_PX {
                    s.measures.remove(i);
                    let now = self.now();
                    self.chrome.toast(t!("mpr-measure-removed"), None, now);
                }
                self.mpr.rev += 1;
                self.redraw = true;
                return true;
            }
            if s.vr.dragging {
                // Refines at rest.
                s.vr.dragging = false;
                self.mpr.rev += 1;
                self.redraw = true;
            }
            if b == Button::Primary
                && let Some(start) = s.draft_press.take()
            {
                // Press–drag–release measures in one go.
                if (lp.0 - start.0).hypot(lp.1 - start.1) > 4.0
                    && let Some((v, r)) = s.view_at(start)
                    && let (Some(a), Some(views)) = (s.draft, s.views.as_ref())
                {
                    let q = views.to_patient(v, r, lp);
                    s.measures.push([a, q]);
                    s.draft = None;
                }
                self.mpr.rev += 1;
                self.redraw = true;
            }
            // Letting go of the crosshair on another seed's voxel shows its row.
            let seed = match (was, s.dense.as_ref(), s.views.as_ref()) {
                (Some(Drag::Move(_)), Some(d), Some(views)) => d.pick(views.c),
                _ => None,
            };
            if let Some(seed) = seed {
                self.mpr_show_seed(seed);
            }
            return was.is_some();
        }
        if let Some((c, _, _)) = s
            .placed
            .controls
            .iter()
            .find(|(_, r, _)| b == Button::Primary && r.contains((lp.0 as f32, lp.1 as f32)))
            .copied()
        {
            self.mpr_control(c);
            return true;
        }
        let Some((v, r)) = s.view_at(lp) else {
            // The fourth slot: the 3D view.
            if !s.in_slot(lp) {
                return false;
            }
            s.active = 3;
            s.drag = match b {
                Button::Secondary if s.vr.mode.composite() => {
                    let series = s.series();
                    let w = s.vr.tf_window(series.hounsfield, series.window);
                    Some(Drag::Transfer(lp, w))
                }
                Button::Secondary => Some(Drag::Window(lp, s.current_window())),
                Button::Middle => Some(Drag::Pan3d(lp)),
                Button::Primary if shift => Some(Drag::Pan3d(lp)),
                Button::Primary => Some(Drag::Orbit(lp)),
            };
            s.vr.dragging = s.drag.is_some();
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        };
        s.active = v;
        let Some(views) = s.views.as_mut() else {
            return true;
        };
        let hit = HANDLE_HIT * f64::from(self.chrome.text_scale);
        // A measurement's end drawn under the pointer is moved (not while placing one).
        let end = (s.draft.is_none() && s.analysis.tmtv.shape.is_none())
            .then(|| views.measure_end_at(v, r, &s.measures, lp, hit))
            .flatten();
        s.drag = match b {
            Button::Secondary => Some(Drag::Window(lp, s.current_window())),
            Button::Middle => Some(Drag::Pan(v, lp)),
            Button::Primary if shift => Some(Drag::Pan(v, lp)),
            Button::Primary if end.is_some() => end.map(|(i, e)| Drag::Measure(v, i, e)),
            Button::Primary if s.analysis.tmtv.shape.is_some() => {
                s.analysis.tmtv.drag = Some((v, views.to_patient(v, r, lp), lp));
                None
            }
            Button::Primary if s.measuring => {
                let q = views.to_patient(v, r, lp);
                match s.draft.take() {
                    Some(a) => s.measures.push([a, q]),
                    None => {
                        s.draft = Some(q);
                        s.draft_press = Some(lp);
                    }
                }
                None
            }
            Button::Primary => match views.grab(v, r, lp, hit) {
                Grab::Rotate(angle) => Some(Drag::Rotate(v, angle)),
                Grab::Move => {
                    views.move_to(v, r, lp);
                    Some(Drag::Move(v))
                }
            },
        };
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    fn mpr_control(&mut self, c: Control) {
        if self.mpr_curve_control(c) {
            return;
        }
        let now = self.now();
        let Some(s) = self.mpr.s.as_mut() else { return };
        match c {
            Control::Close => {
                self.mpr_close();
                return;
            }
            Control::Reset => {
                if let Some(v) = s.views.as_mut() {
                    v.reset();
                }
                s.window = None;
            }
            Control::Slab(m) => {
                if let Some(v) = s.views.as_mut() {
                    v.set_slab(m);
                }
            }
            Control::Segment(i, k) => s.analysis.toggle(i, k),
            Control::Thinner | Control::Thicker => {
                let length = s.length();
                if let Some(v) = s.views.as_mut() {
                    v.step_thickness(c == Control::Thicker);
                    let msg = draw::slab_text(v, length);
                    self.chrome.toast(msg, None, now);
                }
            }
            Control::Preset
            | Control::Render
            | Control::Clip
            | Control::Export
            | Control::Maximise
            | Control::ExportPlanes => {
                let a = match c {
                    Control::Preset => Action::MprPreset,
                    Control::Render => Action::MprRender,
                    Control::Clip => Action::MprClip,
                    Control::Maximise => Action::MprMaximise,
                    Control::ExportPlanes => Action::MprExportPlanes,
                    _ => Action::MprExportStl,
                };
                self.mpr_action(a);
                return;
            }
            Control::Smooth => s.vr.smooth = !s.vr.smooth,
            Control::More(next) => s.buttons_first = next,
            _ => {}
        }
        self.mpr.rev += 1;
        self.redraw = true;
    }

    /// Pointer motion at physical `p`: drives a drag and the readouts. True while MPR is
    /// open (the image under it does not take the pointer).
    pub(super) fn mpr_motion(&mut self, p: (f32, f32)) -> bool {
        let scale = f64::from(self.dl.scale.max(0.1));
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        s.pointer = Some(lp);
        if let Some(d) = s.analysis.tmtv.drag.as_mut() {
            d.2 = lp;
        }
        // A press in a centreline's name field drags a selection.
        if s.rename_drag(lp.0 as f32) {
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        if s.drag.is_none() {
            if let Some((v, _)) = s.view_at(lp) {
                s.active = v;
            } else if s.in_slot(lp) {
                s.active = 3;
            }
        }
        if matches!(s.drag, Some(Drag::CurvePoint(_) | Drag::CprPan(_))) {
            self.mpr_curve_motion(lp);
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        let layout = s.layout;
        let range = s.load.info.as_ref().map_or(1.0, |i| i.range.1 - i.range.0);
        match (s.drag, s.views.as_mut(), layout) {
            (Some(Drag::Move(v)), Some(views), Some(l)) => views.move_to(v, l.views[v], lp),
            (Some(Drag::Rotate(v, last)), Some(views), Some(l)) => {
                let a = views.angle(v, l.views[v], lp);
                let mut d = a - last;
                if d > std::f64::consts::PI {
                    d -= std::f64::consts::TAU;
                } else if d < -std::f64::consts::PI {
                    d += std::f64::consts::TAU;
                }
                views.rotate(v, d);
                s.drag = Some(Drag::Rotate(v, a));
            }
            (Some(Drag::Pan(v, last)), Some(views), Some(l)) => {
                views.pan_by(v, l.views[v], lp.0 - last.0, lp.1 - last.1);
                s.drag = Some(Drag::Pan(v, lp));
            }
            (Some(Drag::Window(start, w)), _, _) => {
                s.window = Some(dragged(w, lp.0 - start.0, lp.1 - start.1, range));
            }
            (Some(Drag::Measure(v, i, e)), Some(views), Some(l)) => {
                if let Some(m) = s.measures.get_mut(i) {
                    m[e] = views.to_patient(v, l.views[v], lp);
                }
            }
            (Some(Drag::Orbit(last)), _, _) => {
                let (dx, dy) = (lp.0 - last.0, lp.1 - last.1);
                s.vr.orbit.orbit(dx * ORBIT_PER_PX, dy * ORBIT_PER_PX);
                s.drag = Some(Drag::Orbit(lp));
            }
            (Some(Drag::Pan3d(last)), Some(views), Some(l)) => {
                let mm = volume3d::mm_per_px(views, &s.vr.orbit, s.placed.view3d.unwrap_or(l.slot));
                let o = &mut s.vr.orbit;
                o.pan[0] -= (lp.0 - last.0) * mm;
                o.pan[1] -= (lp.1 - last.1) * mm;
                s.drag = Some(Drag::Pan3d(lp));
            }
            (Some(Drag::Transfer(start, (c, w))), _, _) => {
                let from = WlWindow {
                    center: c,
                    width: w,
                    shape: VoiShape::Linear,
                    invert: false,
                };
                let to = dragged(from, lp.0 - start.0, lp.1 - start.1, range);
                s.vr.tf_window = Some((to.center, to.width));
            }
            _ => {}
        }
        // The readouts follow the pointer.
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// A wheel turn at physical `p`: steps the view's plane, or zooms with Ctrl. True when
    /// MPR took it.
    pub(super) fn mpr_wheel(&mut self, lines: (f32, f32), p: (f32, f32)) -> bool {
        let scale = f64::from(self.dl.scale.max(0.1));
        let ctrl = self.mods.control_key();
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        if let Some(taken) = self.mpr_curve_wheel(f64::from(lines.1), lp) {
            return taken;
        }
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        let Some((v, r)) = s.view_at(lp) else {
            if s.in_slot(lp) {
                // The 3D view zooms (with or without Ctrl).
                s.active = 3;
                s.vr.orbit.zoom_by(1.2f64.powf(f64::from(lines.1)));
                s.vr.nudge();
                self.mpr.rev += 1;
                self.redraw = true;
            }
            return true;
        };
        s.active = v;
        let Some(views) = s.views.as_mut() else {
            return true;
        };
        // Lines (touchpads' pixels arrive as 40ths of a line).
        let dy = f64::from(lines.1);
        if ctrl {
            views.zoom_at(v, r, 1.2f64.powf(dy), lp);
        } else {
            // As in medical mode: the wheel turned towards the user steps forward. Touchpad
            // fractions add up to whole slices.
            s.wheel += -dy;
            let steps = s.wheel.trunc().clamp(-10.0, 10.0);
            s.wheel -= s.wheel.trunc();
            if steps != 0.0 {
                views.scroll(v, steps);
            }
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// Lays MPR out over the viewport (physical pixels) and makes its readouts; called
    /// before drawing.
    pub(super) fn mpr_layout(&mut self, vp: PxRect, rtl: bool) {
        let scale = f64::from(self.dl.scale.max(0.1));
        let medical_tools = !self.profile_hides(Action::MprDental);
        let Some(s) = self.mpr.s.as_mut() else { return };
        s.medical_tools = medical_tools;
        if !medical_tools && let Some(d) = s.dental.as_mut() {
            d.shown = false;
        }
        let r = [
            f64::from(vp.x) / scale,
            f64::from(vp.y) / scale,
            f64::from(vp.w) / scale,
            f64::from(vp.h) / scale,
        ];
        if let Some(c) = curve_app::shown(&mut s.curved, &mut s.dental) {
            let (l, rects) = curved::layout(r, scale, rtl, c.view, c.dental.is_some());
            c.rects = Some(rects);
            c.update(scale);
            s.layout = Some(l);
            return;
        }
        s.layout = Some(match s.maximised {
            Some(v) => draw::layout_maximised(r, scale, rtl, v),
            None => draw::layout(r, scale, rtl),
        });
    }

    /// Draws the planes and the 3D view into the scene over `canvas` (linear, premultiplied),
    /// grey planes on the GSDF display [`Mpr::gsdf`] when one is set. True when the 3D view
    /// refines further (another frame is wanted).
    pub(super) fn mpr_render(
        mpr: &mut Mpr,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        scale: f64,
        canvas: [f32; 4],
    ) -> bool {
        let Some(s) = mpr.s.as_mut() else {
            return false;
        };
        let (Some(tex), Some(views), Some(l)) = (s.tex.as_ref(), s.views.as_ref(), s.layout) else {
            return false;
        };
        let window = s.current_window();
        let mut planes: Vec<_> = (0..3)
            .map(|v| views.plane(v, l.views[v], scale, window))
            .collect();
        if let Some(c) = s.curve() {
            planes.extend(c.planes(scale, window, views, canvas));
        }
        let r = mpr.renderer.get_or_insert_with(MprRenderer::new);
        // The same display function as the 2D view (`medical.display_function`).
        r.set_gsdf(mpr.gsdf);
        let ready: Vec<&Channel> = s.channels.iter().filter(|c| c.tex.is_some()).collect();
        if ready.is_empty() && s.tint == [1.0; 3] {
            r.draw(gpu, enc, target, size, tex, &planes);
        } else {
            // The channels added in their colours, each through its own display range moved
            // as the user moves the first one's.
            // Each through the 2D view's stretch; a histogram equalisation through its own
            // curve.
            let mut sets: Vec<LayerSet<'_>> =
                vec![(tex, planes.clone(), s.tint, s.tone.curve(), s.mid)];
            for ch in &ready {
                let (Some(t), Some(w)) = (ch.tex.as_ref(), s.channel_window(ch)) else {
                    continue;
                };
                let mut p: Vec<_> = (0..3)
                    .map(|v| views.plane(v, l.views[v], scale, w))
                    .collect();
                if let Some(c) = s.curve() {
                    p.extend(c.planes(scale, w, views, canvas));
                }
                sets.push((t, p, ch.tint, ch.tone.curve(), ch.mid));
            }
            let layers: Vec<specular_render_volume::Layer> = sets
                .iter()
                .map(
                    |(vol, planes, tint, curve, mid)| specular_render_volume::Layer {
                        vol,
                        planes,
                        tint: *tint,
                        stretch: s.stretch,
                        curve,
                        mid: *mid,
                    },
                )
                .collect();
            r.draw_layers(gpu, enc, target, size, &layers);
        }
        s.analysis.draw(gpu, enc, target, size, &planes[..3], scale);
        let series = s.series();
        let now = std::time::Instant::now();
        let view = volume3d::view(
            &s.vr,
            views,
            &l.views,
            s.placed.view3d.unwrap_or(l.slot),
            scale,
            &series,
            window,
            canvas,
            now,
        );
        let vr = mpr.vr.get_or_insert_with(VrRenderer::new);
        let more = vr.draw(gpu, enc, target, size, tex, s.vr.bricks.as_ref(), &view);
        if more {
            mpr.rev += 1;
        }
        if tex.uploaded() > 0 && s.load.first_view.is_none() {
            let t = s.load.started.elapsed();
            s.load.first_view = Some(t);
            specular_log::info!(
                target: specular_log::DECODE,
                "mpr: first view after {:.0} ms ({} of {} slices)",
                t.as_secs_f64() * 1e3,
                tex.uploaded(),
                views_slices(s)
            );
        }
        more
    }

    /// Draws the overlay (lines, letters, readouts, measurements, controls).
    pub(super) fn mpr_overlay(
        mpr: &mut Mpr,
        dl: &mut specular_gfx::DrawList,
        text: &mut specular_gfx::Text,
        pal: &Palette,
        labels_default: bool,
        rtl: bool,
        ts: f32,
    ) {
        let Some(s) = mpr.s.as_mut() else { return };
        let Some(sc) = scene(s, labels_default, f64::from(dl.scale.max(0.1))) else {
            return;
        };
        let placed = draw::draw(dl, text, &sc, pal, rtl, ts);
        s.placed = placed;
        let window = s.current_window();
        let length = s.length();
        let scale = f64::from(dl.scale.max(0.1));
        s.sync_curve_list();
        if let (Some(views), Some(layout)) = (s.views.as_ref(), s.layout)
            && let Some(c) = curve_app::shown(&mut s.curved, &mut s.dental)
        {
            let cx = curved::Ctx {
                views,
                layout,
                scale,
                window,
                length,
                pointer: s.pointer,
                measures: &s.measures,
                draft: s.draft,
            };
            let (placed, field_text) = curved::draw(dl, text, c, &cx, pal, rtl, ts);
            c.placed = placed;
            c.field_text = field_text;
        }
    }

    /// The overlay's AccessKit nodes.
    pub(super) fn mpr_a11y(
        mpr: &Mpr,
        tree: &mut accesskit::TreeUpdate,
        labels_default: bool,
        scale: f64,
        dl_scale: f64,
    ) {
        let Some(s) = mpr.s.as_ref() else { return };
        let Some(sc) = scene(s, labels_default, dl_scale) else {
            return;
        };
        let c = s.views.as_ref().map_or([0.0; 3], |v| v.c);
        let [x, y, z] = c.map(|v| number((v * 10.0).round() / 10.0));
        let crosshair = match s.length() {
            Length::Mm => t!("mpr-crosshair-a11y", x = x, y = y, z = z),
            Length::Angstrom => t!("mpr-crosshair-a11y-angstrom", x = x, y = y, z = z),
            Length::Voxel => t!("mpr-crosshair-a11y-voxels", x = x, y = y, z = z),
            l => t!(
                "mpr-crosshair-a11y-unit",
                x = x,
                y = y,
                z = z,
                unit = l.symbol().unwrap_or_default()
            ),
        };
        let series = s.series();
        let vr = format!(
            "{}: {}",
            t!("mpr-3d"),
            t!(
                "vr-view-a11y",
                preset = t!("vr-preset", preset = s.vr.preset(series.hounsfield).id()),
                mode = sc.vr.mode.clone(),
                clip = sc.vr.clip.clone(),
                threshold = number(s.vr.threshold(series.hounsfield, series.window).round())
            )
        );
        draw::a11y(tree, &sc, &s.placed, crosshair, vr, scale);
        if let Some(c) = s.curve() {
            curved::a11y(tree, c, s.length(), scale);
        }
    }

    /// The 3D view's background work: the empty-space grid, the end of a coarse spell after a
    /// wheel or key move, and the STL and planes exports' steps.
    fn mpr_tick_3d(&mut self) {
        let now = self.now();
        self.mpr_tick_planes();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if let Some(rx) = &s.vr.bricks_rx
            && let Ok(b) = rx.try_recv()
        {
            s.vr.bricks = Some(b);
            s.vr.bricks_rx = None;
            self.mpr.rev += 1;
            self.redraw = true;
        }
        if s.vr.settle.is_some_and(|t| std::time::Instant::now() >= t) {
            s.vr.settle = None;
            self.mpr.rev += 1;
            self.redraw = true;
        }
        if let Some(rx) = &s.vr.save_rx {
            match rx.try_recv() {
                Ok(path) => {
                    s.vr.save_rx = None;
                    let series = s.series();
                    let threshold = s.vr.threshold(series.hounsfield, series.window) as f32;
                    let smooth = s.vr.smooth;
                    let volume = s.load.volume.clone();
                    let (tx, rx) = crossbeam_channel::bounded(1);
                    self.loader.background("mpr-stl", move |l| {
                        let mut mesh = isosurface(&lock(&volume), threshold);
                        if smooth {
                            mesh.smooth(8);
                        }
                        let r = if mesh.triangles.is_empty() {
                            Ok((path, 0))
                        } else {
                            write_stl(&path, &mesh).map(|()| (path, mesh.triangles.len()))
                        };
                        let _ = tx.send(r);
                        l.waker()();
                    });
                    s.vr.export_rx = Some(rx);
                    self.chrome.toast(t!("vr-stl-exporting"), None, now);
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => s.vr.save_rx = None,
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &s.vr.export_rx
            && let Ok(r) = rx.try_recv()
        {
            s.vr.export_rx = None;
            let msg = match r {
                Ok((path, n)) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    t!("vr-stl-saved", count = n as i64, name = name)
                }
                Err(e) => t!("vr-stl-failed", reason = e),
            };
            self.chrome.toast(msg, None, now);
            self.redraw = true;
        }
    }

    /// Asks where to save the surface at the 3D view's threshold (the system's dialog, on a
    /// thread); [`App::mpr_tick_3d`] builds and writes it.
    fn mpr_export_stl(&mut self) {
        let dir = self.folder.clone();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if s.vr.save_rx.is_some() || s.vr.export_rx.is_some() {
            return;
        }
        let series = s.series();
        let threshold = s.vr.threshold(series.hounsfield, series.window).round();
        let title = t!("palette-mpr-export-stl");
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("mpr-stl-dialog", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .set_title(title)
                .add_filter("STL", &["stl"])
                .set_file_name(format!("surface-{threshold}.stl"));
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(mut p) = dlg.save_file() {
                if p.extension().is_none() {
                    p.set_extension("stl");
                }
                let _ = tx.send(p);
            }
            drop(tx);
            l.waker()();
        });
        s.vr.save_rx = Some(rx);
    }

    /// Asks for a folder to save the three planes through the crosshair in (the system's
    /// dialog, on a thread); [`App::mpr_tick_planes`] writes them.
    fn mpr_export_planes(&mut self) {
        let dir = self.folder.clone();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if s.planes_rx.is_some() || s.planes_done.is_some() {
            return;
        }
        let title = t!("palette-mpr-export-planes");
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("mpr-planes-dlg", move |l| {
            let mut dlg = rfd::FileDialog::new().set_title(title);
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_folder() {
                let _ = tx.send(p);
            }
            drop(tx);
            l.waker()();
        });
        s.planes_rx = Some(rx);
    }

    /// The planes export: the folder chosen, then the images written off the UI thread.
    fn mpr_tick_planes(&mut self) {
        let now = self.now();
        let stem = self
            .current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| p.file_stem())
            .map_or_else(|| "volume".to_owned(), |s| s.to_string_lossy().into_owned());
        let Some(s) = self.mpr.s.as_mut() else { return };
        if let Some(rx) = &s.planes_rx {
            match rx.try_recv() {
                Ok(dir) => {
                    s.planes_rx = None;
                    let Some(views) = s.views.clone() else { return };
                    let window = s.current_window();
                    let oriented = s.oriented();
                    let volume = s.load.volume.clone();
                    let (tx, rx) = crossbeam_channel::bounded(1);
                    self.loader.background("mpr-planes", move |l| {
                        let r =
                            planes::export(&lock(&volume), &views, window, oriented, &dir, &stem);
                        let _ = tx.send(r);
                        l.waker()();
                    });
                    s.planes_done = Some(rx);
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => s.planes_rx = None,
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &s.planes_done
            && let Ok(r) = rx.try_recv()
        {
            s.planes_done = None;
            let msg = match r {
                Ok((n, folder)) => t!("mpr-planes-saved", count = n as i64, folder = folder),
                Err(e) => t!("mpr-planes-failed", reason = e),
            };
            self.chrome.toast(msg, None, now);
            self.redraw = true;
        }
    }

    /// Seconds until the 3D view refines after a wheel or key move, if it waits for that.
    pub(super) fn mpr_wake(&self) -> Option<f64> {
        let t = self.mpr.s.as_ref()?.vr.settle?;
        Some(
            t.saturating_duration_since(std::time::Instant::now())
                .as_secs_f64(),
        )
    }

    /// When MPR's first view was drawn, in ms after its load started (the app gates).
    pub(super) fn mpr_first_view_ms(&self) -> Option<f64> {
        let t = self.mpr.s.as_ref()?.load.first_view?;
        Some(t.as_secs_f64() * 1e3)
    }

    /// Nothing of MPR is still on its way (screenshots wait for it).
    pub(super) fn mpr_settled(&self) -> bool {
        self.mpr.show.is_none()
            && self.mpr.s.as_ref().is_none_or(|s| {
                s.load.finished() && s.load.first_view.is_some() && !s.analysis.searching()
            })
    }

    /// `--show mpr`: opens MPR on the image shown (for screenshots).
    pub(super) fn mpr_show(&mut self, what: &str) -> bool {
        if what != "mpr" {
            return false;
        }
        if !self.mpr_active() {
            self.mpr.show = Some(std::time::Instant::now());
        }
        true
    }
}

/// Writes `mesh` as binary STL to `path` (through a temporary file beside it).
fn write_stl(path: &std::path::Path, mesh: &specular_render_volume::Mesh) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension("stl.part");
    let write = || -> std::io::Result<()> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        mesh.write_stl(&mut f)?;
        f.flush()?;
        f.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        crate::error_text::io_reason(&e)
    })
}

impl App {
    /// The analysis actions in MPR: `None` when `a` is not one, else the toast to show.
    fn mpr_analysis_action(&mut self, a: Action) -> Option<Option<String>> {
        use analysis::{BLENDS, FILLS, Fusion, Shape};
        let gpu_max = self.gfx.as_ref().map(|g| max_dimension(&g.gpu));
        let budget = self.mpr_budget();
        let pool = self.loader.pool();
        let wake = self.loader.waker();
        let s = self.mpr.s.as_mut()?;
        let an = &mut s.analysis;
        let msg = match a {
            Action::Segments => {
                if an.sets.is_empty() {
                    Some(if an.searched {
                        t!("segments-none")
                    } else {
                        t!("analysis-searching")
                    })
                } else {
                    an.hide = !an.hide;
                    Some(if an.hide {
                        t!("segments-hidden")
                    } else {
                        t!("segments-shown")
                    })
                }
            }
            Action::SegmentOpacity => {
                an.fill = (an.fill + 1) % FILLS.len();
                Some(t!(
                    "segments-fill",
                    percent = number((FILLS[an.fill] * 100.0).round().into())
                ))
            }
            Action::Fusion if an.partner.is_none() && an.fusion.is_none() && an.dose.is_some() => {
                an.dose.as_mut().map(|d| d.cycle())
            }
            Action::FusionColormap | Action::FusionBlend
                if an.fusion.is_none()
                    && an
                        .dose
                        .as_ref()
                        .is_some_and(|d| d.mode == dose::DoseMode::Colour) =>
            {
                let d = an.dose.as_mut()?;
                Some(if a == Action::FusionColormap {
                    d.colormap = d.colormap.next();
                    specular_i18n::I18n::global().tr(&format!("colormap-{}", d.colormap.id()))
                } else {
                    d.blend = (d.blend + 1) % BLENDS.len();
                    t!(
                        "fusion-blend-toast",
                        percent = number((BLENDS[d.blend] * 100.0).round().into())
                    )
                })
            }
            Action::Fusion => {
                if let Some(f) = an.fusion.take() {
                    f.load.cancel();
                    an.nudge = false;
                    Some(t!("fusion-off"))
                } else if let (Some(p), Some(gpu_max)) = (an.partner.clone(), gpu_max) {
                    let centre = lock(&s.load.volume).geometry.centre();
                    let suv = an
                        .partner_suv
                        .as_ref()
                        .and_then(|r| r.as_ref().ok())
                        .map(|s| s.bw);
                    let modality = p.modality.clone();
                    let load = Loading::start(Source::Series(p), pool, wake, gpu_max, budget);
                    an.fusion = Some(Fusion {
                        load,
                        tex: None,
                        window: None,
                        colormap: specular_render_volume::Colormap::HotIron,
                        blend: 0,
                        rigid: specular_render_volume::Rigid {
                            centre,
                            ..Default::default()
                        },
                        suv,
                        modality: modality.clone(),
                        on: true,
                    });
                    Some(t!("fusion-opening", modality = modality))
                } else {
                    Some(if an.searched {
                        t!("fusion-none")
                    } else {
                        t!("analysis-searching")
                    })
                }
            }
            Action::FusionColormap | Action::FusionBlend | Action::FusionNudge => {
                match an.fusion.as_mut() {
                    None => Some(t!("fusion-needed")),
                    Some(f) => match a {
                        Action::FusionColormap => {
                            f.colormap = f.colormap.next();
                            Some(
                                specular_i18n::I18n::global()
                                    .tr(&format!("colormap-{}", f.colormap.id())),
                            )
                        }
                        Action::FusionBlend => {
                            f.blend = (f.blend + 1) % BLENDS.len();
                            Some(t!(
                                "fusion-blend-toast",
                                percent = number((BLENDS[f.blend] * 100.0).round().into())
                            ))
                        }
                        _ => {
                            an.nudge = !an.nudge;
                            Some(if an.nudge {
                                t!("fusion-nudge-hint")
                            } else {
                                t!("fusion-nudge-off")
                            })
                        }
                    },
                }
            }
            Action::Tmtv => {
                an.tmtv.drag = None;
                an.tmtv.shape = match an.tmtv.shape {
                    None => Some(Shape::Sphere),
                    Some(Shape::Sphere) => Some(Shape::Box),
                    Some(Shape::Box) => None,
                };
                if an.tmtv.shape.is_some() {
                    s.measuring = false;
                    s.draft = None;
                }
                Some(match an.tmtv.shape {
                    Some(Shape::Sphere) => t!("tmtv-hint-sphere"),
                    Some(Shape::Box) => t!("tmtv-hint-box"),
                    None => {
                        an.tmtv.region = None;
                        an.tmtv.result = None;
                        an.tmtv.tex = None;
                        an.tmtv.note = None;
                        t!("tmtv-off")
                    }
                })
            }
            Action::TmtvThreshold => {
                an.tmtv.relative = !an.tmtv.relative;
                // Measure the region again at the new threshold.
                if let (Some(region), Some(gfx)) = (an.tmtv.region, self.gfx.as_ref()) {
                    let vol = lock(&s.load.volume);
                    an.tmtv_measure(&gfx.gpu, &vol, region);
                }
                Some(if an.tmtv.relative {
                    t!("tmtv-threshold-share", share = 41)
                } else {
                    t!("tmtv-threshold-suv", suv = "2.5")
                })
            }
            // The dose's DVHs are MPR's curves.
            Action::TimeCurve => match an.dose.as_ref().map(|d| dose::dvh_sheet(&d.found)) {
                Some(Some(sheet)) => {
                    self.chrome.show_plot(sheet);
                    None
                }
                Some(None) => Some(t!("dvh-none")),
                None => Some(t!("curve-not-mpr")),
            },
            Action::ExportCurve => Some(t!("curve-not-mpr")),
            _ => return None,
        };
        Some(msg)
    }
}

/// A measured length: "12.5 mm", "12.5 Å" on a density map or cube grid, "12.5 µm" on a
/// microscopy stack.
/// A device's motion over 2D pane `v` (rect `r`): the content moves against the stick by
/// `d` logical pixels, as in the image view, and zooms about the pane's centre.
pub(super) fn device_pane(views: &mut Views, v: usize, r: R, d: (f64, f64), zoom: f64) {
    if d != (0.0, 0.0) {
        views.pan_by(v, r, -d.0, -d.1);
    }
    if zoom != 1.0 {
        views.zoom_at(v, r, zoom, (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    }
}

fn length_text(d: f64, length: Length) -> String {
    let value = specular_i18n::fixed(d, 1);
    match length {
        Length::Mm => t!("measure-length-mm", value = value),
        Length::Angstrom => t!("measure-length-angstrom", value = value),
        Length::Voxel => t!("mpr-length-voxels", value = value),
        l => t!(
            "mpr-length-unit",
            value = value,
            unit = l.symbol().unwrap_or_default()
        ),
    }
}

/// A view's name: axial, coronal or sagittal when the volume is oriented, else XY, XZ or
/// YZ (oblique when turned away from its plane).
pub(super) fn view_name(v: usize, oriented: bool, oblique: bool) -> String {
    if oblique {
        return t!("mpr-oblique");
    }
    let id = if oriented {
        KINDS[v].message_id()
    } else {
        KINDS[v].neutral_id()
    };
    specular_i18n::I18n::global().tr(id)
}

/// A seismic view's position line: the time slice's time (or depth), the inline or the
/// crossline number (patient x, y and z are crossline, inline and bottom-up sample).
fn seismic_position(
    s: &specular_decode::science::Seismic,
    v: usize,
    position: f64,
    samples: u32,
) -> String {
    let at = |list: &[i32]| {
        let i = position
            .round()
            .clamp(0.0, list.len().saturating_sub(1) as f64) as usize;
        list.get(i).map_or_else(String::new, i32::to_string)
    };
    match v {
        0 => {
            let k = f64::from(samples.saturating_sub(1)) - position.round();
            let time = super::science::trimmed(s.t0 + s.dt * k.max(0.0), 3);
            if s.depth {
                t!("science-seismic-depth", depth = time, unit = s.unit.clone())
            } else {
                t!("science-seismic-time", time = time)
            }
        }
        1 => t!("mpr-seismic-inline", inline = at(&s.inlines)),
        _ => t!("mpr-seismic-crossline", crossline = at(&s.crosslines)),
    }
}

/// The header numbers of the trace at patient point `q` (crossline, inline, bottom-up
/// sample), its time (or depth) included.
fn seismic_cell(s: &specular_decode::science::Seismic, q: V3, samples: u32) -> Vec<String> {
    let (xl, il) = (q[0].round(), q[1].round());
    let k = f64::from(samples.saturating_sub(1)) - q[2].round();
    if xl < 0.0
        || il < 0.0
        || k < 0.0
        || xl >= s.crosslines.len() as f64
        || il >= s.inlines.len() as f64
        || k >= f64::from(samples)
    {
        return Vec::new();
    }
    super::science::seismic_lines(s, &[il as u32], xl as u32, k as u32)
}

fn views_slices(s: &Session) -> usize {
    s.load.info.as_ref().map_or(0, |i| i.slices)
}

/// The overlay's content for the session's state.
fn scene(s: &Session, labels_default: bool, scale: f64) -> Option<Scene<'_>> {
    let views = s.views.as_ref()?;
    let layout = s.layout?;
    let window = s.current_window();
    let unit = s.unit();
    let seismic = s.seismic();
    let samples = s.load.info.as_ref().map_or(0, |i| i.full[2]);
    let length = s.length();
    let readouts = [0, 1, 2].map(|v| {
        let name = view_name(v, s.oriented(), views.oblique(v));
        let pos = number((views.position(v) * 10.0).round() / 10.0);
        let mut lines = vec![match seismic.filter(|_| !views.oblique(v)) {
            Some(sm) => seismic_position(sm, v, views.position(v), samples),
            None => position_text(&name, pos, length),
        }];
        if let Some(p) = s.pointer
            && draw::contains(layout.views[v], p)
            && let Some(x) = s.value_at(v, layout.views[v], p, scale)
        {
            let xs = match (seismic, &s.load.info) {
                // Amplitudes are often small: digits to the gain's scale.
                (Some(_), Some(i)) => {
                    let w = i.window.map_or(1.0, |w| w.width).abs().max(1e-30);
                    let d = (3.0 - w.log10().floor()).clamp(0.0, 9.0) as usize;
                    super::science::trimmed(f64::from(x), d)
                }
                _ => number(f64::from(x).round()),
            };
            if let Some(sm) = seismic {
                let q = views.to_patient(v, layout.views[v], p);
                lines.extend(seismic_cell(sm, q, samples));
            }
            if s.channels.is_empty() {
                lines.insert(
                    0,
                    match unit {
                        Some(u) => t!("mpr-value", value = xs, unit = u.to_owned()),
                        None => t!("mpr-value-plain", value = xs),
                    },
                );
            } else {
                // A composite: every shown channel's value.
                let vals = s.channel_values(v, layout.views[v], p, scale);
                for (i, l) in channel_lines(&vals).into_iter().enumerate() {
                    lines.insert(i, l);
                }
            }
            let r = layout.views[v];
            let plane = views.plane(v, r, scale, window);
            let extra = s.analysis.readout(
                Some(x),
                views.to_patient(v, r, p),
                Some(&plane),
                (p.0 * scale, p.1 * scale),
            );
            for (i, l) in extra.into_iter().enumerate() {
                lines.insert(1 + i, l);
            }
        }
        lines
    });
    let measures = [0, 1, 2].map(|v| {
        let r = layout.views[v];
        let to = |p: V3| {
            let q = views.to_screen(v, r, p);
            (q.0 as f32, q.1 as f32)
        };
        let mut out: Vec<measure::Drawn> = s
            .measures
            .iter()
            .filter(|m| views.on_plane(v, m[0]) && views.on_plane(v, m[1]))
            .map(|m| {
                let d = (0..3)
                    .map(|i| (m[0][i] - m[1][i]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                measure::Drawn {
                    outline: vec![to(m[0]), to(m[1])],
                    closed: false,
                    handles: vec![to(m[0]), to(m[1])],
                    lines: vec![length_text(d, length)],
                    draft: false,
                }
            })
            .collect();
        if let Some(a) = s.draft.filter(|a| views.on_plane(v, *a)) {
            let mut pts = vec![to(a)];
            if let Some(p) = s.pointer.filter(|p| draw::contains(r, *p)) {
                pts.push((p.0 as f32, p.1 as f32));
            }
            out.push(measure::Drawn {
                outline: pts.clone(),
                closed: false,
                handles: pts,
                lines: Vec::new(),
                draft: true,
            });
        }
        out.extend(s.analysis.tmtv_drawn(views, v, r));
        out
    });
    let mut panel = Vec::new();
    let mut wl = t!(
        "medical-window",
        center = number(window.center),
        width = number(window.width)
    );
    if window.invert {
        wl = specular_i18n::dot_join(&[wl, t!("medical-inverted")]);
    }
    panel.push(wl);
    panel.push(draw::slab_text(views, length));
    if let Some(i) = &s.load.info {
        if s.load.done < i.slices {
            panel.push(t!(
                "mpr-loading",
                loaded = s.load.done as i64,
                count = i.slices as i64
            ));
        } else {
            let d = i.plan.dims;
            panel.push(t!(
                "mpr-loaded",
                count = i.slices as i64,
                width = i64::from(d[0]),
                height = i64::from(d[1])
            ));
        }
        if i.plan.step_xy > 1 || i.plan.step_z > 1 {
            panel.push(t!(
                "mpr-downsampled",
                factor = i64::from(i.plan.step_xy.max(i.plan.step_z))
            ));
        }
        panel.extend(i.notes.iter().cloned());
    }
    if let Some(t) = &s.tex {
        let mib = t.bytes() as f64 / f64::from(1u32 << 20);
        panel.push(t!(
            "mpr-gpu-memory",
            size = format!("{} MiB", number(mib.round()))
        ));
    }
    let series = s.series();
    if s.vr.mode.composite() {
        let x = number(s.vr.threshold(series.hounsfield, series.window).round());
        panel.push(match unit {
            Some(u) => t!("vr-threshold", value = format!("{x} {u}")),
            None => t!("vr-threshold", value = x),
        });
    }
    if s.measuring {
        panel.push(t!("measure-hint-length"));
        if !s.measures.is_empty() {
            panel.push(t!("mpr-measure-hint-edit"));
        }
    }
    if let Some(e) = &s.gpu_error {
        panel.push(t!("mpr-no-gpu", reason = crate::error_text::text_reason(e)));
    }
    if s.medical_tools
        && let Some(v) = s.load.info.as_ref().and_then(|i| i.vendor)
    {
        panel.push(t!("dental-vendor", vendor = v));
    }
    panel.extend(s.analysis.panel());
    panel.extend(s.tracts.text());
    panel.extend(s.dense.as_ref().map(dense::Dense::text));
    Some(Scene {
        views,
        layout,
        readouts,
        // Patient orientation letters mean nothing on seismic sections, microscopy stacks
        // and the like.
        labels: seismic.is_none() && s.oriented() && labels_default != s.labels_toggled,
        measures,
        panel,
        active: Some(s.active),
        vr: draw::VrLabels {
            // CT and MR presets are Medical's tier 2: no button in other profiles.
            preset: if s.medical_tools {
                t!("vr-preset", preset = s.vr.preset(series.hounsfield).id())
            } else {
                String::new()
            },
            mode: t!("vr-mode", mode = s.vr.mode.id()),
            clip: t!("vr-clip", clip = s.vr.clip.id()),
            smooth: s.vr.smooth,
        },
        segments: s.analysis.segment_list(),
        tracts: s.tracts.pieces(views, layout),
        buttons_first: s.buttons_first,
        maximised: s.maximised.is_some(),
        curve: draw::CurveButtons {
            on: s.curved.as_ref().is_some_and(|c| c.shown),
            dental: s
                .medical_tools
                .then(|| s.dental.as_ref().is_some_and(|c| c.shown)),
            segments: s
                .has_segments()
                .then(|| segments::DETAILS[s.curve_io.detail % segments::DETAILS.len()]),
        },
    })
}

/// A view's position line: "Axial 12.5 mm", "XY 3.2 µm", "XY 12" (voxels).
fn position_text(view: &str, pos: String, length: Length) -> String {
    let view = view.to_owned();
    match length {
        Length::Mm => t!("mpr-position", view = view, position = pos),
        Length::Angstrom => t!("mpr-position-angstrom", view = view, position = pos),
        Length::Voxel => t!("mpr-position-voxels", view = view, position = pos),
        l => t!(
            "mpr-position-unit",
            view = view,
            position = pos,
            unit = l.symbol().unwrap_or_default()
        ),
    }
}

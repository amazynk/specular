//! Read-only analysis on the 2D image in medical mode (DESIGN.md §7): segmentations (SEG,
//! RTSTRUCT) of the image's frame of reference drawn over it on the GPU with the same
//! renderer as MPR, the SUV under the pointer and SUV presets on PET, and the time-intensity
//! curve of the last ellipse or rectangle over the frames of a multi-frame file or the time
//! points of a dynamic series, drawn as a small chart and saved as CSV.
//!
//! The search for segmentations is MPR's ([`find`]), run once per folder and frame of
//! reference; the image's position and orientation come from its header, read on the same
//! thread whenever the image changes.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::{Receiver, bounded};
use specular_dicom::suv::{Refusal, Suv};
use specular_dicom::{Dicom, ModalityFrame, series, tic};
use specular_gfx::{Align, DrawList, Gpu, Rect, Text, TextStyle, wgpu};
use specular_i18n::t;
use specular_measure::Shape;
use specular_render::View;
use specular_render_volume::{LabelLayer, LabelRenderer, LabelStyle, Plane, Slab};
use specular_theme::metrics::{FONT_BODY, FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};

use super::super::App;
use super::super::mpr::analysis::{FILLS, Set, find, suv_of, suv_window, upload};
use super::measure::{Tool, roi_of};
use super::number;
use crate::loader::Wake;

/// Where the image lies in the patient: each frame's position, the row and column
/// directions, its series and frame of reference.
#[derive(Clone, Debug, Default)]
pub(crate) struct Geom {
    pub positions: Vec<[f64; 3]>,
    pub row: [f64; 3],
    pub col: [f64; 3],
    pub series: String,
    pub frame_of_reference: String,
    pub modality: String,
}

/// What the thread read for an image.
struct Found {
    image: PathBuf,
    geom: Option<Geom>,
    suv: Option<Result<Suv, Refusal>>,
    /// The segmentations, when the folder (or frame of reference) was searched.
    search: Option<(PathBuf, String, Vec<specular_dicom::labels::Labels>)>,
}

/// A curve being read: its result, its cancel flag and its number of time points.
type CurveJob = (Receiver<Result<Curve, String>>, Arc<AtomicBool>, usize);

/// A time-intensity curve.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Curve {
    pub points: Vec<tic::Point>,
    pub unit: String,
}

/// The 2D analysis state; kept across images (the search is per folder).
#[derive(Default)]
pub(crate) struct State {
    job: Option<(Receiver<Found>, Arc<AtomicBool>)>,
    /// The image whose header is in, and that header.
    pub image: Option<PathBuf>,
    pub geom: Option<Geom>,
    /// The folder and frame of reference searched.
    searched: Option<(PathBuf, String)>,
    /// The SUV of the series `suv_series`.
    pub suv: Option<Result<Suv, Refusal>>,
    suv_series: String,
    pub sets: Vec<Set>,
    pending: Vec<specular_dicom::labels::Labels>,
    /// `Shift+O` hid the segmentations; `Alt+O` the fill.
    pub hide: bool,
    pub fill: usize,
    renderer: Option<LabelRenderer>,
    /// Bumped when the drawn overlay changes.
    pub rev: u64,
    pub curve: Option<Curve>,
    curve_job: Option<CurveJob>,
    saved: Option<Receiver<PathBuf>>,
    pub error: Option<String>,
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some((_, c)) = &self.job {
            c.store(true, Ordering::Relaxed);
        }
        if let Some((_, c, _)) = &self.curve_job {
            c.store(true, Ordering::Relaxed);
        }
    }
}

/// The header of `path` as a [`Geom`].
fn geom_of(path: &Path) -> Option<Geom> {
    let i = series::read_header(path).ok()??;
    let o = i.orientation?;
    let positions = if i.frame_positions.is_empty() {
        vec![i.position?]
    } else {
        i.frame_positions.clone()
    };
    Some(Geom {
        positions,
        row: [o[0], o[1], o[2]],
        col: [o[3], o[4], o[5]],
        series: i.series_uid,
        frame_of_reference: i.frame_of_reference,
        modality: i.modality,
    })
}

impl State {
    /// The GPU device was replaced: the segmentations are uploaded to `gpu` again.
    pub(crate) fn gpu_reset(&mut self, gpu: &Gpu) {
        self.renderer = None;
        for s in &mut self.sets {
            match upload(gpu, &s.labels) {
                Ok(t) => s.textures = t,
                Err(e) => {
                    s.textures.clear();
                    self.error = Some(e);
                }
            }
        }
        self.rev += 1;
    }

    /// The SUVbw factor for the image shown, when it is of the PET series measured.
    pub(crate) fn suv_factor(&self) -> Option<f64> {
        let g = self.geom.as_ref()?;
        if g.series != self.suv_series {
            return None;
        }
        self.suv.as_ref()?.as_ref().ok().map(|s| s.bw)
    }

    /// Why there is no SUV on a PET image (the refusal), if it is one.
    pub(crate) fn suv_refusal(&self) -> Option<&Refusal> {
        let g = self.geom.as_ref()?;
        if g.series != self.suv_series {
            return None;
        }
        self.suv.as_ref()?.as_ref().err()
    }

    /// The segmentations that may lie over the image (its frame of reference).
    fn matching(&self) -> impl Iterator<Item = (usize, &Set)> {
        let fr = self
            .geom
            .as_ref()
            .map_or("", |g| g.frame_of_reference.as_str());
        self.sets
            .iter()
            .enumerate()
            .filter(move |(_, s)| !fr.is_empty() && s.labels.frame_of_reference == fr)
    }

    /// Lays labels the user chose (a label volume) over the image: they join its frame of
    /// reference, and upload with the next sync. False when the image's position is not
    /// known, or the labels do not reach its plane.
    pub(crate) fn add_user_labels(&mut self, mut labels: specular_dicom::labels::Labels) -> bool {
        let Some(g) = self.geom.as_ref() else {
            return false;
        };
        let (r, c) = (g.row, g.col);
        let normal = [
            r[1] * c[2] - r[2] * c[1],
            r[2] * c[0] - r[0] * c[2],
            r[0] * c[1] - r[1] * c[0],
        ];
        // A frame's plane meets the labels' grid when its corners lie on both sides of it.
        let reaches = g.positions.iter().any(|p| {
            labels.layers.iter().any(|l| {
                let along = |q: &[f64; 3]| (0..3).map(|k| (q[k] - p[k]) * normal[k]).sum::<f64>();
                let (lo, hi) = l
                    .volume
                    .geometry
                    .corners()
                    .iter()
                    .map(along)
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |a, x| {
                        (a.0.min(x), a.1.max(x))
                    });
                lo <= 0.0 && hi >= 0.0
            })
        });
        if !reaches {
            return false;
        }
        labels.frame_of_reference = g.frame_of_reference.clone();
        self.pending.push(labels);
        self.rev += 1;
        true
    }

    /// The segment names at patient point `p`.
    pub(crate) fn names_at(&self, p: [f64; 3]) -> Vec<String> {
        if self.hide {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (_, s) in self.matching() {
            for k in s.labels.at(p) {
                if !s.hidden[k] {
                    out.push(s.labels.segments[k].label.clone());
                }
            }
        }
        out
    }

    /// Draws the segmentations over the image along `plane`.
    pub(crate) fn render(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        plane: &Plane,
        scale: f64,
    ) {
        let mut colours = Vec::new();
        let mut layers = Vec::new();
        let fr = self
            .geom
            .as_ref()
            .map_or(String::new(), |g| g.frame_of_reference.clone());
        for s in &self.sets {
            if fr.is_empty() || s.labels.frame_of_reference != fr {
                continue;
            }
            let base = colours.len();
            for (k, seg) in s.labels.segments.iter().enumerate() {
                let on = !self.hide && !s.hidden[k];
                colours.push(LabelStyle::colour(seg.colour, if on { 1.0 } else { 0.0 }));
            }
            for (t, l) in s.textures.iter().zip(&s.labels.layers) {
                layers.push(LabelLayer {
                    texture: t,
                    fraction: l.fraction,
                    base,
                });
            }
        }
        if layers.is_empty() {
            return;
        }
        let style = LabelStyle {
            colours,
            fill: FILLS[self.fill % FILLS.len()],
            outline: (1.5 * scale) as f32,
        };
        let r = self.renderer.get_or_insert_with(LabelRenderer::new);
        r.draw(
            gpu,
            enc,
            target,
            size,
            &layers,
            &style,
            std::slice::from_ref(plane),
        );
    }
}

/// Reads `image`'s header, its series' SUV when PET and new, and searches its folder when
/// `search` (the series' acquisition times of day `times`).
fn read(
    image: PathBuf,
    times: Vec<f64>,
    suv_series: String,
    search: Option<String>,
    cancel: &AtomicBool,
) -> Found {
    let geom = geom_of(&image);
    let suv = geom
        .as_ref()
        .filter(|g| g.modality == "PT" && g.series != suv_series)
        .map(|_| suv_of(&image, &times));
    let search = geom.as_ref().and_then(|g| {
        let folder = image.parent()?.to_path_buf();
        (search.as_deref() != Some(g.frame_of_reference.as_str()) || search.is_none()).then(|| {
            let found = find(&image, &times, cancel);
            (folder, g.frame_of_reference.clone(), found.labels)
        })
    });
    Found {
        image,
        geom,
        suv,
        search,
    }
}

/// The plane through frame `index` of an image at `g` with pixels `sp` mm apart (along a
/// row, down a column), as `view` shows it: target pixel (0, 0) at the window's corner.
pub(crate) fn plane_of(g: &Geom, index: u32, sp: (f64, f64), view: &View) -> Option<Plane> {
    let p0 = *g.positions.get(index as usize).or(g.positions.first())?;
    let a = view.stored_from_screen().m.map(f64::from);
    // Stored pixel (x, y) has its centre at (x + 0.5, y + 0.5); the position is the first
    // pixel's centre.
    let origin =
        [0, 1, 2].map(|i| p0[i] + g.row[i] * sp.0 * (a[2] - 0.5) + g.col[i] * sp.1 * (a[5] - 0.5));
    let du = [0, 1, 2].map(|i| g.row[i] * sp.0 * a[0] + g.col[i] * sp.1 * a[3]);
    let dv = [0, 1, 2].map(|i| g.row[i] * sp.0 * a[1] + g.col[i] * sp.1 * a[4]);
    let vp = view.viewport;
    let x = vp.x.max(0.0);
    let y = vp.y.max(0.0);
    Some(Plane {
        rect: [
            x as u32,
            y as u32,
            (vp.w + vp.x - x).max(0.0) as u32,
            (vp.h + vp.y - y).max(0.0) as u32,
        ],
        origin,
        du,
        dv,
        thickness: 0.0,
        slab: Slab::Mip,
        window: suv_window(1.0, 1.0, false),
        canvas: [0.0; 4],
    })
}

impl super::Medical {
    /// The frame shown.
    pub(crate) fn index_shown(&self) -> u32 {
        self.index
    }
}

impl App {
    /// The curve reader panicked: no curve for this image.
    pub(in crate::app) fn medical_lost(&mut self, task: &str) {
        if task == "time-curve" {
            self.medical.an.curve_job = None;
        }
    }

    /// Takes the analysis threads' news and starts reading the image shown when it changed;
    /// called from [`App::medical_sync`].
    pub(crate) fn medical_analysis_sync(&mut self) {
        let now = self.now();
        let an = &mut self.medical.an;
        if let Some((rx, _)) = &an.job
            && let Ok(f) = rx.try_recv()
        {
            an.job = None;
            if let Some(s) = f.suv
                && let Some(g) = &f.geom
            {
                an.suv = Some(s);
                an.suv_series = g.series.clone();
            }
            an.image = Some(f.image);
            an.geom = f.geom;
            if let Some((folder, fr, labels)) = f.search {
                an.searched = Some((folder, fr));
                an.sets.clear();
                an.pending = labels;
            }
            an.rev += 1;
            self.redraw = true;
        }
        let an = &mut self.medical.an;
        if !an.pending.is_empty()
            && let Some(gfx) = self.gfx.as_ref()
        {
            for labels in std::mem::take(&mut an.pending) {
                match upload(&gfx.gpu, &labels) {
                    Ok(textures) => {
                        let n = labels.segments.len();
                        an.sets.push(Set {
                            labels,
                            textures,
                            hidden: vec![false; n],
                        });
                    }
                    Err(e) => an.error = Some(e),
                }
            }
            an.rev += 1;
            self.redraw = true;
        }
        if let Some((rx, _, _)) = &an.curve_job
            && let Ok(r) = rx.try_recv()
        {
            an.curve_job = None;
            match r {
                Ok(c) => an.curve = Some(c),
                Err(e) => {
                    self.chrome.toast(t!("curve-failed", reason = e), None, now);
                }
            }
            self.redraw = true;
        }
        let an = &mut self.medical.an;
        if let Some(rx) = &an.saved
            && rx.try_recv().is_ok()
        {
            an.saved = None;
            self.chrome.toast(t!("curve-saved"), None, now);
        }
        // The image shown changed: read its header (and search its folder when new).
        if !self.medical.active() || self.medical.an.job.is_some() {
            return;
        }
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        if self.medical.an.image.as_ref() == Some(&path) {
            return;
        }
        let times: Vec<f64> = self
            .series_set()
            .filter(|s| s.items.iter().any(|i| i.key == path))
            .map(|s| {
                s.items
                    .iter()
                    .filter_map(|i| i.instance.acquisition_time)
                    .collect()
            })
            .unwrap_or_default();
        let an = &mut self.medical.an;
        let folder = path.parent().map(Path::to_path_buf);
        let search = match &an.searched {
            Some((f, fr)) if Some(f) == folder.as_ref() => Some(fr.clone()),
            _ => None,
        };
        let suv_series = an.suv_series.clone();
        let (tx, rx) = bounded(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let wake: Wake = self.loader.waker();
        let _ = specular_thread::builder("dcm-analysis").spawn(move || {
            // A panic reading this image's companions leaves it without overlays; the next
            // image is read as usual (the job ends either way).
            let image = path.clone();
            let found = specular_core::isolate::run("dicom analysis", || {
                read(path, times, suv_series, search, &c)
            })
            .unwrap_or(Found {
                image,
                geom: None,
                suv: None,
                search: None,
            });
            let _ = tx.send(found);
            wake();
        });
        an.job = Some((rx, cancel));
    }

    /// The plane the segmentations are drawn along over the 2D image (physical pixels).
    pub(crate) fn medical_plane(&self) -> Option<Plane> {
        self.medical_state()?;
        let m = &self.medical;
        let an = &m.an;
        if an.hide || an.matching().next().is_none() {
            return None;
        }
        let g = an.geom.as_ref()?;
        let sp = m.frame.as_ref()?.calibration().spacing?;
        plane_of(g, m.index, (sp.x, sp.y), &self.view)
    }

    /// The patient point under stored pixel (x, y) of the shown frame.
    pub(crate) fn medical_patient(&self, x: f64, y: f64) -> Option<[f64; 3]> {
        let m = &self.medical;
        let g = m.an.geom.as_ref()?;
        let sp = m.frame.as_ref()?.calibration().spacing?;
        let p0 = *g.positions.get(m.index as usize).or(g.positions.first())?;
        Some([0, 1, 2].map(|i| p0[i] + g.row[i] * sp.x * (x - 0.5) + g.col[i] * sp.y * (y - 0.5)))
    }

    /// The analysis actions on the 2D image; `None` when `a` is not one, else its toast.
    pub(crate) fn medical_analysis_action(&mut self, a: specular_input::Action) -> Option<String> {
        use specular_input::Action;
        let an = &mut self.medical.an;
        Some(match a {
            Action::Segments => {
                if an.matching().next().is_none() {
                    if an.job.is_some() {
                        t!("analysis-searching")
                    } else {
                        t!("segments-none")
                    }
                } else {
                    an.hide = !an.hide;
                    an.rev += 1;
                    if an.hide {
                        t!("segments-hidden")
                    } else {
                        t!("segments-shown")
                    }
                }
            }
            Action::SegmentOpacity => {
                an.fill = (an.fill + 1) % FILLS.len();
                an.rev += 1;
                t!(
                    "segments-fill",
                    percent = number((f64::from(FILLS[an.fill]) * 100.0).round())
                )
            }
            Action::WindowPreset(n @ 1..=3) => {
                if self.medical.baked {
                    return None;
                }
                let f = self.medical.an.suv_factor()?;
                let upper = super::super::mpr::analysis::SUV_PRESETS[n as usize - 1];
                let invert = self.medical.current().invert;
                self.medical.window = Some(suv_window(upper, f, invert));
                t!("suv-preset", upper = number(upper))
            }
            Action::Fusion
            | Action::FusionColormap
            | Action::FusionBlend
            | Action::FusionNudge
            | Action::Tmtv
            | Action::TmtvThreshold => t!("analysis-mpr-only"),
            Action::TimeCurve => {
                if an.curve.take().is_some() {
                    return Some(String::new());
                }
                return Some(self.medical_curve_start());
            }
            Action::ExportCurve => {
                let Some(c) = &an.curve else {
                    return Some(t!("curve-none"));
                };
                let csv = tic::csv(&c.points, &c.unit);
                let (tx, rx) = bounded::<PathBuf>(1);
                self.loader.background("dcm-curve-save", move |l| {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .set_file_name("time-intensity.csv")
                        .save_file()
                        && std::fs::write(&p, csv).is_ok()
                    {
                        let _ = tx.send(p);
                        l.waker()();
                    }
                });
                self.medical.an.saved = Some(rx);
                String::new()
            }
            _ => return None,
        })
    }

    /// Starts the curve of the last ellipse or rectangle; the toast to show.
    fn medical_curve_start(&mut self) -> String {
        let m = &self.medical;
        let Some((tool, pts)) = m
            .measurements
            .iter()
            .rev()
            .find(|x| matches!(x.tool, Tool::Ellipse | Tool::Rect) && x.points.len() >= 2)
            .map(|x| (x.tool, x.points.clone()))
        else {
            return t!("curve-no-roi");
        };
        let Some(roi) = roi_of(tool, pts[0], pts[1]) else {
            return t!("curve-no-roi");
        };
        let unit = m.unit.clone().unwrap_or_default();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let (tx, rx) = bounded(1);
        let wake: Wake = self.loader.waker();
        let count;
        if m.frames > 1 {
            // A multi-frame file: its frames, at their frame times.
            let Some(session) =
                m.id.and_then(|id| self.images.get(&id))
                    .and_then(|i| i.session.clone())
            else {
                return t!("curve-no-roi");
            };
            let n = m.frames;
            let times = m
                .timing
                .as_ref()
                .filter(|t| t.len() >= n as usize)
                .map(|t| tic::frame_times(t));
            count = n as usize;
            self.loader.background("time-curve", move |_| {
                let token = specular_core::CancelToken::new();
                let mut frames = Vec::new();
                for i in 0..n {
                    if c.load(Ordering::Relaxed) {
                        return;
                    }
                    match session.modality_frame_at(i, &token) {
                        Some(Ok(f)) => {
                            frames.push((times.as_ref().map_or(f64::from(i), |t| t[i as usize]), f))
                        }
                        Some(Err(e)) => {
                            let _ = tx.send(Err(crate::error_text::reason(&e)));
                            wake();
                            return;
                        }
                        None => {}
                    }
                }
                let refs: Vec<(f64, &ModalityFrame)> =
                    frames.iter().map(|(t, f)| (*t, f.as_ref())).collect();
                let _ = tx.send(Ok(Curve {
                    points: tic::curve(&refs, &roi),
                    unit,
                }));
                wake();
            });
        } else {
            // A dynamic series: the items at this position, in time order.
            let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
                return t!("curve-no-roi");
            };
            let Some(s) = self.series_set() else {
                return t!("curve-not-dynamic");
            };
            let Some(at) = s.items.iter().position(|i| i.key == path) else {
                return t!("curve-not-dynamic");
            };
            let Some(points) = tic::time_points(s, at) else {
                return t!("curve-not-dynamic");
            };
            let files: Vec<(f64, PathBuf)> = points
                .iter()
                .map(|&(i, t)| (t, s.items[i].key.clone()))
                .collect();
            count = files.len();
            self.loader.background("time-curve", move |_| {
                let r = series_curve(&files, &roi, &c).map(|points| Curve { points, unit });
                if !c.load(Ordering::Relaxed) {
                    let _ = tx.send(r);
                    wake();
                }
            });
        }
        if let Some((_, old, _)) = self.medical.an.curve_job.replace((rx, cancel, count)) {
            old.store(true, Ordering::Relaxed);
        }
        t!("curve-loading", count = count as i64)
    }

    /// The readout's analysis lines for the stored pixel (x, y) with value `v`.
    pub(crate) fn medical_analysis_readout(&self, at: Option<(f64, f64, f64)>) -> Vec<String> {
        let an = &self.medical.an;
        let mut out = Vec::new();
        if let Some((x, y, v)) = at {
            if let Some(f) = an.suv_factor() {
                out.push(t!("suv-value", value = specular_i18n::fixed(v * f, 2)));
            }
            if let Some(p) = self.medical_patient(x, y) {
                out.extend(an.names_at(p));
            }
        }
        if let Some(r) = an.suv_refusal() {
            out.push(t!(
                "suv-refused",
                reason = specular_i18n::I18n::global().tr(r.key())
            ));
        }
        if let Some((_, _, n)) = &an.curve_job {
            out.push(t!("curve-loading", count = *n as i64));
        }
        out
    }

    /// The curve to draw, if any.
    pub(crate) fn medical_curve(&self) -> Option<&Curve> {
        self.medical_state()?;
        self.medical.an.curve.as_ref()
    }
}

/// The curve over the files of a dynamic series (time s, path).
fn series_curve(
    files: &[(f64, PathBuf)],
    roi: &Shape,
    cancel: &AtomicBool,
) -> Result<Vec<tic::Point>, String> {
    let mut frames = Vec::new();
    for (t, p) in files {
        if cancel.load(Ordering::Relaxed) {
            return Err(String::new());
        }
        let file = std::fs::read(p).map_err(|e| crate::error_text::reason(&e))?;
        let d = Dicom::parse(&file).map_err(|e| crate::error_text::reason(&e))?;
        let f = d
            .modality_frame(&file, 0, Some(cancel))
            .map_err(|e| crate::error_text::reason(&e))?;
        frames.push((*t, f));
    }
    let refs: Vec<(f64, &ModalityFrame)> = frames.iter().map(|(t, f)| (*t, f)).collect();
    Ok(tic::curve(&refs, roi))
}

/// Mean's and max's colours.
const MEAN: Color = Color::rgba(0.30, 0.62, 1.0, 1.0);
const MAX: Color = Color::rgba(0.98, 0.55, 0.15, 1.0);

/// Draws the curve as a chart at the bottom-start corner of the image `area` (logical);
/// returns its rectangle.
pub(crate) fn draw_curve(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    curve: &Curve,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let w = (300.0 * ts).min(aw - 2.0 * m).max(120.0);
    let h = (170.0 * ts).min(ah - 2.0 * m).max(80.0);
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = ay + ah - m - h;
    let r = Rect::new(x, y, w, h);
    dl.rect_border(
        r,
        RADIUS_MD,
        pal.surface.with_alpha(0.92),
        pal.separator,
        1.0,
    );
    let st = TextStyle::new(FONT_BODY * ts).weight(600.0);
    let small = TextStyle::new(FONT_SMALL * ts).tabular();
    let pad = PAD * ts * 0.75;
    let title = text.layout(&t!("curve-title"), st, Some(w - 2.0 * pad), Align::Start);
    let tx = if rtl {
        x + w - pad - title.width
    } else {
        x + pad
    };
    text.draw(dl, &title, tx, y + pad, pal.text);
    // The legend after the title.
    let legend: Vec<_> = [(t!("curve-mean"), MEAN), (t!("curve-max"), MAX)]
        .into_iter()
        .map(|(s, c)| (text.layout(&s, small, None, Align::Start), c))
        .collect();
    let ly = y + pad + title.height + 2.0 * ts;
    let mut lx = 0.0;
    for (b, c) in &legend {
        let sw = 12.0 * ts;
        let bx = if rtl {
            x + w - pad - lx - sw
        } else {
            x + pad + lx
        };
        dl.line(
            (bx, ly + b.height / 2.0),
            (bx + sw, ly + b.height / 2.0),
            2.5 * ts,
            *c,
        );
        let tx = if rtl {
            bx - 4.0 * ts - b.width
        } else {
            bx + sw + 4.0 * ts
        };
        text.draw(dl, b, tx, ly, pal.text_muted);
        lx += sw + 8.0 * ts + b.width + 12.0 * ts;
    }
    let pts = &curve.points;
    let (t0, t1) = pts
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.t), b.max(p.t)));
    let (v0, v1) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), p| {
        (a.min(p.mean).min(p.max), b.max(p.mean).max(p.max))
    });
    if pts.is_empty() {
        return r;
    }
    let (v0, v1) = if v1 - v0 < 1e-9 {
        (v0 - 1.0, v1 + 1.0)
    } else {
        (v0, v1)
    };
    let t1 = if t1 - t0 < 1e-9 { t0 + 1.0 } else { t1 };
    let top = number(v1.round());
    let bottom = number(v0.round());
    let tl = text.layout(&top, small, None, Align::Start);
    let bl = text.layout(&bottom, small, None, Align::Start);
    let label_w = tl.width.max(bl.width) + 4.0 * ts;
    let start_l = text.layout(&number(t0.round()), small, None, Align::Start);
    let end_l = text.layout(
        &format!("{} {}", number(t1.round()), t!("curve-seconds")),
        small,
        None,
        Align::Start,
    );
    // The plot area: the value labels on the start side, the time labels below.
    let py0 = ly + legend.first().map_or(0.0, |l| l.0.height) + 6.0 * ts;
    let py1 = y + h - pad - start_l.height - 2.0 * ts;
    let (px0, px1) = if rtl {
        (x + pad, x + w - pad - label_w)
    } else {
        (x + pad + label_w, x + w - pad)
    };
    if py1 - py0 < 10.0 || px1 - px0 < 10.0 {
        return r;
    }
    let axis = pal.text_faint;
    dl.line((px0, py1), (px1, py1), 1.0, axis);
    let ax_x = if rtl { px1 } else { px0 };
    dl.line((ax_x, py0), (ax_x, py1), 1.0, axis);
    let lx = if rtl { px1 + 4.0 * ts } else { x + pad };
    text.draw(dl, &tl, lx, py0 - tl.height / 2.0, pal.text_muted);
    text.draw(dl, &bl, lx, py1 - bl.height, pal.text_muted);
    let (sx, ex) = if rtl {
        (px1 - start_l.width, px0)
    } else {
        (px0, px1 - end_l.width)
    };
    text.draw(dl, &start_l, sx, py1 + 2.0 * ts, pal.text_muted);
    text.draw(dl, &end_l, ex, py1 + 2.0 * ts, pal.text_muted);
    let map = |t: f64, v: f64| {
        let f = ((t - t0) / (t1 - t0)) as f32;
        let fx = if rtl { 1.0 - f } else { f };
        (
            px0 + fx * (px1 - px0),
            py1 - ((v - v0) / (v1 - v0)) as f32 * (py1 - py0),
        )
    };
    for (c, get) in [
        (MAX, (|p: &tic::Point| p.max) as fn(&tic::Point) -> f64),
        (MEAN, |p: &tic::Point| p.mean),
    ] {
        let line: Vec<_> = pts.iter().map(|p| map(p.t, get(p))).collect();
        for w2 in line.windows(2) {
            dl.line(w2[0], w2[1], 2.0 * ts, c);
        }
        for p in &line {
            dl.circle(*p, 2.5 * ts, c);
        }
    }
    r
}

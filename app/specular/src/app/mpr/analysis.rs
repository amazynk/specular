//! Read-only analysis over MPR (DESIGN.md §7: numbers and overlays the user asks for, no
//! findings): segmentations (DICOM SEG, RTSTRUCT) drawn over the planes, PET SUV under the
//! pointer, fusion of a second series of the same frame of reference with a manual rigid
//! nudge, and TMTV in a user-drawn sphere or box.
//!
//! When MPR opens, [`find`] looks on a thread through the series' folder, its parent and the
//! parent's other folders (a patient CD's or a download's layout) for segmentations whose
//! Frame of Reference UID matches the volume's (or that reference its series), and for one
//! series of the same frame of reference to fuse: PET or SPECT over CT or MR, or the other
//! way round. A PET series' SUV factors are read with it ([`Suv::read`], refused with the
//! reason when a tag is missing).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crossbeam_channel::{Receiver, bounded};
use specular_dicom::labels::Labels;
use specular_dicom::series::{self, Instance, Series};
use specular_dicom::suv::{Refusal, Suv};
use specular_dicom::tmtv::{self, Region, Threshold, Tmtv};
use specular_dicom::volume::Volume;
use specular_dicom::{Dicom, rtdose, rtstruct, seg};
use specular_gfx::{Gpu, wgpu};
use specular_i18n::t;
use specular_render::{VoiShape, Window as WlWindow};
use specular_render_volume::{
    Colormap, FusionRenderer, LabelLayer, LabelRenderer, LabelStyle, Plane, Rigid, VolumeTexture,
    reference,
};

use super::super::medical::{measure, number, range_window};
use super::load::{Loading, lock};
use super::views::{R, V3, Views, dot};
use crate::loader::Wake;

/// Files read per search, at most (headers only, except segmentations).
const MAX_FILES: usize = 20_000;
/// Folders beside the series' looked through, at most.
const MAX_SIBLINGS: usize = 64;

/// The segment list's set index of the isodose legend.
pub(crate) const DOSE_SET: usize = usize::MAX;

/// Fill opacities `Alt+O` cycles through.
pub(crate) const FILLS: [f32; 4] = [0.35, 0.6, 0.0, 0.15];
/// Fused series opacities `Shift+B` cycles through.
pub(crate) const BLENDS: [f32; 4] = [0.5, 0.7, 0.3, 1.0];
/// SUV windows (upper end; lower 0) for presets 1–3 on a PET volume.
pub(crate) const SUV_PRESETS: [f64; 3] = [5.0, 10.0, 20.0];
/// TMTV's colour (sRGB), and its thresholds.
const TMTV_COLOUR: [u8; 3] = [255, 214, 0];
const TMTV_SUV: f64 = 2.5;
const TMTV_SHARE: f64 = 0.41;

/// What [`find`] found.
#[derive(Default)]
pub(crate) struct Found {
    pub labels: Vec<Labels>,
    /// The series to fuse, if one shares the frame of reference.
    pub partner: Option<Series<PathBuf>>,
    /// The volume's own SUV (PET only), and the partner's.
    pub suv: Option<Result<Suv, Refusal>>,
    pub partner_suv: Option<Result<Suv, Refusal>>,
    /// An RT dose of the frame of reference, with its prescription and DVHs.
    pub dose: Option<super::dose::DoseFound>,
    /// Files whose headers were read, and the search's time.
    pub files: usize,
    pub ms: f64,
}

/// The SUV of the PET series whose file is `first`, the series' acquisition times of day
/// `times` giving its earliest acquisition.
pub(crate) fn suv_of(first: &Path, times: &[f64]) -> Result<Suv, Refusal> {
    let file = std::fs::read(first).map_err(|_| Refusal::NotPet)?;
    let d = Dicom::parse(&file).map_err(|_| Refusal::NotPet)?;
    let earliest = Suv::earliest_acquisition(&d, &file, times);
    Suv::read(&d, &file, earliest)
}

fn read_labels(path: &Path, inst: &Instance, cancel: &AtomicBool) -> Option<Labels> {
    let file = std::fs::read(path).ok()?;
    let d = Dicom::parse(&file).ok()?;
    let r = if seg::is_seg(&inst.sop_class) {
        seg::read(&d, &file, Some(cancel))
    } else {
        rtstruct::read(&d, &file)
    };
    match r {
        Ok(l) => Some(l),
        Err(e) => {
            specular_log::debug!(
                target: specular_log::DECODE,
                "segmentation {}: {e}",
                path.display()
            );
            None
        }
    }
}

/// The modalities fused over `modality`.
fn partners_of(modality: &str) -> &'static [&'static str] {
    match modality {
        "CT" | "MR" => &["PT", "NM"],
        "PT" | "NM" => &["CT", "MR"],
        _ => &[],
    }
}

/// Searches around `first` (a file of the volume's series, whose items' acquisition times
/// of day are `times`) for segmentations and a series to fuse. Stops early when `cancel`
/// is set.
pub(crate) fn find(first: &Path, times: &[f64], cancel: &AtomicBool) -> Found {
    let started = Instant::now();
    let mut out = Found::default();
    let Ok(Some(own)) = series::read_header(first) else {
        return out;
    };
    if own.modality == "PT" {
        out.suv = Some(suv_of(first, times));
    }
    let fr = own.frame_of_reference.clone();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = first.parent() {
        dirs.push(d.to_path_buf());
        if let Some(p) = d.parent() {
            dirs.push(p.to_path_buf());
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut subs: Vec<PathBuf> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|s| s.is_dir() && s != d)
                    .collect();
                subs.sort();
                subs.truncate(MAX_SIBLINGS);
                dirs.extend(subs);
            }
        }
    }
    let mut images: Vec<(PathBuf, Instance)> = Vec::new();
    let (mut doses, mut plans, mut structs) = (Vec::new(), Vec::new(), Vec::new());
    'dirs: for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for f in files {
            if cancel.load(Ordering::Relaxed) {
                return out;
            }
            out.files += 1;
            if out.files > MAX_FILES {
                break 'dirs;
            }
            let Ok(Some(i)) = series::read_header(&f) else {
                continue;
            };
            let is_seg = seg::is_seg(&i.sop_class);
            if is_seg || rtstruct::is_rtstruct(&i.sop_class) {
                // A SEG states its frame of reference at the top; an RTSTRUCT in a sequence.
                if is_seg
                    && !i.frame_of_reference.is_empty()
                    && i.frame_of_reference != fr
                    && !i.references.contains(&own.series_uid)
                {
                    continue;
                }
                if let Some(l) = read_labels(&f, &i, cancel)
                    && ((!fr.is_empty() && l.frame_of_reference == fr)
                        || l.referenced_series.contains(&own.series_uid))
                {
                    out.labels.push(l);
                    if !is_seg {
                        structs.push(f.clone());
                    }
                }
            } else if rtdose::is_rtdose(&i.sop_class) {
                if !fr.is_empty() && i.frame_of_reference == fr {
                    doses.push(f);
                }
            } else if rtdose::is_rtplan(&i.sop_class) {
                plans.push(f);
            } else if !fr.is_empty()
                && i.frame_of_reference == fr
                && i.series_uid != own.series_uid
                && i.has_image()
                && partners_of(&own.modality).contains(&i.modality.as_str())
            {
                images.push((f, i));
            }
        }
    }
    out.labels.sort_by(|a, b| {
        a.title
            .cmp(&b.title)
            .then(a.segments.len().cmp(&b.segments.len()))
    });
    if !doses.is_empty() && !cancel.load(Ordering::Relaxed) {
        out.dose = super::dose::read(&doses, &plans, &structs);
    }
    out.partner = series::assemble(images)
        .into_iter()
        .max_by_key(|s| s.items.len());
    if let Some(p) = &out.partner
        && p.modality == "PT"
        && let Some(item) = p.items.first()
    {
        let times: Vec<f64> = p
            .items
            .iter()
            .filter_map(|i| i.instance.acquisition_time)
            .collect();
        out.partner_suv = Some(suv_of(&item.key, &times));
    }
    out.ms = started.elapsed().as_secs_f64() * 1e3;
    out
}

/// One segmentation over the volume: its labels, their textures, and the segments hidden.
pub(crate) struct Set {
    pub labels: Labels,
    pub textures: Vec<VolumeTexture>,
    pub hidden: Vec<bool>,
}

/// The fused series.
pub(crate) struct Fusion {
    pub load: Loading,
    pub tex: Option<VolumeTexture>,
    /// The user's window (`None`: SUV 0–7 for PET, else the file's or the value range).
    pub window: Option<WlWindow>,
    pub colormap: Colormap,
    pub blend: usize,
    pub rigid: Rigid,
    pub suv: Option<f64>,
    pub modality: String,
    /// Shown (`Shift+U` hides it without dropping the volume).
    pub on: bool,
}

impl Fusion {
    pub(crate) fn current_window(&self) -> WlWindow {
        if let Some(w) = self.window {
            return w;
        }
        if let Some(f) = self.suv.filter(|f| *f > 0.0) {
            return suv_window(7.0, f, false);
        }
        let info = self.load.info.as_ref();
        info.and_then(|i| i.window).unwrap_or_else(|| {
            let (r, inv) = info.map_or(((0.0, 1.0), false), |i| (i.range, i.inverted));
            range_window(r, inv)
        })
    }

    /// The fused series' plane for a plane of the volume.
    pub(crate) fn plane(&self, fixed: &Plane) -> Plane {
        let mut p = self.rigid.plane(fixed);
        p.window = self.current_window();
        p
    }
}

/// The window SUV 0–`upper` for SUV factor `factor`.
pub(crate) fn suv_window(upper: f64, factor: f64, invert: bool) -> WlWindow {
    let top = upper / factor;
    WlWindow {
        center: top / 2.0,
        width: top,
        shape: VoiShape::Linear,
        invert,
    }
}

/// The TMTV tool's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Sphere,
    Box,
}

/// The TMTV tool: its shape (`None`: off), threshold, the drag and the last result.
#[derive(Default)]
pub(crate) struct TmtvTool {
    pub shape: Option<Shape>,
    /// 41 % of SUVmax instead of SUV 2.5.
    pub relative: bool,
    /// The view, the patient point pressed and the pointer now (logical).
    pub drag: Option<(usize, V3, (f64, f64))>,
    pub region: Option<Region>,
    pub result: Option<Tmtv>,
    pub tex: Option<VolumeTexture>,
    /// Measured on the fused series (its SUV), not the volume.
    pub fused: bool,
    /// Why there is no result.
    pub note: Option<String>,
}

impl TmtvTool {
    pub(crate) fn threshold(&self) -> Threshold {
        if self.relative {
            Threshold::OfMax(TMTV_SHARE)
        } else {
            Threshold::Suv(TMTV_SUV)
        }
    }
}

/// The session's analysis state.
#[derive(Default)]
pub(crate) struct Analysis {
    search: Option<Search>,
    /// The search ended.
    pub searched: bool,
    pub sets: Vec<Set>,
    /// Segmentations drawn (`Shift+O`), and the fill opacity's index in [`FILLS`].
    pub hide: bool,
    pub fill: usize,
    pub suv: Option<Result<Suv, Refusal>>,
    pub partner: Option<Series<PathBuf>>,
    pub partner_suv: Option<Result<Suv, Refusal>>,
    pub fusion: Option<Fusion>,
    /// The arrow keys move the fused series.
    pub nudge: bool,
    pub tmtv: TmtvTool,
    pub labels_r: Option<LabelRenderer>,
    pub fusion_r: Option<FusionRenderer>,
    /// The RT dose over the planes.
    pub dose: Option<super::dose::Dose>,
    /// Upload failures (shown in the panel).
    pub error: Option<String>,
}

/// The search's thread: its result, and its cancel flag (set when dropped).
struct Search {
    rx: Receiver<Found>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Search {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Analysis {
    /// The GPU device was replaced: the segmentations, fused series, TMTV voxels and dose
    /// are uploaded to `gpu` again and the renderers made again when next drawn.
    pub(crate) fn gpu_reset(&mut self, gpu: &Gpu) {
        self.labels_r = None;
        self.fusion_r = None;
        for s in &mut self.sets {
            match upload(gpu, &s.labels) {
                Ok(t) => s.textures = t,
                Err(e) => {
                    s.textures.clear();
                    self.error = Some(e);
                }
            }
        }
        if let Some(f) = self.fusion.as_mut().filter(|f| f.tex.is_some()) {
            f.tex = None;
            match upload_layer(gpu, &lock(&f.load.volume)) {
                Ok(t) => f.tex = Some(t),
                Err(e) => self.error = Some(e),
            }
        }
        self.tmtv.tex = None;
        if let Some(layer) = self
            .tmtv
            .result
            .as_ref()
            .and_then(|r| r.labels.layers.first())
        {
            match upload_layer(gpu, &layer.volume) {
                Ok(t) => self.tmtv.tex = Some(t),
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(d) = self.dose.as_mut()
            && let Err(e) = d.gpu_reset(gpu)
        {
            self.error = Some(e);
        }
    }

    /// Starts the search around `first` (`times`: the series' acquisition times of day).
    pub(crate) fn start(first: PathBuf, times: Vec<f64>, wake: Wake) -> Analysis {
        let (tx, rx) = bounded(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let _ = specular_thread::builder("mpr-analysis").spawn(move || {
            // A panic leaves the volume without overlays instead of "searching" forever.
            let found = specular_core::isolate::run("mpr analysis", || find(&first, &times, &c))
                .unwrap_or_default();
            let _ = tx.send(found);
            wake();
        });
        Analysis {
            search: Some(Search { rx, cancel }),
            ..Analysis::default()
        }
    }

    /// The search beside the volume is still running.
    pub(crate) fn searching(&self) -> bool {
        self.search.is_some()
    }

    /// Takes the search's result: uploads the segmentations. True when something changed.
    pub(crate) fn poll(&mut self, gpu: &Gpu) -> bool {
        let Some(search) = &self.search else {
            return false;
        };
        let Ok(found) = search.rx.try_recv() else {
            return false;
        };
        self.search = None;
        self.searched = true;
        specular_log::info!(
            target: specular_log::DECODE,
            "mpr analysis: {} segmentations, fusion {}, dose {}, {} files in {:.0} ms",
            found.labels.len(),
            found.partner.as_ref().map_or("none", |p| p.modality.as_str()),
            found.dose.as_ref().map_or(0, |d| d.dvhs.len() + 1),
            found.files,
            found.ms
        );
        for labels in found.labels {
            match upload(gpu, &labels) {
                Ok(textures) => {
                    let n = labels.segments.len();
                    self.sets.push(Set {
                        labels,
                        textures,
                        hidden: vec![false; n],
                    });
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(d) = found.dose {
            match super::dose::Dose::new(gpu, d) {
                Ok(d) => self.dose = Some(d),
                Err(e) => self.error = Some(e),
            }
        }
        self.suv = found.suv;
        self.partner = found.partner;
        self.partner_suv = found.partner_suv;
        true
    }

    /// The volume's SUVbw factor.
    pub(crate) fn suv_factor(&self) -> Option<f64> {
        self.suv
            .as_ref()
            .and_then(|s| s.as_ref().ok())
            .map(|s| s.bw)
    }

    /// Every segment, in draw order: (set, segment).
    pub(crate) fn segments(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.sets
            .iter()
            .enumerate()
            .flat_map(|(i, s)| (0..s.labels.segments.len()).map(move |k| (i, k)))
    }

    /// Flips segment `k` of set `i`.
    pub(crate) fn toggle(&mut self, i: usize, k: usize) {
        if i == DOSE_SET {
            if let Some(d) = self.dose.as_mut() {
                d.toggle(k);
            }
            return;
        }
        if let Some(h) = self.sets.get_mut(i).and_then(|s| s.hidden.get_mut(k)) {
            *h = !*h;
        }
    }

    /// The colours of every set (then TMTV's), and each set's first index.
    fn style(&self) -> (LabelStyle, Vec<usize>) {
        let mut colours = Vec::new();
        let mut bases = Vec::new();
        for s in &self.sets {
            bases.push(colours.len());
            for (k, seg) in s.labels.segments.iter().enumerate() {
                let on = !self.hide && !s.hidden[k];
                colours.push(LabelStyle::colour(seg.colour, if on { 1.0 } else { 0.0 }));
            }
        }
        bases.push(colours.len());
        colours.push(LabelStyle::colour(TMTV_COLOUR, 1.0));
        (
            LabelStyle {
                colours,
                fill: FILLS[self.fill % FILLS.len()],
                outline: 1.5,
            },
            bases,
        )
    }

    /// Draws the fused series and the segmentations over `planes` (after the volume).
    pub(crate) fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        planes: &[Plane],
        scale: f64,
    ) {
        if let Some(f) = self.fusion.as_ref().filter(|f| f.on)
            && let Some(tex) = &f.tex
        {
            let moved: Vec<Plane> = planes.iter().map(|p| f.plane(p)).collect();
            let r = self.fusion_r.get_or_insert_with(FusionRenderer::new);
            r.draw(
                gpu,
                enc,
                target,
                size,
                tex,
                f.colormap,
                BLENDS[f.blend % BLENDS.len()],
                &moved,
            );
        }
        if let Some(d) = self.dose.as_mut() {
            d.draw(gpu, enc, target, size, planes, scale);
        }
        let (mut style, bases) = self.style();
        style.outline = (1.5 * scale) as f32;
        let mut layers = Vec::new();
        for (s, base) in self.sets.iter().zip(&bases) {
            for (t, l) in s.textures.iter().zip(&s.labels.layers) {
                layers.push(LabelLayer {
                    texture: t,
                    fraction: l.fraction,
                    base: *base,
                });
            }
        }
        if let Some(t) = &self.tmtv.tex {
            layers.push(LabelLayer {
                texture: t,
                fraction: None,
                base: *bases.last().expect("TMTV's base"),
            });
        }
        if layers.is_empty() {
            return;
        }
        let r = self.labels_r.get_or_insert_with(LabelRenderer::new);
        r.draw(gpu, enc, target, size, &layers, &style, planes);
    }

    /// Readout lines for patient point `p` (`value`: the volume's value there).
    pub(crate) fn readout(
        &self,
        value: Option<f32>,
        p: V3,
        plane: Option<&Plane>,
        at: (f64, f64),
    ) -> Vec<String> {
        let mut out = Vec::new();
        if let (Some(v), Some(f)) = (value, self.suv_factor()) {
            out.push(t!(
                "suv-value",
                value = specular_i18n::fixed(f64::from(v) * f, 2)
            ));
        }
        if let Some(f) = self.fusion.as_ref().filter(|f| f.on && f.tex.is_some())
            && let Some(plane) = plane
        {
            let vol = lock(&f.load.volume);
            if let Some(x) = reference(&vol, &f.plane(plane), at.0, at.1) {
                let x = f64::from(x);
                out.push(match f.suv {
                    Some(s) => t!(
                        "fusion-value-suv",
                        modality = f.modality.clone(),
                        value = specular_i18n::fixed(x * s, 2)
                    ),
                    None => t!(
                        "fusion-value",
                        modality = f.modality.clone(),
                        value = number(x.round())
                    ),
                });
            }
        }
        if let (Some(d), Some(plane)) = (&self.dose, plane)
            && let Some(line) = d.readout(plane, at)
        {
            out.push(line);
        }
        for s in &self.sets {
            for k in s.labels.at(p) {
                if !self.hide && !s.hidden[k] {
                    out.push(s.labels.segments[k].label.clone());
                }
            }
        }
        out
    }

    /// The panel's lines: SUV, fusion, TMTV, the segmentations' notes.
    pub(crate) fn panel(&self) -> Vec<String> {
        let mut out = Vec::new();
        match &self.suv {
            Some(Ok(s)) => out.push(t!(
                "suv-factor",
                factor = format!("{:.3e}", s.bw),
                minutes = number((s.delay_s / 60.0).round())
            )),
            Some(Err(r)) => out.push(t!(
                "suv-refused",
                reason = specular_i18n::I18n::global().tr(r.key())
            )),
            None => {}
        }
        if let Some(f) = &self.fusion {
            let w = f.current_window();
            let mut line = t!(
                "fusion-on",
                modality = f.modality.clone(),
                colormap =
                    specular_i18n::I18n::global().tr(&format!("colormap-{}", f.colormap.id())),
                blend = number((BLENDS[f.blend % BLENDS.len()] * 100.0).round().into())
            );
            if !f.on {
                line = t!("fusion-hidden", modality = f.modality.clone());
            }
            out.push(line);
            if let Some(s) = f.suv {
                let lo = (w.center - w.width / 2.0) * s;
                let hi = (w.center + w.width / 2.0) * s;
                out.push(t!(
                    "fusion-window-suv",
                    low = specular_i18n::fixed(lo, 1),
                    high = specular_i18n::fixed(hi, 1)
                ));
            }
            if let Some(i) = &f.load.info
                && f.load.done < i.slices
            {
                out.push(t!(
                    "mpr-loading",
                    loaded = f.load.done as i64,
                    count = i.slices as i64
                ));
            }
            if !f.rigid.is_identity() {
                let t = f.rigid.translate;
                let r = f.rigid.rotate_deg;
                out.push(t!(
                    "fusion-moved",
                    x = specular_i18n::fixed(t[0], 0),
                    y = specular_i18n::fixed(t[1], 0),
                    z = specular_i18n::fixed(t[2], 0),
                    degrees = specular_i18n::fixed(r[0].abs().max(r[1].abs()).max(r[2].abs()), 0)
                ));
            }
            if self.nudge {
                out.push(t!("fusion-nudge-hint"));
            }
        }
        if let Some(d) = &self.dose {
            out.extend(d.panel());
        }
        if let Some(s) = self.tmtv.shape {
            out.push(match s {
                Shape::Sphere => t!("tmtv-hint-sphere"),
                Shape::Box => t!("tmtv-hint-box"),
            });
            out.push(if self.tmtv.relative {
                t!("tmtv-threshold-share", share = 41)
            } else {
                t!("tmtv-threshold-suv", suv = "2.5")
            });
        }
        if let Some(r) = &self.tmtv.result {
            out.push(t!(
                "tmtv-result",
                ml = specular_i18n::fixed(r.ml, 1),
                count = r.groups as i64
            ));
            out.push(t!(
                "tmtv-stats",
                max = specular_i18n::fixed(r.suv_max, 2),
                mean = specular_i18n::fixed(r.suv_mean, 2),
                tlg = specular_i18n::fixed(r.tlg, 1)
            ));
        } else if let Some(n) = &self.tmtv.note {
            out.push(n.clone());
        }
        if let Some(e) = &self.error {
            out.push(t!("segments-failed", reason = e.clone()));
        }
        if self.hide && !self.sets.is_empty() {
            out.push(t!("segments-hidden"));
        }
        out
    }

    /// The segment list for the panel: (set, segment, name with its volume, colour, shown).
    pub(crate) fn segment_list(&self) -> Vec<(usize, usize, String, [u8; 3], bool)> {
        self.segments()
            .map(|(i, k)| {
                let s = &self.sets[i];
                let seg = &s.labels.segments[k];
                let ml = if seg.ml >= 100.0 {
                    specular_i18n::fixed(seg.ml, 0)
                } else {
                    specular_i18n::fixed(seg.ml, 1)
                };
                (
                    i,
                    k,
                    t!("segment-entry", name = seg.label.clone(), ml = ml),
                    seg.colour,
                    !self.hide && !s.hidden[k],
                )
            })
            .chain(self.dose.iter().flat_map(|d| {
                d.legend()
                    .into_iter()
                    .map(|(k, name, c, on)| (DOSE_SET, k, name, c, on))
            }))
            .collect()
    }

    /// The TMTV region (and its drag) as drawn outlines in view `v`.
    pub(crate) fn tmtv_drawn(&self, views: &Views, v: usize, r: R) -> Vec<measure::Drawn> {
        let mut out = Vec::new();
        let region = match self.tmtv.drag {
            Some((dv, a, now)) if dv == v => self.tmtv_region(views, v, r, a, now),
            _ => self.tmtv.region,
        };
        let Some(region) = region else {
            return out;
        };
        let n = views.normal(v);
        let lines = |closed: Vec<(f32, f32)>| measure::Drawn {
            outline: closed,
            closed: true,
            handles: Vec::new(),
            lines: Vec::new(),
            draft: self.tmtv.drag.is_some(),
        };
        match region {
            Region::Sphere { centre, radius } => {
                let d = dot([0, 1, 2].map(|i| centre[i] - views.c[i]), n);
                if d.abs() >= radius {
                    return out;
                }
                let rr = (radius * radius - d * d).sqrt() / views.mm(v, r);
                let c = views.to_screen(v, r, centre);
                let pts = (0..48)
                    .map(|i| {
                        let a = f64::from(i) / 48.0 * std::f64::consts::TAU;
                        ((c.0 + rr * a.cos()) as f32, (c.1 + rr * a.sin()) as f32)
                    })
                    .collect();
                out.push(lines(pts));
            }
            Region::Box { a, b } => {
                let corners: Vec<V3> = (0..8)
                    .map(|i| {
                        [
                            if i & 1 == 0 { a[0] } else { b[0] },
                            if i & 2 == 0 { a[1] } else { b[1] },
                            if i & 4 == 0 { a[2] } else { b[2] },
                        ]
                    })
                    .collect();
                let ds: Vec<f64> = corners
                    .iter()
                    .map(|k| dot([0, 1, 2].map(|i| k[i] - views.c[i]), n))
                    .collect();
                let (lo, hi) = ds
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(l, h), d| (l.min(*d), h.max(*d)));
                if lo > 0.0 || hi < 0.0 {
                    return out;
                }
                let s: Vec<(f64, f64)> =
                    corners.iter().map(|k| views.to_screen(v, r, *k)).collect();
                let (x0, x1) = s
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.0), h.max(p.0)));
                let (y0, y1) = s
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.1), h.max(p.1)));
                let (x0, x1, y0, y1) = (x0 as f32, x1 as f32, y0 as f32, y1 as f32);
                out.push(lines(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]));
            }
        }
        out
    }

    /// The region dragged in view `v` from patient point `a` to logical point `now`.
    pub(crate) fn tmtv_region(
        &self,
        views: &Views,
        v: usize,
        r: R,
        a: V3,
        now: (f64, f64),
    ) -> Option<Region> {
        let b = views.to_patient(v, r, now);
        let d: f64 = (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt();
        if d < 1.0 {
            return None;
        }
        Some(match self.tmtv.shape? {
            Shape::Sphere => Region::Sphere {
                centre: a,
                radius: d,
            },
            Shape::Box => {
                // The rectangle drawn, as deep as its larger side, centred on the plane.
                let f = &views.frames[v];
                let w = dot([0, 1, 2].map(|i| b[i] - a[i]), f.right).abs();
                let h = dot([0, 1, 2].map(|i| b[i] - a[i]), f.down).abs();
                let half = w.max(h) / 2.0;
                let n = views.normal(v);
                let pts = [
                    [0, 1, 2].map(|i| a[i] + n[i] * half),
                    [0, 1, 2].map(|i| a[i] - n[i] * half),
                    [0, 1, 2].map(|i| b[i] + n[i] * half),
                    [0, 1, 2].map(|i| b[i] - n[i] * half),
                ];
                let lo = [0, 1, 2].map(|i| pts.iter().map(|p| p[i]).fold(f64::MAX, f64::min));
                let hi = [0, 1, 2].map(|i| pts.iter().map(|p| p[i]).fold(f64::MIN, f64::max));
                Region::Box { a: lo, b: hi }
            }
        })
    }

    /// Measures the region on the PET: the volume when it is PET with an SUV, else the
    /// fused series when that is. Uploads the voxels counted for drawing.
    pub(crate) fn tmtv_measure(&mut self, gpu: &Gpu, volume: &Volume, region: Region) {
        self.tmtv.region = Some(region);
        self.tmtv.result = None;
        self.tmtv.tex = None;
        self.tmtv.note = None;
        let threshold = self.tmtv.threshold();
        let started = Instant::now();
        let measured = if let Some(f) = self.suv_factor() {
            self.tmtv.fused = false;
            Ok(tmtv::measure(volume, f, &region, threshold))
        } else if let Some(fu) = self.fusion.as_ref().filter(|f| f.tex.is_some())
            && let Some(s) = fu.suv
        {
            self.tmtv.fused = true;
            let moved = match region {
                Region::Sphere { centre, radius } => Region::Sphere {
                    centre: fu.rigid.to_moving(centre),
                    radius,
                },
                Region::Box { a, b } => {
                    let (a, b) = (fu.rigid.to_moving(a), fu.rigid.to_moving(b));
                    Region::Box {
                        a: [0, 1, 2].map(|i| a[i].min(b[i])),
                        b: [0, 1, 2].map(|i| a[i].max(b[i])),
                    }
                }
            };
            let vol = lock(&fu.load.volume);
            Ok(tmtv::measure(&vol, s, &moved, threshold))
        } else {
            Err(self.no_suv_reason())
        };
        match measured {
            Ok(Some(r)) => {
                specular_log::info!(
                    target: specular_log::DECODE,
                    "tmtv: {:.1} mL in {} groups, {} region voxels, {:.1} ms",
                    r.ml,
                    r.groups,
                    r.region_voxels,
                    started.elapsed().as_secs_f64() * 1e3
                );
                if let Some(layer) = r.labels.layers.first() {
                    match upload_layer(gpu, &layer.volume) {
                        Ok(t) => self.tmtv.tex = Some(t),
                        Err(e) => self.error = Some(e),
                    }
                }
                self.tmtv.result = Some(r);
            }
            Ok(None) => self.tmtv.note = Some(t!("tmtv-outside")),
            Err(e) => self.tmtv.note = Some(e),
        }
    }

    /// Why TMTV and SUV presets have no SUV to work with.
    pub(crate) fn no_suv_reason(&self) -> String {
        match (&self.suv, &self.partner_suv) {
            (Some(Err(r)), _) | (None, Some(Err(r))) => t!(
                "suv-refused",
                reason = specular_i18n::I18n::global().tr(r.key())
            ),
            _ if !self.searched => t!("analysis-searching"),
            _ => t!("tmtv-no-pet"),
        }
    }

    /// The SUV factor that presets apply to, and whether it is the fused series'.
    pub(crate) fn preset_suv(&self) -> Option<(f64, bool)> {
        if let Some(f) = self.suv_factor() {
            return Some((f, false));
        }
        self.fusion
            .as_ref()
            .filter(|f| f.on)
            .and_then(|f| f.suv)
            .map(|s| (s, true))
    }

    /// Polls the fused series' loading: makes and fills its texture. A message on failure.
    pub(crate) fn tick_fusion(&mut self, gpu: &Gpu) -> (bool, Option<String>) {
        let Some(f) = self.fusion.as_mut() else {
            return (false, None);
        };
        let polled = f.load.poll();
        if let Some(e) = polled.refused {
            self.fusion = None;
            return (true, Some(t!("fusion-refused", reason = e)));
        }
        if polled.opened {
            let vol = lock(&f.load.volume);
            match VolumeTexture::new(gpu, &vol) {
                Ok(t) => f.tex = Some(t),
                Err(e) => {
                    drop(vol);
                    self.fusion = None;
                    return (true, Some(t!("mpr-no-gpu", reason = e)));
                }
            }
        }
        let mut changed = polled.opened;
        if f.load.dirty
            && let Some(tex) = f.tex.as_mut()
        {
            f.load.dirty = false;
            let vol = lock(&f.load.volume);
            match tex.sync(gpu, &vol) {
                Ok(c) => changed |= c,
                Err(e) => self.error = Some(e),
            }
        }
        (changed, None)
    }

    /// Moves the fused series: `d` mm along the view's right, down and normal, then turns it
    /// by `turn` degrees about the view's normal (about the patient axis nearest to it).
    pub(crate) fn nudge(&mut self, views: &Views, v: usize, d: [f64; 3], turn: f64) {
        let Some(f) = self.fusion.as_mut() else {
            return;
        };
        let fr = &views.frames[v];
        let n = views.normal(v);
        for (i, t) in f.rigid.translate.iter_mut().enumerate() {
            *t += fr.right[i] * d[0] + fr.down[i] * d[1] + n[i] * d[2];
        }
        if turn != 0.0 {
            let axis = (0..3)
                .max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs()))
                .unwrap_or(2);
            f.rigid.rotate_deg[axis] += turn * n[axis].signum();
        }
    }
}

/// Makes and fills the textures of a segmentation's layers.
pub(crate) fn upload(gpu: &Gpu, labels: &Labels) -> Result<Vec<VolumeTexture>, String> {
    labels
        .layers
        .iter()
        .map(|l| upload_layer(gpu, &l.volume))
        .collect()
}

fn upload_layer(gpu: &Gpu, vol: &Volume) -> Result<VolumeTexture, String> {
    let mut t = VolumeTexture::new(gpu, vol)?;
    t.sync(gpu, vol)?;
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> Option<PathBuf> {
        let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/dicom")
            });
        p.is_dir().then_some(p)
    }

    #[test]
    fn finds_structures_and_ct_for_the_tcia_pet() {
        let Some(root) = corpus().filter(|r| r.join("tcia-sts/pt").is_dir()) else {
            eprintln!("TCIA corpus absent; skipping");
            return;
        };
        let first = root.join("tcia-sts/pt/00000001.dcm");
        let found = find(&first, &[], &AtomicBool::new(false));
        eprintln!(
            "{} files, {} segmentations, {:.0} ms",
            found.files,
            found.labels.len(),
            found.ms
        );
        assert!(matches!(found.suv, Some(Ok(_))));
        let p = found.partner.expect("the CT shares the frame of reference");
        assert_eq!(p.modality, "CT");
        assert!(p.items.len() > 100);
        assert!(!found.labels.is_empty(), "the PET's RTSTRUCT");
        assert!(found.labels.iter().all(|l| !l.segments.is_empty()));
    }

    #[test]
    fn finds_the_liver_segmentation_beside_its_ct() {
        let Some(root) = corpus().filter(|r| r.join("dcmqi/data/segmentations/ct-3slice").is_dir())
        else {
            eprintln!("dcmqi corpus absent; skipping");
            return;
        };
        let first = root.join("dcmqi/data/segmentations/ct-3slice/01.dcm");
        let found = find(&first, &[], &AtomicBool::new(false));
        eprintln!(
            "{} files, {:?}, {:.0} ms",
            found.files,
            found
                .labels
                .iter()
                .map(|l| l.title.clone())
                .collect::<Vec<_>>(),
            found.ms
        );
        assert!(!found.labels.is_empty());
        assert!(found.suv.is_none());
        assert!(found.partner.is_none());
    }

    #[test]
    fn a_cancelled_search_stops() {
        let Some(root) = corpus().filter(|r| r.join("tcia-sts/pt").is_dir()) else {
            return;
        };
        let found = find(
            &root.join("tcia-sts/pt/00000001.dcm"),
            &[],
            &AtomicBool::new(true),
        );
        assert!(found.partner.is_none() && found.labels.is_empty());
    }
}

//! Loading a series into a [`Volume`] off the UI thread: one thread stacks the series
//! ([`volume::stack`], which refuses what would not be a faithful volume), decodes its first
//! slice for the voxel kind, size and spacing, plans the volume within the GPU's 3D texture
//! limit and the memory budget ([`Plan`]: every n-th pixel or slice when over), then queues
//! every slice on the decode pool in [`load_order`] (every n-th slice first). Workers write
//! their slices into the shared volume and wake the UI, which uploads what arrived
//! ([`VolumeTexture::sync`](specular_render_volume::VolumeTexture::sync)). Closing cancels
//! the queued slices.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use specular_core::{CancelToken, JobPool, Priority};
use specular_decode::science::Seismic;
use specular_dicom::series::{self, Series};
use specular_dicom::volume::{self, Plan, Volume, VoxelKind, load_order};
use specular_dicom::{Dicom, ModalityFrame};
use specular_render::Window as WlWindow;

use crate::loader::Wake;

/// What the volume is made of.
pub(crate) enum Source {
    /// A series found by series mode.
    Series(Series<PathBuf>),
    /// One file (an enhanced multi-frame image, its frames the slices).
    File(PathBuf),
    /// A research volume (NIfTI, Analyze, NRRD, MetaImage) and which of its volumes (time
    /// points): its slices in the displayed (canonical) order. With voxels another reader
    /// made ([`Prepared`]), those in place of the file's.
    Research(PathBuf, usize, Option<Prepared>),
    /// A 3-D seismic volume (SEG-Y, Seismic Unix) and what its sections are: time (or
    /// depth) slices stacked bottom up, crosslines across and inlines down in each.
    Seismic(PathBuf, Arc<Seismic>),
    /// A scientific image's planes along its z axis (a microscopy z-stack, an EM tomogram,
    /// a FITS, HDF5 or Zarr cube) at the other axes' current positions.
    Science(
        Arc<dyn specular_decode::Session>,
        Arc<super::science_vol::Stack>,
    ),
}

/// The unit of the volume's lengths (positions, slabs, measurements).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Length {
    /// Patient millimetres (DICOM, research and industrial CT volumes, seismic indices).
    #[default]
    Mm,
    Micron,
    Nanometre,
    /// Density maps and cube grids.
    Angstrom,
    /// No voxel size known: voxel edges (shown as cubes).
    Voxel,
}

impl Length {
    /// The unit's symbol, `None` for voxels.
    pub(crate) fn symbol(self) -> Option<&'static str> {
        match self {
            Length::Mm => Some("mm"),
            Length::Micron => Some("\u{b5}m"),
            Length::Nanometre => Some("nm"),
            Length::Angstrom => Some("\u{c5}"),
            Length::Voxel => None,
        }
    }
}

/// Voxels a research volume takes from elsewhere than its file, which another reader (CIFTI)
/// does not keep as a plain volume.
pub(crate) enum Prepared {
    /// Made when the file was opened (a CIFTI file's volume).
    Volume(Arc<specular_decode::medvol::MedVolume>),
    /// A dense connectivity seed's row, read when MPR opens on it.
    Seed(Arc<specular_decode::neuro_dec::Connectivity>, u32),
}

/// What the first slice says about the whole volume.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Info {
    pub plan: Plan,
    /// Slices queued (the volume's), and the stack's full size (columns, rows, slices).
    pub slices: usize,
    pub full: [u32; 3],
    pub unit: Option<String>,
    /// The file's default window, its value range, whether it shows inverted.
    pub window: Option<WlWindow>,
    pub range: (f64, f64),
    pub inverted: bool,
    /// The stack's slices are unevenly spaced or tilted (kept exact, no resampling).
    pub irregular: bool,
    pub tilt: Option<f64>,
    /// A seismic volume: its views are time, inline and crossline sections, and its
    /// patient coordinates are crossline, inline and (bottom up) sample indices.
    pub seismic: Option<Arc<Seismic>>,
    /// The unit lengths are in (mm for patient volumes).
    pub length: Length,
    /// The volume's axes have patient directions (DICOM, a NIfTI sform or qform): the views
    /// are axial, coronal and sagittal with orientation letters; otherwise XY, XZ and YZ.
    pub oriented: bool,
    /// Lines the panel adds: the channel and time point shown, a voxel size assumed.
    pub notes: Vec<String>,
    /// The dental CBCT maker the series' Manufacturer names, if one is recognised.
    pub vendor: Option<&'static str>,
    /// Whose anatomy the orientation letters name (DICOM Anatomical Orientation Type).
    pub anatomy: specular_dicom::Anatomy,
}

enum Msg {
    Opened(Result<Info, String>),
    Slice(Result<(), String>),
}

/// A volume being loaded.
pub(crate) struct Loading {
    pub volume: Arc<Mutex<Volume>>,
    rx: Receiver<Msg>,
    cancel: CancelToken,
    pub info: Option<Info>,
    /// Slices written (or failed), and the first failure.
    pub done: usize,
    pub failed: usize,
    pub error: Option<String>,
    /// Slices arrived since the last upload.
    pub dirty: bool,
    pub started: Instant,
    /// When the first slice was shown and when the last was uploaded.
    pub first_view: Option<Duration>,
    pub complete: Option<Duration>,
    /// The voxels' share of the memory budget, given back when the volume goes.
    #[cfg_attr(not(test), allow(dead_code))]
    memory: Arc<Mutex<Option<specular_core::Reservation>>>,
    /// Slices still queued (cancelled ones included), for tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pending: Arc<AtomicUsize>,
    /// The pool the slices are queued on, kept while they load (a pool dropped drops its
    /// queue).
    _pool: Arc<JobPool>,
}

/// One slice job queued: counted in [`Loading::pending`] until it runs or the pool drops it
/// cancelled.
struct Queued(Arc<AtomicUsize>);

impl Drop for Queued {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Locks the volume (a worker that panicked leaves it as it was).
pub(crate) fn lock(v: &Mutex<Volume>) -> MutexGuard<'_, Volume> {
    v.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What [`Loading::poll`] found.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Polled {
    /// The volume's size is known (the first slice is in).
    pub opened: bool,
    /// It could not be loaded at all: why.
    pub refused: Option<String>,
    pub slices: usize,
}

impl Loading {
    /// Starts loading `source`; `max_dim` is the largest 3D texture side, `budget` the bytes
    /// the voxels may take. `wake` is called whenever there is something to poll.
    pub(crate) fn start(
        source: Source,
        pool: Arc<JobPool>,
        wake: Wake,
        max_dim: u32,
        budget: u64,
    ) -> Loading {
        let (tx, rx) = unbounded();
        let cancel = CancelToken::new();
        let empty = Volume::new(
            volume::Geometry::new(
                &[[0.0; 3]],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0],
                [1, 1],
            ),
            VoxelKind::I16,
            1,
        )
        .expect("one voxel");
        let volume = Arc::new(Mutex::new(empty));
        let pending = Arc::new(AtomicUsize::new(0));
        let memory = Arc::new(Mutex::new(None));
        let job = Opener {
            source,
            pool: pool.clone(),
            wake,
            tx,
            cancel: cancel.clone(),
            volume: volume.clone(),
            pending: pending.clone(),
            max_dim,
            budget,
            memory: memory.clone(),
        };
        let _ = specular_thread::builder("mpr-open").spawn(move || job.run());
        Loading {
            volume,
            rx,
            cancel,
            info: None,
            done: 0,
            failed: 0,
            error: None,
            dirty: false,
            started: Instant::now(),
            first_view: None,
            complete: None,
            memory,
            pending,
            _pool: pool,
        }
    }

    /// Takes what the workers sent.
    pub(crate) fn poll(&mut self) -> Polled {
        let mut out = Polled::default();
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Opened(Ok(info)) => {
                    // The opener wrote the first slice itself (workers may have
                    // reported before this message).
                    self.done += 1;
                    self.info = Some(info);
                    self.dirty = true;
                    out.opened = true;
                }
                Msg::Opened(Err(e)) => out.refused = Some(e),
                Msg::Slice(r) => {
                    self.done += 1;
                    out.slices += 1;
                    if let Err(e) = r {
                        self.failed += 1;
                        self.error.get_or_insert(e);
                    }
                    self.dirty = true;
                }
            }
        }
        out
    }

    /// Every slice queued has been written or has failed.
    pub(crate) fn finished(&self) -> bool {
        self.info.as_ref().is_some_and(|i| self.done >= i.slices)
    }

    /// Stops the slices still queued.
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Bytes of the memory budget the voxels hold.
    #[cfg(test)]
    pub(crate) fn reserved(&self) -> u64 {
        self.memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or(0, specular_core::Reservation::bytes)
    }

    /// Slices queued and not yet started or finished.
    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }
}

impl Drop for Loading {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

struct Opener {
    source: Source,
    pool: Arc<JobPool>,
    wake: Wake,
    tx: Sender<Msg>,
    cancel: CancelToken,
    volume: Arc<Mutex<Volume>>,
    pending: Arc<AtomicUsize>,
    max_dim: u32,
    budget: u64,
    memory: Arc<Mutex<Option<specular_core::Reservation>>>,
}

/// A file read and parsed once, shared by the jobs decoding its frames.
type Shared = Arc<(Vec<u8>, Dicom)>;

fn read(path: &PathBuf) -> Result<Shared, String> {
    let bytes = std::fs::read(path).map_err(|e| crate::error_text::io_reason(&e))?;
    let d = Dicom::parse(&bytes).map_err(|e| crate::error_text::dicom_reason(&e))?;
    Ok(Arc::new((bytes, d)))
}

fn frame(file: &Shared, index: u32, cancel: &CancelToken) -> Result<ModalityFrame, String> {
    file.1
        .modality_frame(&file.0, index, Some(cancel.flag()))
        .map_err(|e| crate::error_text::dicom_reason(&e))
}

impl Opener {
    fn run(self) {
        // A panic reading the series fails the opening (the view says why) instead of
        // leaving it "opening" forever.
        let r = specular_core::isolate::run("mpr open", || self.open())
            .unwrap_or_else(|_| Err(specular_i18n::t!("error-reason-internal")));
        let _ = self.tx.send(Msg::Opened(r));
        (self.wake)();
    }

    /// The plan for a stack of `size` voxels of `kind`, `pixel` and `spacing` apart, within
    /// the texture limit and the budget, with its voxels' share of the memory budget held
    /// (the reason, in free RAM, when even a downsampled volume does not fit).
    fn plan(
        &self,
        size: [u32; 3],
        kind: VoxelKind,
        pixel: f64,
        spacing: f64,
    ) -> Result<Plan, String> {
        let plan = Plan::new(size, kind, pixel, spacing, self.max_dim, self.budget).map_err(
            |e| match e {
                volume::Refusal::TooLarge { need, .. } => {
                    let free = specular_core::budget::free().unwrap_or(self.budget);
                    needs_memory(need, free)
                }
                e => crate::error_text::refusal_reason(&e),
            },
        )?;
        let r = specular_core::budget::reserve(plan.bytes)
            .map_err(crate::error_text::needs_memory_reason)?;
        *lock_memory(&self.memory) = Some(r);
        Ok(plan)
    }

    fn open(&self) -> Result<Info, String> {
        let set = match &self.source {
            Source::Research(p, v, prep) => return self.open_research(p, *v, prep.as_ref()),
            Source::Seismic(p, s) => return self.open_seismic(p, s),
            Source::Science(session, stack) => return self.open_science(session, stack),
            Source::Series(s) => s.clone(),
            Source::File(p) => {
                let inst = series::read_header(p)
                    .map_err(|e| crate::error_text::dicom_reason(&e))?
                    .ok_or_else(|| {
                        crate::error_text::refusal_reason(&volume::Refusal::NoGeometry)
                    })?;
                series::assemble(vec![(p.clone(), inst)])
                    .into_iter()
                    .next()
                    .ok_or_else(|| crate::error_text::refusal_reason(&volume::Refusal::TooFew(0)))?
            }
        };
        let st = volume::stack(&set).map_err(|e| crate::error_text::refusal_reason(&e))?;
        // One file for every slice (a multi-frame image): read and parsed once.
        let single = st.slices.windows(2).all(|w| w[0].key == w[1].key);
        let first = &st.slices[0];
        let file0 = read(&first.key)?;
        if self.cancel.is_cancelled() {
            return Err(String::new());
        }
        let f0 = frame(&file0, first.frame, &self.cancel)?;
        // A dental CBCT export, by its Manufacturer (0008,0070).
        let vendor = file0
            .1
            .dataset
            .get(0x0008_0070)
            .map(|e| String::from_utf8_lossy(file0.1.values(&file0.0).text_bytes(e)).into_owned())
            .and_then(|m| super::curved::cbct_vendor(&m));
        let kind = VoxelKind::of(&f0);
        let (w, h) = (f0.frame.width, f0.frame.height);
        let sp = f0
            .calibration()
            .spacing
            .ok_or_else(|| crate::error_text::refusal_reason(&volume::Refusal::NoGeometry))?;
        let full = [w, h, st.slices.len() as u32];
        let plan = self.plan(full, kind, sp.x.min(sp.y), st.spacing)?;
        let g = st.planned(&plan, w, h, [sp.x, sp.y]);
        let mut vol = Volume::new(g, kind, plan.step_xy)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        vol.put_frame(0, &f0)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let (voi, _) = f0.default_voi();
        let info = Info {
            plan,
            slices: vol.geometry.slices(),
            full,
            unit: f0.unit().map(str::to_owned),
            window: super::super::medical::window_of(&voi, f0.inverted()),
            range: f0.value_range().unwrap_or((0.0, 1.0)),
            inverted: f0.inverted(),
            irregular: st.irregular,
            tilt: st.tilt,
            seismic: None,
            length: Length::Mm,
            oriented: true,
            notes: Vec::new(),
            vendor,
            anatomy: f0
                .patient_axes()
                .map_or_else(Default::default, |a| a.anatomy),
        };
        *lock(&self.volume) = vol;
        let pick = plan.pick(st.slices.len());
        let shared = single.then_some(file0);
        let order = load_order(pick.len());
        // The coarse pass (every n-th slice) ahead of the gaps.
        let coarse = order.len().min(17);
        for (n, &k) in order.iter().enumerate().filter(|&(_, &k)| k != 0) {
            let slice = &st.slices[pick[k]];
            let path = slice.key.clone();
            let index = slice.frame;
            let shared = shared.clone();
            let volume = self.volume.clone();
            let tx = self.tx.clone();
            let wake = self.wake.clone();
            self.pending.fetch_add(1, Ordering::Relaxed);
            let queued = Queued(self.pending.clone());
            let priority = if n < coarse {
                Priority::Visible
            } else {
                Priority::Near
            };
            self.pool.submit_labeled(
                priority,
                self.cancel.clone(),
                format!("mpr slice {k}"),
                move |cancel| {
                    drop(queued);
                    if cancel.is_cancelled() {
                        return;
                    }
                    // A panic is this slice's failure: the volume still completes.
                    let r = specular_core::isolate::run("mpr slice", || {
                        let file = match &shared {
                            Some(f) => f.clone(),
                            None => read(&path)?,
                        };
                        let f = frame(&file, index, cancel)?;
                        let values = f.values_f32().ok_or_else(|| {
                            crate::error_text::refusal_reason(&volume::Refusal::NotGrey)
                        })?;
                        lock(&volume)
                            .put(k, &values, f.frame.width, f.frame.height)
                            .map(|_| ())
                            .map_err(|e| crate::error_text::refusal_reason(&e))
                    })
                    .unwrap_or_else(|_| Err(specular_i18n::t!("error-reason-internal")));
                    if cancel.is_cancelled() {
                        return;
                    }
                    let _ = tx.send(Msg::Slice(r));
                    wake();
                },
            );
        }
        Ok(info)
    }
}

impl Opener {
    /// A seismic volume: read whole (trace order is inline-major, so time slices need every
    /// trace), then every time slice written here. Slice `k` is sample `samples − 1 − k`
    /// (z up, so time runs down the inline and crossline views); a slice's columns are the
    /// crosslines and its rows the inlines, one unit apart. Gaps are 0 (no amplitude).
    fn open_seismic(&self, path: &std::path::Path, s: &Arc<Seismic>) -> Result<Info, String> {
        let t0 = Instant::now();
        let cube = specular_decode::seismic_cube(path, &self.cancel)
            .map_err(|e| crate::error_text::decode_reason(&e))?
            .ok_or_else(String::new)?;
        let (nil, nxl, ns) = (cube.inlines, cube.crosslines, cube.samples);
        let slices = (0..ns)
            .map(|k| volume::Slice {
                key: k,
                frame: 0,
                position: [0.0, 0.0, k as f64],
            })
            .collect();
        let (row, col) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let st = volume::stack_of(slices, row, col, [0.0, 0.0, 1.0])
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let full = [nxl as u32, nil as u32, ns as u32];
        let plan = self.plan(full, VoxelKind::F32, 1.0, 1.0)?;
        let g = st.planned(&plan, full[0], full[1], [1.0, 1.0]);
        let mut out = Volume::new(g, VoxelKind::F32, plan.step_xy)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let pick = plan.pick(ns);
        let mut plane = vec![0.0f32; nil * nxl];
        for (k, &at) in pick.iter().enumerate() {
            if self.cancel.is_cancelled() {
                return Err(String::new());
            }
            let sample = ns - 1 - st.slices[at].key;
            for (o, trace) in plane.iter_mut().zip(cube.values.chunks_exact(ns)) {
                let v = trace[sample];
                *o = if v.is_finite() { v } else { 0.0 };
            }
            out.put(k, &plane, full[0], full[1])
                .map_err(|e| crate::error_text::refusal_reason(&e))?;
        }
        let (lo, hi) = specular_core::nd::minmax(&cube.values).unwrap_or((-1.0, 1.0));
        let clip = specular_core::nd::Limits::SEISMIC
            .compute(&cube.values, ns)
            .map_or(1.0, |(_, a)| f64::from(a))
            .max(f64::MIN_POSITIVE);
        let info = Info {
            plan,
            slices: out.geometry.slices(),
            full,
            unit: None,
            window: Some(WlWindow {
                center: 0.0,
                width: 2.0 * clip,
                shape: specular_render::VoiShape::LinearExact,
                invert: false,
            }),
            range: (f64::from(lo.min(0.0)), f64::from(hi.max(0.0))),
            inverted: false,
            irregular: false,
            tilt: None,
            seismic: Some(s.clone()),
            length: Length::Mm,
            oriented: false,
            notes: Vec::new(),
            vendor: None,
            anatomy: Default::default(),
        };
        *lock(&self.volume) = out;
        specular_log::debug!(
            target: specular_log::DECODE,
            "mpr: seismic volume {nil}×{nxl}×{ns} read in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1e3
        );
        // Every slice is in: counted as the workers' slices are (the first by the opener).
        for _ in 1..info.slices {
            let _ = self.tx.send(Msg::Slice(Ok(())));
        }
        Ok(info)
    }

    /// A research volume: read once, its canonical slices stacked by their patient
    /// positions (so the stack is checked like a series), every slice a plane of the shared
    /// volume.
    fn open_research(
        &self,
        path: &std::path::Path,
        v: usize,
        prepared: Option<&Prepared>,
    ) -> Result<Info, String> {
        use specular_decode::{medvol, research_frame};
        let t0 = Instant::now();
        let vol = match prepared {
            None => {
                Arc::new(medvol::open_path(path).map_err(|e| crate::error_text::medvol_reason(&e))?)
            }
            Some(Prepared::Volume(v)) => v.clone(),
            Some(Prepared::Seed(conn, seed)) => {
                Arc::new(conn.volume_of_seed(*seed).map_err(|e| e.to_string())?)
            }
        };
        if self.cancel.is_cancelled() {
            return Err(String::new());
        }
        let c = vol.canonical();
        if vol.header.is_colour() {
            return Err(crate::error_text::refusal_reason(&volume::Refusal::NotGrey));
        }
        if c.skewed {
            return Err(specular_i18n::t!("error-reason-volume-skewed"));
        }
        let v = v.min(vol.volumes().saturating_sub(1));
        let slices = (0..c.dims[2])
            .map(|k| volume::Slice {
                key: k,
                frame: 0,
                position: c.position(0.0, 0.0, k as f64),
            })
            .collect();
        let normal = cross(c.row, c.col);
        let st = volume::stack_of(slices, c.row, c.col, normal)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let window = vol.default_window();
        let angstrom = matches!(vol.header.kind, medvol::Kind::Map | medvol::Kind::Cube);
        let f0 = research_frame(&vol, &c, st.slices[0].key, v, window);
        let kind = VoxelKind::of(&f0);
        let (w, h) = (f0.frame.width, f0.frame.height);
        let full = [w, h, st.slices.len() as u32];
        let plan = self.plan(full, kind, c.pixel[0].min(c.pixel[1]), st.spacing)?;
        let g = st.planned(&plan, w, h, [c.pixel[0], c.pixel[1]]);
        let mut out = Volume::new(g, kind, plan.step_xy)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        out.put_frame(0, &f0)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let (voi, _) = f0.default_voi();
        let info = Info {
            plan,
            slices: out.geometry.slices(),
            full,
            unit: None,
            window: super::super::medical::window_of(&voi, false),
            range: vol.value_range(),
            inverted: false,
            irregular: st.irregular,
            tilt: st.tilt,
            seismic: None,
            length: if angstrom {
                Length::Angstrom
            } else {
                Length::Mm
            },
            // Density maps' and industrial CT's axes are not a patient's; NIfTI's sform or
            // qform (and the other formats' stated directions) are.
            oriented: !angstrom && vol.header.orientation.reliable(),
            notes: Vec::new(),
            vendor: None,
            anatomy: Default::default(),
        };
        *lock(&self.volume) = out;
        specular_log::debug!(
            target: specular_log::DECODE,
            "mpr: research volume {}×{}×{} read in {:.1} ms",
            c.dims[0],
            c.dims[1],
            c.dims[2],
            t0.elapsed().as_secs_f64() * 1e3
        );
        let pick = plan.pick(st.slices.len());
        let order = load_order(pick.len());
        let coarse = order.len().min(17);
        for (n, &k) in order.iter().enumerate().filter(|&(_, &k)| k != 0) {
            let at = st.slices[pick[k]].key;
            let (vol, c) = (vol.clone(), c.clone());
            let volume = self.volume.clone();
            let tx = self.tx.clone();
            let wake = self.wake.clone();
            self.pending.fetch_add(1, Ordering::Relaxed);
            let queued = Queued(self.pending.clone());
            let priority = if n < coarse {
                Priority::Visible
            } else {
                Priority::Near
            };
            self.pool.submit_labeled(
                priority,
                self.cancel.clone(),
                format!("mpr slice {k}"),
                move |cancel| {
                    drop(queued);
                    if cancel.is_cancelled() {
                        return;
                    }
                    let r = specular_core::isolate::run("mpr slice", || {
                        let values = vol.plane_f32(&c, at, v);
                        lock(&volume)
                            .put(k, &values, c.dims[0] as u32, c.dims[1] as u32)
                            .map(|_| ())
                            .map_err(|e| crate::error_text::refusal_reason(&e))
                    })
                    .unwrap_or_else(|_| Err(specular_i18n::t!("error-reason-internal")));
                    if cancel.is_cancelled() {
                        return;
                    }
                    let _ = tx.send(Msg::Slice(r));
                    wake();
                },
            );
        }
        Ok(info)
    }
}

/// Locks the reservation slot (a worker that panicked leaves it as it was).
fn lock_memory(
    m: &Mutex<Option<specular_core::Reservation>>,
) -> MutexGuard<'_, Option<specular_core::Reservation>> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// "it needs 24 GB of free RAM, but only 18 GB is free …" for a volume of `need` bytes when
/// `free` budget bytes are left (the budget is 80 % of the free RAM).
fn needs_memory(need: u64, free: u64) -> String {
    let n = specular_core::NeedsMemory {
        needed: need.saturating_mul(5).div_ceil(4),
        free: free.saturating_mul(5) / 4,
    };
    crate::error_text::needs_memory_reason(n)
}

impl Opener {
    /// A scientific image's z planes: every plane read through the session at the other
    /// axes' positions (the shown channel and time point), stacked `stack.spacing` apart.
    fn open_science(
        &self,
        session: &Arc<dyn specular_decode::Session>,
        stack: &Arc<super::science_vol::Stack>,
    ) -> Result<Info, String> {
        let t0 = Instant::now();
        let (w, h, n) = (stack.width, stack.height, stack.slices());
        let [px, py, pz] = stack.spacing;
        let slices = (0..n)
            .map(|k| volume::Slice {
                key: k,
                frame: 0,
                position: [0.0, 0.0, k as f64 * pz],
            })
            .collect();
        let st = volume::stack_of(slices, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let full = [w, h, n as u32];
        let plan = self.plan(full, VoxelKind::F32, px.min(py), pz)?;
        let g = st.planned(&plan, w, h, [px, py]);
        let mut out = Volume::new(g, VoxelKind::F32, plan.step_xy)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let read = |k: usize, cancel: &CancelToken| -> Result<Vec<f32>, String> {
            let plane = session
                .nd_plane(&stack.index_at(k), cancel)
                .ok_or_else(String::new)?
                .map_err(|e| crate::error_text::decode_reason(&e))?;
            if plane.width != w || plane.height != h {
                return Err(crate::error_text::refusal_reason(
                    &volume::Refusal::Mismatch,
                ));
            }
            Ok(super::science_vol::finite(&plane.values))
        };
        let pick = plan.pick(n);
        let first = read(pick[0], &self.cancel)?;
        if self.cancel.is_cancelled() {
            return Err(String::new());
        }
        out.put(0, &first, w, h)
            .map_err(|e| crate::error_text::refusal_reason(&e))?;
        let range = specular_core::nd::minmax(&first).unwrap_or((0.0, 1.0));
        let (lo, hi) = stack
            .limits
            .or_else(|| specular_core::nd::Limits::Percentile(99.5).compute(&first, w as usize))
            .unwrap_or(range);
        let info = Info {
            plan,
            slices: out.geometry.slices(),
            full,
            unit: stack.unit.clone(),
            window: Some(WlWindow {
                center: (f64::from(lo) + f64::from(hi)) / 2.0,
                width: (f64::from(hi) - f64::from(lo)).max(f64::MIN_POSITIVE),
                shape: specular_render::VoiShape::LinearExact,
                invert: false,
            }),
            range: (f64::from(range.0), f64::from(range.1)),
            inverted: false,
            irregular: false,
            tilt: None,
            seismic: None,
            length: stack.length,
            oriented: false,
            notes: stack.notes.clone(),
            vendor: None,
            anatomy: Default::default(),
        };
        *lock(&self.volume) = out;
        specular_log::debug!(
            target: specular_log::DECODE,
            "mpr: science stack {w}×{h}×{n} opened in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1e3
        );
        let order = load_order(pick.len());
        let coarse = order.len().min(17);
        for (m, &k) in order.iter().enumerate().filter(|&(_, &k)| k != 0) {
            let at = pick[k];
            let (session, stack) = (session.clone(), stack.clone());
            let volume = self.volume.clone();
            let tx = self.tx.clone();
            let wake = self.wake.clone();
            self.pending.fetch_add(1, Ordering::Relaxed);
            let queued = Queued(self.pending.clone());
            let priority = if m < coarse {
                Priority::Visible
            } else {
                Priority::Near
            };
            self.pool.submit_labeled(
                priority,
                self.cancel.clone(),
                format!("mpr plane {k}"),
                move |cancel| {
                    drop(queued);
                    if cancel.is_cancelled() {
                        return;
                    }
                    let r = specular_core::isolate::run("mpr plane", || {
                        let plane = session
                            .nd_plane(&stack.index_at(at), cancel)
                            .ok_or_else(String::new)?
                            .map_err(|e| crate::error_text::decode_reason(&e))?;
                        let values = super::science_vol::finite(&plane.values);
                        lock(&volume)
                            .put(k, &values, plane.width, plane.height)
                            .map(|_| ())
                            .map_err(|e| crate::error_text::refusal_reason(&e))
                    })
                    .unwrap_or_else(|_| Err(specular_i18n::t!("error-reason-internal")));
                    if cancel.is_cancelled() {
                        return;
                    }
                    let _ = tx.send(Msg::Slice(r));
                    wake();
                },
            );
        }
        Ok(info)
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

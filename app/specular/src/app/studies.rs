//! The studies browser: a patient's CD, DVD or USB stick (DESIGN.md §6.2).
//!
//! Opening a DICOMDIR, or a folder that holds one (also in a subfolder such as `DICOM/` or
//! `IHE_PDI/`) or DICOM files without extensions, lists its patients, studies and series
//! ([`specular_dicom::media`]): from the DICOMDIR's records when there is one, else from the
//! files' headers (read off the UI thread, cancellable, streamed in batches). Enter on a
//! series opens it in series mode (`series.rs`). Thumbnails are the DICOMDIR's icons, else
//! made from each series' middle image by one background thread, and kept in memory only
//! (no cache key names a patient). Patient names and birth dates are shown only in the
//! browser, behind "Show patient data"; they never go to the window title or the log.
//!
//! On Linux, a disc or stick mounted under `/run/media` or `/media` with a DICOMDIR is
//! offered in a toast (a file system watcher, no polling on the UI thread).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use specular_dicom::media::{self, Detected, FileHeader, Medium, Progress, ScanLimits, Thumb};
use specular_dicom::series::{self, Scan};
use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};
use specular_i18n::{I18n, t};
use specular_log::{FOLDER, debug};
use specular_widgets::{Studies, StudyKind, StudyLine};

use super::App;
use super::dicomweb::send::{Choice, Scope, SendRequest};
use crate::loader::Event;

/// Toast keys of the "Show studies" offers (apart from undo and the other toast actions).
const OFFER_KEYS: u64 = 1 << 61;
/// Thumbnails' longest side, in pixels.
const THUMB: u32 = 128;

/// What the reading of a medium found so far.
pub(crate) enum StudiesUpdate {
    /// The whole medium (from its DICOMDIR).
    Medium(Box<Medium>),
    /// Files' headers (a medium without a DICOMDIR), and how far the scan is.
    Headers(Vec<FileHeader>, Progress),
    /// The reading ended (`Err`: why it failed).
    Done(Result<(), String>),
}

/// A thumbnail on the GPU.
struct Shown {
    _tex: wgpu::Texture,
    id: TexId,
    w: u32,
    h: u32,
}

/// The studies browser's state.
#[derive(Default)]
pub(super) struct StudiesMode {
    /// Bumped by each new medium (and each detection): older events are stale.
    generation: u64,
    /// The reading in flight: its cancel flag.
    cancel: Option<Arc<AtomicBool>>,
    /// The medium's name (its folder's), for the title.
    name: String,
    medium: Medium,
    /// The scan is still reading (and how far it is).
    reading: Option<Progress>,
    /// The reading was cancelled at this point: what it found stays listed, marked as
    /// incomplete, and what the scan still sends is ignored.
    stopped: Option<Progress>,
    capped: bool,
    error: Option<String>,
    /// Each series' id, by its UID (stable while the scan adds series).
    ids: HashMap<String, u64>,
    /// Thumbnails made, waiting for upload, and on the GPU.
    pending: Vec<(u64, Thumb)>,
    shown: HashMap<u64, Shown>,
    /// Series whose thumbnail was asked for (or came from the DICOMDIR).
    asked: std::collections::HashSet<u64>,
    /// Series shown with a provisional thumbnail while the scan reads (the first image
    /// seen), and its file: replaced by the middle image's once the scan ends.
    provisional: HashMap<u64, PathBuf>,
    /// Textures to release at the next frame.
    retired: Vec<TexId>,
    /// The thumbnail thread's queue.
    thumbs: Option<mpsc::Sender<(u64, PathBuf)>>,
    /// The series being opened.
    opening: Option<u64>,
    /// The removable media watcher, and the offers made (toast key → mount point).
    #[cfg(target_os = "linux")]
    watcher: Option<notify::RecommendedWatcher>,
    offers: HashMap<u64, PathBuf>,
    next_offer: u64,
}

impl StudiesMode {
    /// The GPU device was replaced: the thumbnails are made and uploaded again.
    pub(super) fn gpu_reset(&mut self) {
        self.shown.clear();
        self.asked.clear();
        self.provisional.clear();
        self.retired.clear();
    }

    /// Forgets the medium (its thumbnails are released at the next frame).
    fn reset(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        self.thumbs = None;
        self.generation += 1;
        self.medium = Medium::default();
        self.reading = None;
        self.stopped = None;
        self.capped = false;
        self.error = None;
        self.ids.clear();
        self.pending.clear();
        self.asked.clear();
        self.provisional.clear();
        self.opening = None;
        self.retired.extend(self.shown.drain().map(|(_, s)| s.id));
    }

    /// A browser task panicked ([`crate::loader::Event::BackgroundFailed`]): what waited
    /// for it stops waiting. Reading ends with an internal error shown in the browser; a
    /// series being opened is no longer "opening". Whether `task` was one of the browser's.
    fn lost(&mut self, task: &str) -> bool {
        match task {
            "dcm-st-read" if self.reading.is_some() => {
                self.reading = None;
                self.error = Some(t!("error-reason-internal"));
            }
            "dcm-st-series" if self.opening.is_some() => {
                self.opening = None;
                self.error = Some(t!("error-reason-internal"));
            }
            "dcm-st-read" | "dcm-st-series" | "dcm-st-detect" | "dcm-st-find" | "dcm-st-thumbs" => {
            }
            _ => return false,
        }
        true
    }

    fn id_of(&mut self, uid: &str) -> u64 {
        let n = self.ids.len() as u64 + 1;
        *self.ids.entry(uid.to_owned()).or_insert(n)
    }

    /// The medium's one series, once it is read whole (not stopped, capped or failed) and
    /// holds just one with images on the medium: opened at once, no choice to offer.
    fn only_series(&mut self) -> Option<u64> {
        if self.reading.is_some()
            || self.stopped.is_some()
            || self.capped
            || self.error.is_some()
            || self.medium.series_count() != 1
        {
            return None;
        }
        let s = self
            .medium
            .patients
            .iter()
            .flat_map(|p| &p.studies)
            .flat_map(|s| &s.series)
            .next()?;
        let uid = s.uid.clone();
        s.present().next()?;
        Some(self.id_of(&uid))
    }

    fn series_by_id(&self, id: u64) -> Option<&media::MediaSeries> {
        self.medium
            .patients
            .iter()
            .flat_map(|p| &p.studies)
            .flat_map(|s| &s.series)
            .find(|s| self.ids.get(&s.uid) == Some(&id))
    }

    /// Uploads the thumbnails made since the last frame and releases retired ones. True
    /// when the browser's rows changed.
    pub(super) fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) -> bool {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        if self.pending.is_empty() {
            return false;
        }
        for (series, th) in self.pending.drain(..) {
            if th.width == 0 || th.height == 0 {
                continue;
            }
            let (tex, id) = upload_rgba(gpu, ui, th.width, th.height, &th.rgba);
            if let Some(old) = self.shown.insert(
                series,
                Shown {
                    _tex: tex,
                    id,
                    w: th.width,
                    h: th.height,
                },
            ) {
                ui.unregister_texture(old.id);
            }
        }
        true
    }

    /// The browser's content, localised.
    pub(super) fn sheet(&self) -> Studies {
        let mut rows = Vec::new();
        for (pi, p) in self.medium.patients.iter().enumerate() {
            let name = media::display_name(&p.name);
            let title = if !name.is_empty() {
                name
            } else if !p.id.trim().is_empty() {
                p.id.trim().to_owned()
            } else {
                t!("studies-patient-unknown")
            };
            rows.push(StudyLine {
                kind: StudyKind::Patient,
                title,
                detail: dicom_date(&p.birth_date)
                    .map(|d| t!("studies-born", date = d))
                    .unwrap_or_default(),
                identifying: true,
                hidden_title: t!("studies-patient-n", index = (pi + 1) as i64),
                thumb: None,
                badge: String::new(),
                id: 0,
                missing: false,
            });
            for st in &p.studies {
                let mut detail = Vec::new();
                if let Some(d) = dicom_date(&st.date) {
                    detail.push(d);
                }
                let mods = st.modalities().join(", ");
                if !mods.is_empty() {
                    detail.push(mods);
                }
                rows.push(StudyLine {
                    kind: StudyKind::Study,
                    title: non_empty(&st.description)
                        .unwrap_or_else(|| t!("studies-no-description")),
                    detail: specular_i18n::dot_join(&detail),
                    identifying: false,
                    hidden_title: String::new(),
                    thumb: None,
                    badge: String::new(),
                    id: 0,
                    missing: false,
                });
                for s in &st.series {
                    let id = self.ids.get(&s.uid).copied().unwrap_or(0);
                    rows.push(series_line(s, id, self.shown.get(&id)));
                }
            }
        }
        let status = if self.opening.is_some() {
            Some(t!("studies-opening"))
        } else if let Some(e) = &self.error {
            Some(t!("studies-error", reason = e.clone()))
        } else if let Some(p) = self.stopped {
            Some(if p.total == 0 {
                t!("studies-stopped-early")
            } else {
                t!(
                    "studies-stopped",
                    done = p.done as i64,
                    total = p.total as i64
                )
            })
        } else {
            self.reading.map(|p| {
                if p.total == 0 {
                    t!("studies-reading")
                } else {
                    t!(
                        "studies-reading-count",
                        done = p.done as i64,
                        total = p.total as i64
                    )
                }
            })
        };
        let mut notes = Vec::new();
        if self.medium.missing > 0 {
            notes.push(t!("studies-missing", count = self.medium.missing as i64));
        }
        if self.capped {
            notes.push(t!(
                "studies-capped",
                count = ScanLimits::default().files as i64
            ));
        }
        Studies {
            title: self.name.clone(),
            rows,
            status,
            note: (!notes.is_empty()).then(|| notes.join(". ")),
            cancellable: self.reading.is_some() && self.opening.is_none(),
            sendable: self.reading.is_none() && self.opening.is_none(),
        }
    }

    /// What "Send to PACS…" offers for the selected series `sel`: that series, its study,
    /// and everything read (when that is more than the study). Only files on the medium.
    pub(super) fn send_request(&self, sel: Option<u64>) -> Option<SendRequest> {
        fn files(se: &media::MediaSeries) -> impl Iterator<Item = PathBuf> + '_ {
            se.images
                .iter()
                .chain(&se.others)
                .filter(|f| f.present)
                .map(|f| f.path.clone())
        }
        let mut choices = Vec::new();
        let studies = || self.medium.patients.iter().flat_map(|p| &p.studies);
        if let Some(id) = sel
            && let Some(st) =
                studies().find(|st| st.series.iter().any(|x| self.ids.get(&x.uid) == Some(&id)))
            && let Some(se) = self.series_by_id(id)
        {
            choices.push(Choice {
                scope: Scope::Series,
                files: files(se).collect(),
                studies: 1,
            });
            if st.series.len() > 1 {
                choices.push(Choice {
                    scope: Scope::Study,
                    files: st.series.iter().flat_map(files).collect(),
                    studies: 1,
                });
            }
        }
        let n = self.medium.study_count();
        if choices.is_empty() || n > 1 {
            choices.push(Choice {
                scope: Scope::All,
                files: studies()
                    .flat_map(|st| &st.series)
                    .flat_map(files)
                    .collect(),
                studies: n,
            });
        }
        choices.retain(|c| !c.files.is_empty());
        // The study (else the series) is what is meant most often.
        let at = choices
            .iter()
            .position(|c| c.scope == Scope::Study)
            .unwrap_or(0);
        (!choices.is_empty()).then_some(SendRequest { choices, at })
    }
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// A series' row.
fn series_line(s: &media::MediaSeries, id: u64, shown: Option<&Shown>) -> StudyLine {
    let desc = non_empty(&s.description);
    let title = match (s.number, desc) {
        (Some(n), Some(d)) => t!("studies-series", number = n, description = d),
        (Some(n), None) => t!("studies-series-number", number = n),
        (None, Some(d)) => d,
        (None, None) => t!("studies-no-description"),
    };
    let mut detail = Vec::new();
    if !s.modality.is_empty() {
        detail.push(s.modality.clone());
    }
    let present = s.images.len() - s.missing();
    if !s.images.is_empty() {
        detail.push(t!("studies-images", count = s.images.len() as i64));
    }
    if s.missing() > 0 {
        detail.push(t!("studies-images-missing", count = s.missing() as i64));
    }
    if !s.others.is_empty() {
        detail.push(t!("studies-others", count = s.others.len() as i64));
    }
    StudyLine {
        kind: StudyKind::Series,
        title,
        detail: specular_i18n::dot_join(&detail),
        identifying: false,
        hidden_title: String::new(),
        thumb: shown.map(|t| (t.id, t.w, t.h)),
        badge: if s.modality.is_empty() {
            "?".into()
        } else {
            s.modality.clone()
        },
        id,
        missing: !s.images.is_empty() && present == 0,
    }
}

/// A DICOM date (`YYYYMMDD`, also the old `YYYY.MM.DD`), localised.
fn dicom_date(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 8 {
        return None;
    }
    let y: i32 = digits[..4].parse().ok()?;
    let m: u8 = digits[4..6].parse().ok()?;
    let d: u8 = digits[6..8].parse().ok()?;
    I18n::global()
        .localizer()
        .date(y, m, d, specular_i18n::DateStyle::Medium)
}

/// The medium of folder `dir`, for "Show studies on this disc": a DICOMDIR in it or up to three
/// folders up, else the folder's own files. Lists folders: never on the UI thread.
fn medium_of(dir: PathBuf) -> Detected {
    series::find_dicomdir(&dir, 3)
        .map(Detected::Dicomdir)
        .unwrap_or(Detected::Files(dir))
}

/// The medium's name: the folder of the DICOMDIR (or the folder scanned); a mount point's
/// name is the volume's label.
fn medium_name(found: &Detected) -> String {
    let dir = match found {
        Detected::Dicomdir(p) => p.parent(),
        Detected::Files(d) => Some(d.as_path()),
    };
    // `DICOM/DICOMDIR` and the like: the disc, not its standard subfolder.
    let dir = dir.map(|d| {
        let n = d
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_uppercase());
        match (n.as_deref(), d.parent()) {
            (Some("DICOM" | "IHE_PDI"), Some(p)) if p.file_name().is_some() => p,
            _ => d,
        }
    });
    dir.and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(super) fn upload_rgba(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("study thumbnail"),
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
            bytes_per_row: Some(4 * w),
            rows_per_image: None,
        },
        size,
    );
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

/// A series' thumbnail from its file. A panic (a decoder bug on that file) costs that
/// thumbnail, which stays empty; the thread goes on with the next.
fn study_thumb(path: &Path, cancel: &AtomicBool) -> Option<Thumb> {
    specular_core::isolate::run(format_args!("study thumbnail {}", path.display()), || {
        specular_core::isolate::trip(path);
        media::file_thumb(path, THUMB, cancel).ok()
    })
    .ok()
    .flatten()
}

impl App {
    /// A browser task panicked: its waiting state ends (see [`StudiesMode::lost`]). Whether
    /// the task was the browser's.
    pub(super) fn studies_lost(&mut self, task: &str) -> bool {
        if !self.studies.lost(task) {
            return false;
        }
        let sheet = self.studies.sheet();
        self.chrome.set_studies(sheet);
        self.redraw = true;
        true
    }

    /// A folder was opened: whether it is a medium is found in the background
    /// ([`media::detect`] reads a bounded number of folders and file starts); the browser
    /// opens when it is.
    pub(super) fn studies_detect(&mut self, dir: &Path) {
        if self.headless() {
            return;
        }
        self.studies.reset();
        let generation = self.studies.generation;
        let dir = dir.to_path_buf();
        self.loader.background("dcm-st-detect", move |l| {
            let t0 = Instant::now();
            let found = media::detect(&dir);
            debug!(target: FOLDER, "medium detection: {} in {:.1} ms", match &found {
                Some(Detected::Dicomdir(_)) => "a DICOMDIR",
                Some(Detected::Files(_)) => "DICOM files",
                None => "none",
            }, t0.elapsed().as_secs_f64() * 1e3);
            if let Some(found) = found {
                l.post(Event::StudiesDetected { generation, found });
            }
        });
    }

    /// The detection's result (still for the folder open).
    pub(super) fn on_studies_detected(&mut self, generation: u64, found: Detected) {
        if generation == self.studies.generation {
            self.studies_start(found);
        }
    }

    /// Opens the browser on a medium and reads it in the background.
    pub(super) fn studies_start(&mut self, found: Detected) {
        self.studies.reset();
        let s = &mut self.studies;
        s.name = medium_name(&found);
        s.reading = Some(Progress::default());
        let generation = s.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        s.cancel = Some(cancel.clone());
        let sheet = s.sheet();
        self.chrome.show_studies(sheet);
        self.studies_thumb_thread();
        self.loader.background("dcm-st-read", move |l| {
            let t0 = Instant::now();
            let post = |update| l.post(Event::Studies { generation, update });
            let root = match &found {
                Detected::Dicomdir(p) => match media::read_dicomdir(p) {
                    Ok(m) if m.series_count() > 0 => {
                        debug!(target: FOLDER, "DICOMDIR: {} patients, {} studies, {} series, {} files missing, in {:.1} ms",
                            m.patients.len(), m.study_count(), m.series_count(), m.missing, t0.elapsed().as_secs_f64() * 1e3);
                        post(StudiesUpdate::Medium(Box::new(m)));
                        post(StudiesUpdate::Done(Ok(())));
                        return;
                    }
                    // A DICOMDIR that lists nothing, or cannot be read: the files' headers.
                    Ok(_) => p.parent().map(Path::to_path_buf),
                    Err(e) => {
                        debug!(target: FOLDER, "DICOMDIR unreadable ({e}): reading the files' headers");
                        p.parent().map(Path::to_path_buf)
                    }
                },
                Detected::Files(d) => Some(d.clone()),
            };
            let Some(root) = root else {
                post(StudiesUpdate::Done(Ok(())));
                return;
            };
            let mut headers = 0;
            let r = media::scan(&root, ScanLimits::default(), &cancel, &mut |batch, p| {
                headers += batch.len();
                post(StudiesUpdate::Headers(batch, p));
            });
            if matches!(r, Err(specular_dicom::Error::Cancelled)) {
                return;
            }
            debug!(target: FOLDER, "medium scan: {headers} headers in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            post(StudiesUpdate::Done(r.map_err(|e| crate::error_text::dicom_reason(&e))));
        });
        self.redraw = true;
    }

    /// One thread makes the thumbnails, one at a time, until the medium is replaced.
    fn studies_thumb_thread(&mut self) {
        let (tx, rx) = mpsc::channel::<(u64, PathBuf)>();
        self.studies.thumbs = Some(tx);
        let generation = self.studies.generation;
        let cancel = self.studies.cancel.clone().unwrap_or_default();
        self.loader.background("dcm-st-thumbs", move |l| {
            // Ends when the sender is dropped (another medium).
            while let Ok((id, path)) = rx.recv() {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let thumb = study_thumb(&path, &cancel);
                l.post(Event::StudyThumb {
                    generation,
                    id,
                    thumb,
                });
            }
        });
    }

    /// What the reading found.
    pub(super) fn on_studies(&mut self, generation: u64, update: StudiesUpdate) {
        let s = &mut self.studies;
        if generation != s.generation || s.stopped.is_some() {
            return;
        }
        match update {
            StudiesUpdate::Medium(m) => s.medium = *m,
            StudiesUpdate::Headers(h, p) => {
                s.reading = Some(p);
                s.capped = p.capped;
                if !h.is_empty() {
                    s.medium.add(h);
                }
            }
            StudiesUpdate::Done(r) => {
                s.reading = None;
                s.error = r.err();
            }
        }
        s.ask_thumbs();
        if s.medium.patients.is_empty() && s.reading.is_none() && s.error.is_none() {
            debug!(target: FOLDER, "medium: no studies found");
        }
        let only = s.opening.is_none().then(|| s.only_series()).flatten();
        let sheet = s.sheet();
        self.chrome.set_studies(sheet);
        self.redraw = true;
        // A medium of one series: straight into it (the browser closes once it opens).
        if let Some(id) = only {
            debug!(target: FOLDER, "medium: one series, opening it");
            self.studies_open_series(id);
        }
    }

    /// Cancel in the browser (or Esc while it reads): the scan stops at its next file and
    /// what it found so far stays listed, with "Reading stopped after N of M files".
    pub(super) fn studies_cancel(&mut self) {
        let s = &mut self.studies;
        let Some(p) = s.reading.take() else {
            return;
        };
        if let Some(c) = &s.cancel {
            c.store(true, Ordering::Relaxed);
        }
        s.stopped = Some(p);
        debug!(target: FOLDER, "medium: reading stopped after {} of {} files", p.done, p.total);
        s.ask_thumbs();
        let sheet = s.sheet();
        self.chrome.set_studies(sheet);
        self.redraw = true;
    }
}

impl StudiesMode {
    /// Ids, DICOMDIR icons and thumbnails for the series not seen yet.
    fn ask_thumbs(&mut self) {
        let s = self;
        let uids: Vec<String> = s
            .medium
            .patients
            .iter()
            .flat_map(|p| &p.studies)
            .flat_map(|st| &st.series)
            .map(|x| x.uid.clone())
            .collect();
        for uid in uids {
            let id = s.id_of(&uid);
            if s.asked.contains(&id) {
                continue;
            }
            let Some(x) = s.series_by_id(id) else {
                continue;
            };
            let (icon, middle) = (x.icon.clone(), x.middle().map(Path::to_path_buf));
            if let Some(icon) = icon {
                s.asked.insert(id);
                s.pending.push((id, icon));
            } else if let Some(m) = middle
                && let Some(tx) = &s.thumbs
            {
                // While the scan reads, a series' middle image may still change: the first
                // image seen stands in, and the middle one is asked once the scan is done
                // (a DICOMDIR's medium is whole from the start).
                if s.reading.is_none() {
                    s.asked.insert(id);
                    if s.provisional.remove(&id).as_ref() != Some(&m) {
                        let _ = tx.send((id, m));
                    }
                } else if let std::collections::hash_map::Entry::Vacant(e) = s.provisional.entry(id)
                {
                    e.insert(m.clone());
                    let _ = tx.send((id, m));
                }
            }
        }
    }
}

impl App {
    /// A thumbnail was made.
    pub(super) fn on_study_thumb(&mut self, generation: u64, id: u64, thumb: Option<Thumb>) {
        if generation != self.studies.generation {
            return;
        }
        if let Some(t) = thumb {
            self.studies.pending.push((id, t));
            self.redraw = true;
        }
    }

    /// The palette's "Show studies on this disc": the medium of the current file's folder
    /// (a DICOMDIR up to three folders up, else the folder's files).
    pub(super) fn studies_show(&mut self) {
        let Some(dir) = self
            .current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
        else {
            return;
        };
        if let Some(dd) = self.studies.medium.dicomdir.clone()
            && dd.parent().is_some_and(|r| dir.starts_with(r))
            && self.studies.error.is_none()
        {
            // The medium read already: shown as it is.
            self.chrome.show_studies(self.studies.sheet());
            return;
        }
        // Finding the DICOMDIR lists up to four folders (slow on a disc or a share): in the
        // background, as for an opened folder.
        self.studies.reset();
        let generation = self.studies.generation;
        self.loader.background("dcm-st-find", move |l| {
            let found = medium_of(dir);
            l.post(Event::StudiesDetected { generation, found });
        });
    }

    /// Enter on a series: its headers are read (off the UI thread), then it opens in series
    /// mode at its first image.
    pub(super) fn studies_open_series(&mut self, id: u64) {
        let Some(s) = self.studies.series_by_id(id) else {
            return;
        };
        let files: Vec<PathBuf> = s.present().map(Path::to_path_buf).collect();
        if files.is_empty() {
            let now = self.now();
            self.chrome.toast(t!("studies-no-images"), None, now);
            return;
        }
        self.studies.opening = Some(id);
        self.chrome.set_studies(self.studies.sheet());
        let generation = self.studies.generation;
        let cancel = self.studies.cancel.clone().unwrap_or_default();
        self.loader.background("dcm-st-series", move |l| {
            let t0 = Instant::now();
            let mut found = Vec::new();
            for f in &files {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if let Ok(Some(i)) = series::read_header(f)
                    && i.has_image()
                {
                    found.push((f.clone(), i));
                }
            }
            let headers = found.len();
            let scan: crate::app::ScanResult = if found.is_empty() {
                Err(t!("studies-no-images"))
            } else {
                Ok(Scan {
                    series: series::assemble(found),
                    headers,
                    ..Scan::default()
                })
            };
            debug!(target: FOLDER, "studies: series of {headers} images read in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            l.post(Event::StudySeries {
                generation,
                id,
                scan,
            });
        });
        self.redraw = true;
    }

    /// The series' headers, read: series mode at its first image.
    pub(super) fn on_study_series(
        &mut self,
        generation: u64,
        id: u64,
        scan: crate::app::ScanResult,
    ) {
        if generation != self.studies.generation || self.studies.opening != Some(id) {
            return;
        }
        self.studies.opening = None;
        // Comparing series: it goes to a viewport (`linked.rs`).
        if self.linked_wants_study(id) {
            match scan {
                Ok(s) => self.linked_take_study(s),
                Err(e) => {
                    let now = self.now();
                    self.chrome.toast(e, None, now);
                }
            }
            return;
        }
        let scan = match scan {
            Ok(s) if !s.series.is_empty() => s,
            Ok(_) => {
                self.chrome.set_studies(self.studies.sheet());
                return;
            }
            Err(e) => {
                self.studies.error = Some(e);
                self.chrome.set_studies(self.studies.sheet());
                self.studies.error = None;
                return;
            }
        };
        // The largest of what the series' files assemble into (one, unless they differ in
        // orientation or size), at its first image.
        let k = (0..scan.series.len())
            .max_by_key(|&k| scan.series[k].items.len())
            .unwrap_or(0);
        let Some(file) = scan.series[k].items.first().map(|i| i.key.clone()) else {
            return;
        };
        if self.chrome.studies_open() {
            self.chrome.close_modal();
        }
        self.gallery_dismiss();
        self.close_book();
        self.series_close();
        // The listing of the medium's folder, if still on its way, is not for this.
        self.folder = file.parent().map(Path::to_path_buf);
        self.series_enter(scan, k, &file);
        let at = self.current.unwrap_or(0);
        self.current = None;
        self.go_to(at);
    }

    /// The browser's "Send to PACS…" (with the selected series): the DICOM servers sheet on
    /// its Send page; the browser closes (Show studies brings it back as it was).
    pub(super) fn studies_send(&mut self, sel: Option<u64>) {
        let Some(req) = self.studies.send_request(sel) else {
            let now = self.now();
            self.chrome.toast(t!("studies-no-images"), None, now);
            return;
        };
        if self.chrome.studies_open() {
            self.chrome.close_modal();
        }
        self.dicomweb_send(req);
    }

    /// The palette's "Send to PACS…": from the Studies browser when a medium was read (its
    /// selected series), else the series on screen, else the file on screen.
    pub(super) fn send_to_pacs(&mut self) {
        if !self.studies.medium.patients.is_empty() && self.studies.reading.is_none() {
            let sel = self.chrome.studies_selected();
            self.studies_send(sel);
            return;
        }
        let files: Vec<PathBuf> = if self.series_active() {
            self.entries.clone()
        } else {
            self.current
                .and_then(|i| self.entries.get(i))
                .cloned()
                .into_iter()
                .collect()
        };
        if files.is_empty() {
            return;
        }
        self.dicomweb_send(SendRequest {
            choices: vec![Choice {
                scope: Scope::Series,
                files,
                studies: 1,
            }],
            at: 0,
        });
    }

    /// The toast's "Show studies".
    pub(super) fn studies_offer_taken(&mut self, key: u64) -> bool {
        let Some(mount) = self.studies.offers.remove(&key) else {
            return false;
        };
        self.chrome.dismiss_undo(key);
        match media::detect(&mount) {
            Some(found) => self.studies_start(found),
            None => debug!(target: FOLDER, "the offered medium is gone"),
        }
        true
    }

    /// A removable medium with a DICOMDIR was mounted: offered in a toast. Its name (often
    /// the patient's) is not shown or logged.
    pub(super) fn on_studies_medium(&mut self, mount: PathBuf) {
        if self.studies.offers.values().any(|m| *m == mount) {
            return;
        }
        debug!(target: FOLDER, "a removable medium with a DICOMDIR was mounted");
        self.studies.next_offer += 1;
        let key = OFFER_KEYS | self.studies.next_offer;
        self.studies.offers.insert(key, mount);
        let now = self.now();
        self.chrome.toast_action(
            t!("studies-medium-inserted"),
            t!("studies-medium-show"),
            key,
            now,
        );
        self.redraw = true;
    }

    /// Watches where removable media are mounted (not headless).
    pub(super) fn studies_init(&mut self) {
        #[cfg(target_os = "linux")]
        if !self.headless() {
            self.studies.watcher = watch_media(self.loader.clone());
        }
    }
}

/// The folders removable media are mounted in: `/run/media/$USER` (udisks2), `/media/$USER`
/// and `/media` (Debian, Ubuntu), those that exist.
#[cfg(target_os = "linux")]
fn media_roots() -> Vec<PathBuf> {
    let user = std::env::var_os("USER").filter(|u| !u.is_empty());
    let mut roots = Vec::new();
    for base in ["/run/media", "/media"] {
        let base = Path::new(base);
        if let Some(u) = &user {
            let p = base.join(u);
            if p.is_dir() {
                roots.push(p);
            }
        }
        if base.is_dir() {
            roots.push(base.to_path_buf());
        }
    }
    roots
}

/// A watcher on the mount folders: a folder that appears there is checked for a DICOMDIR a
/// few times while the medium mounts (on a thread of its own), and offered when it has one.
#[cfg(target_os = "linux")]
fn watch_media(loader: crate::loader::Loader) -> Option<notify::RecommendedWatcher> {
    use notify::Watcher;
    let roots = media_roots();
    if roots.is_empty() {
        return None;
    }
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let parents = roots.clone();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(e) = res else { return };
        if !matches!(e.kind, notify::EventKind::Create(_)) {
            return;
        }
        for p in e.paths {
            if p.parent().is_some_and(|d| parents.iter().any(|r| r == d)) {
                let _ = tx.send(p);
            }
        }
    })
    .ok()?;
    let mut watching = 0;
    for r in &roots {
        if w.watch(r, notify::RecursiveMode::NonRecursive).is_ok() {
            watching += 1;
        }
    }
    if watching == 0 {
        return None;
    }
    specular_thread::builder("dcm-media")
        .spawn(move || {
            // Ends when the watcher (and with it the sender) is dropped. A panic on one
            // medium costs that offer (logged); the thread goes on with the next mount.
            while let Ok(mount) = rx.recv() {
                let _ = specular_core::isolate::run(
                    format_args!("checking {} for a DICOMDIR", mount.display()),
                    || {
                        // The folder appears before the medium is mounted on it.
                        for wait_ms in [300, 1000, 2500, 5000] {
                            std::thread::sleep(std::time::Duration::from_millis(wait_ms));
                            if matches!(media::detect(&mount), Some(Detected::Dicomdir(_))) {
                                loader.post(Event::StudiesMedium {
                                    mount: mount.clone(),
                                });
                                break;
                            }
                        }
                    },
                );
            }
        })
        .ok()?;
    Some(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// While the scan reads, a series shows its first image seen as a provisional
    /// thumbnail; once the scan ends its middle image replaces it (and is not asked again
    /// when it is the same file).
    #[test]
    fn thumbnails_are_provisional_while_reading() {
        let file = |k: u32| media::MediaFile {
            path: PathBuf::from(format!("/disc/IM{k}")),
            present: true,
            ..Default::default()
        };
        let series = |uid: &str, files: Vec<media::MediaFile>| media::MediaSeries {
            uid: uid.to_owned(),
            images: files,
            ..Default::default()
        };
        let medium = |a: Vec<media::MediaFile>, b: Vec<media::MediaFile>| media::Medium {
            patients: vec![media::Patient {
                studies: vec![media::Study {
                    series: vec![series("1", a), series("2", b)],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let (tx, rx) = mpsc::channel();
        let mut s = StudiesMode {
            reading: Some(Progress::default()),
            thumbs: Some(tx),
            medium: medium(vec![file(1)], vec![file(7)]),
            ..Default::default()
        };
        s.ask_thumbs();
        let got: Vec<(u64, PathBuf)> = rx.try_iter().collect();
        assert_eq!(
            got,
            vec![
                (1, PathBuf::from("/disc/IM1")),
                (2, PathBuf::from("/disc/IM7"))
            ]
        );
        // More files arrive: nothing asked again while reading.
        s.medium = medium(vec![file(1), file(2), file(3)], vec![file(7)]);
        s.ask_thumbs();
        assert_eq!(rx.try_iter().count(), 0);
        // The scan ends: series 1's middle image; series 2 already shows it.
        s.reading = None;
        s.ask_thumbs();
        let got: Vec<(u64, PathBuf)> = rx.try_iter().collect();
        assert_eq!(got, vec![(1, PathBuf::from("/disc/IM2"))]);
        assert!(s.provisional.is_empty() && s.asked.len() == 2);
    }

    /// A panicked read or series open leaves no "Reading…" or "Opening…" behind: the
    /// browser shows an internal error instead.
    #[test]
    fn a_lost_task_ends_the_waiting() {
        let mut s = StudiesMode {
            reading: Some(Progress::default()),
            ..Default::default()
        };
        assert!(s.lost("dcm-st-read"));
        assert!(s.reading.is_none());
        assert_eq!(s.error, Some(t!("error-reason-internal")));
        let mut s = StudiesMode {
            opening: Some(3),
            ..Default::default()
        };
        assert!(s.lost("dcm-st-series"));
        assert!(s.opening.is_none() && s.error.is_some());
        // Not the browser's: nothing changes.
        let mut s = StudiesMode::default();
        assert!(!s.lost("conv-batch"));
        assert!(s.lost("dcm-st-detect"));
        assert!(s.error.is_none());
    }

    /// "Show studies on this disc" finds the DICOMDIR above the folder (off the UI thread),
    /// else reads the folder's files; a lost search is the browser's and changes nothing.
    #[test]
    fn the_medium_is_found_above_the_folder() {
        let root = std::env::temp_dir().join(format!("specular-studies-{}", std::process::id()));
        let deep = root.join("DICOM").join("PAT1").join("ST1");
        std::fs::create_dir_all(&deep).unwrap();
        assert!(matches!(medium_of(deep.clone()), Detected::Files(d) if d == deep));
        std::fs::write(root.join("DICOMDIR"), b"").unwrap();
        assert!(
            matches!(medium_of(deep.clone()), Detected::Dicomdir(p) if p == root.join("DICOMDIR"))
        );
        // Four folders up is too far.
        let deeper = deep.join("SE1");
        std::fs::create_dir_all(&deeper).unwrap();
        assert!(matches!(medium_of(deeper.clone()), Detected::Files(d) if d == deeper));
        let mut s = StudiesMode::default();
        assert!(s.lost("dcm-st-find"));
        assert!(s.error.is_none() && s.reading.is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A thumbnail that panics stays empty; the next file is still read.
    #[test]
    fn a_panicking_thumbnail_is_skipped() {
        let cancel = AtomicBool::new(false);
        let bad = PathBuf::from(format!(
            "/nonexistent/{}.dcm",
            specular_core::isolate::TEST_PANIC_MARK
        ));
        assert!(study_thumb(&bad, &cancel).is_none());
        assert!(study_thumb(Path::new("/nonexistent/plain.dcm"), &cancel).is_none());
    }

    #[test]
    fn dates_and_names() {
        assert!(dicom_date("20240203").is_some());
        assert!(dicom_date("2024.02.03").is_some());
        assert_eq!(dicom_date(""), None);
        assert_eq!(dicom_date("2024"), None);
        assert_eq!(dicom_date("20241399"), None);
        let dd = |p: &str| medium_name(&Detected::Dicomdir(PathBuf::from(p)));
        assert_eq!(dd("/run/media/u/DISC/DICOMDIR"), "DISC");
        assert_eq!(dd("/run/media/u/DISC/DICOM/DICOMDIR"), "DISC");
        assert_eq!(dd("/run/media/u/DISC/IHE_PDI/DICOMDIR"), "DISC");
        assert_eq!(medium_name(&Detected::Files("/x/scan".into())), "scan");
    }

    #[test]
    fn rows_of_a_medium() {
        let mut s = StudiesMode::default();
        let file = |n: &str, present| media::MediaFile {
            path: PathBuf::from(n),
            present,
            frames: 1,
            ..Default::default()
        };
        s.medium = Medium {
            dicomdir: None,
            patients: vec![media::Patient {
                name: "SECRET^NAME".into(),
                id: "ID-1".into(),
                birth_date: "19700101".into(),
                studies: vec![media::Study {
                    uid: "1.2".into(),
                    date: "20240203".into(),
                    description: "CT Thorax".into(),
                    series: vec![
                        media::MediaSeries {
                            uid: "1.2.1".into(),
                            modality: "CT".into(),
                            number: Some(2),
                            description: "Lung".into(),
                            images: vec![file("a", true), file("b", false)],
                            ..Default::default()
                        },
                        media::MediaSeries {
                            uid: "1.2.2".into(),
                            modality: "MR".into(),
                            images: vec![file("c", false)],
                            ..Default::default()
                        },
                    ],
                }],
            }],
            missing: 2,
        };
        let a = s.id_of("1.2.1");
        let b = s.id_of("1.2.2");
        assert_ne!(a, b);
        assert_eq!(s.id_of("1.2.1"), a, "ids are stable");
        let sheet = s.sheet();
        assert_eq!(sheet.rows.len(), 4);
        let p = &sheet.rows[0];
        assert!(p.identifying);
        assert_eq!(p.title, "SECRET, NAME");
        assert!(!p.hidden_title.contains("SECRET"));
        assert_eq!(sheet.rows[2].id, a);
        assert!(sheet.rows[2].title.contains("Lung"));
        assert!(!sheet.rows[2].missing);
        assert!(sheet.rows[3].missing, "no image on the medium");
        assert!(sheet.note.is_some());
        // Nothing that names the patient outside the patient row.
        for r in &sheet.rows[1..] {
            assert!(!r.title.contains("SECRET") && !r.detail.contains("SECRET"));
        }
        assert!(!sheet.title.contains("SECRET"));
        // Two series: the browser offers the choice.
        assert_eq!(s.only_series(), None);
    }

    /// A medium of one series with images opens it at once, but only once read whole.
    #[test]
    fn one_series_needs_no_browser() {
        let mut s = StudiesMode::default();
        let series = |present| media::MediaSeries {
            uid: "1.2.1".into(),
            modality: "CT".into(),
            images: vec![media::MediaFile {
                path: PathBuf::from("a"),
                present,
                frames: 1,
                ..Default::default()
            }],
            ..Default::default()
        };
        let medium = |present| Medium {
            dicomdir: None,
            patients: vec![media::Patient {
                studies: vec![media::Study {
                    uid: "1.2".into(),
                    series: vec![series(present)],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            missing: 0,
        };
        s.medium = medium(true);
        s.reading = Some(Progress::default());
        assert_eq!(s.only_series(), None, "still reading");
        s.reading = None;
        s.capped = true;
        assert_eq!(s.only_series(), None, "more files than were read");
        s.capped = false;
        let id = s.only_series().expect("one series");
        assert_eq!(id, s.id_of("1.2.1"));
        s.medium = medium(false);
        assert_eq!(s.only_series(), None, "no image on the medium");
    }
}

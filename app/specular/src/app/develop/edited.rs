//! Develop in the gallery and the Library:
//!
//! - **Facts** ([`facts_of`]): what each version's edit uses (its output hash, the modules
//!   that change the picture, the styles applied), for `edited:`, `module:`, `style:` and
//!   `version:`, the badges and the edited thumbnails. Read off the UI thread from the
//!   sidecars and the catalogue, as Develop reads them.
//! - **Changes** ([`take_changed`]): every save (an edit, a paste, a style, a sync) says
//!   which photo changed, so its tile re-develops; the old thumbnail stays meanwhile.
//! - **Edited thumbnails** ([`EditedThumbs`]): the edit developed at 512 px on the GPU by
//!   one background worker, visible tiles first, newest asked first otherwise; a raw
//!   developed at 1024 px first, then reduced (512 with Settings › Gallery › Sharper raw
//!   thumbnails off). Cached in the thumbnail cache by (file, edit hash, graph version,
//!   and a raw's develop size), so a photo developed once shows from the cache after that.
//!   Unedited photos never come here.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, unbounded};
use specular_core::CancelToken;
use specular_develop::gpu::pool::Budget;
use specular_develop::graph::GRAPH_VERSION;
use specular_develop::model::{Edit, History, Registry, hash, summary};
use specular_develop::preview as engine;
use specular_develop::sidecar;
use specular_develop::thumb::{self, Source, Thumbnailer};
use specular_gfx::wgpu;
use specular_library::Library;
use specular_thumbs::pixels::Pixels;
use specular_thumbs::{DiskCache, ThumbKey};

/// An edit's output hash.
pub(in crate::app) type Hash = [u8; 32];

/// Cache flags of edited thumbnails (their own keys: no quality options apply).
const FLAGS: u32 = 0;

/// The names styles are applied by (shipped ones in the UI language, and the user's), set
/// from the UI thread when the shelf is read: the history names a style's step by them.
static STYLE_NAMES: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Photos whose edit was saved since the last [`take_changed`].
static CHANGED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(in crate::app) fn set_style_names(names: Vec<String>) {
    if let Ok(mut s) = STYLE_NAMES.write() {
        *s = names;
    }
}

fn style_names() -> Vec<String> {
    STYLE_NAMES.read().map(|s| s.clone()).unwrap_or_default()
}

/// What `edit` uses, as the catalogue keeps it: (modules, styles), one per line.
pub(super) fn summary_of(edit: &Edit, history: &History) -> (String, String) {
    let names = style_names();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    // A raw's starting noise reduction and sharpening are not something used on it.
    let edit = super::controls::without_raw_defaults(edit);
    summary::summary_lines(&edit, history, Registry::builtin(), &names)
}

/// Says that `path`'s edit was saved (any version): its tile re-develops.
pub(super) fn changed(path: &Path) {
    if let Ok(mut c) = CHANGED.lock()
        && !c.iter().any(|p| p == path)
    {
        c.push(path.to_path_buf());
    }
}

/// The photos whose edit was saved since the last call.
pub(in crate::app) fn take_changed() -> Vec<PathBuf> {
    CHANGED
        .lock()
        .map(|mut c| std::mem::take(&mut *c))
        .unwrap_or_default()
}

/// What one version's edit uses.
#[derive(Clone, Debug)]
pub(in crate::app) struct Facts {
    pub version: u32,
    /// The version's name (empty: none).
    pub name: String,
    pub hash: Hash,
    /// Stable module ids and the styles applied, one per line.
    pub modules: String,
    pub styles: String,
    pub edit: Arc<Edit>,
}

/// Every version of `path` with a saved edit (sidecars and catalogue, the newer of each),
/// in order. Off the UI thread.
pub(in crate::app) fn facts_of(path: &Path, lib: Option<&Library>) -> Vec<Facts> {
    super::store::versions_of(path, lib)
        .into_iter()
        .filter_map(|(version, name)| {
            let found = super::store::load_version(path, version, lib)?;
            let (modules, styles) = summary_of(&found.edit, &found.history);
            Some(Facts {
                version,
                name: if found.name.is_empty() {
                    name
                } else {
                    found.name
                },
                hash: hash::edit_hash(&found.edit),
                modules,
                styles,
                edit: Arc::new(found.edit),
            })
        })
        .collect()
}

/// The photos in `dir` with a Develop sidecar (their first version's, or a numbered
/// version's): one listing of the folder.
pub(in crate::app) fn edited_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .filter_map(|e| {
            let n = e.file_name();
            let rest = n.to_str()?.strip_suffix(sidecar::SUFFIX)?;
            // `IMG.CR2.v2` → `IMG.CR2`.
            let image = match rest.rsplit_once(".v") {
                Some((i, v)) if !i.is_empty() && v.parse::<u32>().is_ok_and(|v| v > 0) => i,
                _ => rest,
            };
            (!image.is_empty()).then(|| dir.join(image))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The key a version's tile and thumbnail go by: the photo's own path for the first
/// version, its sidecar's path for the others (a real, unique path that is never an image).
pub(in crate::app) fn version_key(image: &Path, version: u32) -> PathBuf {
    if version == 0 {
        image.to_path_buf()
    } else {
        sidecar::path_for_version(image, version)
    }
}

/// One edited thumbnail to make.
#[derive(Clone, Debug)]
pub(in crate::app) struct Job {
    /// The tile's key ([`version_key`]).
    pub key: PathBuf,
    pub image: PathBuf,
    pub hash: Hash,
    pub edit: Arc<Edit>,
}

/// An edited thumbnail made (or not: `pixels` None, with why).
pub(in crate::app) struct Done {
    pub key: PathBuf,
    pub hash: Hash,
    /// The thumbnail in each class of [`specular_thumbs::CLASSES`], smallest first.
    pub pixels: Result<[Arc<Pixels>; 3], String>,
    /// Read from the cache (not developed).
    pub cached: bool,
    /// Wall time, read to thumbnail.
    pub ms: f64,
}

#[derive(Default)]
struct Queue {
    jobs: Vec<Job>,
    /// Keys on screen, in their order: asked first.
    visible: Vec<PathBuf>,
    stop: bool,
}

struct Shared {
    q: Mutex<Queue>,
    cv: std::sync::Condvar,
    /// The long side raws are developed at ([`thumb::raw_long`]).
    raw_long: AtomicU32,
}

/// The edited-thumbnail worker (see the module docs).
pub(in crate::app) struct EditedThumbs {
    shared: Arc<Shared>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    started: bool,
    /// Timings of thumbnails developed (not read from the cache), ms.
    pub timings: Vec<f64>,
}

impl Default for EditedThumbs {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        EditedThumbs {
            shared: Arc::new(Shared {
                q: Mutex::new(Queue::default()),
                cv: std::sync::Condvar::new(),
                raw_long: AtomicU32::new(thumb::RAW_LONG),
            }),
            tx,
            rx,
            started: false,
            timings: Vec::new(),
        }
    }
}

impl Drop for EditedThumbs {
    fn drop(&mut self) {
        if let Ok(mut q) = self.shared.q.lock() {
            q.stop = true;
        }
        self.shared.cv.notify_all();
    }
}

impl EditedThumbs {
    /// Starts the worker on the app's device (once).
    pub fn start(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        integrated: bool,
        registry: Arc<specular_decode::Registry>,
        cache: Option<Arc<DiskCache>>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) {
        if self.started {
            return;
        }
        self.started = true;
        let w = Worker {
            shared: self.shared.clone(),
            tx: self.tx.clone(),
            device: device.clone(),
            queue: queue.clone(),
            integrated,
            registry,
            cache,
            wake,
        };
        if let Err(e) = specular_thread::builder("edited-thumbs").spawn(move || w.run()) {
            specular_log::warn!(target: specular_log::GPU, "edited thumbnails: no worker: {e}");
            self.started = false;
        }
    }

    pub fn started(&self) -> bool {
        self.started
    }

    /// The long side raws are developed at for their thumbnails ([`thumb::raw_long`]).
    #[cfg(test)]
    pub fn raw_long(&self) -> u32 {
        self.shared.raw_long.load(Ordering::Relaxed)
    }

    /// Sets the long side raws are developed at ([`thumb::raw_long`]); jobs not started
    /// yet take it. True when it changed (the caller asks again for the raws' thumbnails).
    pub fn set_raw_long(&self, long: u32) -> bool {
        self.shared.raw_long.swap(long, Ordering::Relaxed) != long
    }

    /// Asks for `job` (replacing one for the same key not started yet).
    pub fn ask(&self, job: Job) {
        if let Ok(mut q) = self.shared.q.lock() {
            q.jobs.retain(|j| j.key != job.key);
            q.jobs.push(job);
        }
        self.shared.cv.notify_all();
    }

    /// The keys on screen, in order: their thumbnails are made first.
    pub fn set_visible(&self, keys: Vec<PathBuf>) {
        if let Ok(mut q) = self.shared.q.lock() {
            q.visible = keys;
        }
    }

    /// Forgets every job not started (another folder).
    pub fn clear(&self) {
        if let Ok(mut q) = self.shared.q.lock() {
            q.jobs.clear();
            q.visible.clear();
        }
    }

    /// Thumbnails made since the last call.
    pub fn take(&mut self) -> Vec<Done> {
        let v: Vec<Done> = self.rx.try_iter().collect();
        for d in &v {
            if !d.cached && d.pixels.is_ok() {
                self.timings.push(d.ms);
            }
        }
        v
    }
}

struct Worker {
    shared: Arc<Shared>,
    tx: Sender<Done>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    integrated: bool,
    registry: Arc<specular_decode::Registry>,
    cache: Option<Arc<DiskCache>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Worker {
    fn next(&self) -> Option<Job> {
        let mut q = self.shared.q.lock().ok()?;
        loop {
            if q.stop {
                return None;
            }
            if !q.jobs.is_empty() {
                // On screen first (in screen order), else the newest asked.
                let at = q
                    .visible
                    .iter()
                    .find_map(|k| q.jobs.iter().position(|j| &j.key == k))
                    .unwrap_or(q.jobs.len() - 1);
                return Some(q.jobs.remove(at));
            }
            q = self.shared.cv.wait(q).ok()?;
        }
    }

    fn run(self) {
        let mut th: Option<Thumbnailer> = None;
        let mut software = false;
        while let Some(job) = self.next() {
            let t0 = Instant::now();
            // One size for the whole job (the setting may change meanwhile).
            let raw_long = self.shared.raw_long.load(Ordering::Relaxed);
            let (pixels, cached) = match self.cached(&job, raw_long) {
                Some(px) => (Ok(px), true),
                None => {
                    let r = specular_core::isolate::run("edited thumbnail", || {
                        self.develop(&job, raw_long, &mut th, &mut software)
                    })
                    .unwrap_or_else(|p| Err(p.brief()));
                    (r, false)
                }
            };
            if let Err(e) = &pixels {
                specular_log::debug!(target: specular_log::CACHE,
                    "edited thumbnail of {}: {e}", job.image.display());
            }
            let _ = self.tx.send(Done {
                key: job.key,
                hash: job.hash,
                pixels: pixels.map(classes),
                cached,
                ms: t0.elapsed().as_secs_f64() * 1e3,
            });
            (self.wake)();
        }
    }

    fn cache_key(job: &Job, raw_long: u32) -> Option<ThumbKey> {
        ThumbKey::of_file(&job.image).ok().map(|k| {
            k.edited_at(
                &job.hash,
                GRAPH_VERSION,
                develop_long_key(&job.image, raw_long),
            )
        })
    }

    fn cached(&self, job: &Job, raw_long: u32) -> Option<Pixels> {
        let cache = self.cache.as_ref()?;
        let key = Self::cache_key(job, raw_long)?;
        cache.get(&key, thumb::LONG, FLAGS).map(|e| e.pixels)
    }

    fn develop(
        &self,
        job: &Job,
        raw_long: u32,
        th: &mut Option<Thumbnailer>,
        software: &mut bool,
    ) -> Result<Pixels, String> {
        let src = read_source(&self.registry, &job.image, thumb::source_long(&job.edit))?;
        let budget = Budget::for_device(self.integrated || *software, None, None);
        let t = th.get_or_insert_with(|| Thumbnailer::new(&self.device, &self.queue, budget));
        t.raw_long = Some(raw_long);
        let (w, h, rgba) = match t.render(src, &job.edit, thumb::LONG) {
            Ok(r) => r,
            Err(specular_develop::gpu::Error::Gpu(e)) if !*software => {
                // The app's GPU cannot develop: the software adapter (D2).
                specular_log::warn!(target: specular_log::GPU,
                    "edited thumbnails: {e}; on the software adapter");
                let (d, q) = engine::open_software_device()?;
                *software = true;
                let mut t = Thumbnailer::new(&d, &q, Budget::for_device(true, None, None));
                t.raw_long = Some(raw_long);
                *th = Some(t);
                let src = read_source(&self.registry, &job.image, thumb::source_long(&job.edit))?;
                th.as_mut()
                    .ok_or("no developer")?
                    .render(src, &job.edit, thumb::LONG)
                    .map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };
        let px = Pixels::new(w, h, rgba);
        if let Some(cache) = &self.cache
            && let Some(key) = Self::cache_key(job, raw_long)
            && let Err(e) = cache.put(
                &key,
                thumb::LONG,
                FLAGS,
                &px,
                &specular_thumbs::Meta::default(),
            )
        {
            specular_log::debug!(target: specular_log::CACHE, "edited thumbnail not cached: {e}");
        }
        Ok(px)
    }
}

/// The develop size an edited thumbnail of `image` is keyed by: 0 (the old key) when it is
/// developed at the thumbnail's own size (a non-raw, or a raw at 512), else `raw_long`.
fn develop_long_key(image: &Path, raw_long: u32) -> u32 {
    if raw_long == thumb::LONG || !thumb::may_be_raw(image) {
        0
    } else {
        raw_long
    }
}

/// Reads `path` for a thumbnail: a raw's sensor data, else the image decoded at about
/// `side` ([`thumb::source_long`]: larger when the edit's warp resamples it).
fn read_source(
    registry: &specular_decode::Registry,
    path: &Path,
    side: u32,
) -> Result<Source, String> {
    if let Some(s) = thumb::read_raw(path)? {
        return Ok(s);
    }
    let s = registry.open_path(path).map_err(|e| e.to_string())?;
    let info = s.info();
    let orientation = u16::from(info.orientation.0);
    let long = info.width.max(info.height).max(1) as usize;
    let scale = info
        .scales
        .iter()
        .copied()
        .filter(|&k| long / (k.max(1) as usize) >= side as usize || k == 1)
        .max()
        .unwrap_or(1);
    let icc: Option<Vec<u8>> = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| s.embedded_icc().map(<[u8]>::to_vec));
    let cancel = CancelToken::new();
    let r = s
        .decode(&specular_decode::DecodeRequest::full(scale), &cancel)
        .map_err(|e| e.to_string())?;
    // Through its profile, unclipped, as Develop and export read it (not as 8-bit sRGB):
    // a Display P3 or 16-bit photo's thumbnail keeps the colours its export has.
    let input = specular_develop::source::input(&r, icc.as_deref(), Some(side), &cancel)
        .ok_or("cancelled")?;
    Ok(Source {
        input,
        orientation,
        frame: None,
        lensfun: None,
        focal35: focal35_of_path(path),
    })
}

/// The file's 35 mm equivalent focal length (its head holds the Exif), for perspective.
pub(super) fn focal35_of_path(path: &Path) -> Option<f64> {
    use std::io::Read;
    let mut head = Vec::new();
    let f = std::fs::File::open(path).ok()?;
    f.take(1 << 20).read_to_end(&mut head).ok()?;
    specular_develop::ops::geometry::persp::focal35_of_file(&head)
}

/// `px` (the 512 class) and its 256 and 128 reductions, smallest first.
fn classes(px: Pixels) -> [Arc<Pixels>; 3] {
    let big = Arc::new(px);
    let mid = Arc::new(reduce(&big, 256));
    let small = Arc::new(reduce(&mid, 128));
    [small, mid, big]
}

/// `px` area-reduced to a long side of at most `long` (unchanged when it fits).
pub(in crate::app) fn reduce(px: &Pixels, long: u32) -> Pixels {
    let (w, h) = (px.width, px.height);
    let (dw, dh) = engine::fit(w, h, long);
    if (dw, dh) == (w, h) {
        return Pixels::new(w, h, px.rgba.clone());
    }
    let mut out = vec![0u8; dw as usize * dh as usize * 4];
    let (sx, sy) = (w as f32 / dw as f32, h as f32 / dh as f32);
    for oy in 0..dh {
        let y0 = (oy as f32 * sy) as u32;
        let y1 = (((oy + 1) as f32 * sy).ceil() as u32).clamp(y0 + 1, h);
        for ox in 0..dw {
            let x0 = (ox as f32 * sx) as u32;
            let x1 = (((ox + 1) as f32 * sx).ceil() as u32).clamp(x0 + 1, w);
            let mut acc = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) as usize * 4;
                    for (a, v) in acc.iter_mut().zip(&px.rgba[i..i + 4]) {
                        *a += u32::from(*v);
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let o = (oy * dw + ox) as usize * 4;
            for (d, a) in out[o..o + 4].iter_mut().zip(acc) {
                *d = ((a + n / 2) / n) as u8;
            }
        }
    }
    Pixels::new(dw, dh, out)
}

/// A version's name in words: its name, else "Version 2".
pub(in crate::app) fn version_label(version: u32, name: &str) -> String {
    super::batch::version_label(version, name)
}

impl crate::app::App {
    /// Tells the summaries the names styles are applied by, and `module:` the modules'
    /// names in the UI language.
    pub(in crate::app) fn develop_publish_style_names(&mut self) {
        let shelf = &mut self.develop.batch.shelf;
        shelf.load();
        let mut names: Vec<String> = Vec::new();
        for l in shelf.listed() {
            for n in [l.name, l.style.name] {
                if !n.is_empty() && !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        set_style_names(names);
        publish_module_names();
    }

    /// Cull mode's words for an item's edits: "Edited" when one of its files has a
    /// Specular edit (the gallery's facts, else a sidecar beside it), "3 versions".
    pub(in crate::app) fn develop_cull_flags(&self, members: &[PathBuf]) -> Vec<String> {
        let mut v = Vec::new();
        if !self.settings.gallery.edited_badges {
            return v;
        }
        let edited = members.iter().any(|p| {
            self.gallery_edited(p)
                .unwrap_or_else(|| sidecar::path_for(p).is_file())
        });
        if edited {
            v.push(specular_i18n::t!("gallery-badge-edited"));
        }
        if let Some(n) = members
            .iter()
            .filter_map(|p| self.gallery_versions(p))
            .max()
        {
            v.push(specular_i18n::t!(
                "gallery-badge-versions",
                count = i64::from(n)
            ));
        }
        v
    }

    /// Opens `photo` (the viewer's current image) in Develop at version `version`.
    pub(in crate::app) fn develop_open_at(&mut self, photo: &Path, version: u32) {
        let current = self.current.and_then(|i| self.entries.get(i));
        if current.map(PathBuf::as_path) != Some(photo) {
            return;
        }
        if !self.develop.active() {
            self.develop_action(specular_input::Action::ToggleDevelop);
        }
        if version > 0
            && self
                .develop
                .session
                .as_ref()
                .is_some_and(|s| s.path == photo)
        {
            if !self.develop.batch.versions.iter().any(|v| v.0 == version) {
                self.develop.batch.versions.push((version, String::new()));
            }
            self.develop_open_version(version);
        }
    }
}

/// The modules' stable ids with their names in the UI language, for `module:`.
pub(in crate::app) fn publish_module_names() {
    let other = specular_i18n::t!("develop-module-other");
    let names = Registry::builtin()
        .specs()
        .iter()
        .map(|s| (s.name.to_owned(), super::controls::module_title(s.kind)))
        .filter(|(_, title)| *title != other)
        .collect();
    specular_query::set_module_names(names);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::current_dir()
            .unwrap()
            .join("../../target/test-scratch")
            .join(format!("edited-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn edited_photos_are_found_by_their_sidecars() {
        let d = scratch("listing");
        for n in [
            "a.CR2",
            "a.CR2.specular.xmp",
            "b.jpg",
            "b.jpg.v2.specular.xmp",
            "c.jpg",
            "c.jpg.xmp",
            "d.v1.jpg",
            "d.v1.jpg.specular.xmp",
        ] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        assert_eq!(
            edited_in(&d),
            vec![d.join("a.CR2"), d.join("b.jpg"), d.join("d.v1.jpg")]
        );
        assert_eq!(version_key(&d.join("a.CR2"), 0), d.join("a.CR2"));
        assert_eq!(
            version_key(&d.join("a.CR2"), 3),
            d.join("a.CR2.v3.specular.xmp")
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn reductions_keep_the_picture() {
        let mut rgba = Vec::new();
        for y in 0..300u32 {
            for x in 0..512u32 {
                rgba.extend_from_slice(&[(x / 2) as u8, (y / 2) as u8, 200, 255]);
            }
        }
        let [s, m, b] = classes(Pixels::new(512, 300, rgba));
        assert_eq!((b.width, b.height), (512, 300));
        assert_eq!((m.width, m.height), (256, 150));
        assert_eq!((s.width, s.height), (128, 75));
        // The corner and the middle keep their colour.
        assert_eq!(&s.rgba[..4], &[1, 1, 200, 255]);
        let mid = (37 * 128 + 64) * 4;
        assert!((i32::from(s.rgba[mid]) - 128).abs() <= 2);
    }

    /// Raws are keyed by their develop size (Sharper raw thumbnails on: 1024), so switching
    /// makes them again and never shows one size for the other; at 512 a raw keeps the key
    /// it had before the setting, and other images keep theirs at either size.
    #[test]
    fn edited_thumbnails_are_keyed_by_the_raw_develop_size() {
        let raw = Path::new("/p/IMG_1.CR2");
        let jpeg = Path::new("/p/IMG_1.jpg");
        assert_eq!(develop_long_key(raw, thumb::raw_long(true)), 1024);
        assert_eq!(develop_long_key(raw, thumb::raw_long(false)), 0);
        assert_eq!(develop_long_key(jpeg, thumb::raw_long(true)), 0);
        assert_eq!(develop_long_key(jpeg, thumb::raw_long(false)), 0);
        let d = scratch("size-key");
        let file = d.join("a.dng");
        std::fs::write(&file, b"not really a raw").unwrap();
        let job = Job {
            key: file.clone(),
            image: file.clone(),
            hash: [3; 32],
            edit: Arc::new(Edit::default_for(
                Registry::builtin(),
                specular_develop::model::SourceKey::default(),
                true,
            )),
        };
        let sharp = Worker::cache_key(&job, 1024).unwrap();
        let fast = Worker::cache_key(&job, 512).unwrap();
        assert_ne!(sharp, fast);
        assert_eq!(
            fast,
            ThumbKey::of_file(&file)
                .unwrap()
                .edited(&[3; 32], GRAPH_VERSION)
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_worker_takes_the_raw_develop_size() {
        let w = EditedThumbs::default();
        assert_eq!(w.raw_long(), thumb::RAW_LONG);
        assert!(!w.set_raw_long(1024));
        assert!(w.set_raw_long(512));
        assert_eq!(w.raw_long(), 512);
        assert!(!w.set_raw_long(512));
    }

    #[test]
    fn saves_are_said_once() {
        let _ = take_changed();
        changed(Path::new("/p/a.jpg"));
        changed(Path::new("/p/a.jpg"));
        changed(Path::new("/p/b.jpg"));
        let c = take_changed();
        assert!(c.contains(&PathBuf::from("/p/a.jpg")) && c.len() >= 2);
    }

    #[test]
    fn summaries_name_styles_by_the_shelf() {
        use specular_develop::model::{ApplyMode, ModuleKind, SourceKey, Style, Value};
        let reg = Registry::builtin();
        let mut e = Edit::default_for(reg, SourceKey::default(), true);
        let mut h = History::default();
        let mut src = e.clone();
        let m = src.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
        let mut f = m.params.to_fields();
        f.set("exposure_ev", Value::Float(1.0));
        m.params.set_fields(f);
        let style = Style::from_edit("Lift", &src, |m| m.kind == ModuleKind::EXPOSURE);
        assert!(style.paste(&mut e, &mut h, reg, ApplyMode::Replace, "Lift", 1));
        set_style_names(vec!["Lift".into()]);
        let (modules, styles) = summary_of(&e, &h);
        assert_eq!(modules, "exposure");
        assert_eq!(styles, "Lift");
    }
}

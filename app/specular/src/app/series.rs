//! DICOM series in the viewer, and the tag browser's content.
//!
//! Opening a DICOM file (by its extension, or a file without one) also reads the headers of
//! the other files in its folder, and of the files of its series a DICOMDIR of its medium
//! names, in the background ([`specular_dicom::series::scan`]: headers only, at most
//! 20 000 files, cancelled when something else opens). The folder's own listing waits for
//! it. When the file belongs to a series of two or more images, or the folder holds two or
//! more series, the viewer browses the series instead of the folder: its images in slice
//! order, Page Up/Down and the wheel step slices, Ctrl+Page Up/Down change series, and the
//! info panel has a Series group (slice i of n, location, spacing, gaps). Anything else
//! that replaces the listing (another open, the gallery, cull mode) ends series mode.
//!
//! A multi-frame file is one item of its series; its frames are stepped within the file.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use specular_dicom::series::{self, Scan, ScanLimits, Series, SeriesOrder};
use specular_dicom::{TagTree, TagValue};
use specular_i18n::{I18n, t};
use specular_log::{FOLDER, debug};
use specular_metadata::{MetadataReport, SummaryGroup, SummaryItem};
use specular_thumbs::watch::{Changes, TreeWatch};
use specular_widgets::{DicomTags, TagLine};

use super::App;
use crate::loader::{Event, ImageId};

/// Series-mode state.
#[derive(Default)]
pub(super) struct SeriesMode {
    /// The scan in flight: the file it is for, and its cancel flag.
    pending: Option<(PathBuf, Arc<AtomicBool>)>,
    /// The folder's listing, when it came while the scan ran.
    held: Option<(PathBuf, Vec<PathBuf>)>,
    /// In series mode: the series found (none outside it).
    sets: Vec<Series<PathBuf>>,
    /// The folder held more files than were read.
    capped: bool,
    /// The series browsed.
    active: usize,
    /// Each file's series and position in it.
    at: HashMap<PathBuf, (usize, usize)>,
    /// The folder the scan was for.
    dir: Option<PathBuf>,
    /// The tag browser's read in flight: the file, and its cancel flag.
    tags: Option<(PathBuf, Arc<AtomicBool>)>,
    /// Presentation states (GSPS) found by the scan, in or out of series mode.
    presentations: Vec<(PathBuf, series::Instance)>,
    /// Files of whole slides in the folder the viewer steps over: each slide is one image
    /// (its full-resolution level, or the file opened), as one tile in the gallery.
    hidden: HashSet<PathBuf>,
    /// The fingerprint of the folder's last listing ([`listing_print`]): a relist that
    /// differs rescans the folder in series mode.
    listed: Option<u64>,
    /// The scan in flight follows a relist in series mode: it may enter series mode at
    /// whichever of its files is current when it ends.
    rescan: bool,
    /// The series folder's watch, in series mode ([`App::series_watch_tick`]).
    watch: Option<TreeWatch>,
    /// The watch saw DICOM files come, go or change: the next listing reads the series
    /// again even when it lists the same files (a slice still being copied was skipped).
    watched: bool,
    /// The root of the medium when the series came through a DICOMDIR: its files may be in
    /// any folder of it, so the watch covers the medium ([`watch_target`]).
    media: Option<PathBuf>,
}

/// Whether settled folder changes may change the series: a file that may be DICOM came,
/// went or changed (sidecars and other files are left alone), or events were lost.
fn series_changes(ch: &Changes) -> bool {
    ch.rescan || ch.paths.iter().any(|p| is_dicom_name(p))
}

/// Folder levels watched under a DICOM medium's root (PS3.10 allows eight; one more for a
/// copy kept in a folder).
const MEDIA_WATCH_DEPTH: u32 = 9;

/// What series mode watches: the series folder alone, or, for a series a DICOMDIR names (its
/// files in other folders of the medium), the medium from the DICOMDIR's folder down.
fn watch_target(dir: &Path, media: Option<&Path>) -> (PathBuf, u32) {
    match media {
        Some(m) if dir.starts_with(m) => (m.to_path_buf(), MEDIA_WATCH_DEPTH),
        _ => (dir.to_path_buf(), 0),
    }
}

/// A fingerprint of a folder listing (its paths, in order).
fn listing_print(files: &[PathBuf]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    files.hash(&mut h);
    h.finish()
}

/// Whether a listing in series mode (fingerprint `now`) changes what the series scan saw:
/// one that differs from the last. The first one (the scan was quicker than the listing)
/// is what the scan read.
fn relist_changes(last: Option<u64>, now: u64) -> bool {
    last.is_some_and(|l| l != now)
}

/// The files of `slides` to step over: all but one per slide (`file` for its own slide, the
/// full-resolution level for the others).
fn slide_members(slides: &[Vec<PathBuf>], file: &Path) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    for g in slides {
        let keep = if g.iter().any(|p| p == file) {
            file
        } else {
            match g.first() {
                Some(p) => p.as_path(),
                None => continue,
            }
        };
        out.extend(g.iter().filter(|p| p.as_path() != keep).cloned());
    }
    out
}

/// What a series scan found, or why it could not run.
pub(crate) type ScanResult = Result<Scan, String>;

/// Whether `p` may be a DICOM file worth a series scan: a DICOM extension, or none (DICOM
/// media name their files without one).
pub(super) fn is_dicom_name(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        None => true,
        Some(e) => matches!(
            e.to_ascii_lowercase().as_str(),
            "dcm" | "dicom" | "dic" | "ima"
        ),
    }
}

impl SeriesMode {
    fn leave(&mut self) {
        self.sets.clear();
        self.at.clear();
        self.active = 0;
        self.capped = false;
    }
}

impl App {
    /// The series reader panicked: the image shows alone.
    pub(super) fn series_lost(&mut self, task: &str) {
        if task == "dcm-series" {
            self.series.pending = None;
        }
    }

    /// Whether the viewer browses a DICOM series (not the folder).
    pub(super) fn series_active(&self) -> bool {
        !self.series.sets.is_empty()
    }

    /// The UID of the series `file` belongs to, in series mode.
    pub(super) fn series_uid_of(&self, file: &Path) -> Option<String> {
        let &(k, _) = self.series.at.get(file)?;
        Some(self.series.sets.get(k)?.series_uid.clone())
    }

    /// Series found with the current file, in series mode (0 outside it).
    pub(super) fn series_count(&self) -> usize {
        self.series.sets.len()
    }

    /// Every series found, in series mode (linked views show them, `linked.rs`).
    pub(super) fn series_sets(&self) -> &[Series<PathBuf>] {
        &self.series.sets
    }

    /// The series shown, in series mode (MPR stacks it, `mpr.rs`).
    pub(super) fn series_set(&self) -> Option<&Series<PathBuf>> {
        self.series.sets.get(self.series.active)
    }

    /// Starts the series scan around `file` (just opened) when it may be DICOM.
    pub(super) fn series_start(&mut self, file: &Path) {
        self.series_close();
        if !is_dicom_name(file) || self.in_book() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.series.pending = Some((file.to_path_buf(), cancel.clone()));
        self.series.dir = file.parent().map(Path::to_path_buf);
        let f = file.to_path_buf();
        self.loader.background("dcm-series", move |l| {
            let t0 = Instant::now();
            // The file itself first: a file without an extension that is not DICOM costs
            // one header read, not the folder's.
            let scan = match series::read_header(&f) {
                Ok(Some(_)) => series::scan(&f, ScanLimits::default(), &cancel)
                    .map_err(|e| crate::error_text::dicom_reason(&e)),
                Ok(None) => Ok(Scan::default()),
                Err(e) => Err(crate::error_text::dicom_reason(&e)),
            };
            // Whole slides (each one image to the viewer, as one tile in the gallery).
            let slides = match &scan {
                Ok(s) => {
                    let paths: Vec<PathBuf> = s
                        .series
                        .iter()
                        .flat_map(|x| x.items.iter().map(|i| i.key.clone()))
                        .collect();
                    series::whole_slides(&paths, &|| cancel.load(Ordering::Relaxed))
                }
                Err(_) => Vec::new(),
            };
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            l.post(Event::DicomSeries {
                file: f,
                scan,
                slides,
                ms,
            });
        });
    }

    /// Cancels a scan in flight and leaves series mode (something else opens).
    pub(super) fn series_close(&mut self) {
        if let Some((_, c)) = self.series.pending.take() {
            c.store(true, Ordering::Relaxed);
        }
        self.series.held = None;
        self.series.dir = None;
        self.series.presentations.clear();
        self.series.hidden.clear();
        self.series.listed = None;
        self.series.media = None;
        self.series.rescan = false;
        self.series.leave();
    }

    /// In series mode, watches the series folder: once DICOM files copied in, removed or
    /// rewritten settle ([`specular_thumbs::watch::SETTLE`]), the folder is listed again and
    /// the series read again around the image shown. Returns when to look again.
    pub(super) fn series_watch_tick(&mut self) -> Option<Instant> {
        let dir = match &self.series.dir {
            Some(d) if self.series_active() || self.series.rescan => d.clone(),
            _ => {
                self.series.watch = None;
                return None;
            }
        };
        let (root, depth) = watch_target(&dir, self.series.media.as_deref());
        if !self
            .series
            .watch
            .as_ref()
            .is_some_and(|w| w.is(&root, depth, false) && !w.failed())
        {
            debug!(target: FOLDER, "series mode: watching {}", dir.display());
            let wake = self.loader.waker();
            self.series.watch = Some(TreeWatch::start("dcm-watch", &root, depth, false, wake));
        }
        let ch = self.series.watch.as_ref()?.take(Instant::now());
        if let Some(ch) = ch
            && series_changes(&ch)
            && self.folder.as_ref() == Some(&dir)
        {
            debug!(target: FOLDER, "series mode: {} files changed in {}", ch.paths.len(), dir.display());
            self.series.watched = true;
            self.scan_folder(dir);
        }
        self.series.watch.as_ref().and_then(TreeWatch::due)
    }

    /// The presentation states found with the open file.
    pub(super) fn series_presentations(&self) -> &[(PathBuf, series::Instance)] {
        &self.series.presentations
    }

    /// The listing is being replaced: series mode ends unless it is (what is left of) the
    /// series browsed.
    pub(super) fn series_listing_replaced(&mut self, files: &[PathBuf]) {
        if !self.series_active() {
            return;
        }
        let active = self.series.active;
        let same = files
            .iter()
            .all(|p| self.series.at.get(p).is_some_and(|&(s, _)| s == active));
        if !same || files.is_empty() {
            debug!(target: FOLDER, "series mode ends: the listing was replaced");
            self.series.leave();
        }
    }

    /// A folder listing arrived: held while the scan around a file of that folder runs; in
    /// series mode for that folder, ignored unless files came or went, when the series is
    /// read again (and the listing held for it). `Some`: not for series mode, list it.
    pub(super) fn series_hold_listing(
        &mut self,
        dir: PathBuf,
        mut files: Vec<PathBuf>,
    ) -> Option<(PathBuf, Vec<PathBuf>)> {
        // Before stepping over slides' files: what the folder holds.
        let listed = listing_print(&files);
        let mine = self.series.dir.as_ref() == Some(&dir) && !self.slideshow.running();
        if mine && self.series_active() {
            let changed = relist_changes(self.series.listed.replace(listed), listed);
            if !changed && !std::mem::take(&mut self.series.watched) {
                debug!(target: FOLDER, "series mode: not listing {}", dir.display());
                return None;
            }
            // Files came or went: the series is read again around the current file. The
            // listing waits for it, and is shown if no series is left.
            let current = self.current.and_then(|i| self.entries.get(i)).cloned()?;
            debug!(target: FOLDER, "series mode: {} changed, reading the series again", dir.display());
            self.series_start(&current);
            if self.series.pending.is_none() {
                return Some((dir, files));
            }
            self.series.dir = Some(dir.clone());
            self.series.rescan = true;
            self.series.listed = Some(listed);
            self.series.held = Some((dir, files));
            return None;
        }
        if self.series.dir.as_ref() == Some(&dir) && !self.series.hidden.is_empty() {
            files.retain(|p| !self.series.hidden.contains(p));
        }
        if mine && self.series.pending.is_some() {
            debug!(target: FOLDER, "holding the listing of {} for the series scan", dir.display());
            self.series.listed = Some(listed);
            self.series.held = Some((dir, files));
            return None;
        }
        Some((dir, files))
    }

    /// The scan's result: series mode when there is a series to browse, else the folder's
    /// listing as it came.
    pub(super) fn on_dicom_series(
        &mut self,
        file: PathBuf,
        scan: ScanResult,
        slides: Vec<Vec<PathBuf>>,
        ms: f64,
    ) {
        if self.series.pending.as_ref().map(|p| &p.0) != Some(&file) {
            return;
        }
        self.series.pending = None;
        let rescan = std::mem::take(&mut self.series.rescan);
        self.series.hidden = slide_members(&slides, &file);
        let held = self.series.held.take();
        let current = self.current.and_then(|i| self.entries.get(i)).cloned();
        match scan {
            // A whole slide opens as its whole pyramid: no series mode through its files.
            Ok(_) if slides.iter().any(|g| g.contains(&file)) => {
                debug!(target: FOLDER, "{} is a whole slide: no series mode", file.display());
            }
            Ok(mut s) => {
                let hidden = &self.series.hidden;
                for set in &mut s.series {
                    set.items.retain(|i| !hidden.contains(&i.key));
                }
                s.series.retain(|set| !set.items.is_empty());
                self.series.presentations = std::mem::take(&mut s.presentations);
                self.series.media = s
                    .dicomdir
                    .as_deref()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf);
                self.medical_gsps_refresh();
                debug!(
                    target: FOLDER,
                    "series scan around {}: {} headers, {} other files, {} series{}{} in {ms:.1} ms",
                    file.display(),
                    s.headers,
                    s.skipped,
                    s.series.len(),
                    if s.capped { " (capped)" } else { "" },
                    if s.dicomdir.is_some() { ", with a DICOMDIR" } else { "" }
                );
                // After a relist the user may have stepped on while the scan ran: the
                // series mode follows the image shown.
                let at = match &current {
                    Some(c) if rescan => c.clone(),
                    _ => file.clone(),
                };
                let mine = s
                    .series
                    .iter()
                    .position(|x| x.items.iter().any(|i| i.key == at));
                let enter = mine
                    .is_some_and(|k| s.series[k].items.len() >= 2 || s.series.len() >= 2)
                    && current.as_ref() == Some(&at);
                if let (true, Some(k)) = (enter, mine) {
                    self.series_enter(s, k, &at);
                    return;
                }
            }
            Err(e) => debug!(target: FOLDER, "series scan around {}: {e}", file.display()),
        }
        let hidden = &self.series.hidden;
        if let Some((dir, mut files)) = held {
            files.retain(|p| !hidden.contains(p));
            self.folder_listed(dir, files);
        } else if let Some(dir) = self.series.dir.clone()
            && self.entries.iter().any(|p| hidden.contains(p))
        {
            let files = self
                .entries
                .iter()
                .filter(|p| !hidden.contains(*p))
                .cloned()
                .collect();
            self.folder_listed(dir, files);
        }
    }

    /// Series mode over series streamed from a server (`dicomweb`): `sets` are keyed by the
    /// paths their instances have (or will have) in the disk cache; browses set `k` at `file`.
    pub(super) fn series_enter_streamed(
        &mut self,
        sets: Vec<Series<PathBuf>>,
        k: usize,
        file: &Path,
    ) {
        self.series_close();
        let scan = Scan {
            series: sets,
            ..Scan::default()
        };
        self.series_enter(scan, k, file);
    }

    pub(super) fn series_enter(&mut self, s: Scan, k: usize, file: &Path) {
        let m = &mut self.series;
        m.at.clear();
        for (si, set) in s.series.iter().enumerate() {
            for (ii, it) in set.items.iter().enumerate() {
                m.at.insert(it.key.clone(), (si, ii));
            }
        }
        m.sets = s.series;
        m.capped = s.capped;
        m.active = k;
        let paths = self.series_paths(k);
        let at = paths.iter().position(|p| p == file).unwrap_or(0);
        self.set_entries(paths);
        self.current = Some(at);
        self.schedule();
        self.series_toast();
        self.series_patch_all();
        self.update_title();
        self.redraw = true;
    }

    fn series_paths(&self, k: usize) -> Vec<PathBuf> {
        self.series.sets[k]
            .items
            .iter()
            .map(|i| i.key.clone())
            .collect()
    }

    fn series_toast(&mut self) {
        let m = &self.series;
        let set = &m.sets[m.active];
        let mut text = t!(
            "dicom-series-toast",
            index = (m.active + 1) as i64,
            series = m.sets.len() as i64,
            count = set.items.len() as i64
        );
        if m.capped {
            text = format!(
                "{text}. {}",
                t!(
                    "dicom-series-capped",
                    count = ScanLimits::default().files as i64
                )
            );
        }
        let now = self.now();
        self.chrome.toast(text, None, now);
    }

    /// In series mode, shows `file` when it is an image of another series found: that
    /// series becomes the one browsed, at the file. False when no series holds it.
    pub(super) fn series_show_file(&mut self, file: &Path) -> bool {
        if !self.series_active() {
            return false;
        }
        let Some(&(k, _)) = self.series.at.get(file) else {
            return false;
        };
        if k != self.series.active {
            self.series.active = k;
            let paths = self.series_paths(k);
            self.set_entries(paths);
            self.series_toast();
            self.series_patch_all();
        }
        match self.entries.iter().position(|p| p == file) {
            Some(i) => {
                self.go_to(i);
                true
            }
            None => false,
        }
    }

    /// Ctrl+Page Down / Up: the next or previous series, at its first image.
    pub(super) fn series_switch(&mut self, delta: i64) {
        let n = self.series.sets.len();
        if n < 2 {
            return;
        }
        let k = (self.series.active as i64 + delta).rem_euclid(n as i64) as usize;
        self.series.active = k;
        let paths = self.series_paths(k);
        self.set_entries(paths);
        self.nav_dir = if delta >= 0 { 1 } else { -1 };
        self.go_to(0);
        self.series_toast();
        self.series_patch_all();
    }

    /// Adds the Series group to the metadata of each image of the series that has one.
    fn series_patch_all(&mut self) {
        let ids: Vec<ImageId> = self
            .series
            .at
            .keys()
            .filter_map(|p| self.by_path.get(p).copied())
            .collect();
        for id in ids {
            self.series_patch(id);
        }
    }

    /// Adds (or replaces) the Series group in image `id`'s metadata.
    pub(super) fn series_patch(&mut self, id: ImageId) {
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let Some(r) = img.metadata.as_ref() else {
            return;
        };
        let group = self.series_group(&img.path);
        let has = r.summary.iter().any(|g| g.key == "series");
        if group.is_none() && !has {
            return;
        }
        let patched = with_series(r, group);
        if let Some(img) = self.images.get_mut(&id) {
            img.metadata = Some(Arc::new(patched));
        }
        self.redraw = true;
    }

    /// The info panel's Series group for the file at `path`, in series mode.
    fn series_group(&self, path: &Path) -> Option<SummaryGroup> {
        let m = &self.series;
        let &(si, ii) = m.at.get(path)?;
        let set = m.sets.get(si)?;
        let item = set.items.get(ii)?;
        Some(series_group(set, ii, item, si, m.sets.len()))
    }

    /// `t` (or the palette): opens the tag browser on the current DICOM file and reads its
    /// data set in the background; closes it when open.
    pub(super) fn dicom_tags_toggle(&mut self) {
        if self.chrome.dicom_tags_open() {
            self.chrome.close_modal();
            self.report_tags_closed();
            return;
        }
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.chrome.show_dicom_tags(DicomTags {
            title,
            status: Some(t!("dicom-tags-reading")),
            ..DicomTags::default()
        });
        if let Some((_, c)) = self.series.tags.take() {
            c.store(true, Ordering::Relaxed);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.series.tags = Some((path.clone(), cancel.clone()));
        self.loader.background("dcm-tags", move |l| {
            let t0 = Instant::now();
            let mut report = None;
            let tree = std::fs::read(&path)
                .map_err(|e| crate::error_text::io_reason(&e))
                .and_then(|data| {
                    let d = specular_dicom::Dicom::parse(&data)
                        .map_err(|e| crate::error_text::dicom_reason(&e))?;
                    // A structured report is browsed as its content tree.
                    if d.pixel.is_none() {
                        report = specular_dicom::sr::Report::read(&d, &data).ok().map(Box::new);
                    }
                    Ok(TagTree::build(&d, &data))
                });
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            // The count only: values are never logged.
            if let Ok(t) = &tree {
                debug!(target: FOLDER, "tag browser: {} rows read in {:.1} ms", t.rows.len(), t0.elapsed().as_secs_f64() * 1e3);
            }
            l.post(Event::DicomTags {
                file: path,
                tree,
                report,
            });
        });
    }

    /// The tag browser's data set, read.
    pub(super) fn on_dicom_tags(
        &mut self,
        file: PathBuf,
        tree: Result<TagTree, String>,
        report: Option<Box<specular_dicom::sr::Report>>,
    ) {
        if self.series.tags.as_ref().map(|t| &t.0) != Some(&file) {
            return;
        }
        self.series.tags = None;
        self.report_tags_closed();
        let title = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let content = match (tree, report) {
            (Ok(_), Some(r)) => self.report_tags(file, &r),
            (tree, _) => match tree {
                Ok(t) => tag_lines(&t, title),
                Err(reason) => DicomTags {
                    title,
                    status: Some(t!("dicom-tags-error", reason = reason)),
                    ..DicomTags::default()
                },
            },
        };
        self.chrome.set_dicom_tags(content);
        self.redraw = true;
    }
}

/// `report` with the Series group `group` (after the Medical group), or without one.
pub(super) fn with_series(report: &MetadataReport, group: Option<SummaryGroup>) -> MetadataReport {
    let mut r = report.clone();
    r.summary.retain(|g| g.key != "series");
    if let Some(g) = group {
        let at = r
            .summary
            .iter()
            .position(|g| g.key == "medical")
            .map_or(r.summary.len(), |i| i + 1);
        r.summary.insert(at, g);
    }
    r
}

fn item(key: &'static str, label: String, value: String, detail: Option<String>) -> SummaryItem {
    SummaryItem {
        key,
        label,
        value,
        detail,
    }
}

/// The Series group of item `ii` of `set`, series `si` of `n`.
pub(super) fn series_group(
    set: &Series<PathBuf>,
    ii: usize,
    it: &series::SeriesItem<PathBuf>,
    si: usize,
    n: usize,
) -> SummaryGroup {
    let loc = I18n::global().localizer();
    let mm = |x: f64| t!("dicom-series-mm", mm = loc.number(x, 2));
    let mut items = vec![item(
        "slice",
        t!("dicom-series-slice"),
        t!(
            "dicom-series-slice-value",
            index = (ii + 1) as i64,
            count = set.items.len() as i64
        ),
        (it.instance.frames > 1)
            .then(|| t!("dicom-series-frames", frames = it.instance.frames as i64)),
    )];
    if let Some(x) = it.instance.slice_location.or(it.along) {
        items.push(item("location", t!("dicom-series-location"), mm(x), None));
    }
    if let Some(s) = set.spacing {
        items.push(item(
            "spacing",
            t!("dicom-series-spacing"),
            mm(s),
            set.irregular.then(|| t!("dicom-series-irregular")),
        ));
    }
    let mut what: Vec<String> = Vec::new();
    if !set.modality.is_empty() {
        what.push(set.modality.clone());
    }
    if let Some(num) = set.number {
        what.push(format!("#{num}"));
    }
    if !set.description.is_empty() {
        what.push(set.description.clone());
    }
    items.push(item(
        "which",
        t!("dicom-series-which"),
        t!(
            "dicom-series-which-value",
            index = (si + 1) as i64,
            count = n as i64
        ),
        (!what.is_empty()).then(|| specular_i18n::dot_join(&what)),
    ));
    items.push(item(
        "order",
        t!("dicom-series-order"),
        match set.order {
            SeriesOrder::Position => t!("dicom-series-order-position"),
            SeriesOrder::InstanceNumber => t!("dicom-series-order-instance"),
            SeriesOrder::AcquisitionTime => t!("dicom-series-order-time"),
            SeriesOrder::Found => t!("dicom-series-order-name"),
        },
        None,
    ));
    if set.missing() > 0 {
        let after: Vec<String> = set
            .gaps
            .iter()
            .take(8)
            .map(|g| (g.after + 1).to_string())
            .collect();
        let mut slices = after.join(", ");
        if set.gaps.len() > 8 {
            slices.push_str(", …");
        }
        items.push(item(
            "gaps",
            t!("dicom-series-gaps"),
            t!("dicom-series-gaps-value", count = set.missing() as i64),
            Some(t!("dicom-series-gaps-where", slices = slices)),
        ));
    }
    if let Some(d) = set.tilt {
        items.push(item(
            "tilt",
            t!("dicom-series-tilt"),
            t!("dicom-series-tilt-value", degrees = loc.number(d, 1)),
            None,
        ));
    }
    if set.duplicates > 0 {
        items.push(item(
            "duplicates",
            t!("dicom-series-duplicates"),
            loc.number(set.duplicates as f64, 0),
            None,
        ));
    }
    SummaryGroup {
        key: "series",
        title: t!("dicom-series-group"),
        items,
    }
}

/// The tag browser's rows for `tree`, localised.
pub(super) fn tag_lines(tree: &TagTree, title: String) -> DicomTags {
    let loc = I18n::global().localizer();
    let rows = tree
        .rows
        .iter()
        .map(|r| {
            let item = r.tag == 0;
            let (keyword, value) = match &r.value {
                TagValue::Item(k) => (t!("dicom-tags-item", index = *k as i64), String::new()),
                v => (
                    r.keyword.to_owned(),
                    match v {
                        TagValue::Text { text, more } => {
                            if *more {
                                format!("{text}…")
                            } else {
                                text.clone()
                            }
                        }
                        TagValue::Binary { hex, len } => {
                            let bytes = t!("dicom-tags-bytes", count = *len as i64);
                            if hex.is_empty() {
                                bytes
                            } else {
                                format!("{bytes}: {hex}{}", if *len > 16 { " …" } else { "" })
                            }
                        }
                        TagValue::Items(n) => t!("dicom-tags-items", count = *n as i64),
                        TagValue::Fragments(n) => t!("dicom-tags-fragments", count = *n as i64),
                        TagValue::Empty => t!("dicom-tags-empty"),
                        TagValue::Item(_) => String::new(),
                    },
                ),
            };
            // Sequences and items count items or elements, not bytes: their value says it.
            let counted = item || matches!(r.value, TagValue::Items(_));
            TagLine {
                depth: r.depth,
                tag: if item { String::new() } else { r.tag_text() },
                keyword,
                vr: r.vr.to_owned(),
                length: if counted {
                    String::new()
                } else {
                    loc.number(r.length as f64, 0)
                },
                value,
                descendants: r.descendants,
                identifying: r.identifying,
                jump: false,
            }
        })
        .collect();
    DicomTags {
        title,
        rows,
        status: None,
        note: (tree.omitted > 0).then(|| t!("dicom-tags-omitted", count = tree.omitted as i64)),
        heads: None,
    }
}

#[cfg(test)]
#[path = "series_tests.rs"]
mod tests;

//! Merge to HDR in the app.
//!
//! - **Asking:** the gallery's "Merge to HDR…" on two or more raws merges those; on one raw
//!   it finds the bracket that raw belongs to among its neighbours (capture time, camera,
//!   exposure steps). The frames' facts are read in the background, then the sheet opens.
//! - **Sheet** ([`super::sheet`]): the photos (each can be left out) with their exposure
//!   relative to the middle one, then the options (align, reduce ghosts, keep the colour
//!   mosaic, open in Develop) and the float depth. Choices last for the run.
//! - **Job:** the frames are read, merged ([`specular_develop::merge`]) and saved as
//!   `<reference>-HDR.dng` beside them (never over a file) on a background thread, with a
//!   row and Stop in the downloads indicator. The memory needed is reserved first; too
//!   little free RAM says how much is needed.
//! - **Done:** a toast names the file (with "Show ghosts" when frames were left out
//!   somewhere: the merge opens in Develop with its ghost mask over the picture, which is
//!   kept in the cache for later, see [`super::ghosts`]); the merge opens in Develop when
//!   asked; the gallery stacks it with its sources.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

use specular_core::CancelToken;
use specular_develop::merge::{self, Motion, Shot, dng};
use specular_i18n::t;
use specular_widgets::{DownloadRow, DownloadState};

use super::sheet::{DevelopSheet, Purpose};
use crate::app::App;

/// The downloads indicator's Stop key prefix.
const STOP: &str = "develop-merge-stop:";
/// Toast action keys ("Show ghosts").
const SHOW_KEYS: u64 = (1 << 61) | (1 << 60);
/// Neighbours on each side looked at for a bracket around one photo.
pub(in crate::app) const AROUND: usize = 9;

/// Option rows after the photos: align, ghosts, mosaic, open in Develop.
const OPTIONS: usize = 4;

/// The run's choices.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Prefs {
    align: bool,
    ghosts: bool,
    mosaic: bool,
    open: bool,
    /// 0: 16-bit, 1: 24-bit, 2: 32-bit floats.
    depth: usize,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            align: true,
            ghosts: true,
            mosaic: true,
            open: true,
            depth: 1,
        }
    }
}

/// What the background read of the frames found.
enum Asked {
    Frames(Vec<Shot>),
    /// One photo with no bracket around it.
    NoBracket(String),
}

/// Why a job ended without a file.
enum Failure {
    Merge(merge::Error),
    Write(String),
}

struct Done {
    path: PathBuf,
    sources: Vec<PathBuf>,
    ghosts: f32,
    /// The ghost mask was kept (frames were left out somewhere).
    kept: bool,
}

struct Job {
    token: u64,
    name: String,
    progress: Arc<AtomicU32>,
    cancel: CancelToken,
    rx: mpsc::Receiver<Result<Done, Failure>>,
    open: bool,
}

/// Merge state of the app.
#[derive(Default)]
pub(in crate::app) struct State {
    asking: Option<mpsc::Receiver<Asked>>,
    jobs: Vec<Job>,
    next: u64,
    prefs: Prefs,
    /// The photos of the open sheet, in its row order.
    shots: Vec<Shot>,
    /// Ghost masks offered by toasts: key → the merged DNG.
    previews: Vec<(u64, PathBuf)>,
}

/// Exposure of `s` relative to `mid` in EV, as the sheet shows it ("−2 EV", "+1.3 EV").
pub(super) fn ev_text(s: &Shot, mid: Option<f64>) -> Option<String> {
    let d = s.ev()? - mid?;
    let d = (d * 3.0).round() / 3.0;
    let v = if d.abs() < 0.05 {
        "0".to_string()
    } else {
        // In the UI language's digits and decimal separator ("+1,3" in German).
        let l = specular_i18n::I18n::global().localizer();
        specular_i18n::signed_number(&l.number(d.abs(), 1), d < 0.0, true)
    };
    // A signed number stays one left-to-right run in right-to-left text.
    Some(t!("develop-merge-ev", ev = format!("\u{2066}{v}\u{2069}")))
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The facts of `paths` (unreadable files left out), in capture order.
fn read_shots(paths: &[PathBuf]) -> Vec<Shot> {
    let mut shots: Vec<Shot> = paths
        .iter()
        .filter_map(|p| {
            let data = std::fs::read(p).ok()?;
            Shot::of_raw(p.clone(), &data)
        })
        .collect();
    shots.sort_by(|a, b| {
        a.taken
            .partial_cmp(&b.taken)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.path.cmp(&b.path))
    });
    shots
}

/// The sheet for `shots` with `prefs`.
fn sheet(shots: &[Shot], prefs: &Prefs) -> DevelopSheet {
    let reference = merge::default_reference(shots);
    let mid = shots.get(reference).and_then(Shot::ev);
    let mut rows: Vec<(String, bool)> = shots
        .iter()
        .map(|s| {
            let mut parts = vec![name_of(&s.path)];
            parts.extend(ev_text(s, mid));
            (specular_i18n::dot_join(&parts), true)
        })
        .collect();
    let mut ends = Vec::with_capacity(rows.len() + OPTIONS);
    ends.resize(
        rows.len(),
        (t!("develop-sheet-included"), t!("develop-sheet-left-out")),
    );

    for (label, on) in [
        (t!("develop-merge-align"), prefs.align),
        (t!("develop-merge-ghosts"), prefs.ghosts),
        (t!("develop-merge-mosaic"), prefs.mosaic),
        (t!("develop-merge-open"), prefs.open),
    ] {
        rows.push((label, on));
        ends.push((t!("develop-merge-on"), t!("develop-merge-off")));
    }
    let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
    let name = shots
        .get(reference)
        .map(|s| dng::file_name(&s.path))
        .unwrap_or_default();
    DevelopSheet {
        purpose: Purpose::MergeHdr(paths),
        title: t!("develop-merge-title"),
        field: None,
        kinds: Vec::new(),
        mode: Some(prefs.depth.min(2)),
        ok: t!("develop-merge-ok"),
        top: 0,
        note: Some(t!("develop-merge-note", name = name)),
        rows,
        ends,
        choice_text: Some((
            t!("develop-merge-depth"),
            vec![
                t!("develop-merge-depth-16"),
                t!("develop-merge-depth-24"),
                t!("develop-merge-depth-32"),
            ],
        )),
    }
}

/// The sheet for `shots` with the default choices (screenshot tests).
#[cfg(test)]
pub(super) fn sheet_for_shots(shots: &[Shot]) -> DevelopSheet {
    sheet(shots, &Prefs::default())
}

/// The sheet's choices: the photos kept and the prefs.
fn choices(s: &DevelopSheet, paths: &[PathBuf]) -> (Vec<PathBuf>, Prefs) {
    let n = paths.len();
    let kept = paths
        .iter()
        .zip(&s.rows)
        .filter(|(_, r)| r.1)
        .map(|(p, _)| p.clone())
        .collect();
    let opt = |i: usize| s.rows.get(n + i).is_some_and(|r| r.1);
    (
        kept,
        Prefs {
            align: opt(0),
            ghosts: opt(1),
            mosaic: opt(2),
            open: opt(3),
            depth: s.mode.unwrap_or(1).min(2),
        },
    )
}

impl App {
    /// "Merge to HDR…" on `targets` (raws); `around` are the photos near a single target in
    /// the listing's order, where its bracket is looked for.
    pub(in crate::app) fn develop_merge_ask(
        &mut self,
        targets: Vec<PathBuf>,
        around: Vec<PathBuf>,
    ) {
        let raws: Vec<PathBuf> = targets
            .into_iter()
            .filter(|p| super::controls::is_raw(p))
            .collect();
        if raws.is_empty() {
            self.develop_say(t!("develop-merge-not-raw"));
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.develop.merge.asking = Some(rx);
        let wake = self.loader.waker();
        self.loader.background("develop-mread", move |_| {
            let asked = if raws.len() >= 2 {
                Asked::Frames(read_shots(&raws))
            } else {
                let one = raws[0].clone();
                let near: Vec<PathBuf> = around
                    .into_iter()
                    .filter(|p| super::controls::is_raw(p))
                    .collect();
                let shots = read_shots(&near);
                let sets = merge::bracket_sets(&shots);
                match shots
                    .iter()
                    .position(|s| s.path == one)
                    .and_then(|i| merge::detect::set_of(&sets, i))
                {
                    Some(set) => {
                        Asked::Frames(set.frames.iter().map(|&i| shots[i].clone()).collect())
                    }
                    None => Asked::NoBracket(name_of(&one)),
                }
            };
            let _ = tx.send(asked);
            wake();
        });
        self.net_refresh();
    }

    /// The sheet's OK: the job starts.
    pub(super) fn develop_merge_ok(&mut self, s: &DevelopSheet, paths: Vec<PathBuf>) {
        let (kept, prefs) = choices(s, &paths);
        self.develop.merge.prefs = prefs;
        if kept.len() < 2 {
            self.develop_say(t!("develop-merge-too-few"));
            return;
        }
        let shots: Vec<Shot> = self
            .develop
            .merge
            .shots
            .iter()
            .filter(|s| kept.contains(&s.path))
            .cloned()
            .collect();
        let reference = merge::default_reference(&shots);
        let name = shots
            .get(reference)
            .map(|s| dng::file_name(&s.path))
            .unwrap_or_default();
        let o = merge::Options {
            motion: if prefs.align {
                Motion::Euclidean
            } else {
                Motion::None
            },
            ghosts: prefs.ghosts.then_some(4.0),
            reference: Some(reference),
        };
        let d = dng::DngOptions {
            bits: [dng::Bits::F16, dng::Bits::F24, dng::Bits::F32][prefs.depth],
            cfa: prefs.mosaic,
        };
        let progress = Arc::new(AtomicU32::new(0));
        let cancel = CancelToken::new();
        let (tx, rx) = mpsc::channel();
        let m = &mut self.develop.merge;
        m.next += 1;
        let token = m.next;
        m.jobs.push(Job {
            token,
            name: name.clone(),
            progress: progress.clone(),
            cancel: cancel.clone(),
            rx,
            open: prefs.open,
        });
        let wake = self.loader.waker();
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        self.loader.background("develop-merge", move |_| {
            let set = |f: f32| progress.store(f.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
            let r = (|| {
                let n = paths.len();
                let mut frames = Vec::with_capacity(n);
                for (i, p) in paths.iter().enumerate() {
                    if cancel.is_cancelled() {
                        return Err(Failure::Merge(merge::Error::Cancelled));
                    }
                    let mut f = merge::Frame::read(p).map_err(Failure::Merge)?;
                    if let Some(s) = shots.iter().find(|s| s.path == *p) {
                        f.shot = s.clone();
                    }
                    frames.push(f);
                    set(0.3 * (i + 1) as f32 / n as f32);
                }
                let merged = merge::merge(frames, &o, &|f| set(0.3 + 0.6 * f), &cancel)
                    .map_err(Failure::Merge)?;
                let ref_bytes = std::fs::read(&paths[merged.reference]).unwrap_or_default();
                let path = dng::save(&merged, &ref_bytes, &d)
                    .map_err(|e| Failure::Write(e.to_string()))?;
                set(1.0);
                let ghosts = merged.ghosts.coverage();
                // The ghost mask, kept beside the merge in the cache (keyed by the DNG).
                let kept = merged.ghosts.mask.iter().any(|&m| m >= 128)
                    && super::ghosts::save(&path, &merged.ghosts, (merged.width, merged.height))
                        .is_ok();
                Ok(Done {
                    path,
                    sources: paths.clone(),
                    ghosts,
                    kept,
                })
            })();
            let _ = tx.send(r);
            wake();
        });
        self.develop_say(t!("develop-merge-started", name = name));
        self.net_refresh();
    }

    /// Frames read, jobs finished.
    pub(super) fn develop_merge_tick(&mut self) {
        if let Some(rx) = &self.develop.merge.asking
            && let Ok(asked) = rx.try_recv()
        {
            self.develop.merge.asking = None;
            match asked {
                Asked::Frames(shots) if shots.len() >= 2 => {
                    let s = sheet(&shots, &self.develop.merge.prefs);
                    self.develop.merge.shots = shots;
                    self.fileops_open_develop(s);
                }
                Asked::Frames(_) => self.develop_say(t!("develop-merge-too-few")),
                Asked::NoBracket(name) => {
                    self.develop_say(t!("develop-merge-no-bracket", name = name));
                }
            }
            self.redraw = true;
        }
        let mut finished = Vec::new();
        self.develop.merge.jobs.retain(|j| match j.rx.try_recv() {
            Ok(r) => {
                finished.push((j.name.clone(), j.open, r));
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        for (name, open, r) in finished {
            self.develop_merge_done(name, open, r);
        }
    }

    fn develop_merge_done(&mut self, name: String, open: bool, r: Result<Done, Failure>) {
        let now = self.now();
        self.net_refresh();
        self.redraw = true;
        let done = match r {
            Ok(d) => d,
            Err(Failure::Merge(merge::Error::Cancelled)) => {
                self.chrome
                    .toast(t!("develop-merge-stopped", name = name), None, now);
                return;
            }
            Err(Failure::Merge(e)) => {
                let why = merge_reason(&e);
                self.chrome
                    .warning(t!("develop-merge-failed", reason = why), now);
                return;
            }
            Err(Failure::Write(e)) => {
                self.chrome
                    .warning(t!("develop-merge-failed", reason = e), now);
                return;
            }
        };
        self.gallery_merge_known(&done.path, &done.sources);
        let file = name_of(&done.path);
        match done.kept.then(|| done.path.clone()) {
            Some(p) => {
                let m = &mut self.develop.merge;
                m.next += 1;
                let key = SHOW_KEYS | m.next;
                m.previews.push((key, p));
                let pct = (done.ghosts * 100.0).max(1.0).round() as i64;
                self.chrome.toast_action(
                    t!(
                        "develop-merge-done-ghosts",
                        name = file,
                        percent = super::controls::percent_text(pct as f64, 0)
                    ),
                    t!("develop-merge-show-ghosts"),
                    key,
                    now,
                );
            }
            None => {
                self.chrome
                    .toast(t!("develop-merge-done", name = file), None, now);
            }
        }
        if open {
            self.open_target(done.path.clone());
            self.develop_open_at(&done.path, 0);
        }
    }

    /// "Show ghosts" from a merge's toast: the merge in Develop with its ghost mask shown;
    /// false when `key` is not one of ours.
    pub(in crate::app) fn develop_merge_toast_action(&mut self, key: u64) -> bool {
        let Some(i) = self.develop.merge.previews.iter().position(|p| p.0 == key) else {
            return false;
        };
        let (_, p) = self.develop.merge.previews.remove(i);
        let open = self.develop.session.as_ref().is_some_and(|s| s.path == p);
        if !open {
            if self.develop.active() {
                self.develop_leave();
            }
            self.open_target(p.clone());
            self.develop_open_at(&p, 0);
        }
        if self.develop.aids.ghosts.is_none() {
            self.develop.aids.set_ghosts(super::ghosts::load(&p));
        }
        self.develop_ghosts_toggle(Some(true));
        true
    }

    /// The downloads indicator's rows for running merges.
    pub(in crate::app) fn develop_merge_rows(&self) -> Vec<DownloadRow> {
        self.develop
            .merge
            .jobs
            .iter()
            .map(|j| {
                let f = f32::from_bits(j.progress.load(Ordering::Relaxed));
                let stopping = j.cancel.is_cancelled();
                DownloadRow {
                    title: t!("develop-merge-row", name = j.name.clone()),
                    detail: t!(
                        "develop-merge-row-detail",
                        percent = super::controls::percent_text(f64::from((f * 100.0).round()), 0)
                    ),
                    status: if stopping {
                        t!("develop-batch-stopping")
                    } else {
                        t!("develop-merge-merging")
                    },
                    fraction: Some(f),
                    state: DownloadState::Running,
                    stop: (!stopping).then(|| (t!("net-stop"), format!("{STOP}{}", j.token))),
                }
            })
            .collect()
    }

    /// The downloads panel's Stop for a merge; true when the key was a merge's.
    pub(in crate::app) fn develop_merge_stop(&mut self, key: &str) -> bool {
        let Some(token) = key.strip_prefix(STOP).and_then(|t| t.parse::<u64>().ok()) else {
            return false;
        };
        if let Some(j) = self.develop.merge.jobs.iter().find(|j| j.token == token) {
            j.cancel.cancel();
        }
        true
    }
}

/// Where merge files (ghost masks) are kept.
pub(super) fn cache_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("specular-merge-cache-{}", std::process::id()));
    }
    specular_settings::Locations::detect()
        .cache_dir
        .join("merge")
}

/// A merge error in the UI's words.
pub(super) fn merge_reason(e: &merge::Error) -> String {
    match e {
        merge::Error::TooFew => t!("develop-merge-too-few"),
        merge::Error::TooMany => t!("develop-merge-too-many", count = merge::MAX_FRAMES as i64),
        merge::Error::NotOneCamera => t!("develop-merge-not-one-camera"),
        merge::Error::NotRaw(f) => t!("develop-merge-not-raw-file", file = name_of(Path::new(f))),
        merge::Error::Read(f, why) => {
            t!(
                "develop-merge-unreadable",
                file = name_of(Path::new(f)),
                reason = why.clone()
            )
        }
        merge::Error::NoExposures => t!("develop-merge-no-exposures"),
        merge::Error::NotAligned(i) => t!("develop-merge-not-aligned", number = (*i + 1) as i64),
        merge::Error::NoOverlap => t!("develop-pano-no-overlap"),
        merge::Error::Cap(n) => t!("develop-merge-cap", count = *n as i64),
        merge::Error::NeedsMemory(n) => crate::error_text::needs_memory_text(*n),
        merge::Error::Cancelled => t!("develop-merge-cancelled"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(name: &str, t: f64) -> Shot {
        Shot {
            path: PathBuf::from(format!("/p/{name}")),
            exposure_time: Some(t),
            f_number: Some(8.0),
            iso: Some(100.0),
            ..Shot::default()
        }
    }

    #[test]
    fn sheet_lists_photos_then_options_and_reads_back() {
        let shots = vec![
            shot("a.CR2", 1.0 / 500.0),
            shot("b.CR2", 1.0 / 125.0),
            shot("c.CR2", 1.0 / 30.0),
        ];
        let mut s = sheet(&shots, &Prefs::default());
        assert_eq!(s.rows.len(), 3 + OPTIONS);
        assert!(s.rows[0].0.contains("a.CR2"));
        assert!(s.rows[0].0.contains("\u{2212}2"), "{}", s.rows[0].0);
        assert!(s.rows[1].0.contains('0'));
        assert!(s.rows[2].0.contains("+2"), "{}", s.rows[2].0);
        assert_eq!(
            s.note.as_deref().map(|n| n.contains("b-HDR.dng")),
            Some(true)
        );
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        s.toggle(0);
        s.toggle(3 + 1);
        s.mode = Some(2);
        let (kept, prefs) = choices(&s, &paths);
        assert_eq!(kept, paths[1..].to_vec());
        assert!(prefs.align && !prefs.ghosts && prefs.mosaic && prefs.open);
        assert_eq!(prefs.depth, 2);
        let v = s.view(crate::app::fileops::Part::Row(0));
        assert_eq!(v.choice.as_ref().map(|c| c.options.len()), Some(3));
    }
}

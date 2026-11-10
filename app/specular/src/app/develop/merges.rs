//! What the panorama ([`super::pano`]), focus stack ([`super::stack`]) and non-raw HDR
//! ([`super::fusion`]) flows share with each other, as Merge to HDR ([`super::merge`]) does
//! for raw brackets:
//!
//! - **Asking:** the gallery's item on two or more photos takes those; on one it finds the
//!   set that photo belongs to among its neighbours (capture time, camera, exposure or focus
//!   steps). The photos' facts are read in the background, then the flow's sheet opens.
//! - **Job:** a background thread with a row and Stop in the downloads indicator; the
//!   memory needed is reserved by the core first, and too little free RAM says how much is
//!   needed.
//! - **Done:** a toast names the file (and the photos left out of a panorama); the result
//!   opens in Develop when asked and stacks with its sources in the gallery.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

use specular_core::CancelToken;
use specular_develop::merge::{self, Shot};
use specular_i18n::t;
use specular_widgets::{DownloadRow, DownloadState};

use crate::app::App;

/// The downloads indicator's Stop key prefix.
const STOP: &str = "develop-merges-stop:";
/// Bytes read from a photo's start for its facts before falling back to the whole file.
const HEAD: u64 = 4 << 20;

/// Which flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Kind {
    Pano,
    Stack,
    /// HDR from JPEG, HEIC or TIFF brackets (exposure fusion).
    Fusion,
}

impl Kind {
    /// Neighbours on each side looked at for a set around one photo.
    pub(in crate::app) fn around(self) -> usize {
        match self {
            Kind::Pano => merge::detect::MAX_PANO_FRAMES,
            Kind::Stack => merge::detect::MAX_FOCUS_FRAMES,
            Kind::Fusion => super::merge::AROUND,
        }
    }

    fn row(self, name: String) -> String {
        match self {
            Kind::Pano => t!("develop-pano-row", name = name),
            Kind::Stack => t!("develop-stack-row", name = name),
            Kind::Fusion => t!("develop-merge-row", name = name),
        }
    }

    fn status(self) -> String {
        match self {
            Kind::Pano => t!("develop-pano-stitching"),
            Kind::Stack => t!("develop-stack-stacking"),
            Kind::Fusion => t!("develop-merge-merging"),
        }
    }

    fn started(self, name: String) -> String {
        match self {
            Kind::Pano => t!("develop-pano-started", name = name),
            Kind::Stack => t!("develop-stack-started", name = name),
            Kind::Fusion => t!("develop-merge-started", name = name),
        }
    }

    fn stopped(self, name: String) -> String {
        match self {
            Kind::Pano => t!("develop-pano-stopped", name = name),
            Kind::Stack => t!("develop-stack-stopped", name = name),
            Kind::Fusion => t!("develop-merge-stopped", name = name),
        }
    }

    fn failed(self, reason: String) -> String {
        match self {
            Kind::Pano => t!("develop-pano-failed", reason = reason),
            Kind::Stack => t!("develop-stack-failed", reason = reason),
            Kind::Fusion => t!("develop-merge-failed", reason = reason),
        }
    }

    fn done(self, name: String) -> String {
        match self {
            Kind::Pano => t!("develop-pano-done", name = name),
            Kind::Stack => t!("develop-stack-done", name = name),
            Kind::Fusion => t!("develop-merge-done", name = name),
        }
    }

    fn no_set(self, name: String) -> String {
        match self {
            Kind::Pano => t!("develop-pano-no-set", name = name),
            Kind::Stack => t!("develop-stack-no-set", name = name),
            Kind::Fusion => t!("develop-merge-no-bracket", name = name),
        }
    }
}

/// What the background read of the photos found.
pub(super) enum Asked {
    Frames(Vec<Shot>),
    /// One photo with no set around it.
    NoSet(String),
}

/// Why a job ended without a file.
pub(super) enum Failure {
    Merge(merge::Error),
    Write(String),
}

/// A finished job.
pub(super) struct Done {
    /// The result (the one opened in Develop).
    pub path: PathBuf,
    pub sources: Vec<PathBuf>,
    /// Photos the result leaves out (a panorama's frames that overlap nothing).
    pub left_out: Vec<PathBuf>,
}

struct Job {
    token: u64,
    kind: Kind,
    name: String,
    progress: Arc<AtomicU32>,
    cancel: CancelToken,
    rx: mpsc::Receiver<Result<Done, Failure>>,
    open: bool,
}

/// The flows' state.
#[derive(Default)]
pub(in crate::app) struct State {
    asking: Option<(Kind, mpsc::Receiver<Asked>)>,
    jobs: Vec<Job>,
    next: u64,
    pub(super) pano: super::pano::Prefs,
    pub(super) stack: super::stack::Prefs,
    pub(super) fusion: super::fusion::Prefs,
    /// The photos of the open sheet, in its row order.
    pub(super) shots: Vec<Shot>,
}

pub(super) fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The first bytes of a file (its metadata, for the focal length); empty when unreadable.
pub(super) fn head_of(p: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(p) {
        let _ = f.take(HEAD).read_to_end(&mut head);
    }
    head
}

/// The facts of one photo (raw or not) from its start, else from the whole file.
pub(super) fn shot_of(p: &Path) -> Option<Shot> {
    use std::io::Read;
    let of = |data: &[u8]| {
        if super::controls::is_raw(p) {
            Shot::of_raw(p.to_path_buf(), data)
        } else {
            merge::rgb::shot_of(p, data)
        }
    };
    let mut head = Vec::new();
    std::fs::File::open(p)
        .ok()?
        .take(HEAD)
        .read_to_end(&mut head)
        .ok()?;
    if let Some(s) = of(&head).filter(|s| s.taken.is_some()) {
        return Some(s);
    }
    if head.len() as u64 >= HEAD {
        let all = std::fs::read(p).ok()?;
        return of(&all);
    }
    of(&head)
}

/// The facts of `paths` (unreadable files left out), in capture order.
pub(super) fn read_shots(paths: &[PathBuf]) -> Vec<Shot> {
    use rayon::prelude::*;
    let mut shots: Vec<Shot> = paths.par_iter().filter_map(|p| shot_of(p)).collect();
    shots.sort_by(|a, b| {
        a.taken
            .partial_cmp(&b.taken)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.path.cmp(&b.path))
    });
    shots
}

/// The sets of `kind` among `shots`.
fn sets_of(kind: Kind, shots: &[Shot]) -> Vec<merge::BracketSet> {
    match kind {
        Kind::Pano => merge::detect::pano_sets(shots),
        Kind::Stack => merge::detect::focus_sets(shots),
        Kind::Fusion => merge::bracket_sets(shots),
    }
}

impl App {
    /// A flow's gallery item on `targets`; `around` are the photos near a single target in
    /// the listing's order, where its set is looked for. `keep` says which photos the flow
    /// takes (raws only, or not raws).
    pub(super) fn develop_merges_ask(
        &mut self,
        kind: Kind,
        targets: Vec<PathBuf>,
        around: Vec<PathBuf>,
        keep: fn(&Path) -> bool,
    ) {
        let targets: Vec<PathBuf> = targets.into_iter().filter(|p| keep(p)).collect();
        if targets.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.develop.merges.asking = Some((kind, rx));
        let wake = self.loader.waker();
        self.loader.background("develop-mread", move |_| {
            let asked = if targets.len() >= 2 {
                Asked::Frames(read_shots(&targets))
            } else {
                let one = targets[0].clone();
                let near: Vec<PathBuf> = around.into_iter().filter(|p| keep(p)).collect();
                let shots = read_shots(&near);
                let sets = sets_of(kind, &shots);
                match shots
                    .iter()
                    .position(|s| s.path == one)
                    .and_then(|i| merge::detect::set_of(&sets, i))
                {
                    Some(set) => {
                        Asked::Frames(set.frames.iter().map(|&i| shots[i].clone()).collect())
                    }
                    None => Asked::NoSet(name_of(&one)),
                }
            };
            let _ = tx.send(asked);
            wake();
        });
        self.net_refresh();
    }

    /// Starts a job named `name` (the file it makes) that runs `work` in the background.
    pub(super) fn develop_merges_start(
        &mut self,
        kind: Kind,
        name: String,
        open: bool,
        work: impl FnOnce(&(dyn Fn(f32) + Sync), &CancelToken) -> Result<Done, Failure> + Send + 'static,
    ) {
        let progress = Arc::new(AtomicU32::new(0));
        let cancel = CancelToken::new();
        let (tx, rx) = mpsc::channel();
        let m = &mut self.develop.merges;
        m.next += 1;
        let token = m.next;
        m.jobs.push(Job {
            token,
            kind,
            name: name.clone(),
            progress: progress.clone(),
            cancel: cancel.clone(),
            rx,
            open,
        });
        let wake = self.loader.waker();
        self.loader.background("develop-merges", move |_| {
            let set = |f: f32| progress.store(f.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
            let r = work(&set, &cancel);
            set(1.0);
            let _ = tx.send(r);
            wake();
        });
        self.develop_say(kind.started(name));
        self.net_refresh();
    }

    /// Photos read, jobs finished.
    pub(super) fn develop_merges_tick(&mut self) {
        if let Some((kind, rx)) = &self.develop.merges.asking
            && let Ok(asked) = rx.try_recv()
        {
            let kind = *kind;
            self.develop.merges.asking = None;
            match asked {
                Asked::Frames(shots) if shots.len() >= 2 => {
                    let s = match kind {
                        Kind::Pano => super::pano::sheet(&shots, &self.develop.merges.pano),
                        Kind::Stack => super::stack::sheet(&shots, &self.develop.merges.stack),
                        Kind::Fusion => super::fusion::sheet(&shots, &self.develop.merges.fusion),
                    };
                    self.develop.merges.shots = shots;
                    self.fileops_open_develop(s);
                }
                Asked::Frames(_) => self.develop_say(t!("develop-merge-too-few")),
                Asked::NoSet(name) => self.develop_say(kind.no_set(name)),
            }
            self.redraw = true;
        }
        let mut finished = Vec::new();
        self.develop.merges.jobs.retain(|j| match j.rx.try_recv() {
            Ok(r) => {
                finished.push((j.kind, j.name.clone(), j.open, r));
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        for (kind, name, open, r) in finished {
            self.develop_merges_done(kind, name, open, r);
        }
    }

    fn develop_merges_done(
        &mut self,
        kind: Kind,
        name: String,
        open: bool,
        r: Result<Done, Failure>,
    ) {
        let now = self.now();
        self.net_refresh();
        self.redraw = true;
        let done = match r {
            Ok(d) => d,
            Err(Failure::Merge(merge::Error::Cancelled)) => {
                self.chrome.toast(kind.stopped(name), None, now);
                return;
            }
            Err(Failure::Merge(e)) => {
                let why = super::merge::merge_reason(&e);
                self.chrome.warning(kind.failed(why), now);
                return;
            }
            Err(Failure::Write(e)) => {
                self.chrome.warning(kind.failed(e), now);
                return;
            }
        };
        self.gallery_merge_known(&done.path, &done.sources);
        let file = name_of(&done.path);
        let text = if done.left_out.is_empty() {
            kind.done(file)
        } else {
            let names: Vec<String> = done.left_out.iter().map(|p| name_of(p)).collect();
            t!(
                "develop-pano-done-left-out",
                name = file,
                files = names.join(", ")
            )
        };
        self.chrome.toast(text, None, now);
        if open {
            self.open_target(done.path.clone());
            self.develop_open_at(&done.path, 0);
        }
    }

    /// The downloads indicator's rows for running jobs.
    pub(in crate::app) fn develop_merges_rows(&self) -> Vec<DownloadRow> {
        self.develop
            .merges
            .jobs
            .iter()
            .map(|j| {
                let f = f32::from_bits(j.progress.load(Ordering::Relaxed));
                let stopping = j.cancel.is_cancelled();
                DownloadRow {
                    title: j.kind.row(j.name.clone()),
                    detail: t!(
                        "develop-merge-row-detail",
                        percent = super::controls::percent_text(f64::from((f * 100.0).round()), 0)
                    ),
                    status: if stopping {
                        t!("develop-batch-stopping")
                    } else {
                        j.kind.status()
                    },
                    fraction: Some(f),
                    state: DownloadState::Running,
                    stop: (!stopping).then(|| (t!("net-stop"), format!("{STOP}{}", j.token))),
                }
            })
            .collect()
    }

    /// The downloads panel's Stop for a job; true when the key was one of ours.
    pub(in crate::app) fn develop_merges_stop(&mut self, key: &str) -> bool {
        let Some(token) = key.strip_prefix(STOP).and_then(|t| t.parse::<u64>().ok()) else {
            return false;
        };
        if let Some(j) = self.develop.merges.jobs.iter().find(|j| j.token == token) {
            j.cancel.cancel();
        }
        true
    }
}

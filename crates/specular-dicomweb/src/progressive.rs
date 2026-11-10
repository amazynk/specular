//! Progressive series loading: the series' metadata first (enough to order the slices and
//! show the stack), then the middle slice, then the rest nearest the slice being viewed
//! first, a bounded number of requests at a time. Instances land in the disk cache as Part 10
//! files, so the viewer opens them like local files, and a series seen before opens offline.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use parking_lot::Mutex;
use specular_dicom::series::Series;
use specular_remote::{CachePin, CancelToken, DiskCache};

use crate::json::parse_array;
use crate::{DicomWeb, Error, uid};

/// How a series is loaded.
#[derive(Clone, Copy, Debug)]
pub struct LoadOptions {
    /// Requests in flight at once.
    pub parallel: usize,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions { parallel: 6 }
    }
}

/// What a load reports, in order: one `Metadata` (or `Error`), then `Instance`/`Failed` for
/// the slices as they arrive (any order) with `Progress`, then `Done`.
#[derive(Debug)]
pub enum LoadEvent {
    /// The series, assembled from metadata: items in viewing order, keyed by the path each
    /// instance will have in the cache. Slice indices below count over `series[0].items`,
    /// then `series[1].items`, and so on (one series UID normally assembles into one).
    Metadata {
        series: Vec<Series<PathBuf>>,
        /// The metadata came from the cache: the server did not answer.
        offline: bool,
    },
    /// Slice `index` is in the cache at `path`.
    Instance {
        index: usize,
        path: PathBuf,
    },
    /// Slice `index` could not be fetched.
    Failed {
        index: usize,
        error: String,
    },
    Progress {
        done: usize,
        total: usize,
    },
    /// Nothing more will come. `offline`: stopped because the server stopped answering;
    /// `auth`: because it refused the credentials.
    Done {
        failed: usize,
        offline: bool,
        auth: bool,
        cancelled: bool,
    },
    /// The metadata could not be fetched (and was not cached).
    Error(Error),
}

pub type Sink = Arc<dyn Fn(LoadEvent) + Send + Sync>;

/// Consecutive "server unreachable" failures (over all workers) that end a load.
const GIVE_UP: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Pending,
    Running,
    Done,
    Failed,
}

/// A series being loaded. Dropping it cancels the remaining requests (not the files already
/// cached); it keeps the series' cache entries from eviction while alive.
pub struct SeriesLoad {
    focus: Arc<AtomicUsize>,
    cancel: CancelToken,
    thread: Option<JoinHandle<()>>,
}

impl SeriesLoad {
    /// Starts loading on background threads. `prefix`: the server's cache folder
    /// ([`crate::Server::cache_prefix`]).
    pub fn start(
        web: DicomWeb,
        cache: Arc<DiskCache>,
        prefix: &str,
        study: &str,
        series: &str,
        options: LoadOptions,
        sink: Sink,
    ) -> SeriesLoad {
        let focus = Arc::new(AtomicUsize::new(usize::MAX));
        let cancel = CancelToken::new();
        let job = Job {
            web,
            cache,
            dir: format!(
                "{prefix}/{}/{}",
                specular_remote::cache::sanitize(study),
                specular_remote::cache::sanitize(series)
            ),
            study: study.to_string(),
            series: series.to_string(),
            options,
            sink,
            focus: focus.clone(),
            cancel: cancel.clone(),
        };
        let sink = job.sink.clone();
        let thread = specular_thread::builder("dcmweb-load")
            .spawn(move || {
                // A panic ends the load with an error, never "loading" forever.
                if let Err(p) = specular_core::isolate::run("DICOMweb series load", || job.run()) {
                    sink(LoadEvent::Error(crate::Error::Io(std::io::Error::other(
                        p.brief(),
                    ))));
                    sink(LoadEvent::Done {
                        failed: 0,
                        offline: false,
                        auth: false,
                        cancelled: false,
                    });
                }
            })
            .ok();
        SeriesLoad {
            focus,
            cancel,
            thread,
        }
    }

    /// The slice being viewed: the next requests go to the slices nearest it.
    pub fn set_focus(&self, index: usize) {
        self.focus.store(index, Ordering::Relaxed);
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Waits for the load to end (tests, shutdown).
    pub fn join(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for SeriesLoad {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

struct Job {
    web: DicomWeb,
    cache: Arc<DiskCache>,
    dir: String,
    study: String,
    series: String,
    options: LoadOptions,
    sink: Sink,
    focus: Arc<AtomicUsize>,
    cancel: CancelToken,
}

/// Whether two slices `a`, `b` (distances from the focus) favour `a`: nearer first, ahead of
/// the focus before behind it.
fn nearer(i: usize, j: usize, focus: usize) -> bool {
    let (di, dj) = (i.abs_diff(focus), j.abs_diff(focus));
    di < dj || (di == dj && i >= focus && j < focus)
}

impl Job {
    fn run(self) {
        let _pin: CachePin = self.cache.pin(&self.dir);
        let (sets, offline) = match self.metadata() {
            Ok(v) => v,
            Err(e) => {
                (self.sink)(LoadEvent::Error(e));
                return;
            }
        };
        let mut found = Vec::new();
        for d in &sets {
            let mut inst = d.to_instance();
            if uid(&inst.sop_uid).is_err() {
                continue;
            }
            if inst.study_uid.is_empty() {
                inst.study_uid = self.study.clone();
            }
            if inst.series_uid.is_empty() {
                inst.series_uid = self.series.clone();
            }
            let rel = self.rel(&format!("{}.dcm", inst.sop_uid));
            found.push((rel, inst));
        }
        let assembled = specular_dicom::series::assemble(found);
        // (cache key, SOP UID) per slice, in viewing order.
        let slices: Vec<(String, String)> = assembled
            .iter()
            .flat_map(|s| s.items.iter())
            .map(|it| (it.key.clone(), it.instance.sop_uid.clone()))
            .collect();
        let series: Vec<Series<PathBuf>> = assembled
            .into_iter()
            .map(|s| map_keys(s, |k| self.cache.path(&k)))
            .collect();
        let total = slices.len();
        (self.sink)(LoadEvent::Metadata { series, offline });

        let run = Run {
            slices: &slices,
            state: Mutex::new(vec![Slot::Pending; total]),
            done: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
            stop_offline: AtomicBool::new(offline),
            stop_auth: AtomicBool::new(false),
            offline_run: AtomicUsize::new(0),
        };
        // Already cached: shown at once.
        for (i, (rel, _)) in slices.iter().enumerate() {
            if let Some(p) = self.cache.get(rel) {
                run.state.lock()[i] = Slot::Done;
                run.done.fetch_add(1, Ordering::Relaxed);
                (self.sink)(LoadEvent::Instance { index: i, path: p });
            }
        }
        if self.focus.load(Ordering::Relaxed) == usize::MAX {
            self.focus.store(total / 2, Ordering::Relaxed);
        }
        let workers = self.options.parallel.clamp(1, 32).min(total.max(1));
        if !offline {
            // The first slice alone, so it does not share the link with the others.
            let focus = self
                .focus
                .load(Ordering::Relaxed)
                .min(total.saturating_sub(1));
            if run.state.lock().get(focus) == Some(&Slot::Pending) {
                self.worker(&run, true);
            }
            std::thread::scope(|s| {
                for wi in 0..workers {
                    specular_thread::spawn_scoped(s, format!("dcmweb-get-{wi}"), || {
                        self.worker(&run, false)
                    });
                }
            });
        }
        let missing = run
            .state
            .lock()
            .iter()
            .filter(|s| **s != Slot::Done)
            .count();
        (self.sink)(LoadEvent::Done {
            failed: if offline {
                missing
            } else {
                run.failed.load(Ordering::Relaxed)
            },
            offline: run.stop_offline.load(Ordering::Relaxed) && missing > 0,
            auth: run.stop_auth.load(Ordering::Relaxed),
            cancelled: self.cancel.is_cancelled(),
        });
    }

    fn rel(&self, name: &str) -> String {
        format!("{}/{}", self.dir, specular_remote::cache::sanitize(name))
    }

    /// The series metadata from the server (cached for later), or from the cache when the
    /// server does not answer.
    fn metadata(&self) -> Result<(Vec<crate::JsonDataSet>, bool), Error> {
        let rel = self.rel("metadata.json");
        match self
            .web
            .series_metadata(&self.study, &self.series, &self.cancel)
        {
            Ok((sets, raw)) => {
                if let Err(e) = self.cache.insert_bytes(&rel, &raw) {
                    specular_log::warn!(target: specular_log::IO, "dicomweb: caching metadata: {e}");
                }
                Ok((sets, false))
            }
            Err(e) if e.is_offline() => match self.cache.get(&rel) {
                Some(p) => {
                    let raw = std::fs::read(p)?;
                    Ok((parse_array(&raw)?, true))
                }
                None => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    fn next(&self, state: &Mutex<Vec<Slot>>) -> Option<usize> {
        let focus = self.focus.load(Ordering::Relaxed);
        let mut st = state.lock();
        let mut best: Option<usize> = None;
        for (i, s) in st.iter().enumerate() {
            if *s == Slot::Pending && best.is_none_or(|b| nearer(i, b, focus)) {
                best = Some(i);
            }
        }
        if let Some(i) = best {
            st[i] = Slot::Running;
        }
        best
    }

    /// Fetches slices nearest the focus until none are left (`once`: one slice).
    fn worker(&self, run: &Run<'_>, once: bool) {
        let total = run.slices.len();
        while !self.cancel.is_cancelled()
            && !run.stop_offline.load(Ordering::Relaxed)
            && !run.stop_auth.load(Ordering::Relaxed)
        {
            let Some(i) = self.next(&run.state) else {
                break;
            };
            let (rel, sop) = &run.slices[i];
            let got = self
                .web
                .instance(&self.study, &self.series, sop, &self.cancel, |_| {})
                .and_then(|part| Ok(self.cache.insert_bytes(rel, &part.body)?));
            match got {
                Ok(path) => {
                    run.offline_run.store(0, Ordering::Relaxed);
                    run.state.lock()[i] = Slot::Done;
                    let d = run.done.fetch_add(1, Ordering::Relaxed) + 1;
                    (self.sink)(LoadEvent::Instance { index: i, path });
                    (self.sink)(LoadEvent::Progress { done: d, total });
                }
                Err(e) if e.is_cancelled() => {
                    run.state.lock()[i] = Slot::Pending;
                    break;
                }
                Err(e) => {
                    if e.is_auth() {
                        run.stop_auth.store(true, Ordering::Relaxed);
                    }
                    if e.is_offline()
                        && run.offline_run.fetch_add(1, Ordering::Relaxed) + 1 >= GIVE_UP
                    {
                        run.stop_offline.store(true, Ordering::Relaxed);
                    }
                    run.state.lock()[i] = Slot::Failed;
                    run.failed.fetch_add(1, Ordering::Relaxed);
                    specular_log::warn!(target: specular_log::IO, "dicomweb: slice {i}: {e}");
                    (self.sink)(LoadEvent::Failed {
                        index: i,
                        error: e.to_string(),
                    });
                }
            }
            if once {
                break;
            }
        }
    }
}

/// One load's shared progress.
struct Run<'a> {
    /// (cache key, SOP UID) per slice, in viewing order.
    slices: &'a [(String, String)],
    state: Mutex<Vec<Slot>>,
    done: AtomicUsize,
    failed: AtomicUsize,
    stop_offline: AtomicBool,
    stop_auth: AtomicBool,
    /// Consecutive "unreachable" failures over all workers.
    offline_run: AtomicUsize,
}

fn map_keys<A, B>(s: Series<A>, f: impl Fn(A) -> B) -> Series<B> {
    Series {
        study_uid: s.study_uid,
        series_uid: s.series_uid,
        modality: s.modality,
        number: s.number,
        description: s.description,
        items: s
            .items
            .into_iter()
            .map(|it| specular_dicom::series::SeriesItem {
                key: f(it.key),
                instance: it.instance,
                along: it.along,
            })
            .collect(),
        order: s.order,
        spacing: s.spacing,
        gaps: s.gaps,
        irregular: s.irregular,
        tilt: s.tilt,
        duplicates: s.duplicates,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn nearest_first_then_ahead() {
        let mut order: Vec<usize> = (0..7).collect();
        order.sort_by(|a, b| {
            if super::nearer(*a, *b, 3) {
                std::cmp::Ordering::Less
            } else if super::nearer(*b, *a, 3) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        assert_eq!(order, [3, 4, 2, 5, 1, 6, 0]);
    }
}

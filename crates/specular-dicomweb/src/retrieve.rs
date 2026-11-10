//! A series from a DIMSE server, by C-GET or C-MOVE ([`specular_dimse::retrieve`],
//! [`specular_dimse::listen`]), into the same disk-cache folder a DICOMweb series uses, then
//! opened as one: [`DimseLoad`] reports the same [`LoadEvent`]s as [`crate::SeriesLoad`].
//!
//! DIMSE has no per-slice requests, so nothing is shown until the server has sent the
//! series: meanwhile the counts it reports (remaining, completed, failed, warning) go to the
//! "Downloads" indicator and to [`LoadEvent::Progress`]. Then the instances that arrived
//! are assembled from their headers and opened. When the server cannot be reached, a series
//! retrieved before opens from the cache.

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::Mutex;
use specular_core::net::{self, Failure as NetFailure, Items, Label, What};
use specular_dicom::series::{self, Series};
use specular_dimse::listen::Listen;
use specular_dimse::retrieve::{Incoming, Outcome, Progress, Storage, Target};
use specular_dimse::{Error as DimseError, Options, Peer};
use specular_remote::{CachePin, CancelToken, DiskCache};

use crate::progressive::{LoadEvent, Sink};

/// How the instances come.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum How {
    /// C-GET: on the association we open.
    Get,
    /// C-MOVE: the server opens an association to our temporary storage SCP.
    Move(Listen),
}

/// Why a retrieval opened nothing.
#[derive(Debug)]
pub enum Failure {
    /// The association, the connection or the protocol failed.
    Dimse(DimseError),
    /// The server refused the retrieval with this final status (0xA7xx out of resources,
    /// 0xA801 unknown destination, 0xA900 identifier, 0xCxxx cannot process).
    Status(u16),
    /// The server has no instances for the series.
    NoMatches,
    /// The server tried, but nothing arrived: `failed` sub-operations. `roles_refused`: it
    /// refused to send over our association (C-GET); `moved`: by C-MOVE (it may not reach
    /// this computer).
    NothingArrived {
        failed: u32,
        roles_refused: bool,
        moved: bool,
    },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Dimse(e) => e.fmt(f),
            Failure::Status(s) => write!(f, "retrieval refused (status {s:#06x})"),
            Failure::NoMatches => f.write_str("no instances"),
            Failure::NothingArrived { failed, .. } => {
                write!(f, "nothing arrived ({failed} sub-operations failed)")
            }
        }
    }
}

/// The DIMSE server and how to retrieve from it.
#[derive(Clone, Debug)]
pub struct Source {
    pub peer: Peer,
    pub options: Options,
    pub how: How,
    /// The series as the "Downloads" indicator names it.
    pub label: String,
    /// Where activity is reported (tests); the global registry otherwise.
    pub registry: Option<net::Registry>,
}

/// Writes each instance as a Part 10 file next to its cache entry, then records it.
struct CacheStorage {
    cache: Arc<DiskCache>,
    dir: String,
    stored: Arc<Mutex<Vec<String>>>,
}

struct CacheFile {
    cache: Arc<DiskCache>,
    rel: String,
    tmp: PathBuf,
    file: io::BufWriter<std::fs::File>,
    stored: Arc<Mutex<Vec<String>>>,
}

impl Storage for CacheStorage {
    fn begin(&mut self, class: &str, instance: &str, ts: &str) -> io::Result<Box<dyn Incoming>> {
        crate::uid(instance).map_err(|_| io::Error::other("bad SOP Instance UID"))?;
        let rel = format!(
            "{}/{}",
            self.dir,
            specular_remote::cache::sanitize(&format!("{instance}.dcm"))
        );
        let path = self.cache.path(&rel);
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut tmp = path.into_os_string();
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tmp.push(format!(".{}-{n}.tmp", std::process::id()));
        let tmp = PathBuf::from(tmp);
        let mut file = io::BufWriter::new(std::fs::File::create(&tmp)?);
        let meta = specular_dicom::write::FileMeta {
            sop_class: class.to_owned(),
            sop_instance: instance.to_owned(),
            transfer_syntax: ts.to_owned(),
        };
        let head = specular_dicom::write::part10(&specular_dicom::write::Object::default(), &meta)
            .map_err(|e| io::Error::other(e.to_string()))?;
        file.write_all(&head)?;
        Ok(Box::new(CacheFile {
            cache: self.cache.clone(),
            rel,
            tmp,
            file,
            stored: self.stored.clone(),
        }))
    }
}

impl Incoming for CacheFile {
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.file.write_all(data)
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        self.file.flush()?;
        let path = self.cache.path(&self.rel);
        std::fs::rename(&self.tmp, &path)?;
        self.cache.record(&self.rel)?;
        self.stored.lock().push(self.rel.clone());
        Ok(())
    }
}

impl Drop for CacheFile {
    fn drop(&mut self) {
        // Not finished (or already renamed: then this finds nothing).
        let _ = std::fs::remove_file(&self.tmp);
    }
}

/// A series being retrieved; dropping it stops the retrieval (C-CANCEL).
pub struct DimseLoad {
    cancel: CancelToken,
    thread: Option<JoinHandle<()>>,
}

impl DimseLoad {
    /// Starts on a background thread. `prefix`: the server's cache folder.
    pub fn start(
        source: Source,
        cache: Arc<DiskCache>,
        prefix: &str,
        study: &str,
        series: &str,
        sink: Sink,
    ) -> DimseLoad {
        let cancel = CancelToken::new();
        let job = Job {
            dir: format!(
                "{prefix}/{}/{}",
                specular_remote::cache::sanitize(study),
                specular_remote::cache::sanitize(series)
            ),
            target: Target {
                study: study.to_owned(),
                series: Some(series.to_owned()),
            },
            source,
            cache,
            sink: sink.clone(),
            cancel: cancel.clone(),
        };
        let thread = specular_thread::builder("dimse-load")
            .spawn(move || {
                if let Err(p) = specular_core::isolate::run("DIMSE series load", || job.run()) {
                    sink(LoadEvent::Error(crate::Error::Io(io::Error::other(
                        p.brief(),
                    ))));
                }
            })
            .ok();
        DimseLoad { cancel, thread }
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

impl Drop for DimseLoad {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

struct Job {
    source: Source,
    cache: Arc<DiskCache>,
    dir: String,
    target: Target,
    sink: Sink,
    cancel: CancelToken,
}

/// The download indicator's word for how a retrieval failed.
fn net_failure(e: &DimseError) -> NetFailure {
    match e {
        DimseError::Cancelled => NetFailure::Cancelled,
        DimseError::Timeout => NetFailure::Timeout,
        DimseError::Connect(_) | DimseError::UnknownHost | DimseError::NetworkOff => {
            NetFailure::Offline
        }
        DimseError::Rejected { .. } => NetFailure::Auth,
        _ => NetFailure::Other,
    }
}

/// Could not reach the server at all (a series retrieved before opens from the cache).
fn unreachable(e: &DimseError) -> bool {
    matches!(
        e,
        DimseError::Connect(_) | DimseError::UnknownHost | DimseError::Timeout
    )
}

impl Job {
    fn run(self) {
        let _pin: CachePin = self.cache.pin(&self.dir);
        let s = &self.source;
        let reg = s.registry.clone().unwrap_or_else(|| net::global().clone());
        // Nothing is reported once the switch refused it (as the HTTP client does).
        let track = (!specular_core::net::off::is_off()).then(|| {
            reg.begin(Label::new(
                What::DicomSeries(s.label.clone()),
                &format!("dicom://{}", s.peer.host.trim()),
            ))
        });
        let watch = track.as_ref().map(net::Request::watch);
        let track = Mutex::new(track);
        let stored = Arc::new(Mutex::new(Vec::new()));
        let mut storage = CacheStorage {
            cache: self.cache.clone(),
            dir: self.dir.clone(),
            stored: stored.clone(),
        };
        let sink = self.sink.clone();
        let mut last_bytes = 0u64;
        let mut report = |p: &Progress| {
            let (done, total) = p.done_total();
            if let Some(t) = track.lock().as_mut() {
                t.add(p.bytes.saturating_sub(last_bytes));
                let c = p.counts;
                t.items(Items {
                    completed: u32::from(c.completed).max(p.received),
                    failed: u32::from(c.failed),
                    warning: u32::from(c.warning),
                    remaining: u32::from(c.remaining.unwrap_or(0)),
                });
            }
            last_bytes = p.bytes;
            sink(LoadEvent::Progress {
                done: done as usize,
                total: total as usize,
            });
        };
        let stop: Box<dyn Fn() -> bool + Send> =
            Box::new(move || watch.as_ref().is_some_and(net::CancelWatch::is_cancelled));
        let r = match &s.how {
            How::Get => specular_dimse::retrieve::get(
                &s.peer,
                &s.options,
                &self.target,
                &mut storage,
                &mut report,
                stop,
                &self.cancel,
            ),
            How::Move(l) => specular_dimse::listen::move_into(
                &s.peer,
                &s.options,
                &self.target,
                l,
                &mut storage,
                &mut report,
                stop,
                &self.cancel,
            ),
        };
        let track = track.into_inner();
        let end = |r: std::result::Result<(), NetFailure>| {
            if let Some(t) = track {
                t.finish(r);
            }
        };
        let o: Outcome = match r {
            Err(DimseError::Cancelled) => {
                end(Err(NetFailure::Cancelled));
                self.done(0, true);
                return;
            }
            Err(e) if unreachable(&e) && self.open_cached() => {
                end(Err(net_failure(&e)));
                return;
            }
            Err(e) => {
                end(Err(net_failure(&e)));
                (self.sink)(LoadEvent::Error(crate::Error::Retrieve(Failure::Dimse(e))));
                return;
            }
            Ok(o) => o,
        };
        let received = stored.lock().clone();
        if o.cancelled {
            end(Err(NetFailure::Cancelled));
            self.done(0, true);
            return;
        }
        if received.is_empty() {
            let f = if o.no_matches() {
                Failure::NoMatches
            } else if matches!(o.status, 0xA700..=0xAFFF | 0xC000..=0xCFFF) {
                Failure::Status(o.status)
            } else {
                Failure::NothingArrived {
                    failed: u32::from(o.progress.counts.failed) + o.progress.refused,
                    roles_refused: o.roles_refused,
                    moved: matches!(s.how, How::Move(_)),
                }
            };
            end(Err(NetFailure::Other));
            (self.sink)(LoadEvent::Error(crate::Error::Retrieve(f)));
            return;
        }
        end(Ok(()));
        let failed = u32::from(o.progress.counts.failed) + o.progress.refused;
        self.open(&received, false);
        self.done(failed as usize, false);
    }

    fn done(&self, failed: usize, cancelled: bool) {
        (self.sink)(LoadEvent::Done {
            failed,
            offline: false,
            auth: false,
            cancelled,
        });
    }

    /// The instances `rels` assembled into series and opened.
    fn open(&self, rels: &[String], offline: bool) {
        let found: Vec<(PathBuf, series::Instance)> = rels
            .iter()
            .filter_map(|rel| {
                let p = self.cache.path(rel);
                let i = series::read_header(&p).ok()??;
                Some((p, i))
            })
            .collect();
        let assembled: Vec<Series<PathBuf>> = series::assemble(found);
        let paths: Vec<PathBuf> = assembled
            .iter()
            .flat_map(|s| s.items.iter().map(|it| it.key.clone()))
            .collect();
        let total = paths.len();
        (self.sink)(LoadEvent::Metadata {
            series: assembled,
            offline,
        });
        for (index, path) in paths.into_iter().enumerate() {
            (self.sink)(LoadEvent::Instance { index, path });
        }
        (self.sink)(LoadEvent::Progress { done: total, total });
    }

    /// A series retrieved before, from the cache; false when there is none.
    fn open_cached(&self) -> bool {
        let dir = self.cache.path(&self.dir);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return false;
        };
        let mut rels: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().into_string().ok()?;
                (n.ends_with(".dcm")).then(|| format!("{}/{n}", self.dir))
            })
            .filter(|rel| self.cache.get(rel).is_some())
            .collect();
        if rels.is_empty() {
            return false;
        }
        rels.sort();
        self.open(&rels, true);
        self.done(0, false);
        true
    }
}

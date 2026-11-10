//! Tethered capture on top of a session: telling new shots from what the card already held,
//! and downloading each into a folder.
//!
//! New shots are found from ObjectAdded events. A camera that doesn't list that event, or
//! hasn't sent one yet in this session, is also asked for its object handles every
//! [`Arrivals::poll_every`], and handles not seen before are new (shots taken with the
//! camera's own button on cameras that send no events).

use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::camera::{Camera, Error, Event};
use crate::codes::{ev, rc};
use crate::data::ObjectInfo;
use crate::transport::Transport;

/// What happened on the camera.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Happening {
    /// A new object (a shot, or one file of a RAW+JPEG pair).
    Object(u32),
    /// The capture that was asked for is complete.
    CaptureDone,
    /// A property changed on the camera (its dial, or our change taking effect).
    PropChanged(u16),
    /// A card was put in, taken out, or filled.
    Storage,
    /// The card is full.
    StoreFull,
}

/// New shots of one session.
pub struct Arrivals {
    known: HashSet<u32>,
    saw_object_event: bool,
    last_poll: Instant,
    /// How often handles are listed while no ObjectAdded event has come.
    pub poll_every: Duration,
}

impl Arrivals {
    /// Notes every object the camera holds now: none of them is new.
    pub fn start<T: Transport>(cam: &mut Camera<T>, now: Instant) -> Result<Arrivals, Error> {
        let known = match cam.all_handles() {
            Ok(h) => h.into_iter().collect(),
            // No card (cameras that capture to their memory only).
            Err(Error::Response {
                code: rc::STORE_NOT_AVAILABLE | rc::INVALID_STORAGE_ID,
                ..
            }) => HashSet::new(),
            Err(e) => return Err(e),
        };
        Ok(Arrivals {
            known,
            saw_object_event: false,
            last_poll: now,
            poll_every: Duration::from_secs(2),
        })
    }

    /// Objects the session knows of.
    pub fn known(&self) -> usize {
        self.known.len()
    }

    /// Waits up to `wait` for an event, and lists handles when due.
    pub fn poll<T: Transport>(
        &mut self,
        cam: &mut Camera<T>,
        wait: Duration,
        now: Instant,
    ) -> Result<Vec<Happening>, Error> {
        let mut out = Vec::new();
        if let Some(e) = cam.poll_event(wait)? {
            self.event(&e, &mut out);
        }
        if !self.saw_object_event && now.duration_since(self.last_poll) >= self.poll_every {
            self.last_poll = now;
            match cam.all_handles() {
                Ok(mut hs) => {
                    hs.sort_unstable();
                    for h in hs {
                        if self.known.insert(h) {
                            out.push(Happening::Object(h));
                        }
                    }
                }
                Err(e) if e.is_busy() => {}
                Err(Error::Response {
                    code: rc::STORE_NOT_AVAILABLE | rc::INVALID_STORAGE_ID,
                    ..
                }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Turns an event into what happened.
    pub fn event(&mut self, e: &Event, out: &mut Vec<Happening>) {
        match e.code {
            ev::OBJECT_ADDED | ev::REQUEST_OBJECT_TRANSFER => {
                if let Some(h) = e.param(0) {
                    if e.code == ev::OBJECT_ADDED {
                        self.saw_object_event = true;
                    }
                    if self.known.insert(h) {
                        out.push(Happening::Object(h));
                    }
                }
            }
            ev::OBJECT_REMOVED => {
                if let Some(h) = e.param(0) {
                    self.known.remove(&h);
                }
            }
            ev::CAPTURE_COMPLETE => out.push(Happening::CaptureDone),
            ev::DEVICE_PROP_CHANGED => {
                if let Some(c) = e.param(0) {
                    out.push(Happening::PropChanged(c as u16));
                }
            }
            ev::STORE_ADDED | ev::STORE_REMOVED | ev::STORAGE_INFO_CHANGED => {
                out.push(Happening::Storage)
            }
            ev::STORE_FULL => out.push(Happening::StoreFull),
            _ => {}
        }
    }
}

/// A finished download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Downloaded {
    pub path: PathBuf,
    pub bytes: u64,
    pub info: ObjectInfo,
}

/// Why a download failed.
#[derive(Debug)]
pub enum DownloadError {
    Camera(Error),
    /// The file couldn't be written (the folder, its space, its permissions).
    Disk(PathBuf, std::io::Error),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Camera(e) => write!(f, "{e}"),
            DownloadError::Disk(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for DownloadError {}

/// `name` in `dir`, with ` (2)`, ` (3)` … before the extension when taken.
pub fn free_name(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 2..10_000 {
        let p = dir.join(format!("{stem} ({n}){ext}"));
        if !p.exists() {
            return p;
        }
    }
    p
}

/// Downloads object `handle` into `dir` under the name `name(info)` gives (a free variant
/// of it when taken). The data goes to a hidden `.name.part` first and is renamed when
/// complete, so folder watchers see whole files only. `None` for a folder object.
pub fn download<T: Transport>(
    cam: &mut Camera<T>,
    handle: u32,
    dir: &Path,
    name: &mut dyn FnMut(&ObjectInfo) -> String,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Option<Downloaded>, DownloadError> {
    let info = cam.object_info(handle).map_err(DownloadError::Camera)?;
    if info.is_folder() {
        return Ok(None);
    }
    let mut wanted = name(&info);
    if wanted.is_empty() || wanted.starts_with('.') {
        wanted = format!("capture-{handle:08x}");
    }
    let part = dir.join(format!(".{wanted}.part"));
    let file = File::create(&part).map_err(|e| DownloadError::Disk(part.clone(), e))?;
    let mut w = BufWriter::with_capacity(1 << 20, file);
    let r = cam.get_object(handle, &mut w, progress);
    let flushed = std::io::Write::flush(&mut w).and_then(|_| {
        w.into_inner()
            .map_err(|e| e.into_error())
            .and_then(|f| f.sync_all())
    });
    let bytes = match (r, flushed) {
        (Ok(n), Ok(())) => n,
        (Err(Error::Write(e)), _) | (Ok(_), Err(e)) => {
            let _ = std::fs::remove_file(&part);
            return Err(DownloadError::Disk(part, e));
        }
        (Err(e), _) => {
            let _ = std::fs::remove_file(&part);
            return Err(DownloadError::Camera(e));
        }
    };
    let path = free_name(dir, &wanted);
    std::fs::rename(&part, &path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        DownloadError::Disk(path.clone(), e)
    })?;
    Ok(Some(Downloaded { path, bytes, info }))
}

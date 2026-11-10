//! The request loop: one request at a time, in order, until the viewer closes the connection.

use crate::Conn;
use crate::heif::{Failure, Heif, Meta, Rows};
use specular_sandbox::protocol::{
    ErrorKind, HostInfo, ImageReply, Input, Payload, Request, Response, Secret, VERSION,
};
use specular_sandbox::transport::{Fd, shm};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Largest input accepted (the address-space limit bounds it further).
const MAX_INPUT: u64 = 4 << 30;
/// Payloads up to this size travel inline, larger ones as shared memory.
const INLINE_MAX: usize = 64 << 10;

pub struct Host {
    /// Shared with the RAR volume broker, which asks the viewer for volumes mid-request.
    pub conn: Arc<Mutex<Conn>>,
    pub heif: Result<Heif, String>,
    pub occt: Result<crate::occt::Occt, String>,
    /// The Blackmagic RAW SDK, when the viewer named one (`--braw`).
    pub braw: Option<Result<crate::braw::Braw, String>>,
    /// libCZI, when this host was started for CZI files (`--czi`).
    pub czi: Result<crate::czi::Czi, String>,
    /// SQLite connections (GeoPackage, MBTiles), by the viewer's key.
    pub sqlite: crate::sqlite::Sqlite,
    pub threads: usize,
    pub info: HostInfo,
}

impl Host {
    /// Serves until the viewer goes away. Returns the exit status.
    pub fn run(mut self) -> i32 {
        // The first frame must be the hello.
        match self.recv() {
            Some((Request::Hello { version }, _)) => {
                let ok = version == VERSION;
                if !self.send(Response::Hello(Box::new(self.info.clone())), None) || !ok {
                    return 1;
                }
            }
            Some(_) => {
                self.send(Response::error(ErrorKind::Protocol, "expected hello"), None);
                return 1;
            }
            None => return 1,
        }
        while let Some((req, fds)) = self.recv() {
            let (resp, file) = self.handle(req, fds);
            if !self.send(resp, file) {
                return 1;
            }
        }
        0
    }

    fn conn(&self) -> MutexGuard<'_, Conn> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn recv(&mut self) -> Option<(Request, Vec<Fd>)> {
        let (mut body, fds) = self.conn().recv().ok()?;
        let req = Request::decode(&body);
        // The frame may have carried a password; the decoded request wipes its own copy.
        specular_sandbox::protocol::wipe(&mut body);
        match req {
            Ok(r) if r.fd_count() == fds.len() => Some((r, fds)),
            Ok(_) => {
                self.send(
                    Response::error(ErrorKind::Protocol, "descriptor count mismatch"),
                    None,
                );
                None
            }
            Err(e) => {
                self.send(Response::error(ErrorKind::Protocol, e.to_string()), None);
                None
            }
        }
    }

    #[cfg(unix)]
    fn send(&mut self, resp: Response, file: Option<File>) -> bool {
        use std::os::fd::AsFd;
        let fds: Vec<_> = file.iter().map(|f| f.as_fd()).collect();
        self.conn().send(&resp.encode(), &fds).is_ok()
    }

    /// No descriptors travel here, so there is never a file ([`shm::AVAILABLE`] is false).
    #[cfg(not(unix))]
    fn send(&mut self, resp: Response, file: Option<File>) -> bool {
        debug_assert!(file.is_none());
        self.conn().send(&resp.encode(), &[]).is_ok()
    }

    fn handle(&mut self, req: Request, fds: Vec<Fd>) -> (Response, Option<File>) {
        match req {
            Request::Hello { .. } => (Response::error(ErrorKind::Protocol, "repeated hello"), None),
            Request::HeifDecode { input, limits } => {
                let data = match input_bytes(input, fds) {
                    Ok(d) => d,
                    Err(r) => return (r, None),
                };
                let heif = match &self.heif {
                    Ok(h) => h,
                    Err(e) => {
                        return (
                            Response::error(
                                ErrorKind::NotInstalled,
                                format!("HEIC support not installed: libheif not found ({e})"),
                            ),
                            None,
                        );
                    }
                };
                let mut sink: Option<Sink> = None;
                let r = heif.decode(&data, limits, self.threads, &mut sink);
                match (r, sink) {
                    (Ok(m), Some(s)) => match s.finish() {
                        Ok((pixels, file)) => (
                            Response::Image(ImageReply {
                                width: m.width,
                                height: m.height,
                                has_alpha: m.has_alpha,
                                bit_depth: m.bit_depth,
                                icc: m.icc,
                                pixels,
                            }),
                            file,
                        ),
                        Err((k, e)) => (Response::error(k, e), None),
                    },
                    (Ok(_), None) => (Response::error(ErrorKind::Internal, "no image"), None),
                    (Err((k, e)), _) => (Response::error(k, e), None),
                }
            }
            Request::RarList {
                input,
                volume,
                max_entries,
                password,
            } => match take_file(input, fds) {
                Ok(f) => match rar_list(self.volumes(volume, f, password), max_entries) {
                    Ok(l) => (Response::Listing(l), None),
                    Err((k, e)) => (Response::error(k, e), None),
                },
                Err(r) => (r, None),
            },
            Request::RarRead {
                input,
                volume,
                index,
                name,
                max_size,
                password,
            } => match take_file(input, fds) {
                Ok(f) => {
                    match rar_read(self.volumes(volume, f, password), index, &name, max_size) {
                        Ok(data) => match payload(&data) {
                            Ok((p, f)) => (Response::Data(p), f),
                            Err((k, e)) => (Response::error(k, e), None),
                        },
                        Err((k, e)) => (Response::error(k, e), None),
                    }
                }
                Err(r) => (r, None),
            },
            Request::RarReadMany {
                input,
                volume,
                items,
                max_size,
                max_total,
                password,
            } => match take_file(input, fds) {
                Ok(f) => {
                    let v = self.volumes(volume, f, password);
                    let read = rar_read_many(v, &items, max_size, max_total);
                    match read.and_then(|all| {
                        let sizes = all.iter().map(|d| d.len() as u64).collect();
                        let (p, f) = payload_of(&all)?;
                        Ok((sizes, p, f))
                    }) {
                        Ok((sizes, payload, f)) => (Response::DataMany { sizes, payload }, f),
                        Err((k, e)) => (Response::error(k, e), None),
                    }
                }
                Err(r) => (r, None),
            },
            Request::CadRead {
                input,
                format,
                limits,
            } => {
                let data = match input_bytes(input, fds) {
                    Ok(d) => d,
                    Err(r) => return (r, None),
                };
                let occt = match &self.occt {
                    Ok(o) => o,
                    Err(e) => return (Response::error(ErrorKind::NotInstalled, e.clone()), None),
                };
                let model = occt.read(&data, format, limits);
                drop(data);
                match model.and_then(|m| payload(&m)) {
                    Ok((p, f)) => (Response::Model(p), f),
                    Err((k, e)) => (Response::error(k, e), None),
                }
            }
            Request::BrawDecode {
                input,
                frame,
                scale,
                limits,
            } => match take_file(input, fds) {
                Ok(f) => match &self.braw {
                    Some(Ok(b)) => match b.decode(f, frame, scale, limits) {
                        Ok((info, width, height, px)) => match payload(&px) {
                            Ok((pixels, file)) => (
                                Response::Clip {
                                    info,
                                    image: ImageReply {
                                        width,
                                        height,
                                        has_alpha: false,
                                        // Blackmagic RAW is 12-bit; the SDK hands out RGBA8.
                                        bit_depth: 12,
                                        icc: Vec::new(),
                                        pixels,
                                    },
                                },
                                file,
                            ),
                            Err((k, e)) => (Response::error(k, e), None),
                        },
                        Err((k, e)) => (Response::error(k, e), None),
                    },
                    Some(Err(e)) => (
                        Response::error(
                            ErrorKind::NotInstalled,
                            format!("the Blackmagic RAW SDK could not be used: {e}"),
                        ),
                        None,
                    ),
                    None => (
                        Response::error(
                            ErrorKind::NotInstalled,
                            "the Blackmagic RAW SDK is not installed",
                        ),
                        None,
                    ),
                },
                Err(r) => (r, None),
            },
            Request::CziInfo {
                input,
                key,
                max_xml,
            } => match (take_file(input, fds), &mut self.czi) {
                (Err(r), _) => (r, None),
                (Ok(_), Err(e)) => (Response::error(ErrorKind::NotInstalled, e.clone()), None),
                (Ok(f), Ok(c)) => match c.info(key, f, max_xml).and_then(|b| payload(&b)) {
                    Ok((p, f)) => (Response::CziInfo(p), f),
                    Err((k, e)) => (Response::error(k, e), None),
                },
            },
            Request::CziRead { input, key, region } => {
                match (take_file(input, fds), &mut self.czi) {
                    (Err(r), _) => (r, None),
                    (Ok(_), Err(e)) => (Response::error(ErrorKind::NotInstalled, e.clone()), None),
                    (Ok(f), Ok(c)) => czi_reply(c.read(key, f, &region)),
                }
            }
            Request::CziAttachment {
                input,
                key,
                index,
                max_side,
                max_bytes,
            } => match (take_file(input, fds), &mut self.czi) {
                (Err(r), _) => (r, None),
                (Ok(_), Err(e)) => (Response::error(ErrorKind::NotInstalled, e.clone()), None),
                (Ok(f), Ok(c)) => czi_reply(c.attachment(key, f, index, max_side, max_bytes)),
            },
            Request::SqlQuery {
                input,
                key,
                sql,
                params,
                limits,
            } => match take_file(input, fds) {
                Err(r) => (r, None),
                Ok(f) => match self
                    .sqlite
                    .query(key, f, &sql, &params, &limits, &mut || {
                        ask_volume(&self.conn, "wal")
                    })
                    .and_then(|(columns, rows, truncated, data)| {
                        let (p, f) = payload(&data)?;
                        Ok((columns, rows, truncated, p, f))
                    }) {
                    Ok((columns, rows, truncated, payload, f)) => (
                        Response::Rows {
                            columns,
                            rows,
                            truncated,
                            payload,
                        },
                        f,
                    ),
                    Err((k, e)) => (Response::error(k, e), None),
                },
            },
            Request::Test(op) if self.info.test_hooks => (crate::testops::run(op), None),
            Request::Test(_) => (
                Response::error(ErrorKind::Protocol, "test hooks are disabled"),
                None,
            ),
            Request::Volume { input } => {
                // Only valid as the answer to a volume question; drop what it carries.
                if let Some(i) = input {
                    let _ = take_file(i, fds);
                }
                (
                    Response::error(ErrorKind::Protocol, "unexpected volume"),
                    None,
                )
            }
        }
    }

    /// The files of a RAR request: `first`, and a way to ask the viewer for more; and its
    /// password.
    fn volumes(&self, name: String, first: File, password: Option<Secret>) -> Volumes {
        let conn = self.conn.clone();
        Volumes {
            first: (name, first),
            ask: Box::new(move |name: &str| ask_volume(&conn, name)),
            password,
        }
    }
}

/// The files UnRAR may open in one request.
#[cfg(feature = "rar")]
type Volumes = crate::rar::Volumes;

#[cfg(not(feature = "rar"))]
pub struct Volumes {
    pub first: (String, File),
    pub ask: Box<dyn FnMut(&str) -> Option<File> + Send>,
    pub password: Option<Secret>,
}

/// Asks the viewer for volume `name` in the middle of a request. The only valid answer is a
/// [`Request::Volume`]; anything else, or a broken connection, ends the host (the viewer sees
/// it die and restarts one).
fn ask_volume(conn: &Mutex<Conn>, name: &str) -> Option<File> {
    let mut c = conn.lock().unwrap_or_else(PoisonError::into_inner);
    let q = Response::NeedVolume {
        name: name.to_owned(),
    };
    if c.send(&q.encode(), &[]).is_err() {
        std::process::exit(1);
    }
    let Ok((body, fds)) = c.recv() else {
        std::process::exit(1)
    };
    match Request::decode(&body) {
        Ok(r) if r.fd_count() == fds.len() => match r {
            Request::Volume { input: None } => None,
            Request::Volume { input: Some(i) } => take_file(i, fds).ok(),
            _ => std::process::exit(1),
        },
        _ => std::process::exit(1),
    }
}

/// A CZI plane, its bytes moved to shared memory when large.
fn czi_reply(r: Result<specular_sandbox::protocol::CziPlane, Failure>) -> (Response, Option<File>) {
    let mut plane = match r {
        Ok(p) => p,
        Err((k, e)) => return (Response::error(k, e), None),
    };
    let data = match std::mem::replace(&mut plane.payload, Payload::Inline(Vec::new())) {
        Payload::Inline(d) => d,
        Payload::Fd { .. } => return (Response::error(ErrorKind::Internal, "no data"), None),
    };
    match payload(&data) {
        Ok((p, f)) => {
            plane.payload = p;
            (Response::CziPlane(plane), f)
        }
        Err((k, e)) => (Response::error(k, e), None),
    }
}

/// The open file a request carries: the descriptor that came with it (Unix) or the handle the
/// viewer duplicated into this process (Windows). Taken over at once, so it is closed even
/// when the request fails.
// The error is the response sent at once, once per request: its size does not matter.
#[allow(clippy::result_large_err)]
fn take_file(input: Input, fds: Vec<Fd>) -> Result<File, Response> {
    match input {
        #[cfg(unix)]
        Input::Fd => {
            let mut fds = fds;
            fds.pop()
                .map(File::from)
                .ok_or_else(|| Response::error(ErrorKind::Protocol, "input descriptor missing"))
        }
        #[cfg(windows)]
        Input::Handle(h) => {
            let _ = fds;
            use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};
            #[allow(unsafe_code)]
            // SAFETY: the viewer duplicated this handle into this process for this message
            // (`specular_sandbox::win::duplicate_for_reading`) and nothing else refers to it:
            // taking ownership here is the only use, and it is closed exactly once, on drop.
            let h = unsafe { OwnedHandle::from_raw_handle(h as usize as RawHandle) };
            Ok(File::from(h))
        }
        _ => {
            drop(fds);
            Err(Response::error(
                ErrorKind::Unsupported,
                "RAR, CZI and SQLite need the file as an open file",
            ))
        }
    }
}

#[allow(clippy::result_large_err)] // As `take_file`.
fn input_bytes(input: Input, mut fds: Vec<Fd>) -> Result<Vec<u8>, Response> {
    match input {
        Input::Inline(b) => Ok(b),
        Input::Fd => {
            let fd = fds
                .pop()
                .ok_or_else(|| Response::error(ErrorKind::Protocol, "input descriptor missing"))?;
            shm::read_all(fd, MAX_INPUT).map_err(|e| Response::error(ErrorKind::Io, e.to_string()))
        }
        Input::Handle(_) => {
            let _ = take_file(input, fds);
            Err(Response::error(
                ErrorKind::Unsupported,
                "file handles are for RAR only",
            ))
        }
    }
}

#[cfg(feature = "rar")]
fn rar_list(v: Volumes, max: u32) -> Result<specular_sandbox::protocol::Listing, Failure> {
    crate::rar::list(v, max)
}

#[cfg(feature = "rar")]
fn rar_read(v: Volumes, index: u32, name: &str, max: u64) -> Result<Vec<u8>, Failure> {
    crate::rar::read(v, index, name, max)
}

#[cfg(feature = "rar")]
fn rar_read_many(
    v: Volumes,
    items: &[(u32, String)],
    max: u64,
    total: u64,
) -> Result<Vec<Vec<u8>>, Failure> {
    crate::rar::read_many(v, items, max, total)
}

#[cfg(not(feature = "rar"))]
fn rar_read_many(
    _v: Volumes,
    _items: &[(u32, String)],
    _max: u64,
    _total: u64,
) -> Result<Vec<Vec<u8>>, Failure> {
    Err((ErrorKind::NotInstalled, "built without RAR support".into()))
}

#[cfg(not(feature = "rar"))]
fn rar_list(_v: Volumes, _max: u32) -> Result<specular_sandbox::protocol::Listing, Failure> {
    Err((ErrorKind::NotInstalled, "built without RAR support".into()))
}

#[cfg(not(feature = "rar"))]
fn rar_read(_v: Volumes, _i: u32, _n: &str, _max: u64) -> Result<Vec<u8>, Failure> {
    Err((ErrorKind::NotInstalled, "built without RAR support".into()))
}

fn io(e: std::io::Error) -> Failure {
    (ErrorKind::Io, e.to_string())
}

/// Where decoded bytes go: shared memory (Linux) or a buffer sent inline.
enum Sink {
    Shm(BufWriter<File>, u64),
    Vec(Vec<u8>),
}

impl Sink {
    fn new(len: u64) -> Result<Sink, Failure> {
        if shm::AVAILABLE && len > INLINE_MAX as u64 {
            let f = shm::create("specular-pixels").map_err(io)?;
            f.set_len(len)
                .map_err(|e| (ErrorKind::TooLarge, format!("output buffer: {e}")))?;
            return Ok(Sink::Shm(BufWriter::with_capacity(1 << 20, f), len));
        }
        let n = usize::try_from(len).map_err(|_| (ErrorKind::TooLarge, "too large".into()))?;
        let mut v = Vec::new();
        v.try_reserve_exact(n)
            .map_err(|_| (ErrorKind::TooLarge, format!("cannot allocate {len} bytes")))?;
        Ok(Sink::Vec(v))
    }

    fn write(&mut self, b: &[u8]) -> Result<(), Failure> {
        match self {
            Sink::Shm(w, _) => w.write_all(b).map_err(io),
            Sink::Vec(v) => {
                v.extend_from_slice(b);
                Ok(())
            }
        }
    }

    fn finish(self) -> Result<(Payload, Option<File>), Failure> {
        match self {
            Sink::Shm(w, len) => {
                let f = w.into_inner().map_err(|e| io(e.into_error()))?;
                shm::seal(&f).map_err(io)?;
                Ok((Payload::Fd { len }, Some(f)))
            }
            Sink::Vec(v) => Ok((Payload::Inline(v), None)),
        }
    }
}

impl Rows for Option<Sink> {
    fn start(&mut self, m: &Meta) -> Result<(), Failure> {
        *self = Some(Sink::new(u64::from(m.width) * u64::from(m.height) * 4)?);
        Ok(())
    }

    fn row(&mut self, row: &[u8]) -> Result<(), Failure> {
        match self {
            Some(s) => s.write(row),
            None => Err((ErrorKind::Internal, "rows before start".into())),
        }
    }
}

fn payload(data: &[u8]) -> Result<(Payload, Option<File>), Failure> {
    payload_of(&[data])
}

/// `parts` one after another in one payload.
fn payload_of(parts: &[impl AsRef<[u8]>]) -> Result<(Payload, Option<File>), Failure> {
    let len = parts.iter().map(|p| p.as_ref().len() as u64).sum();
    let mut s = Sink::new(len)?;
    for p in parts {
        s.write(p.as_ref())?;
    }
    s.finish()
}

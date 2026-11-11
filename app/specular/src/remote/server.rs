//! The socket server: an accept thread and one thread per connection (at most
//! [`MAX_CONNECTIONS`], and [`MAX_SUBSCRIBERS`] more that subscribed to events). Connection
//! threads parse requests and hand them to the event loop as [`Call`]s (a channel plus the
//! loader's wake); the loop answers each on the call's own reply channel, which the connection
//! thread writes back. Nothing a client sends can block the window: every wait here has a
//! timeout, and the event loop only ever does `try_recv`.
//!
//! The binary commands stay on the connection's thread ([`super::arrays`]): blobs are the
//! connection's own (freed when it closes), arrays are gathered and written here, and a
//! screenshot's pixels come back from the window to be encoded here, off the event loop.
//!
//! Events (version 3): `subscribe` and `unsubscribe` are answered here. A subscribed
//! connection gets a writer thread of its own that sends the lines the window publishes
//! ([`super::events`]) and a `ping` after a minute of silence; the connection then has no idle
//! limit. Both write through one lock, so lines never interleave.
//!
//! Windows: sockets there have no peer check, so the server writes a key next to its socket
//! ([`ipc::KeyFile`]) and answers nothing but `hello` with that key until a client sends it.

use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use parking_lot::Mutex;
use serde_json::{Value, json};
use specular_core::budget::MemoryBudget;
use specular_log::{SETTINGS, debug, info, warn};
use specular_platform::ipc::{self, Line, SocketFile, Stream};

use super::arrays::{self, Array, Blobs, Folders};
use super::events::{self, Bus, Subscriber};
use super::protocol::{self, ArraySource, Code, Envelope, Fail, Reply, Request};
use crate::loader::Wake;

/// Simultaneous clients; more are told `busy` and closed.
pub const MAX_CONNECTIONS: usize = 8;
/// Connections that subscribed to events, on top of [`MAX_CONNECTIONS`].
pub const MAX_SUBSCRIBERS: usize = 4;
/// A client idle this long is dropped (not a subscriber).
const IDLE: Duration = Duration::from_secs(120);
/// How long a connection waits for the window to answer one request.
const ANSWER: Duration = Duration::from_secs(5);
/// The longest `idle` wait a test client may ask for (debug builds).
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub const IDLE_MAX_MS: u64 = 30 * 60 * 1000;
/// Writes to a client that stopped reading give up after this.
const WRITE: Duration = Duration::from_secs(5);
/// A subscriber that has not read for this long is dropped.
const SUBSCRIBER_WRITE: Duration = Duration::from_secs(30);
/// A subscriber hears at least this often (`ping`).
pub const PING: Duration = Duration::from_secs(60);

/// A screenshot's pixels: width, height, RGBA.
pub type Pixels = (u32, u32, Vec<u8>);

/// What the window answers: the reply, and a screenshot's pixels.
pub struct Answer {
    pub reply: Reply,
    pub pixels: Option<Pixels>,
}

/// Who sent a request: a client, or a program a hook started (it said so in `hello`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Origin {
    #[default]
    Ipc,
    Hook(String),
}

/// A request for the event loop.
pub struct Call {
    pub req: Request,
    pub origin: Origin,
    reply: Sender<Answer>,
}

impl Call {
    /// Answers the request (the connection may have given up waiting: then nobody reads it).
    pub fn answer(self, r: Reply) {
        let _ = self.reply.try_send(Answer {
            reply: r,
            pixels: None,
        });
    }

    /// Answers a screenshot with its pixels (encoded on the connection's thread).
    pub fn answer_pixels(self, r: Reply, pixels: Pixels) {
        let _ = self.reply.try_send(Answer {
            reply: r,
            pixels: Some(pixels),
        });
    }

    /// A call and the receiver its answer arrives on (tests, and requests made in-process).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(req: Request) -> (Call, Receiver<Answer>) {
        Call::from(req, Origin::Ipc)
    }

    /// As [`Call::new`], sent by `origin`.
    pub fn from(req: Request, origin: Origin) -> (Call, Receiver<Answer>) {
        let (reply, rx) = crossbeam_channel::bounded(1);
        (Call { req, origin, reply }, rx)
    }
}

/// How a server runs.
pub struct Options {
    /// Where the window's events come from.
    pub bus: Arc<Bus>,
    /// Ask clients for the key ([`ipc::KeyFile`]) before anything else: on Windows.
    pub key: bool,
    /// The longest silence a subscriber hears.
    pub ping: Duration,
    /// An ordinary connection idle this long is dropped.
    pub idle: Duration,
    /// Connections reserve from this budget (tests) instead of the process's.
    pub budget: Option<Arc<MemoryBudget>>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            bus: Bus::new(),
            key: cfg!(windows),
            ping: PING,
            idle: IDLE,
            budget: None,
        }
    }
}

/// The running server. Dropping it stops accepting, removes the socket file and gives
/// answers on their way out a moment to be written.
pub struct Server {
    pub calls: Receiver<Call>,
    stop: Arc<AtomicBool>,
    /// Answers handed to a connection and not yet written.
    writing: Arc<AtomicUsize>,
    file: Option<SocketFile>,
    path: PathBuf,
    /// The arrays shown go with the server.
    folders: Arc<Mutex<Option<Folders>>>,
    #[cfg_attr(not(test), allow(dead_code))]
    bus: Arc<Bus>,
    /// The key file goes with the server (connections keep their copy of the key).
    key: Option<Arc<ipc::KeyFile>>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl Server {
    /// Listens in `dir`, a private folder ([`ipc::socket_dir`]).
    pub fn start(dir: &Path, wake: Wake) -> io::Result<Server> {
        Server::start_opts(dir, wake, Options::default())
    }

    /// As [`Server::start`], with connections reserving from `budget` (tests) instead of
    /// the process's.
    pub fn start_with(
        dir: &Path,
        wake: Wake,
        budget: Option<Arc<MemoryBudget>>,
    ) -> io::Result<Server> {
        Server::start_opts(
            dir,
            wake,
            Options {
                budget,
                ..Options::default()
            },
        )
    }

    /// As [`Server::start`], as `o` says.
    pub fn start_opts(dir: &Path, wake: Wake, o: Options) -> io::Result<Server> {
        let (listener, file) = ipc::bind(dir)?;
        let (tx, calls) = crossbeam_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let writing = Arc::new(AtomicUsize::new(0));
        let path = file.path().to_path_buf();
        let key = if o.key {
            Some(Arc::new(ipc::KeyFile::create(&path)?))
        } else {
            None
        };
        info!(target: SETTINGS, "remote control: listening on {}", path.display());
        let folders = match Folders::new(dir) {
            Ok(f) => Some(f),
            Err(e) => {
                warn!(target: SETTINGS, "remote control: no shared folder for arrays: {e}");
                None
            }
        };
        let folders = Arc::new(Mutex::new(folders));
        let shared = Shared {
            calls: tx,
            wake,
            stop: stop.clone(),
            writing: writing.clone(),
            open: Arc::new(AtomicUsize::new(0)),
            subs: Arc::new(AtomicUsize::new(0)),
            folders: folders.clone(),
            budget: o.budget,
            bus: o.bus.clone(),
            key: key.clone(),
            ping: o.ping,
            idle: o.idle,
        };
        let spawned =
            specular_thread::builder("rc-accept").spawn(move || accept_loop(&listener, &shared));
        spawned?;
        Ok(Server {
            calls,
            stop,
            writing,
            file: Some(file),
            path,
            folders,
            bus: o.bus,
            key,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The events this server's subscribers get.
    pub fn bus(&self) -> &Arc<Bus> {
        &self.bus
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Unblocks `accept`, which then sees the flag.
        let _ = ipc::connect(&self.path, Duration::from_millis(100));
        // The file goes first: no new client finds a server that is going away.
        self.file = None;
        if let Some(k) = self.key.take() {
            let _ = std::fs::remove_file(k.path());
        }
        let until = Instant::now() + Duration::from_millis(250);
        while self.writing.load(Ordering::Acquire) > 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        // The arrays shown go now, not when the last connection thread lets go.
        self.folders.lock().take();
    }
}

#[derive(Clone)]
struct Shared {
    calls: Sender<Call>,
    wake: Wake,
    stop: Arc<AtomicBool>,
    writing: Arc<AtomicUsize>,
    /// Connections that have not subscribed.
    open: Arc<AtomicUsize>,
    /// Connections that subscribed.
    subs: Arc<AtomicUsize>,
    folders: Arc<Mutex<Option<Folders>>>,
    budget: Option<Arc<MemoryBudget>>,
    bus: Arc<Bus>,
    key: Option<Arc<ipc::KeyFile>>,
    ping: Duration,
    idle: Duration,
}

/// A connection's place in the budget: an ordinary one, or among the subscribers. Given back
/// when the connection ends, however it ends.
struct Slot {
    open: Arc<AtomicUsize>,
    subs: Arc<AtomicUsize>,
    subscribed: bool,
}

impl Slot {
    /// Moves to the subscribers' budget; false when it is full.
    fn subscribe(&mut self) -> bool {
        if self.subscribed {
            return true;
        }
        if self.subs.fetch_add(1, Ordering::AcqRel) >= MAX_SUBSCRIBERS {
            self.subs.fetch_sub(1, Ordering::AcqRel);
            return false;
        }
        self.open.fetch_sub(1, Ordering::AcqRel);
        self.subscribed = true;
        true
    }

    /// Back among the ordinary connections.
    fn unsubscribe(&mut self) {
        if self.subscribed {
            self.open.fetch_add(1, Ordering::AcqRel);
            self.subs.fetch_sub(1, Ordering::AcqRel);
            self.subscribed = false;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        if self.subscribed {
            self.subs.fetch_sub(1, Ordering::AcqRel);
        } else {
            self.open.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

fn accept_loop(listener: &ipc::Listener, sh: &Shared) {
    loop {
        let s = match listener.accept() {
            Ok(s) => s,
            Err(e) => {
                if sh.stop.load(Ordering::Acquire) {
                    return;
                }
                warn!(target: SETTINGS, "remote control: accept: {e}");
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        if sh.stop.load(Ordering::Acquire) {
            return;
        }
        if sh.open.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            sh.open.fetch_sub(1, Ordering::AcqRel);
            let mut s = s;
            let _ = s.set_write_timeout(Some(Duration::from_millis(200)));
            let busy = Fail::new(Code::Busy, "too many connections");
            let _ = ipc::write_line(
                &mut s,
                protocol::response(protocol::VERSION, &Value::Null, &Err(busy)).as_bytes(),
            );
            continue;
        }
        let slot = Slot {
            open: sh.open.clone(),
            subs: sh.subs.clone(),
            subscribed: false,
        };
        let sh2 = sh.clone();
        let spawned = specular_thread::builder("rc-client").spawn(move || {
            let run = || connection(s, &sh2, slot);
            let run = || match sh2.budget.clone() {
                Some(b) => specular_core::budget::with_budget(b, run),
                None => run(),
            };
            // A panic ends this connection only, and gives its slot back (the slot goes as
            // the stack unwinds).
            match specular_core::isolate::run("remote control connection", run) {
                Ok(Err(e)) => debug!(target: SETTINGS, "remote control: connection ended: {e}"),
                Err(p) => debug!(target: SETTINGS, "remote control: connection ended: {p}"),
                Ok(Ok(())) => {}
            }
        });
        // A thread that did not start dropped its closure, and the slot with it.
        let _ = spawned;
    }
}

/// The writing half of a connection, shared by answers and events.
type Writer = Arc<Mutex<Stream>>;

fn send(w: &Writer, line: &str) -> io::Result<()> {
    ipc::write_line(&mut *w.lock(), line.as_bytes())
}

/// One connection's state.
struct Conn {
    slot: Slot,
    origin: Origin,
    /// The key was given (or none is asked for).
    known: bool,
    sub: Option<Arc<Subscriber>>,
}

fn connection(s: Stream, sh: &Shared, slot: Slot) -> io::Result<()> {
    s.set_read_timeout(Some(sh.idle))?;
    s.set_write_timeout(Some(WRITE))?;
    let w: Writer = Arc::new(Mutex::new(s.try_clone()?));
    let mut r = BufReader::new(s);
    // This connection's blobs, freed (and their memory given back) when it closes.
    let mut blobs = Blobs::default();
    let mut c = Conn {
        slot,
        origin: Origin::Ipc,
        known: sh.key.is_none(),
        sub: None,
    };
    let end = serve(&mut r, &w, sh, &mut c, &mut blobs);
    if let Some(s) = c.sub.take() {
        sh.bus.detach(&s);
    }
    end
}

fn serve(
    r: &mut BufReader<Stream>,
    w: &Writer,
    sh: &Shared,
    c: &mut Conn,
    blobs: &mut Blobs,
) -> io::Result<()> {
    loop {
        let line = match ipc::read_line(r, protocol::MAX_LINE)? {
            Line::End => return Ok(()),
            Line::TooLong => {
                let f = Fail::new(Code::TooLong, "request too long");
                send(
                    w,
                    &protocol::response(protocol::VERSION, &Value::Null, &Err(f)),
                )?;
                // What follows is the rest of that line: nothing to resynchronise on. Read a
                // little of it (briefly), so closing does not reset the error reply away.
                let _ = r
                    .get_ref()
                    .set_read_timeout(Some(Duration::from_millis(500)));
                ipc::drain(r, 4 * protocol::MAX_LINE as u64);
                return Ok(());
            }
            Line::Text(l) => l,
        };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if sh.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let (v, id, reply, quit) = match protocol::parse_request(&line) {
            Err((v, id, f)) => (v, id, Err(f), false),
            Ok(env) => {
                let quit = env.req == Request::Quit;
                let Envelope { v, id, req } = env;
                if req == Request::Hello {
                    let (key, hook) = protocol::hello_extras(&line);
                    if let Some(k) = &sh.key {
                        c.known = c.known || key.is_some_and(|g| k.matches(&g));
                    }
                    if let Some(h) = hook {
                        c.origin = Origin::Hook(h);
                    }
                }
                if !c.known {
                    let f = Fail::new(
                        Code::Refused,
                        if req == Request::Hello {
                            "wrong or missing key: send the key from the file next to the \
                             socket (<socket>.key) in hello"
                        } else {
                            "say hello first, with the key from the file next to the socket \
                             (<socket>.key)"
                        },
                    );
                    (v, id, Err(f), false)
                } else {
                    match req {
                        Request::Subscribe { events } => {
                            subscribe(w, r.get_ref(), sh, c, v, &id, &events)?;
                            continue;
                        }
                        Request::Unsubscribe { events } => {
                            let reply = unsubscribe(r.get_ref(), sh, c, events.as_deref());
                            (v, id, reply, false)
                        }
                        req => {
                            let reply = handle(v, req, sh, blobs, &c.origin);
                            (v, id, reply, quit)
                        }
                    }
                }
            }
        };
        let ok = reply.is_ok();
        let text = protocol::response(v, &id, &reply);
        let res = send(w, &text);
        if quit {
            sh.writing.fetch_sub(1, Ordering::AcqRel);
        }
        res?;
        if quit && ok {
            return Ok(());
        }
    }
}

/// `subscribe`: the first one starts this connection's event writer. The answer goes out
/// before any event, and every event numbered after its `seq` follows (or is counted lost).
fn subscribe(
    w: &Writer,
    stream: &Stream,
    sh: &Shared,
    c: &mut Conn,
    v: u64,
    id: &Value,
    events: &[String],
) -> io::Result<()> {
    if let Some(s) = &c.sub {
        s.add(events);
        let reply = Ok(json!({ "seq": sh.bus.seq() }));
        return send(w, &protocol::response(v, id, &reply));
    }
    if !c.slot.subscribe() {
        let f = Fail::new(
            Code::Busy,
            format!("at most {MAX_SUBSCRIBERS} connections may subscribe at once"),
        );
        return send(w, &protocol::response(v, id, &Err(f)));
    }
    let (s, rx) = Subscriber::new();
    s.add(events);
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(Some(SUBSCRIBER_WRITE))?;
    {
        // Held until the answer is out: the writer cannot send an event before it.
        let mut g = w.lock();
        let seq = sh.bus.attach(&s);
        let reply = Ok(json!({ "seq": seq }));
        ipc::write_line(&mut *g, protocol::response(v, id, &reply).as_bytes())?;
    }
    let weak = Arc::downgrade(&s);
    c.sub = Some(s);
    let (w2, bus, ping, stop) = (w.clone(), sh.bus.clone(), sh.ping, sh.stop.clone());
    let down = stream.try_clone()?;
    let spawned = specular_thread::builder("rc-events")
        .spawn(move || write_events(&rx, &weak, &w2, &bus, ping, &down, &stop));
    if let Err(e) = spawned {
        if let Some(s) = c.sub.take() {
            sh.bus.detach(&s);
        }
        c.slot.unsubscribe();
        return Err(e);
    }
    Ok(())
}

/// `unsubscribe`: what is left still comes; with nothing left the connection is an ordinary
/// one again (idle limit and all).
fn unsubscribe(stream: &Stream, sh: &Shared, c: &mut Conn, events: Option<&[String]>) -> Reply {
    let Some(s) = &c.sub else {
        return Ok(json!({ "subscribed": false }));
    };
    if !s.remove(events) {
        return Ok(json!({ "subscribed": true }));
    }
    sh.bus.detach(s);
    c.sub = None;
    c.slot.unsubscribe();
    let io = |e: io::Error| Fail::new(Code::Internal, e.to_string());
    stream.set_read_timeout(Some(sh.idle)).map_err(io)?;
    stream.set_write_timeout(Some(WRITE)).map_err(io)?;
    Ok(json!({ "subscribed": false }))
}

/// A subscriber's writer: its queued lines, `lost` once it has caught up after dropping
/// some, and `ping` after `ping` of silence. A write that fails (the client stopped reading
/// for [`SUBSCRIBER_WRITE`]) ends the connection, as the server going away does.
fn write_events(
    rx: &Receiver<Arc<str>>,
    sub: &Weak<Subscriber>,
    w: &Writer,
    bus: &Bus,
    ping: Duration,
    down: &Stream,
    stop: &AtomicBool,
) {
    let fail = |e: io::Error| {
        debug!(target: SETTINGS, "remote control: a subscriber stopped reading ({e})");
        if let Some(s) = sub.upgrade() {
            bus.detach(&s);
        }
        let _ = down.shutdown(std::net::Shutdown::Both);
    };
    loop {
        let r = match rx.recv_timeout(ping) {
            Ok(line) => send(w, &line),
            Err(RecvTimeoutError::Timeout) => {
                if sub.strong_count() == 0 {
                    return;
                }
                if stop.load(Ordering::Acquire) {
                    let _ = down.shutdown(std::net::Shutdown::Both);
                    return;
                }
                send(w, &events::ping(bus.seq()))
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Err(e) = r {
            fail(e);
            return;
        }
        if rx.is_empty()
            && let Some(s) = sub.upgrade()
        {
            let n = s.take_lost();
            if n > 0
                && let Err(e) = send(w, &events::lost(n, bus.seq()))
            {
                fail(e);
                return;
            }
        }
    }
}

fn handle(v: u64, req: Request, sh: &Shared, blobs: &mut Blobs, origin: &Origin) -> Reply {
    match req {
        Request::Hello => Ok(hello(v, sh)),
        Request::BlobNew { bytes } => Ok(json!({ "blob": blobs.create(bytes)?, "bytes": bytes })),
        Request::BlobWrite { blob, offset, data } => {
            blobs.write(blob, offset, &data)?;
            Ok(json!({ "written": data.len() }))
        }
        Request::BlobRead {
            blob,
            offset,
            length,
        } => {
            let d = blobs.read(blob, offset, length)?;
            Ok(json!({
                "data": specular_client::base64::encode(d),
                "bytes": d.len(),
                "total": blobs.len(blob)?,
            }))
        }
        Request::BlobFree { blob } => {
            blobs.free(blob)?;
            Ok(json!({}))
        }
        Request::ArrayShow {
            source,
            shape,
            dtype,
            strides,
            name,
        } => array_show(sh, blobs, source, &shape, &dtype, strides.as_deref(), name),
        Request::Screenshot => {
            let a = window(Request::Screenshot, sh, origin)?;
            let Some((w, h, rgba)) = a.pixels else {
                return a.reply;
            };
            let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, &rgba)
                .map_err(|e| Fail::new(Code::Internal, e.to_string()))?;
            drop(rgba);
            let bytes = png.len();
            let blob = blobs.insert(png)?;
            Ok(json!({
                "blob": blob,
                "bytes": bytes,
                "width": w,
                "height": h,
                "format": "png",
            }))
        }
        req => window(req, sh, origin)?.reply,
    }
}

/// Hands `req` to the window and waits (a while) for its answer.
fn window(req: Request, sh: &Shared, origin: &Origin) -> Result<Answer, Fail> {
    let quit = req == Request::Quit;
    // `idle` (a test command) is answered when the window settles or its own time runs out.
    let wait = match req {
        Request::Idle { timeout_ms, .. } => ANSWER + Duration::from_millis(timeout_ms),
        _ => ANSWER,
    };
    let (call, rx) = Call::from(req, origin.clone());
    // Counted before the window can see it: the window exits right after answering `quit`,
    // and the server's drop waits for this answer to be written.
    if quit {
        sh.writing.fetch_add(1, Ordering::AcqRel);
    }
    if sh.calls.send(call).is_err() {
        return Err(Fail::new(Code::Internal, "the viewer is shutting down"));
    }
    (sh.wake)();
    rx.recv_timeout(wait)
        .map_err(|_| Fail::new(Code::Timeout, "the viewer did not answer in time"))
}

fn array_show(
    sh: &Shared,
    blobs: &mut Blobs,
    source: ArraySource,
    shape: &[u64],
    dtype: &str,
    strides: Option<&[i64]>,
    name: Option<String>,
) -> Reply {
    let a = Array {
        shape,
        dtype,
        strides,
    };
    let no_folder = || Fail::new(Code::Internal, "the viewer has no folder for arrays");
    let (data, _reserved) = match &source {
        ArraySource::Blob(id) => (blobs.take(*id)?, None),
        ArraySource::File(f) => {
            let g = sh.folders.lock();
            let (d, r) = g.as_ref().ok_or_else(no_folder)?.take_shared(f)?;
            (d, Some(r))
        }
    };
    let c = a.gather(&data)?;
    drop(data);
    let npy = arrays::npy(dtype, shape, &c);
    drop(c);
    let path = {
        let mut g = sh.folders.lock();
        g.as_mut()
            .ok_or_else(no_folder)?
            .write(name.as_deref(), &npy)?
    };
    let shown = window(
        Request::Open {
            files: vec![path.clone()],
            window: protocol::Window::Current,
            view: None,
            compare: None,
        },
        sh,
        &Origin::Ipc,
    )?
    .reply?;
    let _ = shown;
    Ok(json!({
        "file": protocol::path_value(&path),
        "shape": shape,
        "dtype": dtype,
    }))
}

fn hello(v: u64, sh: &Shared) -> Value {
    if v < 2 {
        return json!({
            "app": "specular",
            "version": env!("CARGO_PKG_VERSION"),
            "protocol": 1,
            "pid": std::process::id(),
            "commands": protocol::COMMANDS_V1,
        });
    }
    let mut commands: Vec<&str> = protocol::COMMANDS_V1
        .iter()
        .chain(protocol::COMMANDS_V2)
        .copied()
        .collect();
    if v >= 3 {
        commands.extend(protocol::COMMANDS_V3);
    }
    let colormaps: Vec<&str> = specular_inspect::ColourMap::ALL
        .iter()
        .map(|m| m.id())
        .collect();
    let shared = sh
        .folders
        .lock()
        .as_ref()
        .map(|f| protocol::path_value(&f.shared));
    let mut out = json!({
        "app": "specular",
        "version": env!("CARGO_PKG_VERSION"),
        // A version 2 client is answered as before: in its version.
        "protocol": v.min(protocol::VERSION),
        "protocols": protocol::VERSIONS,
        "pid": std::process::id(),
        "commands": commands,
        "capabilities": {
            "max_line": protocol::MAX_LINE,
            "max_chunk": protocol::MAX_CHUNK,
            "max_blob": protocol::MAX_BLOB,
            "max_blobs": arrays::MAX_BLOBS,
            "max_dims": protocol::MAX_DIMS,
            "max_shapes": protocol::MAX_SHAPES,
            "max_points": protocol::MAX_POINTS,
            "dtypes": protocol::DTYPES,
            "array_sources": ["blob", "file"],
            "shared_dir": shared,
            "colormaps": colormaps,
            "shapes": ["point", "line", "polyline", "polygon", "rect", "text"],
            "roi_formats": ["json", "geojson"],
            "screenshot": ["png"],
            "view_command": ["shell", "python"],
        },
    });
    if v >= 3 {
        out["events"] = json!(protocol::EVENTS);
        let c = &mut out["capabilities"];
        c["max_subscribers"] = json!(MAX_SUBSCRIBERS);
        c["event_queue"] = json!(events::QUEUE);
        c["max_part_items"] = json!(events::MAX_PART_ITEMS);
        c["ping_seconds"] = json!(sh.ping.as_secs());
        c["labels"] = json!(protocol::LABELS);
    }
    out
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use std::io::{BufRead, Read, Write};

    use super::*;
    use crate::remote::client::Client;
    use crate::remote::protocol::Nav;

    fn test_dir(name: &str) -> PathBuf {
        // Short (socket paths are limited to ~100 bytes, which a long `$TMPDIR` exceeds) and
        // private to this process. Never `ipc::socket_dir`: a running viewer's socket is there.
        ipc::scratch_dir(&format!("specular-rc-{name}")).unwrap()
    }

    /// A fake window: answers calls from its own thread until the server goes away.
    fn fake_window(calls: Receiver<Call>) -> std::thread::JoinHandle<Vec<Request>> {
        std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut index = 0usize;
            while let Ok(c) = calls.recv() {
                seen.push(c.req.clone());
                let r = match &c.req {
                    Request::Navigate(Nav::Next) => {
                        index += 1;
                        Ok(json!({"index": index}))
                    }
                    Request::Navigate(Nav::Index(i)) if *i < 10 => {
                        index = *i;
                        Ok(json!({"index": index}))
                    }
                    Request::Navigate(_) => Err(Fail::new(Code::NotFound, "no such image")),
                    Request::State => Ok(json!({"index": index, "count": 10})),
                    Request::Action { name } if name == "slow" => {
                        std::thread::sleep(ANSWER + Duration::from_millis(500));
                        Ok(json!({}))
                    }
                    Request::Quit => {
                        c.answer(Ok(json!({})));
                        break;
                    }
                    _ => Ok(json!({})),
                };
                c.answer(r);
            }
            seen
        })
    }

    fn raw(path: &Path) -> (BufReader<Stream>, Stream) {
        let s = ipc::connect(path, Duration::from_secs(10)).unwrap();
        let w = s.try_clone().unwrap();
        (BufReader::new(s), w)
    }

    fn roundtrip(r: &mut BufReader<Stream>, w: &mut Stream, line: &[u8]) -> Value {
        w.write_all(line).unwrap();
        w.write_all(b"\n").unwrap();
        let mut out = String::new();
        r.read_line(&mut out).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    /// The hand-over of a second start (Windows single instance): files go to the current
    /// window, no files just raise it, and with no viewer the start carries on itself.
    #[test]
    fn a_second_start_hands_over() {
        let dir = test_dir("handover");
        let server = Server::start(&dir, Arc::new(|| {})).unwrap();
        let path = server.path().to_path_buf();
        let window = fake_window(server.calls.clone());
        let files = vec![dir.join("a.jpg"), dir.join("b.jpg")];
        assert!(crate::remote::forward_files(&path, &files));
        assert!(crate::remote::forward_files(&path, &[]));
        Client::connect(&path)
            .unwrap()
            .call(&Request::Quit)
            .unwrap();
        let seen = window.join().unwrap();
        assert!(seen.contains(&Request::Open {
            files,
            window: crate::remote::protocol::Window::Current,
            view: None,
            compare: None,
        }));
        assert!(seen.contains(&Request::Activate));
        drop(server);
        assert!(!crate::remote::forward_files(&path, &[]), "nobody listens");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn drives_the_window_through_the_socket() {
        let dir = test_dir("drive");
        let woken = Arc::new(AtomicUsize::new(0));
        let w2 = woken.clone();
        // Raw connections below talk the line protocol: no key (tests_v3 tests the key).
        let server = Server::start_opts(
            &dir,
            Arc::new(move || {
                w2.fetch_add(1, Ordering::Relaxed);
            }),
            Options {
                key: false,
                ..Options::default()
            },
        )
        .unwrap();
        let path = server.path().to_path_buf();
        let window = fake_window(server.calls.clone());

        // Through the client the CLI uses.
        let mut c = Client::connect(&path).unwrap();
        let hello = c.call(&Request::Hello).unwrap();
        assert_eq!(hello["protocol"], 3);
        assert_eq!(hello["pid"], std::process::id());
        assert_eq!(c.call(&Request::Navigate(Nav::Next)).unwrap()["index"], 1);
        assert_eq!(
            c.call(&Request::Navigate(Nav::Index(7))).unwrap()["index"],
            7
        );
        let e = c.call(&Request::Navigate(Nav::Index(70))).unwrap_err();
        assert_eq!(e.code, "not-found");
        assert_eq!(c.call(&Request::State).unwrap()["index"], 7);

        // Raw lines: garbage gets an error and the connection stays usable.
        let (mut r, mut w) = raw(&path);
        let v = roundtrip(&mut r, &mut w, b"{oops");
        assert_eq!(v["error"]["code"], "parse");
        let v = roundtrip(&mut r, &mut w, b"\xff\xfe\x00");
        assert_eq!(v["error"]["code"], "parse");
        let v = roundtrip(&mut r, &mut w, br#"{"v":9,"id":"x","cmd":"state"}"#);
        assert_eq!(
            (v["id"].clone(), v["error"]["code"].clone()),
            (json!("x"), json!("version"))
        );
        let v = roundtrip(&mut r, &mut w, br#"{"v":1,"id":2,"cmd":"state"}"#);
        assert_eq!((v["id"].clone(), v["ok"].clone()), (json!(2), json!(true)));
        // CRLF is fine; blank lines are skipped.
        w.write_all(b"\r\n\n").unwrap();
        let v = roundtrip(&mut r, &mut w, b"{\"v\":1,\"cmd\":\"hello\"}\r");
        assert_eq!(v["result"]["app"], "specular");

        // An over-long line: one error, then the server hangs up.
        let (mut r2, mut w2) = raw(&path);
        let big = vec![b'x'; protocol::MAX_LINE + 10];
        let writer = std::thread::spawn(move || {
            let _ = w2.write_all(&big);
            let _ = w2.write_all(b"\n");
        });
        let mut out = String::new();
        r2.read_line(&mut out).unwrap();
        assert!(out.contains("too-long"), "{out}");
        writer.join().unwrap();
        // The server hangs up by itself even though this side never closes.
        let t = Instant::now();
        let mut rest = Vec::new();
        r2.read_to_end(&mut rest).unwrap();
        assert!(rest.is_empty());
        assert!(t.elapsed() < Duration::from_secs(5));

        // More than MAX_CONNECTIONS at once: the extra one is told it is busy. (`c` holds
        // one; the closed ones need a moment to give theirs back.)
        // The closed ones give theirs back on their own threads, which a loaded machine
        // may delay: until then a new connection is told it is busy, so try again.
        drop((r, w));
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut held = Vec::new();
        while held.len() < MAX_CONNECTIONS - 1 {
            // Each holds a slot once its thread has answered.
            let (mut r, mut w) = raw(&path);
            let mut out = String::new();
            let ok = w.write_all(b"{\"v\":1,\"cmd\":\"hello\"}\n").is_ok()
                && r.read_line(&mut out).is_ok()
                && serde_json::from_str::<Value>(&out).is_ok_and(|v| v["ok"] == true);
            if ok {
                held.push((r, w));
            } else {
                assert!(Instant::now() < deadline, "slots never came back: {out}");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let (mut r3, _w3) = raw(&path);
        let mut out = String::new();
        r3.read_line(&mut out).unwrap();
        assert!(out.contains("busy"), "{out}");
        drop(held);
        drop(c);

        // A window that does not answer in time: a timeout, not a hang. The connections
        // just dropped give their slots back on their own threads, which can take a while on
        // a loaded machine: until then the server says "busy" (or closes the connection
        // before the answer arrives, an "io" error), so try again.
        let deadline = Instant::now() + Duration::from_secs(30);
        let (mut c, e, took) = loop {
            let mut c = Client::connect_with_retry(&path, Duration::from_secs(2)).unwrap();
            let t = Instant::now();
            let e = c
                .call(&Request::Action {
                    name: "slow".into(),
                })
                .unwrap_err();
            if (e.code == "busy" || e.code == "io") && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            break (c, e, t.elapsed());
        };
        assert_eq!(e.code, "timeout");
        assert!(took < ANSWER + Duration::from_secs(3));

        // `quit` is answered, then the window loop ends.
        c.call(&Request::Quit).unwrap();
        let seen = window.join().unwrap();
        assert!(seen.contains(&Request::Navigate(Nav::Index(7))));
        assert!(
            !seen.contains(&Request::Hello),
            "hello is answered by the server"
        );
        assert!(woken.load(Ordering::Relaxed) >= 5);

        drop(server);
        assert!(!path.exists(), "socket file removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn socket_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = test_dir("perm");
        let sock_dir = dir.clone();
        let server = Server::start(&sock_dir, Arc::new(|| {})).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&sock_dir), 0o700);
        assert_eq!(mode(server.path()), 0o600);
        // A second viewer gets its own socket; the first keeps the default name.
        let second = Server::start(&sock_dir, Arc::new(|| {})).unwrap();
        assert_ne!(second.path(), server.path());
        drop(second);
        assert!(server.path().exists());
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

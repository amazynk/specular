//! Tethered capture (`specular_ptp`): a camera on USB, its shots downloaded into a folder as
//! they are taken, and each shown as it arrives.
//!
//! - The panel ([`Action::Tether`], top end of the window, any mode): the camera (the next
//!   one on a click when several are plugged in), its battery and card, the exposure
//!   settings it reports (− and + step through the values it offers), the folder and file
//!   names (`cull.tether_folder`, `cull.tether_rename`, the Import rename tokens), the
//!   Develop style new shots get (Develop's "Next capture"), and Capture.
//! - The session runs on thread `io-tether`: events, downloads (to a hidden `.part` file,
//!   renamed when complete) and the camera's settings. Shots taken with the camera's own
//!   button are downloaded too.
//! - Arrival: in cull mode on that folder the shot joins the run as the hot folder's files
//!   do (`t` follows them; following is turned on when the camera connects); otherwise the
//!   viewer shows it. Develop's next-capture style applies either way.
//! - When the camera can't be opened, the panel says why and what to do (Linux: the udev
//!   rule, with a Copy button; another program holding it, by name). Nothing on the system
//!   is changed.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::{I18n, t};
use specular_import::RenamePattern;
use specular_import::pattern::{Facts, sanitize};
use specular_input::Action;
use specular_log::{IO, debug, info};
use specular_ptp::codes::{op, prop, rc};
use specular_ptp::props::{Program, Shown, WhiteBalance, show};
use specular_ptp::tether::{Arrivals, DownloadError, Happening, download};
use specular_ptp::usb::{self, Found, OpenError};
use specular_ptp::{Camera, Error, ObjectInfo, PropDesc, StorageInfo, Transport, Value};
use specular_theme::Palette;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_widgets::accesskit;

use super::App;
use crate::loader::{Copied, Wake};

/// AccessKit ids: the panel, then its controls in drawing order.
const A11Y_ID: u64 = 0x6700_0000_0000_0000;
/// Logical width of the panel.
const PANEL_W: f32 = 320.0;
/// How long the session waits for an event per turn of its loop.
const EVENT_WAIT: Duration = Duration::from_millis(150);
/// How often battery and card are read again.
const FACTS_EVERY: Duration = Duration::from_secs(30);

/// What the session is asked to do.
enum Cmd {
    Capture,
    Set {
        code: u16,
        data_type: u16,
        value: Value,
    },
    Stop,
}

/// Why the camera couldn't be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Problem {
    /// Linux: the device node isn't the user's; the udev rule that makes it so.
    Permission { node: Option<String>, rule: String },
    /// Another program has it.
    Busy { holders: Vec<String> },
    /// The system driver doesn't let programs in (Windows).
    Driver,
    /// The camera didn't answer as a PTP camera does.
    Other(String),
}

impl From<OpenError> for Problem {
    fn from(e: OpenError) -> Problem {
        match e {
            OpenError::Permission { node, rule } => Problem::Permission { node, rule },
            OpenError::Busy { holders } => Problem::Busy { holders },
            OpenError::Driver => Problem::Driver,
            OpenError::Gone => Problem::Other(t!("tether-gone")),
            OpenError::Other(s) => Problem::Other(s),
        }
    }
}

/// Battery, cards and exposure settings, as last read.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct CameraFacts {
    pub battery: Option<u8>,
    pub storages: Vec<StorageInfo>,
    pub props: Vec<PropDesc>,
}

/// What the session reports.
pub(super) enum Msg {
    Connected {
        name: String,
        capture: bool,
    },
    Facts(CameraFacts),
    Progress {
        done: u64,
        total: u64,
    },
    Saved(PathBuf),
    /// Something asked or done failed; the message to show.
    Failed(String),
    Problem(Problem),
    Gone,
}

/// The connection.
#[derive(Clone, Debug, Default, PartialEq)]
enum Conn {
    #[default]
    Idle,
    Connecting,
    Connected {
        name: String,
        capture: bool,
    },
    Problem(Problem),
}

/// A running session.
struct Link {
    tx: Sender<Cmd>,
    rx: Receiver<Msg>,
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

/// A control of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Close,
    NextCamera,
    LookAgain,
    Connect,
    Disconnect,
    Capture,
    /// Steps property `code` by one value down (false) or up.
    Step(u16, bool),
    Folder,
    Arrival,
    CopyRule,
}

/// The panel's state.
#[derive(Default)]
pub(super) struct Tether {
    open: bool,
    found: Vec<Found>,
    pick: usize,
    link: Option<Link>,
    conn: Conn,
    facts: Option<CameraFacts>,
    /// The last line: progress, the last file saved, or what failed.
    status: Option<String>,
    saved: usize,
    drawn: Option<(Rect, Vec<(Rect, Hit)>)>,
    /// Debug builds, `SPECULAR_TETHER_MOCK=DIR`: the in-memory camera on the session and its
    /// folder (whose `.card-full` and `.unplug` files pull the card or the cable).
    #[cfg(debug_assertions)]
    mock: Option<(PathBuf, specular_ptp::mock::MockCamera)>,
}

impl Tether {
    /// Where the panel and its controls were drawn (logical px), for clicks.
    pub(super) fn set_drawn(&mut self, d: Option<(Rect, Vec<(Rect, Hit)>)>) {
        self.drawn = d;
    }
}

/// A line of the panel.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Row {
    Text {
        text: String,
        muted: bool,
    },
    /// Text to copy as it is (the udev rule).
    Code(String),
    /// A setting: its name and value, with − and + when it can be changed.
    Value {
        label: String,
        value: String,
        step: Option<u16>,
    },
    /// Buttons: label, control, primary, enabled.
    Buttons(Vec<(String, Hit, bool, bool)>),
}

/// The panel to draw.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Panel {
    pub title: String,
    pub rows: Vec<Row>,
}

/// A property's name.
fn prop_label(code: u16) -> String {
    match code {
        prop::EXPOSURE_PROGRAM_MODE => t!("tether-prop-mode"),
        prop::EXPOSURE_TIME => t!("tether-prop-shutter"),
        prop::F_NUMBER => t!("tether-prop-aperture"),
        prop::EXPOSURE_INDEX => t!("tether-prop-iso"),
        prop::EXPOSURE_BIAS_COMPENSATION => t!("tether-prop-bias"),
        prop::WHITE_BALANCE => t!("tether-prop-white-balance"),
        c => format!("{c:#06x}"),
    }
}

/// A property's value as shown.
pub(super) fn value_text(code: u16, v: &Value) -> String {
    match show(code, v) {
        Shown::Text(s) => s,
        Shown::Auto => t!("tether-auto"),
        Shown::Bulb => t!("tether-bulb"),
        Shown::WhiteBalance(w) => match w {
            WhiteBalance::Manual => t!("tether-wb-manual"),
            WhiteBalance::Auto => t!("tether-auto"),
            WhiteBalance::OnePush => t!("tether-wb-one-push"),
            WhiteBalance::Daylight => t!("tether-wb-daylight"),
            WhiteBalance::Fluorescent => t!("tether-wb-fluorescent"),
            WhiteBalance::Tungsten => t!("tether-wb-tungsten"),
            WhiteBalance::Flash => t!("tether-wb-flash"),
        },
        Shown::Program(p) => match p {
            Program::Manual => t!("tether-mode-manual"),
            Program::Auto => t!("tether-mode-program"),
            Program::Aperture => t!("tether-mode-aperture"),
            Program::Shutter => t!("tether-mode-shutter"),
            Program::Creative => t!("tether-mode-creative"),
            Program::Action => t!("tether-mode-action"),
            Program::Portrait => t!("tether-mode-portrait"),
        },
        Shown::Other(x) => t!("tether-camera-value", value = format!("{x:#x}")),
    }
}

fn size_text(bytes: u64) -> String {
    I18n::global()
        .localizer()
        .bytes(bytes, specular_i18n::ByteUnits::Decimal)
}

/// What a failed operation means to the user.
fn failure_text(e: &Error) -> String {
    match e {
        Error::Unsupported(op::INITIATE_CAPTURE) => t!("tether-no-remote"),
        Error::Response { code, op: o } => match *code {
            rc::DEVICE_BUSY => t!("tether-busy"),
            rc::STORE_FULL => t!("tether-card-full"),
            rc::STORE_NOT_AVAILABLE => t!("tether-no-card"),
            rc::OPERATION_NOT_SUPPORTED if *o == op::INITIATE_CAPTURE => t!("tether-no-remote"),
            rc::ACCESS_DENIED | rc::INVALID_DEVICE_PROP_VALUE | rc::INVALID_DEVICE_PROP_FORMAT
                if *o == op::SET_DEVICE_PROP_VALUE =>
            {
                t!("tether-set-refused")
            }
            _ => t!("tether-error", error = e.to_string()),
        },
        e if e.is_gone() => t!("tether-gone"),
        e => t!("tether-error", error = e.to_string()),
    }
}

/// Names the session's files: the camera's name, or the rename pattern with the shot's
/// number (a RAW and its JPEG share one).
struct Namer {
    pattern: Option<RenamePattern>,
    camera: String,
    shot: usize,
    last_stem: Option<String>,
}

impl Namer {
    fn name(&mut self, info: &ObjectInfo) -> String {
        let file = sanitize(&info.filename);
        let (stem, ext) = match file.rfind('.') {
            Some(i) if i > 0 => (file[..i].to_owned(), file[i + 1..].to_owned()),
            _ => (file.clone(), String::new()),
        };
        if self.last_stem.as_deref() != Some(&stem) {
            self.shot += 1;
            self.last_stem = Some(stem.clone());
        }
        let Some(p) = &self.pattern else {
            return file;
        };
        let date = info.capture_time().unwrap_or_else(now_parts);
        p.render(
            &stem,
            &ext,
            self.shot,
            &Facts {
                date,
                camera: &self.camera,
                event: "",
            },
        )
    }
}

fn now_parts() -> [u32; 6] {
    use chrono::{Datelike, Timelike};
    let n = chrono::Local::now();
    [
        n.year().max(0) as u32,
        n.month(),
        n.day(),
        n.hour(),
        n.minute(),
        n.second(),
    ]
}

fn read_facts<T: Transport>(cam: &mut Camera<T>) -> CameraFacts {
    let mut storages = Vec::new();
    if let Ok(ids) = cam.storage_ids() {
        for id in ids {
            if let Ok(s) = cam.storage_info(id) {
                storages.push(s);
            }
        }
    }
    CameraFacts {
        battery: cam.battery(),
        storages,
        props: cam.exposure(),
    }
}

/// The session: runs until told to stop, the app drops its end, or the camera goes.
fn session<T: Transport>(
    open: impl FnOnce() -> Result<T, Problem>,
    dir: PathBuf,
    pattern: Option<RenamePattern>,
    tx: Sender<Msg>,
    rx: Receiver<Cmd>,
    wake: Wake,
) {
    let send = |m: Msg| {
        if tx.send(m).is_ok() {
            wake();
        }
    };
    let t = match open() {
        Ok(t) => t,
        Err(p) => return send(Msg::Problem(p)),
    };
    let mut cam = match Camera::open(t) {
        Ok(c) => c,
        Err(e) => return send(Msg::Problem(Problem::Other(e.to_string()))),
    };
    let name = cam.info().name();
    info!(target: IO, "tether: {name} connected");
    send(Msg::Connected {
        name: name.clone(),
        capture: cam.supports(op::INITIATE_CAPTURE),
    });
    send(Msg::Facts(read_facts(&mut cam)));
    let mut arr = match Arrivals::start(&mut cam, Instant::now()) {
        Ok(a) => a,
        Err(e) => return send(Msg::Problem(Problem::Other(e.to_string()))),
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        send(Msg::Failed(t!(
            "tether-disk",
            error = format!("{}: {e}", dir.display())
        )));
    }
    let mut namer = Namer {
        pattern,
        camera: name,
        shot: 0,
        last_stem: None,
    };
    let mut facts_at = Instant::now();
    loop {
        let mut refresh = false;
        loop {
            match rx.try_recv() {
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => break,
                Ok(Cmd::Capture) => {
                    if let Err(e) = cam.initiate_capture() {
                        send(Msg::Failed(failure_text(&e)));
                        if e.is_gone() {
                            return send(Msg::Gone);
                        }
                        // A refusal (a full or missing card) changes what the panel shows
                        // of the card: read it again now, not at the next interval.
                        refresh = true;
                    }
                }
                Ok(Cmd::Set {
                    code,
                    data_type,
                    value,
                }) => {
                    if let Err(e) = cam.set_prop_value(code, data_type, &value) {
                        send(Msg::Failed(failure_text(&e)));
                        if e.is_gone() {
                            return send(Msg::Gone);
                        }
                    }
                    refresh = true;
                }
            }
        }
        let happened = match arr.poll(&mut cam, EVENT_WAIT, Instant::now()) {
            Ok(h) => h,
            Err(e) if e.is_gone() => return send(Msg::Gone),
            Err(e) if e.is_busy() => Vec::new(),
            Err(e) => {
                debug!(target: IO, "tether: {e}");
                std::thread::sleep(EVENT_WAIT);
                Vec::new()
            }
        };
        for h in happened {
            match h {
                Happening::Object(handle) => {
                    let r = download(
                        &mut cam,
                        handle,
                        &dir,
                        &mut |i| namer.name(i),
                        &mut |done, total| {
                            // Progress at most every 4 MiB.
                            if done == total || done % (4 << 20) < (1 << 20) {
                                send(Msg::Progress { done, total });
                            }
                        },
                    );
                    match r {
                        Ok(Some(d)) => {
                            debug!(target: IO, "tether: saved {} ({} bytes)", d.path.display(), d.bytes);
                            send(Msg::Saved(d.path));
                        }
                        Ok(None) => {}
                        Err(DownloadError::Camera(e)) if e.is_gone() => return send(Msg::Gone),
                        Err(DownloadError::Camera(e)) => send(Msg::Failed(failure_text(&e))),
                        Err(e @ DownloadError::Disk(..)) => {
                            send(Msg::Failed(t!("tether-disk", error = e.to_string())))
                        }
                    }
                }
                Happening::StoreFull => {
                    send(Msg::Failed(t!("tether-card-full")));
                    refresh = true;
                }
                Happening::CaptureDone | Happening::PropChanged(_) | Happening::Storage => {
                    refresh = true
                }
            }
        }
        if refresh || facts_at.elapsed() >= FACTS_EVERY {
            facts_at = Instant::now();
            send(Msg::Facts(read_facts(&mut cam)));
        }
    }
}

/// Starts a session on its thread.
fn start<T: Transport + 'static>(
    open: impl FnOnce() -> Result<T, Problem> + Send + 'static,
    dir: PathBuf,
    pattern: Option<RenamePattern>,
    wake: Wake,
) -> Link {
    let (ctx, crx) = crossbeam_channel::unbounded();
    let (mtx, mrx) = crossbeam_channel::unbounded();
    specular_thread::spawn("io-tether", move || {
        session(open, dir, pattern, mtx, crx, wake)
    });
    Link { tx: ctx, rx: mrx }
}

/// The panel's rows for the state (the folder, the names and the arrival style given).
fn rows(t: &Tether, folder: &Path, rename: &str, arrival: String) -> Vec<Row> {
    let mut rows = Vec::new();
    let text = |s: String| Row::Text {
        text: s,
        muted: false,
    };
    let muted = |s: String| Row::Text {
        text: s,
        muted: true,
    };
    let found = t.found.get(t.pick);
    match (&t.conn, found) {
        (Conn::Idle, None) => {
            rows.push(text(t!("tether-none")));
            rows.push(muted(t!("tether-none-hint")));
            rows.push(Row::Buttons(vec![(
                t!("tether-look-again"),
                Hit::LookAgain,
                false,
                true,
            )]));
        }
        (Conn::Idle, Some(f)) => {
            rows.push(text(f.name()));
            let mut b = vec![(t!("tether-connect"), Hit::Connect, true, true)];
            if t.found.len() > 1 {
                b.push((t!("tether-next-camera"), Hit::NextCamera, false, true));
            }
            b.push((t!("tether-look-again"), Hit::LookAgain, false, true));
            rows.push(Row::Buttons(b));
        }
        (Conn::Connecting, f) => {
            rows.push(text(f.map(Found::name).unwrap_or_default()));
            rows.push(muted(t!("tether-connecting")));
        }
        (Conn::Problem(p), f) => {
            if let Some(f) = f {
                rows.push(text(f.name()));
            }
            match p {
                Problem::Permission { node, rule } => {
                    rows.push(text(t!(
                        "tether-permission",
                        node = node.clone().unwrap_or_default()
                    )));
                    rows.push(muted(t!(
                        "tether-permission-rule",
                        file = usb::UDEV_RULE_FILE
                    )));
                    rows.push(Row::Code(rule.clone()));
                    rows.push(muted(t!("tether-permission-reload")));
                    rows.push(Row::Buttons(vec![
                        (t!("tether-copy-rule"), Hit::CopyRule, false, true),
                        (t!("tether-try-again"), Hit::Connect, true, true),
                    ]));
                }
                Problem::Busy { holders } => {
                    rows.push(text(if holders.is_empty() {
                        t!("tether-busy-other")
                    } else {
                        t!("tether-busy-named", programs = holders.join(", "))
                    }));
                    rows.push(muted(t!("tether-busy-hint")));
                    rows.push(Row::Buttons(vec![(
                        t!("tether-try-again"),
                        Hit::Connect,
                        true,
                        true,
                    )]));
                }
                Problem::Driver => {
                    rows.push(text(t!("tether-driver")));
                    rows.push(Row::Buttons(vec![(
                        t!("tether-try-again"),
                        Hit::Connect,
                        true,
                        true,
                    )]));
                }
                Problem::Other(e) => {
                    rows.push(text(t!("tether-error", error = e.clone())));
                    rows.push(Row::Buttons(vec![
                        (t!("tether-try-again"), Hit::Connect, true, true),
                        (t!("tether-look-again"), Hit::LookAgain, false, true),
                    ]));
                }
            }
        }
        (Conn::Connected { name, capture }, _) => {
            rows.push(text(name.clone()));
            if let Some(f) = &t.facts {
                if let Some(b) = f.battery {
                    rows.push(muted(t!("tether-battery", percent = i64::from(b))));
                }
                if f.storages.is_empty() {
                    rows.push(muted(t!("tether-no-card")));
                }
                for s in &f.storages {
                    let label = if s.volume_label.trim().is_empty() {
                        s.description.trim().to_owned()
                    } else {
                        s.volume_label.trim().to_owned()
                    };
                    let free = size_text(s.free_bytes);
                    rows.push(muted(if s.free_images == u32::MAX {
                        t!("tether-card", card = label, free = free)
                    } else {
                        t!(
                            "tether-card-shots",
                            card = label,
                            free = free,
                            shots = i64::from(s.free_images)
                        )
                    }));
                }
                for d in &f.props {
                    let step =
                        (d.writable && d.choices(2).is_some_and(|c| c.len() > 1)).then_some(d.code);
                    rows.push(Row::Value {
                        label: prop_label(d.code),
                        value: value_text(d.code, &d.current),
                        step,
                    });
                }
                if f.props.is_empty() {
                    rows.push(muted(t!("tether-no-settings")));
                }
            }
            if !capture {
                rows.push(muted(t!("tether-no-remote")));
            }
            rows.push(Row::Buttons(vec![
                (t!("tether-capture"), Hit::Capture, true, *capture),
                (t!("tether-disconnect"), Hit::Disconnect, false, true),
            ]));
        }
    }
    rows.push(Row::Value {
        label: t!("tether-folder"),
        value: folder.display().to_string(),
        step: None,
    });
    rows.push(Row::Value {
        label: t!("tether-names"),
        value: if rename.trim().is_empty() {
            t!("tether-names-camera")
        } else {
            rename.to_owned()
        },
        step: None,
    });
    rows.push(Row::Value {
        label: t!("tether-arrival"),
        value: arrival,
        step: None,
    });
    rows.push(Row::Buttons(vec![
        (t!("tether-choose-folder"), Hit::Folder, false, true),
        (t!("tether-choose-style"), Hit::Arrival, false, true),
    ]));
    if let Some(s) = &t.status {
        rows.push(muted(s.clone()));
    } else if t.saved > 0 {
        rows.push(muted(t!("tether-saved-count", count = t.saved as i64)));
    }
    rows
}

impl App {
    /// The folder shots go to: `cull.tether_folder`, else Pictures › Tethered.
    fn tether_folder(&self) -> PathBuf {
        self.settings.cull.tether_folder.clone().unwrap_or_else(|| {
            specular_platform::integration::user_pictures_dir().join(t!("tether-default-folder"))
        })
    }

    /// The panel's actions. True when `a` was one.
    pub(super) fn tether_action(&mut self, a: Action) -> bool {
        match a {
            Action::Tether => {
                self.tether.open = !self.tether.open;
                if self.tether.open {
                    #[cfg(debug_assertions)]
                    if self.tether.link.is_none()
                        && let Some(dir) = std::env::var_os("SPECULAR_TETHER_MOCK")
                    {
                        self.tether_connect_mock(PathBuf::from(dir));
                    }
                    if self.tether.link.is_none() {
                        self.tether_look_again();
                        // One camera plugged in: connect to it.
                        if self.tether.found.len() == 1 {
                            self.tether_connect();
                        }
                    }
                    self.chrome.announce_status(t!("tether-title"));
                }
            }
            Action::TetherCapture => self.tether_capture(),
            _ => return false,
        }
        self.redraw = true;
        true
    }

    fn tether_look_again(&mut self) {
        let t = &mut self.tether;
        let keep = t.found.get(t.pick).map(|f| f.key.clone());
        t.found = match usb::cameras() {
            Ok(f) => f,
            Err(e) => {
                t.conn = Conn::Problem(e.into());
                Vec::new()
            }
        };
        t.pick = keep
            .and_then(|k| t.found.iter().position(|f| f.key == k))
            .unwrap_or(0);
        if matches!(t.conn, Conn::Problem(_)) && !t.found.is_empty() {
            t.conn = Conn::Idle;
        }
        debug!(target: IO, "tether: {} cameras", t.found.len());
    }

    fn tether_connect(&mut self) {
        let Some(f) = self.tether.found.get(self.tether.pick).cloned() else {
            self.tether_look_again();
            return;
        };
        let pattern = RenamePattern::parse(&self.settings.cull.tether_rename)
            .ok()
            .flatten();
        let dir = self.tether_folder();
        let wake = self.loader.waker();
        self.tether.link = None;
        self.tether.conn = Conn::Connecting;
        self.tether.status = None;
        self.tether.link = Some(start(
            move || usb::open(&f).map_err(Problem::from),
            dir,
            pattern,
            wake,
        ));
    }

    /// Debug builds: connects the in-memory camera (`specular_ptp::mock`), whose shots are
    /// the files of `dir` in name order, one per shot (the real-use runs, `tools/realuse/`).
    #[cfg(debug_assertions)]
    fn tether_connect_mock(&mut self, dir: PathBuf) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|r| {
                r.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.is_file())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| !n.starts_with('.'))
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        let cam = specular_ptp::mock::MockCamera::new();
        {
            let mut s = cam.state();
            for f in files {
                if let (Some(n), Ok(b)) = (f.file_name(), std::fs::read(&f)) {
                    s.feed.push_back((n.to_string_lossy().into_owned(), b));
                }
            }
            info!(target: IO, "tether: mock camera with {} shots from {}", s.feed.len(), dir.display());
        }
        let pattern = RenamePattern::parse(&self.settings.cull.tether_rename)
            .ok()
            .flatten();
        let folder = self.tether_folder();
        let wake = self.loader.waker();
        let link_cam = cam.handle();
        self.tether.mock = Some((dir, cam));
        self.tether.conn = Conn::Connecting;
        self.tether.status = None;
        self.tether.link = Some(start(move || Ok(link_cam), folder, pattern, wake));
    }

    /// Debug builds: the mock camera's faults, asked for by files in its folder.
    #[cfg(debug_assertions)]
    fn tether_mock_faults(&mut self) {
        let Some((dir, cam)) = &self.tether.mock else {
            return;
        };
        let mut s = cam.state();
        if dir.join(".card-full").exists() {
            for (_, st) in s.storages.iter_mut() {
                st.free_images = 0;
            }
        }
        if dir.join(".unplug").exists() {
            s.unplugged = true;
        }
    }

    fn tether_capture(&mut self) {
        match (&self.tether.link, &self.tether.conn) {
            (Some(l), Conn::Connected { capture: true, .. }) => {
                let _ = l.tx.send(Cmd::Capture);
            }
            (Some(_), Conn::Connected { capture: false, .. }) => {
                let now = self.now();
                self.chrome.toast(t!("tether-no-remote"), None, now);
            }
            _ => {
                let now = self.now();
                self.chrome.toast(t!("tether-not-connected"), None, now);
            }
        }
    }

    fn tether_step(&mut self, code: u16, up: bool) {
        let Some(f) = self.tether.facts.as_mut() else {
            return;
        };
        let Some(d) = f.props.iter_mut().find(|d| d.code == code) else {
            return;
        };
        let Some(v) = d.step(if up { 1 } else { -1 }) else {
            return;
        };
        if v == d.current {
            return;
        }
        // Shown at once; the camera's answer follows.
        d.current = v.clone();
        if let Some(l) = &self.tether.link {
            let _ = l.tx.send(Cmd::Set {
                code,
                data_type: d.data_type,
                value: v,
            });
        }
    }

    /// From `about_to_wait`: what the session sent.
    pub(super) fn tether_tick(&mut self) {
        #[cfg(debug_assertions)]
        self.tether_mock_faults();
        let Some(l) = &self.tether.link else {
            return;
        };
        let msgs: Vec<Msg> = l.rx.try_iter().collect();
        if msgs.is_empty() {
            return;
        }
        for m in msgs {
            match m {
                Msg::Connected { name, capture } => {
                    self.tether.conn = Conn::Connected {
                        name: name.clone(),
                        capture,
                    };
                    self.chrome
                        .announce_status(t!("tether-connected", camera = name));
                    // In cull mode on the folder: follow the shots.
                    let dir = self.tether_folder();
                    if self.cull_dir() == Some(dir.as_path()) {
                        self.cull_follow_on();
                    }
                }
                Msg::Facts(f) => self.tether.facts = Some(f),
                Msg::Progress { done, total } => {
                    self.tether.status = (total > 0 && done < total).then(|| {
                        t!(
                            "tether-downloading",
                            percent = (done * 100 / total.max(1)) as i64
                        )
                    });
                }
                Msg::Saved(p) => {
                    self.tether.saved += 1;
                    self.tether.status = Some(t!(
                        "tether-saved",
                        name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    ));
                    self.tether_arrived(p);
                }
                Msg::Failed(s) => {
                    let now = self.now();
                    self.chrome.toast(s.clone(), None, now);
                    self.tether.status = Some(s);
                }
                Msg::Problem(p) => {
                    self.tether.conn = Conn::Problem(p);
                    self.tether.link = None;
                    self.tether.open = true;
                }
                Msg::Gone => {
                    let now = self.now();
                    self.chrome.toast(t!("tether-gone"), None, now);
                    self.tether.conn = Conn::Idle;
                    self.tether.facts = None;
                    // The last line was the camera's (a full card, a download): gone with it.
                    self.tether.status = None;
                    self.tether.link = None;
                    self.tether_look_again();
                }
            }
            if self.tether.link.is_none() {
                break;
            }
        }
        self.redraw = true;
    }

    /// A shot was saved: it joins the cull run on its folder, or the viewer shows it.
    fn tether_arrived(&mut self, p: PathBuf) {
        if self.events_listening("captured") {
            let camera = match &self.tether.conn {
                Conn::Connected { name, .. } => name.clone(),
                _ => String::new(),
            };
            self.emit(super::events::captured(&p, &camera));
        }
        let dir = p.parent().map(Path::to_path_buf);
        if dir.is_some() && self.cull_dir() == dir.as_deref() {
            self.cull_arrived(vec![p]);
            return;
        }
        self.develop_next_capture(std::slice::from_ref(&p));
        let same_folder = !self.gallery_active()
            && self
                .current
                .and_then(|i| self.entries.get(i))
                .is_some_and(|c| c.parent() == dir.as_deref());
        if same_folder {
            let mut e = self.entries.clone();
            if !e.contains(&p) {
                e.push(p.clone());
            }
            self.set_entries(e);
            if let Some(i) = self.index_of(&p) {
                self.go_to(i);
            }
        } else {
            self.open_target(p);
        }
    }

    /// The panel to draw, when it is open.
    pub(super) fn tether_panel(&self) -> Option<Panel> {
        if !self.tether.open {
            return None;
        }
        let folder = self.tether_folder();
        Some(Panel {
            title: t!("tether-title"),
            rows: rows(
                &self.tether,
                &folder,
                &self.settings.cull.tether_rename,
                self.develop_next_capture_label(),
            ),
        })
    }

    fn tether_hit(&mut self, h: Hit) {
        match h {
            Hit::Close => self.tether.open = false,
            Hit::NextCamera => {
                if !self.tether.found.is_empty() {
                    self.tether.pick = (self.tether.pick + 1) % self.tether.found.len();
                }
            }
            Hit::LookAgain => {
                self.tether.conn = Conn::Idle;
                self.tether_look_again();
            }
            Hit::Connect => {
                self.tether_look_again();
                self.tether_connect();
            }
            Hit::Disconnect => {
                self.tether.link = None;
                self.tether.conn = Conn::Idle;
                self.tether.facts = None;
                self.tether.status = None;
            }
            Hit::Capture => self.tether_capture(),
            Hit::Step(code, up) => self.tether_step(code, up),
            Hit::Folder => self.choose_setting_path("cull.tether_folder", true),
            Hit::Arrival => {
                self.develop_action(Action::DevelopNextCapture);
            }
            Hit::CopyRule => {
                if let Conn::Problem(Problem::Permission { rule, .. }) = &self.tether.conn {
                    self.clipboard
                        .copy_text(&self.loader, rule.clone(), Copied::Value);
                }
            }
        }
        self.redraw = true;
    }

    /// A press at logical `lp`: a control acts, the rest of the panel takes the press.
    pub(super) fn tether_press(&mut self, lp: (f32, f32)) -> bool {
        let Some((panel, hits)) = &self.tether.drawn else {
            return false;
        };
        if !panel.contains(lp) {
            return false;
        }
        if let Some(&(_, h)) = hits.iter().find(|(r, _)| r.contains(lp)) {
            self.tether_hit(h);
        }
        true
    }

    /// A screen reader's click on a control; true when the node is the panel's.
    pub(super) fn tether_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let id = req.target_node.0;
        if !(A11Y_ID..A11Y_ID + 0x1000).contains(&id) {
            return false;
        }
        if id > A11Y_ID
            && req.action == accesskit::Action::Click
            && let Some(&(_, h)) = self
                .tether
                .drawn
                .as_ref()
                .and_then(|(_, hits)| hits.get((id - A11Y_ID - 1) as usize))
        {
            self.tether_hit(h);
        }
        true
    }
}

/// Draws the panel at the top end of `area`; returns it and its controls.
pub(super) fn draw_panel(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &Panel,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> (Rect, Vec<(Rect, Hit)>) {
    let (ax, ay, aw, _) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let bold = st.weight(600.0);
    let w = (PANEL_W * ts).min(aw - 2.0 * m).max(160.0);
    let inner = w - 2.0 * PAD;
    let btn_h = 24.0 * ts;
    let gap = 6.0 * ts;
    // Measure first: the height follows the rows.
    let title = text.layout(&p.title, bold, Some(inner - btn_h), Align::Start);
    let mut h = PAD + title.height.max(btn_h) + gap;
    let mut blocks = Vec::with_capacity(p.rows.len());
    for r in &p.rows {
        let (b, rh) = match r {
            Row::Text { text: s, .. } | Row::Code(s) => {
                let b = text.layout(s, st, Some(inner), Align::Start);
                let hh = b.height;
                (Some(b), hh)
            }
            Row::Value { label, value, .. } => {
                let b = text.layout(
                    &specular_i18n::dot_join(&[label.as_str(), value.as_str()]),
                    st,
                    Some(inner - 2.0 * (btn_h + gap)),
                    Align::Start,
                );
                let hh = b.height.max(btn_h);
                (Some(b), hh)
            }
            Row::Buttons(_) => (None, btn_h),
        };
        h += rh + gap;
        blocks.push(b);
    }
    h += PAD - gap;
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let panel = Rect::new(x, ay + m, w, h);
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let mut hits = Vec::new();
    // A rect in reading order (mirrored for right-to-left).
    let flip = |r: Rect| -> Rect {
        if rtl {
            Rect::new(panel.x + panel.right() - r.right(), r.y, r.w, r.h)
        } else {
            r
        }
    };
    let button = |dl: &mut DrawList,
                  text: &mut Text,
                  hits: &mut Vec<(Rect, Hit)>,
                  label: &str,
                  r: Rect,
                  hit: Hit,
                  primary: bool,
                  enabled: bool| {
        let fill = if !enabled {
            pal.surface_pressed.with_alpha(0.5)
        } else if primary {
            pal.accent_fill
        } else {
            pal.surface_hover
        };
        dl.rect(r, RADIUS_SM, fill);
        let ink = if !enabled {
            pal.text_faint
        } else if primary {
            pal.accent_text
        } else {
            pal.text
        };
        let b = text.layout(label, st, Some(r.w), Align::Start);
        text.draw(
            dl,
            &b,
            r.x + (r.w - b.width) * 0.5,
            r.y + (r.h - b.height) * 0.5,
            ink,
        );
        if enabled {
            hits.push((r, hit));
        }
    };
    let mut y = panel.y + PAD;
    let tr = flip(Rect::new(panel.x + PAD, y, inner - btn_h, title.height));
    text.draw(dl, &title, tr.x, y, pal.text);
    let close = flip(Rect::new(
        panel.right() - PAD - btn_h,
        y - 2.0 * ts,
        btn_h,
        btn_h,
    ));
    button(dl, text, &mut hits, "×", close, Hit::Close, false, true);
    y += title.height.max(btn_h) + gap;
    for (r, b) in p.rows.iter().zip(blocks) {
        match (r, b) {
            (Row::Text { muted, .. }, Some(b)) => {
                let rr = flip(Rect::new(panel.x + PAD, y, b.width.min(inner), b.height));
                let ink = if *muted { pal.text_muted } else { pal.text };
                text.draw(dl, &b, rr.x, y, ink);
                y += b.height + gap;
            }
            (Row::Code(_), Some(b)) => {
                let bg = Rect::new(
                    panel.x + PAD / 2.0,
                    y - 2.0 * ts,
                    w - PAD,
                    b.height + 4.0 * ts,
                );
                dl.rect(bg, RADIUS_SM, pal.surface_pressed);
                let rr = flip(Rect::new(panel.x + PAD, y, b.width.min(inner), b.height));
                text.draw(dl, &b, rr.x, y, pal.text);
                y += b.height + gap;
            }
            (Row::Value { step, .. }, Some(b)) => {
                let rh = b.height.max(btn_h);
                let rr = flip(Rect::new(panel.x + PAD, y, b.width.min(inner), b.height));
                text.draw(dl, &b, rr.x, y + (rh - b.height) * 0.5, pal.text);
                if let Some(code) = step {
                    let plus = flip(Rect::new(panel.right() - PAD - btn_h, y, btn_h, btn_h));
                    let minus = flip(Rect::new(
                        panel.right() - PAD - 2.0 * btn_h - gap,
                        y,
                        btn_h,
                        btn_h,
                    ));
                    button(
                        dl,
                        text,
                        &mut hits,
                        "−",
                        minus,
                        Hit::Step(*code, false),
                        false,
                        true,
                    );
                    button(
                        dl,
                        text,
                        &mut hits,
                        "+",
                        plus,
                        Hit::Step(*code, true),
                        false,
                        true,
                    );
                }
                y += rh + gap;
            }
            (Row::Buttons(bs), _) => {
                let mut bx = panel.x + PAD;
                for (label, hit, primary, enabled) in bs {
                    let bw = (text.measure(label, st) + 16.0 * ts).min(inner);
                    if bx + bw > panel.right() - PAD + 0.5 {
                        break;
                    }
                    let r = flip(Rect::new(bx, y, bw, btn_h));
                    button(dl, text, &mut hits, label, r, *hit, *primary, *enabled);
                    bx += bw + gap;
                }
                y += btn_h + gap;
            }
            _ => {}
        }
    }
    (panel, hits)
}

/// The AccessKit nodes: the panel as a group, its lines as labels and its controls as
/// buttons.
pub(super) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    p: &Panel,
    panel: Rect,
    hits: &[(Rect, Hit)],
    sf: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::Group);
    g.set_label(p.title.clone());
    g.set_bounds(bounds(panel));
    let lines: Vec<String> = p
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Text { text, .. } | Row::Code(text) => Some(text.clone()),
            Row::Value { label, value, .. } => Some(format!("{label}: {value}")),
            Row::Buttons(_) => None,
        })
        .collect();
    g.set_description(lines.join("\n"));
    let label_of = |h: Hit| -> String {
        for r in &p.rows {
            match r {
                Row::Buttons(bs) => {
                    if let Some(b) = bs.iter().find(|b| b.1 == h) {
                        return b.0.clone();
                    }
                }
                Row::Value {
                    label,
                    value,
                    step: Some(c),
                } => {
                    if let Hit::Step(code, up) = h
                        && code == *c
                    {
                        let (setting, value) = (label.clone(), value.clone());
                        return if up {
                            t!("tether-step-up", setting = setting, value = value)
                        } else {
                            t!("tether-step-down", setting = setting, value = value)
                        };
                    }
                }
                _ => {}
            }
        }
        if h == Hit::Close {
            t!("tether-close")
        } else {
            String::new()
        }
    };
    let mut children = Vec::new();
    for (i, (r, h)) in hits.iter().enumerate() {
        let id = NodeId(A11Y_ID + 1 + i as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(label_of(*h));
        n.set_bounds(bounds(*r));
        n.add_action(accesskit::Action::Click);
        g.push_child(id);
        children.push((id, n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), g));
    tree.nodes.extend(children);
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_ptp::mock::MockCamera;

    fn wake() -> Wake {
        std::sync::Arc::new(|| {})
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("tether-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn wait_for(rx: &Receiver<Msg>, mut want: impl FnMut(&Msg) -> bool) -> Vec<Msg> {
        let mut seen = Vec::new();
        let until = Instant::now() + Duration::from_secs(20);
        while Instant::now() < until {
            if let Ok(m) = rx.recv_timeout(Duration::from_millis(50)) {
                let done = want(&m);
                seen.push(m);
                if done {
                    return seen;
                }
            }
        }
        panic!("timed out");
    }

    #[test]
    fn session_captures_and_names_with_the_pattern() {
        let dir = scratch("capture");
        let m = MockCamera::new();
        let h = m.handle();
        let pattern = RenamePattern::parse("{date:%Y%m%d}-{n:03}").unwrap();
        let link = start(move || Ok(m), dir.clone(), pattern, wake());
        let seen = wait_for(&link.rx, |m| matches!(m, Msg::Facts(_)));
        assert!(seen.iter().any(|m| matches!(
            m,
            Msg::Connected { name, capture: true } if name == "Mock M-1"
        )));
        let Some(Msg::Facts(f)) = seen.last() else {
            unreachable!()
        };
        assert_eq!(f.battery, Some(80));
        assert_eq!(f.props.len(), 6);
        link.tx.send(Cmd::Capture).unwrap();
        let mut saved = Vec::new();
        wait_for(&link.rx, |m| {
            if let Msg::Saved(p) = m {
                saved.push(p.file_name().unwrap().to_string_lossy().into_owned());
            }
            saved.len() == 2
        });
        // A RAW and its JPEG share the shot's number.
        assert_eq!(saved, ["20241103-001.RAW", "20241103-001.JPG"]);
        // The camera's own button: the next number.
        h.press_shutter();
        let mut more = Vec::new();
        wait_for(&link.rx, |m| {
            if let Msg::Saved(p) = m {
                more.push(p.file_name().unwrap().to_string_lossy().into_owned());
            }
            more.len() == 2
        });
        assert_eq!(more, ["20241103-002.RAW", "20241103-002.JPG"]);
        drop(link);
    }

    #[test]
    fn session_reports_refusals_and_the_camera_going() {
        let dir = scratch("errors");
        let m = MockCamera::new();
        let h = m.handle();
        let link = start(move || Ok(m), dir, None, wake());
        wait_for(&link.rx, |m| matches!(m, Msg::Facts(_)));
        h.state().busy_captures = 1;
        link.tx.send(Cmd::Capture).unwrap();
        wait_for(
            &link.rx,
            |m| matches!(m, Msg::Failed(s) if *s == t!("tether-busy")),
        );
        link.tx
            .send(Cmd::Set {
                code: prop::EXPOSURE_INDEX,
                data_type: specular_ptp::codes::ty::UINT16,
                value: Value::UInt(123),
            })
            .unwrap();
        wait_for(
            &link.rx,
            |m| matches!(m, Msg::Failed(s) if *s == t!("tether-set-refused")),
        );
        link.tx
            .send(Cmd::Set {
                code: prop::EXPOSURE_INDEX,
                data_type: specular_ptp::codes::ty::UINT16,
                value: Value::UInt(800),
            })
            .unwrap();
        wait_for(
            &link.rx,
            |m| matches!(m, Msg::Facts(f) if f.props.iter().any(|d| d.code == prop::EXPOSURE_INDEX && d.current == Value::UInt(800))),
        );
        h.state().unplugged = true;
        wait_for(&link.rx, |m| matches!(m, Msg::Gone));
    }

    #[test]
    fn card_full_answer_reads_the_card_again() {
        let dir = scratch("card-full");
        let m = MockCamera::new();
        let h = m.handle();
        let link = start(move || Ok(m), dir, None, wake());
        let seen = wait_for(&link.rx, |m| matches!(m, Msg::Facts(_)));
        let Some(Msg::Facts(f)) = seen.last() else {
            unreachable!()
        };
        assert!(f.storages.iter().all(|s| s.free_images > 0));
        // The card fills behind the panel's back (no event); the next capture is refused.
        for (_, st) in h.state().storages.iter_mut() {
            st.free_images = 0;
        }
        link.tx.send(Cmd::Capture).unwrap();
        let seen = wait_for(
            &link.rx,
            |m| matches!(m, Msg::Facts(f) if f.storages.iter().all(|s| s.free_images == 0)),
        );
        // Said first, then the card's facts follow at once (not after FACTS_EVERY).
        assert!(
            seen.iter()
                .any(|m| matches!(m, Msg::Failed(s) if *s == t!("tether-card-full")))
        );
    }

    #[test]
    fn open_problems_reach_the_panel() {
        let link = start::<MockCamera>(
            || {
                Err(Problem::Permission {
                    node: Some("/dev/bus/usb/001/007".into()),
                    rule: usb::udev_rule(0x04a9, 0x32b4),
                })
            },
            scratch("perm"),
            None,
            wake(),
        );
        let seen = wait_for(&link.rx, |m| matches!(m, Msg::Problem(_)));
        let Some(Msg::Problem(p)) = seen.last() else {
            unreachable!()
        };
        let t = Tether {
            conn: Conn::Problem(p.clone()),
            ..Tether::default()
        };
        let r = rows(&t, Path::new("/x"), "", "None".into());
        assert!(r.contains(&Row::Code(usb::udev_rule(0x04a9, 0x32b4))));
        assert!(
            r.iter()
                .any(|r| matches!(r, Row::Buttons(b) if b.iter().any(|b| b.1 == Hit::CopyRule)))
        );
        let busy = Tether {
            conn: Conn::Problem(Problem::Busy {
                holders: vec!["gvfs-gphoto2-vo".into()],
            }),
            ..Tether::default()
        };
        let r = rows(&busy, Path::new("/x"), "", "None".into());
        assert!(
            r.iter()
                .any(|r| matches!(r, Row::Text { text, .. } if text.contains("gvfs-gphoto2-vo")))
        );
    }

    #[test]
    fn connected_rows_list_settings_and_capture() {
        let t = Tether {
            conn: Conn::Connected {
                name: "Mock M-1".into(),
                capture: false,
            },
            facts: Some(CameraFacts {
                battery: Some(55),
                storages: vec![StorageInfo {
                    volume_label: "CARD".into(),
                    free_bytes: 2_000_000_000,
                    free_images: 120,
                    ..StorageInfo::default()
                }],
                props: vec![PropDesc {
                    code: prop::F_NUMBER,
                    data_type: specular_ptp::codes::ty::UINT16,
                    writable: true,
                    default: Value::UInt(280),
                    current: Value::UInt(560),
                    form: specular_ptp::Form::Enum(vec![Value::UInt(280), Value::UInt(560)]),
                }],
            }),
            ..Tether::default()
        };
        let r = rows(&t, Path::new("/shots"), "{name}", "None".into());
        assert!(r.contains(&Row::Value {
            label: t!("tether-prop-aperture"),
            value: "f/5.6".into(),
            step: Some(prop::F_NUMBER),
        }));
        // No remote capture: said, and the button is off.
        assert!(
            r.iter()
                .any(|r| matches!(r, Row::Text { text, .. } if *text == t!("tether-no-remote")))
        );
        assert!(r.iter().any(|r| matches!(r, Row::Buttons(b) if b.contains(&(t!("tether-capture"), Hit::Capture, true, false)))));
        assert_eq!(
            value_text(prop::EXPOSURE_INDEX, &Value::UInt(0xFFFF)),
            t!("tether-auto")
        );
    }
}

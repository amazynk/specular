//! One file playing on one renderer, driven from a worker: the app sends [`Cmd`]s and reads
//! [`Event`]s; the worker serves the file, sets it on the renderer, plays it from where the
//! app was, follows its state about once a second, and stops it (and the server) at the end.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::accept;
use crate::didl::{self, Class};
use crate::find::Wake;
use crate::renderer::{Renderer, TransportState};
use crate::serve::{FileServer, local_ip_towards};
use crate::{Error, Result};

/// How often the renderer's state is asked for.
pub const POLL: Duration = Duration::from_secs(1);
/// Failed polls in a row before the device counts as gone.
const GONE_AFTER: u32 = 4;
/// How long the renderer may take to start playing before the first seek.
const START_WAIT: Duration = Duration::from_secs(10);

/// What to play.
#[derive(Clone, Debug)]
pub struct Media {
    pub path: PathBuf,
    pub mime: String,
    /// The title the renderer may show (the file's name).
    pub title: String,
    /// Where to start (seconds): where the app's player was.
    pub start_at: f64,
    pub duration: Option<f64>,
    /// Play at once (else it starts paused at `start_at`).
    pub play: bool,
    /// The speed to play at (1: as recorded).
    pub speed: f64,
    /// Subtitles as SubRip, sent when the renderer takes them.
    pub subtitles: Option<PathBuf>,
}

/// What the app asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Play,
    Pause,
    /// To this many seconds from the start.
    Seek(f64),
    /// 0–100.
    Volume(u8),
    Mute(bool),
    /// Play at this speed (1: as recorded), when the renderer plays at it.
    Speed(f64),
    /// Other subtitles (a new delay, another track; `None`: none): served anew and the file
    /// loaded again where it is, as it was (playing or paused), so the device reads them.
    Subtitles(Option<PathBuf>),
    /// Stop the renderer and end the session.
    Stop,
}

/// Where the renderer is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    pub state: TransportState,
    pub position: Option<f64>,
    pub duration: Option<f64>,
}

/// What the worker reports.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The renderer took the file and plays it; its volume (0–100) when it says.
    Started {
        volume: Option<u8>,
        /// The subtitles went with the file.
        subtitles: bool,
    },
    /// The speed it plays at now (1 when it did not take the one asked for).
    Speed(f64),
    /// It plays only at normal speed (said once per session).
    NormalSpeedOnly,
    /// New subtitles were sent (true), or the device takes none (false).
    Subtitles(bool),
    Status(Status),
    /// A command the renderer refused (the session goes on).
    Refused(Cmd, Error),
    /// The session could not start, or the device went away: it is over.
    Failed(Error),
    /// Stopped as asked: it is over.
    Stopped,
}

/// A session running.
pub struct Session {
    tx: mpsc::Sender<Cmd>,
    rx: mpsc::Receiver<Event>,
}

impl Session {
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The worker stops the renderer and its server on its own; the app does not wait.
        let _ = self.tx.send(Cmd::Stop);
    }
}

/// Starts playing `media` on `renderer`.
pub fn start(renderer: Renderer, media: Media, wake: Wake) -> Session {
    let (tx, cmd_rx) = mpsc::channel();
    let (ev_tx, rx) = mpsc::channel();
    specular_thread::spawn("net-cast-play", move || {
        let send = |e: Event| {
            let _ = ev_tx.send(e);
            wake();
        };
        match begin(&renderer, &media) {
            Ok((server, volume, subtitles, speeds)) => {
                send(Event::Started { volume, subtitles });
                let mut speed = Speed {
                    list: speeds,
                    now: 1.0,
                    said: false,
                };
                if (media.speed - 1.0).abs() > 1e-3 {
                    speed.set(&renderer, media.speed, media.play, &send);
                }
                let mut served = Served { server, media };
                let end = run(&renderer, &cmd_rx, &mut speed, &mut served, &send);
                // The file stops being served before the app hears it is over.
                drop(served);
                send(end);
            }
            Err(e) => send(Event::Failed(e)),
        }
    });
    Session { tx, rx }
}

/// The speeds a renderer plays at, and the one it plays at now.
struct Speed {
    list: Vec<String>,
    now: f64,
    said: bool,
}

impl Speed {
    /// Asks for `want` (playing: at once; paused: from the next play). A speed the device
    /// does not list leaves it at 1×, said once.
    fn set(&mut self, r: &Renderer, want: f64, playing: bool, send: &impl Fn(Event)) {
        let Some(s) = crate::renderer::speed_for(&self.list, want) else {
            if !self.said {
                self.said = true;
                send(Event::NormalSpeedOnly);
            }
            if (self.now - 1.0).abs() > 1e-3 {
                self.now = 1.0;
                if playing && r.play().is_ok() {
                    send(Event::Speed(1.0));
                }
            }
            return;
        };
        if playing {
            match r.play_at(&s) {
                Ok(()) => self.now = want,
                Err(e) => {
                    send(Event::Refused(Cmd::Speed(want), e));
                    return;
                }
            }
        } else {
            self.now = want;
        }
        send(Event::Speed(self.now));
    }

    /// Play, at the speed set.
    fn play(&self, r: &Renderer) -> Result<()> {
        match crate::renderer::speed_for(&self.list, self.now) {
            Some(s) if (self.now - 1.0).abs() > 1e-3 => r.play_at(&s),
            _ => r.play(),
        }
    }
}

/// Checks the type, serves the file (and its subtitles, when the renderer takes them), sets
/// it and plays it from `start_at`. The server, the volume, whether the subtitles went, and
/// the speeds the renderer plays at.
fn begin(r: &Renderer, m: &Media) -> Result<(FileServer, Option<u8>, bool, Vec<String>)> {
    let (server, srt) = load(r, m, m.start_at, m.play)?;
    let speeds = r.speeds();
    Ok((server, r.volume().ok(), srt, speeds))
}

/// The file served now and what it is.
struct Served {
    server: FileServer,
    media: Media,
}

/// Serves `m` (and its subtitles, when the renderer takes them), sets it on the renderer and
/// plays it from `start_at` (seconds), paused unless `play`. The server, and whether the
/// subtitles went.
fn load(r: &Renderer, m: &Media, start_at: f64, play: bool) -> Result<(FileServer, bool)> {
    let sink = r.sink()?;
    let mime = accept::accepts(&sink, &m.mime).ok_or_else(|| Error::NotAccepted(m.mime.clone()))?;
    let device_ip = crate::http::Url::parse(&r.device.av_transport.control)
        .ok_or(Error::BadReply("control URL"))?
        .ip;
    let ip = local_ip_towards(device_ip)?;
    let subtitles = m
        .subtitles
        .as_deref()
        .filter(|_| r.device.sec_captions || accept::takes_subtitles(&sink));
    let mut files: Vec<(&std::path::Path, &str)> = vec![(&m.path, &mime)];
    if let Some(s) = subtitles {
        files.push((s, "text/srt"));
    }
    // Only the renderer is answered (and the random path is needed too).
    let server = FileServer::start_for(ip, Some(device_ip), &files).map_err(|e| {
        if specular_core::net::off::is_refusal(&e) {
            Error::NetworkOff
        } else {
            Error::File(e.to_string())
        }
    })?;
    let url = server.url();
    let size = std::fs::metadata(&m.path).ok().map(|md| md.len());
    let srt = server.url_of(1);
    let meta = didl::item_with_subtitles(
        &m.title,
        Class::of(&mime),
        &url,
        &accept::protocol_info(&mime),
        size,
        m.duration,
        srt.as_deref(),
    );
    r.set_uri(&url, &meta)?;
    r.play()?;
    if start_at > 1.0 {
        // Most renderers only seek once they play.
        let t0 = Instant::now();
        while t0.elapsed() < START_WAIT {
            match r.transport_state() {
                Ok(TransportState::Playing | TransportState::Paused) => break,
                Ok(_) => std::thread::sleep(Duration::from_millis(250)),
                Err(e) => return Err(e),
            }
        }
        if let Err(e) = r.seek(start_at) {
            specular_log::debug!(target: specular_log::IO, "cast: first seek: {e}");
        }
    }
    if !play {
        r.pause()?;
    }
    Ok((server, srt.is_some()))
}

/// New subtitles: the file is served again with them (a new address, so the device reads
/// them afresh) and loaded where it is, as it was. Whether they went.
fn reload(
    r: &Renderer,
    served: &mut Served,
    subtitles: Option<PathBuf>,
    speed: &Speed,
    last: Option<Status>,
) -> Result<bool> {
    let at = r
        .position()
        .ok()
        .and_then(|p| p.position)
        .or(last.and_then(|l| l.position))
        .unwrap_or(0.0);
    let playing = last.is_none_or(|l| l.state == TransportState::Playing);
    let mut m = served.media.clone();
    m.subtitles = subtitles;
    let (server, sent) = load(r, &m, at, playing)?;
    if playing && (speed.now - 1.0).abs() > 1e-3 {
        let _ = speed.play(r);
    }
    // The old address goes once the device has the new one.
    served.server = server;
    served.media = m;
    Ok(sent)
}

/// Follows the renderer and carries out commands until asked to stop or the device goes;
/// the session's last event.
fn run(
    r: &Renderer,
    rx: &mpsc::Receiver<Cmd>,
    speed: &mut Speed,
    served: &mut Served,
    send: &impl Fn(Event),
) -> Event {
    let mut last: Option<Status> = None;
    let mut fails = 0;
    loop {
        match rx.recv_timeout(POLL) {
            Ok(Cmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = r.stop();
                return Event::Stopped;
            }
            Ok(c) => {
                let done = match &c {
                    Cmd::Play => speed.play(r),
                    Cmd::Speed(s) => {
                        let playing = last.is_some_and(|l| l.state == TransportState::Playing);
                        speed.set(r, *s, playing, send);
                        Ok(())
                    }
                    Cmd::Pause => r.pause(),
                    Cmd::Seek(s) => r.seek(*s),
                    Cmd::Volume(v) => r.set_volume(*v),
                    Cmd::Mute(m) => r.set_mute(*m),
                    Cmd::Subtitles(p) => reload(r, served, p.clone(), speed, last).map(|sent| {
                        send(Event::Subtitles(sent));
                    }),
                    Cmd::Stop => Ok(()),
                };
                if let Err(e) = done {
                    send(Event::Refused(c, e));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        match r.position() {
            Ok(p) => {
                fails = 0;
                let s = Status {
                    state: p.state,
                    position: p.position,
                    duration: p.duration,
                };
                if last != Some(s) {
                    last = Some(s);
                    send(Event::Status(s));
                }
            }
            Err(e) => {
                fails += 1;
                if fails >= GONE_AFTER || e == Error::NetworkOff {
                    return Event::Failed(e);
                }
            }
        }
    }
}

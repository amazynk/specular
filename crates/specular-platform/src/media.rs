//! The system's media controls: what plays, and its play, pause, next, previous and seek
//! buttons (keyboard media keys routed by the desktop, the lock screen, panel widgets).
//!
//! - Linux and the BSDs: MPRIS (`org.mpris.MediaPlayer2` and `.Player`) on the session bus,
//!   under `org.mpris.MediaPlayer2.specular.instance<pid>`. Local D-Bus only: nothing leaves
//!   the computer.
//! - Windows: the System Media Transport Controls of a `Windows.Media.Playback.MediaPlayer`
//!   whose own commands are off (safe WinRT; the desktop window needs no interop call).
//! - macOS: not offered. `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` are reached
//!   only through calls the Rust bindings mark unsafe, and Specular adds unsafe code only
//!   where nothing else will do; the window's media keys still work while it has the focus.
//!
//! The controls run on a worker thread; button presses come back as [`MediaEvent`]s.

use std::path::PathBuf;
use std::sync::mpsc;

/// A press on the system's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaEvent {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    /// Relative seek, microseconds (negative: back).
    Seek(i64),
    /// Absolute position, microseconds.
    SetPosition(i64),
    /// Bring the window to the front.
    Raise,
}

/// What is playing, for the controls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NowPlaying {
    /// `None`: nothing (stopped).
    pub playing: Option<bool>,
    pub title: String,
    pub length_us: i64,
    pub position_us: i64,
    /// A picture of it (a PNG file: the frame or cover), for the panel.
    pub art: Option<PathBuf>,
    pub can_next: bool,
    pub can_previous: bool,
}

// macOS has no controls to send to; Windows has no seek signal.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
enum Msg {
    Update(NowPlaying),
    /// A jump the user made (MPRIS `Seeked`).
    Seeked(i64),
}

/// The controls while the viewer runs.
pub struct MediaControls {
    tx: mpsc::Sender<Msg>,
    events: mpsc::Receiver<MediaEvent>,
    last: NowPlaying,
}

impl std::fmt::Debug for MediaControls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MediaControls")
    }
}

impl MediaControls {
    /// The system's controls, when this platform has them and they could be reached.
    /// `wake` is called after each press. `None` with `SPECULAR_NO_MEDIA_CONTROLS` set (tests,
    /// headless runs).
    pub fn start(wake: Box<dyn Fn() + Send>) -> Option<MediaControls> {
        if std::env::var_os("SPECULAR_NO_MEDIA_CONTROLS").is_some() || cfg!(test) {
            return None;
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        return mpris::start(None, wake);
        #[cfg(windows)]
        return smtc::start(wake);
        #[allow(unreachable_code)]
        {
            let _ = wake;
            None
        }
    }

    /// Says what plays now. A jump in the position (more than the time since the last
    /// update would explain) is also said as a seek.
    pub fn update(&mut self, now: NowPlaying) {
        let same_item = now.title == self.last.title && now.playing.is_some();
        let jumped = same_item && (now.position_us - self.last.position_us).abs() > 2_000_000;
        self.last = now.clone();
        let pos = now.position_us;
        let _ = self.tx.send(Msg::Update(now));
        if jumped {
            let _ = self.tx.send(Msg::Seeked(pos));
        }
    }

    /// The presses since the last call.
    pub fn poll(&self) -> Vec<MediaEvent> {
        self.events.try_iter().collect()
    }
}

/// `path` (absolute) as a `file://` URL: `file:///home/a/b%20c.png`, `file:///C:/a/b.png`.
/// Bytes outside the unreserved set and `/` are percent-encoded.
// macOS has no controls to send a picture to.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(crate) fn file_url(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod mpris {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use zbus::object_server::SignalEmitter;
    use zbus::zvariant::{ObjectPath, OwnedValue, Value};

    use super::{MediaControls, MediaEvent, Msg, NowPlaying};

    pub(crate) const PATH: &str = "/org/mpris/MediaPlayer2";

    type Shared = Arc<Mutex<NowPlaying>>;
    type Send = Arc<
        Mutex<(
            std::sync::mpsc::Sender<MediaEvent>,
            Box<dyn Fn() + std::marker::Send>,
        )>,
    >;

    fn fire(s: &Send, e: MediaEvent) {
        if let Ok(g) = s.lock() {
            let _ = g.0.send(e);
            (g.1)();
        }
    }

    pub(crate) struct Root(Send);

    #[zbus::interface(name = "org.mpris.MediaPlayer2")]
    impl Root {
        fn raise(&self) {
            fire(&self.0, MediaEvent::Raise);
        }
        fn quit(&self) {}
        #[zbus(property)]
        fn can_quit(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn can_raise(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn has_track_list(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn identity(&self) -> String {
            "Specular".into()
        }
        #[zbus(property)]
        fn desktop_entry(&self) -> String {
            "app.specular.Specular".into()
        }
        #[zbus(property)]
        fn supported_uri_schemes(&self) -> Vec<String> {
            Vec::new()
        }
        #[zbus(property)]
        fn supported_mime_types(&self) -> Vec<String> {
            Vec::new()
        }
    }

    pub(crate) struct Player {
        now: Shared,
        send: Send,
    }

    /// The track's object path (one track: the movie on screen).
    fn track_id() -> ObjectPath<'static> {
        ObjectPath::from_static_str_unchecked("/app/specular/Specular/track/current")
    }

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Player {
        fn next(&self) {
            fire(&self.send, MediaEvent::Next);
        }
        fn previous(&self) {
            fire(&self.send, MediaEvent::Previous);
        }
        fn pause(&self) {
            fire(&self.send, MediaEvent::Pause);
        }
        fn play_pause(&self) {
            fire(&self.send, MediaEvent::PlayPause);
        }
        fn stop(&self) {
            fire(&self.send, MediaEvent::Stop);
        }
        fn play(&self) {
            fire(&self.send, MediaEvent::Play);
        }
        fn seek(&self, offset: i64) {
            fire(&self.send, MediaEvent::Seek(offset));
        }
        fn set_position(&self, track: ObjectPath<'_>, position: i64) {
            if track == track_id() {
                fire(&self.send, MediaEvent::SetPosition(position));
            }
        }
        fn open_uri(&self, _uri: String) {}

        #[zbus(signal)]
        async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

        #[zbus(property)]
        fn playback_status(&self) -> String {
            match self.now.lock().ok().and_then(|n| n.playing) {
                Some(true) => "Playing",
                Some(false) => "Paused",
                None => "Stopped",
            }
            .into()
        }
        #[zbus(property)]
        fn rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn minimum_rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn maximum_rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn metadata(&self) -> HashMap<String, OwnedValue> {
            let n = self.now.lock().map(|n| n.clone()).unwrap_or_default();
            metadata(&n)
        }
        #[zbus(property)]
        fn volume(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn position(&self) -> i64 {
            self.now.lock().map_or(0, |n| n.position_us)
        }
        #[zbus(property)]
        fn can_go_next(&self) -> bool {
            self.now.lock().is_ok_and(|n| n.can_next)
        }
        #[zbus(property)]
        fn can_go_previous(&self) -> bool {
            self.now.lock().is_ok_and(|n| n.can_previous)
        }
        #[zbus(property)]
        fn can_play(&self) -> bool {
            self.now.lock().is_ok_and(|n| n.playing.is_some())
        }
        #[zbus(property)]
        fn can_pause(&self) -> bool {
            self.can_play()
        }
        #[zbus(property)]
        fn can_seek(&self) -> bool {
            self.now
                .lock()
                .is_ok_and(|n| n.playing.is_some() && n.length_us > 0)
        }
        #[zbus(property)]
        fn can_control(&self) -> bool {
            true
        }
    }

    /// MPRIS metadata of what plays.
    pub(crate) fn metadata(n: &NowPlaying) -> HashMap<String, OwnedValue> {
        let mut m = HashMap::new();
        let mut put = |k: &str, v: Value<'_>| {
            if let Ok(o) = OwnedValue::try_from(v) {
                m.insert(k.to_owned(), o);
            }
        };
        put("mpris:trackid", Value::from(track_id()));
        if n.playing.is_some() {
            put("xesam:title", Value::from(n.title.clone()));
            if n.length_us > 0 {
                put("mpris:length", Value::from(n.length_us));
            }
            if let Some(a) = &n.art {
                put("mpris:artUrl", Value::from(super::file_url(a)));
            }
        }
        m
    }

    /// Serves the controls on the session bus (or the bus at `address`, in tests).
    pub(crate) fn start(
        address: Option<&str>,
        wake: Box<dyn Fn() + std::marker::Send>,
    ) -> Option<MediaControls> {
        let (ev_tx, events) = std::sync::mpsc::channel();
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        let now: Shared = Arc::new(Mutex::new(NowPlaying::default()));
        let send: Send = Arc::new(Mutex::new((ev_tx, wake)));
        let builder = match address {
            Some(a) => zbus::blocking::connection::Builder::address(a).ok()?,
            None => zbus::blocking::connection::Builder::session().ok()?,
        };
        let name = format!(
            "org.mpris.MediaPlayer2.specular.instance{}",
            std::process::id()
        );
        let conn = builder
            .serve_at(PATH, Root(Arc::clone(&send)))
            .ok()?
            .serve_at(
                PATH,
                Player {
                    now: Arc::clone(&now),
                    send,
                },
            )
            .ok()?
            .name(name)
            .ok()?
            .build()
            .map_err(|e| specular_log::debug!(target: specular_log::STARTUP, "media controls: {e}"))
            .ok()?;
        specular_thread::builder("platform-mpris")
            .spawn(move || {
                let Ok(iface) = conn.object_server().interface::<_, Player>(PATH) else {
                    return;
                };
                while let Ok(m) = rx.recv() {
                    match m {
                        Msg::Update(n) => {
                            let changed = now.lock().map(|mut cur| {
                                let c = NowPlaying {
                                    position_us: cur.position_us,
                                    ..n.clone()
                                } != *cur;
                                *cur = n;
                                c
                            });
                            if changed.unwrap_or(false) {
                                let p = iface.get_mut();
                                let e = iface.signal_emitter();
                                let _ = zbus::block_on(p.playback_status_changed(e));
                                let _ = zbus::block_on(p.metadata_changed(e));
                                let _ = zbus::block_on(p.can_play_changed(e));
                                let _ = zbus::block_on(p.can_pause_changed(e));
                                let _ = zbus::block_on(p.can_seek_changed(e));
                                let _ = zbus::block_on(p.can_go_next_changed(e));
                                let _ = zbus::block_on(p.can_go_previous_changed(e));
                            }
                        }
                        Msg::Seeked(pos) => {
                            let _ = zbus::block_on(Player::seeked(iface.signal_emitter(), pos));
                        }
                    }
                }
                drop(conn);
            })
            .ok()?;
        Some(MediaControls {
            tx,
            events,
            last: NowPlaying::default(),
        })
    }
}

#[cfg(windows)]
mod smtc {
    use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
    use windows::Media::Playback::MediaPlayer;
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
        SystemMediaTransportControlsTimelineProperties,
    };
    use windows::Storage::Streams::RandomAccessStreamReference;

    use super::{MediaControls, MediaEvent, Msg};

    pub(super) fn start(wake: Box<dyn Fn() + Send>) -> Option<MediaControls> {
        let (ev_tx, events) = std::sync::mpsc::channel();
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        specular_thread::builder("platform-smtc")
            .spawn(move || {
                let setup =
                    || -> windows::core::Result<(MediaPlayer, SystemMediaTransportControls)> {
                        let player = MediaPlayer::new()?;
                        // The player plays nothing: only its controls are used.
                        player.CommandManager()?.SetIsEnabled(false)?;
                        let c = player.SystemMediaTransportControls()?;
                        c.SetIsEnabled(true)?;
                        c.SetIsPlayEnabled(true)?;
                        c.SetIsPauseEnabled(true)?;
                        c.SetIsStopEnabled(true)?;
                        c.SetIsNextEnabled(true)?;
                        c.SetIsPreviousEnabled(true)?;
                        let tx = ev_tx.clone();
                        let wake = std::sync::Mutex::new(wake);
                        c.ButtonPressed(&TypedEventHandler::<
                            SystemMediaTransportControls,
                            SystemMediaTransportControlsButtonPressedEventArgs,
                        >::new(move |_, args| {
                            if let Some(a) = args.as_ref() {
                                let e = match a.Button()? {
                                    SystemMediaTransportControlsButton::Play => {
                                        Some(MediaEvent::Play)
                                    }
                                    SystemMediaTransportControlsButton::Pause => {
                                        Some(MediaEvent::Pause)
                                    }
                                    SystemMediaTransportControlsButton::Stop => {
                                        Some(MediaEvent::Stop)
                                    }
                                    SystemMediaTransportControlsButton::Next => {
                                        Some(MediaEvent::Next)
                                    }
                                    SystemMediaTransportControlsButton::Previous => {
                                        Some(MediaEvent::Previous)
                                    }
                                    _ => None,
                                };
                                if let Some(e) = e {
                                    let _ = tx.send(e);
                                    if let Ok(w) = wake.lock() {
                                        w();
                                    }
                                }
                            }
                            Ok(())
                        }))?;
                        Ok((player, c))
                    };
                let Ok((_player, c)) = setup() else {
                    let _ = ready_tx.send(false);
                    return;
                };
                let _ = ready_tx.send(true);
                while let Ok(m) = rx.recv() {
                    let Msg::Update(n) = m else { continue };
                    let _ = (|| -> windows::core::Result<()> {
                        c.SetPlaybackStatus(match n.playing {
                            Some(true) => MediaPlaybackStatus::Playing,
                            Some(false) => MediaPlaybackStatus::Paused,
                            None => MediaPlaybackStatus::Stopped,
                        })?;
                        c.SetIsNextEnabled(n.can_next)?;
                        c.SetIsPreviousEnabled(n.can_previous)?;
                        let u = c.DisplayUpdater()?;
                        u.SetType(MediaPlaybackType::Video)?;
                        u.VideoProperties()?.SetTitle(&n.title.as_str().into())?;
                        if let Some(a) = &n.art {
                            let uri = Uri::CreateUri(&super::file_url(a).into())?;
                            u.SetThumbnail(&RandomAccessStreamReference::CreateFromUri(&uri)?)?;
                        }
                        u.Update()?;
                        let t = SystemMediaTransportControlsTimelineProperties::new()?;
                        t.SetStartTime(TimeSpan { Duration: 0 })?;
                        t.SetEndTime(TimeSpan {
                            Duration: n.length_us * 10,
                        })?;
                        t.SetPosition(TimeSpan {
                            Duration: n.position_us * 10,
                        })?;
                        c.UpdateTimelineProperties(&t)?;
                        Ok(())
                    })();
                }
            })
            .ok()?;
        if !ready_rx.recv().unwrap_or(false) {
            return None;
        }
        Some(MediaControls {
            tx,
            events,
            last: Default::default(),
        })
    }
}

#[cfg(test)]
mod tests;

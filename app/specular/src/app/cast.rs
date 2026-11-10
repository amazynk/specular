//! "Play on…": a movie or sound file played on a TV or speaker on the local network (DLNA
//! renderers, `specular_cast`).
//!
//! Devices are looked for only while the player's "Play on…" menu is open (and never with the
//! network off). Choosing one serves the file to it, plays it from where the local player
//! was, and pauses the local player: the transport (play, pause, seek, volume) then drives
//! the device and shows where it is, and a card over the picture says "Playing on <device>"
//! with "Stop casting". Stopping (or "This device") brings the local player back at the
//! device's position, playing if it was.
//!
//! The player's speed goes to the device when it lists that speed (AVTransport
//! `TransportPlaySpeed`); otherwise it stays at 1× and says so once. The chosen text
//! subtitles go as SubRip beside the file to a device that takes them; another track or
//! delay while casting is sent again (the device loads the file again where it is).
//! Picture subtitles (PGS, VobSub, DVB) stay here: DLNA takes text subtitles only, and the
//! "Play on…" menu says so.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use specular_cast::{
    Cmd, Device, Error, Event, Finder, Found, Media, Renderer, Session, Status, TransportState,
};
use specular_core::net::off;
use specular_i18n::t;
use specular_input::Action;
use specular_widgets::{CastView, Playback, PlayerMenu};

use super::App;
use crate::loader::ImageId;

/// How long one search listens for answers.
const SEARCH: Duration = Duration::from_secs(4);
/// A seek shows its target this long, or until the device reports near it.
const SEEK_HOLD: Duration = Duration::from_secs(3);
/// The volume keys' step on a device (its scale is 0–100).
const VOLUME_STEP: u8 = 5;
/// The least time between volumes sent while the slider is dragged.
const VOLUME_SEND: Duration = Duration::from_millis(150);
/// The gallery's "Play on…" opens the menu once the file is on screen, within this.
const MENU_WAIT: Duration = Duration::from_secs(10);

/// What plays where.
pub(super) struct CastState {
    finder: Option<Finder>,
    devices: Vec<Device>,
    /// The menu was open at the last tick (the search runs only while it is).
    menu_open: bool,
    /// Where the search goes: the SSDP group (tests: a fake renderer's responder).
    pub(super) target: SocketAddr,
    active: Option<Active>,
    /// The gallery's "Play on…": open the menu once the file plays here.
    pub(super) menu_wanted: Option<Instant>,
}

impl Default for CastState {
    fn default() -> CastState {
        CastState {
            finder: None,
            devices: Vec::new(),
            menu_open: false,
            target: specular_cast::ssdp::MULTICAST.into(),
            active: None,
            menu_wanted: None,
        }
    }
}

/// The device playing.
struct Active {
    session: Session,
    name: String,
    udn: String,
    movie: ImageId,
    started: bool,
    state: Option<TransportState>,
    /// The device's position when last told, and when.
    pos: f64,
    at: Instant,
    duration: f64,
    playing: bool,
    volume: Option<u8>,
    /// The volume last sent to the device, and when (the slider's drag sends at most every
    /// [`VOLUME_SEND`]).
    volume_sent: Option<(u8, Instant)>,
    muted: bool,
    seek: Option<(f64, Instant)>,
    ended: bool,
    /// The speed asked of the device, and the one it plays at.
    asked_speed: f64,
    speed: f64,
    /// The subtitles sent: which track and the delay they carry.
    subs: SubsKey,
}

/// Which subtitles went to the device: the track (by address) and the delay (ms).
type SubsKey = (Option<usize>, i64);

impl Active {
    /// Where the device is now (advancing between reports while it plays).
    fn position(&self, now: Instant) -> f64 {
        if let Some((t, _)) = self.seek {
            return t;
        }
        let p = if self.playing && self.state == Some(TransportState::Playing) {
            self.pos + now.saturating_duration_since(self.at).as_secs_f64() * self.speed
        } else {
            self.pos
        };
        if self.duration > 0.0 {
            p.min(self.duration)
        } else {
            p
        }
    }

    fn status(&mut self, s: Status, now: Instant) {
        let was = self.state;
        self.state = Some(s.state);
        if let Some(d) = s.duration {
            self.duration = d;
        }
        if let Some(p) = s.position {
            // The seek's target shows until the device is there (or it took too long).
            if let Some((t, since)) = self.seek
                && ((p - t).abs() < 2.0 || now.duration_since(since) > SEEK_HOLD)
            {
                self.seek = None;
            }
            self.pos = p;
            self.at = now;
        }
        match s.state {
            TransportState::Playing => {
                self.playing = true;
                self.ended = false;
            }
            TransportState::Paused => self.playing = false,
            // Stopped after playing, on its own: the end.
            TransportState::Stopped | TransportState::NoMedia
                if was == Some(TransportState::Playing) =>
            {
                self.playing = false;
                self.ended = true;
            }
            _ => {}
        }
    }
}

/// Why a cast failed, in the UI language.
fn reason(e: &Error) -> String {
    match e {
        Error::NetworkOff => t!("cast-error-network-off"),
        Error::Unreachable => t!("cast-error-unreachable"),
        Error::Timeout => t!("cast-error-timeout"),
        Error::Status(c) => t!("cast-error-refused", code = i64::from(*c)),
        Error::Fault(c, _) => t!("cast-error-refused", code = i64::from(*c)),
        Error::File(_) => t!("cast-error-file"),
        Error::BadReply(_) | Error::NotARenderer | Error::NotAccepted(_) => {
            t!("cast-error-broken")
        }
    }
}

impl App {
    /// The movie or sound file playing here, and its file.
    fn cast_media(&self) -> Option<(ImageId, PathBuf)> {
        let a = self.anim.as_ref()?;
        a.movie.as_ref()?;
        if Some(a.id) != self.current_id() {
            return None;
        }
        let path = self
            .images
            .get(&a.id)?
            .session
            .as_ref()?
            .source()
            .path()
            .to_path_buf();
        Some((a.id, path))
    }

    /// Casting the file on screen.
    pub(super) fn casting(&self) -> bool {
        self.cast
            .active
            .as_ref()
            .is_some_and(|a| Some(a.movie) == self.current_id())
    }

    /// Each tick: the search follows the menu, reports come in, and casting ends when the
    /// file on screen changes.
    pub(super) fn cast_tick(&mut self) {
        let open = self.chrome.player_menu() == Some(PlayerMenu::Cast);
        if open != self.cast.menu_open {
            self.cast.menu_open = open;
            if open {
                self.cast_search();
            } else {
                // Closed: the search stops.
                self.cast.finder = None;
            }
            self.redraw = true;
        }
        while let Some(f) = self.cast.finder.as_ref().and_then(Finder::try_recv) {
            match f {
                Found::Device(d) => match self.cast.devices.iter_mut().find(|x| x.udn == d.udn) {
                    Some(x) => *x = *d,
                    None => self.cast.devices.push(*d),
                },
                Found::Done(_) => self.cast.finder = None,
            }
            self.redraw = true;
        }
        self.cast_events();
        self.cast_follow_speed();
        self.cast_follow_subtitles();
        if let Some(a) = &self.cast.active
            && self.cast_media().map(|m| m.0) != Some(a.movie)
        {
            // Another file: casting ends; it plays here.
            self.cast.active = None;
            self.redraw = true;
        }
        if let Some(t) = self.cast.menu_wanted {
            if t.elapsed() > MENU_WAIT {
                self.cast.menu_wanted = None;
            } else if self.cast_media().is_some() && self.chrome.has_player() {
                self.cast.menu_wanted = None;
                if self.chrome.player_menu() != Some(PlayerMenu::Cast) {
                    self.chrome.toggle_player_menu(PlayerMenu::Cast);
                }
                self.redraw = true;
            }
        }
        // While something plays there, its position moves on screen.
        if self
            .cast
            .active
            .as_ref()
            .is_some_and(|a| a.playing && a.seek.is_none())
        {
            self.wake_by(
                Instant::now() + Duration::from_millis(250),
                super::frame_gate::Cause::Timer,
            );
        }
    }

    /// Looks for devices (again), unless the network is off.
    fn cast_search(&mut self) {
        if off::is_off() {
            self.cast.finder = None;
            return;
        }
        let wake = self.loader.waker();
        self.cast.finder = Some(specular_cast::find::find(self.cast.target, SEARCH, wake));
    }

    fn cast_events(&mut self) {
        let now = Instant::now();
        loop {
            let Some(a) = self.cast.active.as_mut() else {
                return;
            };
            let Some(e) = a.session.try_recv() else {
                return;
            };
            self.redraw = true;
            match e {
                Event::Started { volume, .. } => {
                    a.started = true;
                    a.volume = volume;
                    a.at = now;
                }
                Event::Status(s) => a.status(s, now),
                Event::Refused(_, e) => {
                    let msg = t!(
                        "toast-cast-refused",
                        device = a.name.clone(),
                        reason = reason(&e)
                    );
                    let n = self.now();
                    self.chrome.toast(msg, None, n);
                }
                Event::Failed(e) => {
                    let name = a.name.clone();
                    let at = a.position(now);
                    let started = a.started;
                    self.cast.active = None;
                    let msg = match &e {
                        Error::NotAccepted(m) => {
                            t!("toast-cast-not-accepted", device = name, kind = m.clone())
                        }
                        e => t!("toast-cast-failed", device = name, reason = reason(e)),
                    };
                    let n = self.now();
                    self.chrome.toast(msg, None, n);
                    // The local player takes over where the device was, paused.
                    if started {
                        self.cast_local_at(at, false);
                    }
                    return;
                }
                Event::Speed(s) => {
                    a.pos = a.position(now);
                    a.at = now;
                    a.speed = s;
                }
                Event::NormalSpeedOnly => {
                    let msg = t!("toast-cast-normal-speed", device = a.name.clone());
                    let n = self.now();
                    self.chrome.toast(msg, None, n);
                }
                Event::Subtitles(_) | Event::Stopped => {}
            }
        }
    }

    /// Plays the file on screen on device `i` of the list.
    fn cast_to(&mut self, i: usize) {
        let Some(d) = self.cast.devices.get(i).cloned() else {
            return;
        };
        if self.cast.active.as_ref().is_some_and(|a| a.udn == d.udn) {
            return;
        }
        let now = self.now();
        if off::is_off() {
            self.chrome.toast(t!("network-off"), None, now);
            return;
        }
        let Some((id, path)) = self.cast_media() else {
            return;
        };
        // Another device first lets go of it.
        let (start_at, play) = match self.cast.active.take() {
            Some(a) => (a.position(Instant::now()), a.playing),
            None => (
                self.player_time(),
                self.anim.as_ref().is_some_and(|a| a.is_playing()),
            ),
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let Some(mime) = specular_decode::mime::mime_of_extension(&ext).flatten() else {
            let msg = t!(
                "toast-cast-not-accepted",
                device = d.name.clone(),
                kind = format!(".{ext}")
            );
            self.chrome.toast(msg, None, now);
            return;
        };
        let duration = self
            .anim
            .as_ref()
            .and_then(|a| a.playback().video)
            .map_or(0.0, |v| v.duration);
        // The local player pauses: the device plays now.
        if let Some(a) = self.anim.as_mut()
            && a.is_playing()
        {
            a.toggle(Instant::now());
        }
        let speed = self.anim.as_ref().map_or(1.0, crate::anim::Anim::speed);
        let subtitles = self.cast_subtitles(&path);
        let media = Media {
            title: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            mime: mime.to_string(),
            start_at,
            duration: (duration > 0.0).then_some(duration),
            play,
            speed,
            subtitles,
        };
        let session =
            specular_cast::session::start(Renderer::new(d.clone()), media, self.loader.waker());
        self.cast.active = Some(Active {
            session,
            name: d.name.clone(),
            udn: d.udn.clone(),
            movie: id,
            started: false,
            state: None,
            pos: start_at,
            at: Instant::now(),
            duration,
            playing: play,
            volume: None,
            volume_sent: None,
            muted: false,
            seek: None,
            ended: false,
            asked_speed: speed,
            speed: 1.0,
            subs: self.cast_subs_key(),
        });
        self.redraw = true;
    }

    /// The text subtitles chosen now and their delay (picture subtitles count as none).
    fn cast_subs_key(&self) -> SubsKey {
        let track = self
            .player
            .track
            .as_ref()
            .filter(|t| !t.is_bitmap())
            .map(|t| std::sync::Arc::as_ptr(t) as usize);
        (track, track.map_or(0, |_| self.player.sub_delay_ms))
    }

    /// The subtitles or their delay changed while casting: sent again (the device loads
    /// the file again where it is).
    fn cast_follow_subtitles(&mut self) {
        let key = self.cast_subs_key();
        let Some(a) = self.cast.active.as_ref() else {
            return;
        };
        if !a.started || a.ended || a.subs == key {
            return;
        }
        let Some(path) = self.cast_media().map(|m| m.1) else {
            return;
        };
        let srt = self.cast_subtitles(&path);
        if let Some(a) = self.cast.active.as_mut() {
            a.subs = key;
            a.session.send(Cmd::Subtitles(srt));
        }
    }

    /// The chosen text subtitles as a SubRip file in the cache, to send with the movie, with
    /// the subtitle delay applied (the device shows them as they are timed).
    fn cast_subtitles(&self, movie: &std::path::Path) -> Option<PathBuf> {
        let track = self.player.track.as_ref()?;
        let text = specular_subs::write::srt(&delayed(track, self.player.sub_delay_ms * 1000));
        if text.is_empty() {
            return None;
        }
        let dir = specular_settings::Locations::detect()
            .cache_dir
            .join("cast");
        std::fs::create_dir_all(&dir).ok()?;
        let stem = movie
            .file_stem()
            .map_or_else(|| "subtitles".into(), |s| s.to_string_lossy().into_owned());
        let p = dir.join(format!("{stem}.srt"));
        std::fs::write(&p, text).ok()?;
        Some(p)
    }

    /// The player's speed changed while casting: the device is asked for it.
    fn cast_follow_speed(&mut self) {
        let Some(s) = self.anim.as_ref().map(crate::anim::Anim::speed) else {
            return;
        };
        if let Some(a) = self.cast.active.as_mut()
            && a.started
            && (a.asked_speed - s).abs() > 1e-6
        {
            a.asked_speed = s;
            a.session.send(Cmd::Speed(s));
        }
    }

    /// Stops casting: the local player goes on from the device's position.
    pub(super) fn cast_stop(&mut self) {
        let Some(a) = self.cast.active.take() else {
            return;
        };
        let at = a.position(Instant::now());
        let playing = a.playing && !a.ended;
        let started = a.started;
        // Dropping the session stops the device and the file's server.
        drop(a);
        if started {
            self.cast_local_at(at, playing);
        }
        let now = self.now();
        self.chrome.toast(t!("toast-cast-back"), None, now);
        self.redraw = true;
    }

    /// The local player at `secs`, playing or paused.
    fn cast_local_at(&mut self, secs: f64, play: bool) {
        if let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) {
            a.seek_time(Duration::from_secs_f64(secs.max(0.0)));
            if a.is_playing() != play {
                a.toggle(Instant::now());
            }
        }
        self.tick_animation();
        self.redraw = true;
    }

    /// The player's "Play on…" choices.
    pub(super) fn cast_command(&mut self, c: &specular_widgets::PlayerCmd) {
        use specular_widgets::PlayerCmd as P;
        match c {
            P::CastTo(i) => self.cast_to(*i),
            P::CastStop => self.cast_stop(),
            P::CastSearch => {
                if off::is_off() {
                    let now = self.now();
                    self.chrome.toast(t!("network-off"), None, now);
                } else if self.cast.finder.is_none() {
                    self.cast_search();
                }
            }
            _ => {}
        }
        self.redraw = true;
    }

    /// What the menu and the card show.
    pub(super) fn cast_view(&self) -> CastView {
        let c = &self.cast;
        let active = c.active.as_ref().filter(|_| self.casting());
        CastView {
            devices: c.devices.iter().map(|d| d.name.clone()).collect(),
            searching: c.finder.is_some(),
            network_off: off::is_off(),
            active: active.and_then(|a| c.devices.iter().position(|d| d.udn == a.udn)),
            playing_on: active.map(|a| a.name.clone()),
            status: active.and_then(|a| {
                Some(if !a.started {
                    t!("cast-connecting")
                } else if a.ended {
                    t!("cast-finished")
                } else {
                    match a.state? {
                        TransportState::Transitioning => t!("cast-loading"),
                        TransportState::Paused => t!("cast-paused"),
                        _ => return None,
                    }
                })
            }),
            // Devices take text subtitles only (DLNA has no picture subtitles beside a file).
            subtitles_note: self
                .player
                .track
                .as_ref()
                .filter(|t| t.is_bitmap())
                .map(|_| t!("cast-subtitles-pictures")),
        }
    }

    /// The transport shows the device: its position, length, play state and volume.
    pub(super) fn cast_playback(&self, mut pb: Playback) -> Playback {
        let Some(a) = self.cast.active.as_ref().filter(|_| self.casting()) else {
            return pb;
        };
        pb.playing = a.playing;
        if let Some(v) = pb.video.as_mut() {
            // The device's own volume, up to its 100 %.
            v.volume_max = 1.0;
            v.position = a.position(Instant::now());
            if a.duration > 0.0 {
                v.duration = a.duration;
            }
            if let Some(vol) = a.volume {
                v.sound = Some((f32::from(vol) / 100.0, a.muted));
            }
        }
        pb
    }

    /// Seeks the device to `secs`.
    fn cast_seek(&mut self, secs: f64) {
        let Some(a) = self.cast.active.as_mut() else {
            return;
        };
        let max = if a.duration > 0.0 {
            a.duration
        } else {
            f64::MAX
        };
        let t = secs.clamp(0.0, max);
        a.seek = Some((t, Instant::now()));
        a.pos = t;
        a.at = Instant::now();
        a.ended = false;
        a.session.send(Cmd::Seek(t));
        self.redraw = true;
    }

    /// Seeks the device by `secs` (negative: back). Whether casting took it.
    pub(super) fn cast_seek_by(&mut self, secs: f64) -> bool {
        if !self.casting() {
            return false;
        }
        let now = Instant::now();
        let at = self.cast.active.as_ref().map_or(0.0, |a| a.position(now));
        self.cast_seek(at + secs);
        true
    }

    /// Seeks the device to `fraction` of the file (the seek bar, the digit keys). Whether
    /// casting took it; a drag seeks once, when let go.
    pub(super) fn cast_seek_fraction(&mut self, fraction: f64, done: bool) -> bool {
        if !self.casting() {
            return false;
        }
        let d = self.cast.active.as_ref().map_or(0.0, |a| a.duration);
        if d > 0.0 {
            let t = d * fraction.clamp(0.0, 1.0);
            if done {
                self.cast_seek(t);
            } else if let Some(a) = self.cast.active.as_mut() {
                // The bar follows the drag.
                a.seek = Some((t, Instant::now()));
            }
        }
        self.redraw = true;
        true
    }

    /// The volume slider while casting: the device's own volume (0..=1 of its top). Raising it
    /// unmutes, as the volume keys do. A drag sends at most every [`VOLUME_SEND`] and once
    /// more when let go, so the device's queue never fills. Whether casting took it.
    pub(super) fn cast_set_volume(&mut self, volume: f32, done: bool) -> bool {
        if !self.casting() {
            return false;
        }
        let Some(a) = self.cast.active.as_mut() else {
            return false;
        };
        let v = (volume * 100.0).round().clamp(0.0, 100.0) as u8;
        if v > a.volume.unwrap_or(50) {
            a.muted = false;
        }
        a.volume = Some(v);
        let now = Instant::now();
        let due = match a.volume_sent {
            Some((sent, _)) if sent == v => false,
            Some((_, at)) => done || now.duration_since(at) >= VOLUME_SEND,
            None => true,
        };
        if due {
            a.session.send(Cmd::Volume(v));
            a.volume_sent = Some((v, now));
        }
        self.redraw = true;
        true
    }

    /// The transport's keys while casting: play and pause, seek, volume and mute go to the
    /// device. Whether casting took `action`.
    pub(super) fn cast_action(&mut self, action: Action) -> bool {
        if !self.casting() {
            return false;
        }
        let now = self.now();
        let step = crate::anim::SEEK_STEP.as_secs_f64();
        match action {
            Action::PlayPause | Action::PlayPauseVideo => {
                let Some(a) = self.cast.active.as_mut() else {
                    return false;
                };
                let play = !a.playing || a.ended;
                if play && a.ended {
                    a.ended = false;
                    a.session.send(Cmd::Seek(0.0));
                    a.pos = 0.0;
                }
                a.pos = a.position(Instant::now());
                a.at = Instant::now();
                a.playing = play;
                a.session.send(if play { Cmd::Play } else { Cmd::Pause });
            }
            Action::SeekForward => return self.cast_seek_by(step),
            Action::SeekBack => return self.cast_seek_by(-step),
            Action::VolumeUp | Action::VolumeDown | Action::ToggleMute => {
                let Some(a) = self.cast.active.as_mut() else {
                    return false;
                };
                let msg = if action == Action::ToggleMute {
                    a.muted = !a.muted;
                    a.session.send(Cmd::Mute(a.muted));
                    if a.muted {
                        t!("toast-muted")
                    } else {
                        t!("toast-unmuted")
                    }
                } else {
                    let v = a.volume.unwrap_or(50);
                    let v = if action == Action::VolumeUp {
                        v.saturating_add(VOLUME_STEP).min(100)
                    } else {
                        v.saturating_sub(VOLUME_STEP)
                    };
                    a.volume = Some(v);
                    a.muted = false;
                    a.session.send(Cmd::Volume(v));
                    a.volume_sent = Some((v, Instant::now()));
                    t!("toast-volume", percent = i64::from(v))
                };
                self.chrome.toast(msg, None, now);
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(now: Instant) -> Active {
        // A session to nowhere: its worker fails at once (nothing is listening).
        let d = Device {
            name: "TV".into(),
            udn: "uuid:x".into(),
            location: "http://127.0.0.1:9/d.xml".into(),
            manufacturer: None,
            model: None,
            av_transport: specular_cast::device::Service {
                kind: "urn:schemas-upnp-org:service:AVTransport:1".into(),
                control: "http://127.0.0.1:9/avt".into(),
                scpd: None,
            },
            rendering: None,
            connection: None,
            sec_captions: false,
        };
        let session = specular_cast::session::start(
            Renderer::new(d),
            Media {
                path: PathBuf::from("/nonexistent"),
                mime: "video/mp4".into(),
                title: String::new(),
                start_at: 0.0,
                duration: None,
                play: true,
                speed: 1.0,
                subtitles: None,
            },
            std::sync::Arc::new(|| {}),
        );
        Active {
            session,
            name: "TV".into(),
            udn: "uuid:x".into(),
            movie: 1,
            started: true,
            state: Some(TransportState::Playing),
            pos: 10.0,
            at: now,
            duration: 100.0,
            playing: true,
            volume: Some(20),
            volume_sent: None,
            muted: false,
            seek: None,
            ended: false,
            asked_speed: 1.0,
            speed: 1.0,
            subs: (None, 0),
        }
    }

    #[test]
    fn position_runs_while_playing_and_holds_a_seek() {
        let t0 = Instant::now();
        let mut a = active(t0);
        let later = t0 + Duration::from_secs(3);
        assert!((a.position(later) - 13.0).abs() < 1e-6);
        a.seek = Some((50.0, t0));
        assert_eq!(a.position(later), 50.0);
        // The device reports near the target: the seek is done.
        a.status(
            Status {
                state: TransportState::Playing,
                position: Some(49.5),
                duration: Some(100.0),
            },
            later,
        );
        assert!(a.seek.is_none());
        assert!((a.position(later) - 49.5).abs() < 1e-6);
        // Stopped after playing: the end.
        a.status(
            Status {
                state: TransportState::Stopped,
                position: Some(0.0),
                duration: None,
            },
            later,
        );
        assert!(a.ended && !a.playing);
        assert!(a.position(t0 + Duration::from_secs(1000)) <= 100.0);
    }

    #[test]
    fn reasons_are_worded() {
        for e in [
            Error::NetworkOff,
            Error::Unreachable,
            Error::Timeout,
            Error::Status(500),
            Error::Fault(714, "x".into()),
            Error::File("x".into()),
            Error::BadReply("x"),
        ] {
            let r = reason(&e);
            assert!(!r.is_empty() && !r.starts_with("cast-error"), "{e:?}: {r}");
        }
        assert!(reason(&Error::Fault(714, String::new())).contains("714"));
    }
}

/// `track` as the player shows it with a subtitle delay of `delay_us` (later when positive):
/// every cue moved by it, those then ending before the start dropped, starts kept at 0 or
/// after.
fn delayed(track: &specular_subs::Track, delay_us: i64) -> specular_subs::Track {
    if delay_us == 0 {
        return track.clone();
    }
    specular_subs::Track::new(
        track
            .cues
            .iter()
            .filter(|c| c.end_us.saturating_add(delay_us) > 0)
            .map(|c| {
                let mut c = c.clone();
                c.start_us = c.start_us.saturating_add(delay_us).max(0);
                c.end_us = c.end_us.saturating_add(delay_us);
                c
            })
            .collect(),
    )
}

#[cfg(test)]
mod delay_tests {
    use specular_subs::{Body, Cue, Span, TextCue, Track};

    fn cue(a: i64, b: i64, t: &str) -> Cue {
        Cue {
            start_us: a,
            end_us: b,
            body: Body::Text(TextCue {
                lines: vec![vec![Span::plain(t)]],
                align: 2,
                pos: None,
                margin_v: None,
            }),
        }
    }

    fn text_at(t: &Track, at: i64) -> Vec<String> {
        t.active(at)
            .filter_map(|c| c.text().map(|x| x.plain_text()))
            .collect()
    }

    /// The served SubRip file has the delay in it: what the device shows at a time is what
    /// the player shows then.
    #[test]
    fn cast_subtitles_carry_the_delay() {
        let t = Track::new(vec![
            cue(500_000, 1_500_000, "a"),
            cue(4_000_000, 6_000_000, "b"),
        ]);
        for delay in [-1_000_000i64, 0, 2_500_000] {
            let d = super::delayed(&t, delay);
            let back = specular_subs::srt::parse(&specular_subs::write::srt(&d)).unwrap();
            for at in (0..9_000_000).step_by(250_000) {
                let want = text_at(&t, at - delay);
                let got = text_at(&back, at);
                assert_eq!(got, want, "delay {delay} at {at}");
            }
        }
        assert_eq!(super::delayed(&t, -1_600_000).cues.len(), 1);
    }
}

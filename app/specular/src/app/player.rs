//! A movie's player beyond play, pause and seek: the screen kept awake while it plays,
//! subtitles (the movie's tracks and files beside it), the choice of sound track, subtitle
//! and sound delays, chapters, A–B repeat, resuming where it was left, the seek bar's
//! preview frame, saving and copying the frame, the mini player and
//! the picture's shape and crop.
//!
//! What the movie holds besides its picture and sound (`specular_video::extras`) is read on
//! a worker when the movie opens; subtitles decode on a worker too.

use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use specular_i18n::t;
use specular_input::Action;
use specular_settings::Origin;
use specular_subs::Track;
use specular_video::extras::Extras;
use specular_widgets::{PlayerCmd, PlayerView};

use super::App;
use crate::loader::ImageId;

pub(super) mod frame;
pub(super) mod media;
pub(super) mod mini;
pub(super) mod picture;
#[cfg(test)]
mod picture_shots;
pub(super) mod preview;
pub(super) mod resume;
#[cfg(test)]
mod shots;
mod stt;
pub(super) mod subs;

/// The toast button "Start over" (after resuming).
pub(super) const START_OVER_KEY: u64 = (1 << 59) | 0x5E7;
/// How often the position is written while playing (app clock, seconds).
const SAVE_EVERY: f64 = 5.0;
/// The coarse and fine delay steps (milliseconds).
const STEP_MS: i64 = 50;
const FINE_MS: i64 = 10;
/// Where "Subtitle style…" opens Settings: the subtitles' look, from their size on.
const SUBTITLE_STYLE_AT: &str = "viewing.subtitle_size";
/// The picture shapes `CycleAspect` and `CycleCrop` go through (0: as stored / none).
pub(super) const SHAPES: [Option<(f32, &str)>; 5] = [
    None,
    Some((4.0 / 3.0, "4:3")),
    Some((16.0 / 9.0, "16:9")),
    Some((1.85, "1.85:1")),
    Some((2.39, "2.39:1")),
];

/// The player's state for the movie on screen.
#[derive(Default)]
pub(super) struct Player {
    pub awake: specular_platform::awake::Awake,
    /// The movie the state is for, and its file.
    movie: Option<(ImageId, PathBuf)>,
    /// Its key in the resume store.
    key: Option<String>,
    pub(super) extras: Option<Arc<Extras>>,
    extras_rx: Option<mpsc::Receiver<(ImageId, Extras)>>,
    /// The subtitle tracks on offer and the one chosen.
    pub(super) sources: Vec<subs::Source>,
    pub(super) sub: Option<usize>,
    pub(super) track: Option<Arc<Track>>,
    sub_rx: Option<(usize, mpsc::Receiver<subs::Decoded>)>,
    /// A character set forced for the chosen subtitle file, and the one it was read in.
    charset: Option<&'static encoding_rs::Encoding>,
    read_as: Option<&'static encoding_rs::Encoding>,
    pub(super) bitmaps: subs::Bitmaps,
    /// The sound track chosen (its container id; `None`: the movie's default).
    pub(super) sound: Option<u64>,
    pub(super) sub_delay_ms: i64,
    pub(super) sound_delay_ms: i64,
    /// A–B repeat points (seconds).
    pub(super) ab: (Option<f64>, Option<f64>),
    resume: Option<resume::Store>,
    /// Where to resume once the first frame shows.
    pending_resume: Option<f64>,
    last_saved: f64,
    /// The picture shape and crop chosen (indexes into [`SHAPES`]).
    pub(super) aspect: usize,
    pub(super) crop: usize,
    /// The view's own pixel aspect, to put back.
    base_aspect: Option<f32>,
    pub(super) preview: preview::Preview,
    pub(super) mini: mini::Mini,
    /// The end of the movie was seen (to act on it once).
    ended: bool,
    /// "Open subtitle file…" answered.
    open_rx: Option<mpsc::Receiver<PathBuf>>,
    /// Subtitles were chosen or turned off by hand for this movie.
    sub_chosen: bool,
    /// The system's media controls.
    pub(super) media: media::Media,
    /// A frame saved or copied: the outcome to say.
    pub(super) frame_rx: Option<mpsc::Receiver<frame::Done>>,
    /// The font families were offered to the subtitle font setting (once the platform's
    /// fonts are in).
    pub(super) fonts_offered: bool,
    /// The picture's turn (rotate and flip) on top of the file's own, kept for the file.
    pub(super) turn: specular_core::Orientation,
    /// The picture adjustments, for this session (kept from movie to movie).
    pub(super) picture: picture::Values,
    /// Generated subtitles: the job under way and the cache lookup.
    stt: stt::Stt,
}

impl Player {
    /// The sound track chosen for movie `id` (`None`: its default, or another movie).
    pub(super) fn sound_track_of(&self, id: ImageId) -> Option<u64> {
        self.sound
            .filter(|_| self.movie.as_ref().is_some_and(|m| m.0 == id))
    }

    fn store(&mut self) -> &mut resume::Store {
        self.resume
            .get_or_insert_with(|| resume::Store::at(resume::Store::default_dir()))
    }
}

/// "12:34" from seconds.
fn clock(secs: f64) -> String {
    super::playback::clock(secs)
}

/// "+350 ms" / "−120 ms".
fn delay(ms: i64) -> String {
    specular_widgets::delay_text(ms)
}

/// The subtitle file a drop names while a movie is on screen (`movie`): a subtitle file
/// itself, or the `.idx` beside a dropped VobSub `.sub`.
fn dropped_subtitle(movie: bool, p: &std::path::Path) -> Option<PathBuf> {
    if !movie {
        return None;
    }
    let sub = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("sub"));
    let idx = p.with_extension("idx");
    if sub && idx.is_file() {
        return Some(idx);
    }
    specular_subs::FileKind::of(p)
        .is_some()
        .then(|| p.to_path_buf())
}

impl App {
    /// The movie on screen with a picture, and its file.
    fn player_movie(&self) -> Option<(ImageId, PathBuf)> {
        let a = self.anim.as_ref()?;
        let m = a.movie.as_ref()?;
        if Some(a.id) != self.current_id() || !m.picture {
            return None;
        }
        let img = self.images.get(&a.id)?;
        let path = img.session.as_ref()?.source().path().to_path_buf();
        Some((a.id, path))
    }

    /// Each tick: follows the movie on screen (its extras, subtitles, sound choice,
    /// resume), holds the screen awake while it plays, loops A–B, and acts on its end.
    pub(super) fn player_tick(&mut self) {
        // What plays next when this ends (queue.rs).
        self.queue_tick();
        let cur = self.player_movie();
        if cur.as_ref().map(|c| c.0) != self.player.movie.as_ref().map(|c| c.0) {
            self.player_leave();
            if let Some((id, path)) = cur.clone() {
                self.player_enter(id, path);
            }
        }
        self.player_poll();
        self.cast_tick();
        let playing = self
            .anim
            .as_ref()
            .is_some_and(|a| a.is_playing() && !a.hidden)
            && cur.is_some();
        self.player.awake.set(playing);
        let title = self
            .anim
            .as_ref()
            .filter(|a| a.movie.is_some() && Some(a.id) == self.current_id())
            .and_then(|a| self.images.get(&a.id))
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.source().path().file_name())
            .map(|n| n.to_string_lossy().into_owned());
        self.media_tick(title);
        // The mini player ends with the movie.
        if cur.is_none() && self.player.mini.active() {
            self.mini_leave();
        }
        let n = self.now();
        let Some(a) = self.anim.as_mut() else { return };
        if cur.is_none() {
            return;
        }
        let now = Instant::now();
        let t = a.time(now).map_or(0.0, |d| d.as_secs_f64());
        // Resume once the first frame is on screen.
        if let Some(at) = self.player.pending_resume.take() {
            if a.frame().is_some() {
                a.seek_time(Duration::from_secs_f64(at));
                self.chrome.toast_action(
                    t!("toast-video-resumed", time = clock(at)),
                    t!("toast-video-start-over"),
                    START_OVER_KEY,
                    n,
                );
                self.redraw = true;
            } else {
                self.player.pending_resume = Some(at);
            }
        }
        // A–B repeat.
        if let (Some(ta), Some(tb)) = self.player.ab
            && a.is_playing()
            && t >= tb
        {
            a.seek_time(Duration::from_secs_f64(ta));
        }
        // Where it is, now and then while playing.
        if a.is_playing() && n - self.player.last_saved > SAVE_EVERY {
            self.player.last_saved = n;
            self.player_save();
        }
        // The end.
        let finished = self.anim.as_ref().is_some_and(crate::anim::Anim::finished);
        if finished && !self.player.ended {
            self.player.ended = true;
            self.player_save();
        } else if !finished {
            self.player.ended = false;
        }
    }

    /// A new movie: its extras are read, its remembered settings come back.
    fn player_enter(&mut self, id: ImageId, path: PathBuf) {
        self.player.key = resume::key(&path);
        let entry = self
            .player
            .key
            .clone()
            .and_then(|k| self.player.store().get(&k));
        let e = entry.unwrap_or_default();
        self.player.sub_delay_ms = e.sub_delay_ms;
        self.player.sound_delay_ms = e.sound_delay_ms;
        self.player.turn = specular_core::Orientation(e.turn.unwrap_or(1));
        self.player.charset = e
            .charset
            .as_deref()
            .and_then(|c| encoding_rs::Encoding::for_label(c.as_bytes()));
        if let Some(a) = self.anim.as_mut()
            && e.sound_delay_ms != 0
        {
            a.set_sound_delay(e.sound_delay_ms * 1000);
        }
        let duration = self
            .anim
            .as_ref()
            .and_then(|a| a.playback().video)
            .map_or(0.0, |v| v.duration);
        if self.settings.viewing.video_resume {
            self.player.pending_resume =
                e.position.and_then(|p| resume::keep_position(p, duration));
        }
        self.player.last_saved = self.now();
        // Extras off the UI thread.
        let (tx, rx) = mpsc::channel();
        let source = self
            .images
            .get(&id)
            .and_then(|i| i.session.as_ref())
            .map(|s| Arc::clone(s.source()));
        let waker = self.loader.waker();
        if let Some(source) = source {
            let _ = specular_thread::builder("player-extras").spawn(move || {
                let x = specular_video::extras::read(source.bytes());
                let _ = tx.send((id, x));
                waker();
            });
        }
        self.player.extras_rx = Some(rx);
        self.player.movie = Some((id, path.clone()));
        // Files beside it show in the menu at once.
        self.player.sources = subs::sources(&path, None);
        self.stt_enter(&path);
        self.player_remembered_choice(e.subtitle.clone(), e.sound);
        self.player_apply_turn(id);
    }

    /// The remembered (or preferred) subtitle track and sound choice.
    fn player_remembered_choice(&mut self, subtitle: Option<String>, sound: Option<u64>) {
        let extras = self.player.extras.clone();
        // The sound first: forced subtitles follow its language.
        if let Some(x) = extras.as_deref() {
            let v = &self.settings.viewing;
            let chosen = subs::pick_sound(
                &x.audio,
                sound,
                &v.sound_language,
                v.sound_prefer_description,
            );
            let default = x
                .audio
                .iter()
                .find(|a| a.default)
                .or(x.audio.first())
                .map(|a| a.id);
            if chosen.is_some() && chosen != default && chosen != self.player.sound {
                self.player_choose_sound(chosen, false);
            }
        }
        let sound_lang = self.player_sound_lang();
        let pick = subs::pick(
            &self.player.sources,
            extras.as_deref(),
            subtitle.as_deref(),
            subs::Prefs::of(&self.settings),
            sound_lang.as_deref(),
        );
        // Embedded tracks need the extras: chosen once they are read.
        let need_extras = pick.is_some_and(|i| {
            matches!(self.player.sources[i].origin, subs::Origin::Embedded(_)) && extras.is_none()
        });
        if !need_extras {
            self.player_choose_subtitle(pick, false);
        }
    }

    fn player_sound_lang(&self) -> Option<String> {
        let x = self.player.extras.as_ref()?;
        let a = match self.player.sound {
            Some(id) => x.audio.iter().find(|a| a.id == id),
            None => x.audio.iter().find(|a| a.default).or(x.audio.first()),
        }?;
        let l = a.lang.as_deref()?;
        let base = l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase();
        Some(specular_video::extras::iso639_2(&base).map_or(base, str::to_owned))
    }

    /// The movie is left: where it was is kept.
    fn player_leave(&mut self) {
        if self.player.movie.is_none() {
            return;
        }
        self.player_save();
        if let Some(gfx) = self.gfx.as_mut() {
            self.player.bitmaps.clear(&mut gfx.ui);
            self.player.preview.clear(&mut gfx.ui);
        }
        let keep_resume = self.player.resume.take();
        let awake = std::mem::take(&mut self.player.awake);
        let mini = std::mem::take(&mut self.player.mini);
        let media = std::mem::take(&mut self.player.media);
        let base = self.player.base_aspect.take();
        if let Some(b) = base {
            self.view.set_pixel_aspect(b);
        }
        let picture = self.player.picture;
        self.player = Player {
            awake,
            mini,
            media,
            resume: keep_resume,
            picture,
            ..Player::default()
        };
    }

    /// Writes the movie's entry in the resume store.
    pub(super) fn player_save(&mut self) {
        let Some(key) = self.player.key.clone() else {
            return;
        };
        let (pos, duration) = self
            .anim
            .as_ref()
            .filter(|a| Some(a.id) == self.player.movie.as_ref().map(|m| m.0))
            .and_then(|a| a.playback().video)
            .map_or((0.0, 0.0), |v| (v.position, v.duration));
        let finished = self.anim.as_ref().is_some_and(crate::anim::Anim::finished);
        let position = if self.settings.viewing.video_resume && !finished {
            resume::keep_position(pos, duration)
        } else {
            None
        };
        let extras = self.player.extras.clone();
        let entry = resume::Entry {
            position,
            sub_delay_ms: self.player.sub_delay_ms,
            sound_delay_ms: self.player.sound_delay_ms,
            sound: self.player.sound,
            subtitle: self.player.sub.map_or_else(
                || self.player.sub_chosen.then(String::new),
                |i| self.player.sources.get(i).map(|s| s.key(extras.as_deref())),
            ),
            charset: self.player.charset.map(|e| e.name().to_owned()),
            turn: (self.player.turn.0 > 1).then_some(self.player.turn.0),
            used: 0,
        };
        self.player.store().put(&key, entry);
    }

    /// Picks up the extras, decoded subtitles, a picked subtitle file and saved frames.
    fn player_poll(&mut self) {
        if let Some(rx) = &self.player.extras_rx
            && let Ok((id, x)) = rx.try_recv()
        {
            self.player.extras_rx = None;
            if self.player.movie.as_ref().is_some_and(|m| m.0 == id) {
                let path = self
                    .player
                    .movie
                    .as_ref()
                    .map(|m| m.1.clone())
                    .unwrap_or_default();
                // A stream's piece: its subtitle tracks span the stream.
                let x = Arc::new(self.stream_piece_extras(&path, x));
                self.player.extras = Some(Arc::clone(&x));
                let chosen_key = self
                    .player
                    .sub
                    .and_then(|i| self.player.sources.get(i))
                    .map(|s| s.key(Some(&x)));
                // The movie's tracks come first; files picked by hand stay.
                let mut sources = subs::sources(&path, Some(&x));
                for s in &self.player.sources {
                    if !sources.contains(s) {
                        sources.push(s.clone());
                    }
                }
                self.player.sources = sources;
                self.player.sub = chosen_key.and_then(|k| {
                    self.player
                        .sources
                        .iter()
                        .position(|s| s.key(Some(&x)) == k)
                });
                let e = self
                    .player
                    .key
                    .clone()
                    .and_then(|k| self.player.store().get(&k))
                    .unwrap_or_default();
                if self.player.sub.is_none() && self.player.sub_rx.is_none() {
                    self.player_remembered_choice(e.subtitle, e.sound);
                } else {
                    self.player_remembered_choice(None, e.sound);
                }
                self.redraw = true;
            }
        }
        if let Some((i, rx)) = &self.player.sub_rx
            && let Ok(r) = rx.try_recv()
        {
            let i = *i;
            self.player.sub_rx = None;
            let now = self.now();
            match r {
                Ok((track, enc)) if self.player.sub == Some(i) => {
                    if track.simplified {
                        specular_log::debug!(target: specular_log::DECODE, "subtitles: some ASS effects left out");
                    }
                    if track.cues.is_empty() {
                        self.chrome.toast(t!("toast-subtitles-empty"), None, now);
                    }
                    self.player.track = Some(Arc::new(track));
                    self.player.read_as = enc;
                }
                Ok(_) => {}
                Err(why) => {
                    self.player.sub = None;
                    self.chrome.toast(
                        t!("toast-subtitles-unreadable", reason = subs_reason(&why)),
                        None,
                        now,
                    );
                }
            }
            self.redraw = true;
        }
        if let Some(rx) = &self.player.open_rx
            && let Ok(p) = rx.try_recv()
        {
            self.player.open_rx = None;
            self.player_add_file(p);
        }
        self.stt_poll();
        self.frame_poll();
    }

    /// Shows subtitle track `i` (or none). `chosen`: by the user (the language becomes the
    /// preferred one).
    pub(super) fn player_choose_subtitle(&mut self, i: Option<usize>, chosen: bool) {
        let i = i.filter(|&i| i < self.player.sources.len());
        if i != self.player.sub {
            self.player.track = None;
            self.player.read_as = None;
            if self.player.sub.is_some() {
                self.player.charset = None;
            }
        }
        self.player.sub = i;
        self.player.sub_chosen = chosen || self.player.sub_chosen;
        if let Some(i) = i
            && self.player.track.is_none()
            && let Some(t) = self.stt_track(&self.player.sources[i])
        {
            self.player.track = Some(t);
        }
        if let Some(i) = i
            && self.player.track.is_none()
        {
            let src = self.player.sources[i].clone();
            let rx = subs::decode(
                &src,
                self.player.extras.clone(),
                self.player.charset,
                Some(self.loader.waker()),
            );
            self.player.sub_rx = Some((i, rx));
        }
        if chosen {
            let lang = i
                .and_then(|i| self.player.sources.get(i))
                .and_then(subs::Source::lang2)
                .unwrap_or_default();
            if lang != self.settings.viewing.subtitle_language {
                self.store
                    .update(Origin::Program, |s| s.viewing.subtitle_language = lang);
            }
            let now = self.now();
            let msg = match i {
                Some(i) => t!("toast-subtitles", track = self.player.sources[i].label(i)),
                None => t!("toast-subtitles-off"),
            };
            self.chrome.toast(msg, None, now);
            self.player_save();
        }
        self.redraw = true;
    }

    /// Plays sound track `id` (a container id).
    pub(super) fn player_choose_sound(&mut self, id: Option<u64>, chosen: bool) {
        self.player.sound = id;
        let Some(source) = self
            .player
            .movie
            .as_ref()
            .and_then(|m| self.images.get(&m.0))
            .and_then(|i| i.session.as_ref())
            .map(|s| Arc::clone(s.source()))
        else {
            return;
        };
        let no_device = std::env::var_os("SPECULAR_NO_SOUND").is_some();
        // The sound settings from the first buffer (a movie: no album run).
        let sound = super::sound_fx::sound_settings(&self.settings.viewing, false);
        let open: Option<Box<dyn FnOnce() -> Result<_, specular_video::audio::Silence> + Send>> =
            (!no_device).then(|| {
                Box::new(move || {
                    specular_video::session::open_sound_track_with(
                        source,
                        Box::new(crate::sound::CpalOutput::new()),
                        id,
                        &sound,
                    )
                })
                    as Box<dyn FnOnce() -> Result<_, specular_video::audio::Silence> + Send>
            });
        let s = &self.settings.viewing;
        let movie = crate::anim::Movie::new(
            true,
            self.loop_forever,
            crate::anim::volume(&self.settings),
            s.video_muted,
            open,
        );
        if let Some(a) = self.anim.as_mut() {
            a.replace_sound(movie);
        }
        if chosen {
            if let Some(x) = self.player.extras.clone()
                && let Some(i) = x.audio.iter().position(|a| Some(a.id) == id)
            {
                let lang = x.audio[i]
                    .lang
                    .as_deref()
                    .map(|l| l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase())
                    .map(|b| specular_video::extras::iso639_2(&b).map_or(b, str::to_owned))
                    .unwrap_or_default();
                if lang != self.settings.viewing.sound_language {
                    self.store
                        .update(Origin::Program, |s| s.viewing.sound_language = lang);
                }
                let now = self.now();
                let label = sound_label(&x.audio[i], i);
                self.chrome
                    .toast(t!("toast-sound-track", track = label), None, now);
            }
            self.player_save();
        }
        self.redraw = true;
    }

    /// A subtitle file to show (from "Open subtitle file…" or dropped on a movie).
    pub(super) fn player_add_file(&mut self, p: PathBuf) {
        if specular_subs::FileKind::of(&p).is_none() {
            let now = self.now();
            self.chrome.toast(
                t!(
                    "toast-subtitles-unreadable",
                    reason = t!("subtitles-reason-not-subtitles")
                ),
                None,
                now,
            );
            return;
        }
        let stem = self
            .player
            .movie
            .as_ref()
            .and_then(|m| m.1.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        let flags = specular_subs::file_flags(&p);
        let src = subs::Source {
            lang: specular_subs::file_language(&stem, &p),
            name: None,
            forced: flags.forced,
            sdh: flags.sdh,
            caption: None,
            origin: subs::Origin::File(p),
        };
        let i = match self.player.sources.iter().position(|s| *s == src) {
            Some(i) => i,
            None => {
                self.player.sources.push(src);
                self.player.sources.len() - 1
            }
        };
        self.player_choose_subtitle(Some(i), true);
    }

    /// A file dropped on the window: a subtitle file while a movie is on screen is shown
    /// with it (true); anything else opens as usual.
    pub(super) fn player_subtitle_drop(&mut self, p: &std::path::Path) -> bool {
        let Some(p) = dropped_subtitle(self.player_movie().is_some(), p) else {
            return false;
        };
        self.player_add_file(p);
        true
    }

    /// "Open subtitle file…": the platform's file dialog, off the UI thread.
    fn player_open_dialog(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.player.open_rx = Some(rx);
        let dir = self
            .player
            .movie
            .as_ref()
            .and_then(|m| m.1.parent().map(PathBuf::from));
        let title = t!("player-subtitles-open-title");
        let name = t!("player-subtitles-file-kind");
        self.loader.background("dlg-subtitle", move |l| {
            let mut d = rfd::FileDialog::new().set_title(&title).add_filter(
                &name,
                &[
                    "srt", "vtt", "ass", "ssa", "sup", "idx", "sub", "lrc", "ttml", "dfxp", "scc",
                ],
            );
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
    }

    /// The subtitle delay `ms` later (negative: earlier); `None` resets it.
    fn player_sub_delay(&mut self, by: Option<i64>) {
        self.player.sub_delay_ms = by.map_or(0, |b| {
            (self.player.sub_delay_ms + b).clamp(-600_000, 600_000)
        });
        let now = self.now();
        let msg = t!(
            "toast-subtitle-delay",
            delay = delay(self.player.sub_delay_ms)
        );
        self.chrome.toast(msg, None, now);
        self.player_save();
        self.redraw = true;
    }

    /// The subtitles' place and size keys: saved in the settings, said in a toast. The cue on
    /// screen follows at once.
    fn player_sub_look(&mut self, action: Action) {
        let v = &self.settings.viewing;
        let (mut position, mut size) = (v.subtitle_position, v.subtitle_size);
        let msg = match action {
            Action::SubtitleUp | Action::SubtitleDown => {
                let by = if action == Action::SubtitleUp { 1 } else { -1 };
                position = subs::step_position(position, by);
                t!("toast-subtitle-position", percent = position)
            }
            Action::SubtitleBigger | Action::SubtitleSmaller => {
                size = subs::step_size(size, action == Action::SubtitleBigger);
                t!("toast-subtitle-size", percent = size)
            }
            _ => {
                position = subs::DEFAULT_POSITION_PERCENT;
                size = 100;
                t!("toast-subtitle-place-reset")
            }
        };
        self.store.update(Origin::Program, |s| {
            s.viewing.subtitle_position = position;
            s.viewing.subtitle_size = size;
        });
        // The store's change reaches `self.settings` with the next settings event; the cue on
        // screen takes the new values now.
        let v = &mut Arc::make_mut(&mut self.settings).viewing;
        v.subtitle_position = position;
        v.subtitle_size = size;
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The sound delay `ms` later (negative: earlier); `None` resets it.
    fn player_sound_delay(&mut self, by: Option<i64>) {
        self.player.sound_delay_ms = by.map_or(0, |b| {
            (self.player.sound_delay_ms + b).clamp(-600_000, 600_000)
        });
        let us = self.player.sound_delay_ms * 1000;
        if let Some(a) = self.anim.as_mut() {
            a.set_sound_delay(us);
        }
        let now = self.now();
        let msg = t!(
            "toast-sound-delay",
            delay = delay(self.player.sound_delay_ms)
        );
        self.chrome.toast(msg, None, now);
        self.player_save();
        self.redraw = true;
    }

    /// The player time on screen (seconds).
    pub(super) fn player_time(&self) -> f64 {
        self.anim
            .as_ref()
            .and_then(|a| a.time(Instant::now()))
            .map_or(0.0, |d| d.as_secs_f64())
    }

    /// The movie's time (microseconds on its own timeline) of player time `t`.
    fn player_media_us(&self, t: f64) -> i64 {
        let start = self
            .player
            .extras
            .as_ref()
            .map_or(0, |x| x.picture_start_us);
        (t * 1e6) as i64 + start
    }

    /// Shifts the subtitle delay so the previous (`forward` false) or next subtitle starts
    /// now.
    fn player_sub_step(&mut self, forward: bool) {
        let Some(track) = self.player.track.clone() else {
            let now = self.now();
            self.chrome.toast(t!("toast-subtitles-none"), None, now);
            return;
        };
        let now_us = self.player_media_us(self.player_time());
        let d = self.player.sub_delay_ms * 1000;
        // In the track's own time, where "now" falls with the current delay.
        let at = now_us - d;
        let target = if forward {
            track.next_start(at)
        } else {
            // The one showing now counts as current: step to the one before it.
            let cur = track.prev_start(at + 1);
            cur.and_then(|c| track.prev_start(c))
        };
        let Some(start) = target else { return };
        let new_ms = (now_us - start) / 1000;
        self.player_sub_delay(Some(new_ms - self.player.sub_delay_ms));
    }

    /// The previous or next chapter.
    fn player_chapter(&mut self, forward: bool) {
        let Some(x) = self.player.extras.clone() else {
            return;
        };
        if x.chapters.is_empty() {
            let now = self.now();
            self.chrome.toast(t!("toast-no-chapters"), None, now);
            return;
        }
        let t_us = self.player_media_us(self.player_time());
        let start = x.picture_start_us;
        let idx = if forward {
            x.chapters.iter().position(|c| c.start_us > t_us + 500_000)
        } else {
            // Within the first 3 s of a chapter: the one before.
            let cur = x.chapters.iter().rposition(|c| c.start_us <= t_us + 1);
            cur.map(|c| {
                if t_us - x.chapters[c].start_us < 3_000_000 {
                    c.saturating_sub(1)
                } else {
                    c
                }
            })
        };
        if let Some(i) = idx {
            self.player_go_chapter(i, &x, start);
        }
    }

    fn player_go_chapter(&mut self, i: usize, x: &Extras, start: i64) {
        let Some(c) = x.chapters.get(i) else { return };
        let t = ((c.start_us - start).max(0) as f64) / 1e6;
        if let Some(a) = self.anim.as_mut() {
            a.seek_time(Duration::from_secs_f64(t));
        }
        let lang = specular_i18n::I18n::global().language().to_string();
        let name = c.name(&lang).map_or_else(
            || t!("player-chapter-number", number = i as i64 + 1),
            str::to_owned,
        );
        let now = self.now();
        self.chrome
            .toast(t!("toast-chapter", chapter = name), None, now);
        self.tick_animation();
        self.redraw = true;
    }

    /// A–B repeat: set A, then B, then clear.
    fn player_ab(&mut self) {
        let t = self.player_time();
        let now = self.now();
        let msg = match self.player.ab {
            (None, _) => {
                self.player.ab = (Some(t), None);
                t!("toast-ab-a", time = clock(t))
            }
            (Some(a), None) if t > a + 0.2 => {
                self.player.ab = (Some(a), Some(t));
                t!("toast-ab-b", a = clock(a), b = clock(t))
            }
            _ => {
                self.player.ab = (None, None);
                t!("toast-ab-off")
            }
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The picture's shape: as stored, then each of [`SHAPES`].
    fn player_aspect(&mut self) {
        let (w, h) = self.view.stored_size();
        if w == 0 || h == 0 {
            return;
        }
        let base = *self
            .player
            .base_aspect
            .get_or_insert(self.view.pixel_aspect());
        self.player.aspect = (self.player.aspect + 1) % SHAPES.len();
        let (pa, name) = match SHAPES[self.player.aspect] {
            Some((dar, name)) => (dar * h as f32 / w as f32, name.to_owned()),
            None => (base, t!("player-shape-stored")),
        };
        self.view.set_pixel_aspect(pa);
        self.view.zoom_fit();
        self.after_view_change();
        let now = self.now();
        self.chrome
            .toast(t!("toast-video-shape", shape = name), None, now);
        self.scene = None;
        self.redraw = true;
    }

    /// Crops the picture to a shape (black bars cut off): the shape's middle fills the view.
    fn player_crop(&mut self) {
        self.player.crop = (self.player.crop + 1) % SHAPES.len();
        let (w, h) = self.view.stored_size();
        let (dw, dh) = self.view.display_size();
        let now = self.now();
        match SHAPES[self.player.crop] {
            Some((ar, name)) if w > 0 && h > 0 => {
                // The part of the shown picture of that shape, in stored pixels.
                let (cw, ch) = if dw / dh > ar {
                    (dh * ar, dh)
                } else {
                    (dw, dw / ar)
                };
                let (sx, sy) = (w as f32 / dw, h as f32 / dh);
                let (rw, rh) = (cw * sx, ch * sy);
                let r = ((w as f32 - rw) * 0.5, (h as f32 - rh) * 0.5, rw, rh);
                self.view.show_stored_rect(r, None);
                self.chrome
                    .toast(t!("toast-video-crop", shape = name), None, now);
            }
            _ => {
                self.view.zoom_fit();
                self.chrome.toast(t!("toast-video-crop-off"), None, now);
            }
        }
        self.after_view_change();
        self.scene = None;
        self.redraw = true;
    }

    /// Jumps by `secs` (negative: back).
    fn player_seek_by(&mut self, secs: f64) {
        if self.cast_seek_by(secs) {
            return;
        }
        // A stream's piece: the seek may land in another piece.
        if self.stream_seek_by(secs) {
            return;
        }
        if let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) {
            a.seek_by(
                Duration::from_secs_f64(secs.abs()),
                secs > 0.0,
                Instant::now(),
            );
        }
        self.tick_animation();
        self.redraw = true;
    }

    /// The player's actions (keys, palette). `false` for other actions.
    pub(super) fn player_action(&mut self, action: Action) -> bool {
        match action {
            Action::SeekForwardLong => self.player_seek_by(10.0),
            Action::SeekBackLong => self.player_seek_by(-10.0),
            Action::SeekPercent(n) => {
                self.video_seek(f64::from(n) / 100.0, true);
            }
            Action::CycleSubtitles => {
                let n = self.player.sources.len();
                let next = match self.player.sub {
                    None if n > 0 => Some(0),
                    Some(i) if i + 1 < n => Some(i + 1),
                    _ => None,
                };
                if n == 0 {
                    let now = self.now();
                    self.chrome.toast(t!("toast-subtitles-none"), None, now);
                } else {
                    self.player_choose_subtitle(next, true);
                }
            }
            Action::OpenSubtitle => self.player_open_dialog(),
            Action::GenerateSubtitles => self.stt_offer(),
            Action::SaveSubtitles => self.stt_save(),
            Action::SubtitleEarlier => self.player_sub_delay(Some(-STEP_MS)),
            Action::SubtitleLater => self.player_sub_delay(Some(STEP_MS)),
            Action::SubtitleEarlierFine => self.player_sub_delay(Some(-FINE_MS)),
            Action::SubtitleLaterFine => self.player_sub_delay(Some(FINE_MS)),
            Action::SubtitleStepPrev => self.player_sub_step(false),
            Action::SubtitleStepNext => self.player_sub_step(true),
            Action::SubtitleUp
            | Action::SubtitleDown
            | Action::SubtitleBigger
            | Action::SubtitleSmaller
            | Action::SubtitleResetPlace => self.player_sub_look(action),
            Action::SubtitleStyle => self.chrome.open_settings_at(SUBTITLE_STYLE_AT),
            Action::DialogueBoost => self.sound_fx_dialogue(),
            Action::Equaliser => self.eq_toggle_panel(),
            Action::SoundEarlier => self.player_sound_delay(Some(-STEP_MS)),
            Action::SoundLater => self.player_sound_delay(Some(STEP_MS)),
            Action::SoundEarlierFine => self.player_sound_delay(Some(-FINE_MS)),
            Action::SoundLaterFine => self.player_sound_delay(Some(FINE_MS)),
            Action::ResetDelays => {
                self.player.sub_delay_ms = 0;
                self.player_sound_delay(None);
                let now = self.now();
                self.chrome.toast(t!("toast-delays-reset"), None, now);
            }
            Action::CycleSound => {
                let Some(x) = self.player.extras.clone() else {
                    return true;
                };
                if x.audio.len() < 2 {
                    let now = self.now();
                    self.chrome.toast(t!("toast-sound-one-track"), None, now);
                    return true;
                }
                let cur = self
                    .player
                    .sound
                    .and_then(|id| x.audio.iter().position(|a| a.id == id))
                    .or_else(|| x.audio.iter().position(|a| a.default))
                    .unwrap_or(0);
                let next = x.audio[(cur + 1) % x.audio.len()].id;
                self.player_choose_sound(Some(next), true);
            }
            Action::PrevChapter => self.player_chapter(false),
            Action::NextChapter => self.player_chapter(true),
            Action::AbLoop => self.player_ab(),
            Action::SaveFrame => self.frame_save(),
            Action::CopyFrame => self.frame_copy(),
            Action::MiniPlayer => self.mini_toggle(),
            Action::CycleAspect => self.player_aspect(),
            Action::CycleCrop => self.player_crop(),
            _ => return false,
        }
        true
    }

    /// A choice in the player's menus.
    pub(super) fn player_command(&mut self, cmd: PlayerCmd) {
        match cmd {
            PlayerCmd::Subtitle(i) => self.player_choose_subtitle(i, true),
            PlayerCmd::OpenSubtitle => self.player_open_dialog(),
            PlayerCmd::GenerateSubtitles => self.stt_offer(),
            PlayerCmd::SaveSubtitles => self.stt_save(),
            PlayerCmd::SubtitleCharset => {
                let list = specular_subs::charset::CHOICES;
                let cur = self.player.charset.or(self.player.read_as);
                let next = cur
                    .and_then(|c| list.iter().position(|e| *e == c))
                    .map_or(list[0], |i| list[(i + 1) % list.len()]);
                self.player.charset = Some(next);
                let i = self.player.sub;
                self.player.track = None;
                self.player.sub = None;
                self.player.charset = Some(next);
                self.player_choose_subtitle(i, false);
                self.player_save();
            }
            PlayerCmd::SubtitleStyle => self.chrome.open_settings_at(SUBTITLE_STYLE_AT),
            PlayerCmd::SubtitleDelay(0) => self.player_sub_delay(None),
            PlayerCmd::SubtitleDelay(ms) => self.player_sub_delay(Some(ms)),
            PlayerCmd::SoundDelay(0) => self.player_sound_delay(None),
            PlayerCmd::SoundDelay(ms) => self.player_sound_delay(Some(ms)),
            PlayerCmd::DialogueBoost => self.sound_fx_dialogue(),
            PlayerCmd::Mono => self.sound_fx_mono(),
            PlayerCmd::Equaliser => self.eq_toggle_panel(),
            PlayerCmd::Sound(i) => {
                if let Some(id) = self
                    .player
                    .extras
                    .as_ref()
                    .and_then(|x| x.audio.get(i))
                    .map(|a| a.id)
                {
                    self.player_choose_sound(Some(id), true);
                }
            }
            PlayerCmd::Chapter(i) => {
                if let Some(x) = self.player.extras.clone() {
                    let start = x.picture_start_us;
                    self.player_go_chapter(i, &x, start);
                }
            }
            PlayerCmd::Speed(s) => self.player_speed(s),
            PlayerCmd::Picture(k, step) => self.player_picture(Some((k, step))),
            PlayerCmd::PictureReset => self.player_picture(None),
            PlayerCmd::OpenOutput => self.outdev_refresh(),
            PlayerCmd::Output(i) => self.outdev_choose(i),
            PlayerCmd::Speakers(i) => {
                if let Some(&c) = super::outdev::CHANNELS.get(i) {
                    self.outdev_channels(c);
                }
            }
            PlayerCmd::CastTo(_) | PlayerCmd::CastStop | PlayerCmd::CastSearch => {
                self.cast_command(&cmd);
            }
            PlayerCmd::Quality(q) => self.stream_choose_quality(q),
            // The chrome's own (it opens them), or sent on as the action itself.
            PlayerCmd::Action(_)
            | PlayerCmd::GoToTime
            | PlayerCmd::OpenPicture
            | PlayerCmd::OpenCast
            | PlayerCmd::CastNote => {}
        }
        self.redraw = true;
    }

    /// "Start over" on the resumed toast. True when `key` was it.
    pub(super) fn player_toast_action(&mut self, key: u64) -> bool {
        if key != START_OVER_KEY {
            return false;
        }
        self.chrome.dismiss_undo(key);
        if let Some(a) = self.anim.as_mut() {
            a.seek_time(Duration::ZERO);
        }
        self.tick_animation();
        self.redraw = true;
        true
    }

    /// "Forget playback positions…" in Settings: asks first.
    pub(super) fn player_forget_ask(&mut self) {
        let n = self.player.store().positions();
        self.chrome.confirm_with_detail(
            t!("player-forget-ask", count = n as i64),
            t!("player-forget-detail"),
            t!("player-forget-ok"),
            super::items::PLAYBACK_FORGET,
        );
        self.redraw = true;
    }

    /// The confirmation's accept. True when `id` was ours.
    pub(super) fn player_item(&mut self, id: u64) -> bool {
        if self.stt_item(id) {
            return true;
        }
        if id != super::items::PLAYBACK_FORGET {
            return false;
        }
        self.player.pending_resume = None;
        let n = self.player.store().forget_positions();
        let now = self.now();
        self.chrome
            .toast(t!("toast-playback-forgotten", count = n as i64), None, now);
        true
    }

    /// What the player menus offer for the movie on screen.
    pub(super) fn player_view(&self) -> Option<PlayerView> {
        let a = self
            .anim
            .as_ref()
            .filter(|a| Some(a.id) == self.current_id())?;
        let m = a.movie.as_ref()?;
        let v = a.playback().video?;
        let (qualities, quality) = self.stream_qualities();
        let extras = self.player.extras.as_deref();
        let dur = v.duration.max(1e-6);
        let start = extras.map_or(0, |x| x.picture_start_us);
        let lang = specular_i18n::I18n::global().language().to_string();
        let chapters = extras
            .map(|x| {
                x.chapters
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let s = ((c.start_us - start).max(0) as f64) / 1e6;
                        let name = c.name(&lang).map_or_else(
                            || t!("player-chapter-number", number = i as i64 + 1),
                            str::to_owned,
                        );
                        ((s / dur).clamp(0.0, 1.0), s, name)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let sounds: Vec<String> = extras
            .map(|x| {
                x.audio
                    .iter()
                    .enumerate()
                    .map(|(i, t)| sound_label(t, i))
                    .collect()
            })
            .unwrap_or_default();
        let sound = extras
            .and_then(|x| {
                self.player
                    .sound
                    .and_then(|id| x.audio.iter().position(|a| a.id == id))
                    .or_else(|| x.audio.iter().position(|a| a.default))
            })
            .unwrap_or(0);
        let (outputs, output) = self.outdev_menu();
        let speaker = super::outdev::CHANNELS
            .iter()
            .position(|c| *c == self.settings.viewing.sound_channels)
            .unwrap_or(0);
        let text_file = self.player.sub.and_then(|i| self.player.sources.get(i)).is_some_and(|s| {
            matches!(&s.origin, subs::Origin::File(p) if specular_subs::FileKind::of(p).is_some_and(specular_subs::FileKind::is_text))
        });
        Some(PlayerView {
            subtitles: self
                .player
                .sources
                .iter()
                .enumerate()
                .map(|(i, s)| s.label(i))
                .collect(),
            subtitle: self.player.sub,
            subtitle_delay_ms: self.player.sub_delay_ms,
            charset: text_file
                .then(|| {
                    self.player
                        .charset
                        .or(self.player.read_as)
                        .map(|e| e.name().to_owned())
                })
                .flatten(),
            sounds,
            sound,
            sound_delay_ms: self.player.sound_delay_ms,
            chapters,
            ab: (
                self.player.ab.0.map(|t| t / dur),
                self.player.ab.1.map(|t| t / dur),
            ),
            preview: self.player.preview.view(),
            has_sound: m.has_sound,
            dialogue: self.settings.viewing.sound_dialogue_boost,
            mono: self.settings.viewing.sound_mono,
            mini: self.player.mini.active(),
            generate: Some(self.stt_row()),
            save_subtitles: self.stt_chosen(),
            speed: a.speed(),
            picture_menu: self.video_picture_current(),
            picture: self.player.picture.0,
            zoomed: !self.view.is_at_fit(),
            outputs,
            output,
            speakers: super::outdev::CHANNELS
                .map(super::outdev::speakers_text)
                .to_vec(),
            speaker,
            cast: self.cast_view(),
            qualities,
            quality,
        })
    }

    /// The subtitle cues showing now.
    pub(super) fn player_cues(&self) -> Vec<specular_subs::Cue> {
        let Some(track) = &self.player.track else {
            return Vec::new();
        };
        if self.player_movie().is_none() {
            return Vec::new();
        }
        let t = self.player_media_us(self.player_time());
        subs::active(track, t, self.player.sub_delay_ms * 1000)
            .into_iter()
            .cloned()
            .collect()
    }
}

/// A sound track's menu label: its language, name and channels.
pub(super) fn sound_label(t: &specular_video::extras::TrackInfo, i: usize) -> String {
    let lang = t.lang.as_deref().map(|l| {
        let base = l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase();
        subs::language_name(specular_video::extras::iso639_2(&base).unwrap_or(&base))
    });
    let mut s = match (lang, &t.name) {
        (Some(l), Some(n)) => format!("{l} – {n}"),
        (Some(l), None) => l,
        (None, Some(n)) => n.clone(),
        (None, None) => t!("player-track-number", number = i as i64 + 1),
    };
    if t.visual_impaired && !t.name.as_deref().is_some_and(subs::names_description) {
        s = t!("player-track-description", track = s);
    }
    if t.hearing_impaired {
        s = t!("player-track-hearing", track = s);
    }
    let ch = match t.channels {
        0 | 2 => None,
        1 => Some(t!("player-channels-mono")),
        6 => Some("5.1".to_owned()),
        8 => Some("7.1".to_owned()),
        n => Some(t!("player-channels", count = i64::from(n))),
    };
    if let Some(c) = ch {
        s = format!("{s} ({c})");
    }
    s
}

/// A decoder's error in plain words.
fn subs_reason(why: &str) -> String {
    if let Some(name) = why.strip_prefix("unsupported: ") {
        t!("subtitles-reason-unsupported", kind = name.to_owned())
    } else if why.starts_with("malformed") {
        t!("subtitles-reason-damaged")
    } else {
        t!("subtitles-reason-unreadable", reason = why.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_subtitle_files() {
        let dir = std::env::temp_dir().join(format!("specular-subdrop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let srt = dir.join("Movie.en.SRT");
        assert_eq!(dropped_subtitle(true, &srt), Some(srt.clone()));
        // No movie on screen: opened as any file.
        assert_eq!(dropped_subtitle(false, &srt), None);
        assert_eq!(dropped_subtitle(true, &dir.join("photo.jpg")), None);
        // A .sub alone is MicroDVD text; a VobSub .sub stands for the .idx beside it.
        let sub = dir.join("m.sub");
        assert_eq!(dropped_subtitle(true, &sub), Some(sub.clone()));
        std::fs::write(dir.join("m.idx"), b"# VobSub index file, v7").unwrap();
        assert_eq!(dropped_subtitle(true, &sub), Some(dir.join("m.idx")));
        let _ = std::fs::remove_dir_all(dir);
    }
}

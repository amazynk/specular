//! Playback of animated images (GIF, APNG, WebP, FLI/FLC, ANI and any other decoder whose
//! session has `animation()`/`frames()`): one `specular_anim::Player` for the current image,
//! ticked from the event loop. Frames go to the image's overview texture; animated images
//! skip the tile pyramid.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use specular_anim::{
    DelayPolicy, FrameRef, LoopMode, MAX_SPEED, MIN_SPEED, Player, PlayerConfig, Waker,
};
use specular_decode::Session;
use specular_settings::{AnimationLoop, Settings, ShortDelays};
use specular_video::audio::{AudioPlayer, Silence};
use specular_widgets::{Playback, VideoPlayback};

use crate::loader::ImageId;

mod present;
pub use present::PresentTrace;

/// Speeds the faster/slower keys step through.
const SPEEDS: &[f64] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

/// A movie's speeds: 0.5× to 2×.
pub const VIDEO_MIN_SPEED: f64 = 0.5;
pub const VIDEO_MAX_SPEED: f64 = 2.0;

/// How far the seek keys jump.
pub const SEEK_STEP: Duration = Duration::from_secs(5);

/// Movie player settings: frame times exactly as stored (a 120 fps movie's 8 ms frames are
/// not "short delays"), played once unless the loop toggle is on.
pub fn video_config(s: &Settings, looping: bool) -> PlayerConfig {
    PlayerConfig {
        delay_policy: DelayPolicy::AS_STORED,
        loop_mode: if looping {
            LoopMode::Forever
        } else {
            LoopMode::Times(1)
        },
        ..config(s)
    }
}

/// The top of the sound's gain control (100 %).
pub const VOLUME_MAX: f32 = 1.0;

/// The volume setting (percent) as the sound's gain control (0..=1).
pub fn volume(s: &Settings) -> f32 {
    (s.viewing.video_volume.clamp(0, 100) as f32) / 100.0
}

/// A time moved by a signed number of microseconds (not below zero).
pub fn shift(t: Duration, us: i64) -> Duration {
    if us >= 0 {
        t + Duration::from_micros(us as u64)
    } else {
        t.saturating_sub(Duration::from_micros(us.unsigned_abs()))
    }
}

/// What a movie adds to an animation: its sound and looping.
pub struct Movie {
    /// The sound, once its device is open.
    pub sound: Option<AudioPlayer>,
    /// The sound's device being opened (off the UI thread).
    opening: Option<mpsc::Receiver<Result<AudioPlayer, Silence>>>,
    /// Why the movie plays silently, until it is said (then `said`).
    pub silence: Option<Silence>,
    pub said: bool,
    /// The movie has a sound track that can play (the controls show).
    pub has_sound: bool,
    /// A picture plays (false for a sound file, whose picture is its cover and waveform):
    /// the transport offers full-screen play.
    pub picture: bool,
    pub looping: bool,
    pub volume: f32,
    pub muted: bool,
    /// The loop the sound was started in (a new loop restarts it).
    lap: u64,
    /// The player follows the sound's clock.
    clock_set: bool,
    /// The sound plays this much later than the picture (negative: earlier), microseconds.
    pub delay_us: Arc<AtomicI64>,
    /// The sound fades in when it starts (it moved from another device).
    fade_in: bool,
}

impl Movie {
    /// A movie whose sound (if any) `open` opens on another thread.
    pub fn new(
        has_sound: bool,
        looping: bool,
        volume: f32,
        muted: bool,
        open: Option<Box<dyn FnOnce() -> Result<AudioPlayer, Silence> + Send>>,
    ) -> Movie {
        let opening = open.and_then(|open| {
            let (tx, rx) = mpsc::channel();
            specular_thread::builder("anim-sound-open")
                .spawn(move || {
                    let _ = tx.send(open());
                })
                .ok()
                .map(|_| rx)
        });
        Movie {
            sound: None,
            opening,
            silence: None,
            said: false,
            has_sound,
            picture: true,
            looping,
            volume,
            muted,
            lap: 0,
            clock_set: false,
            delay_us: Arc::new(AtomicI64::new(0)),
            fade_in: false,
        }
    }

    /// Picks up the sound once its device is open (or why it is not).
    fn poll(&mut self) {
        let Some(rx) = &self.opening else { return };
        match rx.try_recv() {
            Ok(Ok(s)) => {
                s.set_volume(self.volume);
                s.set_muted(self.muted);
                if self.fade_in {
                    s.fade_in();
                }
                self.sound = Some(s);
                self.opening = None;
            }
            Ok(Err(why)) => {
                self.silence = Some(why);
                self.opening = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.opening = None,
        }
    }
}

pub fn loop_mode(l: AnimationLoop) -> LoopMode {
    match l {
        AnimationLoop::File => LoopMode::File,
        AnimationLoop::Forever => LoopMode::Forever,
        AnimationLoop::Once => LoopMode::Times(1),
    }
}

pub fn delay_policy(d: ShortDelays) -> DelayPolicy {
    match d {
        ShortDelays::Browser => DelayPolicy::BROWSER,
        ShortDelays::AsStored => DelayPolicy::AS_STORED,
    }
}

/// Player settings. Autoplay follows its own setting only: "Reduce motion" covers the
/// interface's transitions, not the image's content.
pub fn config(s: &Settings) -> PlayerConfig {
    PlayerConfig {
        autoplay: s.viewing.animation_autoplay,
        loop_mode: loop_mode(s.viewing.animation_loop),
        delay_policy: delay_policy(s.viewing.animation_short_delays),
        ..PlayerConfig::default()
    }
}

/// The next speed up (`faster`) or down from `speed`.
pub fn step_speed(speed: f64, faster: bool) -> f64 {
    let next = if faster {
        SPEEDS.iter().copied().find(|&s| s > speed + 1e-6)
    } else {
        SPEEDS.iter().rev().copied().find(|&s| s < speed - 1e-6)
    };
    next.unwrap_or(speed).clamp(MIN_SPEED, MAX_SPEED)
}

/// The decode scale for frames: the coarsest the decoder offers that is still sharp at fit
/// (`need`, from `needed_scale`).
pub fn pick_scale(scales: &[u32], need: u32) -> u32 {
    scales
        .iter()
        .copied()
        .filter(|&s| s >= 1 && s <= need)
        .max()
        .unwrap_or(1)
}

/// Frame pacing: how late frames reach the screen relative to their deadline.
#[derive(Clone, Debug, Default)]
pub struct Pacing {
    pub frames: u64,
    pub late_sum: Duration,
    pub late_max: Duration,
    /// Frames more than 4 ms late.
    pub late_4ms: u64,
}

impl Pacing {
    fn record(&mut self, late: Duration) {
        self.frames += 1;
        self.late_sum += late;
        self.late_max = self.late_max.max(late);
        if late > Duration::from_millis(4) {
            self.late_4ms += 1;
        }
    }

    pub fn mean_ms(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.late_sum.as_secs_f64() * 1e3 / self.frames as f64
        }
    }
}

/// The animation of the current image.
pub struct Anim {
    pub id: ImageId,
    /// Frames are decoded at 1/`scale`.
    pub scale: u32,
    player: Player,
    /// The deadline the last tick asked for.
    deadline: Option<Instant>,
    /// Frame on screen.
    frame: Option<u32>,
    pub pacing: Pacing,
    pub dropped: u64,
    /// Shown frames followed to their present.
    pub present: PresentTrace,
    /// A movie's sound and controls.
    pub movie: Option<Movie>,
    /// Hidden (the gallery is over it): the sound pauses.
    pub hidden: bool,
    /// Playback stopped for memory and the user was told.
    memory_said: bool,
    /// A sound moved away from its device, fading out until this instant.
    retiring: Option<(AudioPlayer, Instant)>,
}

impl Anim {
    /// A player for `session` when it is animated. `at`: start on this frame (a re-created
    /// player after a scale change), `speed`: keep the user's speed.
    pub fn new(
        id: ImageId,
        session: &dyn Session,
        scale: u32,
        cfg: PlayerConfig,
        waker: Waker,
    ) -> Result<Option<Anim>, String> {
        // A sound file's page (`app/sound_page.rs`) draws itself and its playhead on the GPU:
        // its player takes the timeline only, nothing drawn per frame.
        if let (Some(info), Some(sound)) = (
            session.animation(),
            session
                .as_any()
                .and_then(|a| a.downcast_ref::<specular_video::sound_view::SoundSession>()),
        ) {
            let p = Player::new(sound.clock_frames(), info, cfg, Some(waker));
            return Ok(Some(Anim::with_player(id, scale, p)));
        }
        let player =
            Player::from_session(session, scale, cfg, Some(waker)).map_err(|e| e.to_string())?;
        Ok(player.map(|p| Anim::with_player(id, scale, p)))
    }

    pub fn with_player(id: ImageId, scale: u32, player: Player) -> Anim {
        Anim {
            id,
            scale,
            player,
            deadline: None,
            frame: None,
            pacing: Pacing::default(),
            dropped: 0,
            present: PresentTrace::default(),
            movie: None,
            hidden: false,
            memory_said: false,
            retiring: None,
        }
    }

    /// Makes this a movie: the player follows the sound's clock once it plays.
    pub fn set_movie(&mut self, movie: Movie) {
        self.movie = Some(movie);
    }

    /// Starts, stops or restarts the sound so it matches the picture: it plays while the
    /// movie plays and is shown, at the movie's speed; a seek, a step or a new loop restarts
    /// it where the picture is.
    fn sync_sound(&mut self, now: Instant) {
        let Some(m) = &mut self.movie else { return };
        m.poll();
        let Some(s) = &m.sound else { return };
        if !m.clock_set {
            // The picture follows the sound heard, moved by the sound's delay: sound `d`
            // later means the picture is `d` ahead of what is heard.
            let heard = s.clock();
            let d = Arc::clone(&m.delay_us);
            let clock: Arc<dyn Fn(Instant) -> Option<Duration> + Send + Sync> =
                Arc::new(move |now| heard(now).map(|h| shift(h, d.load(Ordering::Relaxed))));
            self.player.set_clock(Some(clock));
            m.clock_set = true;
        }
        let p = self.player.position();
        // The sound keeps its pitch at any speed and its clock runs at the speed.
        s.set_speed(p.speed);
        // A sound file whose next file is primed plays on into it past its end (gapless).
        let playing = p.playing && !p.finished || p.finished && s.has_next();
        // Silent while the seek bar is dragged.
        let want = playing && p.pending.is_none() && !p.scrubbing && !self.hidden;
        if !want {
            if s.is_playing() {
                s.pause();
            }
            return;
        }
        if (!s.is_playing() || p.lap != m.lap)
            && let Some(t) = self.player.time(now)
        {
            s.play(shift(t, -m.delay_us.load(Ordering::Relaxed)));
            m.lap = p.lap;
        }
    }

    /// The picture jumped (seek, step, play from the start): the sound starts again where
    /// it lands.
    fn sound_jumped(&mut self) {
        if let Some(s) = self.movie.as_ref().and_then(|m| m.sound.as_ref()) {
            s.pause();
        }
    }

    /// Seeks to `t` from the start (clamped to the movie), keeping the play state.
    pub fn seek_time(&mut self, t: Duration) {
        if let Some(i) = self.player.index_at(t) {
            self.player.seek(i);
            self.deadline = None;
            self.present.reset_run();
            self.sound_jumped();
        }
    }

    /// Seeks by `by` forwards or backwards from where playback is.
    pub fn seek_by(&mut self, by: Duration, forward: bool, now: Instant) {
        let at = self.player.time(now).unwrap_or_default();
        let t = if forward {
            at + by
        } else {
            at.saturating_sub(by)
        };
        let end = self.player.duration().unwrap_or(t);
        self.seek_time(t.min(end));
    }

    /// Seeks to `fraction` of the movie's length (the seek bar let go, or clicked): the exact
    /// frame there, ending a drag.
    pub fn seek_fraction(&mut self, fraction: f64) {
        if let Some(d) = self.player.duration() {
            self.seek_time(d.mul_f64(fraction.clamp(0.0, 1.0)));
        }
    }

    /// The seek bar is dragged to `fraction` of the movie's length: the picture follows the
    /// pointer (the nearest frame decoded so far until the one there is), silently, and
    /// playback pauses until it is let go ([`Anim::seek_fraction`]).
    pub fn scrub_fraction(&mut self, fraction: f64) {
        let Some(d) = self.player.duration() else {
            return;
        };
        if let Some(i) = self.player.index_at(d.mul_f64(fraction.clamp(0.0, 1.0))) {
            self.player.scrub(i);
            self.deadline = None;
            self.present.reset_run();
            self.sound_jumped();
        }
    }

    /// The sound `us` microseconds later than the picture (negative: earlier); it starts
    /// again where the new delay puts it.
    pub fn set_sound_delay(&mut self, us: i64) {
        if let Some(m) = &self.movie {
            m.delay_us.store(us, Ordering::Relaxed);
        }
        self.sound_jumped();
    }

    /// The sound moves to another device: `movie` (opening it there) takes its place and
    /// fades in where the picture is, while the old sound fades out on its device (kept
    /// until it has; no click on either).
    pub fn move_sound(&mut self, mut movie: Movie) {
        let old = self.movie.as_mut().and_then(|m| m.sound.take());
        movie.fade_in = true;
        self.replace_sound(movie);
        if let Some(s) = old {
            s.fade_out();
            // The fade, and the device's buffer after it.
            let until = Instant::now() + specular_video::audio::FADE + Duration::from_millis(120);
            self.retiring = Some((s, until));
        }
    }

    /// Another sound track: `movie` (opening it) takes the place of the movie's sound, with
    /// the volume, mute and delay as they were.
    pub fn replace_sound(&mut self, mut movie: Movie) {
        if let Some(old) = self.movie.take() {
            movie.volume = old.volume;
            movie.muted = old.muted;
            movie.looping = old.looping;
            movie.delay_us = old.delay_us;
            movie.picture = old.picture;
        }
        self.player.set_clock(None);
        self.movie = Some(movie);
    }

    /// Takes the sound out (it plays on): the next file's player keeps it, so the sound
    /// goes on without a gap.
    pub fn take_sound(&mut self) -> Option<AudioPlayer> {
        let m = self.movie.as_mut()?;
        self.player.set_clock(None);
        m.clock_set = false;
        m.sound.take()
    }

    /// The sound, once it is open.
    pub fn sound(&self) -> Option<&AudioPlayer> {
        self.movie.as_ref()?.sound.as_ref()
    }

    /// The time on screen.
    pub fn time(&self, now: Instant) -> Option<Duration> {
        self.player.time(now)
    }

    /// The movie has played to its end (and does not loop).
    pub fn finished(&self) -> bool {
        self.player.position().finished
    }

    /// The sound's volume and mute, as set.
    pub fn set_volume(&mut self, volume: f32, muted: bool) {
        if let Some(m) = &mut self.movie {
            m.volume = volume;
            m.muted = muted;
            if let Some(s) = &m.sound {
                s.set_volume(volume);
                s.set_muted(muted);
            }
        }
    }

    /// Why the movie plays silently, the first time it is asked (to say it once): no device,
    /// or the device went away.
    pub fn take_silence(&mut self) -> Option<Silence> {
        let m = self.movie.as_mut()?;
        if let Some(s) = &m.sound
            && s.take_lost()
        {
            return Some(Silence::NoDevice("lost".into()));
        }
        if let Some(why) = m.sound.as_ref().and_then(|s| s.take_failed()) {
            return Some(Silence::Failed(why));
        }
        if m.said {
            return None;
        }
        let why = m.silence.clone()?;
        m.said = true;
        Some(why)
    }

    /// How much free RAM a frame needed, the first time playback stops for it (to say it
    /// once).
    pub fn take_needs_memory(&mut self) -> Option<specular_core::NeedsMemory> {
        if self.memory_said {
            return None;
        }
        let n = self.player.needs_memory()?;
        self.memory_said = true;
        Some(n)
    }

    /// Advances to `now`: the frame to show (if it changed) and when to tick next.
    pub fn tick(&mut self, now: Instant) -> (Option<FrameRef>, Option<Instant>) {
        let r = self.player.tick(now);
        self.sync_sound(now);
        if self.retiring.as_ref().is_some_and(|r| now >= r.1) {
            self.retiring = None;
        }
        self.dropped += r.dropped;
        if let Some(f) = &r.show {
            // Lateness only while playing: a step or seek has no deadline.
            if self.player.is_playing()
                && let Some(d) = self.deadline
            {
                self.pacing.record(now.saturating_duration_since(d));
            }
            if self.player.is_playing() {
                let due = r.due.unwrap_or(now);
                // A new schedule (start, resume after a wait, seek): a new drift run.
                if self.deadline.is_none() || due == now {
                    self.present.reset_run();
                }
                let cfg = self.player.config();
                let eff = cfg.delay_policy.apply(f.delay).div_f64(self.player.speed());
                self.present.shown(f.index, due, eff, r.dropped);
            } else {
                self.present.reset_run();
            }
            self.frame = Some(f.index);
        }
        self.deadline = r.next_deadline;
        (r.show, r.next_deadline)
    }

    pub fn player(&mut self) -> &mut Player {
        &mut self.player
    }

    pub fn frame(&self) -> Option<u32> {
        self.frame
    }

    /// What the transport bar shows.
    pub fn playback(&self) -> Playback {
        let p = self.player.position();
        let video = self.movie.as_ref().map(|m| {
            let now = Instant::now();
            VideoPlayback {
                position: self.player.time(now).unwrap_or_default().as_secs_f64(),
                duration: self.player.duration().unwrap_or_default().as_secs_f64(),
                looping: m.looping,
                sound: m.has_sound.then_some((m.volume, m.muted)),
                volume_max: VOLUME_MAX,
                picture: m.picture,
            }
        });
        Playback {
            frame: self.frame.or(p.index).unwrap_or(0) as usize,
            count: p.count,
            playing: p.playing && !p.finished,
            speed: p.speed,
            video,
        }
    }

    pub fn is_playing(&self) -> bool {
        let p = self.player.position();
        p.playing && !p.finished
    }

    /// Play/pause; at the end of a finite loop, play starts over.
    pub fn toggle(&mut self, now: Instant) {
        let p = self.player.position();
        if p.finished {
            self.player.seek(0);
            self.player.play(now);
        } else {
            self.player.toggle(now);
        }
        self.deadline = None;
        self.present.reset_run();
        self.sound_jumped();
    }

    pub fn step(&mut self, forward: bool) {
        if forward {
            self.player.step_forward();
        } else {
            self.player.step_back();
        }
        self.deadline = None;
        self.present.reset_run();
        self.sound_jumped();
    }

    pub fn set_speed(&mut self, speed: f64, now: Instant) {
        self.player.set_speed(speed, now);
        self.deadline = None;
        self.present.reset_run();
    }

    pub fn speed(&self) -> f64 {
        self.player.speed()
    }

    /// The frame shown last went to the compositor.
    pub fn presented(&mut self, now: Instant) {
        self.present.presented(now);
    }

    pub fn shutdown(self) -> (Pacing, u64) {
        let (p, d) = (self.pacing.clone(), self.dropped);
        self.player.shutdown();
        (p, d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_anim::{FakeFrames, FakeSpec};
    use specular_decode::AnimationInfo;
    use std::sync::Arc;

    fn fake(count: u32, delay_ms: u64, cfg: PlayerConfig) -> Anim {
        let (frames, _probe) =
            FakeFrames::new(FakeSpec::new(count, Duration::from_millis(delay_ms), 8, 8));
        let info = AnimationInfo {
            frame_count: Some(count),
            loop_count: 0,
        };
        let waker: Waker = Arc::new(|| {});
        Anim::with_player(7, 1, Player::new(Box::new(frames), &info, cfg, Some(waker)))
    }

    /// Ticks until a frame shows (the worker decodes in the background).
    fn first_frame(a: &mut Anim) -> u32 {
        let deadline = specular_log::timing::Deadline::new("no frame", Duration::from_secs(5));
        loop {
            let (show, _) = a.tick(Instant::now());
            if let Some(f) = show {
                return f.index;
            }
            deadline.check();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn settings_map_to_the_player() {
        let mut s = Settings::default();
        assert!(config(&s).autoplay);
        s.viewing.animation_autoplay = false;
        assert!(!config(&s).autoplay);
        s.viewing.animation_loop = AnimationLoop::Once;
        s.viewing.animation_short_delays = ShortDelays::AsStored;
        let c = config(&s);
        assert_eq!(c.loop_mode, LoopMode::Times(1));
        assert_eq!(c.delay_policy, DelayPolicy::AS_STORED);
        assert_eq!(loop_mode(AnimationLoop::Forever), LoopMode::Forever);
        assert_eq!(delay_policy(ShortDelays::Browser), DelayPolicy::BROWSER);
    }

    #[test]
    fn speed_steps_stay_in_range() {
        assert_eq!(step_speed(1.0, true), 1.25);
        assert_eq!(step_speed(1.0, false), 0.75);
        assert_eq!(step_speed(4.0, true), 4.0);
        assert_eq!(step_speed(0.25, false), 0.25);
        assert_eq!(step_speed(1.1, true), 1.25);
        assert_eq!(step_speed(1.1, false), 1.0);
    }

    #[test]
    fn frame_scale_is_the_coarsest_sharp_one() {
        assert_eq!(pick_scale(&[1, 2, 4, 8], 4), 4);
        assert_eq!(pick_scale(&[1], 8), 1);
        assert_eq!(pick_scale(&[1, 2], 1), 1);
        assert_eq!(pick_scale(&[], 2), 1);
    }

    #[test]
    fn autoplay_off_shows_frame_zero_paused_and_steps() {
        let cfg = PlayerConfig {
            autoplay: false,
            ..PlayerConfig::default()
        };
        let mut a = fake(5, 40, cfg);
        assert_eq!(first_frame(&mut a), 0);
        let pb = a.playback();
        assert_eq!((pb.frame, pb.count, pb.playing), (0, Some(5), false));
        a.step(true);
        assert_eq!(first_frame(&mut a), 1);
        a.step(false);
        assert_eq!(first_frame(&mut a), 0);
        a.toggle(Instant::now());
        assert!(a.is_playing());
        a.set_speed(step_speed(a.speed(), true), Instant::now());
        assert_eq!(a.playback().speed, 1.25);
        a.shutdown();
    }

    #[test]
    fn movies_keep_their_frame_times_and_seek_by_time() {
        let s = Settings::default();
        let c = video_config(&s, false);
        assert_eq!(c.delay_policy, DelayPolicy::AS_STORED);
        assert_eq!(c.loop_mode, LoopMode::Times(1));
        assert_eq!(video_config(&s, true).loop_mode, LoopMode::Forever);
        assert_eq!(volume(&s), 1.0);
        // 250 frames of 8 ms (a 120 fps clip is not a GIF's "short delay").
        let mut spec = FakeSpec::new(250, Duration::from_millis(8), 8, 8);
        spec.seekable = true;
        let (frames, _probe) = FakeFrames::new(spec);
        let info = AnimationInfo {
            frame_count: Some(250),
            loop_count: 1,
        };
        let waker: Waker = Arc::new(|| {});
        let cfg = PlayerConfig {
            autoplay: false,
            ..c
        };
        let mut a = Anim::with_player(7, 1, Player::new(Box::new(frames), &info, cfg, Some(waker)));
        a.set_movie(Movie::new(false, false, 1.0, false, None));
        assert_eq!(first_frame(&mut a), 0);
        let pb = a.playback();
        let v = pb.video.expect("movie controls");
        assert!((v.duration - 2.0).abs() < 1e-9, "{v:?}");
        assert_eq!((v.position, v.looping, v.sound), (0.0, false, None));
        let now = Instant::now();
        a.seek_by(Duration::from_secs(1), true, now);
        assert_eq!(first_frame(&mut a), 125);
        a.seek_by(SEEK_STEP, true, now);
        assert_eq!(first_frame(&mut a), 249, "clamped to the last frame");
        a.seek_by(SEEK_STEP, false, now);
        assert_eq!(first_frame(&mut a), 0);
        a.seek_fraction(0.5);
        assert_eq!(first_frame(&mut a), 125);
        assert!((a.playback().video.unwrap().position - 1.0).abs() < 1e-9);
        // No sound, nothing to say.
        assert!(a.take_silence().is_none());
        a.shutdown();
    }

    #[test]
    fn a_movie_without_a_device_says_so_once() {
        let mut m = Movie::new(
            true,
            false,
            0.5,
            false,
            Some(Box::new(|| {
                Err(specular_video::audio::Silence::NoDevice("none".into()))
            })),
        );
        let t0 = Instant::now();
        while m.silence.is_none() && t0.elapsed() < Duration::from_secs(5) {
            m.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut a = fake(3, 40, PlayerConfig::default());
        a.set_movie(m);
        assert!(matches!(
            a.take_silence(),
            Some(specular_video::audio::Silence::NoDevice(_))
        ));
        assert!(a.take_silence().is_none(), "once");
        a.shutdown();
    }

    /// A WAV file: 16 kHz mono, 3 s of silence with a 50 ms beep at 1 s.
    fn beep_wav(dir: &std::path::Path) -> std::path::PathBuf {
        let rate = 16_000u32;
        let n = rate as usize * 3;
        let mut pcm = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f64 / f64::from(rate);
            let v = if (1.0..1.05).contains(&t) {
                (0.8 * (t * 2.0 * std::f64::consts::PI * 1000.0).sin() * 32767.0) as i16
            } else {
                0
            };
            pcm.extend_from_slice(&v.to_le_bytes());
        }
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        w.extend_from_slice(&pcm);
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join("beep.wav");
        std::fs::write(&p, w).unwrap();
        p
    }

    /// Plays a clip whose picture flashes at 1 s (frame 25 of 40 ms frames) with sound that
    /// beeps at 1 s, the sound `delay_ms` later, at `speed`, on a fake device: returns how
    /// far the picture is from the beep when it is heard (ms; the delay when in step).
    fn flash_to_beep(delay_ms: i64, speed: f64) -> i64 {
        let dir =
            std::env::temp_dir().join(format!("av-{}-{delay_ms}-{speed}", std::process::id()));
        let wav = beep_wav(&dir);
        let source = Arc::new(specular_core::Source::open(&wav).unwrap());
        let out = specular_video::audio::FakeOutput::new(16_000, 1);
        let dev = out.clone();
        let src = Arc::clone(&source);
        let m = Movie::new(
            true,
            false,
            1.0,
            false,
            Some(Box::new(move || {
                let movie =
                    specular_video::parse_movie(src.bytes(), true).map_err(|_| Silence::NoTrack)?;
                specular_video::audio::AudioPlayer::open(src, &movie, 0, Box::new(out))
            })),
        );
        m.delay_us.store(delay_ms * 1000, Ordering::Relaxed);
        let mut spec = FakeSpec::new(75, Duration::from_millis(40), 8, 8);
        spec.seekable = true;
        let (frames, _probe) = FakeFrames::new(spec);
        let info = AnimationInfo {
            frame_count: Some(75),
            loop_count: 1,
        };
        let waker: Waker = Arc::new(|| {});
        let cfg = PlayerConfig {
            autoplay: true,
            ..video_config(&Settings::default(), false)
        };
        let mut a = Anim::with_player(7, 1, Player::new(Box::new(frames), &info, cfg, Some(waker)));
        a.set_movie(m);
        // The sound opens on its own thread: the clock starts once it is open, so a slow
        // open on a loaded machine cannot run the picture past the beep first.
        let opening = specular_log::timing::Deadline::new("sound open", Duration::from_secs(5));
        loop {
            let m = a.movie.as_mut().unwrap();
            m.poll();
            if m.sound.is_some() {
                break;
            }
            assert!(m.opening.is_some(), "the sound did not open");
            opening.check();
            std::thread::sleep(Duration::from_millis(1));
        }
        let t0 = Instant::now();
        a.set_speed(speed, t0);
        // Until the sound is open and plays.
        let mut now = t0;
        let mut k = 0u64;
        let mut result = None;
        while k < 600 {
            now = t0 + Duration::from_millis(10 * k);
            assert!(a.player().wait_idle(Duration::from_secs(10)));
            a.tick(now);
            let playing = a
                .movie
                .as_ref()
                .and_then(|m| m.sound.as_ref())
                .is_some_and(|s| s.is_playing());
            if !playing {
                std::thread::sleep(Duration::from_millis(2));
                k += 1;
                continue;
            }
            // Wait for the decoder rather than starve the device.
            let before = dev.played.lock().len();
            let t_wait = Instant::now();
            loop {
                dev.played.lock().truncate(before);
                assert!(dev.pull(160, now));
                let filled = dev.played.lock()[before..].iter().any(|v| *v != 0.0);
                if filled || t_wait.elapsed() > Duration::from_millis(30) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let beep = dev.played.lock()[before..]
                .iter()
                .position(|v| v.abs() > 0.05);
            if let Some(i) = beep {
                // The beep is heard at `now` plus its place in this buffer.
                let heard = now + Duration::from_micros(i as u64 * 1_000_000 / 16_000);
                assert!(a.player().wait_idle(Duration::from_secs(10)));
                a.tick(heard);
                let shown = a.player().time(heard).unwrap_or_default().as_secs_f64();
                result = Some(((shown - 1.0) * 1000.0).round() as i64);
                break;
            }
            k += 1;
        }
        let _ = now;
        a.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
        result.expect("the beep was heard")
    }

    /// Sound delay: the picture keeps to the sound moved by the delay, both ways (a clip
    /// with a flash and a beep at the same time, measured where the beep is heard).
    #[test]
    fn sound_delay_moves_the_sound_against_the_picture() {
        for d in [0i64, 200, -200] {
            let off = flash_to_beep(d, 1.0);
            assert!(
                (off - d).abs() <= 45,
                "delay {d} ms: picture {off} ms from the beep"
            );
        }
    }

    /// Away from 1× the sound plays, time-stretched, and the picture keeps to it.
    #[test]
    fn sound_plays_in_step_at_other_speeds() {
        for speed in [0.5, 1.5, 2.0] {
            let off = flash_to_beep(0, speed);
            assert!(off.abs() <= 45, "{speed}×: picture {off} ms from the beep");
        }
    }

    #[test]
    fn playing_advances_frames_on_their_deadlines() {
        // Long frames: on a busy CI runner (3 cores, the whole suite in parallel) a sleeping
        // thread wakes up to ~100 ms late, which misses short deadlines, and a missed deadline
        // drops the frame, as it should.
        const FRAME_MS: u64 = 250;
        let mut a = fake(4, FRAME_MS, PlayerConfig::default());
        assert_eq!(first_frame(&mut a), 0);
        let t0 = Instant::now();
        let mut seen = vec![0];
        while seen.len() < 4 && t0.elapsed() < Duration::from_secs(5) {
            let (show, next) = a.tick(Instant::now());
            if let Some(f) = show {
                seen.push(f.index);
            }
            let wait = next
                .map(|d| d.saturating_duration_since(Instant::now()))
                .unwrap_or(Duration::from_millis(1));
            std::thread::sleep(wait.min(Duration::from_millis(20)));
        }
        assert_eq!(seen, [0, 1, 2, 3]);
        assert!(a.is_playing());
        // Ticked at the deadlines: lateness is scheduling noise, well under a frame.
        assert!(a.pacing.frames >= 3);
        assert!(
            a.pacing.late_max < Duration::from_millis(FRAME_MS / 2),
            "{:?}",
            a.pacing
        );
        a.shutdown();
    }
}

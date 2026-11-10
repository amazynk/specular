//! Animated images in the viewer: the player of the current image, its frames on the GPU,
//! the transport actions and the playback settings.

use std::sync::Arc;
use std::time::Instant;

use specular_core::Raster;
use specular_i18n::t;
use specular_input::Action;
use specular_log::debug;
use specular_settings::Origin;
use specular_video::audio::Silence;
use specular_video::play::PlayError;
use specular_widgets::{MotionBadge, Playback};

use super::App;
use crate::anim::{self, Anim, Movie};
use crate::loader::{ImageId, MotionClip, needed_scale};

/// Starts the player's scrub strip on a decoder of its own, opened on the strip's thread
/// at low priority (sound files show no frames: none).
fn start_strip(
    player: &mut specular_anim::Player,
    session: &Arc<dyn specular_decode::Session>,
    scale: u32,
    shown_height: u32,
) {
    if session
        .as_any()
        .is_some_and(|a| a.is::<specular_video::sound_view::SoundSession>())
    {
        return;
    }
    let session = Arc::clone(session);
    player.start_strip(
        Box::new(move || {
            specular_platform::lower_thread_priority();
            session.frames(scale)
        }),
        specular_anim::StripConfig {
            height: shown_height.clamp(1, specular_anim::STRIP_MAX_HEIGHT),
            ..specular_anim::StripConfig::default()
        },
    );
}

impl App {
    /// Starts, re-creates or stops the player so it matches the current image, its window
    /// size and the settings.
    pub(super) fn sync_animation(&mut self) {
        let cur = self.current_id();
        if let Some(a) = &self.anim
            && Some(a.id) != cur
        {
            self.stop_animation();
        }
        // A motion clip plays only while its image is on.
        if self.motion.is_some_and(|m| Some(m.0) != cur) {
            self.motion = None;
        }
        let Some(id) = cur else { return };
        if self.anim_failed == Some(id) {
            return;
        }
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let (Some(session), Some(info)) = (img.session.clone(), img.info.as_ref()) else {
            return;
        };
        if session.animation().is_none() {
            return;
        }
        // A movie that plays (one that can't says why in place of its picture).
        let video = session.video().filter(|f| f.unplayable.is_none()).cloned();
        // Frames at the coarsest scale that is still sharp at fit; a new window size may need
        // another (the player is re-created and seeks to where it was).
        let (dw, dh) = info.display_size();
        let need = self
            .viewport_px()
            .map_or(1, |(vw, vh)| needed_scale(dw, dh, vw, vh, 8));
        let scale = anim::pick_scale(info.scales, need);
        let mut cfg = if video.is_some() {
            anim::video_config(&self.settings, self.loop_forever)
        } else {
            anim::config(&self.settings)
        };
        if self.loop_forever {
            cfg.loop_mode = specular_anim::LoopMode::Forever;
        }
        if let Some(a) = &mut self.anim {
            let now = Instant::now();
            let have = a.player().config().clone();
            if have.loop_mode != cfg.loop_mode {
                a.player().set_loop_mode(cfg.loop_mode);
            }
            if have.delay_policy != cfg.delay_policy {
                a.player().set_delay_policy(cfg.delay_policy, now);
            }
            if a.scale == scale {
                return;
            }
            debug!(target: specular_log::DECODE, "img={id} animation: frames now at 1/{scale} (were 1/{})", a.scale);
            cfg.speed = a.speed();
            cfg.autoplay = a.is_playing();
        }
        let resume = self.anim.as_ref().and_then(Anim::frame);
        self.stop_animation();
        // A stream's next piece whose player was made ahead (its first pictures ready).
        if resume.is_none()
            && let Some(mut a) = self.stream_take_ready(id, scale)
        {
            if let Some(facts) = &video {
                a.set_movie(self.movie_sound(session.source(), facts));
            }
            debug!(target: specular_log::DECODE, "img={id} animation: the player made ahead");
            self.anim = Some(a);
            self.redraw = true;
            return;
        }
        match Anim::new(id, session.as_ref(), scale, cfg, self.loader.waker()) {
            Ok(Some(mut a)) => {
                if let Some(f) = resume {
                    a.player().seek(f);
                }
                if let Some(facts) = &video {
                    a.set_movie(self.movie_sound(session.source(), facts));
                    // Small frames over the whole movie for dragging the seek bar, as tall as
                    // the movie shows (480 px at most).
                    let shown = self.viewport_px().map_or(dh, |(_, vh)| vh.min(dh));
                    start_strip(a.player(), &session, scale, shown);
                }
                debug!(target: specular_log::DECODE, "img={id} animation: player at 1/{scale}, {:?} frames", a.playback().count);
                self.anim = Some(a);
                self.redraw = true;
            }
            Ok(None) => {}
            Err(e) => {
                debug!(target: specular_log::DECODE, "img={id} animation cannot play: {e}");
                self.anim_failed = Some(id);
            }
        }
    }

    /// Stops the player; the image goes back to its still overview when shown again.
    fn stop_animation(&mut self) {
        let Some(a) = self.anim.take() else { return };
        let id = a.id;
        if a.present.total.presents > 0 {
            debug!(
                target: specular_log::RENDER,
                "img={id} animation presents: {}; drift max {:.2} ms",
                a.present.total.summary(),
                a.present.drift_max_ms
            );
        }
        let (p, dropped) = a.shutdown();
        if p.frames > 0 {
            debug!(
                target: specular_log::DECODE,
                "img={id} animation pacing: {} frames, late mean {:.2} ms, max {:.2} ms, {} over 4 ms, {} dropped",
                p.frames,
                p.mean_ms(),
                p.late_max.as_secs_f64() * 1e3,
                p.late_4ms,
                dropped
            );
        }
        if let Some(img) = self.images.get_mut(&id)
            && img.gpu_rank == Some(0)
        {
            img.gpu_rank = None;
        }
    }

    /// Advances the player (from `about_to_wait`): uploads a due frame and asks to wake at
    /// the next deadline.
    pub(super) fn tick_animation(&mut self) {
        // A report's jump shows its frame once the file is in medical mode.
        self.report_tick();
        // Grey DICOM cine (medical mode) runs on the same clock.
        self.medical_tick();
        self.sync_animation();
        self.player_tick();
        self.sound_fx_tick();
        self.outdev_tick();
        let gallery = self.gallery_active();
        let Some(a) = self.anim.as_mut() else { return };
        let id = a.id;
        a.hidden = gallery;
        let (show, next) = a.tick(Instant::now());
        // (`wake_by`, with the fields apart: `a` borrows the player.)
        if let Some(d) = next
            && self.wake_at.is_none_or(|w| d < w)
        {
            self.wake_at = Some(d);
            self.causes.wake = Some(super::Cause::Animation);
        }
        // A movie's sound that can't play is said once; the picture plays on.
        let silence = a.take_silence();
        // A frame that needs more free RAM than there is stops playback: said once.
        let memory = a.take_needs_memory();
        if memory.is_some() && a.is_playing() {
            a.toggle(Instant::now());
        }
        if let Some(n) = memory {
            let now = self.now();
            self.chrome
                .toast(crate::error_text::needs_memory_text(n), None, now);
            self.redraw = true;
        }
        let Some(a) = self.anim.as_mut() else { return };
        if a.movie.is_some() {
            // The playhead moves while playing: the readout and seek bar follow each frame.
            self.redraw |= show.is_some();
        }
        if let Some(why) = silence {
            let msg = match why {
                Silence::NoDevice(w) if w == "lost" => t!("toast-sound-device-lost"),
                Silence::NoDevice(_) => t!("toast-no-sound-device"),
                Silence::Unsupported(codec) if codec == specular_video::sound_view::DRM => {
                    t!("toast-sound-drm")
                }
                Silence::Unsupported(codec) => t!("toast-sound-unsupported", codec = codec),
                Silence::NoTrack => t!("toast-sound-unsupported", codec = "?"),
                Silence::Failed(_) => {
                    t!("toast-sound-stopped", reason = t!("error-reason-internal"))
                }
            };
            let now = self.now();
            self.chrome.toast(msg, None, now);
        }
        if let Some(f) = show {
            self.show_frame(id, &f.raster);
            if let Some((m, t0)) = &mut self.motion
                && *m == id
                && let Some(t) = t0.take()
            {
                debug!(target: specular_log::DECODE, "img={id} motion clip: first frame on screen {:.1} ms after play", t.elapsed().as_secs_f64() * 1e3);
            }
        }
    }

    /// Puts an animation frame into the image's overview texture (pooled, so frames of one
    /// size reuse the same textures).
    fn show_frame(&mut self, id: ImageId, raster: &Raster) {
        // A sound file's page draws its own playhead over the picture; its player's frames
        // carry no picture (`SoundSession::clock_frames`) and are never shown.
        if self.sound_page.shows(id) {
            self.redraw = true;
            return;
        }
        let Some(gfx) = self.gfx.as_mut() else { return };
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        if img.session.as_ref().is_some_and(|s| {
            s.as_any()
                .is_some_and(|a| a.is::<specular_video::sound_view::SoundSession>())
        }) {
            return;
        }
        let Some(info) = &img.info else { return };
        if !self.gpu_images.contains(id) {
            let g = gfx.renderer.create_image(
                &gfx.gpu,
                id,
                info.width,
                info.height,
                img.lut.as_deref(),
            );
            self.gpu_images.insert(g);
        }
        let Some(g) = self.gpu_images.get(id) else {
            return;
        };
        gfx.renderer.upload_overview(&gfx.gpu, g, raster);
        // Rank 0 beats every still overview: decodes still in flight don't replace frames.
        img.gpu_rank = Some(0);
        self.hud.upload_ms = gfx.renderer.last_upload_ms;
        self.redraw = true;
        self.present_now = true;
    }

    /// The current image plays (or can play) as an animation: no detail tiles.
    pub(super) fn animated_current(&self) -> bool {
        self.anim
            .as_ref()
            .is_some_and(|a| Some(a.id) == self.current_id())
    }

    pub(super) fn playback(&self) -> Option<Playback> {
        self.anim
            .as_ref()
            .filter(|a| Some(a.id) == self.current_id())
            .map(Anim::playback)
            .map(|p| self.stream_playback(p))
            // Casting: the transport shows the device.
            .map(|p| self.cast_playback(p))
    }

    /// The transport actions. Returns `false` for other actions.
    pub(super) fn playback_action(&mut self, action: Action) -> bool {
        if !matches!(
            action,
            Action::PlayPause
                | Action::NextFrame
                | Action::PrevFrame
                | Action::FasterPlayback
                | Action::SlowerPlayback
                | Action::NormalPlayback
                | Action::ToggleLoopForever
                | Action::PlayPauseVideo
                | Action::SeekForward
                | Action::SeekBack
                | Action::VolumeUp
                | Action::VolumeDown
                | Action::ToggleMute
        ) {
            return false;
        }
        let action = if action == Action::PlayPauseVideo {
            Action::PlayPause
        } else {
            action
        };
        // Casting: the device takes play, pause, seek and volume.
        if self.cast_action(action) {
            return true;
        }
        let now = Instant::now();
        let t_now = self.now();
        if matches!(
            action,
            Action::VolumeUp | Action::VolumeDown | Action::ToggleMute
        ) {
            self.video_volume(action);
            return true;
        }
        if matches!(action, Action::SeekForward | Action::SeekBack) {
            if let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) {
                a.seek_by(anim::SEEK_STEP, action == Action::SeekForward, now);
            }
            self.tick_animation();
            self.redraw = true;
            return true;
        }
        if action == Action::ToggleLoopForever {
            self.loop_forever = !self.loop_forever;
            let movie = self.anim.as_ref().is_some_and(|a| a.movie.is_some());
            if let Some(m) = self.anim.as_mut().and_then(|a| a.movie.as_mut()) {
                m.looping = self.loop_forever;
            }
            let msg = match (movie, self.loop_forever) {
                (true, true) => t!("toast-video-loop-on"),
                (true, false) => t!("toast-video-loop-off"),
                (false, true) => t!("toast-loop-forever-on"),
                (false, false) => t!("toast-loop-forever-off"),
            };
            self.chrome.toast(msg, None, t_now);
            // The player takes the new mode; a finished animation starts over.
            self.sync_animation();
            if self.loop_forever
                && let Some(a) = self.anim.as_mut()
                && !a.is_playing()
            {
                a.toggle(now);
            }
        } else {
            if action == Action::PlayPause && !self.animated_current() && self.start_motion() {
                return true;
            }
            let Some(a) = self.anim.as_mut() else {
                return true;
            };
            let mut toast = None;
            match action {
                Action::PlayPause => a.toggle(now),
                Action::NextFrame | Action::PrevFrame => {
                    if a.is_playing() {
                        a.toggle(now);
                    }
                    a.step(action == Action::NextFrame);
                }
                _ => {
                    let s = match action {
                        Action::NormalPlayback => 1.0,
                        _ => anim::step_speed(a.speed(), action == Action::FasterPlayback),
                    };
                    // Movies play from half to twice their speed.
                    let s = match &a.movie {
                        Some(_) => s.clamp(anim::VIDEO_MIN_SPEED, anim::VIDEO_MAX_SPEED),
                        None => s,
                    };
                    a.set_speed(s, now);
                    toast = Some(t!("toast-speed", speed = I18nNumber(s).to_string()));
                }
            }
            if let Some(msg) = toast {
                self.chrome.toast(msg, None, t_now);
            }
        }
        // Tick now: a step or play shows at once rather than at the next event.
        self.tick_animation();
        self.redraw = true;
        true
    }
}

impl App {
    /// The current file's movie facts, when it is a movie.
    pub(super) fn video_facts(&self) -> Option<&specular_decode::VideoFacts> {
        let id = self.current_id()?;
        self.images.get(&id)?.session.as_ref()?.video()
    }

    /// A movie that plays is on screen (video mode's keys apply).
    pub(super) fn video_current(&self) -> bool {
        self.video_facts().is_some_and(|f| f.unplayable.is_none())
    }

    /// The movie on screen has sound that plays (the volume and mute controls apply).
    pub(super) fn sound_current(&self) -> bool {
        self.anim
            .as_ref()
            .filter(|a| Some(a.id) == self.current_id())
            .and_then(|a| a.movie.as_ref())
            .is_some_and(|m| m.has_sound)
    }

    /// A movie's sound for its player: opened on another thread on the default device (none
    /// with `SPECULAR_NO_SOUND` set, as in tests and headless runs), at the volume set.
    fn movie_sound(
        &mut self,
        source: &Arc<specular_core::Source>,
        facts: &specular_decode::VideoFacts,
    ) -> Movie {
        let has_track = facts.audio.is_some();
        let has_sound = has_track && facts.silent_reason.is_none();
        let no_device = std::env::var_os("SPECULAR_NO_SOUND").is_some();
        // The sound chain's settings from the start, or the sound handed on from the file
        // before (gapless; `sound_fx.rs`).
        let open = (has_sound && !no_device).then(|| self.sound_opener(source));
        let s = &self.settings.viewing;
        let mut m = Movie::new(
            has_sound,
            self.loop_forever,
            anim::volume(&self.settings),
            s.video_muted,
            open,
        );
        if let Some(why) = &facts.silent_reason {
            m.silence = Some(Silence::Unsupported(why.clone()));
        }
        m.picture = facts.sound.is_none();
        m
    }

    /// The volume keys and the mute key or button: saved in the settings, applied to the
    /// sound, said in a toast.
    fn video_volume(&mut self, action: Action) {
        let s = &self.settings.viewing;
        let (mut volume, mut muted) = (s.video_volume, s.video_muted);
        match action {
            Action::VolumeUp => {
                volume = (volume + 10).min(100);
                muted = false;
            }
            Action::VolumeDown => volume = (volume - 10).max(0),
            _ => muted = !muted,
        }
        self.store.update(Origin::Program, |s| {
            s.viewing.video_volume = volume;
            s.viewing.video_muted = muted;
        });
        // The store's change reaches `self.settings` with the next settings event; the sound
        // takes the new values now.
        if let Some(a) = self.anim.as_mut() {
            a.set_volume(volume as f32 / 100.0, muted);
        }
        let msg = match action {
            Action::ToggleMute if muted => t!("toast-muted"),
            Action::ToggleMute => t!("toast-unmuted"),
            _ => t!("toast-volume", percent = volume),
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The volume slider was pressed, dragged, wheeled or stepped to `volume` (1 is normal):
    /// the sound takes it at once, raising it unmutes, and it is saved in the settings as
    /// the volume keys' is (when let go, so a drag saves once). No toast: the slider shows
    /// the percentage. Casting, the device takes it.
    pub(super) fn video_set_volume(&mut self, volume: f32, done: bool) {
        if self.cast_set_volume(volume, done) {
            return;
        }
        let s = &self.settings.viewing;
        let (old, muted) = self
            .anim
            .as_ref()
            .and_then(|a| a.movie.as_ref())
            .map_or((anim::volume(&self.settings), s.video_muted), |m| {
                (m.volume, m.muted)
            });
        let (pct, muted) = slider_volume(volume, old, muted);
        if let Some(a) = self.anim.as_mut() {
            a.set_volume(pct as f32 / 100.0, muted);
        }
        if done {
            save_volume(&self.store, pct, muted);
        }
        self.redraw = true;
    }

    /// The seek bar was pressed or dragged: the movie shows the frame there (scrubbing: the
    /// nearest frame decoded so far until that one is, silently), and when let go (`done`)
    /// the exact frame, playing on from it if it was playing.
    pub(super) fn video_seek(&mut self, fraction: f64, done: bool) {
        if self.cast_seek_fraction(fraction, done) {
            return;
        }
        // A stream's piece: the bar is the whole stream's.
        if self.stream_seek_fraction(fraction) {
            return;
        }
        let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) else {
            return;
        };
        if done {
            a.seek_fraction(fraction);
        } else {
            a.scrub_fraction(fraction);
        }
        self.tick_animation();
        self.redraw = true;
    }

    /// A click at `p` (physical px) on a sound file's waveform or spectrogram plays from
    /// there. Whether it was on them.
    pub(super) fn sound_click(&mut self, p: (f32, f32)) -> bool {
        let Some(cover) = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.video())
            .and_then(|v| v.sound.as_ref())
            .map(|s| s.cover_shown)
        else {
            return false;
        };
        if self
            .anim
            .as_ref()
            .and_then(|a| a.movie.as_ref())
            .is_none_or(|m| m.picture)
        {
            return false;
        }
        let (x, y) = self.view.screen_to_display(p);
        let Some(f) = specular_video::sound_view::time_at(cover, x, y) else {
            return false;
        };
        self.video_seek(f, true);
        if self.casting() {
            return true;
        }
        if let Some(a) = self.anim.as_mut()
            && !a.is_playing()
        {
            a.toggle(Instant::now());
        }
        self.tick_animation();
        true
    }

    /// Adds (or replaces) the Movie group in image `id`'s metadata, when it is a movie.
    pub(super) fn video_patch(&mut self, id: ImageId) {
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let (Some(r), Some(facts)) = (
            img.metadata.as_ref(),
            img.session.as_ref().and_then(|s| s.video()),
        ) else {
            return;
        };
        let mut group = match &facts.sound {
            Some(sound) => sound_group(facts, sound, self.sound_page.loudness_of(id)),
            None => {
                let mut g = video_group(facts);
                // Measured while Info is open (`info_loudness.rs`).
                if let Some(l) = self.movie_loudness_of(id) {
                    g.items.extend(super::sound_page::info_items(&l));
                }
                g
            }
        };
        // A piece of a network stream: the stream's length and the part on screen.
        if let Some((chunk, total)) = self.stream_piece_at(&img.path) {
            Self::piece_rows(&mut group, &chunk, total);
        }
        // The levelling applied to its sound (once it plays).
        if let Some(row) = self.sound_fx_info(id) {
            group.items.push(row);
        }
        // The device it plays on.
        if let Some(row) = self.outdev_info(id) {
            group.items.push(row);
        }
        if r.summary.contains(&group) {
            return;
        }
        let mut patched = (**r).clone();
        patched.summary.retain(|g| g.key != "video");
        let at = patched
            .summary
            .iter()
            .position(|g| g.key == "image")
            .map_or(0, |i| i + 1);
        patched.summary.insert(at, group);
        if let Some(img) = self.images.get_mut(&id) {
            img.metadata = Some(Arc::new(patched));
        }
        self.redraw = true;
    }

    /// The current image's motion clip, when it is a Motion Photo or Live Photo.
    pub(super) fn motion_current(&self) -> Option<&Arc<specular_video::MotionInfo>> {
        let id = self.current_id()?;
        self.images.get(&id)?.motion.as_ref()
    }

    /// The motion badge of the current image.
    pub(super) fn motion_badge(&self) -> Option<MotionBadge> {
        let m = self.motion_current()?;
        Some(MotionBadge {
            live: m.kind == specular_video::Kind::LivePhoto,
            playing: self.motion.is_some_and(|p| Some(p.0) == self.current_id()),
        })
    }

    /// The motion badge: plays the clip, or goes back to the still.
    pub(super) fn toggle_motion(&mut self) {
        if !self.stop_motion() {
            self.start_motion();
        }
    }

    /// Opens the current image's motion clip (off the UI thread; it plays in place of the
    /// still once its decoder is open, `on_motion_clip`). `false` when it has none.
    fn start_motion(&mut self) -> bool {
        let Some(id) = self.current_id() else {
            return false;
        };
        if self.motion.is_some_and(|m| m.0 == id) {
            return true;
        }
        let Some(img) = self.images.get(&id) else {
            return false;
        };
        let (Some(m), Some(session), Some(info)) =
            (img.motion.clone(), img.session.clone(), img.info.as_ref())
        else {
            return false;
        };
        debug!(target: specular_log::DECODE, "img={id} motion clip: {} ({:?}) requested", m.kind.key(), m.codec);
        let (orientation, display) = (info.orientation, info.display_size());
        self.motion = Some((id, Some(Instant::now())));
        self.loader
            .motion_clip(id, session, m, orientation, display);
        self.redraw = true;
        true
    }

    /// Back to the still from a playing (or opening) motion clip. `false` when none plays.
    pub(super) fn stop_motion(&mut self) -> bool {
        let Some((id, _)) = self.motion.take() else {
            return false;
        };
        if self.anim.as_ref().is_some_and(|a| a.id == id) {
            self.stop_animation();
        }
        if let (Some(gfx), Some(g), Some(img)) = (
            self.gfx.as_mut(),
            self.gpu_images.get(id),
            self.images.get(&id),
        ) && let Some(gm) = &img.gain_map
        {
            gfx.renderer.set_gain_map(&gfx.gpu, g, Some(gm));
        }
        self.upload(id);
        self.scene = None;
        self.redraw = true;
        true
    }

    /// The clip `start_motion` asked for: plays it, looping, in place of the still (frames go
    /// to the still's overview texture, as an animation's do), or says why it cannot.
    pub(super) fn on_motion_clip(
        &mut self,
        id: ImageId,
        clip: Result<MotionClip, PlayError>,
        ms: f64,
    ) {
        // Stopped, or another image is on: the clip and its decoder are dropped.
        if self.motion.is_none_or(|m| m.0 != id) || self.current_id() != Some(id) {
            return;
        }
        let now = self.now();
        let clip = match clip {
            Ok(c) => c,
            Err(e) => {
                self.motion = None;
                let msg = t!(
                    "toast-motion-failed",
                    reason = crate::error_text::play_reason(&e)
                );
                self.chrome.toast(msg, None, now);
                self.redraw = true;
                return;
            }
        };
        debug!(target: specular_log::DECODE, "img={id} motion clip: decoder open after {ms:.1} ms");
        self.stop_animation();
        // The user asked for it: it plays even with "Reduce motion", and loops until stopped.
        let mut cfg = anim::config(&self.settings);
        cfg.autoplay = true;
        cfg.loop_mode = specular_anim::LoopMode::Forever;
        let player =
            specular_anim::Player::new(clip.frames, &clip.info, cfg, Some(self.loader.waker()));
        self.anim = Some(Anim::with_player(id, 1, player));
        // The clip is SDR video: the still's gain map does not apply to it.
        if let (Some(gfx), Some(g)) = (self.gfx.as_mut(), self.gpu_images.get(id)) {
            gfx.renderer.set_gain_map(&gfx.gpu, g, None);
        }
        self.scene = None;
        self.tick_animation();
        self.redraw = true;
    }
}

/// A length as a clock reading: "0:07", "12:30", "1:02:03".
pub(super) fn clock(secs: f64) -> String {
    let s = if secs.is_finite() {
        secs.max(0.0) as u64
    } else {
        0
    };
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The info panel's Movie group: length, codecs, frame rate, sound, recording facts.
pub(super) fn video_group(f: &specular_decode::VideoFacts) -> specular_metadata::SummaryGroup {
    use specular_metadata::{SummaryGroup, SummaryItem};
    let item = |key: &'static str, label: String, value: String| SummaryItem {
        key,
        label,
        value,
        detail: None,
    };
    let mut items = Vec::new();
    if let Some(d) = f.duration_us {
        items.push(item(
            "video-duration",
            t!("info-video-duration"),
            clock(d as f64 / 1e6),
        ));
    }
    let mut parts = vec![f.codec.clone()];
    parts.extend(f.profile.clone());
    parts.extend(f.hdr.map(|h| h.to_string()));
    if f.interlaced {
        parts.push(t!("info-video-interlaced"));
    }
    let codec = specular_i18n::dot_join(&parts);
    items.push(item("video-codec", t!("info-video-codec"), codec));
    if let Some(fps) = f.frame_rate {
        let n = specular_i18n::I18n::global().localizer().number(fps, 2);
        items.push(item(
            "video-fps",
            t!("info-video-frame-rate"),
            t!("info-video-fps", fps = n),
        ));
    }
    if let Some((h, v)) = f.pixel_aspect {
        items.push(item(
            "video-pixel-aspect",
            t!("info-video-pixel-aspect"),
            t!("info-video-pixel-aspect-value", h = h, v = v),
        ));
    }
    items.push(item(
        "video-audio",
        t!("info-video-audio"),
        f.audio.clone().unwrap_or_else(|| t!("info-video-no-audio")),
    ));
    if let Some(codec) = &f.silent_reason {
        items.push(item(
            "video-silent",
            t!("info-video-silent"),
            t!("info-video-silent-value", codec = codec.clone()),
        ));
    }
    if let Some(d) = &f.date {
        items.push(item("video-recorded", t!("info-video-recorded"), d.clone()));
    }
    let camera = [f.make.as_deref(), f.model.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if !camera.is_empty() {
        items.push(item("video-camera", t!("info-video-camera"), camera));
    }
    SummaryGroup {
        key: "video",
        title: t!("info-video-group"),
        items,
    }
}

/// The info panel's Sound group of a sound file: its tags, length, coding and cover.
pub(super) fn sound_group(
    f: &specular_decode::VideoFacts,
    s: &specular_decode::SoundFacts,
    loudness: Option<&specular_video::loudness::Report>,
) -> specular_metadata::SummaryGroup {
    use specular_metadata::{SummaryGroup, SummaryItem};
    let item = |key: &'static str, label: String, value: String| SummaryItem {
        key,
        label,
        value,
        detail: None,
    };
    let mut items = Vec::new();
    for (k, v) in &s.fields {
        let (key, label) = match *k {
            "title" => ("sound-title", t!("info-sound-title")),
            "artist" => ("sound-artist", t!("info-sound-artist")),
            "album" => ("sound-album", t!("info-sound-album")),
            "album-artist" => ("sound-album-artist", t!("info-sound-album-artist")),
            "composer" => ("sound-composer", t!("info-sound-composer")),
            "genre" => ("sound-genre", t!("info-sound-genre")),
            "date" => ("sound-date", t!("info-sound-date")),
            "track" => ("sound-track", t!("info-sound-track")),
            "disc" => ("sound-disc", t!("info-sound-disc")),
            "comment" => ("sound-comment", t!("info-sound-comment")),
            "copyright" => ("sound-copyright", t!("info-sound-copyright")),
            "encoder" => ("sound-encoder", t!("info-sound-encoder")),
            "track-gain" => ("sound-track-gain", t!("info-sound-track-gain")),
            "album-gain" => ("sound-album-gain", t!("info-sound-album-gain")),
            "midi" => ("sound-midi", t!("info-sound-midi")),
            "instruments" => ("sound-instruments", t!("info-sound-instruments")),
            "samples" => ("sound-samples", t!("info-sound-samples")),
            _ => continue,
        };
        items.push(item(key, label, v.clone()));
    }
    if let Some(d) = f.duration_us {
        items.push(item(
            "video-duration",
            t!("info-video-duration"),
            clock(d as f64 / 1e6),
        ));
    }
    if let Some(a) = &f.audio {
        let mut coding = a.clone();
        if let Some(b) = s.bits {
            coding = specular_i18n::dot_join(&[coding, t!("info-sound-bits-value", bits = b)]);
        }
        items.push(item("sound-coding", t!("info-sound-coding"), coding));
    }
    if let Some(k) = s.kbps {
        items.push(item(
            "sound-bitrate",
            t!("info-sound-bitrate"),
            t!("info-sound-kbps", kbps = k),
        ));
    }
    if let Some(codec) = &f.silent_reason {
        let value = if codec == specular_video::sound_view::DRM {
            t!("info-sound-drm")
        } else {
            t!("info-video-silent-value", codec = codec.clone())
        };
        items.push(item("video-silent", t!("info-video-silent"), value));
    }
    // Measured on the sound page's worker (EBU R 128).
    if let Some(l) = loudness {
        items.extend(super::sound_page::info_items(l));
    }
    if let Some(c) = &s.cover {
        items.push(item("sound-cover", t!("info-sound-cover"), c.clone()));
    }
    if !s.tag_kinds.is_empty() {
        items.push(item(
            "sound-tags",
            t!("info-sound-tags"),
            s.tag_kinds.join(", "),
        ));
    }
    SummaryGroup {
        key: "video",
        title: t!("info-sound-group"),
        items,
    }
}

/// A speed in the UI language's digits ("1.25", "1,25").
struct I18nNumber(f64);

impl std::fmt::Display for I18nNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = specular_i18n::I18n::global().localizer().number(self.0, 2);
        f.write_str(&s)
    }
}

/// The volume slider's `volume` (1 is normal) over the sound at `old`: the percent it
/// sets (0..=200) and whether the sound stays muted (raising it unmutes).
fn slider_volume(volume: f32, old: f32, muted: bool) -> (i64, bool) {
    let pct = (volume * 100.0).round().clamp(0.0, 100.0) as i64;
    (pct, muted && pct as f32 / 100.0 <= old)
}

/// Saves the volume and mute in the settings, as the volume keys do.
fn save_volume(store: &specular_settings::SettingsStore, pct: i64, muted: bool) {
    store.update(Origin::Program, |s| {
        s.viewing.video_volume = pct;
        s.viewing.video_muted = muted;
    });
}

#[cfg(test)]
mod tests {
    use specular_i18n::strip_isolates;

    /// The slider's value in whole percents up to 100; raising it unmutes, lowering it or
    /// setting the same keeps the mute.
    #[test]
    fn slider_volume_rounds_and_unmutes_when_raised() {
        use super::slider_volume as v;
        assert_eq!(v(0.804, 0.8, false), (80, false));
        assert_eq!(v(1.5, 0.8, false), (100, false));
        assert_eq!(v(-0.2, 0.8, false), (0, false));
        assert_eq!(v(0.85, 0.8, true), (85, false));
        assert_eq!(v(0.75, 0.8, true), (75, true));
        assert_eq!(v(0.8, 0.8, true), (80, true));
    }

    /// The slider's change is saved in the settings (a persisting change, like the keys').
    #[test]
    fn slider_volume_is_saved() {
        let store = specular_settings::SettingsStore::in_memory();
        super::save_volume(&store, 65, false);
        let s = store.snapshot();
        assert_eq!((s.viewing.video_volume, s.viewing.video_muted), (65, false));
        super::save_volume(&store, 0, true);
        let s = store.snapshot();
        assert_eq!((s.viewing.video_volume, s.viewing.video_muted), (0, true));
    }

    /// Non-square pixels are named in the Movie group (fidelity round 8).
    #[test]
    fn pixel_shape_in_the_movie_group() {
        let f = specular_decode::VideoFacts {
            pixel_aspect: Some((8, 9)),
            ..Default::default()
        };
        let g = super::video_group(&f);
        let row = g.items.iter().find(|i| i.key == "video-pixel-aspect");
        let value = row.map(|i| strip_isolates(&i.value));
        assert_eq!(value.as_deref(), Some("8:9, shown stretched"));
        let square = super::video_group(&specular_decode::VideoFacts::default());
        assert!(square.items.iter().all(|i| i.key != "video-pixel-aspect"));
    }
}

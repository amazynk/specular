//! The movie on the system's media controls (MPRIS on Linux, the media overlay on Windows)
//! and the keyboard's media keys while the window has the focus. Next and Previous go to
//! the next and previous item (the queue's, else the folder's files of the same kind; the
//! folder's next file when no movie or sound file is on screen). Their picture is the file's embedded cover,
//! else the movie's frame on screen once it shows, as a small PNG in the app's cache folder.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use specular_input::Action;
use specular_platform::media::{MediaControls, MediaEvent, NowPlaying};

use super::super::App;
use crate::loader::ImageId;

/// How often the controls hear the position (app clock, seconds).
const EVERY: f64 = 0.5;
/// The picture's longest side (px).
const ART_PX: u32 = 512;

#[derive(Default)]
pub(crate) struct Media {
    controls: Option<MediaControls>,
    /// Tried to start them (once per run).
    tried: bool,
    last: f64,
    /// The picture of a file, written.
    art: Option<(ImageId, PathBuf)>,
    /// The file a picture was asked for, and the answer to come.
    art_for: Option<ImageId>,
    art_rx: Option<mpsc::Receiver<(ImageId, Option<PathBuf>)>>,
}

/// Where the picture is written.
fn art_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("specular-media-art-{}", std::process::id()));
    }
    specular_settings::Locations::detect()
        .cache_dir
        .join("now-playing")
}

/// `rgba` shrunk (box filter) so its longest side is at most `max`.
fn shrink(w: u32, h: u32, rgba: &[u8], max: u32) -> (u32, u32, Vec<u8>) {
    let f = w.max(h).div_ceil(max.max(1)).max(1);
    if f == 1 {
        return (w, h, rgba.to_vec());
    }
    let (ow, oh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = Vec::with_capacity((ow * oh * 4) as usize);
    for oy in 0..oh {
        for ox in 0..ow {
            let mut acc = [0u32; 4];
            let mut n = 0;
            for y in oy * f..((oy + 1) * f).min(h) {
                for x in ox * f..((ox + 1) * f).min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    for (a, v) in acc.iter_mut().zip(&rgba[i..i + 4]) {
                        *a += u32::from(*v);
                    }
                    n += 1;
                }
            }
            out.extend(acc.map(|a| (a / n.max(1)) as u8));
        }
    }
    (ow, oh, out)
}

/// The picture of `session`: its embedded cover, else (`frame`) that frame of the movie.
fn picture(
    session: &Arc<dyn specular_decode::Session>,
    frame: Option<u32>,
) -> Option<(u32, u32, Vec<u8>)> {
    let tags = specular_video::sound_tags::read(session.source().bytes());
    if let Some(c) = tags.cover
        && let Some(p) = specular_decode::decode_rgba8(&c.data)
    {
        return Some(p);
    }
    super::frame::decode(session, frame?).ok()
}

/// Writes the picture of `file` into `dir` as a PNG of at most [`ART_PX`] (a name of its
/// own per file, so the panels don't show a stale cached one), leaving no other there.
fn write_art(dir: &Path, file: &Path, w: u32, h: u32, rgba: &[u8]) -> Option<PathBuf> {
    let (w, h, px) = shrink(w, h, rgba, ART_PX);
    let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, &px).ok()?;
    std::fs::create_dir_all(dir).ok()?;
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for b in file.as_os_str().as_encoded_bytes() {
        hash = (hash ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
    }
    let name = format!("art-{hash:016x}.png");
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name();
            let n = n.to_string_lossy();
            if n.starts_with("art-") && n.ends_with(".png") && n != name.as_str() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let out = dir.join(name);
    let tmp = out.with_extension("png.part");
    std::fs::write(&tmp, &png).ok()?;
    std::fs::rename(&tmp, &out).ok()?;
    Some(out)
}

/// The action a media key runs (`None`: not a media key).
pub(crate) fn key_action(key: &specular_gfx::winit::keyboard::Key, video: bool) -> Option<Action> {
    use specular_gfx::winit::keyboard::{Key, NamedKey};
    let Key::Named(n) = key else { return None };
    let play = if video {
        Action::PlayPauseVideo
    } else {
        Action::PlayPause
    };
    Some(match n {
        NamedKey::MediaPlayPause | NamedKey::MediaPlay | NamedKey::MediaPause => play,
        NamedKey::MediaStop if video => play,
        NamedKey::MediaTrackNext if video => Action::NextTrack,
        NamedKey::MediaTrackPrevious if video => Action::PrevTrack,
        NamedKey::MediaTrackNext => Action::NextImage,
        NamedKey::MediaTrackPrevious => Action::PrevImage,
        NamedKey::MediaFastForward if video => Action::SeekForwardLong,
        NamedKey::MediaRewind if video => Action::SeekBackLong,
        _ => return None,
    })
}

impl App {
    /// Each tick: what plays, to the controls; their presses, to the player.
    pub(in crate::app) fn media_tick(&mut self, title: Option<String>) {
        let n = self.now();
        if title.is_some() && !self.player.media.tried {
            self.player.media.tried = true;
            let waker = self.loader.waker();
            self.player.media.controls = MediaControls::start(Box::new(move || waker()));
        }
        if self.player.media.controls.is_none() {
            return;
        }
        let art = self.media_art();
        let Some(c) = self.player.media.controls.as_mut() else {
            return;
        };
        let events = c.poll();
        if n - self.player.media.last > EVERY || !events.is_empty() {
            self.player.media.last = n;
            let (playing, length, position) = self
                .anim
                .as_ref()
                .and_then(|a| {
                    a.playback()
                        .video
                        .map(|v| (a.is_playing(), v.duration, v.position))
                })
                .unwrap_or((false, 0.0, 0.0));
            let count = self.entries.len();
            let cur = self.current.unwrap_or(0);
            let now = NowPlaying {
                playing: title.as_ref().map(|_| playing),
                title: title.unwrap_or_default(),
                length_us: (length * 1e6) as i64,
                position_us: (position * 1e6) as i64,
                art,
                can_next: cur + 1 < count,
                can_previous: cur > 0,
            };
            c.update(now);
        }
        for e in events {
            self.media_event(e);
        }
    }

    /// The picture of the file playing, once written; asks for it once its first frame
    /// shows (after resuming).
    fn media_art(&mut self) -> Option<PathBuf> {
        let cur = self
            .anim
            .as_ref()
            .filter(|a| a.movie.is_some() && Some(a.id) == self.current_id());
        let m = &mut self.player.media;
        if let Some(rx) = &m.art_rx
            && let Ok((id, p)) = rx.try_recv()
        {
            m.art_rx = None;
            m.art = p.map(|p| (id, p));
            // Said at once.
            m.last = f64::NEG_INFINITY;
        }
        let a = cur?;
        if m.art_for != Some(a.id) && m.art_rx.is_none() && self.player.pending_resume.is_none() {
            let frame = a.frame();
            let picture_movie = a.movie.as_ref().is_some_and(|mv| mv.picture);
            if let Some(session) = self.images.get(&a.id).and_then(|i| i.session.clone())
                && (frame.is_some() || !picture_movie)
            {
                m.art_for = Some(a.id);
                let (tx, rx) = mpsc::channel();
                m.art_rx = Some(rx);
                let id = a.id;
                let dir = art_dir();
                self.loader.background("io-media-art", move |l| {
                    let file = session.source().path().to_path_buf();
                    let out = picture(&session, frame.filter(|_| picture_movie))
                        .and_then(|(w, h, px)| write_art(&dir, &file, w, h, &px));
                    let _ = tx.send((id, out));
                    l.waker()();
                });
            }
        }
        let m = &self.player.media;
        m.art
            .as_ref()
            .filter(|(id, _)| *id == a.id)
            .map(|(_, p)| p.clone())
    }

    fn media_event(&mut self, e: MediaEvent) {
        let playing = self
            .anim
            .as_ref()
            .is_some_and(crate::anim::Anim::is_playing);
        let video = self.video_current();
        let toggle = if video {
            Action::PlayPauseVideo
        } else {
            Action::PlayPause
        };
        match e {
            MediaEvent::PlayPause => {
                self.playback_action(toggle);
            }
            MediaEvent::Play if !playing => {
                self.playback_action(toggle);
            }
            MediaEvent::Pause | MediaEvent::Stop if playing => {
                self.playback_action(toggle);
            }
            MediaEvent::Next if video => self.queue_step(true),
            MediaEvent::Previous if video => self.queue_step(false),
            MediaEvent::Next => self.media_navigate(1),
            MediaEvent::Previous => self.media_navigate(-1),
            MediaEvent::Seek(us) => {
                if let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) {
                    let by = Duration::from_micros(us.unsigned_abs());
                    a.seek_by(by, us > 0, std::time::Instant::now());
                }
            }
            MediaEvent::SetPosition(us) => {
                if let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) {
                    a.seek_time(Duration::from_micros(us.max(0) as u64));
                }
            }
            MediaEvent::Raise => {
                if let Some(g) = self.gfx.as_ref() {
                    g.window.focus_window();
                }
            }
            _ => {}
        }
        self.redraw = true;
    }

    fn media_navigate(&mut self, delta: i64) {
        let Some(cur) = self.current else { return };
        let to = cur as i64 + delta;
        if to >= 0 && (to as usize) < self.entries.len() {
            self.go_to(to as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use specular_gfx::winit::keyboard::{Key, NamedKey};

    use super::*;

    #[test]
    fn media_keys() {
        let k = |n| Key::Named(n);
        assert_eq!(
            key_action(&k(NamedKey::MediaPlayPause), true),
            Some(Action::PlayPauseVideo)
        );
        assert_eq!(
            key_action(&k(NamedKey::MediaPlayPause), false),
            Some(Action::PlayPause)
        );
        assert_eq!(
            key_action(&k(NamedKey::MediaTrackNext), true),
            Some(Action::NextTrack)
        );
        assert_eq!(
            key_action(&k(NamedKey::MediaTrackNext), false),
            Some(Action::NextImage)
        );
        assert_eq!(key_action(&k(NamedKey::MediaStop), false), None);
        assert_eq!(key_action(&k(NamedKey::ArrowLeft), true), None);
    }

    #[test]
    fn art_is_small_and_alone() {
        // A 1300 × 650 gradient: shrunk by 3 to 434 × 217, averaged.
        let (w, h) = (1300u32, 650u32);
        let px: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % w * 255 / w) as u8, 0, 0, 255])
            .collect();
        let (sw, sh, s) = shrink(w, h, &px, ART_PX);
        assert_eq!((sw, sh), (434, 217));
        assert_eq!(s.len(), (sw * sh * 4) as usize);
        assert_eq!(&s[..4], &[0, 0, 0, 255]);
        assert_eq!(shrink(4, 2, &px[..32], ART_PX).0, 4);
        let dir = art_dir().join("t");
        let a = write_art(&dir, Path::new("/v/a.mkv"), w, h, &px).unwrap();
        let b = write_art(&dir, Path::new("/v/b.mkv"), w, h, &px).unwrap();
        assert_ne!(a, b);
        // One picture at a time in the folder.
        assert!(!a.exists() && b.exists());
        let png = std::fs::read(&b).unwrap();
        let (dw, dh, _) = specular_decode::decode_rgba8(&png).unwrap();
        assert_eq!((dw, dh), (434, 217));
        let _ = std::fs::remove_dir_all(art_dir());
    }
}

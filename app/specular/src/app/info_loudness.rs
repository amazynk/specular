//! Info's loudness (EBU R 128) of a movie's sound: measured on a worker while Info is open
//! on the movie, kept for the window's life. Sound files get theirs from the sound page.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use specular_core::CancelToken;
use specular_video::loudness::Report;

use super::App;
use crate::loader::ImageId;

/// Movies whose loudness is kept.
const KEEP: usize = 32;

/// A movie's file and sound track (`None`: its default track).
type Key = (PathBuf, Option<u64>);

/// What was measured, and the measuring under way.
#[derive(Default)]
pub(super) struct InfoLoudness {
    /// Measured (`None`: no sound that can be decoded), oldest first.
    done: VecDeque<(Key, Option<Arc<Report>>)>,
    job: Option<Job>,
}

struct Job {
    id: ImageId,
    key: Key,
    rx: mpsc::Receiver<Option<Report>>,
    cancel: CancelToken,
}

impl InfoLoudness {
    fn get(&self, key: &Key) -> Option<&Option<Arc<Report>>> {
        self.done.iter().find(|(k, _)| k == key).map(|(_, r)| r)
    }

    fn insert(&mut self, key: Key, r: Option<Arc<Report>>) {
        self.done.retain(|(k, _)| *k != key);
        if self.done.len() >= KEEP {
            self.done.pop_front();
        }
        self.done.push_back((key, r));
    }

    fn cancel(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel.cancel();
        }
    }
}

/// A movie (not a sound file) with sound that plays: its loudness can be measured.
fn measurable(f: &specular_decode::VideoFacts) -> bool {
    f.sound.is_none() && f.audio.is_some() && f.silent_reason.is_none()
}

impl App {
    /// The movie on screen and its sound track, when its loudness can be measured.
    fn info_loudness_key(&self, id: ImageId) -> Option<(Key, Arc<specular_core::Source>)> {
        let session = self.images.get(&id)?.session.as_ref()?;
        if !session.video().is_some_and(measurable) {
            return None;
        }
        let source = Arc::clone(session.source());
        let key = (source.path().to_path_buf(), self.player.sound_track_of(id));
        Some((key, source))
    }

    /// The loudness of movie `id`'s sound, once measured.
    pub(super) fn movie_loudness_of(&self, id: ImageId) -> Option<Arc<Report>> {
        let (key, _) = self.info_loudness_key(id)?;
        self.info_loudness.get(&key).cloned().flatten()
    }

    /// Each frame: starts measuring the movie on screen while Info is open, and puts the
    /// result in Info when it comes.
    pub(super) fn info_loudness_tick(&mut self) {
        if let Some(j) = &self.info_loudness.job {
            match j.rx.try_recv() {
                Ok(r) => {
                    let (id, key) = (j.id, j.key.clone());
                    self.info_loudness.job = None;
                    self.info_loudness.insert(key, r.map(Arc::new));
                    self.video_patch(id);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.info_loudness.job = None,
            }
        }
        let cur = self
            .current_id()
            .filter(|_| !self.gallery_active() && self.chrome.info_open());
        let Some((id, (key, source))) =
            cur.and_then(|id| self.info_loudness_key(id).map(|k| (id, k)))
        else {
            return;
        };
        if self.info_loudness.get(&key).is_some() {
            return;
        }
        if self
            .info_loudness
            .job
            .as_ref()
            .is_some_and(|j| j.key == key)
        {
            return;
        }
        self.info_loudness.cancel();
        let cancel = CancelToken::new();
        let (tx, rx) = mpsc::channel();
        let wake = self.loader.waker();
        let (c, track) = (cancel.clone(), key.1);
        let spawned = specular_thread::builder("info-loudness").spawn(move || {
            let Ok(r) = specular_video::sound_view::movie_loudness(&source, track, &c) else {
                return;
            };
            let _ = tx.send(r);
            wake();
        });
        if spawned.is_ok() {
            self.info_loudness.job = Some(Job {
                id,
                key,
                rx,
                cancel,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: usize) -> Key {
        (PathBuf::from(format!("{n}.mkv")), None)
    }

    #[test]
    fn keeps_the_latest_movies() {
        let mut l = InfoLoudness::default();
        for n in 0..KEEP + 3 {
            l.insert(key(n), None);
        }
        assert_eq!(l.done.len(), KEEP);
        assert!(l.get(&key(0)).is_none(), "the oldest went");
        assert!(l.get(&key(KEEP + 2)).is_some());
        // Another track of the same movie is measured on its own.
        assert!(l.get(&(PathBuf::from("3.mkv"), Some(2))).is_none());
    }

    #[test]
    fn only_movies_whose_sound_plays_are_measured() {
        let movie = specular_decode::VideoFacts {
            audio: Some("AAC".into()),
            ..Default::default()
        };
        assert!(measurable(&movie));
        let silent = specular_decode::VideoFacts {
            silent_reason: Some("DTS:X".into()),
            ..movie.clone()
        };
        assert!(!measurable(&silent));
        let none = specular_decode::VideoFacts::default();
        assert!(!measurable(&none));
        let sound_file = specular_decode::VideoFacts {
            sound: Some(Default::default()),
            ..movie
        };
        assert!(
            !measurable(&sound_file),
            "the sound page measures sound files"
        );
    }
}

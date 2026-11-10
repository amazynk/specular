//! What plays next: the play queue (`specular-playlist` has its model) and, when no queue
//! item plays, the folder's files of the kind playing (sound files, or videos) in the
//! folder's order, or the gallery's selection when the file is part of it.
//!
//! - At the end of a sound file the next one plays (`viewing.sound_play_next`, on by
//!   default); at the end of a video, as `viewing.video_play_next` says. A queue always
//!   plays on, videos included.
//! - Repeat (off, all, one) and shuffle are settings (`viewing.play_repeat`,
//!   `viewing.play_shuffle`), for the queue and the folder alike.
//! - "Play next" and "Add to queue" from the gallery (its selection; a folder's sound files
//!   and videos) and the viewer (the file on screen); playlists (M3U, PLS, XSPF) open as the
//!   queue, and the queue saves as M3U8 or XSPF. Web (`http(s)`) items play as network
//!   streams (`stream.rs`); other addresses are listed only.
//! - Items' titles and lengths come from their tags and stream headers, read on a worker.
//!
//! The queue lives as long as the window: closing a file keeps it; it is not kept across
//! restarts.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use specular_i18n::t;
use specular_input::Action;
use specular_playlist::{Entry, Location, Queue, Repeat};
use specular_settings::{Origin, PlayRepeat};
use specular_widgets::{PlayOrder, QueueCmd, QueueRow, QueueView};

use super::App;
use crate::loader::ImageId;

/// Previous within this many seconds of the start goes to the previous item; later, it
/// starts the item over.
const RESTART_AFTER: f64 = 3.0;

/// The kind of file that plays on: sound files after sound files, videos after videos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Sound,
    Video,
}

impl Kind {
    /// The kind of `p` by its extension.
    pub(super) fn of(p: &Path) -> Option<Kind> {
        let ext = p.extension()?.to_str()?;
        if specular_query::is_sound_ext(ext) {
            Some(Kind::Sound)
        } else if specular_query::is_video_ext(ext) {
            Some(Kind::Video)
        } else {
            None
        }
    }
}

/// The folder's files of one kind, in play order, while no queue item plays.
struct FolderPlay {
    kind: Kind,
    paths: Vec<PathBuf>,
    q: Queue,
}

/// A title and length read for a file.
type Info = (PathBuf, Option<String>, Option<f64>);

/// The queue and what plays next.
#[derive(Default)]
pub(super) struct PlayQueue {
    /// The queue built or opened by the user (the panel lists it).
    pub(super) q: Queue,
    /// Its title, from the playlist it came from.
    title: Option<String>,
    /// The folder's files of the kind playing, when no queue item does.
    folder: Option<FolderPlay>,
    /// The movie or sound file whose end was acted on.
    ended: Option<ImageId>,
    /// A queue or folder item opened to play: it starts once it is on screen.
    autoplay: Option<PathBuf>,
    /// Titles and lengths being read, and the files asked for.
    info_rx: Option<mpsc::Receiver<Info>>,
    asked: HashSet<PathBuf>,
    /// "Save queue as" answered.
    save_rx: Option<mpsc::Receiver<PathBuf>>,
    /// The panel's list is out of date.
    dirty: bool,
}

/// Whether `p` is a playlist: by its name, and for `.pls` (also a Gerber layer's) by its
/// `[playlist]` line.
pub(super) fn is_playlist(p: &Path) -> bool {
    match specular_playlist::Kind::of_path(p) {
        None => false,
        Some(specular_playlist::Kind::Pls) => {
            use std::io::Read;
            let mut head = [0u8; 256];
            let n = std::fs::File::open(p)
                .and_then(|mut f| f.read(&mut head))
                .unwrap_or(0);
            specular_playlist::Kind::probe(&head[..n]) == Some(specular_playlist::Kind::Pls)
        }
        Some(_) => true,
    }
}

/// The playable items among `paths`: sound files and videos as they are, a folder's sound
/// files and videos (not its subfolders') in natural order, and a playlist's items.
pub(super) fn expand(paths: &[PathBuf]) -> Vec<Entry> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(p)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|f| f.is_file() && Kind::of(f).is_some())
                .collect();
            files.sort_by(|a, b| natural(a, b));
            out.extend(files.into_iter().map(Entry::file));
        } else if is_playlist(p) {
            if let Ok(list) = specular_playlist::read_file(p) {
                out.extend(list.entries);
            }
        } else if Kind::of(p).is_some() {
            out.push(Entry::file(p.clone()));
        }
    }
    out
}

/// File names in natural order ("2" before "10"), by the user's collation.
fn natural(a: &Path, b: &Path) -> std::cmp::Ordering {
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    specular_i18n::I18n::global().natural_cmp(&name(a), &name(b))
}

/// The files of `kind` among `entries` (the folder, or the gallery's selection), in order.
pub(super) fn candidates(entries: &[PathBuf], kind: Kind) -> Vec<PathBuf> {
    entries
        .iter()
        .filter(|p| Kind::of(p) == Some(kind))
        .cloned()
        .collect()
}

/// The title and length of a sound file or video, from its tags and stream headers.
pub(super) fn read_info(registry: &specular_decode::Registry, p: &Path) -> Info {
    let facts = registry.open_path(p).ok().and_then(|s| s.video().cloned());
    let Some(f) = facts else {
        return (p.to_path_buf(), None, None);
    };
    let duration = f.duration_us.map(|us| us as f64 / 1e6);
    let field = |k: &str| {
        f.sound
            .as_ref()?
            .fields
            .iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let title = match (field("artist"), field("title")) {
        (Some(a), Some(t)) => Some(t!("queue-artist-title", artist = a, title = t)),
        (None, Some(t)) => Some(t),
        _ => None,
    };
    (p.to_path_buf(), title, duration)
}

/// The play order of the folder's files `list`.
pub(super) fn folder_queue(
    list: &[PathBuf],
    repeat: PlayRepeat,
    shuffle: bool,
    seed: u64,
) -> Queue {
    let mut q = Queue::new(seed);
    q.set_repeat(repeat_of(repeat));
    q.append(list.iter().cloned().map(Entry::file).collect());
    q.set_shuffle(shuffle);
    q
}

fn repeat_of(r: PlayRepeat) -> Repeat {
    match r {
        PlayRepeat::Off => Repeat::Off,
        PlayRepeat::All => Repeat::All,
        PlayRepeat::One => Repeat::One,
    }
}

/// A seed for shuffling: the clock's nanoseconds.
fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x5EED, |d| d.as_nanos() as u64)
}

impl PlayQueue {
    pub(super) fn new() -> PlayQueue {
        PlayQueue {
            q: Queue::new(seed()),
            ..PlayQueue::default()
        }
    }
}

impl App {
    /// The movie or sound file on screen: its image, file and whether it has a picture.
    fn queue_media(&self) -> Option<(ImageId, PathBuf, bool)> {
        let a = self.anim.as_ref()?;
        let m = a.movie.as_ref()?;
        if Some(a.id) != self.current_id() {
            return None;
        }
        let path = self.images.get(&a.id)?.session.as_ref()?.source().path();
        Some((a.id, path.to_path_buf(), m.picture))
    }

    /// Each tick: follows the file on screen into or out of the queue, starts an item opened
    /// to play, and at the end of one plays the next.
    pub(super) fn queue_tick(&mut self) {
        self.queue_poll();
        let s = &self.settings.viewing;
        let (repeat, shuffle) = (repeat_of(s.play_repeat), s.play_shuffle);
        let pq = &mut self.queue;
        if pq.q.repeat() != repeat || pq.q.shuffle() != shuffle {
            pq.q.set_repeat(repeat);
            pq.q.set_shuffle(shuffle);
            pq.dirty = true;
        }
        if let Some(f) = pq.folder.as_mut() {
            f.q.set_repeat(repeat);
            f.q.set_shuffle(shuffle);
        }
        let media = self.queue_media();
        let order = media.as_ref().map(|_| PlayOrder {
            repeat: s.play_repeat,
            shuffle,
        });
        if self.chrome.play_order() != order {
            self.chrome.set_play_order(order);
            self.redraw = true;
        }
        let current_path = self.current.and_then(|i| self.entries.get(i)).cloned();
        // A network stream's piece stands for the queue's web item playing.
        let streaming = current_path.as_ref().is_some_and(|p| self.stream_owns(p));
        // The file on screen is a queue item, or the queue waits.
        if let Some(p) = &current_path {
            let pq = &mut self.queue;
            // (The item playing may be a second copy of a file further up.)
            let on_current =
                streaming || pq.q.current_entry().and_then(|e| e.location.path()) == Some(p);
            match pq.q.find(p) {
                _ if on_current => {}
                Some(i) => {
                    pq.q.play(i);
                    pq.dirty = true;
                }
                None if pq.q.current().is_some() => {
                    pq.q.detach();
                    pq.dirty = true;
                }
                _ => {}
            }
            if pq.autoplay.as_ref().is_some_and(|a| a != p) {
                pq.autoplay = None;
            }
        }
        if self.queue.dirty {
            self.queue.dirty = false;
            self.queue_refresh_panel();
        }
        let Some((id, path, picture)) = media else {
            return;
        };
        let now = Instant::now();
        if self.queue.autoplay.as_deref() == Some(path.as_path())
            && let Some(a) = self.anim.as_mut()
        {
            self.queue.autoplay = None;
            if !a.is_playing() {
                a.toggle(now);
                self.redraw = true;
            }
        }
        let finished = self.anim.as_ref().is_some_and(crate::anim::Anim::finished);
        if !finished {
            if self.queue.ended == Some(id) {
                self.queue.ended = None;
            }
            return;
        }
        if self.queue.ended == Some(id) {
            return;
        }
        // A sound playing on into its primed next file (gapless): the move comes when the
        // next one is heard (`sound_fx.rs`).
        if self
            .anim
            .as_ref()
            .and_then(|a| a.sound())
            .is_some_and(specular_video::audio::AudioPlayer::has_next)
        {
            return;
        }
        self.queue.ended = Some(id);
        self.queue_on_end(&path, picture);
    }

    /// What plays after the sound file `path` on screen ends, without moving on to it (gapless
    /// play opens and primes it): the queue's next item, else the folder's next sound file;
    /// none with repeat one.
    pub(super) fn queue_up_next(&mut self, path: &Path) -> Option<PathBuf> {
        // A network stream's next piece (whatever repeat says: the pieces are one item).
        if self.stream_owns(path) {
            return self.stream_next_for(path);
        }
        if self.settings.viewing.play_repeat == PlayRepeat::One {
            return None;
        }
        let playable =
            |p: &Path| p.is_file() && !is_playlist(p) && Kind::of(p) == Some(Kind::Sound);
        if self.queue.q.current().is_some() {
            let i = self.queue.q.next_item(true)?;
            let p = self.queue.q.get(i)?.location.path()?.to_path_buf();
            return playable(&p).then_some(p);
        }
        if !self.settings.viewing.sound_play_next {
            return None;
        }
        let f = self.folder_play(path, Kind::Sound)?;
        let i = f.q.next_item(true)?;
        let p = f.q.get(i)?.location.path()?.to_path_buf();
        playable(&p).then_some(p)
    }

    /// A network stream from the queue ended: the queue's next item plays.
    pub(super) fn queue_after_stream(&mut self) {
        self.queue_go(true, true);
    }

    /// The next sound file is heard (gapless): the queue moves on to it as at an end.
    pub(super) fn queue_heard_next(&mut self, id: ImageId, path: &Path) {
        self.queue.ended = Some(id);
        self.queue_on_end(path, false);
    }

    /// The item on screen came to its end: the same again (repeat one), the queue's next,
    /// or the folder's next file of the kind.
    fn queue_on_end(&mut self, path: &Path, picture: bool) {
        // A network stream's piece: its next piece, not the queue's next item.
        if self.stream_on_end(path) {
            return;
        }
        if self.settings.viewing.play_repeat == PlayRepeat::One {
            if let Some(a) = self.anim.as_mut() {
                a.toggle(Instant::now());
            }
            self.queue.ended = None;
            self.redraw = true;
            return;
        }
        if self.queue.q.current().is_some() {
            self.queue_go(true, true);
            return;
        }
        let on = if picture {
            self.settings.viewing.video_play_next
        } else {
            self.settings.viewing.sound_play_next
        };
        let kind = if picture { Kind::Video } else { Kind::Sound };
        if on {
            self.folder_go(path, kind, true, true);
        }
    }

    /// Next (`forward`) or Previous asked for: the queue's when one of its items plays,
    /// else the folder's next file of the kind on screen. Previous later than 3 s into the
    /// item starts it over.
    pub(super) fn queue_step(&mut self, forward: bool) {
        if !forward
            && let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some())
            && a.time(Instant::now())
                .is_some_and(|t| t.as_secs_f64() > RESTART_AFTER)
        {
            a.seek_time(Duration::ZERO);
            self.redraw = true;
            return;
        }
        if self.queue.q.current().is_some() {
            self.queue_go(forward, false);
            return;
        }
        let Some((_, path, picture)) = self.queue_media() else {
            // Nothing plays: the folder's next or previous file.
            self.navigate(if forward { 1 } else { -1 });
            return;
        };
        let kind = if picture { Kind::Video } else { Kind::Sound };
        self.folder_go(&path, kind, forward, false);
    }

    /// Moves the queue on (or back) and opens the item; files no longer there are skipped.
    fn queue_go(&mut self, forward: bool, auto: bool) {
        let n = self.queue.q.len();
        for _ in 0..n.max(1) {
            let next = if forward {
                self.queue.q.advance(auto)
            } else {
                self.queue.q.go_back()
            };
            self.queue.dirty = true;
            let Some(i) = next else {
                if !auto {
                    self.queue_toast(if forward {
                        t!("queue-at-end")
                    } else {
                        t!("queue-at-start")
                    });
                }
                return;
            };
            if let Some(Location::Url(u)) = self.queue.q.get(i).map(|e| e.location.clone()) {
                self.queue.ended = None;
                self.stream_open(u, true);
                return;
            }
            let Some(p) = self
                .queue
                .q
                .get(i)
                .and_then(|e| e.location.path())
                .map(Path::to_path_buf)
            else {
                continue;
            };
            // A playlist inside a playlist is not opened (it would take the queue's place).
            if p.is_file() && !is_playlist(&p) {
                self.queue_open(p, !auto);
                return;
            }
            if !p.is_file() {
                let name = self.queue.q.get(i).map(Entry::name).unwrap_or_default();
                self.queue_toast(t!("queue-missing", name = name));
            }
        }
    }

    /// The folder's (or the gallery selection's) next or previous file of `kind` after
    /// `path`.
    fn folder_go(&mut self, path: &Path, kind: Kind, forward: bool, auto: bool) {
        let Some(f) = self.folder_play(path, kind) else {
            return;
        };
        let next = if forward {
            f.q.advance(auto)
        } else {
            f.q.go_back()
        };
        let Some(p) = next.and_then(|i| f.q.get(i)?.location.path().map(Path::to_path_buf)) else {
            if !auto {
                self.queue_toast(if forward {
                    t!("queue-at-end")
                } else {
                    t!("queue-at-start")
                });
            }
            return;
        };
        self.queue_open(p, !auto);
    }

    /// The folder's (or the gallery selection's) files of `kind`, in play order, at `path`.
    fn folder_play(&mut self, path: &Path, kind: Kind) -> Option<&mut FolderPlay> {
        let sel = self.gallery.targets();
        let list = if sel.len() >= 2 && sel.iter().any(|p| p == path) {
            candidates(&sel, kind)
        } else {
            candidates(&self.entries, kind)
        };
        let pq = &mut self.queue;
        let stale = pq
            .folder
            .as_ref()
            .is_none_or(|f| f.kind != kind || f.paths != list);
        if stale {
            let s = &self.settings.viewing;
            let q = folder_queue(&list, s.play_repeat, s.play_shuffle, seed());
            pq.folder = Some(FolderPlay {
                kind,
                paths: list,
                q,
            });
        }
        let f = pq.folder.as_mut()?;
        if let Some(i) = f.q.find(path)
            && f.q.current() != Some(i)
        {
            f.q.play(i);
        }
        Some(f)
    }

    /// Opens `p` to play: in the listing when it is there, else as a file of its own (its
    /// folder listed around it). Asked for by the user (`shown`), the viewer comes up over
    /// the gallery; at the end of an item the next one plays behind it.
    pub(super) fn queue_open(&mut self, p: PathBuf, shown: bool) {
        self.queue.autoplay = Some(p.clone());
        self.queue.ended = None;
        if shown {
            self.gallery_dismiss();
        }
        match self.index_of(&p) {
            Some(i) => self.go_to(i),
            None => self.open_target(p),
        }
        self.redraw = true;
    }

    fn queue_toast(&mut self, msg: String) {
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The queue's actions. Returns `false` for other actions.
    pub(super) fn queue_action(&mut self, a: Action) -> bool {
        match a {
            Action::NextTrack => self.queue_step(true),
            Action::PrevTrack => self.queue_step(false),
            Action::CycleRepeat => self.queue_command(QueueCmd::Repeat),
            Action::ToggleShuffle => self.queue_command(QueueCmd::Shuffle),
            Action::ToggleQueue => {
                if self.chrome.queue_open() {
                    self.chrome.close_modal();
                } else {
                    self.queue_refresh_panel();
                    self.chrome.show_queue();
                }
            }
            Action::QueuePlayNext | Action::QueueAdd => {
                let paths = if self.gallery_active() {
                    self.gallery.targets()
                } else {
                    self.current
                        .and_then(|i| self.entries.get(i))
                        .cloned()
                        .into_iter()
                        .collect()
                };
                self.queue_add_paths(&paths, a == Action::QueuePlayNext);
            }
            Action::SaveQueue => self.queue_save_dialog(),
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// Adds the playable items among `paths` to play next (`next`) or at the end, and says
    /// how many.
    pub(super) fn queue_add_paths(&mut self, paths: &[PathBuf], next: bool) {
        let entries = expand(paths);
        if entries.is_empty() {
            self.queue_toast(t!("queue-nothing-to-add"));
            return;
        }
        let n = entries.len() as i64;
        // An empty queue starts with what plays now, so the new items follow it.
        if self.queue.q.is_empty()
            && let Some((_, p, _)) = self.queue_media()
            && !entries
                .iter()
                .any(|e| e.location.path() == Some(p.as_path()))
        {
            self.queue.q.append(vec![Entry::file(p.clone())]);
            self.queue.q.play(0);
        }
        if next {
            self.queue.q.insert_next(entries);
        } else {
            self.queue.q.append(entries);
        }
        self.queue.dirty = true;
        self.queue_request_info();
        self.queue_toast(if next {
            t!("queue-added-next", count = n)
        } else {
            t!("queue-added", count = n)
        });
    }

    /// Opens a playlist as the queue and plays its first item. Whether `p` was one.
    pub(super) fn queue_open_playlist(&mut self, p: &Path) -> bool {
        if !p.is_file() || !is_playlist(p) {
            return false;
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let list = match specular_playlist::read_file(p) {
            Ok(l) => l,
            Err(e) => {
                specular_log::debug!(target: specular_log::FOLDER, "playlist {}: {e}", p.display());
                let why = match e {
                    specular_playlist::ReadFileError::Io(_) => t!("queue-playlist-unreadable"),
                    _ => t!("queue-playlist-damaged"),
                };
                self.queue_toast(t!("queue-playlist-failed", name = name, reason = why));
                return true;
            }
        };
        let total = list.entries.len() as i64;
        self.queue.q.clear();
        self.queue.q.append(list.entries);
        self.queue.title = list
            .title
            .or_else(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()));
        self.queue.dirty = true;
        self.queue_request_info();
        if self.queue.q.next_item(true).is_none() {
            // Nothing plays here (empty, network only or every file missing): the list shows
            // what it holds.
            self.queue_toast(t!("queue-playlist-nothing", name = name));
            self.queue_refresh_panel();
            self.chrome.show_queue();
            return true;
        }
        self.queue_toast(t!("queue-playlist-opened", name = name, count = total));
        self.queue_go(true, true);
        true
    }

    /// A choice in the queue panel (or the repeat and shuffle keys).
    pub(super) fn queue_command(&mut self, c: QueueCmd) {
        match c {
            QueueCmd::Play(i) => {
                if self.queue.q.play(i)
                    && let Some(p) = self.queue.q.get(i).and_then(|e| e.location.path())
                {
                    let p = p.to_path_buf();
                    if p.is_file() {
                        self.queue_open(p, true);
                    } else {
                        let name = self.queue.q.get(i).map(Entry::name).unwrap_or_default();
                        self.queue_toast(t!("queue-missing", name = name));
                    }
                } else if let Some(Location::Url(u)) =
                    self.queue.q.get(i).map(|e| e.location.clone())
                    && self.queue.q.current() == Some(i)
                {
                    self.stream_open(u, true);
                } else if self.queue.q.get(i).is_some_and(|e| !e.playable()) {
                    self.queue_toast(t!("queue-network-not-played"));
                }
            }
            QueueCmd::Remove(i) => {
                self.queue.q.remove(i);
            }
            QueueCmd::Move { from, to } => self.queue.q.move_item(from, to),
            QueueCmd::Clear => {
                if self.queue.q.is_empty() {
                    return;
                }
                let n = self.queue.q.len() as i64;
                self.chrome.confirm_destructive(
                    t!("queue-clear-ask", count = n),
                    t!("queue-clear-detail"),
                    t!("queue-clear-ok"),
                    super::items::QUEUE_CLEAR,
                );
            }
            QueueCmd::Save => self.queue_save_dialog(),
            QueueCmd::Repeat => {
                let next = match self.settings.viewing.play_repeat {
                    PlayRepeat::Off => PlayRepeat::All,
                    PlayRepeat::All => PlayRepeat::One,
                    PlayRepeat::One => PlayRepeat::Off,
                };
                self.store
                    .update(Origin::Program, |s| s.viewing.play_repeat = next);
                // The store's change reaches `self.settings` with the next settings event; a
                // second press before it counts from this one.
                std::sync::Arc::make_mut(&mut self.settings)
                    .viewing
                    .play_repeat = next;
                self.queue_toast(specular_widgets::repeat_label(next));
            }
            QueueCmd::Shuffle => {
                let on = !self.settings.viewing.play_shuffle;
                self.store
                    .update(Origin::Program, |s| s.viewing.play_shuffle = on);
                std::sync::Arc::make_mut(&mut self.settings)
                    .viewing
                    .play_shuffle = on;
                self.queue_toast(specular_widgets::shuffle_label(on));
            }
        }
        self.queue.dirty = true;
        self.redraw = true;
    }

    /// The confirmation's accept. True when `id` was ours.
    pub(super) fn queue_item(&mut self, id: u64) -> bool {
        if id != super::items::QUEUE_CLEAR {
            return false;
        }
        self.queue.q.clear();
        self.queue.title = None;
        self.queue.dirty = true;
        self.queue_toast(t!("queue-cleared"));
        true
    }

    /// The panel's list from the queue.
    fn queue_refresh_panel(&mut self) {
        let q = &self.queue.q;
        let mut total = 0.0;
        let mut known = true;
        let rows = q
            .items()
            .iter()
            .enumerate()
            .map(|(i, e)| {
                match e.duration {
                    Some(d) => total += d,
                    None => known &= !e.playable(),
                }
                let detail = match &e.location {
                    Location::File(p) => p
                        .parent()
                        .and_then(Path::file_name)
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    Location::Url(u) if e.playable() => {
                        t!("queue-stream-item", host = specular_core::net::host_of(u))
                    }
                    Location::Url(u) => t!("queue-network-item", address = u.as_str()),
                };
                QueueRow {
                    title: e.title.clone().unwrap_or_else(|| e.name()),
                    detail,
                    duration: e.duration.map(super::playback::clock).unwrap_or_default(),
                    current: q.current() == Some(i),
                    playable: e.playable(),
                }
            })
            .collect::<Vec<_>>();
        let n = rows.len() as i64;
        let summary = if n == 0 {
            String::new()
        } else if known && total > 0.0 {
            t!(
                "queue-summary-time",
                count = n,
                time = super::playback::clock(total)
            )
        } else {
            t!("queue-summary", count = n)
        };
        self.chrome.set_queue(QueueView { rows, summary });
        self.redraw = true;
    }

    /// Reads the titles and lengths not known yet, on a worker.
    fn queue_request_info(&mut self) {
        let pq = &mut self.queue;
        let want: Vec<PathBuf> =
            pq.q.items()
                .iter()
                .filter(|e| e.duration.is_none())
                .filter_map(|e| e.location.path().map(Path::to_path_buf))
                .filter(|p| !pq.asked.contains(p))
                .collect();
        if want.is_empty() {
            return;
        }
        pq.asked.extend(want.iter().cloned());
        let (tx, rx) = mpsc::channel();
        pq.info_rx = Some(rx);
        let waker = self.loader.waker();
        let registry = self.loader.registry();
        let _ = specular_thread::builder("queue-info").spawn(move || {
            for p in want {
                if tx.send(read_info(&registry, &p)).is_err() {
                    return;
                }
                waker();
            }
        });
    }

    /// Titles and lengths read, and the save dialog's answer.
    fn queue_poll(&mut self) {
        let pq = &mut self.queue;
        if let Some(rx) = &pq.info_rx {
            loop {
                match rx.try_recv() {
                    Ok((p, title, duration)) => {
                        for i in 0..pq.q.len() {
                            if let Some(e) = pq.q.get_mut(i)
                                && e.location.path() == Some(p.as_path())
                            {
                                if e.title.is_none() {
                                    e.title.clone_from(&title);
                                }
                                if e.duration.is_none() {
                                    e.duration = duration;
                                }
                            }
                        }
                        pq.dirty = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        pq.info_rx = None;
                        break;
                    }
                }
            }
        }
        if let Some(rx) = &pq.save_rx
            && let Ok(dest) = rx.try_recv()
        {
            pq.save_rx = None;
            self.queue_save(&dest);
        }
    }

    /// "Save queue as…": where, and as M3U8 or XSPF (by the name's extension).
    fn queue_save_dialog(&mut self) {
        if self.queue.q.is_empty() {
            self.queue_toast(t!("queue-empty-short"));
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.queue.save_rx = Some(rx);
        let title = t!("queue-save-title");
        let name = format!(
            "{}.m3u8",
            self.queue
                .title
                .clone()
                .unwrap_or_else(|| t!("queue-default-name"))
        );
        let dir = self
            .queue
            .q
            .items()
            .iter()
            .find_map(|e| e.location.path()?.parent().map(Path::to_path_buf));
        let m3u = t!("queue-format-m3u8");
        let xspf = t!("queue-format-xspf");
        self.loader.background("dlg-save-queue", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .set_file_name(&name)
                .add_filter(&m3u, &["m3u8", "m3u"])
                .add_filter(&xspf, &["xspf"]);
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.save_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
    }

    /// Writes the queue to `dest`: XSPF for `.xspf`, else M3U8.
    pub(super) fn queue_save(&mut self, dest: &Path) {
        let xspf = dest
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xspf"));
        let dest = if dest.extension().is_none() {
            dest.with_extension("m3u8")
        } else {
            dest.to_path_buf()
        };
        let title = self.queue.title.as_deref();
        let text = if xspf {
            specular_playlist::write_xspf(title, self.queue.q.items(), &dest)
        } else {
            specular_playlist::write_m3u8(title, self.queue.q.items(), &dest)
        };
        let name = dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let msg = match std::fs::write(&dest, text) {
            Ok(()) => t!("queue-saved", name = name),
            Err(e) => t!(
                "queue-save-failed",
                name = name,
                reason = crate::error_text::io_reason(&e)
            ),
        };
        self.queue_toast(msg);
    }
}

#[cfg(test)]
mod tests;

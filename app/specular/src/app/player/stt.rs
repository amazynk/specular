//! Generated subtitles: the movie's speech turned into a subtitle track on this computer by
//! the Local AI pack's Speech model (Whisper, `specular_infer::whisper`).
//!
//! "Generate subtitles…" (the CC menu, the palette) asks for the language spoken (or to
//! detect it, or to translate to English), then a background job reads the sound at 16 kHz
//! mono ([`specular_video::speech_sound`]), skips silence ([`vad`]) and reads it 30 s at a
//! time: first from the playhead on, then the rest from the start. Cues appear as the
//! track "Generated (English)" as they are read; the job's progress shows in the CC menu
//! and in the background-work panel. Seeking far ahead moves the job there.
//!
//! Finished subtitles are kept in the cache (`<cache>/subtitles/<fingerprint>.<lang>.srt`,
//! the fingerprint from the file's size and three 1 MiB samples, so a renamed or moved
//! copy still finds them); reopening the movie offers them again. "Save subtitles…" writes
//! them beside the movie as SRT or WebVTT, through the save dialog (never unasked).
//!
//! What a job made is kept as it goes (`<fingerprint>.<asked>[.to-en].partial.srt` and its
//! `.state`: the language read and the stretches done). Reopening a movie whose job was
//! interrupted continues it from there; a job the user stopped shows its cues and continues
//! when the same choice is asked for again. Done stretches are never read twice.
//!
//! A sound file without a picture (a song, a recording) gets a transcript the same way,
//! shown in the sound page's lyrics column ([`Lyrics`] from `Generated`) while it is read and
//! when reopened (the file's own lyrics win unless the user asked for the transcript); "Save
//! subtitles…" writes it as SRT, WebVTT or LRC.
//!
//! Nothing downloads here: without the Speech model the sheet says where to get it.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use parking_lot::Mutex;
use specular_ai::speech::SpeechFiles;
use specular_core::Source;
use specular_core::work::{Kind, Step, Unit, Work};
use specular_gfx::{Icon, wgpu};
use specular_i18n::t;
use specular_infer::whisper::decode::{Options, Task};
use specular_infer::whisper::{cues, vad};
use specular_subs::{Body, Span, TextCue, Track};
use specular_video::lyrics::{From as LyricsFrom, Line as LyricsLine, Lyrics};
use specular_widgets::PaletteItem;

use super::subs;
use crate::app::App;
use crate::app::ai::device::Session as AiSession;
use crate::loader::ImageId;

/// The language sheet's rows: detect, translate, then a language each; the settings row.
pub(crate) const ITEM_BASE: u64 = (1 << 44) + (1 << 22);
const ITEM_DETECT: u64 = ITEM_BASE;
const ITEM_TRANSLATE: u64 = ITEM_BASE + 1;
const ITEM_LANG0: u64 = ITEM_BASE + 2;
const ITEM_SETTINGS: u64 = ITEM_BASE + 500;
/// Seconds read per step of the job (plus one window more, so the last can finish).
const BLOCK: f64 = 120.0;
const RATE: usize = 16_000;

/// What the job has read so far.
#[derive(Default)]
struct Progress {
    cues: Vec<cues::Cue>,
    /// Bumped with every change of `cues`.
    version: u64,
    /// Seconds of sound read, of `total`.
    done: f64,
    total: f64,
    /// The language read (given or detected).
    language: Option<String>,
    /// The cache file written when all was read.
    finished: Option<PathBuf>,
    error: Option<String>,
}

struct Shared {
    /// The playhead, in milliseconds of movie time.
    playhead: AtomicU64,
    cancel: AtomicBool,
    /// Stopped by the user (not continued on its own when the file is opened again).
    stopped: AtomicBool,
    progress: Mutex<Progress>,
}

impl Shared {
    fn new(playhead_ms: u64) -> Shared {
        Shared {
            playhead: AtomicU64::new(playhead_ms),
            cancel: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            progress: Mutex::new(Progress::default()),
        }
    }
}

/// What a job reads: the movie on screen, or the sound file on the sound page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Movie,
    Sound(ImageId),
}

/// A job under way for the movie on screen.
struct Job {
    /// The track's source path (a placeholder until the cache file is written).
    path: PathBuf,
    task: Task,
    shared: Arc<Shared>,
    seen: u64,
    target: Target,
}

/// What a job made so far, kept between runs.
#[derive(Clone, Debug, Default, PartialEq)]
struct Partial {
    /// The language read (asked for or detected).
    language: Option<String>,
    /// Stretches of sound done (seconds from the sound's start).
    covered: Vec<(f64, f64)>,
    /// Its cues (movie time).
    cues: Vec<cues::Cue>,
    stopped: bool,
}

/// A transcript of a sound file: (the file, its cues, language, translated).
type SoundText = (ImageId, Arc<Vec<cues::Cue>>, Option<String>, bool);

/// A sound file's cached transcript or unfinished job, looked up when it is shown.
struct SoundFound {
    id: ImageId,
    text: Option<(Vec<cues::Cue>, String, bool)>,
    partial: Option<(Option<String>, Task, Partial)>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }
}

/// Cache files found for a movie: (path, language, translated).
type Found = Vec<(PathBuf, String, bool)>;

/// Unfinished jobs found for a file: (language asked for, task, what was made).
type FoundPartial = Vec<(Option<String>, Task, Partial)>;

#[derive(Default)]
pub(crate) struct Stt {
    job: Option<Job>,
    lookup: Option<mpsc::Receiver<(PathBuf, Found, FoundPartial)>>,
    saved: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    jobs: u64,
    /// The sound file whose transcript was looked up, the lookup, and the transcript.
    sound_seen: Option<ImageId>,
    sound_lookup: Option<mpsc::Receiver<SoundFound>>,
    sound_text: Option<SoundText>,
    /// The user asked for the transcript: it shows over the file's own lyrics.
    sound_forced: bool,
}

/// The folder generated subtitles are kept in.
fn cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        if cfg!(test) {
            return std::env::temp_dir().join(format!("specular-stt-cache-{}", std::process::id()));
        }
        specular_settings::Locations::detect()
            .cache_dir
            .join("subtitles")
    })
    .clone()
}

/// The file's fingerprint: FNV-1a over its size and 1 MiB at its start, middle and end.
/// A web file still being fetched is never read where its bytes are not there yet: then
/// `WouldBlock` (see [`fingerprint_waiting`]).
pub(crate) fn fingerprint(path: &Path) -> std::io::Result<String> {
    fingerprint_with(path, None)
}

/// Waits this long for a part of a file still being fetched, a few times.
const FILL_WAIT: Duration = Duration::from_secs(10);
const FILL_TRIES: u32 = 3;

/// [`fingerprint`] off the UI thread: the parts of a web file still being fetched are asked
/// for and waited for (tried again a few times while the fetch goes on).
pub(crate) fn fingerprint_waiting(path: &Path) -> std::io::Result<String> {
    let mut r = fingerprint_with(path, Some(FILL_WAIT));
    for _ in 1..FILL_TRIES {
        match &r {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if specular_core::fill::of_path(path).is_none() {
                    // The fetch ended (whole, or stopped): one plain read decides.
                    return fingerprint_with(path, None);
                }
                r = fingerprint_with(path, Some(FILL_WAIT));
            }
            _ => break,
        }
    }
    r
}

fn fingerprint_with(path: &Path, wait: Option<Duration>) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let filling = specular_core::fill::of_path(path).filter(|f| !f.complete());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h ^= u64::from(x);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&len.to_le_bytes());
    const PART: u64 = 1 << 20;
    let mut buf = vec![0u8; PART as usize];
    for at in [0, len.saturating_sub(PART) / 2, len.saturating_sub(PART)] {
        if let Some(fl) = &filling {
            let end = (at + PART).min(len);
            let there = fl.has(at, end) || wait.is_some_and(|d| fl.wait_quietly(at, end, d));
            if !there {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "part of the file is still being fetched",
                ));
            }
        }
        f.seek(SeekFrom::Start(at))?;
        let n = f.read(&mut buf)?;
        eat(&buf[..n]);
    }
    Ok(format!("{h:016x}"))
}

/// The cache file name for a fingerprint, language and task.
fn cache_name(hash: &str, lang: &str, task: Task) -> String {
    match task {
        Task::Transcribe => format!("{hash}.{lang}.srt"),
        Task::Translate => format!("{hash}.{lang}.to-en.srt"),
    }
}

/// The language asked for in a partial file's name ("auto": detect).
fn asked_name(asked: Option<&str>) -> &str {
    asked.unwrap_or("auto")
}

/// An unfinished job's files: its cues (SRT) and its state.
fn partial_paths(dir: &Path, hash: &str, asked: Option<&str>, task: Task) -> (PathBuf, PathBuf) {
    let to = if task == Task::Translate {
        ".to-en"
    } else {
        ""
    };
    let base = format!("{hash}.{}{to}.partial", asked_name(asked));
    (
        dir.join(format!("{base}.srt")),
        dir.join(format!("{base}.state")),
    )
}

/// The language asked for and the task of an unfinished job's state file named for `hash`.
fn parse_partial_name(hash: &str, name: &str) -> Option<(Option<String>, Task)> {
    let rest = name
        .strip_prefix(hash)?
        .strip_prefix('.')?
        .strip_suffix(".partial.state")?;
    let (asked, task) = match rest.strip_suffix(".to-en") {
        Some(l) => (l, Task::Translate),
        None => (rest, Task::Transcribe),
    };
    if asked.is_empty() || asked.contains('.') {
        return None;
    }
    Some(((asked != "auto").then(|| asked.to_owned()), task))
}

/// The state file's text.
fn partial_state(p: &Partial) -> String {
    let mut out = String::from("specular-stt 1\n");
    out.push_str(&format!(
        "language {}\nstopped {}\n",
        p.language.as_deref().unwrap_or("-"),
        u8::from(p.stopped)
    ));
    for (a, b) in &p.covered {
        out.push_str(&format!("covered {a:.3} {b:.3}\n"));
    }
    out
}

/// Reads an unfinished job (its state and cues); `None` when either is missing or unreadable.
fn read_partial(srt: &Path, state: &Path) -> Option<Partial> {
    let text = std::fs::read_to_string(state).ok()?;
    let mut lines = text.lines();
    if lines.next()? != "specular-stt 1" {
        return None;
    }
    let mut p = Partial::default();
    for l in lines {
        let mut w = l.split_whitespace();
        match (w.next(), w.next(), w.next()) {
            (Some("language"), Some(v), None) if v != "-" => p.language = Some(v.to_owned()),
            (Some("stopped"), Some(v), None) => p.stopped = v == "1",
            (Some("covered"), Some(a), Some(b)) => {
                let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
                if a.is_finite() && b.is_finite() && b >= a {
                    cover(&mut p.covered, a, b);
                }
            }
            _ => {}
        }
    }
    let srt = std::fs::read_to_string(srt).unwrap_or_default();
    if let Ok(t) = specular_subs::srt::parse(&srt) {
        p.cues = from_track(&t);
    }
    Some(p)
}

/// Writes an unfinished job's files (each replaced whole).
fn write_partial(dir: &Path, hash: &str, asked: Option<&str>, task: Task, p: &Partial) {
    let (srt, state) = partial_paths(dir, hash, asked, task);
    let put = |path: &Path, text: String| {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path))
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    // the cues first: a state file always has its cues
    let r = put(&srt, specular_subs::write::srt(&to_track(&p.cues)))
        .and_then(|_| put(&state, partial_state(p)));
    if let Err(e) = r {
        specular_log::debug!(target: specular_log::CACHE, "stt: partial results not kept: {e}");
    }
}

/// The language and task of a cache file named for `hash`.
fn parse_cache_name(hash: &str, name: &str) -> Option<(String, bool)> {
    let rest = name
        .strip_prefix(hash)?
        .strip_prefix('.')?
        .strip_suffix(".srt")?;
    match rest.strip_suffix(".to-en") {
        Some(l) if !l.is_empty() && !l.contains('.') => Some((l.to_owned(), true)),
        None if !rest.is_empty() && !rest.contains('.') => Some((rest.to_owned(), false)),
        _ => None,
    }
}

/// The track's menu name.
fn track_name(lang: Option<&str>, translated: bool) -> String {
    match (lang, translated) {
        (Some(l), true) => t!("stt-track-translated", language = subs::language_name(l)),
        (Some(l), false) => t!("stt-track", language = subs::language_name(l)),
        (None, _) => t!("stt-track-pending"),
    }
}

fn source_for(path: PathBuf, lang: Option<&str>, translated: bool) -> subs::Source {
    subs::Source {
        origin: subs::Origin::File(path),
        lang: None,
        name: Some(track_name(lang, translated)),
        forced: false,
        sdh: false,
        caption: None,
    }
}

/// Whisper's cues as a subtitle track (movie time).
fn to_track(cues: &[cues::Cue]) -> Track {
    Track::new(
        cues.iter()
            .map(|c| specular_subs::Cue {
                start_us: (c.start * 1e6) as i64,
                end_us: (c.end * 1e6) as i64,
                body: Body::Text(TextCue {
                    lines: c.text.lines().map(|l| vec![Span::plain(l)]).collect(),
                    align: 2,
                    pos: None,
                    margin_v: None,
                }),
            })
            .collect(),
    )
}

/// A subtitle track's text cues as Whisper's (seconds).
fn from_track(t: &Track) -> Vec<cues::Cue> {
    t.cues
        .iter()
        .filter_map(|c| {
            Some(cues::Cue {
                start: c.start_us as f64 / 1e6,
                end: c.end_us as f64 / 1e6,
                text: c.text()?.plain_text(),
            })
        })
        .collect()
}

/// A transcript as synced lyrics: a line per cue, its lines joined.
fn to_lyrics(cues: &[cues::Cue]) -> Lyrics {
    Lyrics {
        lines: cues
            .iter()
            .map(|c| LyricsLine {
                at: Some(c.start),
                text: c.text.lines().map(str::trim).collect::<Vec<_>>().join(" "),
                words: Vec::new(),
            })
            .collect(),
        synced: true,
        from: LyricsFrom::Generated,
    }
}

/// The first second at or after `from` (else from 0) no span of `covered` (sorted, merged)
/// holds, and how long until the next covered span; `None` when everything is covered.
fn next_gap(covered: &[(f64, f64)], from: f64, total: f64) -> Option<(f64, f64)> {
    let gap_at = |t: f64| -> Option<(f64, f64)> {
        let mut t = t;
        for &(a, b) in covered {
            if t >= a && t < b {
                t = b;
            }
        }
        if t >= total - 0.5 {
            return None;
        }
        let next = covered
            .iter()
            .map(|c| c.0)
            .filter(|&a| a > t)
            .fold(total, f64::min);
        Some((t, next - t))
    };
    gap_at(from.clamp(0.0, total)).or_else(|| gap_at(0.0))
}

fn cover(covered: &mut Vec<(f64, f64)>, a: f64, b: f64) {
    covered.push((a, b));
    covered.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in covered.iter() {
        match out.last_mut() {
            Some(l) if a <= l.1 + 1e-6 => l.1 = l.1.max(b),
            _ => out.push((a, b)),
        }
    }
    *covered = out;
}

/// The job: reads the sound around the playhead first, then the rest. What an earlier run
/// of the same choice made (`<hash>.<asked>….partial.*`) is taken up, its stretches not
/// read again; what this one makes is kept there as it goes.
#[allow(clippy::too_many_arguments)]
fn run(
    shared: &Shared,
    source: Arc<Source>,
    sound_track: Option<u64>,
    language: Option<String>,
    task: Task,
    gpu: Option<(wgpu::Device, wgpu::Queue)>,
    files: SpeechFiles,
    wake: &(dyn Fn() + Send + Sync),
) -> Result<Option<PathBuf>, String> {
    let movie = source.path().to_path_buf();
    let hash = fingerprint_waiting(&movie).map_err(|e| e.to_string())?;
    let dir = cache_dir();
    let asked = language.clone();
    let (part_srt, part_state) = partial_paths(&dir, &hash, asked.as_deref(), task);
    let earlier = read_partial(&part_srt, &part_state).unwrap_or_default();
    let mut sound = specular_video::speech_sound::SpeechSound::open(source, sound_track)
        .map_err(|_| t!("stt-no-sound"))?;
    let (total, offset) = (sound.duration(), sound.offset());
    shared.progress.lock().total = total;
    let work = Work::start(Kind::Speech, Step::Transcribing);
    work.set_unit(Unit::Items);
    if let Some(n) = movie.file_name() {
        work.set_item(Some(n.to_string_lossy().as_ref()));
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    // After a GPU reset the model is loaded again on the new device (or the CPU).
    let mut ai = AiSession::new(gpu);
    let load = |ai: &mut AiSession| {
        specular_ai::speech::load(
            &files,
            ai.context(),
            specular_core::paths::selector(),
            threads,
        )
        .map_err(|e| e.to_string())
    };
    let mut w = load(&mut ai)?;
    let cancel = || shared.cancel.load(Ordering::Relaxed);
    let mut opts = Options {
        // a language detected before is kept
        language: language.or(earlier.language.clone()),
        task,
        ..Options::default()
    };
    let mut covered: Vec<(f64, f64)> = earlier.covered.clone();
    if !earlier.cues.is_empty() || !covered.is_empty() {
        let mut p = shared.progress.lock();
        p.cues = earlier.cues.clone();
        p.version += 1;
        p.language = opts.language.clone();
        p.done = covered.iter().map(|c| c.1 - c.0).sum();
        drop(p);
        wake();
    }
    // What was made so far, kept for the next run (and marked when the user stopped it).
    let keep = |covered: &[(f64, f64)]| {
        let p = shared.progress.lock();
        let part = Partial {
            language: p.language.clone(),
            covered: covered.to_vec(),
            cues: p.cues.clone(),
            stopped: shared.stopped.load(Ordering::Relaxed),
        };
        drop(p);
        write_partial(&dir, &hash, asked.as_deref(), task, &part);
    };
    loop {
        if cancel() {
            keep(&covered);
            return Ok(None);
        }
        let ph = shared.playhead.load(Ordering::Relaxed) as f64 / 1000.0 - offset;
        let Some((t, gap)) = next_gap(&covered, ph, total) else {
            break;
        };
        let audio = sound.read(t, gap.min(BLOCK + 30.0));
        if audio.len() < RATE / 10 {
            cover(&mut covered, t, t + gap.min(BLOCK));
            continue;
        }
        let regions = vad::speech(&audio);
        let windows = vad::windows(&regions, &audio, 30 * RATE);
        let limit = ((gap.min(BLOCK)) * RATE as f64) as usize;
        let mut end = t + (limit.min(audio.len()) as f64 / RATE as f64);
        let speech: Vec<(f64, f64)> = regions
            .iter()
            .map(|r| {
                (
                    t + r.start as f64 / RATE as f64,
                    t + r.end as f64 / RATE as f64,
                )
            })
            .collect();
        for win in windows.iter().filter(|w| w.start < limit) {
            let off = t + win.start as f64 / RATE as f64;
            let mut lang = None;
            if ai.stale() {
                w = load(&mut ai)?;
            }
            // A window the GPU lost on the way is read once more.
            let mut tries = 0;
            let read = loop {
                tries += 1;
                match w.transcribe(
                    &audio[win.start..win.end],
                    &opts,
                    &cancel,
                    &mut |_, l, _| lang = Some(l),
                ) {
                    Err(_) if ai.stale() && tries < 2 => w = load(&mut ai)?,
                    r => break r,
                }
            };
            let Some(segs) = read.map_err(|e| e.to_string())? else {
                keep(&covered);
                return Ok(None);
            };
            let segs: Vec<_> = segs
                .into_iter()
                .map(|mut s| {
                    s.start += off as f32;
                    s.end += off as f32;
                    s
                })
                .collect();
            if opts.language.is_none() && !segs.is_empty() {
                opts.language = lang.map(str::to_owned);
            }
            end = end.max(t + win.end as f64 / RATE as f64);
            // this window and the silence before it are done
            let win_end = t + win.end as f64 / RATE as f64;
            cover(&mut covered, t, win_end.min(t + gap));
            let new = cues::cues(&segs, &speech);
            {
                let mut p = shared.progress.lock();
                p.cues.extend(new.into_iter().map(|mut c| {
                    c.start += offset;
                    c.end += offset;
                    c
                }));
                p.cues.sort_by(|a, b| a.start.total_cmp(&b.start));
                p.version += 1;
                p.language = opts.language.clone();
                p.done = covered.iter().map(|c| c.1 - c.0).sum::<f64>();
            }
            keep(&covered);
            work.set_progress(
                shared.progress.lock().done as u64,
                Some(total.max(1.0) as u64),
            );
            wake();
            if cancel() {
                keep(&covered);
                return Ok(None);
            }
        }
        cover(&mut covered, t, end.min(t + gap));
        keep(&covered);
        let done: f64 = covered.iter().map(|c| c.1 - c.0).sum();
        shared.progress.lock().done = done;
        work.set_progress(done as u64, Some(total.max(1.0) as u64));
        wake();
    }
    // All read: kept in the cache; the partial files go.
    let p = shared.progress.lock();
    let lang = p.language.clone().unwrap_or_else(|| "und".into());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(cache_name(&hash, &lang, task));
    let text = specular_subs::write::srt(&to_track(&p.cues));
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, text)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&part_state);
    let _ = std::fs::remove_file(&part_srt);
    Ok(Some(path))
}

/// The finished transcripts and unfinished jobs in the cache for the file at `path`.
fn look_up(path: &Path) -> (Found, FoundPartial) {
    let mut found = Vec::new();
    let mut partial = Vec::new();
    let dir = cache_dir();
    if let Ok(hash) = fingerprint_waiting(path)
        && let Ok(rd) = std::fs::read_dir(&dir)
    {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some((lang, tr)) = parse_cache_name(&hash, &name) {
                found.push((e.path(), lang, tr));
            } else if let Some((asked, task)) = parse_partial_name(&hash, &name) {
                let (srt, state) = partial_paths(&dir, &hash, asked.as_deref(), task);
                if let Some(p) = read_partial(&srt, &state) {
                    partial.push((asked, task, p));
                }
            }
        }
    }
    found.sort();
    partial.sort_by(|a, b| (&a.0, a.1 == Task::Translate).cmp(&(&b.0, b.1 == Task::Translate)));
    (found, partial)
}

/// The Speech feature's installed files.
fn installed() -> Option<SpeechFiles> {
    let data = specular_settings::Locations::detect().data_dir;
    let store = specular_ai::install::Store::new(&data);
    SpeechFiles::find(
        &specular_ai::Manifest::load(),
        store.root(),
        &store.installed(),
    )
}

impl App {
    /// A new movie: its generated subtitles in the cache are looked for off the UI thread.
    pub(super) fn stt_enter(&mut self, path: &Path) {
        let (tx, rx) = mpsc::channel();
        let path = path.to_path_buf();
        let waker = self.loader.waker();
        self.player.stt.lookup = Some(rx);
        self.loader.background("stt-lookup", move |_| {
            let (found, partial) = look_up(&path);
            let _ = tx.send((path, found, partial));
            waker();
        });
    }

    /// What generating reads now: the movie on screen, else the sound file on the sound
    /// page; with its id, path and source.
    fn stt_target(&self) -> Option<(Target, ImageId, PathBuf, Arc<Source>)> {
        let (target, id) = match self.player_movie() {
            Some((id, _)) => (Target::Movie, id),
            None => {
                let id = self.current_id().filter(|_| self.sound_page_active())?;
                self.sound_page
                    .shows(id)
                    .then_some((Target::Sound(id), id))?
            }
        };
        let source = Arc::clone(self.images.get(&id)?.session.as_ref()?.source());
        let path = source.path().to_path_buf();
        Some((target, id, path, source))
    }

    /// The playhead (milliseconds of media time) for a job on `target`.
    fn stt_playhead(&self, target: Target) -> u64 {
        let us = match target {
            Target::Movie => self.player_media_us(self.player_time()),
            Target::Sound(id) => self
                .anim
                .as_ref()
                .filter(|a| a.id == id)
                .and_then(|a| a.time(std::time::Instant::now()))
                .map_or(0, |d| d.as_micros() as i64),
        };
        (us.max(0) / 1000) as u64
    }

    /// The sound file on the sound page: its transcript looked up once, shown in the lyrics
    /// column when the file has none of its own (or the user asked for it); a job left
    /// unfinished continues.
    fn stt_sound_tick(&mut self) {
        let cur = match self.stt_target() {
            Some((Target::Sound(id), ..)) => Some(id),
            _ => None,
        };
        // A job for a sound file ends when the file is left (kept to continue later).
        if let Some(j) = &self.player.stt.job
            && matches!(j.target, Target::Sound(id) if Some(id) != cur)
        {
            self.player.stt.job = None;
        }
        if cur != self.player.stt.sound_seen {
            self.player.stt.sound_seen = cur;
            self.player.stt.sound_text = None;
            self.player.stt.sound_forced = false;
            self.player.stt.sound_lookup = None;
            if let Some((_, id, path, _)) = cur.and_then(|_| self.stt_target()) {
                let (tx, rx) = mpsc::channel();
                let waker = self.loader.waker();
                self.player.stt.sound_lookup = Some(rx);
                self.loader.background("stt-lookup", move |_| {
                    let (found, partial) = look_up(&path);
                    // the transcript (not a translation) first
                    let text = found.iter().min_by_key(|f| f.2).and_then(|(p, lang, tr)| {
                        let t =
                            specular_subs::srt::parse(&std::fs::read_to_string(p).ok()?).ok()?;
                        Some((from_track(&t), lang.clone(), *tr))
                    });
                    let partial = partial.into_iter().next();
                    let _ = tx.send(SoundFound { id, text, partial });
                    waker();
                });
            }
        }
        if let Some(rx) = &self.player.stt.sound_lookup
            && let Ok(f) = rx.try_recv()
        {
            self.player.stt.sound_lookup = None;
            if Some(f.id) == cur {
                if let Some((cues, lang, tr)) = f.text {
                    self.player.stt.sound_text = Some((f.id, Arc::new(cues), Some(lang), tr));
                } else if let Some((asked, task, p)) = f.partial {
                    let tr = task == Task::Translate;
                    let lang = p.language.clone();
                    self.player.stt.sound_text = Some((f.id, Arc::new(p.cues), lang, tr));
                    if !p.stopped && self.player.stt.job.is_none() && installed().is_some() {
                        self.stt_start(asked, task);
                        let now = self.now();
                        self.chrome.toast(t!("stt-resumed"), None, now);
                    }
                }
            }
        }
        // The transcript in the lyrics column.
        if let Some((id, cues, ..)) = &self.player.stt.sound_text
            && self.sound_page.shows(*id)
            && !cues.is_empty()
        {
            let own = self
                .sound_page
                .lyrics
                .as_ref()
                .is_some_and(|l| l.from != LyricsFrom::Generated);
            let have =
                self.sound_page.lyrics.as_ref().is_some_and(|l| {
                    l.from == LyricsFrom::Generated && l.lines.len() == cues.len()
                });
            if (!own || self.player.stt.sound_forced) && !have {
                self.sound_page.lyrics = Some(Arc::new(to_lyrics(cues)));
                self.redraw = true;
            }
        }
    }

    /// Each tick: cache files found, the job's new cues and its end, a save's outcome.
    pub(super) fn stt_poll(&mut self) {
        self.stt_sound_tick();
        if let Some(rx) = &self.player.stt.lookup
            && let Ok((path, found, partial)) = rx.try_recv()
        {
            self.player.stt.lookup = None;
            if self.player.movie.as_ref().is_some_and(|m| m.1 == path) {
                for (p, lang, tr) in found {
                    let src = source_for(p, Some(&lang), tr);
                    if !self.player.sources.iter().any(|s| s.origin == src.origin) {
                        self.player.sources.push(src);
                    }
                }
                // Unfinished: an interrupted job continues; one the user stopped shows
                // what it made.
                let dir = cache_dir();
                let hash = fingerprint(&path).ok();
                for (asked, task, p) in partial {
                    let tr = task == Task::Translate;
                    if !p.stopped && self.player.stt.job.is_none() && installed().is_some() {
                        self.stt_start(asked, task);
                        let now = self.now();
                        self.chrome.toast(t!("stt-resumed"), None, now);
                    } else if let Some(h) = &hash {
                        let (srt, _) = partial_paths(&dir, h, asked.as_deref(), task);
                        let mut src = source_for(srt, p.language.as_deref(), tr);
                        src.name = Some(match p.language.as_deref() {
                            Some(l) => t!("stt-track-partial", language = subs::language_name(l)),
                            None => t!("stt-track-pending"),
                        });
                        if !self.player.sources.iter().any(|s| s.origin == src.origin) {
                            self.player.sources.push(src);
                        }
                    }
                }
                // Chosen last time: chosen again.
                let remembered = self
                    .player
                    .key
                    .clone()
                    .and_then(|k| self.player.store().get(&k))
                    .and_then(|e| e.subtitle);
                if self.player.sub.is_none()
                    && let Some(k) = remembered
                    && let Some(i) = self
                        .player
                        .sources
                        .iter()
                        .position(|s| s.key(self.player.extras.as_deref()) == k)
                {
                    self.player_choose_subtitle(Some(i), false);
                }
                self.redraw = true;
            }
        }
        if let Some(rx) = &self.player.stt.saved
            && let Ok(r) = rx.try_recv()
        {
            self.player.stt.saved = None;
            let now = self.now();
            let msg = match r {
                Ok(p) => t!(
                    "stt-saved",
                    name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                Err(e) => t!("stt-save-failed", reason = e),
            };
            self.chrome.toast(msg, None, now);
        }
        // The playhead, for the job to follow.
        if let Some(target) = self.player.stt.job.as_ref().map(|j| j.target) {
            let ms = self.stt_playhead(target);
            if let Some(j) = &self.player.stt.job {
                j.shared.playhead.store(ms, Ordering::Relaxed);
            }
        }
        let Some(job) = self.player.stt.job.as_mut() else {
            return;
        };
        let (version, cues, lang, finished, error) = {
            let p = job.shared.progress.lock();
            (
                p.version,
                (p.version != job.seen).then(|| p.cues.clone()),
                p.language.clone(),
                p.finished.clone(),
                p.error.clone(),
            )
        };
        let path = job.path.clone();
        let task = job.task;
        let target = job.target;
        job.seen = version;
        if let Target::Sound(id) = target {
            // A sound file's transcript fills the lyrics column.
            if let Some(cues) = cues {
                self.player.stt.sound_text =
                    Some((id, Arc::new(cues), lang, task == Task::Translate));
            }
            let now = self.now();
            if let Some(e) = error {
                self.player.stt.job = None;
                self.chrome.toast(t!("stt-failed", reason = e), None, now);
            } else if finished.is_some() {
                self.player.stt.job = None;
                self.chrome.toast(t!("stt-done"), None, now);
            }
            self.redraw = true;
            return;
        }
        let at = self
            .player
            .sources
            .iter()
            .position(|s| s.origin == subs::Origin::File(path.clone()));
        if let Some(cues) = cues {
            if let Some(i) = at {
                self.player.sources[i].name =
                    Some(track_name(lang.as_deref(), task == Task::Translate));
                if self.player.sub == Some(i) {
                    self.player.track = Some(Arc::new(to_track(&cues)));
                }
            }
            self.redraw = true;
        }
        let now = self.now();
        if let Some(e) = error {
            self.player.stt.job = None;
            if let Some(i) = at {
                self.stt_remove_source(i);
            }
            self.chrome.toast(t!("stt-failed", reason = e), None, now);
            self.redraw = true;
        } else if let Some(done) = finished {
            self.player.stt.job = None;
            if let Some(i) = at {
                self.player.sources[i].origin = subs::Origin::File(done);
                if self.player.sub == Some(i) {
                    self.player_save();
                }
            }
            self.chrome.toast(t!("stt-done"), None, now);
            self.redraw = true;
        }
    }

    fn stt_remove_source(&mut self, i: usize) {
        self.player.sources.remove(i);
        match self.player.sub {
            Some(s) if s == i => {
                self.player.sub = None;
                self.player.track = None;
            }
            Some(s) if s > i => self.player.sub = Some(s - 1),
            _ => {}
        }
    }

    /// The cues of a track being generated, for `src` (else `None`: decode it as a file).
    pub(super) fn stt_track(&self, src: &subs::Source) -> Option<Arc<Track>> {
        let job = self.player.stt.job.as_ref()?;
        if src.origin != subs::Origin::File(job.path.clone()) {
            return None;
        }
        Some(Arc::new(to_track(&job.shared.progress.lock().cues)))
    }

    /// The CC menu's generate row: the progress while a job runs.
    pub(super) fn stt_row(&self) -> String {
        match &self.player.stt.job {
            Some(j) => {
                let p = j.shared.progress.lock();
                let r = if p.total > 0.0 { p.done / p.total } else { 0.0 };
                let loc = specular_i18n::I18n::global().localizer();
                t!(
                    "player-subtitles-generating",
                    percent = loc.percent(r.clamp(0.0, 1.0), 0)
                )
            }
            None => t!("player-subtitles-generate"),
        }
    }

    /// The transcript of the sound file on the sound page, while it shows.
    fn stt_sound_shown(&self) -> Option<&SoundText> {
        let st = self.player.stt.sound_text.as_ref()?;
        let shows = self.sound_page.shows(st.0)
            && self
                .sound_page
                .lyrics
                .as_ref()
                .is_some_and(|l| l.from == LyricsFrom::Generated);
        (shows && !st.1.is_empty()).then_some(st)
    }

    /// Whether the chosen subtitles were generated (they can be saved).
    pub(super) fn stt_chosen(&self) -> bool {
        if self.stt_sound_shown().is_some() {
            return true;
        }
        let dir = cache_dir();
        self.player
            .sub
            .and_then(|i| self.player.sources.get(i))
            .is_some_and(|s| matches!(&s.origin, subs::Origin::File(p) if p.starts_with(&dir)))
            && self
                .player
                .track
                .as_ref()
                .is_some_and(|t| !t.cues.is_empty())
    }

    /// "Generate subtitles…": the language sheet (or stops the job under way).
    pub(super) fn stt_offer(&mut self) {
        let now = self.now();
        if self.stt_target().is_none() {
            self.chrome.toast(t!("toast-subtitles-none"), None, now);
            return;
        }
        if let Some(j) = self.player.stt.job.take() {
            // kept, and not continued on its own next time
            j.shared.stopped.store(true, Ordering::Relaxed);
            drop(j);
            self.chrome.toast(t!("stt-stopped"), None, now);
            self.redraw = true;
            return;
        }
        let Some(files) = installed() else {
            self.chrome.open_list(
                t!("stt-choose-language"),
                vec![PaletteItem {
                    id: ITEM_SETTINGS,
                    label: t!("stt-settings"),
                    key: None,
                    unavailable: None,
                    icon: Icon::Settings,
                }],
                None,
            );
            self.chrome.toast(t!("stt-needs-model"), None, now);
            return;
        };
        let multilingual = files.multilingual();
        let mut items = vec![PaletteItem {
            id: ITEM_DETECT,
            label: t!("stt-detect"),
            key: None,
            unavailable: None,
            icon: Icon::Subtitles,
        }];
        if multilingual {
            // large-v3 turbo can't translate: the row says why
            items.push(PaletteItem {
                id: ITEM_TRANSLATE,
                label: t!("stt-translate"),
                key: None,
                unavailable: (!files.translates()).then(|| t!("stt-translate-turbo")),
                icon: Icon::Subtitles,
            });
            let mut langs: Vec<(String, usize)> = specular_infer::whisper::tokens::LANGUAGES[..99]
                .iter()
                .enumerate()
                .map(|(i, c)| (subs::language_name(c), i))
                .collect();
            langs.sort();
            for (name, i) in langs {
                items.push(PaletteItem {
                    id: ITEM_LANG0 + i as u64,
                    label: name,
                    key: None,
                    unavailable: None,
                    icon: Icon::Subtitles,
                });
            }
        }
        self.chrome
            .open_list(t!("stt-choose-language"), items, None);
    }

    /// A row of the language sheet.
    pub(super) fn stt_item(&mut self, id: u64) -> bool {
        if !(ITEM_BASE..=ITEM_SETTINGS).contains(&id) {
            return false;
        }
        self.chrome.close_modal();
        match id {
            ITEM_SETTINGS => self.chrome.open_settings_at("ai.speech"),
            ITEM_DETECT => self.stt_start(None, Task::Transcribe),
            ITEM_TRANSLATE => self.stt_start(None, Task::Translate),
            _ => {
                let i = (id - ITEM_LANG0) as usize;
                if let Some(l) = specular_infer::whisper::tokens::LANGUAGES.get(i) {
                    self.stt_start(Some((*l).to_owned()), Task::Transcribe);
                }
            }
        }
        true
    }

    /// Starts reading the movie's speech; its track appears at once and fills.
    fn stt_start(&mut self, language: Option<String>, task: Task) {
        let now = self.now();
        let Some(files) = installed() else {
            self.chrome.toast(t!("stt-needs-model"), None, now);
            return;
        };
        let Some((target, _, _, source)) = self.stt_target() else {
            return;
        };
        self.player.stt.jobs += 1;
        let path = cache_dir().join(format!(
            "generating-{}-{}.srt",
            std::process::id(),
            self.player.stt.jobs
        ));
        let shared = Arc::new(Shared::new(self.stt_playhead(target)));
        let gpu = self
            .gfx
            .as_ref()
            .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone()));
        let sound = match target {
            Target::Movie => self.player.sound,
            Target::Sound(_) => None,
        };
        let waker = self.loader.waker();
        let sh = Arc::clone(&shared);
        let lang = language.clone();
        let spawned = specular_thread::builder("stt").spawn(move || {
            let r = run(&sh, source, sound, lang, task, gpu, files, &*waker);
            let mut p = sh.progress.lock();
            match r {
                Ok(Some(path)) => p.finished = Some(path),
                Ok(None) => {}
                Err(e) => p.error = Some(e),
            }
            drop(p);
            waker();
        });
        if spawned.is_err() {
            return;
        }
        self.player.stt.job = Some(Job {
            path: path.clone(),
            task,
            shared,
            seen: 0,
            target,
        });
        if let Target::Sound(_) = target {
            // asked for: the transcript shows over the file's own lyrics
            self.player.stt.sound_forced = true;
            self.chrome.toast(t!("stt-started"), None, now);
            self.redraw = true;
            return;
        }
        // An unfinished job's cues shown before: the job takes them over.
        if let Ok(hash) = fingerprint(source_path_of(&self.player.movie)) {
            let (srt, _) = partial_paths(&cache_dir(), &hash, language.as_deref(), task);
            if let Some(i) = self
                .player
                .sources
                .iter()
                .position(|s| s.origin == subs::Origin::File(srt.clone()))
            {
                self.stt_remove_source(i);
            }
        }
        self.player.sources.push(source_for(
            path,
            language.as_deref(),
            task == Task::Translate,
        ));
        let i = self.player.sources.len() - 1;
        self.player_choose_subtitle(Some(i), false);
        self.chrome.toast(t!("stt-started"), None, now);
        self.redraw = true;
    }

    /// "Save subtitles…": the generated subtitles beside the movie, through the save dialog.
    pub(super) fn stt_save(&mut self) {
        let now = self.now();
        if let Some((id, cues, lang, _)) = self.stt_sound_shown().cloned() {
            self.stt_save_sound(id, &cues, lang);
            return;
        }
        if !self.stt_chosen() {
            self.chrome.toast(t!("toast-subtitles-none"), None, now);
            return;
        }
        let Some(track) = self.player.track.clone() else {
            return;
        };
        let Some((_, movie)) = self.player.movie.clone() else {
            return;
        };
        let lang = self
            .player
            .sub
            .and_then(|i| self.player.sources.get(i))
            .and_then(|s| match &s.origin {
                subs::Origin::File(p) => p
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.split('.').nth(1))
                    .filter(|l| !l.starts_with("generating"))
                    .map(str::to_owned),
                subs::Origin::Embedded(_) => None,
            });
        let stem = movie
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = match lang {
            Some(l) => format!("{stem}.{l}.srt"),
            None => format!("{stem}.srt"),
        };
        let (tx, rx) = mpsc::channel();
        self.player.stt.saved = Some(rx);
        let title = t!("stt-save-title");
        let (srt, vtt) = (t!("stt-file-srt"), t!("stt-file-vtt"));
        self.loader.background("dlg-stt-save", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .set_file_name(&name)
                .add_filter(&srt, &["srt"])
                .add_filter(&vtt, &["vtt"]);
            if let Some(dir) = movie.parent() {
                d = d.set_directory(dir);
            }
            let Some(p) = d.save_file() else { return };
            let vtt = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vtt"));
            let text = if vtt {
                specular_subs::write::vtt(&track)
            } else {
                specular_subs::write::srt(&track)
            };
            let _ = tx.send(
                std::fs::write(&p, text)
                    .map(|_| p)
                    .map_err(|e| e.to_string()),
            );
            l.waker()();
        });
    }
}

impl App {
    /// "Save subtitles…" on the sound page: the transcript as LRC (beside the song it is its
    /// lyrics), SRT or WebVTT, through the save dialog.
    fn stt_save_sound(&mut self, id: ImageId, cues: &[cues::Cue], lang: Option<String>) {
        let Some(path) = self
            .images
            .get(&id)
            .and_then(|i| i.session.as_ref())
            .map(|s| s.source().path().to_path_buf())
        else {
            return;
        };
        let _ = lang;
        let track = to_track(cues);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = format!("{stem}.lrc");
        let (tx, rx) = mpsc::channel();
        self.player.stt.saved = Some(rx);
        let title = t!("stt-save-title");
        let (lrc, srt, vtt) = (t!("stt-file-lrc"), t!("stt-file-srt"), t!("stt-file-vtt"));
        self.loader.background("dlg-stt-save", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .set_file_name(&name)
                .add_filter(&lrc, &["lrc"])
                .add_filter(&srt, &["srt"])
                .add_filter(&vtt, &["vtt"]);
            if let Some(dir) = path.parent() {
                d = d.set_directory(dir);
            }
            let Some(p) = d.save_file() else { return };
            let _ = tx.send(
                std::fs::write(&p, transcript_text(&p, &track))
                    .map(|_| p)
                    .map_err(|e| e.to_string()),
            );
            l.waker()();
        });
    }
}

/// A transcript in the format its file name asks for: LRC, WebVTT, else SRT.
fn transcript_text(p: &Path, track: &Track) -> String {
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "lrc" => specular_subs::write::lrc(track),
        "vtt" => specular_subs::write::vtt(track),
        _ => specular_subs::write::srt(track),
    }
}

/// The path of the movie, or an empty one.
fn source_path_of(movie: &Option<(ImageId, PathBuf)>) -> &Path {
    movie.as_ref().map_or(Path::new(""), |m| m.1.as_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A web file still being fetched: its fingerprint never reads the parts not there
    /// (`WouldBlock` at once on the UI thread); off it, the parts are asked for and waited
    /// for, and the result is the whole file's.
    #[test]
    fn fingerprints_wait_for_a_filling_file() {
        use specular_core::fill::{self, Filling};
        let dir = std::env::temp_dir().join(format!("specular-stt-fill-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let whole = dir.join("whole.mkv");
        let len = 5u64 << 20;
        let bytes: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
        std::fs::write(&whole, &bytes).unwrap();
        let want = fingerprint(&whole).unwrap();
        // The same file, only its first 2 MiB there; the rest zeros on disk.
        let part = dir.join("part.mkv");
        let mut sparse = vec![0u8; len as usize];
        sparse[..2 << 20].copy_from_slice(&bytes[..2 << 20]);
        std::fs::write(&part, &sparse).unwrap();
        let f = Filling::new(len, Duration::from_secs(20));
        f.add(0, 2 << 20);
        let reg = fill::register(&part, &f);
        let e = fingerprint(&part).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::WouldBlock);
        // A fetcher answering what is asked for.
        let (g, path, src) = (Arc::clone(&f), part.clone(), bytes.clone());
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let fetcher = specular_thread::spawn("test-fetch", move || {
            use std::io::Write;
            while !stopped.load(Ordering::Relaxed) {
                g.wait_change(Duration::from_millis(5));
                if let Some((a, b)) = g.wanted() {
                    let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
                    file.seek(SeekFrom::Start(a)).unwrap();
                    file.write_all(&src[a as usize..b as usize]).unwrap();
                    drop(file);
                    g.add(a, b);
                }
            }
        });
        assert_eq!(fingerprint_waiting(&part).unwrap(), want);
        assert!(!f.stalled(), "a fingerprint is never a stall");
        stop.store(true, Ordering::Relaxed);
        fetcher.join().unwrap();
        drop(reg);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_names_round_trip() {
        let h = "0123456789abcdef";
        assert_eq!(
            cache_name(h, "he", Task::Transcribe),
            "0123456789abcdef.he.srt"
        );
        assert_eq!(
            parse_cache_name(h, &cache_name(h, "fr", Task::Translate)),
            Some(("fr".into(), true))
        );
        assert_eq!(
            parse_cache_name(h, "0123456789abcdef.en.srt"),
            Some(("en".into(), false))
        );
        assert_eq!(parse_cache_name(h, "0123456789abcdef.srt"), None);
        assert_eq!(parse_cache_name(h, "ffff.en.srt"), None);
        assert_eq!(parse_cache_name(h, "0123456789abcdef.en.part"), None);
    }

    #[test]
    fn the_job_starts_at_the_playhead_then_fills_the_rest() {
        let mut c = Vec::new();
        // Nothing read: from the playhead.
        assert_eq!(next_gap(&c, 300.0, 1000.0), Some((300.0, 700.0)));
        cover(&mut c, 300.0, 420.0);
        assert_eq!(next_gap(&c, 310.0, 1000.0), Some((420.0, 580.0)));
        cover(&mut c, 420.0, 1000.0);
        // To the end: then from the start, up to what was read.
        assert_eq!(next_gap(&c, 900.0, 1000.0), Some((0.0, 300.0)));
        // A seek back into the start.
        assert_eq!(next_gap(&c, 100.0, 1000.0), Some((100.0, 200.0)));
        cover(&mut c, 0.0, 300.0);
        assert_eq!(c, vec![(0.0, 1000.0)]);
        assert_eq!(next_gap(&c, 0.0, 1000.0), None);
    }

    #[test]
    fn fingerprints_follow_the_content_not_the_name() {
        let dir = std::env::temp_dir().join(format!("specular-stt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b, c) = (dir.join("a.mkv"), dir.join("b.mkv"), dir.join("c.mkv"));
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&a, &data).unwrap();
        std::fs::write(&b, &data).unwrap();
        let mut d2 = data.clone();
        d2[1_500_000] ^= 1;
        std::fs::write(&c, &d2).unwrap();
        assert_eq!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
        assert_ne!(fingerprint(&a).unwrap(), fingerprint(&c).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole job on a recording, with a model placed by hand
    /// (`SPECULAR_WHISPER_MODEL`: `model.safetensors` and `vocab.json`;
    /// `SPECULAR_WHISPER_AUDIO`: a WAV of speech); skipped otherwise.
    #[test]
    fn the_job_reads_a_placed_recording() {
        let (Some(model), Some(audio)) = (
            std::env::var_os("SPECULAR_WHISPER_MODEL").map(PathBuf::from),
            std::env::var_os("SPECULAR_WHISPER_AUDIO").map(PathBuf::from),
        ) else {
            eprintln!("SPECULAR_WHISPER_MODEL / SPECULAR_WHISPER_AUDIO not set: skipped");
            return;
        };
        let files = SpeechFiles {
            model: model.join("model.safetensors"),
            vocab: model.join("vocab.json"),
            id: "whisper-test".into(),
        };
        let shared = Shared::new(5_000);
        let source = Arc::new(Source::open(&audio).unwrap());
        let t0 = std::time::Instant::now();
        let out = run(
            &shared,
            source,
            None,
            None,
            Task::Transcribe,
            None,
            files,
            &|| {},
        )
        .unwrap()
        .unwrap();
        let p = shared.progress.lock();
        eprintln!(
            "{:.1} s of sound in {:.1} s, {} cues, {:?}",
            p.total,
            t0.elapsed().as_secs_f64(),
            p.cues.len(),
            p.language
        );
        assert!(!p.cues.is_empty());
        assert!((p.done - p.total).abs() < 1.0, "{} of {}", p.done, p.total);
        let text = std::fs::read_to_string(&out).unwrap();
        let back = specular_subs::srt::parse(&text).unwrap();
        assert_eq!(back.cues.len(), p.cues.len());
        let name = out.file_name().unwrap().to_string_lossy().into_owned();
        let hash = fingerprint(&audio).unwrap();
        assert!(parse_cache_name(&hash, &name).is_some(), "{name}");
        let _ = std::fs::remove_dir_all(cache_dir());
    }

    /// An interrupted job keeps what it made and the next run of the same choice takes it
    /// up: it starts with those cues and stretches, reads only the rest, and ends with
    /// cues in order and none twice (same model and recording as above; skipped otherwise).
    #[test]
    fn an_interrupted_job_continues() {
        let (Some(model), Some(audio)) = (
            std::env::var_os("SPECULAR_WHISPER_MODEL").map(PathBuf::from),
            std::env::var_os("SPECULAR_WHISPER_AUDIO").map(PathBuf::from),
        ) else {
            eprintln!("SPECULAR_WHISPER_MODEL / SPECULAR_WHISPER_AUDIO not set: skipped");
            return;
        };
        let files = || SpeechFiles {
            model: model.join("model.safetensors"),
            vocab: model.join("vocab.json"),
            id: "whisper-test".into(),
        };
        let source = Arc::new(Source::open(&audio).unwrap());
        let _ = std::fs::remove_dir_all(cache_dir());
        // The first run stops once it has cues.
        let first = Shared::new(0);
        let t0 = std::time::Instant::now();
        let stop = || {
            if !first.progress.lock().cues.is_empty() {
                first.cancel.store(true, Ordering::Relaxed);
            }
        };
        let out = run(
            &first,
            Arc::clone(&source),
            None,
            Some("en".into()),
            Task::Transcribe,
            None,
            files(),
            &stop,
        )
        .unwrap();
        let t_first = t0.elapsed().as_secs_f64();
        assert!(out.is_none());
        let (made, done) = {
            let p = first.progress.lock();
            (p.cues.clone(), p.done)
        };
        assert!(!made.is_empty() && done > 0.0);
        // only what was read counts as done (one window, at most 30 s)
        assert!(done <= 30.5, "{done}");
        let hash = fingerprint(&audio).unwrap();
        let (srt, state) = partial_paths(&cache_dir(), &hash, Some("en"), Task::Transcribe);
        let kept = read_partial(&srt, &state).unwrap();
        assert_eq!(kept.cues.len(), made.len());
        assert!(!kept.stopped);
        // The second run starts from there.
        let second = Shared::new(0);
        let seen_at_start = std::sync::Mutex::new(None);
        let watch = || {
            let p = second.progress.lock();
            let mut s = seen_at_start.lock().unwrap();
            if s.is_none() {
                *s = Some((p.cues.len(), p.done));
            }
        };
        let t1 = std::time::Instant::now();
        let out = run(
            &second,
            Arc::clone(&source),
            None,
            Some("en".into()),
            Task::Transcribe,
            None,
            files(),
            &watch,
        )
        .unwrap()
        .unwrap();
        let t_second = t1.elapsed().as_secs_f64();
        let (n0, d0) = seen_at_start.lock().unwrap().unwrap();
        assert_eq!(
            n0,
            made.len(),
            "the second run starts with the first's cues"
        );
        assert!((d0 - done).abs() < 1e-6, "{d0} vs {done}");
        let p = second.progress.lock();
        assert!(p.cues.len() > made.len());
        for (a, b) in made.iter().zip(&p.cues) {
            assert_eq!(a.text, b.text);
        }
        for w in p.cues.windows(2) {
            assert!(w[0].start <= w[1].start, "{:?}", w);
            assert!(
                w[0].text != w[1].text || w[0].start != w[1].start,
                "{:?}",
                w
            );
        }
        eprintln!(
            "first run {t_first:.1} s ({} cues, {done:.1} s read), second {t_second:.1} s \
             ({} cues, {:.1} s in all)",
            made.len(),
            p.cues.len(),
            p.total
        );
        // finished: the partial files are gone, the cache file stays
        assert!(!srt.exists() && !state.exists());
        assert!(out.exists());
        let _ = std::fs::remove_dir_all(cache_dir());
    }

    #[test]
    fn partial_files_round_trip() {
        let h = "0123456789abcdef";
        let dir = std::env::temp_dir().join(format!("specular-stt-part-{}", std::process::id()));
        for (asked, task) in [
            (None, Task::Transcribe),
            (Some("fr"), Task::Translate),
            (Some("he"), Task::Transcribe),
        ] {
            let (srt, state) = partial_paths(&dir, h, asked, task);
            let name = state.file_name().unwrap().to_string_lossy().into_owned();
            assert_eq!(
                parse_partial_name(h, &name),
                Some((asked.map(str::to_owned), task))
            );
            // never taken for a finished file
            let srt_name = srt.file_name().unwrap().to_string_lossy().into_owned();
            assert_eq!(parse_cache_name(h, &srt_name), None, "{srt_name}");
            let p = Partial {
                language: Some("fr".into()),
                covered: vec![(0.0, 31.5), (300.0, 420.25)],
                cues: vec![
                    cues::Cue {
                        start: 1.5,
                        end: 3.0,
                        text: "Bonjour\nà tous".into(),
                    },
                    cues::Cue {
                        start: 301.0,
                        end: 302.5,
                        text: "Encore".into(),
                    },
                ],
                stopped: task == Task::Translate,
            };
            write_partial(&dir, h, asked, task, &p);
            assert_eq!(read_partial(&srt, &state), Some(p));
        }
        assert_eq!(
            parse_partial_name(h, "0123456789abcdef.a.b.partial.state"),
            None
        );
        assert_eq!(parse_partial_name(h, "ffff.auto.partial.state"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A transcript as lyrics, saved as LRC, reads back as the same synced lines.
    #[test]
    fn transcripts_become_lyrics_and_lrc() {
        let c = [
            cues::Cue {
                start: 1.5,
                end: 3.0,
                text: "Hello\nthere".into(),
            },
            cues::Cue {
                start: 4.25,
                end: 6.0,
                text: "Again".into(),
            },
        ];
        let l = to_lyrics(&c);
        assert!(l.synced);
        assert_eq!(l.from, LyricsFrom::Generated);
        assert_eq!(l.lines[0].text, "Hello there");
        assert_eq!(l.lines[1].at, Some(4.25));
        let text = transcript_text(Path::new("song.LRC"), &to_track(&c));
        let back = specular_video::lyrics::parse_lrc(&text, LyricsFrom::Sidecar).unwrap();
        let lines: Vec<_> = back
            .lines
            .iter()
            .filter(|l| !l.text.is_empty())
            .map(|l| (l.at, l.text.as_str()))
            .collect();
        assert_eq!(lines, [(Some(1.5), "Hello there"), (Some(4.25), "Again")]);
        assert!(transcript_text(Path::new("a.vtt"), &to_track(&c)).starts_with("WEBVTT"));
        assert!(transcript_text(Path::new("a.srt"), &to_track(&c)).starts_with("1\n"));
    }

    #[test]
    fn whisper_cues_become_a_track() {
        let t = to_track(&[cues::Cue {
            start: 1.5,
            end: 3.0,
            text: "Hello\nthere".into(),
        }]);
        assert_eq!(t.cues[0].start_us, 1_500_000);
        assert_eq!(t.cues[0].text().unwrap().plain_text(), "Hello\nthere");
    }
}

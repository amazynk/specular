//! Open reports: why an image took as long as it did to open (DESIGN.md §12.7, the first step
//! of the hardware advisor).
//!
//! The loader times each stage of an open (reading the file, the header, the colour transform,
//! decoding the level that makes the view sharp, with the path the selector took and why) and
//! the app adds the GPU upload and when the first and the sharp pixels were on screen. That is
//! a [`Timeline`]: a few numbers per open, no allocation per row. [`verdict`] turns it into a
//! short finding with a hint when there is an obvious fix; it is a pure function, so the rules
//! are unit-tested below. The verdict is shown in the information panel ("Performance"),
//! logged at debug level, and printed with `--bench-open` or `SPECULAR_OPEN_REPORT=1`.

use specular_core::paths::{
    Engine, Failure, Fallback, Mode, Note, Override, PathSelector, Reason, ScaleFallback, Scaled,
    Source, Task, Trust, selector,
};
use specular_i18n::{I18n, t};
use specular_widgets::OpenReport;
use std::time::Instant;

pub use crate::loader::OpenTimes;

pub mod advisor;
pub mod insights;
pub mod store;
pub mod thumbs;

use advisor::{Advice, Change, Pattern};

/// Below this the open counts as fast whatever the split (about what feels instant).
pub const SLOW_MS: f64 = 250.0;
/// Reading slower than this (MB/s) while reading dominates points at the storage.
const SLOW_STORAGE_MB_S: f64 = 150.0;

/// What a hardware decoder probe found (`specular_hwdec::probed`), for the reasons a decode
/// stayed in software.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HwState {
    /// Not probed yet (it runs in the background once the window is up).
    NotProbed,
    /// No device or no driver.
    None { api: &'static str },
    /// A device, but without a decoder for this codec.
    NoCodec { api: &'static str },
    /// A device with a decoder for this codec.
    Present { api: &'static str },
}

/// The path selector's decision for one decode, and what stood around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathFacts {
    pub task: Task,
    pub engine: Engine,
    pub reason: Reason,
    /// The chosen non-reference path was not used after all.
    pub fallback: Option<Fallback>,
    /// The chosen path ran beside the CPU reference to compare (the work was done twice).
    pub checked: bool,
    /// The task's other path (path 1).
    pub alt_engine: Option<Engine>,
    pub alt_trust: Trust,
    pub alt_blocked: Option<Failure>,
    /// Predicted wall times (ms) of the CPU reference and the other path for this request.
    pub cpu_est_ms: Option<f64>,
    pub alt_est_ms: Option<f64>,
    /// The estimates come from measurements on this machine (not the built-in guess).
    pub measured: bool,
    pub mode: Mode,
    /// The per-task setting forces the CPU reference.
    pub forced_off: bool,
    /// What the hardware decoder probe had found when the decode started.
    pub hw: HwState,
    /// A reduced-size level: the scale asked for, the one decoded and any fallback.
    pub scaled: Option<Scaled>,
}

/// The decode of the level that made the view sharp.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct DecodeStage {
    /// 1/scale of the full image.
    pub scale: u32,
    /// Wall time of the decode call, including [`DecodeStage::io_ms`].
    pub ms: f64,
    /// Of `ms`, the estimated time spent waiting for the file's pages (memory-mapped files are
    /// read while decoding): the decoding thread's wall minus CPU time, when it had major page
    /// faults.
    pub io_ms: f64,
    pub faults: u64,
    pub from_cache: bool,
    pub path: Option<PathFacts>,
}

/// One open, from the moment it was asked for (navigation, or the command line).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Timeline {
    pub format: &'static str,
    pub bytes: u64,
    /// Full-size pixels.
    pub pixels: u64,
    pub in_archive: bool,
    /// Opening (or mapping) the file.
    pub read_ms: f64,
    /// The container and codec headers.
    pub parse_ms: f64,
    /// Building the ICC colour transform.
    pub colour_ms: f64,
    pub decode: Option<DecodeStage>,
    /// GPU uploads of this image while it was being opened.
    pub upload_ms: f64,
    /// Where YCbCr was converted to RGB (the upload's path), if it was.
    pub ycbcr: Option<Engine>,
    /// Waiting for the window and the GPU (the first image of a session).
    pub startup_ms: f64,
    /// Since asked for.
    pub first_pixels_ms: Option<f64>,
    /// Since asked for: a whole level of the image (any size) was on screen.
    pub whole_ms: Option<f64>,
    pub sharp_ms: f64,
    /// The sharp level was ready before the image was asked for (prefetched).
    pub prefetched: bool,
    pub memory_short: bool,
}

/// What the app gathers for one image's report as the loader's events arrive.
#[derive(Clone, Copy, Debug, Default)]
pub struct Track {
    pub opened: Option<OpenTimes>,
    /// The decode of the best level so far, and when it arrived.
    pub decode: Option<(DecodeStage, Instant)>,
    /// Uploads since the image was last shown.
    pub upload_ms: f64,
    pub ycbcr: Option<Engine>,
    pub memory_short: bool,
    /// The report of the current showing, once it was sharp.
    pub done: Option<(Timeline, Verdict)>,
}

impl Track {
    /// The image is shown (again): a new report follows.
    pub fn shown(&mut self) {
        self.upload_ms = 0.0;
        self.done = None;
    }

    /// The timeline of the showing that started at `shown_at` and is sharp now.
    pub fn timeline(
        &self,
        info: &specular_decode::ImageInfo,
        shown_at: Instant,
        first_pixels_ms: Option<f64>,
        whole_ms: Option<f64>,
        ready_at: Option<Instant>,
    ) -> Timeline {
        let o = self.opened.unwrap_or_default();
        let since = |t: Instant| t.saturating_duration_since(shown_at).as_secs_f64() * 1e3;
        Timeline {
            format: info.format.name(),
            bytes: o.bytes,
            pixels: u64::from(info.width) * u64::from(info.height),
            in_archive: o.in_archive,
            read_ms: o.read_ms,
            parse_ms: o.parse_ms,
            colour_ms: o.colour_ms,
            decode: self.decode.map(|d| d.0),
            upload_ms: self.upload_ms,
            ycbcr: self.ycbcr,
            startup_ms: ready_at.map_or(0.0, since),
            first_pixels_ms,
            whole_ms,
            sharp_ms: since(Instant::now()),
            prefetched: self.decode.is_some_and(|d| d.1 < shown_at),
            memory_short: self.memory_short,
        }
    }
}

/// A stage of an open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Startup,
    Read,
    Parse,
    Colour,
    Decode,
    Upload,
    /// Not accounted for by the others: queued behind other images' work, waiting for a frame.
    Waiting,
}

impl Stage {
    pub const fn key(self) -> &'static str {
        match self {
            Stage::Startup => "startup",
            Stage::Read => "read",
            Stage::Parse => "parse",
            Stage::Colour => "colour",
            Stage::Decode => "decode",
            Stage::Upload => "upload",
            Stage::Waiting => "waiting",
        }
    }
}

/// Why a decode that a hardware decoder could do ran on the processor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Software {
    NotProbed,
    NoDevice {
        api: &'static str,
    },
    NoCodec {
        api: &'static str,
        codec: &'static str,
    },
    Blocked(Failure),
    Failed,
    Busy,
    Unsupported,
    /// Off for this session (it timed out or failed its check earlier).
    Unavailable,
    CpuOnly,
    SettingOff,
    /// Not checked against the processor yet: an unchecked path is only tried on images
    /// decoded in the background, never while the user waits.
    Unchecked,
    /// On battery, the processor was the more economical choice.
    LowerPower,
    /// The selector measured (or expects) the processor to be faster here.
    Faster {
        cpu_ms: f64,
        hw_ms: f64,
    },
    /// A reduced-size level: the hardware decoder only decodes whole full-size images.
    Scaled,
}

impl Software {
    pub const fn key(self) -> &'static str {
        match self {
            Software::NotProbed => "not-probed",
            Software::NoDevice { .. } => "no-device",
            Software::NoCodec { .. } => "no-codec",
            Software::Blocked(Failure::CheckFailed) => "blocked-mismatch",
            Software::Blocked(Failure::Error) => "blocked-error",
            Software::Blocked(Failure::DeviceLost) => "blocked-lost",
            Software::Failed => "failed",
            Software::Busy => "busy",
            Software::Unsupported => "unsupported",
            Software::Unavailable => "unavailable",
            Software::CpuOnly => "cpu-only",
            Software::SettingOff => "setting-off",
            Software::Unchecked => "unchecked",
            Software::LowerPower => "lower-power",
            Software::Faster { .. } => "faster",
            Software::Scaled => "scaled",
        }
    }
}

/// The main cause.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finding {
    Fast,
    /// The sharp level came from the display cache.
    DisplayCache,
    SlowRead {
        percent: u8,
        mb_s: f64,
        /// The file's size: a throughput is only worth showing for a big enough read.
        bytes: u64,
    },
    ArchiveRead {
        percent: u8,
    },
    Startup {
        percent: u8,
    },
    /// Decoding dominated and ran in software although a hardware decoder could do it.
    Software(Software),
    /// The hardware decoder's result was compared with the processor's (decoded twice).
    Checked {
        percent: u8,
    },
    /// Decoding dominated, on the hardware decoder.
    Hardware {
        percent: u8,
    },
    /// Decoding dominated (a large image).
    Decode {
        percent: u8,
        mpx: f64,
    },
    Upload {
        percent: u8,
    },
    Waiting {
        percent: u8,
    },
    /// Another stage dominated.
    Stage {
        stage: Stage,
        percent: u8,
    },
    /// Memory ran short (the system may swap).
    MemoryShort,
}

impl Finding {
    pub const fn key(self) -> &'static str {
        match self {
            Finding::Fast => "fast",
            Finding::DisplayCache => "display-cache",
            Finding::SlowRead { .. } => "slow-read",
            Finding::ArchiveRead { .. } => "archive-read",
            Finding::Startup { .. } => "startup",
            Finding::Software(_) => "software",
            Finding::Checked { .. } => "checked",
            Finding::Hardware { .. } => "hardware",
            Finding::Decode { .. } => "decode",
            Finding::Upload { .. } => "upload",
            Finding::Waiting { .. } => "waiting",
            Finding::Stage { .. } => "stage",
            Finding::MemoryShort => "memory-short",
        }
    }
}

/// An obvious fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    CopyLocal,
    Extract,
    /// Settings → Performance → Try again (a blocklisted path).
    TryAgain,
    /// Processing "CPU only" → Automatic.
    Automatic,
    /// Hardware decoding "Off" → Automatic.
    TurnOn,
    FreeMemory,
    /// Only the first hardware decode is checked.
    OnceOnly,
}

impl Hint {
    pub const fn key(self) -> &'static str {
        match self {
            Hint::CopyLocal => "copy-local",
            Hint::Extract => "extract",
            Hint::TryAgain => "try-again",
            Hint::Automatic => "automatic",
            Hint::TurnOn => "turn-on",
            Hint::FreeMemory => "free-memory",
            Hint::OnceOnly => "once-only",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Verdict {
    pub total_ms: f64,
    /// The largest stage and its share of the total (percent).
    pub bottleneck: Option<(Stage, u8)>,
    pub finding: Finding,
    pub hint: Option<Hint>,
    /// Why the decode ran in software when a hardware decoder exists for the task (also
    /// when it was not the bottleneck).
    pub software: Option<Software>,
}

impl Timeline {
    /// The stages with their times (ms); file reads during decoding count as reading.
    pub fn stages(&self) -> [(Stage, f64); 7] {
        let d = self.decode.unwrap_or_default();
        let io = d.io_ms.min(d.ms);
        let known =
            self.startup_ms + self.read_ms + self.parse_ms + self.colour_ms + d.ms + self.upload_ms;
        [
            (Stage::Startup, self.startup_ms),
            (Stage::Read, self.read_ms + io),
            (Stage::Parse, self.parse_ms),
            (Stage::Colour, self.colour_ms),
            (Stage::Decode, d.ms - io),
            (Stage::Upload, self.upload_ms),
            (Stage::Waiting, (self.sharp_ms - known).max(0.0)),
        ]
    }
}

/// Why `p`'s decode stayed in software, if a hardware decoder is a path of its task.
pub fn software_reason(p: &PathFacts) -> Option<Software> {
    if p.engine != Engine::Cpu || p.alt_engine != Some(Engine::HwDecoder) {
        return None;
    }
    if p.reason == Reason::Scaled {
        return Some(Software::Scaled);
    }
    if let Some(f) = p.fallback {
        return Some(match f {
            Fallback::Failed => Software::Failed,
            Fallback::Busy => Software::Busy,
            Fallback::Unsupported => Software::Unsupported,
            Fallback::Unavailable => match p.alt_blocked {
                Some(b) => Software::Blocked(b),
                None => Software::Unavailable,
            },
        });
    }
    if p.mode == Mode::CpuOnly || p.reason == Reason::CpuOnly {
        return Some(Software::CpuOnly);
    }
    if p.forced_off || p.reason == Reason::Forced {
        return Some(Software::SettingOff);
    }
    Some(match p.alt_trust {
        Trust::Blocked => Software::Blocked(p.alt_blocked.unwrap_or(Failure::Error)),
        Trust::Absent => match p.hw {
            HwState::NotProbed => Software::NotProbed,
            HwState::None { api } => Software::NoDevice { api },
            HwState::NoCodec { api } => Software::NoCodec {
                api,
                codec: codec_of(p.task),
            },
            // A decoder that is there but was not offered: given up on this session (it
            // crashed or hung earlier).
            HwState::Present { .. } => Software::Unavailable,
        },
        Trust::Unverified => Software::Unchecked,
        Trust::Trusted if p.reason == Reason::LowerPower => Software::LowerPower,
        Trust::Trusted => Software::Faster {
            cpu_ms: p.cpu_est_ms.unwrap_or(0.0),
            hw_ms: p.alt_est_ms.unwrap_or(0.0),
        },
    })
}

/// The facts around the decision `n` (reads the process's path selector and what the hardware
/// decoder probe found; call right after the decode, on its thread or any other). `probed`:
/// the probe had registered its findings before the decode started; when it had not, a
/// hardware path the decision did not see is put down to the probe still running, even if it
/// finished meanwhile.
pub fn path_facts(n: Note, probed: bool) -> PathFacts {
    let hw = if probed {
        hw_state(n.choice.task)
    } else {
        HwState::NotProbed
    };
    facts_from(n, selector(), hw)
}

/// [`path_facts`] with the selector and the probe's findings given.
fn facts_from(n: Note, sel: &PathSelector, hw: HwState) -> PathFacts {
    let c = n.choice;
    let task = c.task;
    let alt = task.engines().get(1).copied();
    let est = |path| {
        let e = sel.estimate(task, path, c.key, c.mpx);
        (f64::from(e.wall_us) / 1e3, e.source == Source::Measured)
    };
    let (cpu_ms, cpu_measured) = est(0);
    let alt_est = alt.map(|_| est(1));
    PathFacts {
        task,
        engine: c.engine,
        reason: c.reason,
        fallback: n.fallback,
        checked: n.checked,
        alt_engine: alt,
        // As the decision saw it (a probe or check finishing meanwhile changes the selector).
        alt_trust: alt.map_or(Trust::Absent, |_| n.alt_trust),
        alt_blocked: alt
            .and_then(|_| sel.blocked_info(task, 1))
            .map(|b| b.failure),
        cpu_est_ms: Some(cpu_ms),
        alt_est_ms: alt_est.map(|a| a.0),
        measured: cpu_measured && alt_est.is_none_or(|a| a.1),
        mode: sel.mode(),
        forced_off: sel.override_of(task) == Override::Off,
        hw,
        scaled: n.scaled,
    }
}

/// What the hardware decoder probe found for `task`'s codec.
fn hw_state(task: Task) -> HwState {
    const API: &str = if cfg!(target_os = "linux") {
        "VA-API"
    } else if cfg!(windows) {
        "Media Foundation"
    } else {
        "VideoToolbox"
    };
    match specular_hwdec::probed() {
        None => HwState::NotProbed,
        Some(None) => HwState::None { api: API },
        Some(Some(c)) => {
            let has = match task {
                Task::JpegDecode => c.jpeg.is_some(),
                Task::HeicDecode => c.hevc.is_some(),
                _ => false,
            };
            if has {
                HwState::Present { api: c.api }
            } else {
                HwState::NoCodec { api: c.api }
            }
        }
    }
}

fn codec_of(task: Task) -> &'static str {
    match task {
        Task::JpegDecode => "JPEG",
        Task::HeicDecode => "HEVC",
        _ => "",
    }
}

fn percent(part: f64, total: f64) -> u8 {
    if total <= 0.0 {
        return 0;
    }
    (part / total * 100.0).round().clamp(0.0, 100.0) as u8
}

/// The verdict on one open.
pub fn verdict(t: &Timeline) -> Verdict {
    let total = t.sharp_ms.max(0.0);
    let stages = t.stages();
    let (stage, ms) = stages
        .iter()
        .copied()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((Stage::Waiting, 0.0));
    let bottleneck = (ms > 0.0).then(|| (stage, percent(ms, total.max(ms))));
    let path = t.decode.and_then(|d| d.path);
    let software = path.as_ref().and_then(software_reason);
    let mut v = Verdict {
        total_ms: total,
        bottleneck,
        finding: Finding::Fast,
        hint: None,
        software,
    };
    // Decoded in the background before it was asked for: the stages ran before the ask, so
    // only the time to show it counts.
    if t.prefetched {
        return v;
    }
    if total < SLOW_MS {
        return v;
    }
    if t.memory_short {
        v.finding = Finding::MemoryShort;
        v.hint = Some(Hint::FreeMemory);
        return v;
    }
    let pc = bottleneck.map_or(0, |b| b.1);
    let decode = t.decode.unwrap_or_default();
    v.finding = match stage {
        Stage::Startup => Finding::Startup { percent: pc },
        Stage::Read if t.in_archive => {
            v.hint = Some(Hint::Extract);
            Finding::ArchiveRead { percent: pc }
        }
        Stage::Read => {
            let mb_s = t.bytes as f64 / 1e6 / (ms / 1e3).max(1e-6);
            if mb_s < SLOW_STORAGE_MB_S {
                v.hint = Some(Hint::CopyLocal);
            }
            Finding::SlowRead {
                percent: pc,
                mb_s,
                bytes: t.bytes,
            }
        }
        Stage::Decode if decode.from_cache => Finding::DisplayCache,
        Stage::Decode => match (software, path) {
            // Reduced levels never go to the hardware decoder: plainly decode-bound.
            (Some(Software::Scaled), _) => Finding::Decode {
                percent: pc,
                mpx: t.pixels as f64 / 1e6,
            },
            (Some(s), _) => {
                v.hint = match s {
                    Software::Blocked(_) => Some(Hint::TryAgain),
                    Software::CpuOnly => Some(Hint::Automatic),
                    Software::SettingOff => Some(Hint::TurnOn),
                    _ => None,
                };
                Finding::Software(s)
            }
            (None, Some(p)) if p.checked => {
                v.hint = Some(Hint::OnceOnly);
                Finding::Checked { percent: pc }
            }
            (None, Some(p)) if p.engine == Engine::HwDecoder => Finding::Hardware { percent: pc },
            _ => Finding::Decode {
                percent: pc,
                // The whole image: a scaled level still reads (and mostly decodes) all of it.
                mpx: t.pixels as f64 / 1e6,
            },
        },
        Stage::Upload => Finding::Upload { percent: pc },
        Stage::Waiting => Finding::Waiting { percent: pc },
        Stage::Parse | Stage::Colour => Finding::Stage { stage, percent: pc },
    };
    v
}

// ---------------------------------------------------------------------------------------------
// Text

/// One line in plain English for the log and `SPECULAR_OPEN_REPORT` (stable keys, not
/// localised).
pub fn log_line(t: &Timeline, v: &Verdict) -> String {
    use std::fmt::Write;
    let mut s = format!(
        "{} {:.1} MB {:.1} MP{}: sharp in {:.0} ms",
        t.format,
        t.bytes as f64 / 1e6,
        t.pixels as f64 / 1e6,
        if t.in_archive { " (archive)" } else { "" },
        v.total_ms
    );
    if let Some(f) = t.first_pixels_ms {
        let _ = write!(s, ", first pixels {f:.0} ms");
    }
    if let Some(w) = t.whole_ms {
        let _ = write!(s, ", whole image {w:.0} ms");
    }
    s.push_str(" [");
    for (i, (stage, ms)) in t.stages().iter().enumerate() {
        let _ = write!(
            s,
            "{}{} {ms:.1}",
            if i > 0 { ", " } else { "" },
            stage.key()
        );
    }
    s.push(']');
    if let Some(d) = t.decode {
        let _ = write!(s, " level 1/{}", d.scale);
        if d.from_cache {
            s.push_str(" from display cache");
        }
        if d.faults > 0 {
            let _ = write!(s, " ({} page faults, ~{:.1} ms I/O)", d.faults, d.io_ms);
        }
        if let Some(p) = d.path {
            let _ = write!(
                s,
                " path {}={} ({})",
                p.task.key(),
                p.engine.key(),
                p.reason.key()
            );
            if let Some(f) = p.fallback {
                let _ = write!(s, " fell back: {}", f.key());
            }
            if p.checked {
                s.push_str(" checked against cpu");
            }
            if let Some(sc) = p.scaled {
                let _ = write!(s, " scaled 1/{} (dct 1/{})", sc.requested, sc.dct);
                if let Some(f) = sc.fallback {
                    let _ = write!(s, " scale fallback: {}", f.key());
                }
            }
        }
    }
    if let Some(e) = t.ycbcr {
        let _ = write!(s, " ycbcr={}", e.key());
    }
    let _ = write!(s, "; verdict {}", v.finding.key());
    if let Some((stage, pc)) = v.bottleneck {
        let _ = write!(s, " ({} {pc}%)", stage.key());
    }
    if let Some(sw) = v.software {
        let _ = write!(s, "; software: {}", sw.key());
        if let Software::Faster { cpu_ms, hw_ms } = sw {
            let _ = write!(s, " (cpu ~{cpu_ms:.0} ms, hw ~{hw_ms:.0} ms)");
        }
    }
    if let Some(h) = v.hint {
        let _ = write!(s, "; hint {}", h.key());
    }
    s
}

fn num(x: f64) -> String {
    I18n::global()
        .localizer()
        .number(x, if x < 10.0 { 1 } else { 0 })
}

/// The smallest read whose throughput is worth showing: below it the time goes to
/// finding the file (latency: a network share's round trips, a disk's seek), and a figure
/// like "0 MB/s" for a few kilobytes says nothing about the storage.
pub(crate) const MIN_SPEED_BYTES: u64 = 1_000_000;

/// The throughput of reads of about `bytes` at `mb_s` as shown ("2.5"), or `None` when the
/// figure means nothing: reads too small (`MIN_SPEED_BYTES`), or slower than shows (it
/// would round to 0).
pub(crate) fn speed(bytes: u64, mb_s: f64) -> Option<String> {
    (bytes >= MIN_SPEED_BYTES && mb_s >= 0.1 && mb_s.is_finite()).then(|| num(mb_s))
}

fn pct(p: u8) -> String {
    I18n::global().localizer().percent(f64::from(p) / 100.0, 0)
}

/// A stage's name.
pub fn stage_name(s: Stage) -> String {
    match s {
        Stage::Startup => t!("open-report-stage-startup"),
        Stage::Read => t!("open-report-stage-read"),
        Stage::Parse => t!("open-report-stage-parse"),
        Stage::Colour => t!("open-report-stage-colour"),
        Stage::Decode => t!("open-report-stage-decode"),
        Stage::Upload => t!("open-report-stage-upload"),
        Stage::Waiting => t!("open-report-stage-waiting"),
    }
}

/// The engine's standalone name.
fn engine_name(e: Engine) -> String {
    match e {
        Engine::Cpu => t!("paths-engine-cpu"),
        Engine::CpuParallel => t!("paths-engine-cpu-parallel"),
        Engine::Gpu => t!("paths-engine-gpu"),
        Engine::HwDecoder => t!("paths-engine-hw-decoder"),
        Engine::Npu => t!("paths-engine-npu"),
        Engine::HwEncoder => t!("paths-engine-hw-encoder"),
    }
}

/// "Decoded in software: …".
pub fn software_text(s: Software) -> String {
    match s {
        Software::NotProbed => t!("open-report-sw-not-probed"),
        Software::NoDevice { api } => t!("open-report-sw-no-device", api = api),
        Software::NoCodec { api, codec } => {
            t!("open-report-sw-no-codec", api = api, codec = codec)
        }
        Software::Blocked(Failure::CheckFailed) => t!("open-report-sw-blocked-mismatch"),
        Software::Blocked(Failure::Error) => t!("open-report-sw-blocked-error"),
        Software::Blocked(Failure::DeviceLost) => t!("open-report-sw-blocked-lost"),
        Software::Failed => t!("open-report-sw-failed"),
        Software::Busy => t!("open-report-sw-busy"),
        Software::Unsupported => t!("open-report-sw-unsupported"),
        Software::Unavailable => t!("open-report-sw-unavailable"),
        Software::CpuOnly => t!("open-report-sw-cpu-only"),
        Software::SettingOff => t!("open-report-sw-setting-off"),
        Software::Unchecked => t!("open-report-sw-unchecked"),
        Software::LowerPower => t!("open-report-sw-lower-power"),
        Software::Faster { cpu_ms, hw_ms } => {
            t!("open-report-sw-faster", cpu = num(cpu_ms), hw = num(hw_ms))
        }
        Software::Scaled => t!("open-report-sw-scaled"),
    }
}

/// The headline.
pub fn headline(v: &Verdict) -> String {
    match v.finding {
        Finding::Fast => t!("open-report-fast", ms = num(v.total_ms)),
        Finding::DisplayCache => t!("open-report-display-cache"),
        Finding::SlowRead {
            percent,
            mb_s,
            bytes,
        } => match speed(bytes, mb_s) {
            Some(speed) => t!(
                "open-report-slow-read",
                percent = pct(percent),
                speed = speed
            ),
            None => t!("open-report-slow-read-no-speed", percent = pct(percent)),
        },
        Finding::ArchiveRead { percent } => {
            t!("open-report-archive-read", percent = pct(percent))
        }
        Finding::Startup { percent } => t!("open-report-startup", percent = pct(percent)),
        Finding::Software(s) => software_text(s),
        Finding::Checked { percent } => t!("open-report-checked", percent = pct(percent)),
        Finding::Hardware { percent } => t!("open-report-hardware", percent = pct(percent)),
        Finding::Decode { percent, mpx } => {
            t!("open-report-decode", percent = pct(percent), mpx = num(mpx))
        }
        Finding::Upload { percent } => t!("open-report-upload", percent = pct(percent)),
        Finding::Waiting { percent } => t!("open-report-waiting", percent = pct(percent)),
        Finding::Stage { stage, percent } => t!(
            "open-report-stage",
            stage = stage_name(stage),
            percent = pct(percent)
        ),
        Finding::MemoryShort => t!("open-report-memory-short"),
    }
}

pub fn hint_text(h: Hint) -> String {
    match h {
        Hint::CopyLocal => t!("open-report-hint-copy-local"),
        Hint::Extract => t!("open-report-hint-extract"),
        Hint::TryAgain => t!("open-report-hint-try-again"),
        Hint::Automatic => t!("open-report-hint-automatic"),
        Hint::TurnOn => t!("open-report-hint-turn-on"),
        Hint::FreeMemory => t!("open-report-hint-free-memory"),
        Hint::OnceOnly => t!("open-report-hint-once-only"),
    }
}

/// "CPU, at 1/4 size" and how a reduced level was decoded.
fn scaled_text(engine: Engine, sc: Scaled) -> String {
    let engine = engine_name(engine);
    let scale = i64::from(sc.requested);
    match sc.fallback {
        None => t!("open-report-decoder-scaled", engine = engine, scale = scale),
        Some(ScaleFallback::FullSize) => {
            t!(
                "open-report-decoder-scaled-full",
                engine = engine,
                scale = scale
            )
        }
        Some(ScaleFallback::NoIndex) => {
            t!(
                "open-report-decoder-scaled-no-index",
                engine = engine,
                scale = scale
            )
        }
    }
}

/// Localises a report for the information panel.
pub fn localise(t: &Timeline, v: &Verdict) -> OpenReport {
    let ms = |x: f64| t!("unit-milliseconds", value = num(x));
    // The moments in the order they happen, sharp last, then where the time went.
    let mut rows = Vec::new();
    if let Some(f) = t.first_pixels_ms {
        rows.push((t!("open-report-first-pixels"), ms(f)));
    }
    if let Some(w) = t.whole_ms {
        rows.push((t!("open-report-whole"), ms(w)));
    }
    rows.push((t!("open-report-total"), ms(v.total_ms)));
    for (stage, x) in t.stages() {
        // Stages that took no time say nothing.
        if x >= 0.05 {
            rows.push((stage_name(stage), ms(x)));
        }
    }
    if let Some(d) = t.decode {
        let how = if d.from_cache {
            t!("open-report-decoder-cache")
        } else if let Some(p) = d.path {
            if p.checked {
                t!(
                    "open-report-decoder-checked",
                    engine = engine_name(p.engine)
                )
            } else if let Some(sc) = p.scaled {
                scaled_text(p.engine, sc)
            } else {
                engine_name(p.engine)
            }
        } else {
            engine_name(Engine::Cpu)
        };
        rows.push((t!("open-report-decoder"), how));
        // Why not the hardware decoder, unless the headline already says so.
        if let Some(s) = v.software
            && !d.from_cache
            && !matches!(v.finding, Finding::Software(_))
        {
            rows.push((t!("paths-engine-hw-decoder"), software_text(s)));
        }
    }
    if let Some(e) = t.ycbcr {
        rows.push((t!("open-report-colour-conversion"), engine_name(e)));
    }
    OpenReport {
        headline: headline(v),
        hint: v.hint.map(hint_text),
        rows,
    }
}

// ---------------------------------------------------------------------------------------------
// Advisor

/// The recent reports, for the advisor.
static HISTORY: parking_lot::Mutex<advisor::History> =
    parking_lot::Mutex::new(advisor::History::new());

/// The slow opens of the past week and the hidden insights, and the file they are kept in
/// (none: not kept, as in tests and screenshots; hidden insights then last the session).
static KEPT: parking_lot::Mutex<(Option<std::path::PathBuf>, store::Store)> =
    parking_lot::Mutex::new((None, store::Store::new()));

/// The advisor's history file: in the cache folder, on this computer only.
pub fn kept_path() -> std::path::PathBuf {
    specular_settings::Locations::detect()
        .cache_dir
        .join(store::FILE)
}

/// Keeps the slow opens in `path` from now on, after reading the past week's from it into the
/// advisor's history (once, at start-up).
pub fn keep_at(path: std::path::PathBuf) {
    let mut st = store::load(&path, store::unix_now());
    specular_log::debug!(target: specular_log::DECODE,
        "advisor: {} slow opens of the past week, {} hidden insights from {}",
        st.records.len(), st.hidden.len(), path.display());
    HISTORY.lock().seed(st.records.iter().map(|r| (r.t, r.v)));
    let mut kept = KEPT.lock();
    // Insights hidden while the file was being read stay hidden.
    let early = std::mem::take(&mut kept.1.hidden);
    let mut added = false;
    for h in &early {
        added |= st.hide(&h.key, h.scope, h.at);
    }
    *kept = (Some(path), st);
    if added {
        save_kept(&kept);
    }
}

/// Writes the kept history and hidden insights, if they are kept in a file.
fn save_kept(kept: &(Option<std::path::PathBuf>, store::Store)) {
    if let (Some(path), st) = kept
        && let Err(e) = store::save(path, st)
    {
        specular_log::warn!(target: specular_log::DECODE,
            "advisor: cannot keep the history in {}: {e}", path.display());
    }
}

/// Hides the insight with pattern key `key` (the indicator's ×), across sessions: in the
/// gallery's indicator (`thumbnails`) or the viewer's.
pub fn hide(key: &str, thumbnails: bool) {
    let mut kept = KEPT.lock();
    if kept
        .1
        .hide(key, store::Scope::of(thumbnails), store::unix_now())
    {
        save_kept(&kept);
    }
}

/// The pattern keys of the insights hidden in the gallery's indicator (`thumbnails`) or the
/// viewer's.
pub fn hidden(thumbnails: bool) -> Vec<String> {
    KEPT.lock().1.hidden_keys(store::Scope::of(thumbnails))
}

/// Whether any insight is hidden (in either context): "Show hidden insights again" does
/// something.
pub fn any_hidden() -> bool {
    !KEPT.lock().1.hidden.is_empty()
}

/// Shows hidden insights again ("Show hidden insights again"): those of the gallery's
/// indicator (`Some(true)`), the viewer's (`Some(false)`), or all (`None`, Settings).
pub fn unhide(thumbnails: Option<bool>) {
    let scope = thumbnails.map_or(store::Scope::Both, store::Scope::of);
    let mut kept = KEPT.lock();
    if kept.1.unhide(scope) {
        save_kept(&kept);
    }
}

/// The past week's slow opens as the history file has them, for `--hw-report` (read only).
pub fn read_kept(path: &std::path::Path) -> Vec<(Timeline, Verdict)> {
    store::load(path, store::unix_now())
        .records
        .iter()
        .map(|r| (r.t, r.v))
        .collect()
}

/// Adds a finished report to the advisor's history (logging advice that is new), and keeps it
/// when the user waited for it.
pub fn remember(t: &Timeline, v: &Verdict) {
    let new = HISTORY.lock().push(*t, *v);
    for a in new {
        specular_log::debug!(target: specular_log::DECODE, "{}", advisor::log_line(&a));
    }
    if store::worth_keeping(t, v) {
        let mut kept = KEPT.lock();
        if kept.0.is_some() {
            kept.1.push(store::Record {
                at: store::unix_now(),
                t: *t,
                v: *v,
            });
            save_kept(&kept);
        }
    }
}

/// A finished report.
pub type Report = (Timeline, Verdict);

/// The advisor's advice now, and the newest report and slow report it has seen.
pub fn advice() -> (Vec<Advice>, Option<Report>, Option<Report>) {
    let h = HISTORY.lock();
    (h.advise(), h.last().copied(), h.last_slow().copied())
}

/// The report for the information panel: [`localise`], and the advisor's marked advice that
/// bears on this open (the same format decoded in software for the same reason, or reading
/// that held it up).
pub fn panel(t: &Timeline, v: &Verdict) -> OpenReport {
    let mut r = localise(t, v);
    let advice = HISTORY.lock().advise();
    for a in advice.iter().filter(|a| a.marked && bears_on(a, t, v)) {
        r.rows.extend(advice_rows(a));
    }
    r
}

/// Whether `a` is about an open like `t`.
pub fn bears_on(a: &Advice, t: &Timeline, v: &Verdict) -> bool {
    match a.pattern {
        Pattern::Software { format, why, .. } => {
            format == t.format && v.software.is_some_and(|s| s.key() == why.key())
        }
        Pattern::SlowStorage { .. } => v.bottleneck.is_some_and(|b| b.0 == Stage::Read),
        Pattern::FirstOpen { .. } => false,
    }
}

/// The panel rows for one piece of advice: the pattern, the change and the estimated gain.
fn advice_rows(a: &Advice) -> Vec<(String, String)> {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    let pattern = match a.pattern {
        Pattern::Software {
            format,
            count: n,
            of,
            ..
        } => t!(
            "open-report-advice-software",
            count = count(n),
            total = count(of),
            format = format
        ),
        Pattern::SlowStorage {
            archive: true,
            count: n,
            of,
            ..
        } => t!(
            "open-report-advice-archive",
            count = count(n),
            total = count(of)
        ),
        Pattern::SlowStorage {
            mb_s,
            bytes,
            count: n,
            of,
            ..
        } => match speed(bytes, mb_s) {
            Some(speed) => t!(
                "open-report-advice-slow-storage",
                count = count(n),
                total = count(of),
                speed = speed
            ),
            None => t!(
                "open-report-advice-slow-storage-no-speed",
                count = count(n),
                total = count(of)
            ),
        },
        Pattern::FirstOpen { .. } => return Vec::new(),
    };
    let mut rows = vec![(t!("open-report-advice-recurring"), pattern)];
    match a.change {
        Some(Change::Hint(h)) => rows.push((t!("open-report-advice-change"), hint_text(h))),
        Some(Change::HardwareDecoder { codec }) => rows.push((
            t!("open-report-advice-change"),
            t!("open-report-advice-hardware-decoder", codec = codec),
        )),
        None => {}
    }
    if let Some(g) = a.gain {
        rows.push((t!("open-report-advice-gain"), gain_text(g)));
    }
    rows
}

/// "About 3 to 6 times faster" (one decimal below 10).
pub fn gain_text(g: advisor::Gain) -> String {
    if g.high - g.low < 0.1 {
        t!("open-report-advice-gain-one", low = num(g.low))
    } else {
        t!(
            "open-report-advice-gain-range",
            low = num(g.low),
            high = num(g.high)
        )
    }
}

/// The advisor's sentence for a pattern ("5 of the last 6 slow HEIC images were decoded in
/// software for this reason"); nothing for a first open.
pub fn pattern_text(p: &Pattern) -> Option<String> {
    advice_rows(&Advice {
        pattern: *p,
        change: None,
        gain: None,
        marked: false,
    })
    .into_iter()
    .next()
    .map(|r| r.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::paths::Request;

    fn facts(engine: Engine, reason: Reason) -> PathFacts {
        PathFacts {
            task: Task::JpegDecode,
            engine,
            reason,
            fallback: None,
            checked: false,
            alt_engine: Some(Engine::HwDecoder),
            alt_trust: Trust::Absent,
            alt_blocked: None,
            cpu_est_ms: Some(300.0),
            alt_est_ms: Some(400.0),
            measured: true,
            mode: Mode::Automatic,
            forced_off: false,
            hw: HwState::None { api: "VA-API" },
            scaled: None,
        }
    }

    /// A 24 MP JPEG, 10 MB, decoded at full size in `decode_ms`.
    fn jpeg(decode_ms: f64, path: Option<PathFacts>) -> Timeline {
        Timeline {
            format: "JPEG",
            bytes: 10_000_000,
            pixels: 24_000_000,
            read_ms: 0.2,
            parse_ms: 0.5,
            decode: Some(DecodeStage {
                scale: 1,
                ms: decode_ms,
                path,
                ..DecodeStage::default()
            }),
            upload_ms: 12.0,
            first_pixels_ms: Some(40.0),
            sharp_ms: decode_ms + 20.0,
            ..Timeline::default()
        }
    }

    #[test]
    fn fast_opens_are_fast_whatever_the_path() {
        let v = verdict(&jpeg(80.0, Some(facts(Engine::Cpu, Reason::Only))));
        assert_eq!(v.finding, Finding::Fast);
        assert_eq!(v.hint, None);
        // The software reason is still known for the details.
        assert_eq!(v.software, Some(Software::NoDevice { api: "VA-API" }));
        assert_eq!(v.bottleneck.map(|b| b.0), Some(Stage::Decode));
    }

    #[test]
    fn software_without_a_device() {
        let v = verdict(&jpeg(700.0, Some(facts(Engine::Cpu, Reason::Only))));
        assert_eq!(
            v.finding,
            Finding::Software(Software::NoDevice { api: "VA-API" })
        );
        assert_eq!(v.bottleneck, Some((Stage::Decode, 97)));
        assert_eq!(v.hint, None);
    }

    #[test]
    fn software_without_a_codec_or_before_the_probe() {
        let mut p = facts(Engine::Cpu, Reason::Only);
        p.hw = HwState::NoCodec { api: "VA-API" };
        assert_eq!(
            verdict(&jpeg(700.0, Some(p))).finding,
            Finding::Software(Software::NoCodec {
                api: "VA-API",
                codec: "JPEG"
            })
        );
        p.hw = HwState::NotProbed;
        assert_eq!(
            verdict(&jpeg(700.0, Some(p))).finding,
            Finding::Software(Software::NotProbed)
        );
    }

    #[test]
    fn blocklisted_after_a_mismatch_suggests_try_again() {
        let mut p = facts(Engine::Cpu, Reason::Only);
        p.alt_trust = Trust::Blocked;
        p.alt_blocked = Some(Failure::CheckFailed);
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(
            v.finding,
            Finding::Software(Software::Blocked(Failure::CheckFailed))
        );
        assert_eq!(v.hint, Some(Hint::TryAgain));
    }

    #[test]
    fn settings_that_keep_it_in_software_say_so() {
        let mut p = facts(Engine::Cpu, Reason::CpuOnly);
        p.mode = Mode::CpuOnly;
        p.alt_trust = Trust::Trusted;
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(v.finding, Finding::Software(Software::CpuOnly));
        assert_eq!(v.hint, Some(Hint::Automatic));
        let mut p = facts(Engine::Cpu, Reason::Forced);
        p.forced_off = true;
        p.alt_trust = Trust::Trusted;
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(v.finding, Finding::Software(Software::SettingOff));
        assert_eq!(v.hint, Some(Hint::TurnOn));
    }

    #[test]
    fn measured_faster_in_software_has_no_hint() {
        let mut p = facts(Engine::Cpu, Reason::Faster);
        p.alt_trust = Trust::Trusted;
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(
            v.finding,
            Finding::Software(Software::Faster {
                cpu_ms: 300.0,
                hw_ms: 400.0
            })
        );
        assert_eq!(v.hint, None);
    }

    #[test]
    fn unchecked_or_economical_hardware_is_not_called_slower() {
        // An unverified hardware path is never tried while the user waits, even when its
        // prior says it would be faster: the reason is that it is unchecked, not speed.
        let mut p = facts(Engine::Cpu, Reason::Only);
        p.alt_trust = Trust::Unverified;
        p.hw = HwState::Present { api: "VA-API" };
        p.alt_est_ms = Some(100.0);
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(v.finding, Finding::Software(Software::Unchecked));
        assert_eq!(v.hint, None);
        let mut p = facts(Engine::Cpu, Reason::LowerPower);
        p.alt_trust = Trust::Trusted;
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(v.finding, Finding::Software(Software::LowerPower));
        assert!(log_line(&jpeg(700.0, Some(p)), &v).contains("software: lower-power"));
    }

    #[test]
    fn fallbacks_name_what_happened() {
        for (f, want) in [
            (Fallback::Failed, Software::Failed),
            (Fallback::Busy, Software::Busy),
            (Fallback::Unsupported, Software::Unsupported),
            (Fallback::Unavailable, Software::Unavailable),
        ] {
            let mut p = facts(Engine::Cpu, Reason::Faster);
            p.alt_trust = Trust::Trusted;
            p.fallback = Some(f);
            assert_eq!(software_reason(&p), Some(want), "{f:?}");
        }
        // Unavailable because it was blocklisted earlier: the block is the reason.
        let mut p = facts(Engine::Cpu, Reason::Faster);
        p.fallback = Some(Fallback::Unavailable);
        p.alt_blocked = Some(Failure::DeviceLost);
        assert_eq!(
            software_reason(&p),
            Some(Software::Blocked(Failure::DeviceLost))
        );
    }

    #[test]
    fn hardware_and_checked_decodes() {
        let mut p = facts(Engine::HwDecoder, Reason::Faster);
        p.alt_trust = Trust::Trusted;
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(v.finding, Finding::Hardware { percent: 97 });
        assert_eq!(v.software, None);
        p.checked = true;
        let v = verdict(&jpeg(700.0, Some(p)));
        assert_eq!(v.finding, Finding::Checked { percent: 97 });
        assert_eq!(v.hint, Some(Hint::OnceOnly));
    }

    #[test]
    fn tasks_without_a_hardware_decoder_are_plain_decode_bound() {
        let mut p = facts(Engine::CpuParallel, Reason::Faster);
        p.task = Task::PngUnfilter;
        p.alt_engine = Some(Engine::CpuParallel);
        let mut t = jpeg(900.0, Some(p));
        t.format = "PNG";
        t.pixels = 100_000_000;
        let v = verdict(&t);
        assert_eq!(v.software, None);
        assert_eq!(
            v.finding,
            Finding::Decode {
                percent: 98,
                mpx: 100.0
            }
        );
        // No path decision: decode-bound, named by the full size.
        let mut t = jpeg(400.0, None);
        t.decode.as_mut().unwrap().scale = 2;
        assert_eq!(
            verdict(&t).finding,
            Finding::Decode {
                percent: 95,
                mpx: 24.0
            }
        );
    }

    #[test]
    fn scaled_levels_say_how_they_were_decoded() {
        let mut p = facts(Engine::Cpu, Reason::Scaled);
        p.hw = HwState::Present { api: "VA-API" };
        p.alt_trust = Trust::Trusted;
        p.scaled = Some(Scaled {
            requested: 4,
            dct: 4,
            fallback: None,
        });
        let mut t = jpeg(400.0, Some(p));
        t.decode.as_mut().unwrap().scale = 4;
        let v = verdict(&t);
        // Software because the level is reduced, which is no fault: plainly decode-bound.
        assert_eq!(v.software, Some(Software::Scaled));
        assert_eq!(
            v.finding,
            Finding::Decode {
                percent: 95,
                mpx: 24.0
            }
        );
        assert_eq!(v.hint, None);
        let line = log_line(&t, &v);
        assert!(line.contains("jpeg_decode=cpu (scaled)"), "{line}");
        assert!(line.contains("scaled 1/4 (dct 1/4)"), "{line}");
        assert!(!line.contains("scale fallback"), "{line}");
        let plain = localise(&t, &v);
        // Lossless JPEG decodes at full size and reduces afterwards.
        p.scaled = Some(Scaled {
            requested: 8,
            dct: 1,
            fallback: Some(ScaleFallback::FullSize),
        });
        let mut t = jpeg(900.0, Some(p));
        t.decode.as_mut().unwrap().scale = 8;
        let v = verdict(&t);
        let line = log_line(&t, &v);
        assert!(
            line.contains("scaled 1/8 (dct 1/1) scale fallback: full-size"),
            "{line}"
        );
        let full = localise(&t, &v);
        p.scaled = Some(Scaled {
            requested: 2,
            dct: 2,
            fallback: Some(ScaleFallback::NoIndex),
        });
        let t = jpeg(900.0, Some(p));
        let v = verdict(&t);
        assert!(log_line(&t, &v).contains("scale fallback: no-index"));
        let no_index = localise(&t, &v);
        let label = t!("open-report-decoder");
        let decoder = |r: &OpenReport| r.rows.iter().find(|r| r.0 == label).map(|r| r.1.clone());
        let (a, b, c) = (decoder(&plain), decoder(&full), decoder(&no_index));
        assert!(a.is_some() && b.is_some() && c.is_some(), "{plain:?}");
        assert!(a != b && b != c && a != c);
    }

    #[test]
    fn a_probe_that_finishes_during_the_decode_does_not_change_its_reason() {
        // A HEIC decided before the probe registered the hardware decoder (path 1 absent).
        let sel = PathSelector::new();
        let c = sel.choose(Task::HeicDecode, &Request::new(12_000_000, 0));
        assert_eq!(c.engine, Engine::Cpu);
        let n = Note {
            choice: c,
            fallback: None,
            checked: false,
            alt_trust: sel.trust(Task::HeicDecode, 1),
            scaled: None,
        };
        assert_eq!(n.alt_trust, Trust::Absent);
        // The probe finishes while it decodes: the selector now offers an unchecked decoder.
        sel.set_available(Task::HeicDecode, Engine::HwDecoder);
        assert_eq!(sel.trust(Task::HeicDecode, 1), Trust::Unverified);
        let p = facts_from(n, &sel, HwState::NotProbed);
        assert_eq!(p.alt_trust, Trust::Absent);
        assert_eq!(software_reason(&p), Some(Software::NotProbed));
        let mut t = jpeg(900.0, Some(p));
        t.format = "HEIC";
        assert_eq!(verdict(&t).finding, Finding::Software(Software::NotProbed));
        // Probed before it started and nothing there: no device, whatever the selector says.
        let p = facts_from(n, &sel, HwState::None { api: "VA-API" });
        assert_eq!(
            software_reason(&p),
            Some(Software::NoDevice { api: "VA-API" })
        );
    }

    #[test]
    fn slow_storage_counts_page_faults_as_reading() {
        let mut t = jpeg(1000.0, Some(facts(Engine::Cpu, Reason::Only)));
        let d = t.decode.as_mut().unwrap();
        d.io_ms = 700.0;
        d.faults = 2400;
        t.sharp_ms = 1020.0;
        let v = verdict(&t);
        assert_eq!(v.bottleneck, Some((Stage::Read, 69)));
        let Finding::SlowRead { percent, mb_s, .. } = v.finding else {
            panic!("{:?}", v.finding)
        };
        assert_eq!(percent, 69);
        assert!((mb_s - 14.3).abs() < 0.1, "{mb_s}");
        assert_eq!(v.hint, Some(Hint::CopyLocal));
        // Fast storage but a huge file: no hint to copy it.
        t.bytes = 400_000_000;
        assert_eq!(verdict(&t).hint, None);
        // In an archive: extracting helps.
        t.in_archive = true;
        let v = verdict(&t);
        assert_eq!(v.finding, Finding::ArchiveRead { percent: 69 });
        assert_eq!(v.hint, Some(Hint::Extract));
    }

    #[test]
    fn small_reads_show_no_throughput() {
        // The figure needs a big enough read, and one that would not round to 0.
        assert_eq!(speed(500_000, 20.0), None);
        assert_eq!(speed(5_000_000, 0.04), None);
        assert_eq!(speed(5_000_000, f64::INFINITY), None);
        assert!(speed(MIN_SPEED_BYTES, 2.5).is_some());
        // 20 KB read in 700 ms off a slow share: 0.03 MB/s, which says nothing about it
        // (the time went to the share answering): the headline gives no figure.
        let mut t = jpeg(1000.0, Some(facts(Engine::Cpu, Reason::Only)));
        let d = t.decode.as_mut().unwrap();
        d.io_ms = 700.0;
        t.sharp_ms = 1020.0;
        t.bytes = 20_000;
        let v = verdict(&t);
        assert!(matches!(v.finding, Finding::SlowRead { .. }), "{v:?}");
        let h = headline(&v);
        assert!(!h.contains("MB/s") && !h.contains("open-report"), "{h}");
        assert_eq!(v.hint, Some(Hint::CopyLocal));
        // The same read of a 10 MB file does: about 14 MB/s.
        t.bytes = 10_000_000;
        let h = headline(&verdict(&t));
        assert!(h.contains("MB/s"), "{h}");
        // A pattern of small files: no figure either.
        let small = Advice {
            pattern: Pattern::SlowStorage {
                mb_s: 0.03,
                bytes: 20_000,
                archive: false,
                count: 6,
                of: 6,
            },
            change: Some(Change::Hint(Hint::CopyLocal)),
            gain: None,
            marked: true,
        };
        let rows = advice_rows(&small);
        assert!(
            rows.iter()
                .all(|(_, v)| !v.contains("MB/s") && !v.contains("open-report")),
            "{rows:?}"
        );
        assert!(!rows.is_empty());
    }

    #[test]
    fn startup_upload_and_waiting() {
        let mut t = jpeg(100.0, None);
        t.startup_ms = 600.0;
        t.sharp_ms = 740.0;
        assert_eq!(verdict(&t).finding, Finding::Startup { percent: 81 });
        let mut t = jpeg(100.0, None);
        t.upload_ms = 300.0;
        t.sharp_ms = 420.0;
        assert_eq!(verdict(&t).finding, Finding::Upload { percent: 71 });
        let mut t = jpeg(100.0, None);
        t.sharp_ms = 900.0;
        assert_eq!(verdict(&t).finding, Finding::Waiting { percent: 87 });
    }

    #[test]
    fn prefetched_cached_and_memory() {
        let mut t = jpeg(900.0, None);
        t.prefetched = true;
        t.sharp_ms = 15.0;
        assert_eq!(verdict(&t).finding, Finding::Fast);
        let mut t = jpeg(400.0, None);
        t.decode.as_mut().unwrap().from_cache = true;
        assert_eq!(verdict(&t).finding, Finding::DisplayCache);
        let mut t = jpeg(900.0, None);
        t.memory_short = true;
        let v = verdict(&t);
        assert_eq!(v.finding, Finding::MemoryShort);
        assert_eq!(v.hint, Some(Hint::FreeMemory));
    }

    #[test]
    fn stages_never_go_negative_and_the_log_line_names_the_verdict() {
        // Stages measured on other threads can add up to more than the total.
        let mut t = jpeg(700.0, Some(facts(Engine::Cpu, Reason::Only)));
        t.sharp_ms = 500.0;
        assert!(t.stages().iter().all(|s| s.1 >= 0.0));
        let v = verdict(&t);
        assert_eq!(v.bottleneck, Some((Stage::Decode, 100)));
        let line = log_line(&t, &v);
        assert!(line.contains("verdict software"), "{line}");
        assert!(line.contains("software: no-device"), "{line}");
        assert!(line.contains("jpeg_decode=cpu (only)"), "{line}");
    }

    #[test]
    fn advice_localises_and_shows_only_with_its_kind_of_open() {
        use advisor::Gain;
        let storage = Advice {
            pattern: Pattern::SlowStorage {
                mb_s: 20.0,
                bytes: 10_000_000,
                archive: false,
                count: 6,
                of: 6,
            },
            change: Some(Change::Hint(Hint::CopyLocal)),
            gain: Some(Gain {
                low: 3.0,
                high: 4.0,
            }),
            marked: true,
        };
        let heic = Advice {
            pattern: Pattern::Software {
                format: "HEIC",
                why: Software::NoDevice { api: "VA-API" },
                count: 5,
                of: 5,
            },
            change: Some(Change::HardwareDecoder { codec: "HEVC" }),
            gain: Some(Gain {
                low: 4.0,
                high: 4.0,
            }),
            marked: true,
        };
        for a in [storage, heic] {
            let rows = advice_rows(&a);
            assert_eq!(rows.len(), 3, "{rows:?}");
            assert!(
                rows.iter()
                    .all(|(k, v)| !k.is_empty() && !v.is_empty() && !v.contains("open-report")),
                "{rows:?}"
            );
        }
        // A JPEG decoded in software for want of a device: the HEIC advice is not about it.
        let t = jpeg(700.0, Some(facts(Engine::Cpu, Reason::Only)));
        let v = verdict(&t);
        assert!(!bears_on(&heic, &t, &v));
        assert!(!bears_on(&storage, &t, &v));
        let mut t = t;
        t.format = "HEIC";
        assert!(bears_on(&heic, &t, &v));
    }

    #[test]
    fn every_report_localises() {
        let mut p = facts(Engine::Cpu, Reason::Only);
        p.alt_trust = Trust::Blocked;
        p.alt_blocked = Some(Failure::CheckFailed);
        let mut t = jpeg(700.0, Some(p));
        t.ycbcr = Some(Engine::Gpu);
        let s = localise(&t, &verdict(&t));
        assert!(!s.headline.is_empty() && !s.headline.contains("open-report"));
        assert!(s.hint.is_some());
        assert!(s.rows.len() >= 5, "{:?}", s.rows);
        assert!(s.rows.iter().all(|(k, v)| !k.is_empty() && !v.is_empty()));
    }
}

//! Settings → Performance → "How Specular runs on this computer": the path selector's
//! [`Snapshot`] in plain language, under the "Where images are processed" setting
//! (DESIGN.md §12.3.1). Rebuilt when Settings opens, every [`POLL`] while it stays open, and
//! after a settings change or a button; never per frame.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use specular_core::paths::{
    Blocked, Comparison, Confidence, Decision, Engine, Estimate, Failure, Mode, PathState, Reason,
    Setting, Snapshot, Source, Task, TaskSnapshot, Trust, Unit, Why, selector,
};
use specular_i18n::{DateStyle, FluentValue, I18n, t};
use specular_settings::Settings;
use specular_widgets::{Modal, PanelAction, PanelEvent, PanelRow, SettingPanel};

use super::{STATE, save};
use crate::app::App;

/// The setting the panel sits under.
pub(crate) const UNDER: &str = "performance.processing";
/// How often the open panel is rebuilt (measurements trickle in while idle).
const POLL: Duration = Duration::from_secs(2);
/// Below this speed-up the two paths are "about as fast".
const SIMILAR: f32 = 1.15;

struct Poll {
    at: Option<Instant>,
    i18n_gen: u64,
    open: bool,
}

static POLLED: Mutex<Poll> = Mutex::new(Poll {
    at: None,
    i18n_gen: u64::MAX,
    open: false,
});
/// Set by a settings change or a button: rebuild on the next turn.
static STALE: AtomicBool = AtomicBool::new(true);

pub(super) fn mark_stale() {
    STALE.store(true, Ordering::Relaxed);
}

impl App {
    /// Once per event-loop turn: rebuilds the panel while Settings is open. Returns when to
    /// be woken for the next rebuild.
    pub(crate) fn drain_paths_panel(&mut self) -> Option<Instant> {
        let open = self.chrome.modal() == Some(Modal::Settings);
        let now = Instant::now();
        let gen_ = I18n::global().generation();
        let mut p = POLLED.lock();
        let opened = open && !p.open;
        p.open = open;
        if !open {
            return None;
        }
        if opened
            || STALE.swap(false, Ordering::Relaxed)
            || p.i18n_gen != gen_
            || p.at.is_none_or(|t| now >= t)
        {
            p.at = Some(now + POLL);
            p.i18n_gen = gen_;
            let panel = build(&super::snapshot(), &self.settings, unix_now());
            if self.chrome.set_setting_panel(UNDER, Some(panel)) {
                self.redraw = true;
            }
        }
        p.at
    }

    /// The panel's buttons.
    pub(crate) fn paths_panel_event(&mut self, e: &PanelEvent) {
        let sel = selector();
        match e {
            PanelEvent::Row(id) => {
                let Some((task, engine)) = parse_retry(id) else {
                    return;
                };
                if sel.retry(task, engine) {
                    specular_log::info!(target: specular_log::DECODE,
                        "path selector: retry {} on {}", task.key(), engine.key());
                    if engine == Engine::HwDecoder {
                        specular_hwdec::reset_quality_check();
                    }
                    restart_calibration();
                }
            }
            PanelEvent::Action => {
                specular_log::info!(target: specular_log::DECODE,
                    "path selector: measurements forgotten (Measure again)");
                sel.reset_measurements();
                specular_hwdec::reset_quality_check();
                restart_calibration();
            }
            // The rows have no tick boxes or arrows.
            PanelEvent::Check(_) | PanelEvent::Expand(_) => return,
        }
        mark_stale();
        self.redraw = true;
    }
}

/// Calibration from the start, and the table saved now (so a crash does not bring the old
/// blocklist back).
fn restart_calibration() {
    super::IDLE.store(false, Ordering::Relaxed);
    let mut st = STATE.lock();
    st.colour_step = 0;
    st.decoders_done = false;
    st.decoders_needed = true;
    st.next_poll = None;
    save(&st);
}

fn retry_id(task: Task, engine: Engine) -> String {
    format!("retry:{}:{}", task.key(), engine.key())
}

fn parse_retry(id: &str) -> Option<(Task, Engine)> {
    let mut it = id.strip_prefix("retry:")?.split(':');
    let (t, e) = (it.next()?, it.next()?);
    let task = Task::ALL.into_iter().find(|x| x.key() == t)?;
    let engine = task.engines().iter().copied().find(|x| x.key() == e)?;
    Some((task, engine))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// -- wording ---------------------------------------------------------------------------------

/// The standalone name ("GPU").
fn name(e: Engine) -> String {
    I18n::global().tr(e.message_id())
}

/// The name mid-sentence ("the GPU").
fn inline(e: Engine) -> String {
    match e {
        Engine::Cpu => t!("hw-report-engine-cpu"),
        Engine::CpuParallel => t!("hw-report-engine-cpu-parallel"),
        Engine::Gpu => t!("hw-report-engine-gpu"),
        Engine::HwDecoder => t!("hw-report-engine-hw-decoder"),
        Engine::Npu => t!("hw-report-engine-npu"),
        Engine::HwEncoder => t!("hw-report-engine-hw-encoder"),
    }
}

fn ms(us: f32) -> String {
    let ms = f64::from(us) / 1000.0;
    I18n::global()
        .localizer()
        .number(ms, if ms < 10.0 { 1 } else { 0 })
}

/// A day from Unix seconds (UTC), proleptic Gregorian.
fn civil(unix: u64) -> (i32, u8, u8) {
    let z = (unix / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = (yoe + era * 400 + i64::from(m <= 2)) as i32;
    (y, m, d)
}

fn blocked_line(engine: Engine, b: Blocked) -> String {
    let (y, m, d) = civil(b.at_unix);
    let date = (b.at_unix > 0)
        .then(|| I18n::global().localizer().date(y, m, d, DateStyle::Medium))
        .flatten();
    let engine = inline(engine);
    match (date, b.failure) {
        (None, _) => t!("settings-paths-blocked-undated", engine = engine),
        (Some(date), Failure::CheckFailed) => {
            t!("settings-paths-blocked-check", engine = engine, date = date)
        }
        (Some(date), Failure::Error) => {
            t!("settings-paths-blocked-error", engine = engine, date = date)
        }
        (Some(date), Failure::DeviceLost) => {
            t!("settings-paths-blocked-lost", engine = engine, date = date)
        }
    }
}

/// The setting behind a decision and its value ("Where images are processed", "Processor
/// only").
fn setting_line(s: Setting, settings: &Settings) -> String {
    let key = match s {
        Setting::Processing(_) => UNDER,
        Setting::Override(Task::YcbcrToRgb, _) => "performance.fast_yuv_upload",
        Setting::Override(..) => "performance.hardware_decode",
    };
    let i18n = I18n::global();
    let (setting, option) = specular_settings::find(key)
        .zip(settings.get(key))
        .map_or_else(
            || (key.to_owned(), String::new()),
            |(m, v)| (m.label(i18n), m.display_value(&v, i18n)),
        );
    t!("settings-paths-set-by", setting = setting, option = option)
}

/// Why the interactive choice was made, in one sentence; `unit` is what one run of the task
/// works on (only image tasks run on "the image you are looking at").
fn why_line(d: &Decision, unit: Unit, power_saving: bool, settings: &Settings) -> String {
    match d.why {
        Why::Measured(_) if power_saving && d.reason == Reason::LowerPower => {
            t!("settings-paths-measured-power")
        }
        Why::Measured(Comparison {
            chosen,
            other_engine,
            other,
        }) => {
            let ratio = other.wall_us / chosen.wall_us.max(1.0);
            if ratio >= SIMILAR {
                let r = I18n::global()
                    .localizer()
                    .number(f64::from(ratio), if ratio < 10.0 { 1 } else { 0 });
                t!(
                    "settings-paths-measured-faster",
                    ratio = r,
                    other = inline(other_engine)
                )
            } else {
                t!(
                    "settings-paths-measured-similar",
                    other = inline(other_engine)
                )
            }
        }
        Why::NotMeasured(_) => t!("settings-paths-not-measured"),
        Why::Setting(s) => setting_line(s, settings),
        Why::Blocked { engine, blocked } => blocked_line(engine, blocked),
        Why::Checking { engine } => match unit {
            Unit::Image => t!("settings-paths-checking", engine = inline(engine)),
            Unit::Thumbnail { .. } | Unit::Frame { .. } => {
                t!(
                    "settings-paths-checking-before-use",
                    engine = inline(engine)
                )
            }
        },
        Why::Unavailable { engine } => t!("settings-paths-unavailable", engine = inline(engine)),
    }
}

/// What background work (prefetch, thumbnails) uses, when it differs.
fn background_line(t: &TaskSnapshot) -> Option<String> {
    let (i, b) = (&t.interactive, &t.background);
    if b.engine == i.engine {
        return None;
    }
    let engine = inline(b.engine);
    Some(match b.reason {
        Reason::Cheaper => {
            let cpu = |e: Engine| {
                t.paths
                    .iter()
                    .find(|p| p.engine == e)
                    .map(|p| p.estimate.cpu_us)
            };
            match (cpu(b.engine), cpu(i.engine)) {
                (Some(bc), Some(ic)) if ic > 0.0 && bc < ic => t!(
                    "settings-paths-background-cheaper",
                    engine = engine,
                    percent = I18n::global()
                        .localizer()
                        .percent(f64::from(1.0 - bc / ic), 0)
                ),
                _ => t!("settings-paths-background", engine = engine),
            }
        }
        Reason::LowerPower => t!("settings-paths-background-power", engine = engine),
        _ => t!("settings-paths-background", engine = engine),
    })
}

fn confidence(c: Confidence) -> String {
    match c {
        Confidence::None | Confidence::Low => t!("settings-paths-confidence-low"),
        Confidence::Medium => t!("settings-paths-confidence-medium"),
        Confidence::High => t!("settings-paths-confidence-high"),
    }
}

/// One path's numbers: measured ("GPU: 3.1 ms, 0.4 ms CPU time,
/// 12 measurements (medium confidence)") or the built-in estimate.
fn detail(p: &PathState) -> Option<String> {
    let Estimate {
        wall_us,
        cpu_us,
        source,
    } = p.estimate;
    if p.trust == Trust::Absent || !wall_us.is_finite() {
        return None;
    }
    Some(match source {
        Source::Measured if p.samples > 0 => t!(
            "settings-paths-detail-measured",
            engine = name(p.engine),
            time = ms(wall_us),
            cpu = ms(cpu_us),
            runs = FluentValue::from(i64::from(p.samples)),
            confidence = confidence(p.confidence)
        ),
        _ => t!(
            "settings-paths-detail-guess",
            engine = name(p.engine),
            time = ms(wall_us)
        ),
    })
}

/// What a task's times are for when it is not the intro's photo: Inference runs on a small
/// thumbnail, video encoding per frame.
fn unit_line(unit: Unit) -> Option<String> {
    let size = |width: u32, height: u32| {
        let l = I18n::global().localizer();
        t!(
            "hw-report-pixels",
            width = l.integer_plain(i64::from(width)),
            height = l.integer_plain(i64::from(height))
        )
    };
    match unit {
        Unit::Image => None,
        Unit::Thumbnail { width, height } => Some(t!(
            "settings-paths-unit-thumbnail",
            size = size(width, height)
        )),
        Unit::Frame { width, height } => {
            Some(t!("settings-paths-unit-frame", size = size(width, height)))
        }
    }
}

fn row(t: &TaskSnapshot, s: &Snapshot, settings: &Settings) -> PanelRow {
    let d = &t.interactive;
    let by_setting = matches!(d.why, Why::Setting(_));
    let unit = t.task.unit();
    let mut lines = vec![why_line(d, unit, s.power_saving, settings)];
    // Blocked paths the first line does not already name.
    let mut button = None;
    let mut id = String::new();
    for (p, b) in t.blocked() {
        if !matches!(d.why, Why::Blocked { engine, .. } if engine == p.engine) {
            lines.push(blocked_line(p.engine, b));
        }
        if button.is_none() {
            button = Some(t!("settings-paths-retry"));
            id = retry_id(t.task, p.engine);
        }
    }
    if t.exploring_now {
        let trying = t
            .paths
            .iter()
            .filter(|p| {
                p.engine != d.engine && matches!(p.trust, Trust::Trusted | Trust::Unverified)
            })
            .min_by_key(|p| p.samples)
            .map_or(t.background.engine, |p| p.engine);
        lines.push(t!("settings-paths-exploring", engine = inline(trying)));
    }
    lines.extend(background_line(t));
    let details: Vec<String> = unit_line(unit)
        .into_iter()
        .chain(t.paths.iter().filter_map(detail))
        .collect();
    PanelRow {
        id,
        label: I18n::global().tr(t.task.message_id()),
        value: if by_setting {
            t!("settings-paths-your-setting", engine = name(d.engine))
        } else {
            name(d.engine)
        },
        lines,
        detail: specular_i18n::dot_join(&details),
        attention: by_setting || button.is_some(),
        button,
        ..Default::default()
    }
}

/// The whole panel for `s`.
pub(crate) fn build(s: &Snapshot, settings: &Settings, _now_unix: u64) -> SettingPanel {
    let l = I18n::global().localizer();
    let mut intro = t!("settings-paths-intro", mpx = l.number(f64::from(s.mpx), 0));
    if !s.calibrated && s.mode != Mode::CpuOnly {
        intro.push(' ');
        intro.push_str(&t!("settings-paths-intro-measuring"));
    }
    if s.power_saving {
        intro.push(' ');
        intro.push_str(&t!("settings-paths-intro-battery"));
    }
    let gpu = if s.machine.gpu.trim().is_empty() {
        t!("hw-report-none")
    } else {
        s.machine.gpu.clone()
    };
    let rows = s
        .tasks
        .iter()
        .filter(|t| t.active)
        .map(|t| row(t, s, settings))
        .collect();
    SettingPanel {
        title: t!("settings-paths-title"),
        intro,
        rows,
        footer: t!(
            "settings-paths-machine",
            gpu = specular_i18n::isolate_ltr(&gpu),
            driver = specular_i18n::isolate_ltr(s.machine.driver.trim()),
            cpu = specular_i18n::isolate_ltr(s.machine.cpu.trim())
        ),
        action: Some(PanelAction {
            label: t!("settings-paths-measure-again"),
            question: t!("settings-paths-measure-again-question"),
            confirm: t!("settings-paths-measure-again-confirm"),
            cancel: t!("app-cancel"),
        }),
        keywords: t!("settings-paths-keywords"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::paths::PathSelector;

    #[test]
    fn retry_ids_round_trip() {
        for task in Task::ALL {
            for &e in task.engines() {
                assert_eq!(parse_retry(&retry_id(task, e)), Some((task, e)));
            }
        }
        assert_eq!(parse_retry("retry:jpeg_decode:gpu"), None);
        assert_eq!(parse_retry("other"), None);
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(1_759_311_000), (2025, 10, 1));
        assert_eq!(civil(951_782_400), (2000, 2, 29));
    }

    fn table(fixture: &str) -> Box<PathSelector> {
        let sel = Box::new(PathSelector::new());
        assert!(sel.load_table(fixture, None));
        sel
    }

    const BLOCKED: &str =
        include_str!("../../../../../crates/specular-core/tests/paths/blocked-hwdec.txt");

    #[test]
    fn blocked_path_has_its_reason_and_a_retry_button() {
        let sel = table(BLOCKED);
        let p = build(&sel.snapshot(), &Settings::default(), 0);
        let jpeg = p
            .rows
            .iter()
            .find(|r| r.label == t!("hw-report-path-jpeg-decode"))
            .expect("JPEG row");
        assert_eq!(jpeg.value, t!("paths-engine-cpu"));
        assert!(jpeg.attention);
        assert_eq!(
            jpeg.button.as_deref(),
            Some(t!("settings-paths-retry").as_str())
        );
        assert_eq!(jpeg.id, "retry:jpeg_decode:hw_decoder");
        assert!(
            jpeg.lines.iter().any(|l| l.contains("did not match")),
            "{:?}",
            jpeg.lines
        );
        assert!(p.matches("GPU"));
        assert!(p.matches("hardware"));
        assert!(p.footer.contains("860M"), "{}", p.footer);
    }

    #[test]
    fn cpu_only_is_named_as_the_users_setting() {
        let sel = table(BLOCKED);
        sel.set_mode(Mode::CpuOnly);
        let mut s = Settings::default();
        s.performance.processing = specular_settings::PathMode::CpuOnly;
        let p = build(&sel.snapshot(), &s, 0);
        let colour = p
            .rows
            .iter()
            .find(|r| r.label == t!("hw-report-path-ycbcr-to-rgb"))
            .expect("colour row");
        assert_eq!(
            colour.value.replace(['\u{2068}', '\u{2069}'], ""),
            "CPU (your setting)"
        );
        assert!(colour.lines[0].contains("CPU only"), "{:?}", colour.lines);
    }

    /// After the HEIC calibration (measurements on a Radeon 860M: software
    /// 24.9 ms/MP, VA-API 8.0 ms/MP, pictures identical) the panel shows the HEIC row choosing
    /// the hardware decoder.
    #[test]
    fn heic_row_shows_the_calibrated_choice() {
        let sel = table(concat!(
            "# Specular path timings (DESIGN.md §12.3.1), version 1\n",
            "machine gpu=integrated threads=16\n",
            "calibrated true\n",
            "trust heic_decode hw_decoder trusted\n",
            "cell heic_decode cpu 0 2 idle 2 24903.6 2 153961.4\n",
            "cell heic_decode hw_decoder 0 2 idle 2 8008.5 2 13633.5\n",
        ));
        let p = build(&sel.snapshot(), &Settings::default(), 0);
        let label = I18n::global().tr(Task::HeicDecode.message_id());
        let heic = p
            .rows
            .iter()
            .find(|r| r.label == label)
            .unwrap_or_else(|| panic!("no HEIC row in {:?}", p.rows));
        assert_eq!(heic.value, name(Engine::HwDecoder));
        assert!(!heic.attention);

        // Without a usable hardware decoder the calibrated row names the processor.
        let sel = table(concat!(
            "# Specular path timings (DESIGN.md §12.3.1), version 1\n",
            "machine gpu=integrated threads=16\n",
            "calibrated true\n",
            "cell heic_decode cpu 0 2 idle 2 24903.6 2 153961.4\n",
        ));
        let p = build(&sel.snapshot(), &Settings::default(), 0);
        let heic = p.rows.iter().find(|r| r.label == label).expect("HEIC row");
        assert_eq!(heic.value, name(Engine::Cpu));
    }

    /// After the inference calibration (a ViT-Ti/16 at 224² on a Radeon 860M: GPU
    /// about 26 ms, CPU about 90 ms) the row shows thumbnail times, not a 24-megapixel
    /// extrapolation, and says so.
    #[test]
    fn inference_row_is_timed_on_a_thumbnail() {
        let sel = table(concat!(
            "# Specular path timings (DESIGN.md §12.3.1), version 1\n",
            "machine gpu=integrated threads=16\n",
            "calibrated true\n",
            "trust inference gpu unverified\n",
        ));
        let px = specular_infer::calibrate::PIXELS;
        for (path, ms) in [(1, 26.0), (1, 25.0), (0, 90.0)] {
            let key = sel.calibration(Task::Inference, path, px, 0).key;
            sel.record(Task::Inference, path, key, px as f32 / 1e6, ms * 1e3, None);
        }
        let p = build(&super::super::snapshot_of(&sel), &Settings::default(), 0);
        let label = I18n::global().tr(Task::Inference.message_id());
        let row = p
            .rows
            .iter()
            .find(|r| r.label == label)
            .unwrap_or_else(|| panic!("no inference row in {:?}", p.rows));
        let side = specular_infer::calibrate::SIDE;
        assert!(row.detail.contains(&side.to_string()), "{}", row.detail);
        // Times at the thumbnail's size: tens of milliseconds, not seconds.
        let unisolated = row.detail.replace(['\u{2068}', '\u{2069}'], "");
        assert!(unisolated.contains("GPU: 2"), "{unisolated}");
        assert!(unisolated.contains("CPU: 9"), "{unisolated}");
        eprintln!(
            "{} = {}: {:?} / {}",
            row.label, row.value, row.lines, unisolated
        );
    }

    /// Image analysis runs on a 224 × 224 thumbnail, not on the image being viewed: its row
    /// says so, and its GPU check is not "for the image you are looking at".
    #[test]
    fn thumbnail_tasks_say_what_they_are_timed_on() {
        let sel = PathSelector::new();
        sel.set_machine(specular_core::paths::GpuClass::Integrated, 16);
        sel.set_available(Task::Inference, Engine::Gpu);
        sel.set_available(Task::JpegDecode, Engine::HwDecoder);
        let p = build(&sel.snapshot(), &Settings::default(), 0);
        let row = |task: Task| {
            let label = I18n::global().tr(task.message_id());
            p.rows
                .iter()
                .find(|r| r.label == label)
                .unwrap_or_else(|| panic!("no {task:?} row in {:?}", p.rows))
        };
        let plain = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
        let ai = row(Task::Inference);
        assert_eq!(
            plain(&ai.lines[0]),
            "The GPU is checked against the CPU before it is used."
        );
        assert!(
            plain(&ai.detail).starts_with("Times are for one 224 × 224 px thumbnail · "),
            "{}",
            ai.detail
        );
        let jpeg = row(Task::JpegDecode);
        assert!(
            jpeg.lines[0].contains("the image you are looking at"),
            "{:?}",
            jpeg.lines
        );
        assert!(!jpeg.detail.contains("Times are"), "{}", jpeg.detail);
        assert_eq!(
            unit_line(Task::VideoEncode.unit())
                .map(|l| plain(&l))
                .as_deref(),
            Some("Times are per 1920 × 1080 px video frame")
        );
    }

    #[test]
    fn every_reason_has_wording() {
        let sel = table(BLOCKED);
        let p = build(&sel.snapshot(), &Settings::default(), 0);
        for r in &p.rows {
            assert!(!r.value.is_empty());
            assert!(!r.lines.is_empty() && r.lines.iter().all(|l| !l.is_empty()));
            // No raw message ids (a missing message falls back to its id).
            for l in r.lines.iter().chain([&r.value, &r.detail, &r.label]) {
                assert!(!l.contains("settings-paths-"), "{l}");
                assert!(!l.contains("paths-engine-"), "{l}");
            }
        }
    }
}

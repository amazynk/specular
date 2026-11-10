//! Background work: the indicator shown while something runs in the background and the
//! "Background work" panel it and the palette open. The jobs report into
//! [`specular_core::work`]; this localises a snapshot of them for the chrome, announces
//! starts and finishes, and carries out the panel's buttons.

use std::time::{Duration, Instant};

use specular_core::work::{self, Job, Kind, Pause, Step, Unit};
use specular_i18n::{ByteUnits, I18n, ListKind, Localizer, t};
use specular_widgets::{PaletteItem, WorkChip, WorkPanel, WorkRow};

use super::App;

/// The palette item "Show background work".
pub(super) const ITEM: u64 = 1 << 43;

/// Looked at least this often while something runs (the time left moves on).
const REFRESH: Duration = Duration::from_secs(1);
/// Changes are shown at most this often (progress moves many times a second).
const MIN_REFRESH: Duration = Duration::from_millis(250);
/// A job is announced only once it has run this long (no chatter for short passes).
const ANNOUNCE_AFTER: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct WorkState {
    waker_set: bool,
    /// The registry's generation and the language's when last shown.
    seen: Option<(u64, u64)>,
    last: Option<Instant>,
    /// Kinds running at the last look, and whether their start was announced.
    running: Vec<(Kind, bool)>,
    /// The indicator as last set: an unchanged one draws nothing, and while the panel is
    /// closed its count moves at most once per `REFRESH` (a Library walk out of sight cost
    /// four frames a second; the app gate's hidden walk).
    chip: Option<WorkChip>,
    chip_at: Option<Instant>,
}

impl std::fmt::Debug for WorkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkState")
            .field("seen", &self.seen)
            .field("running", &self.running)
            .finish_non_exhaustive()
    }
}

impl App {
    pub(super) fn work_items(&self) -> Vec<PaletteItem> {
        vec![PaletteItem {
            id: ITEM,
            label: t!("work-show"),
            key: None,
            unavailable: None,
            icon: specular_gfx::Icon::Info,
        }]
    }

    /// Opens the panel (the palette; the indicator opens it itself).
    pub(super) fn open_work(&mut self) {
        let jobs = work::snapshot();
        let loc = I18n::global().localizer();
        self.chrome.show_work(panel(&jobs, &loc));
        self.redraw = true;
    }

    /// The panel's buttons: "pause:<kind>", "resume:<kind>", "settings:<page>".
    pub(super) fn work_command(&mut self, key: &str) {
        let Some((verb, arg)) = key.split_once(':') else {
            return;
        };
        match verb {
            "pause" | "resume" => {
                if let Some(k) = kind_of(arg) {
                    work::set_user_paused(k, verb == "pause");
                }
            }
            "settings" => self.chrome.open_settings_at(arg),
            _ => return,
        }
        self.work.seen = None;
        self.work_tick();
        self.redraw = true;
    }

    /// Follows the registry: the indicator, the open panel and the announcements. Returns
    /// when to look again.
    pub(super) fn work_tick(&mut self) -> Option<Instant> {
        if !self.work.waker_set {
            work::set_waker(self.loader.waker());
            self.work.waker_set = true;
        }
        let now = Instant::now();
        let at = (work::generation(), I18n::global().generation());
        let since = self.work.last.map(|l| now.duration_since(l));
        let busy = !self.work.running.is_empty();
        let due = match (self.work.seen, since) {
            (Some(s), Some(d)) if s == at => busy && d >= REFRESH,
            (Some(_), Some(d)) => d >= MIN_REFRESH,
            _ => true,
        };
        if !due {
            let last = self.work.last?;
            return if self.work.seen == Some(at) {
                busy.then(|| last + REFRESH)
            } else {
                Some(last + MIN_REFRESH)
            };
        }
        self.work.seen = Some(at);
        self.work.last = Some(now);
        let jobs = work::snapshot();
        let loc = I18n::global().localizer();
        self.announce_work(&jobs, now);
        let chip = chip(&jobs, &loc);
        let paused = |c: &Option<WorkChip>| c.as_ref().map(|c| c.paused);
        // Shown, gone, paused or resumed: at once; only the label (a count): once a second
        // unless the panel is open.
        let state = chip.is_some() != self.chrome.work_chip_shown()
            || paused(&chip) != paused(&self.work.chip);
        let label_due = self.chrome.work_open()
            || self
                .work
                .chip_at
                .is_none_or(|t| now.duration_since(t) >= REFRESH);
        if state || (chip != self.work.chip && label_due) {
            self.work.chip = chip.clone();
            self.work.chip_at = Some(now);
            self.chrome.set_work_chip(chip);
            self.redraw = true;
        }
        if self.chrome.work_open() {
            self.chrome.set_work(panel(&jobs, &loc));
            self.redraw = true;
        }
        (!jobs.is_empty()).then(|| now + REFRESH)
    }

    /// Polite announcements: a job's start once it has run a second, its finish if its
    /// start was announced.
    fn announce_work(&mut self, jobs: &[Job], now: Instant) {
        let mut next: Vec<(Kind, bool)> = Vec::new();
        for j in jobs {
            if next.iter().any(|(k, _)| *k == j.kind) {
                continue;
            }
            let told = self
                .work
                .running
                .iter()
                .find(|(k, _)| *k == j.kind)
                .is_some_and(|(_, t)| *t);
            let oldest = jobs
                .iter()
                .filter(|o| o.kind == j.kind)
                .map(|o| o.started)
                .min()
                .unwrap_or(now);
            let tell = !told && now.duration_since(oldest) >= ANNOUNCE_AFTER;
            if tell {
                self.chrome
                    .announce_status(t!("work-started", job = kind_name(j.kind)));
            }
            next.push((j.kind, told || tell));
        }
        for (k, told) in &self.work.running {
            if *told && !next.iter().any(|(n, _)| n == k) {
                self.chrome
                    .announce_status(t!("work-finished", job = kind_name(*k)));
            }
        }
        self.work.running = next;
    }
}

fn kind_key(k: Kind) -> &'static str {
    match k {
        Kind::Library => "library",
        Kind::AiSearch => "ai-search",
        Kind::Faces => "faces",
        Kind::Text => "text",
        Kind::Culling => "culling",
        Kind::Download => "download",
        Kind::Similar => "similar",
        Kind::Speech => "speech",
    }
}

fn kind_of(key: &str) -> Option<Kind> {
    [
        Kind::Library,
        Kind::AiSearch,
        Kind::Faces,
        Kind::Text,
        Kind::Culling,
        Kind::Download,
        Kind::Similar,
        Kind::Speech,
    ]
    .into_iter()
    .find(|k| kind_key(*k) == key)
}

/// The settings a job's "Settings…" opens.
fn settings_page(k: Kind) -> &'static str {
    match k {
        Kind::Library | Kind::Similar => "gallery",
        Kind::AiSearch => "ai.search",
        Kind::Faces => "ai.people",
        Kind::Text => "ai.text",
        Kind::Culling => "ai.culling",
        Kind::Speech => "ai.speech",
        Kind::Download => "ai",
    }
}

fn kind_name(k: Kind) -> String {
    match k {
        Kind::Library => t!("work-kind-library"),
        Kind::AiSearch => t!("work-kind-ai-search"),
        Kind::Faces => t!("work-kind-faces"),
        Kind::Text => t!("work-kind-text"),
        Kind::Culling => t!("work-kind-culling"),
        Kind::Download => t!("work-kind-download"),
        Kind::Similar => t!("work-kind-similar"),
        Kind::Speech => t!("work-kind-speech"),
    }
}

fn step_name(s: Step) -> String {
    match s {
        Step::Finding => t!("work-step-finding"),
        Step::Reading => t!("work-step-reading"),
        Step::Signatures => t!("work-step-signatures"),
        Step::Grouping => t!("work-step-grouping"),
        Step::TextIndex => t!("work-step-text-index"),
        Step::Embedding => t!("work-step-embedding"),
        Step::Switching => t!("work-step-switching"),
        Step::Faces => t!("work-step-faces"),
        Step::Ocr => t!("work-step-ocr"),
        Step::Scoring => t!("work-step-scoring"),
        Step::Downloading => t!("work-step-downloading"),
        Step::Focus => t!("work-step-focus"),
        Step::Transcribing => t!("work-step-transcribing"),
    }
}

fn pause_reason(p: Pause) -> String {
    match p {
        Pause::Browsing => t!("work-paused-browsing"),
        Pause::Battery => t!("work-paused-battery"),
        Pause::LowPower => t!("work-paused-low-power"),
        Pause::Heat => t!("work-paused-heat"),
        Pause::Memory => t!("work-paused-memory"),
        Pause::User => t!("work-paused-user"),
    }
}

fn amount(unit: Unit, n: u64, loc: &Localizer) -> String {
    match unit {
        Unit::Bytes => loc.bytes(n, ByteUnits::Decimal),
        Unit::Photos | Unit::Items => loc.integer(i64::try_from(n).unwrap_or(i64::MAX)),
    }
}

fn plain(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// "1,240 of 9,800 photos · 8,560 to go", or "1,240 photos so far".
fn count(j: &Job, loc: &Localizer) -> String {
    let done = amount(j.unit, j.done, loc);
    match (j.total, j.unit) {
        (Some(total), unit) => {
            let of = if unit == Unit::Photos {
                t!(
                    "work-count-photos",
                    done = done,
                    total = amount(unit, total, loc),
                    count = plain(total)
                )
            } else {
                t!("work-count", done = done, total = amount(unit, total, loc))
            };
            match j.left() {
                Some(left) if left > 0 => {
                    t!(
                        "work-count-left",
                        count = of,
                        left = amount(unit, left, loc)
                    )
                }
                _ => of,
            }
        }
        (None, Unit::Photos) => t!("work-so-far-photos", done = done, count = plain(j.done)),
        (None, _) => t!("work-so-far", done = done),
    }
}

/// The time left: seconds (in fives) under a minute, minutes under an hour, then hours and
/// minutes.
pub(crate) fn eta_text(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 57.5 {
        let n = ((s / 5.0).round() as i64 * 5).max(5);
        return t!("work-eta-seconds", n = n);
    }
    let mins = (s / 60.0).round() as i64;
    if mins < 60 {
        return t!("work-eta-minutes", n = mins.max(1));
    }
    let (h, m) = (mins / 60, mins % 60);
    if m == 0 {
        t!("work-eta-hours", h = h)
    } else {
        t!("work-eta-hours-minutes", h = h, m = m)
    }
}

fn status(j: &Job) -> String {
    if let Some(p) = j.paused {
        return pause_reason(p);
    }
    if j.estimating() {
        return t!("work-eta-estimating");
    }
    j.eta().map(eta_text).unwrap_or_default()
}

fn row(j: &Job, loc: &Localizer) -> WorkRow {
    let step = step_name(j.step);
    let step = match &j.item {
        Some(item) => t!("work-step-item", step = step, item = item.clone()),
        None => step,
    };
    let then = (!j.then.is_empty()).then(|| {
        let steps: Vec<String> = j.then.iter().map(|s| step_name(*s)).collect();
        t!("work-then", steps = loc.list(&steps, ListKind::And))
    });
    let pause = j.pausable.then(|| {
        if j.paused == Some(Pause::User) {
            (t!("work-resume"), format!("resume:{}", kind_key(j.kind)))
        } else {
            (t!("work-pause"), format!("pause:{}", kind_key(j.kind)))
        }
    });
    WorkRow {
        title: kind_name(j.kind),
        step,
        count: count(j, loc),
        fraction: j.fraction(),
        status: status(j),
        paused: j.paused.is_some(),
        then,
        pause,
        settings: Some((
            t!("work-settings"),
            format!("settings:{}", settings_page(j.kind)),
        )),
    }
}

/// The panel's content for these jobs.
pub(crate) fn panel(jobs: &[Job], loc: &Localizer) -> WorkPanel {
    WorkPanel {
        rows: jobs.iter().map(|j| row(j, loc)).collect(),
    }
}

/// The indicator for these jobs: the first one running (else the first), and how many
/// more there are.
pub(crate) fn chip(jobs: &[Job], loc: &Localizer) -> Option<WorkChip> {
    let main = jobs
        .iter()
        .find(|j| j.paused.is_none())
        .or_else(|| jobs.first())?;
    let job = kind_name(main.kind);
    let label = match (main.paused, main.total) {
        (Some(p), _) => t!("work-chip-paused", job = job, reason = pause_reason(p)),
        (None, Some(total)) => t!(
            "work-chip",
            job = job,
            done = amount(main.unit, main.done, loc),
            total = amount(main.unit, total, loc)
        ),
        (None, None) => t!(
            "work-chip-so-far",
            job = job,
            done = amount(main.unit, main.done, loc)
        ),
    };
    let more = jobs.len() - 1;
    let label = if more > 0 {
        t!("work-chip-more", chip = label, more = plain(more as u64))
    } else {
        label
    };
    Some(WorkChip {
        name: t!("work-chip-name", status = label.clone()),
        label,
        paused: main.paused.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc() -> std::sync::Arc<Localizer> {
        I18n::global().localizer()
    }

    #[test]
    fn eta_rounds_by_size() {
        let s = |n: u64| eta_text(Duration::from_secs(n)).replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(s(2), "About 5 seconds left");
        assert_eq!(s(23), "About 25 seconds left");
        assert_eq!(s(59), "About 1 minute left");
        assert_eq!(s(25 * 60 + 10), "About 25 minutes left");
        assert_eq!(s(3600), "About 1 hour left");
        assert_eq!(s(2 * 3600 + 5 * 60), "About 2 hours 5 minutes left");
    }

    #[test]
    fn chip_and_panel_from_jobs() {
        let a = work::fake(Kind::AiSearch, Step::Embedding, 1240, Some(9800), 20.0);
        let b = work::fake(Kind::Library, Step::Reading, 10, None, 0.0);
        b.set_paused(Some(Pause::Battery));
        let jobs: Vec<Job> = work::snapshot()
            .into_iter()
            .filter(|j| j.id == a.id() || j.id == b.id())
            .collect();
        let strip = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
        let c = chip(&jobs, &loc()).unwrap();
        assert!(
            strip(&c.label).starts_with("AI search · 1,240 of 9,800"),
            "{}",
            c.label
        );
        assert!(strip(&c.label).ends_with("(+1)"), "{}", c.label);
        assert!(!c.paused);
        assert!(strip(&c.name).starts_with("Background work: AI search"));
        let p = panel(&jobs, &loc());
        let ai = p
            .rows
            .iter()
            .find(|r| strip(&r.title) == "AI search")
            .unwrap();
        assert_eq!(strip(&ai.count), "1,240 of 9,800 photos · 8,560 to go");
        assert!(strip(&ai.status).starts_with("About "), "{}", ai.status);
        let lib = p
            .rows
            .iter()
            .find(|r| strip(&r.title) == "Photo library")
            .unwrap();
        assert!(lib.paused);
        assert_eq!(strip(&lib.status), "Paused on battery power");
        assert_eq!(strip(&lib.count), "10 photos so far");
        assert_eq!(lib.fraction, None);
        assert_eq!(
            lib.settings.as_ref().map(|s| s.1.as_str()),
            Some("settings:gallery")
        );
        drop((a, b));
    }

    #[test]
    fn keys_round_trip() {
        for k in [
            Kind::Library,
            Kind::AiSearch,
            Kind::Faces,
            Kind::Text,
            Kind::Culling,
            Kind::Download,
            Kind::Similar,
            Kind::Speech,
        ] {
            assert_eq!(kind_of(kind_key(k)), Some(k));
            assert!(
                specular_settings::find(settings_page(k)).is_some()
                    || settings_page(k) == "gallery"
                    || settings_page(k) == "ai"
            );
        }
    }
}

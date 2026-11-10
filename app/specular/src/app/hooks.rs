//! Hooks: programs settings.toml names (`[[hooks]]`) to run when something happens in the
//! viewer ([`super::events`]). No scripting language and no plugins: a hook is a program the
//! user installed, started without a shell, with the event's files as arguments and the event
//! as one JSON line on its standard input (`$SPECULAR_EVENT` names it, `$SPECULAR_HOOK` the
//! hook, `$SPECULAR_SOCKET` the viewer's socket to answer through).
//!
//! - **Consent.** Anything that can write the settings file (a synced dotfile, a template)
//!   could add a hook, so each hook asks once before its first run, naming its program and
//!   events: Allow or Not now. The answer is kept by a fingerprint of the program, its
//!   arguments and events (`hooks-trusted.txt`, as recipes keep theirs): a changed hook asks
//!   again. Not now holds for this session.
//! - **Off the window's thread.** At most [`PER_HOOK`] runs of a hook at once and [`TOTAL`]
//!   in all; further events wait, up to [`QUEUE`] per hook, then are dropped with a log line.
//!   Each run is stopped after its `timeout`. Failures are a quiet note, never a dialog.
//! - **No loops.** Events caused by a hook's program (it said so through `$SPECULAR_HOOK`)
//!   run no hooks; subscribers still hear them.
//! - Never in screenshots, benchmarks or other runs without a window.
//!
//! Settings › Integration lists the hooks under "Remote control and events", each with an
//! on/off switch and Remove… (which asks first; the program and files stay).

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use serde_json::Value;
use specular_i18n::{I18n, t};
use specular_log::{SETTINGS, info, warn};
use specular_platform::launch::{self, HookVars, OnExit, Piped};
use specular_settings::commands::{Hook, HookProblem, WhenValue};
use specular_widgets::{CheckState, PanelEvent, PanelRow, SettingPanel};

use super::App;
use super::events::Source;
use crate::remote::events::Line;

/// The setting the hooks' panel sits under.
pub(super) const UNDER: &str = "integration.ipc";
/// Where the allowed hooks are listed, in the config folder.
const TRUST_FILE: &str = "hooks-trusted.txt";
pub(super) const ALLOW_ITEM: u64 = super::items::HOOK_ALLOW;
pub(super) const NOT_NOW_ITEM: u64 = super::items::HOOK_NOT_NOW;
pub(super) const REMOVE_ITEM: u64 = super::items::HOOK_REMOVE;

/// Runs of one hook at once.
pub(crate) const PER_HOOK: usize = 2;
/// Runs of all hooks at once.
pub(crate) const TOTAL: usize = 4;
/// Events a hook keeps waiting (while busy, or until it is allowed); more are dropped.
pub(crate) const QUEUE: usize = 64;

/// A stable fingerprint of what a hook runs (FNV-1a over its program, arguments and
/// events): any change asks again.
pub(crate) fn fingerprint(h: &Hook) -> u64 {
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |s: &str| {
        for b in s.bytes().chain([0]) {
            x ^= u64::from(b);
            x = x.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat("hook");
    eat(&h.program);
    for a in &h.args {
        eat(a);
    }
    eat("on");
    for e in &h.on {
        eat(e);
    }
    x
}

/// The member `key` of an event: at the top, else in one of its objects (`flag` is
/// `mark.flag`), or a dotted path (`mark.stars`).
fn member<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    if let Some(x) = v.get(key) {
        return Some(x);
    }
    if key.contains('.') {
        return key.split('.').try_fold(v, |o, k| o.get(k));
    }
    v.as_object()?
        .values()
        .find_map(|o| o.as_object()?.get(key))
}

/// Whether event `v` passes the hook's `when` (each member equal; `"none"` also matches an
/// empty one).
pub(crate) fn passes(h: &Hook, v: &Value) -> bool {
    h.when.iter().all(|(k, want)| {
        let got = member(v, k).unwrap_or(&Value::Null);
        match (want, got) {
            (WhenValue::Text(w), Value::String(g)) => w == g,
            (WhenValue::Text(w), Value::Null) => w == "none",
            (WhenValue::Number(w), Value::Number(g)) => g.as_f64() == Some(*w),
            (WhenValue::Bool(w), Value::Bool(g)) => w == g,
            _ => false,
        }
    })
}

/// One run of a hook, ready to start.
#[derive(Clone, Debug)]
pub(crate) struct Job {
    pub key: u64,
    pub name: String,
    pub program: OsString,
    pub args: Vec<OsString>,
    pub input: Vec<u8>,
    pub env: Vec<(&'static str, OsString)>,
    pub timeout: Duration,
}

/// The run of hook `h` for event line `l` (`socket`: the viewer's, for its program).
pub(crate) fn job(h: &Hook, l: &Line, socket: Option<&std::path::Path>) -> Result<Job, String> {
    let v = &l.value;
    let files: Vec<PathBuf> = v["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    let outputs: Vec<PathBuf> = v["outputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.get(1)?.as_str())
        .map(PathBuf::from)
        .collect();
    let event = v["event"].as_str().unwrap_or("");
    let vars = HookVars {
        event,
        recipe: v["recipe"].as_str().unwrap_or(""),
        outputs: &outputs,
        stars: v["mark"]["stars"].as_u64().map(|s| s.min(5) as u8),
    };
    let args = launch::expand_hook_args(&h.args, &files, &vars).map_err(|e| e.to_string())?;
    let mut env = vec![
        ("SPECULAR_EVENT", OsString::from(event)),
        ("SPECULAR_HOOK", OsString::from(&h.name)),
    ];
    if let Some(s) = socket {
        env.push(("SPECULAR_SOCKET", s.as_os_str().to_owned()));
    }
    let mut input = l.text.as_bytes().to_vec();
    input.push(b'\n');
    Ok(Job {
        key: fingerprint(h),
        name: h.name.clone(),
        program: OsString::from(&h.program),
        args,
        input,
        env,
        timeout: Duration::from_secs(u64::from(h.timeout)),
    })
}

/// Why a run did not work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    Exit(i32),
    /// Ended by a signal.
    Killed,
    Timeout(u64),
    Start(String),
}

/// What the runner tells the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
    Failed {
        name: String,
        why: Why,
    },
    /// An event the hook had no room for.
    Dropped {
        name: String,
    },
}

/// Starts a job, calling `on_exit` when it ends.
pub(crate) type Spawn = dyn Fn(&Job, OnExit) -> io::Result<()> + Send + Sync;

#[derive(Default)]
struct RunState {
    total: usize,
    running: HashMap<u64, usize>,
    queued: HashMap<u64, VecDeque<Job>>,
    /// Hooks with waiting jobs, in the order they first waited (fair turns).
    order: VecDeque<u64>,
}

/// Runs jobs within the limits, off the window's thread.
pub(crate) struct Runner {
    state: Mutex<RunState>,
    notes: Sender<Note>,
    spawn: Box<Spawn>,
}

impl Runner {
    pub(crate) fn new(spawn: Box<Spawn>) -> (Arc<Runner>, Receiver<Note>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        (
            Arc::new(Runner {
                state: Mutex::new(RunState::default()),
                notes: tx,
                spawn,
            }),
            rx,
        )
    }

    /// The runner of the app: [`launch::spawn_piped`].
    fn real() -> (Arc<Runner>, Receiver<Note>) {
        Runner::new(Box::new(|j: &Job, on_exit: OnExit| {
            launch::spawn_piped(
                Piped {
                    program: &j.program,
                    args: &j.args,
                    cwd: None,
                    env: &j.env,
                    input: j.input.clone(),
                    timeout: j.timeout,
                },
                on_exit,
            )
            .map(drop)
        }))
    }

    /// Starts `j` when there is room, else queues it (or drops it when its queue is full).
    pub(crate) fn submit(self: &Arc<Self>, j: Job) {
        {
            let mut s = self.state.lock();
            let mine = s.running.get(&j.key).copied().unwrap_or(0);
            let busy = s.total >= TOTAL || mine >= PER_HOOK || s.queued.contains_key(&j.key);
            if busy {
                let key = j.key;
                let q = s.queued.entry(key).or_default();
                if q.len() >= QUEUE {
                    warn!(target: SETTINGS, "hook {:?}: {QUEUE} events already waiting; one dropped", j.name);
                    let _ = self.notes.send(Note::Dropped { name: j.name });
                    return;
                }
                q.push_back(j);
                if !s.order.contains(&key) {
                    s.order.push_back(key);
                }
                return;
            }
            s.total += 1;
            *s.running.entry(j.key).or_default() += 1;
        }
        self.start(j);
    }

    /// Starts `j`, its place already taken.
    fn start(self: &Arc<Self>, j: Job) {
        let me = self.clone();
        let (key, name, timeout) = (j.key, j.name.clone(), j.timeout.as_secs());
        let on_exit: OnExit = Box::new(move |r: io::Result<std::process::ExitStatus>| {
            let why = match r {
                Ok(s) if s.success() => None,
                Ok(s) => Some(s.code().map_or(Why::Killed, Why::Exit)),
                Err(e) if e.kind() == io::ErrorKind::TimedOut => Some(Why::Timeout(timeout)),
                Err(e) => Some(Why::Start(e.to_string())),
            };
            me.finished(key, &name, why);
        });
        info!(target: SETTINGS, "hook {:?}: running {:?}", j.name, j.program);
        if let Err(e) = (self.spawn)(&j, on_exit) {
            self.finished(key, &j.name, Some(Why::Start(e.to_string())));
        }
    }

    /// A run of hook `key` ended: its place goes to the next job waiting.
    fn finished(self: &Arc<Self>, key: u64, name: &str, why: Option<Why>) {
        if let Some(why) = why {
            warn!(target: SETTINGS, "hook {name:?} did not work: {why:?}");
            let _ = self.notes.send(Note::Failed {
                name: name.to_owned(),
                why,
            });
        }
        let next = {
            let mut s = self.state.lock();
            s.total = s.total.saturating_sub(1);
            if let Some(n) = s.running.get_mut(&key) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    s.running.remove(&key);
                }
            }
            take_next(&mut s)
        };
        if let Some(j) = next {
            self.start(j);
        }
    }

    /// Runs going now (tests).
    #[cfg(test)]
    fn running(&self) -> usize {
        self.state.lock().total
    }
}

/// The next waiting job that has room, its place taken.
fn take_next(s: &mut RunState) -> Option<Job> {
    if s.total >= TOTAL {
        return None;
    }
    for _ in 0..s.order.len() {
        let key = s.order.pop_front()?;
        let room = s.running.get(&key).copied().unwrap_or(0) < PER_HOOK;
        let job = if room {
            s.queued.get_mut(&key).and_then(VecDeque::pop_front)
        } else {
            None
        };
        let left = s.queued.get(&key).is_some_and(|q| !q.is_empty());
        if left {
            s.order.push_back(key);
        } else {
            s.queued.remove(&key);
        }
        if let Some(j) = job {
            s.total += 1;
            *s.running.entry(key).or_default() += 1;
            return Some(j);
        }
    }
    None
}

/// A hook asked about: its fingerprint, and the runs waiting for the answer.
struct Waiting {
    key: u64,
    hook: Hook,
    jobs: Vec<Job>,
}

/// The window's hooks.
pub(super) struct Hooks {
    list: Vec<Hook>,
    problems: Vec<HookProblem>,
    /// The settings file generation the list was read at.
    read: Option<u64>,
    trusted: Option<HashSet<u64>>,
    /// "Not now" this session.
    declined: HashSet<u64>,
    runner: Arc<Runner>,
    notes: Receiver<Note>,
    /// Hooks waiting for their first answer, in order.
    waiting: Vec<Waiting>,
    /// The one being asked about now.
    asking: Option<u64>,
    /// The hook whose Remove… is being confirmed: its index and name.
    removing: Option<(usize, String)>,
    /// The panel was built from (file generation, language generation, revision).
    built: Option<(u64, u64, u64)>,
    revision: u64,
}

impl Default for Hooks {
    fn default() -> Hooks {
        let (runner, notes) = Runner::real();
        Hooks {
            list: Vec::new(),
            problems: Vec::new(),
            read: None,
            trusted: None,
            declined: HashSet::new(),
            runner,
            notes,
            waiting: Vec::new(),
            asking: None,
            removing: None,
            built: None,
            revision: 0,
        }
    }
}

impl Hooks {
    /// The runs of `h` that may start now: all of them once it is allowed (`trusted`), none
    /// before (they wait for the answer, up to [`QUEUE`]), none after Not now.
    pub(crate) fn admit(&mut self, h: &Hook, jobs: Vec<Job>, trusted: bool) -> Vec<Job> {
        let key = fingerprint(h);
        if self.declined.contains(&key) {
            return Vec::new();
        }
        if trusted {
            return jobs;
        }
        let i = match self.waiting.iter().position(|w| w.key == key) {
            Some(i) => i,
            None => {
                self.waiting.push(Waiting {
                    key,
                    hook: h.clone(),
                    jobs: Vec::new(),
                });
                self.waiting.len() - 1
            }
        };
        let w = &mut self.waiting[i];
        for j in jobs {
            if w.jobs.len() < QUEUE {
                w.jobs.push(j);
            } else {
                warn!(target: SETTINGS, "hook {:?}: {QUEUE} events wait for its first answer; one dropped", h.name);
            }
        }
        Vec::new()
    }

    /// The answer about hook `key`: Allow gives its waiting runs; Not now drops them and
    /// keeps it from running or asking for the rest of the session.
    pub(crate) fn answered(&mut self, key: u64, allow: bool) -> Vec<Job> {
        let i = self.waiting.iter().position(|w| w.key == key);
        let w = i.map(|i| self.waiting.remove(i));
        if allow {
            if let Some(w) = &w {
                info!(target: SETTINGS, "hook {:?} allowed", w.hook.name);
            }
            return w.map(|w| w.jobs).unwrap_or_default();
        }
        self.declined.insert(key);
        Vec::new()
    }

    /// The hook to ask about next.
    pub(crate) fn next_question(&self) -> Option<&Hook> {
        self.waiting.first().map(|w| &w.hook)
    }
}

/// How the hook's events read in the consent question ("photos are marked or …").
fn events_text(h: &Hook) -> String {
    let or = t!("hook-or");
    h.on.iter()
        .map(|e| match e.as_str() {
            "opened" => t!("hook-event-opened"),
            "marked" => t!("hook-event-marked"),
            "keywords" => t!("hook-event-keywords"),
            "exported" => t!("hook-event-exported"),
            "imported" => t!("hook-event-imported"),
            "captured" => t!("hook-event-captured"),
            "develop-saved" => t!("hook-event-develop-saved"),
            _ => t!("hook-event-any"),
        })
        .collect::<Vec<_>>()
        .join(&or)
}

/// The consent question for `h`: title and detail.
pub(super) fn consent_text(h: &Hook) -> (String, String) {
    let args = if h.args.is_empty() {
        "—".to_owned()
    } else {
        h.args.join(" ")
    };
    (
        t!(
            "hook-consent-title",
            name = h.name.clone(),
            program = h.program.clone(),
            events = events_text(h)
        ),
        format!(
            "{}\n{}",
            t!("hook-consent-detail", args = args),
            t!("hook-consent-trust")
        ),
    )
}

/// The hooks' panel: each hook with its switch, what it runs and Remove…; entries that
/// cannot be used say why.
pub(super) fn panel(
    hooks: &[Hook],
    problems: &[HookProblem],
    trusted: &HashSet<u64>,
) -> SettingPanel {
    let mut rows = Vec::new();
    for h in hooks {
        let command = std::iter::once(h.program.clone())
            .chain(h.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        rows.push(PanelRow {
            id: format!("hook:{}", h.index),
            label: h.name.clone(),
            value: t!("hooks-panel-on", events = h.on.join(", ")),
            lines: vec![command],
            detail: if trusted.contains(&fingerprint(h)) {
                t!("hooks-panel-allowed")
            } else {
                t!("hooks-panel-untrusted")
            },
            check: Some(if h.enabled {
                CheckState::On
            } else {
                CheckState::Off
            }),
            button: Some(t!("hooks-panel-remove")),
            ..Default::default()
        });
    }
    // Entries not used at all (a note on a used one is in the log and the settings editor).
    for p in problems
        .iter()
        .filter(|p| p.index > 0 && !hooks.iter().any(|h| h.index == p.index))
    {
        rows.push(PanelRow {
            id: format!("hook:{}", p.index),
            label: format!("#{}", p.index),
            value: t!("hooks-panel-unused"),
            lines: vec![p.reason.clone()],
            attention: true,
            button: Some(t!("hooks-panel-remove")),
            ..Default::default()
        });
    }
    SettingPanel {
        title: t!("hooks-panel-title"),
        intro: if rows.is_empty() {
            t!("hooks-panel-none")
        } else {
            t!("hooks-panel-intro")
        },
        rows,
        keywords: t!("hooks-panel-keywords"),
        ..Default::default()
    }
}

/// A failure in words.
pub(super) fn note_text(n: &Note) -> String {
    match n {
        Note::Failed { name, why } => {
            let reason = match why {
                Why::Exit(c) => t!("hook-failed-exit", code = i64::from(*c)),
                Why::Killed => t!("hook-failed-exit", code = -1i64),
                Why::Timeout(s) => t!("hook-failed-timeout", seconds = *s as i64),
                Why::Start(e) => t!("hook-failed-start", reason = e.clone()),
            };
            t!("hook-failed", name = name.clone(), reason = reason)
        }
        Note::Dropped { name } => t!("hook-dropped", name = name.clone(), count = 1i64),
    }
}

impl App {
    /// The hooks, read again when the settings file changed.
    fn hooks_refresh(&mut self) {
        let g = self.store.file_generation();
        if self.remote.hooks.read == Some(g) {
            return;
        }
        let (list, problems) = self.store.hooks();
        for p in &problems {
            warn!(target: SETTINGS, "{p}");
        }
        let h = &mut self.remote.hooks;
        h.read = Some(g);
        h.list = list;
        h.problems = problems;
        // A hook that changed is asked about afresh.
        let keys: HashSet<u64> = h.list.iter().map(fingerprint).collect();
        h.waiting.retain(|w| keys.contains(&w.key));
        h.revision += 1;
    }

    fn hooks_trust_path(&self) -> PathBuf {
        self.integration_locations().config_dir.join(TRUST_FILE)
    }

    fn hooks_trusted(&mut self) -> &mut HashSet<u64> {
        let path = self.hooks_trust_path();
        self.remote
            .hooks
            .trusted
            .get_or_insert_with(|| super::recipes::read_trust(&path))
    }

    /// Whether any hook would run on `name` done by `source` (none for a hook's own acts,
    /// nor without a window).
    pub(super) fn hooks_listening(&mut self, name: &str, source: &Source) -> bool {
        if self.headless() || source.hook().is_some() {
            return false;
        }
        self.hooks_refresh();
        let h = &self.remote.hooks;
        h.list
            .iter()
            .any(|x| x.enabled && x.listens(name) && !h.declined.contains(&fingerprint(x)))
    }

    /// Runs the hooks of event `name` (its `lines`, one per part).
    pub(super) fn hooks_fire(&mut self, name: &str, lines: &[Line]) {
        let socket = self.remote_socket();
        let hooks: Vec<Hook> = self
            .remote
            .hooks
            .list
            .iter()
            .filter(|h| h.enabled && h.listens(name))
            .cloned()
            .collect();
        for h in hooks {
            let key = fingerprint(&h);
            if self.remote.hooks.declined.contains(&key) {
                continue;
            }
            let jobs: Vec<Job> = lines
                .iter()
                .filter(|l| passes(&h, &l.value))
                .filter_map(|l| match job(&h, l, socket.as_deref()) {
                    Ok(j) => Some(j),
                    Err(e) => {
                        warn!(target: SETTINGS, "hook {:?}: {e}", h.name);
                        None
                    }
                })
                .collect();
            if jobs.is_empty() {
                continue;
            }
            let trusted = self.hooks_trusted().contains(&key);
            let hs = &mut self.remote.hooks;
            for j in hs.admit(&h, jobs, trusted) {
                hs.runner.submit(j);
            }
        }
        self.hooks_ask();
    }

    /// Asks about the next hook waiting, when nothing else is on screen.
    fn hooks_ask(&mut self) {
        if self.remote.hooks.asking.is_some() || self.chrome.modal().is_some() {
            return;
        }
        let Some(hook) = self.remote.hooks.next_question().cloned() else {
            return;
        };
        let key = fingerprint(&hook);
        let (title, detail) = consent_text(&hook);
        self.remote.hooks.asking = Some(key);
        self.chrome.confirm_choice(
            title,
            detail,
            t!("hook-consent-allow"),
            ALLOW_ITEM,
            t!("hook-consent-not-now"),
            NOT_NOW_ITEM,
        );
        self.redraw = true;
    }

    /// Allow: the hook is trusted from now on, and its waiting runs start.
    pub(super) fn hooks_allowed(&mut self) {
        let Some(key) = self.remote.hooks.asking.take() else {
            return;
        };
        let path = self.hooks_trust_path();
        let trusted = self.hooks_trusted();
        trusted.insert(key);
        let all = trusted.clone();
        if let Err(e) = super::recipes::write_trust(&path, &all) {
            warn!(target: SETTINGS, "hooks: could not remember the answer: {e}");
        }
        let hs = &mut self.remote.hooks;
        for j in hs.answered(key, true) {
            hs.runner.submit(j);
        }
        hs.revision += 1;
        self.hooks_ask();
    }

    /// Not now (or the question closed): this session it does not run or ask again.
    pub(super) fn hooks_declined(&mut self) {
        let Some(key) = self.remote.hooks.asking.take() else {
            return;
        };
        self.remote.hooks.answered(key, false);
        self.hooks_ask();
    }

    /// Once per turn: notes from runs, a question closed without an answer, the next
    /// question, and the panel in Settings.
    pub(super) fn hooks_tick(&mut self) {
        let notes: Vec<Note> = self.remote.hooks.notes.try_iter().collect();
        let mut dropped: Vec<String> = Vec::new();
        for n in notes {
            match &n {
                // One note per hook and turn for skipped events.
                Note::Dropped { name } if dropped.contains(name) => continue,
                Note::Dropped { name } => dropped.push(name.clone()),
                Note::Failed { .. } => {}
            }
            let text = note_text(&n);
            let now = self.now();
            self.chrome.toast(text, None, now);
            self.redraw = true;
        }
        if self.remote.hooks.asking.is_some()
            && self.chrome.modal() != Some(specular_widgets::Modal::ConfirmTrash)
        {
            self.hooks_declined();
        }
        if !self.remote.hooks.waiting.is_empty() {
            self.hooks_ask();
        }
        if self.chrome.modal() == Some(specular_widgets::Modal::Settings) {
            self.hooks_refresh();
            let hs = &self.remote.hooks;
            let built = (
                self.store.file_generation(),
                I18n::global().generation(),
                hs.revision,
            );
            if hs.built != Some(built) {
                let trusted = self.hooks_trusted().clone();
                let hs = &mut self.remote.hooks;
                hs.built = Some(built);
                let p = panel(&hs.list, &hs.problems, &trusted);
                if self.chrome.set_setting_panel(UNDER, Some(p)) {
                    self.redraw = true;
                }
            }
        }
    }

    /// The panel's switch (on/off, written to settings.toml) and Remove… (asks first).
    pub(super) fn hooks_panel_event(&mut self, e: &PanelEvent) {
        let (PanelEvent::Check(id) | PanelEvent::Row(id)) = e else {
            return;
        };
        let Some(index) = id
            .strip_prefix("hook:")
            .and_then(|i| i.parse::<usize>().ok())
        else {
            return;
        };
        self.hooks_refresh();
        let hook = self
            .remote
            .hooks
            .list
            .iter()
            .find(|h| h.index == index)
            .cloned();
        match e {
            PanelEvent::Check(_) => {
                let Some(h) = hook else { return };
                let text = self.store.file_text();
                let r = specular_settings::commands::set_hook_enabled(&text, index, !h.enabled)
                    .and_then(|t| self.store.write_text(&t).map_err(|e| e.to_string()));
                if let Err(e) = r {
                    let now = self.now();
                    self.chrome
                        .toast(t!("hook-settings-unwritable", reason = e), None, now);
                }
                self.redraw = true;
            }
            _ => {
                let name = hook.map_or_else(|| format!("#{index}"), |h| h.name);
                self.chrome.confirm_destructive(
                    t!("hook-remove-title", name = name.clone()),
                    t!("hook-remove-detail"),
                    t!("hook-remove-ok"),
                    REMOVE_ITEM,
                );
                self.remote.hooks.removing = Some((index, name));
                self.redraw = true;
            }
        }
    }

    /// Remove… was confirmed: the entry leaves settings.toml.
    pub(super) fn hooks_remove_confirmed(&mut self) {
        let Some((index, name)) = self.remote.hooks.removing.take() else {
            return;
        };
        let text = self.store.file_text();
        let r = specular_settings::commands::remove_hook(&text, index)
            .and_then(|t| self.store.write_text(&t).map_err(|e| e.to_string()));
        let now = self.now();
        let msg = match r {
            Ok(_) => {
                info!(target: SETTINGS, "hooks: removed {name:?}");
                t!("hook-removed", name = name)
            }
            Err(e) => t!("hook-settings-unwritable", reason = e),
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }
}

#[cfg(test)]
mod shots;
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests;

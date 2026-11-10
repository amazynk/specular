//! Crash reports, the diagnostics bundle and the update check, on the app's side.
//!
//! - The crash report written by `specular_core::isolate` gets this machine's context (version,
//!   OS, GPU) and is redacted ([`install_report_hooks`]); the next start offers it
//!   ([`App::crash_prompt`]).
//! - Save diagnostics bundle… previews exactly what the zip will hold, then writes it where
//!   the user chooses ([`App::diag_open`]). Nothing is uploaded.
//! - The update check is off unless Settings › Updates turns it on: at most once a day, one
//!   plain GET of [`specular_diag::RELEASES_URL`]; the answer shows under the setting, with
//!   the release notes and the download page opening in the browser. Nothing is installed.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crossbeam_channel::Receiver;
use specular_diag::bundle::{self, Inputs, What};
use specular_diag::update::{self, Channel, Outcome, Release, Version};
use specular_diag::{ISSUES_URL, RELEASES_URL, Redactor, SUPPORT_EMAIL};
use specular_i18n::{I18n, t};
use specular_settings::UpdateChannel;
use specular_widgets::{CrashSheet, DiagEntry, DiagSheet, PanelEvent, PanelRow, SettingPanel};

use crate::app::App;

/// The setting the update panel sits under.
pub(super) const UPDATE_PANEL: &str = "updates.check_now";

/// The GPU as the crash report and the bundle name it (set once the device is up).
static GPU_LINE: Mutex<String> = Mutex::new(String::new());

/// The redactor of crash reports (this computer's home folder and user name; file names
/// hidden).
fn report_redactor() -> &'static Redactor {
    static R: OnceLock<Redactor> = OnceLock::new();
    R.get_or_init(Redactor::from_env)
}

fn gpu_line() -> String {
    GPU_LINE.lock().map(|g| g.clone()).unwrap_or_default()
}

/// The system lines every report and bundle starts with.
fn system_lines() -> String {
    let b = specular_help::build_info();
    let mut s = format!(
        "version:  {} ({} {})\nos:       {} {}\n",
        b.version,
        if b.commit.is_empty() { "-" } else { b.commit },
        b.profile,
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let gpu = gpu_line();
    if !gpu.is_empty() {
        s.push_str(&format!("gpu:      {gpu}\n"));
    }
    s
}

/// Crash reports get the system lines and are redacted (main, after `isolate::install`).
pub(crate) fn install_report_hooks() {
    specular_core::isolate::set_report_hooks(specular_core::isolate::ReportHooks {
        context: Box::new(system_lines),
        redact: Box::new(|s| report_redactor().redact(s)),
    });
}

/// Records the GPU for reports: "AMD Radeon RX 7800 XT, Vulkan, Mesa 25.2.1".
pub(crate) fn set_gpu(name: &str, backend: &str, driver: &str) {
    if let Ok(mut g) = GPU_LINE.lock() {
        *g = format!("{name}, {backend}, {driver}");
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Where the time of the last update check is kept.
fn last_check_file() -> PathBuf {
    specular_settings::Locations::detect()
        .data_dir
        .join("last-update-check")
}

/// The app's state for this module.
#[derive(Default)]
pub(crate) struct Diag {
    /// A save of the bundle under way: where it went, or why it failed.
    save: Option<Receiver<Result<PathBuf, String>>>,
    /// An update check under way.
    check: Option<Receiver<Result<Outcome, String>>>,
    /// The release offered (its links).
    release: Option<Release>,
}

impl App {
    /// After a crash: the prompt with the report (already redacted when it was written).
    pub(crate) fn crash_prompt(&mut self, report: &Path) {
        let text = std::fs::read_to_string(report)
            .map(|t| report_redactor().redact(&t))
            .unwrap_or_else(|e| format!("{}: {e}", report.display()));
        self.chrome.show_crash(CrashSheet { report: text });
    }

    /// What goes into the bundle, before redaction.
    fn diag_inputs(&self) -> Inputs {
        let i18n = I18n::global();
        let mut summary = system_lines();
        summary.push_str(&format!("language: {}\n", i18n.language()));
        if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
            summary.push_str(&format!("open:     {}\n", p.display()));
        }
        summary.push_str(&format!("images:   {} in the folder\n", self.entries.len()));
        let crash_dir = specular_settings::Locations::detect().crash_dir();
        let crashes = specular_core::isolate::reports(&crash_dir)
            .into_iter()
            .rev()
            .take(5)
            .rev()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().into_owned();
                Some((name, std::fs::read_to_string(&p).ok()?))
            })
            .collect();
        Inputs {
            readme: i18n.tr("diag-readme"),
            summary,
            gpu: self.hw_report().text(),
            crashes,
            log: specular_log::recent_lines(specular_core::isolate::REPORT_LOG_LINES),
            settings: Some(self.store.file_text()),
        }
    }

    /// The bundle's files, redacted (`names`: file names kept).
    fn diag_entries(&self, names: bool) -> Vec<bundle::Entry> {
        let mut known: Vec<String> = Vec::new();
        if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
            known.push(p.display().to_string());
            if let Some(d) = p.parent() {
                known.push(d.display().to_string());
            }
        }
        let r = Redactor::from_env()
            .hide_file_names(!names)
            .known_paths(known);
        bundle::entries(&self.diag_inputs(), &r)
    }

    fn diag_sheet(&self, names: bool) -> DiagSheet {
        let i18n = I18n::global();
        let entries = self
            .diag_entries(names)
            .into_iter()
            .map(|e| {
                let description = match e.what {
                    What::Crashes(n) => t!("diag-entry-crashes", count = n as i64),
                    What::Log(n) => t!("diag-entry-log", count = n as i64),
                    w => i18n.tr(w.message_id()),
                };
                DiagEntry {
                    name: e.name,
                    description,
                    text: e.text,
                }
            })
            .collect();
        DiagSheet {
            entries,
            include_names: names,
            issues: ISSUES_URL.to_owned(),
            email: SUPPORT_EMAIL.to_owned(),
        }
    }

    /// Save diagnostics bundle…: the preview.
    pub(crate) fn diag_open(&mut self) {
        let sheet = self.diag_sheet(false);
        self.chrome.show_diagnostics(sheet);
        self.redraw = true;
    }

    /// "Include file names" changed.
    pub(crate) fn diag_names(&mut self, names: bool) {
        let sheet = self.diag_sheet(names);
        self.chrome.set_diagnostics(sheet);
        self.redraw = true;
    }

    /// Save…: asks where, then writes the bundle as previewed.
    pub(crate) fn diag_save(&mut self, names: bool) {
        let entries = self.diag_entries(names);
        let unix = now_unix();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-diag", move |l| {
            let Some(p) = rfd::FileDialog::new()
                .set_file_name(bundle::suggested_name(unix))
                .add_filter("Zip", &["zip"])
                .save_file()
            else {
                return;
            };
            let r = bundle::save(&entries, unix, &p)
                .map(|()| p)
                .map_err(|e| e.to_string());
            let _ = tx.send(r);
            l.waker()();
        });
        self.diag.save = Some(rx);
    }

    /// Runs an update check when one is due (`manual`: Check now).
    pub(crate) fn update_check(&mut self, manual: bool) {
        if self.diag.check.is_some() || self.args.screenshot.is_some() {
            return;
        }
        // No network: automatic checks wait; Check now says why it can't.
        if specular_core::net::off::is_off() {
            if manual {
                self.update_panel(Some(Err(super::privacy::off_text())));
            }
            return;
        }
        let u = &self.settings.updates;
        let file = last_check_file();
        let now = now_unix();
        if !manual && !update::due(u.check, update::load_last(&file), now) {
            return;
        }
        let channel = match u.channel {
            UpdateChannel::Stable => Channel::Stable,
            UpdateChannel::Beta => Channel::Beta,
        };
        let Some(current) = Version::parse(specular_diag::VERSION) else {
            return;
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("net-update", move |l| {
            let r = update::client().and_then(|c| {
                update::check(
                    &c,
                    RELEASES_URL,
                    &current,
                    channel,
                    &specular_core::CancelToken::new(),
                )
            });
            let _ = update::save_last(&file, now);
            let _ = tx.send(r);
            l.waker()();
        });
        self.diag.check = Some(rx);
        self.update_panel(Some(Err(t!("update-checking"))));
    }

    /// The panel under Check now: the release found (its notes and download), up to date,
    /// checking or failed.
    fn update_panel(&mut self, state: Option<Result<&Outcome, String>>) {
        let panel = state.map(|s| match s {
            Ok(Outcome::Available(r)) => SettingPanel {
                title: t!("update-available", version = r.version_text.clone()),
                intro: r.date.clone(),
                rows: vec![
                    PanelRow {
                        id: "notes".into(),
                        label: t!("update-notes"),
                        lines: vec![r.notes_url.clone()],
                        button: Some(t!("update-notes")),
                        ..PanelRow::default()
                    },
                    PanelRow {
                        id: "download".into(),
                        label: t!("update-download"),
                        lines: vec![r.download_url.clone()],
                        button: Some(t!("update-download")),
                        ..PanelRow::default()
                    },
                ],
                ..SettingPanel::default()
            },
            Ok(Outcome::Current) => SettingPanel {
                title: t!("update-none"),
                ..SettingPanel::default()
            },
            Err(e) => SettingPanel {
                title: e,
                ..SettingPanel::default()
            },
        });
        if self.chrome.set_setting_panel(UPDATE_PANEL, panel) {
            self.redraw = true;
        }
    }

    /// A button of the update panel: open the link in the browser.
    pub(crate) fn update_panel_event(&mut self, e: &PanelEvent) {
        let Some(r) = &self.diag.release else {
            return;
        };
        let url = match e {
            PanelEvent::Row(id) if id == "notes" => r.notes_url.clone(),
            PanelEvent::Row(id) if id == "download" => r.download_url.clone(),
            _ => return,
        };
        super::ai::open_url(url);
    }

    /// Answers of the background work: the bundle saved, the update check done.
    pub(crate) fn diag_tick(&mut self) {
        if let Some(rx) = &self.diag.save
            && let Ok(r) = rx.try_recv()
        {
            self.diag.save = None;
            let now = self.now();
            match r {
                Ok(p) => {
                    self.chrome
                        .toast(t!("diag-saved", path = p.display().to_string()), None, now);
                }
                Err(e) => {
                    self.chrome.warning(t!("diag-failed", reason = e), now);
                }
            }
            self.redraw = true;
        }
        if let Some(rx) = &self.diag.check
            && let Ok(r) = rx.try_recv()
        {
            self.diag.check = None;
            match r {
                Ok(o) => {
                    self.update_panel(Some(Ok(&o)));
                    if let Outcome::Available(rel) = &o {
                        let now = self.now();
                        self.chrome.toast(
                            t!("update-available", version = rel.version_text.clone()),
                            None,
                            now,
                        );
                    }
                    self.diag.release = match o {
                        Outcome::Available(r) => Some(r),
                        Outcome::Current => None,
                    };
                }
                Err(e) => self.update_panel(Some(Err(t!("update-failed", reason = e)))),
            }
        }
    }
}

//! Deep desktop integration in the viewer (DESIGN.md §11): Settings → Integration, the
//! D-Bus single instance (`org.freedesktop.Application`), "Set as wallpaper" in the palette,
//! the desktop's recent-files list, and the Library searches the desktop search asks for.
//!
//! Everything that touches files or the bus runs on background threads; results come back
//! through this module's channel, drained from `drain_integration`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use specular_i18n::{I18n, t};
use specular_log::{SETTINGS, info};
use specular_platform::integration::{self as int, Env, OptionId, State, Status, Unavailable};
use specular_widgets::{Modal, SettingStatus};

use crate::app::App;

mod extra_types;

pub(super) const EXTRA_TYPES_UNDER: &str = extra_types::UNDER;

/// How often the Integration page re-reads the state while the settings are open.
const REFRESH: Duration = Duration::from_secs(3);

enum Msg {
    Statuses(Vec<Status>),
    Done {
        id: OptionId,
        on: bool,
        result: Result<Status, String>,
    },
    Wallpaper(Result<&'static str, String>),
    #[cfg(all(unix, not(target_os = "macos")))]
    Request(int::linux::dbus::Request),
}

pub(in crate::app) struct Desktop {
    env: Env,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    statuses: Vec<Status>,
    busy: Option<OptionId>,
    refreshed: Option<Instant>,
    refreshing: bool,
    started: bool,
    wallpaper: bool,
    noted: Option<PathBuf>,
    /// Removing the desktop search was confirmed: the next removal goes ahead.
    remove_confirmed: bool,
    /// The recent-files option is on: add viewed files to the desktop's recent list.
    recent_on: bool,
    /// A slideshow asked for (`--slideshow`, the launcher menu) waits for its folder: since
    /// when, and whether a file (not a folder) was given.
    slideshow_wait: Option<(Instant, bool)>,
    /// UI generation the state texts were made at.
    i18n_gen: Option<u64>,
    dirty: bool,
    /// "More file types" (extra_types.rs): the setting `env` was given, the open rows,
    /// what the panel was built from.
    extras_setting: String,
    /// The stored setting last seen (changes from outside the panel).
    extras_seen: String,
    extras_open: bool,
    groups_open: Vec<int::mime::ExtraGroup>,
    extras_built: Option<extra_types::Built>,
    #[cfg(all(unix, not(target_os = "macos")))]
    service: Option<int::linux::dbus::Service>,
}

impl Desktop {
    pub(in crate::app) fn new() -> Desktop {
        let (tx, rx) = crossbeam_channel::unbounded();
        Desktop {
            env: Env::detect(),
            tx,
            rx,
            statuses: Vec::new(),
            busy: None,
            refreshed: None,
            refreshing: false,
            started: false,
            wallpaper: false,
            noted: None,
            remove_confirmed: false,
            recent_on: false,
            slideshow_wait: None,
            i18n_gen: None,
            dirty: false,
            extras_setting: String::new(),
            extras_seen: String::new(),
            extras_open: false,
            groups_open: Vec::new(),
            extras_built: None,
            #[cfg(all(unix, not(target_os = "macos")))]
            service: None,
        }
    }
}

fn option_name(id: OptionId) -> String {
    specular_settings::find(id.setting_key())
        .map_or_else(|| id.key().to_owned(), |m| m.label(I18n::global()))
}

/// The state line of an option.
fn state_text(s: &Status) -> String {
    let state = match s.state {
        State::On => t!("integration-state-on"),
        State::Off => t!("integration-state-off"),
        State::Partial => t!("integration-state-partial"),
        State::Unavailable(Unavailable::Sandboxed) => t!("integration-state-sandboxed"),
        State::Unavailable(Unavailable::NoSessionBus) => t!("integration-state-no-bus"),
        State::Unavailable(Unavailable::NoFileManager) => {
            t!("integration-state-no-file-manager")
        }
        State::Unavailable(Unavailable::NoSearch) => t!("integration-state-no-search"),
        State::Unavailable(_) => t!("integration-state-unavailable"),
    };
    if !s.is_applied() || s.targets.is_empty() {
        return state;
    }
    let details = match s.id {
        OptionId::DefaultViewer => match s.targets[0].split_once('/') {
            Some((n, total)) => t!(
                "integration-detail-types",
                count = n.to_owned(),
                total = total.to_owned()
            ),
            None => s.targets.join(", "),
        },
        OptionId::Thumbnailer => t!(
            "integration-detail-thumbnail-types",
            count = s.targets[0].clone()
        ),
        OptionId::RecentFiles => t!("integration-detail-recent", count = s.targets[0].clone()),
        _ => s.targets.join(", "),
    };
    t!(
        "integration-state-details",
        state = state,
        details = details
    )
}

fn status_row(s: &Status, busy: bool) -> SettingStatus {
    if busy {
        return SettingStatus {
            state: t!("integration-state-working"),
            attention: false,
            button: None,
        };
    }
    let button = match s.state {
        State::Unavailable(_) => None,
        State::On | State::Partial => Some(t!("integration-remove")),
        State::Off => Some(t!("integration-apply")),
    };
    SettingStatus {
        state: state_text(s),
        attention: s.state == State::Partial,
        button,
    }
}

impl App {
    /// Once per event-loop turn (from `drain_integration`).
    pub(super) fn drain_desktop(&mut self) {
        if !self.integration.desktop.started {
            self.integration.desktop.started = true;
            self.start_desktop();
        }
        while let Ok(m) = self.integration.desktop.rx.try_recv() {
            self.on_desktop(m);
        }
        self.drain_extra_types();
        let d = &mut self.integration.desktop;
        let settings_open = self.chrome.modal() == Some(Modal::Settings);
        if settings_open
            && !d.refreshing
            && d.busy.is_none()
            && d.refreshed.is_none_or(|t| t.elapsed() >= REFRESH)
        {
            self.refresh_desktop();
        }
        let gen_ = I18n::global().generation();
        let d = &mut self.integration.desktop;
        if d.dirty || d.i18n_gen != Some(gen_) {
            d.dirty = false;
            d.i18n_gen = Some(gen_);
            let map: HashMap<&'static str, SettingStatus> = d
                .statuses
                .iter()
                .map(|s| (s.id.setting_key(), status_row(s, d.busy == Some(s.id))))
                .collect();
            if self.chrome.set_setting_status(map) {
                self.redraw = true;
            }
        }
        self.note_recent();
        self.slideshow_when_listed();
    }

    /// First turn: the single-instance service, files from the command line, the launcher
    /// actions (`--library`, `--slideshow`), what the palette offers.
    fn start_desktop(&mut self) {
        let files = crate::integration_cli::take_startup_files();
        let many = files.len() > 1;
        if many {
            self.open_pasted(files);
        }
        match self.args.library_search.take() {
            Some(q) => self.library_search_from_desktop(q),
            None if self.args.library => self.library_from_desktop(),
            None => {}
        }
        let given = many || self.args.library || self.args.slideshow.is_some();
        if let Some(p) = self.args.slideshow.take() {
            self.desktop_slideshow(p);
        }
        // Started without anything to show: the start screen, the last folder's gallery or
        // the Library (`general.on_start`).
        self.welcome_on_start(given);
        let d = &mut self.integration.desktop;
        d.wallpaper = int::wallpaper_available(&d.env);
        // Rebuild the palette items with "Set as wallpaper".
        self.integration.i18n_gen = None;
        self.refresh_desktop();
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use int::linux::dbus;
            let single = self.store.snapshot().general.single_instance;
            let by_bus = crate::integration_cli::service_launch();
            let env = &self.integration.desktop.env;
            if by_bus || (single && dbus::enabled(env) && dbus::session_bus_exists()) {
                let tx = self.integration.desktop.tx.clone();
                let wake = self.loader.waker();
                match dbus::serve(move |r| {
                    let _ = tx.send(Msg::Request(r));
                    wake();
                }) {
                    Ok(s) => self.integration.desktop.service = s,
                    Err(e) => specular_log::warn!(target: SETTINGS, "single instance: {e}"),
                }
            }
        }
    }

    /// Re-reads every option's state in the background.
    fn refresh_desktop(&mut self) {
        let d = &mut self.integration.desktop;
        if int::options().is_empty() {
            d.refreshed = Some(Instant::now());
            return;
        }
        d.refreshing = true;
        let env = d.env.clone();
        let tx = d.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("int-status", move |_| {
            let v = int::options()
                .iter()
                .map(|id| int::status(&env, *id))
                .collect();
            let _ = tx.send(Msg::Statuses(v));
            wake();
        });
    }

    fn on_desktop(&mut self, m: Msg) {
        let d = &mut self.integration.desktop;
        match m {
            Msg::Statuses(v) => {
                d.refreshing = false;
                d.refreshed = Some(Instant::now());
                d.recent_on = v
                    .iter()
                    .any(|s| s.id == OptionId::RecentFiles && s.is_applied());
                if v != d.statuses {
                    d.statuses = v;
                    d.dirty = true;
                }
            }
            Msg::Done { id, on, result } => {
                d.busy = None;
                d.dirty = true;
                match result {
                    Ok(s) => {
                        if let Some(old) = d.statuses.iter_mut().find(|o| o.id == id) {
                            *old = s;
                        }
                        let option = option_name(id);
                        let text = if on {
                            t!("integration-done-on", option = option)
                        } else {
                            t!("integration-done-off", option = option)
                        };
                        self.integration_toast(text);
                    }
                    Err(reason) => {
                        let option = option_name(id);
                        self.integration_toast(t!(
                            "integration-failed",
                            option = option,
                            reason = reason
                        ));
                    }
                }
                self.integration.desktop.refreshed = None;
            }
            Msg::Wallpaper(Ok(method)) => {
                self.integration_toast(t!("wallpaper-set", method = method.to_owned()));
            }
            Msg::Wallpaper(Err(reason)) => {
                self.integration_toast(t!("wallpaper-failed", reason = reason));
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            Msg::Request(r) => self.on_bus_request(r),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn on_bus_request(&mut self, r: int::linux::dbus::Request) {
        use int::linux::dbus::Request;
        info!(target: SETTINGS, "single instance: {r:?}");
        if let Some(g) = &self.gfx {
            if g.window.is_minimized() == Some(true) {
                g.window.set_minimized(false);
            }
            g.window.focus_window();
        }
        match r {
            Request::Open { files, .. } if !files.is_empty() => self.open_pasted(files),
            Request::Action {
                action,
                folder,
                query,
                ..
            } => {
                use int::linux::DesktopAction;
                match action {
                    DesktopAction::NewWindow => {
                        let r = std::env::current_exe().and_then(|exe| {
                            specular_platform::launch::spawn(
                                exe.as_os_str(),
                                &["--new-window".into()],
                                None,
                                None,
                            )
                        });
                        if let Err(e) = r {
                            specular_log::warn!(target: SETTINGS, "new window: {e}");
                        }
                    }
                    DesktopAction::Library => match query {
                        Some(q) => self.library_search_from_desktop(q),
                        None => self.library_from_desktop(),
                    },
                    DesktopAction::Slideshow => {
                        let p = folder.unwrap_or_else(int::user_pictures_dir);
                        self.desktop_slideshow(p);
                    }
                }
            }
            _ => {}
        }
        self.redraw = true;
    }

    /// A slideshow of `p`'s folder (`p` itself when it is a folder, else starting at `p`):
    /// opens it in the viewer and starts the show once it is listed.
    pub(in crate::app) fn desktop_slideshow(&mut self, p: PathBuf) {
        info!(target: SETTINGS, "slideshow of {}", p.display());
        if self.slideshow.running() {
            self.slideshow_action(specular_input::Action::Slideshow);
        }
        if self.present_show_running() {
            self.present_stop();
        }
        self.gallery_dismiss();
        let file = !p.is_dir();
        if file {
            self.open_target(p);
        } else {
            self.set_entries(Vec::new());
            self.current = None;
            self.scan_folder(p);
        }
        self.integration.desktop.slideshow_wait = Some((Instant::now(), file));
        self.redraw = true;
    }

    /// Starts the slideshow [`desktop_slideshow`](Self::desktop_slideshow) asked for once
    /// the folder's listing is in (a folder: its first image is current; a file: the rest of
    /// its folder arrived, or a moment passed).
    fn slideshow_when_listed(&mut self) {
        let Some((since, file)) = self.integration.desktop.slideshow_wait else {
            return;
        };
        let listed = self.current.is_some()
            && (!file || self.entries.len() > 1 || since.elapsed() >= Duration::from_secs(3));
        if listed {
            self.integration.desktop.slideshow_wait = None;
            if !self.slideshow.running() && !self.present_show_running() {
                self.slideshow_action(specular_input::Action::Slideshow);
            }
        } else if since.elapsed() >= Duration::from_secs(30) {
            // An empty or unreadable folder: give up quietly (the viewer shows why).
            self.integration.desktop.slideshow_wait = None;
        }
    }

    /// An `integration.*` button in Settings → Integration: apply or remove the option.
    pub(in crate::app) fn desktop_setting(&mut self, key: &str) {
        // General → "File associations…": the default-viewer option lives on the Integration page.
        if key == "general.file_associations" {
            if !int::options().contains(&OptionId::DefaultViewer) {
                info!(target: SETTINGS, "file associations are not available on this platform yet");
            } else {
                self.chrome.open_settings_at("integration");
                self.redraw = true;
            }
            return;
        }
        let Some(id) = OptionId::from_setting_key(key) else {
            return;
        };
        let d = &mut self.integration.desktop;
        if d.busy.is_some() {
            return;
        }
        let status = d.statuses.iter().find(|s| s.id == id);
        let on = !status.is_some_and(Status::is_applied);
        // Desktop search: removing it asks first, naming what goes.
        if !on && id == OptionId::SearchProvider && !std::mem::take(&mut d.remove_confirmed) {
            let targets = status.map(|s| s.targets.join(", ")).unwrap_or_default();
            self.chrome.confirm_destructive(
                t!("integration-search-remove-ask"),
                t!("integration-search-remove-detail", desktops = targets),
                t!("integration-remove"),
                crate::app::items::INTEGRATION_SEARCH_REMOVE,
            );
            self.redraw = true;
            return;
        }
        d.busy = Some(id);
        d.dirty = true;
        let env = d.env.clone();
        let tx = d.tx.clone();
        let labels = crate::integration_cli::labels();
        let wake = self.loader.waker();
        info!(target: SETTINGS, "integration: {} {}", if on { "apply" } else { "remove" }, id.key());
        self.loader.background("int-apply", move |_| {
            let r = if on {
                int::apply(&env, id, &labels)
            } else {
                int::remove(&env, id)
            };
            let _ = tx.send(Msg::Done {
                id,
                on,
                result: r.map_err(|e| crate::error_text::reason(&e)),
            });
            wake();
        });
        self.redraw = true;
    }

    /// "Turn off desktop search?" was confirmed.
    pub(in crate::app) fn desktop_search_remove_confirmed(&mut self) {
        self.integration.desktop.remove_confirmed = true;
        self.desktop_setting(OptionId::SearchProvider.setting_key());
    }

    /// Whether "Set as wallpaper" can work on this desktop (the action's enabled state).
    pub(in crate::app) fn wallpaper_available(&self) -> bool {
        self.integration.desktop.wallpaper
    }

    /// "Set as wallpaper" for the current file.
    pub(in crate::app) fn set_wallpaper(&mut self) {
        let Some(file) = self.integration_file() else {
            return;
        };
        let env = self.integration.desktop.env.clone();
        let tx = self.integration.desktop.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("int-wallpaper", move |_| {
            let r = int::set_wallpaper(&env, &file)
                .map(|b| b.name())
                .map_err(|e| crate::error_text::reason(&e));
            let _ = tx.send(Msg::Wallpaper(r));
            wake();
        });
    }

    /// Keeps the desktop's recently-used files (only while its option is on).
    fn note_recent(&mut self) {
        let recent = self.integration.desktop.recent_on;
        if !recent {
            return;
        }
        let Some(file) = self.integration_file() else {
            return;
        };
        let d = &mut self.integration.desktop;
        if d.noted.as_ref() == Some(&file) {
            return;
        }
        d.noted = Some(file.clone());
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let env = d.env.clone();
            self.loader.background("int-recent", move |_| {
                if let Err(e) = int::linux::recent::note(&env, &file, int::mime::of_path(&file)) {
                    specular_log::warn!(target: SETTINGS, "recent files: {e}");
                }
            });
        }
        #[cfg(windows)]
        {
            let env = d.env.clone();
            self.loader.background("int-recent", move |_| {
                if let Err(e) = int::windows::recent::note(&env, &file) {
                    specular_log::warn!(target: SETTINGS, "recent files: {e}");
                }
            });
        }
        #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
        let _ = file;
    }
}

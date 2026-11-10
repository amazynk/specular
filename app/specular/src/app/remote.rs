//! The window's side of remote control (`crate::remote`): requests from the socket answered
//! once per event-loop turn, `specular://` links (with their confirmation), and
//! `--print-marked`. Version 3's `select`, `mark`, `apply-style` and `export` are answered
//! here too; what they do is told as events ([`super::events`]) marked as the client's.
//!
//! "Marked" means picked: the pick flag of the culling marks (`P` in the gallery; Lightroom's
//! "flagged"). Stars, colour labels and rejects do not make a file marked. Marks live in the
//! gallery's tables in the config folder, so a pick made in an earlier session counts.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_i18n::t;
use specular_input::Action;
use specular_log::{SETTINGS, info, warn};
use specular_platform::url_scheme::{self, Reason, Target};
use specular_render::ZoomMode;

use super::App;
use super::events::Source;
use crate::remote::protocol::{Code, Fail, Flag, MarkSet, Nav, Reply, Request, Window, path_value};
use crate::remote::server::{Options, Server};

/// The palette/confirm item that opens the files of a confirmed `specular://` link.
pub(super) const CONFIRM_ITEM: u64 = super::items::REMOTE_CONFIRM;

/// Calls answered per event-loop turn (the rest wait for the next turn).
const PER_TURN: usize = 32;

#[derive(Default)]
pub(super) struct Remote {
    server: Option<Server>,
    /// `specular://` links from the command line, opened on the first turn.
    urls: Vec<String>,
    /// The files of the link waiting for the user's confirmation.
    confirm: Option<Vec<PathBuf>>,
    /// Protocol version 2's state: overlays, a pending view, a screenshot (`script.rs`).
    pub(super) script: super::script::Script,
    /// Version 3: the events and who is acting (`events.rs`).
    pub(super) events: super::events::Events,
    /// The hooks of settings.toml (`hooks.rs`).
    pub(super) hooks: super::hooks::Hooks,
    /// Test command `idle` calls waiting for the window to settle (debug builds).
    #[cfg(debug_assertions)]
    pub(super) idle: Vec<super::test_hooks::IdleWait>,
}

impl Remote {
    pub(super) fn new(urls: Vec<String>) -> Remote {
        Remote {
            urls,
            ..Remote::default()
        }
    }
}

impl App {
    /// Starts listening (not for benchmarks and screenshots, nor with
    /// `integration.ipc` off).
    pub(super) fn start_remote(&mut self) {
        if self.headless() {
            return;
        }
        if !self.settings.integration.ipc {
            info!(target: SETTINGS, "remote control: off (integration.ipc)");
            return;
        }
        let o = Options {
            bus: self.remote.events.bus.clone(),
            ..Options::default()
        };
        let r = crate::remote::socket_dir()
            .and_then(|dir| Server::start_opts(&dir, self.loader.waker(), o));
        match r {
            Ok(s) => self.remote.server = Some(s),
            Err(e) => warn!(target: SETTINGS, "remote control unavailable: {e}"),
        }
    }

    /// Once per event-loop turn: links from the command line, then requests.
    pub(super) fn drain_remote(&mut self, el: &ActiveEventLoop) {
        for url in std::mem::take(&mut self.remote.urls) {
            let _ = self.open_url(&url);
        }
        self.script_apply_pending();
        if let Some(call) = self.remote.script.screenshot_overdue(Instant::now()) {
            call.answer(Err(Fail::new(Code::Mode, super::script::NO_FRAME)));
        }
        self.recipes_poll();
        self.events_tick();
        self.hooks_tick();
        #[cfg(debug_assertions)]
        self.test_idle_tick();
        for _ in 0..PER_TURN {
            let Some(call) = self
                .remote
                .server
                .as_ref()
                .and_then(|s| s.calls.try_recv().ok())
            else {
                return;
            };
            if call.req == Request::Screenshot {
                self.script_screenshot(call);
                continue;
            }
            #[cfg(debug_assertions)]
            if matches!(call.req, Request::Idle { .. }) {
                self.test_idle_wait(call);
                continue;
            }
            let quit = call.req == Request::Quit;
            // What the request does is told as the client's (or its hook's).
            self.remote.events.source = Source::of(&call.origin);
            let reply = self.remote_call(call.req.clone(), el);
            self.remote.events.source = Source::User;
            call.answer(reply);
            if quit {
                self.exit_requested = true;
                el.exit();
                return;
            }
        }
    }

    /// Stops listening (the window is closing): the socket file goes now.
    pub(super) fn stop_remote(&mut self) {
        self.remote.server = None;
    }

    /// When a waiting test command looks again (debug builds; never in release builds).
    pub(super) fn remote_test_wake(&self) -> Option<Instant> {
        #[cfg(debug_assertions)]
        return self.test_idle_wake();
        #[cfg(not(debug_assertions))]
        None
    }

    /// The socket this window listens on (for hooks' programs).
    pub(super) fn remote_socket(&self) -> Option<PathBuf> {
        self.remote.server.as_ref().map(|s| s.path().to_path_buf())
    }

    fn remote_call(&mut self, req: Request, el: &ActiveEventLoop) -> Reply {
        info!(target: SETTINGS, "remote control: {}", req.cmd());
        if let Some(r) = self.script_call(&req) {
            return r;
        }
        match req {
            Request::Hello => Ok(json!({})),
            Request::Open {
                files,
                compare: Some(mode),
                ..
            } => {
                let mode = super::gallery::CmpMode::from_id(&mode).unwrap_or_default();
                let missing: Vec<Value> = files
                    .iter()
                    .filter(|p| !p.is_file())
                    .map(|p| path_value(p))
                    .collect();
                let n = files.len();
                if missing.is_empty() {
                    self.raise_window();
                    self.compare_files(files, mode);
                }
                let opened = if missing.is_empty() { n } else { 0 };
                Ok(json!({ "opened": opened, "compared": missing.is_empty(), "missing": missing }))
            }
            Request::Open {
                files,
                window,
                view,
                compare: None,
            } => {
                let first = files.first().cloned();
                let r = self.remote_open(files, window);
                if let (Ok(_), Some(f), Some(v), Window::Current) = (&r, first, view, window) {
                    self.script_expect_view(f, v);
                }
                r
            }
            Request::OpenUrl { url } => self.open_url(&url),
            Request::Navigate(n) => self.remote_navigate(n, el),
            Request::State => Ok(self.remote_state()),
            Request::Marked => {
                let files: Vec<Value> = self.marked_files().iter().map(|p| path_value(p)).collect();
                Ok(json!({ "files": files }))
            }
            Request::Action { name } => {
                let Some(a) = Action::from_id(&name) else {
                    return Err(Fail::new(
                        Code::UnknownAction,
                        format!("no action {name:?}"),
                    ));
                };
                // Hidden by the profile, it would do nothing: say so instead of "done".
                if let Some(c) = a.hidden(&self.action_state()) {
                    return Err(Fail::new(
                        Code::Disabled,
                        format!("{} tools are off in this profile", c.id()),
                    ));
                }
                if let Some(why) = a.disabled(&self.action_state()) {
                    return Err(Fail::new(Code::Disabled, why.message_id()));
                }
                self.action(a, el);
                Ok(json!({}))
            }
            Request::Activate => {
                self.raise_window();
                Ok(json!({}))
            }
            Request::Quit => Ok(json!({})),
            Request::Select { files, add } => {
                let Some((n, missing)) = self.gallery_select_paths(&files, add) else {
                    return Err(Fail::new(Code::Mode, "the gallery is not open"));
                };
                if n == 0 && missing.len() == files.len() {
                    return Err(Fail::new(
                        Code::NotFound,
                        "none of the files are in the gallery",
                    ));
                }
                let missing: Vec<Value> = missing.iter().map(|p| path_value(p)).collect();
                Ok(json!({ "selected": n, "missing": missing }))
            }
            Request::Mark { files, mark } => self.remote_mark(files, &mark),
            Request::ApplyStyle { files, style } => self.remote_apply_style(files, &style),
            Request::Export { files, recipe } => self.remote_export(files, &recipe),
            #[cfg(debug_assertions)]
            Request::UiState => Ok(self.test_ui_state()),
            // Answered by the connection (blobs, arrays, events) or above (`script_call`).
            _ => Err(Fail::new(Code::Internal, "not a request for the window")),
        }
    }

    /// `mark`: through the gallery's marks, as one undo step (Ctrl+Z puts them back).
    fn remote_mark(&mut self, files: Vec<PathBuf>, mark: &MarkSet) -> Reply {
        let (found, missing): (Vec<PathBuf>, Vec<PathBuf>) =
            files.into_iter().partition(|p| p.exists());
        if found.is_empty() {
            return Err(Fail::new(Code::NotFound, "none of the files exist"));
        }
        let m = mark.clone();
        self.gallery_marks_ready();
        let undo = self.gallery_set_marks(&found, move |mut x| {
            if let Some(f) = m.flag {
                x.flag = match f {
                    Flag::Pick => 1,
                    Flag::Reject => -1,
                    Flag::None => 0,
                };
            }
            if let Some(s) = m.stars {
                x.rating = s;
            }
            if let Some(l) = m.label {
                x.label = l;
            }
            x
        });
        self.redraw = true;
        let missing: Vec<Value> = missing.iter().map(|p| path_value(p)).collect();
        Ok(json!({ "marked": found.len(), "changed": undo.is_some(), "missing": missing }))
    }

    /// `apply-style`: a Develop batch in the background; `develop-saved` tells when it is
    /// written (with the job number answered here).
    fn remote_apply_style(&mut self, files: Vec<PathBuf>, style: &str) -> Reply {
        let found: Vec<PathBuf> = files.into_iter().filter(|p| p.is_file()).collect();
        if found.is_empty() {
            return Err(Fail::new(Code::NotFound, "none of the files exist"));
        }
        let Ok((token, n)) = self.develop_apply_style_named(found, style) else {
            return Err(Fail::new(
                Code::UnknownStyle,
                format!("no style named {style:?}"),
            ));
        };
        if n == 0 {
            return Err(Fail::new(
                Code::Refused,
                "none of the files are photos Develop can edit",
            ));
        }
        let job = self.remote.events.job();
        if let Some(t) = token {
            let src = self.remote.events.source.clone();
            self.remote.events.styles.insert(t, (job, src));
        }
        Ok(json!({ "job": job, "style": style, "files": n }))
    }

    /// `export`: the recipe in the background, answered at once with its job; `exported`
    /// tells when it is done.
    fn remote_export(&mut self, files: Vec<PathBuf>, recipe: &str) -> Reply {
        use super::export_recipes::RemoteRefusal;
        let found: Vec<PathBuf> = files.into_iter().filter(|p| p.is_file()).collect();
        if found.is_empty() {
            return Err(Fail::new(Code::NotFound, "none of the files exist"));
        }
        let n = found.len();
        match self.recipes_run_remote(found, recipe) {
            Ok(()) => {
                let job = self.remote.events.job();
                let src = self.remote.events.source.clone();
                self.remote.events.exports = Some((job, src));
                Ok(json!({ "job": job, "recipe": recipe, "files": n }))
            }
            Err(RemoteRefusal::Busy) => Err(Fail::new(
                Code::Busy,
                "an export is running; try again when it is done",
            )),
            Err(RemoteRefusal::Unknown) => Err(Fail::new(
                Code::UnknownRecipe,
                format!("no export recipe named {recipe:?}"),
            )),
            Err(RemoteRefusal::AsksForFolder) => Err(Fail::new(
                Code::Refused,
                "this recipe asks for its folder each time, which only the window can do; \
                 give it a fixed folder to export with it from here",
            )),
        }
    }

    /// `screenshot`: answered with the pixels of the next frame drawn.
    fn script_screenshot(&mut self, call: crate::remote::server::Call) {
        let Some(g) = &self.gfx else {
            call.answer(Err(Fail::new(Code::Mode, "the window is not shown")));
            return;
        };
        // A minimised window draws no frame; where the platform says so, answer now (on
        // Wayland it cannot tell, and the wait runs out instead).
        if g.window.is_minimized() == Some(true) {
            call.answer(Err(Fail::new(Code::Mode, super::script::NO_FRAME)));
            return;
        }
        if let Some(old) = self.remote.script.screenshot.replace(call) {
            old.answer(Err(Fail::new(
                Code::Busy,
                "another screenshot was asked for",
            )));
        }
        self.remote.script.screenshot_at = Some(Instant::now());
        self.redraw = true;
    }

    fn raise_window(&mut self) {
        if let Some(g) = &self.gfx {
            if g.window.is_minimized() == Some(true) {
                g.window.set_minimized(false);
            }
            g.window.focus_window();
        }
    }

    fn remote_open(&mut self, files: Vec<PathBuf>, window: Window) -> Reply {
        let (found, missing): (Vec<PathBuf>, Vec<PathBuf>) =
            files.into_iter().partition(|p| p.exists());
        let missing: Vec<Value> = missing.iter().map(|p| path_value(p)).collect();
        if found.is_empty() {
            return Err(Fail::new(Code::NotFound, "none of the files exist"));
        }
        let opened = found.len();
        match window {
            Window::Current => {
                self.raise_window();
                self.open_pasted(found);
            }
            Window::New => {
                let exe = std::env::current_exe()
                    .map_err(|e| Fail::new(Code::Internal, e.to_string()))?;
                let mut args = vec!["--new-window".into(), "--".into()];
                args.extend(found.into_iter().map(PathBuf::into_os_string));
                specular_platform::launch::spawn(exe.as_os_str(), &args, None, None)
                    .map_err(|e| Fail::new(Code::Internal, e.to_string()))?;
            }
        }
        Ok(json!({ "opened": opened, "missing": missing }))
    }

    /// Opens a `specular:` link: local files at once, unusual ones after a confirmation,
    /// remote sources refused (there is no remote source yet).
    pub(super) fn open_url(&mut self, url: &str) -> Reply {
        let target = match url_scheme::parse(url) {
            Ok(t) => t,
            Err(e) => {
                warn!(target: SETTINGS, "specular: link refused: {e}");
                self.integration_toast(t!(
                    "url-invalid",
                    reason = crate::error_text::url_reason(&e)
                ));
                return Err(Fail::new(Code::BadArguments, e.to_string()));
            }
        };
        self.raise_window();
        match target {
            Target::Local(paths) => {
                let existing: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
                if existing.is_empty() {
                    self.integration_toast(t!("url-not-found"));
                    return Err(Fail::new(Code::NotFound, "the linked files do not exist"));
                }
                self.gallery_dismiss();
                self.open_pasted(existing);
                Ok(json!({ "status": "opened" }))
            }
            Target::Confirm { paths, reason } => {
                let file = paths
                    .first()
                    .map(|p| url_scheme::escape_for_display(&p.to_string_lossy()))
                    .unwrap_or_default();
                let reason = match reason {
                    Reason::DotSegments => "dots",
                    Reason::NetworkOrDevice => "network",
                    Reason::HiddenCharacters => "hidden",
                };
                let more = paths.len().saturating_sub(1) as i64;
                self.remote.confirm = Some(paths);
                self.chrome.confirm(
                    t!(
                        "url-confirm-title",
                        file = file,
                        more = more,
                        reason = reason
                    ),
                    t!("url-confirm-open"),
                    CONFIRM_ITEM,
                );
                self.redraw = true;
                Ok(json!({ "status": "confirm" }))
            }
            Target::Remote { source } => {
                self.integration_toast(t!("url-remote-refused", source = source.clone()));
                Err(Fail::new(
                    Code::Refused,
                    format!("{source}: only files on this computer can be opened"),
                ))
            }
        }
    }

    /// The user accepted the link's confirmation.
    pub(super) fn url_confirmed(&mut self) {
        let Some(paths) = self.remote.confirm.take() else {
            return;
        };
        let existing: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
        if existing.is_empty() {
            self.integration_toast(t!("url-not-found"));
            return;
        }
        self.gallery_dismiss();
        self.open_pasted(existing);
    }

    fn remote_navigate(&mut self, n: Nav, el: &ActiveEventLoop) -> Reply {
        // Navigation is the viewer's: the gallery steps aside.
        self.gallery_dismiss();
        if self.entries.is_empty() || self.current.is_none() {
            return Err(Fail::new(Code::NotFound, "no image is open"));
        }
        let action = match n {
            Nav::Next => Some(Action::NextImage),
            Nav::Prev => Some(Action::PrevImage),
            Nav::First => Some(Action::FirstImage),
            Nav::Last => Some(Action::LastImage),
            Nav::Index(_) | Nav::By(_) => None,
        };
        if let Some(a) = action {
            if let Some(why) = a.disabled(&self.action_state()) {
                return Err(Fail::new(Code::Disabled, why.message_id()));
            }
            self.action(a, el);
        } else {
            let len = self.entries.len();
            let cur = self.current.unwrap_or(0);
            let target = match n {
                Nav::Index(i) => i,
                Nav::By(d) => {
                    let t = cur as i64 + d;
                    if t < 0 || t >= len as i64 {
                        return Err(Fail::new(
                            Code::NotFound,
                            format!("position {} of {len} is outside the listing", t + 1),
                        ));
                    }
                    t as usize
                }
                _ => cur,
            };
            if target >= len {
                return Err(Fail::new(
                    Code::NotFound,
                    format!("position {} of {len} is outside the listing", target + 1),
                ));
            }
            if self.in_book() && self.spread_active() {
                return Err(Fail::new(
                    Code::Mode,
                    "two-page spreads go by pages: use next and prev",
                ));
            }
            if target != cur {
                self.nav_dir = if target > cur { 1 } else { -1 };
                self.go_to(target);
            }
        }
        Ok(json!({ "index": self.current }))
    }

    fn remote_state(&mut self) -> Value {
        let file = self.current.and_then(|i| self.entries.get(i)).cloned();
        let mode = if self.gallery_active() {
            "gallery"
        } else if self.slideshow.running() || self.present_show_running() {
            "slideshow"
        } else if self.in_book() {
            "reading"
        } else {
            "viewer"
        };
        let marks = self.marks_for(&self.entries.clone());
        let marked = marks.iter().filter(|m| m.2 == 1).count();
        let mark = self
            .current
            .and_then(|i| marks.get(i))
            .map(|&(rating, label, flag)| {
                json!({
                    "rating": rating,
                    "label": super::gallery::label_english(label),
                    "flag": match flag {
                        1 => Value::from("pick"),
                        -1 => Value::from("reject"),
                        _ => Value::Null,
                    },
                })
            });
        let zoom = (f64::from(self.view.target_scale()) * 1000.0).round() / 10.0;
        json!({
            "file": file.as_deref().map(path_value),
            "index": self.current,
            "count": self.entries.len(),
            "zoom": zoom,
            "fit": self.view.mode == ZoomMode::Fit,
            "fullscreen": self.fullscreen,
            "mode": mode,
            "folder": self.folder.as_deref().map(path_value),
            "mark": mark,
            "marked": marked,
        })
    }

    /// The picked files of the listing, in order.
    pub(super) fn marked_files(&mut self) -> Vec<PathBuf> {
        let entries = self.entries.clone();
        let marks = self.marks_for(&entries);
        entries
            .into_iter()
            .zip(marks)
            .filter(|(_, m)| m.2 == 1)
            .map(|(p, _)| p)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use specular_i18n::t;

    #[test]
    fn confirmation_texts() {
        let one = t!(
            "url-confirm-title",
            file = "/a/b.jpg",
            more = 0i64,
            reason = "dots"
        );
        assert!(
            one.contains("b.jpg") && one.contains("..") && !one.contains("more"),
            "{one}"
        );
        let three = t!(
            "url-confirm-title",
            file = "/x",
            more = 2i64,
            reason = "hidden"
        )
        .replace(['\u{2068}', '\u{2069}'], "");
        assert!(
            three.contains("and 2 more") && three.contains("invisible"),
            "{three}"
        );
        let net = t!(
            "url-confirm-title",
            file = "//srv/x",
            more = 0i64,
            reason = "network"
        );
        assert!(net.contains("network share"), "{net}");
    }
}

//! Key bindings (DESIGN.md §11): `keymap.toml` beside `settings.toml` is their source of
//! truth. It is read at start-up (problems are logged and shown in Settings → Keys), every
//! edit from the editor is applied at once and written atomically, an edit made by hand is
//! picked up while Specular runs, and keymaps are imported and exported as TOML. The file
//! can also be edited as text in the config editor; its save is written and applied here
//! the same way. Key sequences (`g g`) wait `keys.sequence_timeout` for their next key.
//!
//! Benchmarks and screenshots (headless) neither read nor write the file: they use the
//! default keys.

use std::path::{Path, PathBuf};

use crossbeam_channel::Receiver;
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo, KeySeq, Keymap, KeymapEdit, Mode};
use specular_log::{SETTINGS, info, warn};

use super::App;

/// The file name, in the settings folder.
pub(super) const FILE: &str = "keymap.toml";

/// The application's side of the key bindings.
#[derive(Default)]
pub(super) struct Keys {
    /// `keymap.toml` (`None`: headless, or no settings folder).
    path: Option<PathBuf>,
    /// The text last read or written, to tell an edit by hand from our own write.
    last_text: Option<String>,
    /// The file changed on disk (debounced).
    changed: Option<Receiver<()>>,
    watcher: Option<notify::RecommendedWatcher>,
    /// A file chosen to import.
    import: Option<Receiver<PathBuf>>,
    /// A file exported.
    exported: Option<Receiver<PathBuf>>,
    /// A key sequence begun: its keys, when it times out (app clock, s), and the toast that
    /// shows how it can go on.
    pub pending: Option<(KeySeq, f64, Option<u64>)>,
    /// The medical layer is on: a grey medical image in the viewer (app/medical.rs).
    pub medical: bool,
    /// The science layer is on: a FITS or XISF image in the viewer (app/science.rs).
    pub science: bool,
    /// The drawing layer is on: a DXF drawing in the viewer (app/cad.rs).
    pub drawing: bool,
    /// The mesh layer is on: a 3D model in the viewer (app/mesh.rs).
    pub mesh: bool,
    /// The relight layer is on: a reflectance transformation image (app/relight.rs).
    pub relight: bool,
}

/// `keymap.toml` beside the settings file `settings`.
pub(super) fn path_beside(settings: &Path) -> PathBuf {
    settings.with_file_name(FILE)
}

fn log_problems(path: &Path, problems: &[specular_input::KeymapError]) {
    for p in problems {
        warn!(target: SETTINGS, "{}: {p}", path.display());
    }
}

/// Watches the folder of `path` for changes to it, debounced (editors write in steps).
fn watch(
    path: &Path,
    wake: impl Fn() + Send + 'static,
) -> Option<(notify::RecommendedWatcher, Receiver<()>)> {
    use notify::Watcher;
    let dir = path.parent()?.to_path_buf();
    let name = path.file_name()?.to_os_string();
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<()>();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(e) = res
            && e.paths
                .iter()
                .any(|p| p.file_name() == Some(name.as_os_str()))
        {
            let _ = raw_tx.send(());
        }
    })
    .ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    w.watch(&dir, notify::RecursiveMode::NonRecursive).ok()?;
    let (tx, rx) = crossbeam_channel::bounded::<()>(1);
    specular_thread::builder("set-keymap")
        .spawn(move || {
            // Ends when the watcher (and with it the sender) is dropped.
            while raw_rx.recv().is_ok() {
                while raw_rx
                    .recv_timeout(std::time::Duration::from_millis(200))
                    .is_ok()
                {}
                let _ = tx.try_send(());
                wake();
            }
        })
        .ok()?;
    Some((w, rx))
}

impl App {
    /// Reads `keymap.toml` and watches it (not headless).
    pub(super) fn keys_init(&mut self) {
        if self.headless() {
            return;
        }
        let Some(path) = self.store.path().map(|p| path_beside(&p)) else {
            return;
        };
        let (km, problems) = Keymap::load_user(&path);
        log_problems(&path, &problems);
        if path.exists() {
            info!(target: SETTINGS, "key bindings from {}", path.display());
        }
        self.keymap = km;
        self.keys_sync_modes();
        self.keys.last_text = std::fs::read_to_string(&path).ok();
        self.chrome
            .set_keymap_file(Some(&path), self.keys.last_text.as_deref());
        let wake = self.loader.waker();
        if let Some((w, rx)) = watch(&path, move || wake()) {
            self.keys.watcher = Some(w);
            self.keys.changed = Some(rx);
        }
        self.keys.path = Some(path);
    }

    /// The bindings were replaced by a settings import: saved as an import of keys is.
    pub(super) fn keys_save_imported(&mut self) {
        self.keys.pending = None;
        self.keys_save();
    }

    /// Writes the bindings to `keymap.toml`, atomically.
    fn keys_save(&mut self) {
        let Some(path) = self.keys.path.clone() else {
            return;
        };
        let text = self.keymap.to_toml();
        if self.keys.last_text.as_deref() == Some(text.as_str()) {
            return;
        }
        match specular_settings::write_atomic(&path, &text) {
            Ok(()) => {
                self.chrome.set_keymap_file(Some(&path), Some(&text));
                self.keys.last_text = Some(text);
            }
            Err(e) => {
                warn!(target: SETTINGS, "{}: {e}", path.display());
                let now = self.now();
                self.chrome.warning(
                    t!(
                        "keys-toast-write-error",
                        error = crate::error_text::io_reason(&e)
                    ),
                    now,
                );
            }
        }
    }

    /// Save from the keymap.toml editor: the text is written as typed (atomically) and
    /// applied at once, as an edit on the Keys page is; the editor hears how it went.
    pub(super) fn keys_save_text(&mut self, text: &str) {
        let res = self.keys_write_text(text);
        if let Err(e) = &res {
            warn!(target: SETTINGS, "keymap.toml: {e}");
        }
        self.chrome.keymap_saved(res);
        self.redraw = true;
    }

    fn keys_write_text(&mut self, text: &str) -> Result<(), String> {
        let Some(path) = self.keys.path.clone() else {
            return Err(t!("config-editor-no-folder"));
        };
        specular_settings::write_atomic(&path, text)
            .map_err(|e| crate::error_text::io_reason(&e))?;
        let (km, problems) = Keymap::from_toml(text);
        log_problems(&path, &problems);
        self.keys_apply(km, Some(text.to_owned()));
        self.chrome.set_keymap_file(Some(&path), Some(text));
        info!(target: SETTINGS, "key bindings saved from the editor");
        Ok(())
    }

    /// `km` read from `text` (the file now) replaces the bindings. Returns whether they
    /// changed.
    fn keys_apply(&mut self, km: Keymap, text: Option<String>) -> bool {
        let same = km.same_bindings(&self.keymap) && km.problems() == self.keymap.problems();
        self.keymap = km;
        self.keys_sync_modes();
        self.keys.last_text = text;
        self.keys.pending = None;
        self.redraw |= !same;
        !same
    }

    /// An edit from Settings → Keys.
    pub(super) fn keys_edit(&mut self, e: &KeymapEdit) {
        if self.keymap.apply_edit(e) {
            self.keys.pending = None;
            self.keys_save();
            self.redraw = true;
        }
    }

    /// Export… (`Some`: one mode's bindings, which import as that mode only).
    pub(super) fn keys_export(&mut self, mode: Option<Mode>) {
        let (text, name) = match mode {
            Some(m) => (
                self.keymap.mode_to_toml(m),
                format!("specular-keys-{}.toml", m.id()),
            ),
            None => (self.keymap.to_toml(), "specular-keys.toml".to_owned()),
        };
        let (tx, rx) = crossbeam_channel::bounded::<PathBuf>(1);
        self.loader.background("dlg-keys-export", move |l| {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("TOML", &["toml"])
                .set_file_name(name)
                .save_file()
            {
                match specular_settings::write_atomic(&p, &text) {
                    Ok(()) => {
                        let _ = tx.send(p);
                        l.waker()();
                    }
                    Err(e) => warn!(target: SETTINGS, "export keys: {e}"),
                }
            }
        });
        self.keys.exported = Some(rx);
    }

    /// Import…: the file is read once chosen ([`App::keys_poll`]).
    pub(super) fn keys_import(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded::<PathBuf>(1);
        self.loader.background("dlg-keys-import", move |l| {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("TOML", &["toml"])
                .pick_file()
            {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        self.keys.import = Some(rx);
    }

    fn keys_import_from(&mut self, p: &Path) {
        let now = self.now();
        let res = std::fs::read_to_string(p)
            .map_err(|e| crate::error_text::io_reason(&e))
            .and_then(|text| {
                self.keymap
                    .import(&text)
                    .map_err(|e| crate::error_text::text_reason(&e.to_string()))
            });
        match res {
            Ok(problems) => {
                log_problems(p, &problems);
                self.keys.pending = None;
                self.keys_save();
                self.chrome.toast(t!("keys-toast-imported"), None, now);
            }
            Err(e) => {
                self.chrome.warning(t!("keys-toast-error", error = e), now);
            }
        }
        self.redraw = true;
    }

    /// Each turn of the loop: a file to import, an edit by hand, a sequence timed out.
    pub(super) fn keys_poll(&mut self, el: &super::ActiveEventLoop) {
        self.keys_sync_modes();
        if let Some(rx) = &self.keys.import
            && let Ok(p) = rx.try_recv()
        {
            self.keys.import = None;
            self.keys_import_from(&p);
        }
        if let Some(rx) = &self.keys.exported
            && rx.try_recv().is_ok()
        {
            self.keys.exported = None;
            let now = self.now();
            self.chrome.toast(t!("keys-toast-exported"), None, now);
            self.redraw = true;
        }
        if self
            .keys
            .changed
            .as_ref()
            .is_some_and(|rx| rx.try_recv().is_ok())
        {
            self.keys_reload();
        }
        if let Some((_, due, _)) = self.keys.pending
            && self.now() >= due - 1e-3
        {
            let (seq, _, toast) = self.keys.pending.take().expect("pending");
            if let Some(id) = toast {
                self.chrome.dismiss_toast(id);
            }
            let modes = self.key_modes();
            if let Some(a) = self.keymap.expire(modes, seq) {
                self.action(a, el);
            }
            self.redraw = true;
        }
    }

    /// `keymap.toml` changed on disk: read it again unless it is what we wrote.
    fn keys_reload(&mut self) {
        let Some(path) = self.keys.path.clone() else {
            return;
        };
        let text = std::fs::read_to_string(&path).ok();
        if text == self.keys.last_text {
            return;
        }
        let (km, problems) = Keymap::load_user(&path);
        log_problems(&path, &problems);
        self.chrome.set_keymap_file(Some(&path), text.as_deref());
        if self.keys_apply(km, text) {
            info!(target: SETTINGS, "key bindings reloaded");
            let now = self.now();
            self.chrome.toast(t!("keys-toast-reloaded"), None, now);
        }
    }

    /// The keymap's modes looked up for a key, innermost first: cull mode, the medical,
    /// science, drawing or 3D-model layer, then the viewer.
    pub(super) fn key_modes(&self) -> &'static [Mode] {
        // Develop's keys over the viewer's (cull mode's between them when it is on).
        if self.develop_active() {
            return if self.cull.active() {
                &[Mode::Develop, Mode::Cull, Mode::Viewer]
            } else {
                &[Mode::Develop, Mode::Viewer]
            };
        }
        // MPR's keys are medical mode's, for any volume (a microscopy stack in science
        // mode, a cube): its tier-1 tools show whatever the profile (`Profile::mpr`).
        if self.mpr_active() {
            return if self.cull.active() {
                &[Mode::Cull, Mode::Medical, Mode::Viewer]
            } else {
                &[Mode::Medical, Mode::Viewer]
            };
        }
        if self.keys.mesh {
            return if self.cull.active() {
                &[Mode::Cull, Mode::Mesh, Mode::Viewer]
            } else {
                &[Mode::Mesh, Mode::Viewer]
            };
        }
        if self.keys.relight {
            return if self.cull.active() {
                &[Mode::Cull, Mode::Relight, Mode::Viewer]
            } else {
                &[Mode::Relight, Mode::Viewer]
            };
        }
        // A movie: Space plays and pauses, the seek and sound keys (app/playback.rs).
        if self.video_current() && !self.gallery_active() {
            return if self.cull.active() {
                &[Mode::Cull, Mode::Video, Mode::Viewer]
            } else {
                &[Mode::Video, Mode::Viewer]
            };
        }
        let layer = if self.keys.medical {
            Some(Mode::Medical)
        } else if self.keys.science {
            Some(Mode::Science)
        } else if self.keys.drawing {
            Some(Mode::Drawing)
        } else {
            None
        };
        // The map view's keys over the drawing or science keys of the map.
        if self.gis_on() {
            return match (self.cull.active(), layer) {
                (true, Some(Mode::Science)) => {
                    &[Mode::Cull, Mode::Map, Mode::Science, Mode::Viewer]
                }
                (true, Some(Mode::Drawing)) => {
                    &[Mode::Cull, Mode::Map, Mode::Drawing, Mode::Viewer]
                }
                (true, _) => &[Mode::Cull, Mode::Map, Mode::Viewer],
                (false, Some(Mode::Science)) => &[Mode::Map, Mode::Science, Mode::Viewer],
                (false, Some(Mode::Drawing)) => &[Mode::Map, Mode::Drawing, Mode::Viewer],
                (false, _) => &[Mode::Map, Mode::Viewer],
            };
        }
        match (self.cull.active(), layer) {
            (true, Some(Mode::Medical)) => &[Mode::Cull, Mode::Medical, Mode::Viewer],
            (true, Some(Mode::Science)) => &[Mode::Cull, Mode::Science, Mode::Viewer],
            (true, Some(_)) => &[Mode::Cull, Mode::Drawing, Mode::Viewer],
            (true, None) => &[Mode::Cull, Mode::Viewer],
            (false, Some(Mode::Medical)) => &[Mode::Medical, Mode::Viewer],
            (false, Some(Mode::Science)) => &[Mode::Science, Mode::Viewer],
            (false, Some(_)) => &[Mode::Drawing, Mode::Viewer],
            (false, None) => &[Mode::Viewer],
        }
    }

    /// Tells the keymap which modes are on, so lookups, the digit row and the keys shown in
    /// the palette, tooltips and the cheat sheet follow them.
    pub(super) fn keys_sync_modes(&mut self) {
        let modes = self.key_modes();
        if self.keymap.active() != modes {
            self.keymap.set_active(modes);
            self.redraw = true;
        }
        // Keys of capabilities that are off fall through.
        let p = self.action_state().content_profile();
        if self.keymap.profile() != p {
            self.keymap.set_profile(p);
            self.redraw = true;
        }
    }

    /// A key pressed (not taken by the chrome): the actions it runs now, in order. A key that
    /// begins a sequence waits for the next one (or the timeout), with the keys that can
    /// follow shown.
    pub(super) fn keys_step(&mut self, combo: KeyCombo, repeat: bool) -> Vec<Action> {
        self.keys_sync_modes();
        let modes = self.key_modes();
        if repeat {
            // Auto-repeat runs single keys only, never a sequence.
            if self.keys.pending.is_some() {
                return Vec::new();
            }
            return self
                .keymap
                .lookup_in(modes, &KeySeq::one(combo))
                .into_iter()
                .collect();
        }
        let pending = self.keys.pending.take();
        if let Some((_, _, Some(id))) = pending {
            self.chrome.dismiss_toast(id);
        }
        let seq = pending.map(|p| p.0);
        let mut step = self.keymap.step(modes, seq, combo);
        // Caps Lock: a letter bound in lower case still works.
        if step.run.is_empty()
            && step.pending.is_none()
            && let Key::Char(c) = combo.key
            && c.is_uppercase()
            && !combo.mods.shift
        {
            let lower = KeyCombo::new(Key::Char(c.to_lowercase().next().unwrap_or(c)), combo.mods);
            step = self.keymap.step(modes, seq, lower);
        }
        if let Some(s) = step.pending {
            let now = self.now();
            let timeout = self.settings.keys.sequence_timeout.max(0.1);
            let next: Vec<String> = self
                .keymap
                .continuations(modes, &s)
                .into_iter()
                .map(|(k, a)| {
                    let rest: Vec<String> =
                        k.keys()[s.len()..].iter().map(|c| c.to_string()).collect();
                    format!("{} {}", rest.join(" "), specular_widgets::action_label(a))
                })
                .collect();
            let toast = (!next.is_empty()).then(|| {
                self.chrome.toast(
                    t!(
                        "keys-pending",
                        keys = s.to_string(),
                        next = specular_i18n::dot_join(&next)
                    ),
                    None,
                    now,
                )
            });
            self.keys.pending = Some((s, now + timeout, toast));
            let t = std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout);
            self.wake_by(t, super::Cause::KeySequence);
            self.redraw = true;
        }
        step.run
    }
}

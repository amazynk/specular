//! Desktop integration in the viewer: "Open with…", user commands (`[[commands]]` in the
//! settings file), dragging the file out of the window and pasting from the clipboard.
//!
//! Slow work (listing applications, reading the clipboard, waiting for a command to exit)
//! runs on background threads; results come back through a channel drained once per
//! event-loop turn (`drain_integration`).

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::Icon;
use specular_i18n::{I18n, t};
use specular_input::KeyCombo;
use specular_log::{UI, debug, info, warn};
use specular_platform::drag::{DragError, DragOut};
use specular_platform::launch::{self, Target};
use specular_platform::open_with::{self, Handler};
use specular_settings::{Locations, UserCommand};
use specular_widgets::PaletteItem;
use specular_widgets::line_edit::PendingClipboard;

use super::App;
use crate::clipboard::Pasted;
use crate::loader::Copied;

mod desktop;

/// The setting the "More file types" panel sits under.
pub(super) const EXTRA_TYPES_UNDER: &str = desktop::EXTRA_TYPES_UNDER;

/// Palette item ids: user commands, then handlers of the open list, then "choose another".
const COMMAND_BASE: u64 = 1 << 20;
const HANDLER_BASE: u64 = 1 << 32;
const CHOOSE_OTHER: u64 = 1 << 40;

/// Pointer travel (physical px) before a Ctrl+press becomes a drag.
const DRAG_THRESHOLD: f32 = 6.0;

/// Pasted images kept in the cache (older ones are removed).
const KEEP_PASTED: usize = 20;

/// The text field a paste is for (the clipboard is read off the UI thread).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PasteTo {
    /// The chrome's (the settings.toml editor, the Settings search fields).
    Editor,
    GalleryFilter,
    /// Convert's name pattern.
    ConvertName,
    /// The export recipes editor's focused field.
    ExportRecipes,
    /// The DICOMweb sheet's focused field.
    DicomWeb,
    /// The OMERO sheet's focused field.
    Omero,
    /// The import view's focused field.
    Import,
    /// The archive password prompt's field.
    Password,
    /// The thermal report dialog's focused field.
    ThermalReport,
    /// The rename and batch rename fields.
    FileOps,
    /// The map's Go to coordinate field.
    MapGoTo,
    /// The markup tool's text field.
    Markup,
    /// The Open address dialog's field.
    Address,
    /// The keyword panel's field.
    Keywords,
    /// The client gallery and client picks dialogs' focused field.
    WebGallery,
    /// The metadata editor's field.
    MetaEdit,
}

enum Msg {
    /// Applications for the file of open-list request `req`.
    Handlers {
        req: u64,
        list: Vec<Handler>,
    },
    /// A user command ended.
    Exited {
        name: String,
        file: PathBuf,
        reload: bool,
        result: Result<(), String>,
    },
    Pasted(Result<Vec<PathBuf>, String>),
    /// A remote Zarr store's address was pasted.
    PastedAddress(String),
    /// The clipboard's text for a text field.
    PastedText(PasteTo, String),
    /// Something started in the background failed: show this text.
    Failed(String),
}

struct Cmd {
    c: UserCommand,
    key: Option<KeyCombo>,
    /// The program is not on `PATH` (shown greyed out).
    missing: bool,
}

pub(super) struct Integration {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    locations: Locations,
    commands: Vec<Cmd>,
    keys: HashMap<KeyCombo, usize>,
    /// Settings generation the commands were read at.
    commands_gen: Option<u64>,
    /// UI generation the translated texts were made at.
    i18n_gen: Option<u64>,
    /// The open list: its request number, file and applications (None while listing).
    req: u64,
    list_file: Option<PathBuf>,
    handlers: Option<Vec<Handler>>,
    /// The right-click menu's row the list opened beside, as its submenu (none: the
    /// palette's list).
    menu_row: Option<u64>,
    drag: Option<DragOut>,
    /// Ctrl+press on the image, waiting to travel far enough to become a drag.
    drag_press: Option<(f32, f32)>,
    /// Settings → Integration, the D-Bus single instance, "Set as wallpaper".
    desktop: desktop::Desktop,
}

impl Integration {
    pub(super) fn new() -> Integration {
        let (tx, rx) = crossbeam_channel::unbounded();
        Integration {
            tx,
            rx,
            locations: Locations::detect(),
            commands: Vec::new(),
            keys: HashMap::new(),
            commands_gen: None,
            i18n_gen: None,
            req: 0,
            list_file: None,
            handlers: None,
            menu_row: None,
            drag: None,
            drag_press: None,
            desktop: desktop::Desktop::new(),
        }
    }

    /// Drops the drag source, whose Wayland thread uses winit's display: called before the
    /// event loop (and with it the display) goes away.
    pub(super) fn close_window(&mut self) {
        self.drag = None;
        self.drag_press = None;
    }
}

/// The About box's storage line.
fn storage_text(l: &Locations) -> String {
    if l.portable {
        t!(
            "about-storage-portable",
            dir = l.config_dir.display().to_string()
        )
    } else {
        t!(
            "about-storage",
            settings = l.settings_file.display().to_string(),
            cache = l.cache_dir.display().to_string()
        )
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

impl App {
    /// The file integration acts on: the current image, when it is a real file (not an
    /// archive page).
    pub(in crate::app) fn integration_file(&self) -> Option<PathBuf> {
        if self.gallery.active {
            return self.gallery.focus_file();
        }
        if self.in_book() {
            return None;
        }
        self.current
            .and_then(|i| self.entries.get(i))
            .filter(|p| p.is_file())
            .cloned()
    }

    pub(super) fn integration_toast(&mut self, text: String) {
        let now = self.now();
        self.chrome.toast(text, None, now);
        self.redraw = true;
    }

    /// Once per event-loop turn: results from background threads, reloading the commands
    /// when the settings file changed, and the drag source once the window exists.
    pub(super) fn drain_integration(&mut self) {
        let gen_ = self.store.generation();
        let i18n_gen = I18n::global().generation();
        if self.integration.commands_gen != Some(gen_)
            || self.integration.i18n_gen != Some(i18n_gen)
        {
            if self.integration.commands_gen != Some(gen_) {
                self.load_commands();
            }
            self.integration.commands_gen = Some(gen_);
            self.integration.i18n_gen = Some(i18n_gen);
            let mut items = self.command_items();
            items.extend(self.insight_items());
            items.extend(self.work_items());
            items.extend(self.net_items());
            items.extend(self.gallery_palette_items());
            self.chrome.set_palette_items(items);
            let storage = storage_text(&self.integration.locations);
            self.chrome.set_about_storage(storage);
        }
        if self.integration.drag.is_none()
            && let Some(g) = &self.gfx
        {
            let d = DragOut::new(&*g.window);
            debug!(target: UI, "drag source: {d:?}");
            self.integration.drag = Some(d);
        }
        while let Ok(m) = self.integration.rx.try_recv() {
            self.on_integration(m);
        }
        self.drain_desktop();
    }

    fn load_commands(&mut self) {
        let (mut commands, problems) = self.store.user_commands();
        for p in problems {
            warn!(target: UI, "user command: {p:?}");
        }
        // Recipe files: one recipe each, after the commands of settings.toml.
        commands.extend(super::recipes::read_dir(
            &self
                .integration
                .locations
                .config_dir
                .join(super::recipes::RECIPE_DIR),
        ));
        let mut keys = HashMap::new();
        let commands: Vec<Cmd> = commands
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let key = c.key.as_deref().and_then(|k| {
                    let combo = KeyCombo::parse(k);
                    if combo.is_none() {
                        warn!(target: UI, "user command {:?}: key {k:?} not understood", c.name);
                    }
                    combo
                });
                if let Some(k) = key {
                    keys.entry(k).or_insert(i);
                }
                let missing = launch::find_program(&c.program).is_none();
                Cmd { c, key, missing }
            })
            .collect();
        if !commands.is_empty() {
            info!(target: UI, "{} user command(s)", commands.len());
        }
        self.integration.commands = commands;
        self.integration.keys = keys;
    }

    pub(super) fn command_items(&self) -> Vec<PaletteItem> {
        self.integration
            .commands
            .iter()
            .enumerate()
            .map(|(i, c)| PaletteItem {
                id: COMMAND_BASE + i as u64,
                label: c.c.name.clone(),
                key: c.key.map(|k| k.to_string()),
                unavailable: c
                    .missing
                    .then(|| t!("command-missing", program = c.c.program.clone())),
                icon: Icon::Play,
            })
            .collect()
    }

    /// The open list: applications, user commands, then "choose another application".
    fn open_with_items(&self) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        for (i, h) in self.integration.handlers.iter().flatten().enumerate() {
            items.push(PaletteItem {
                id: HANDLER_BASE + i as u64,
                label: if h.is_default {
                    t!("open-with-default", name = h.name.clone())
                } else {
                    h.name.clone()
                },
                key: None,
                unavailable: None,
                icon: Icon::Image,
            });
        }
        items.extend(self.command_items());
        items.push(PaletteItem {
            id: CHOOSE_OTHER,
            label: t!("open-with-choose-other"),
            key: None,
            unavailable: None,
            icon: Icon::Folder,
        });
        items
    }

    /// "Open with…": the list opens at once and fills when the applications are known. From
    /// the right-click menu it opens beside its row there.
    pub(super) fn open_with(&mut self) {
        let Some(file) = self.integration_file() else {
            return;
        };
        self.integration.req += 1;
        let req = self.integration.req;
        self.integration.handlers = None;
        self.integration.list_file = Some(file.clone());
        let items = self.open_with_items();
        let loading = Some(t!("open-with-loading"));
        match self.chrome.open_submenu(items, loading.clone()) {
            Ok(row) => self.integration.menu_row = Some(row),
            Err(items) => {
                self.integration.menu_row = None;
                self.chrome
                    .open_list(t!("open-with-placeholder"), items, loading);
            }
        }
        let lang = I18n::global().language().to_string();
        let tx = self.integration.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("int-open-with", move |_| {
            let list = open_with::handlers_for(&file, &lang);
            debug!(target: UI, "{} application(s) for {}", list.len(), file.display());
            let _ = tx.send(Msg::Handlers { req, list });
            wake();
        });
        self.redraw = true;
    }

    /// A palette item of ours was chosen.
    pub(super) fn integration_item(&mut self, id: u64) {
        if id == CHOOSE_OTHER {
            self.choose_other();
        } else if id >= HANDLER_BASE {
            let i = (id - HANDLER_BASE) as usize;
            let (Some(h), Some(file)) = (
                self.integration
                    .handlers
                    .as_ref()
                    .and_then(|l| l.get(i))
                    .cloned(),
                self.integration.list_file.clone(),
            ) else {
                return;
            };
            let tx = self.integration.tx.clone();
            let wake = self.loader.waker();
            self.loader.background("int-open-with", move |_| {
                if let Err(e) = open_with::launch(&h, &file) {
                    warn!(target: UI, "open with {}: {e}", h.name);
                    let _ = tx.send(Msg::Failed(t!(
                        "open-with-failed",
                        name = h.name.clone(),
                        reason = crate::error_text::spawn_reason(&e)
                    )));
                    wake();
                }
            });
        } else if id >= COMMAND_BASE {
            self.run_command((id - COMMAND_BASE) as usize);
        }
    }

    /// "Choose another application…": the system chooser on Windows; elsewhere a file
    /// dialog picks the program, which is started with the file.
    fn choose_other(&mut self) {
        let Some(file) = self
            .integration
            .list_file
            .clone()
            .or_else(|| self.integration_file())
        else {
            return;
        };
        let title = t!("open-with-choose-title");
        let tx = self.integration.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("int-open-with", move |_| {
            let r = match open_with::choose_other(&file) {
                Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
                    pick_and_start(&title, &file)
                }
                r => r,
            };
            if let Err(e) = r {
                let _ = tx.send(Msg::Failed(t!(
                    "open-with-failed",
                    name = title.clone(),
                    reason = crate::error_text::spawn_reason(&e)
                )));
                wake();
            }
        });
    }

    /// User command `i` (recipes included).
    pub(super) fn user_command(&self, i: usize) -> Option<specular_settings::UserCommand> {
        self.integration.commands.get(i).map(|c| c.c.clone())
    }

    /// Where settings and caches live.
    pub(super) fn integration_locations(&self) -> &Locations {
        &self.integration.locations
    }

    /// Runs user command `i` on the current file.
    fn run_command(&mut self, i: usize) {
        let Some(cmd) = self.integration.commands.get(i) else {
            return;
        };
        if cmd.c.recipe.is_some() {
            self.recipe_start(i);
            return;
        }
        let (name, program, args, reload) = (
            cmd.c.name.clone(),
            cmd.c.program.clone(),
            cmd.c.args.clone(),
            cmd.c.reload,
        );
        let Some(file) = self.integration_file() else {
            return;
        };
        let mut files = vec![file.clone()];
        if self.gallery.active {
            let t = self.gallery.targets();
            if !t.is_empty() {
                files = t;
            }
        }
        let target = Target {
            file: &file,
            files: &files,
            page: 1,
        };
        let args = match launch::expand_args(&args, &target) {
            Ok(a) => a,
            Err(e) => {
                self.integration_toast(t!(
                    "command-failed",
                    name = name,
                    reason = crate::error_text::reason(&e)
                ));
                return;
            }
        };
        let prog: OsString = launch::find_program(&program)
            .map_or_else(|| program.clone().into(), PathBuf::into_os_string);
        let tx = self.integration.tx.clone();
        let wake = self.loader.waker();
        let (n, f) = (name.clone(), file.clone());
        let on_exit: launch::OnExit = Box::new(move |r| {
            let result = match r {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(s.to_string()),
                Err(e) => Err(crate::error_text::spawn_reason(&e)),
            };
            let _ = tx.send(Msg::Exited {
                name: n,
                file: f,
                reload,
                result,
            });
            wake();
        });
        info!(target: UI, "run {name:?}: {} {args:?}", prog.to_string_lossy());
        if let Err(e) = launch::spawn(&prog, &args, file.parent(), Some(on_exit)) {
            self.integration_toast(t!(
                "command-failed",
                name = name,
                reason = crate::error_text::spawn_reason(&e)
            ));
        }
    }

    /// User command shortcuts (they win over the built-in ones). True: the key was used.
    pub(super) fn integration_key(&mut self, combo: &KeyCombo) -> bool {
        if self.chrome.modal().is_some() || self.chrome.wants_text() {
            return false;
        }
        let Some(&i) = self.integration.keys.get(combo) else {
            return false;
        };
        if self.integration.commands[i].missing {
            let program = self.integration.commands[i].c.program.clone();
            self.integration_toast(t!("command-missing", program = program));
        } else {
            self.run_command(i);
        }
        true
    }

    fn on_integration(&mut self, m: Msg) {
        match m {
            Msg::Handlers { req, list } => {
                if req != self.integration.req {
                    return;
                }
                self.integration.handlers = Some(list);
                let items = self.open_with_items();
                match self.integration.menu_row {
                    Some(row) => self.chrome.update_submenu(row, items, None),
                    None => self.chrome.update_list(items, None),
                }
                self.redraw = true;
            }
            Msg::Exited {
                name,
                file,
                reload,
                result,
            } => {
                debug!(target: UI, "{name:?} ended: {result:?}");
                if let Err(e) = result {
                    self.integration_toast(t!("command-exited", name = name, status = e));
                }
                if reload {
                    self.reload_path(&file);
                }
            }
            Msg::Pasted(Ok(files)) => self.open_pasted(files),
            Msg::PastedAddress(url) => {
                if crate::clipboard::is_stream_address(&url) {
                    self.stream_open(url, false);
                } else {
                    self.zarr_open(url);
                }
            }
            Msg::PastedText(to, s) => {
                match to {
                    PasteTo::Editor => self.chrome.paste_text(&s),
                    PasteTo::GalleryFilter => {
                        if self.gallery_wants_text() {
                            self.gallery.filter_paste(&s);
                        }
                    }
                    PasteTo::ConvertName => self.convert_paste(&s),
                    PasteTo::FileOps => self.fileops_paste(&s),
                    PasteTo::DicomWeb => self.dicomweb_paste(&s),
                    PasteTo::Omero => self.omero_paste(&s),
                    PasteTo::Import => self.import_paste(&s),
                    PasteTo::Password => self.password_paste(s),
                    PasteTo::ThermalReport => self.thermal_report_paste(&s),
                    PasteTo::WebGallery => self.web_gallery_paste(&s),
                    PasteTo::MapGoTo => self.map_goto_paste(&s),
                    PasteTo::Markup => self.markup_paste(&s),
                    PasteTo::ExportRecipes => self.recipes_paste(&s),
                    PasteTo::Address => self.address_paste(&s),
                    PasteTo::Keywords => self.keywords_paste(&s),
                    PasteTo::MetaEdit => self.meta_paste(&s),
                }
                self.redraw = true;
            }
            Msg::Pasted(Err(e)) | Msg::Failed(e) => self.integration_toast(e),
        }
    }

    /// Reloads `file` if it is the current image (an editor saved it).
    pub(super) fn reload_path(&mut self, file: &Path) {
        let Some(id) = self.by_path.get(file).copied() else {
            return;
        };
        if self.current_id() != Some(id) {
            // Not on screen: just forget it so it is read afresh when shown.
            if let Some(img) = self.images.remove(&id) {
                img.token.cancel();
                self.by_path.remove(&img.path);
                self.gpu_images.remove(id);
                if let Some(g) = self.gfx.as_mut() {
                    g.renderer.release_image(id);
                }
            }
            return;
        }
        if let Some(img) = self.images.remove(&id) {
            info!(target: UI, "reloading {}", file.display());
            img.token.cancel();
            self.by_path.remove(&img.path);
            self.gpu_images.remove(id);
            if let Some(g) = self.gfx.as_mut() {
                g.renderer.release_image(id);
            }
            self.shown = None;
            if let Some(i) = self.current {
                self.go_to(i);
            }
            self.redraw = true;
        }
    }

    // ----- Paste --------------------------------------------------------------------------

    /// Ctrl+V: opens the files on the clipboard, or saves a pasted image to the cache and
    /// opens that.
    pub(super) fn paste(&mut self) {
        let clipboard = self.clipboard.clone();
        let dir = self.integration.locations.cache_dir.join("pasted");
        let tx = self.integration.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("clip-paste", move |_| {
            let r = match clipboard.read_paste() {
                Ok(Pasted::Files(files)) => Ok(files),
                Ok(Pasted::Address(url)) => {
                    let _ = tx.send(Msg::PastedAddress(url));
                    wake();
                    return;
                }
                Ok(Pasted::Image {
                    width,
                    height,
                    rgba,
                }) => save_pasted(&dir, width, height, &rgba)
                    .map(|p| vec![p])
                    .map_err(|e| t!("paste-failed", reason = e)),
                Err(e) => {
                    debug!(target: UI, "paste: {e}");
                    Err(t!("paste-nothing"))
                }
            };
            let _ = tx.send(Msg::Pasted(r));
            wake();
        });
    }

    /// What a text field did with the clipboard and the primary selection: copied text goes
    /// on the clipboard, a new selection becomes the primary selection (copy-on-select), and
    /// a paste (Ctrl+V, or a middle click: the primary selection) reads off the UI thread for
    /// `to`.
    pub(super) fn field_clipboard(&mut self, clip: PendingClipboard, to: PasteTo) {
        if let Some(text) = clip.copied {
            self.clipboard.copy_text(&self.loader, text, Copied::Editor);
        }
        if let Some(text) = clip.primary {
            self.set_primary(text);
        }
        if clip.paste {
            self.paste_text(to);
        }
        if clip.paste_primary {
            self.paste_primary(to);
        }
    }

    /// Makes `text` the primary selection, where the platform has one.
    pub(super) fn set_primary(&mut self, text: String) {
        if specular_widgets::line_edit::PRIMARY_SELECTION {
            self.clipboard.set_primary(&self.loader, text);
        }
    }

    /// Reads the clipboard's text in the background for the text field `to`.
    pub(super) fn paste_text(&mut self, to: PasteTo) {
        self.read_for_field(to, "paste-text", |c| c.read_text());
    }

    /// Reads the primary selection's text in the background for the text field `to` (a
    /// middle click in it), where the platform has one.
    pub(super) fn paste_primary(&mut self, to: PasteTo) {
        if specular_widgets::line_edit::PRIMARY_SELECTION {
            self.read_for_field(to, "paste-primary", |c| c.read_primary());
        }
    }

    fn read_for_field(
        &mut self,
        to: PasteTo,
        job: &'static str,
        read: fn(&crate::clipboard::Clipboard) -> Result<String, String>,
    ) {
        let clipboard = self.clipboard.clone();
        let tx = self.integration.tx.clone();
        let wake = self.loader.waker();
        self.loader.background(job, move |_| {
            match read(&clipboard) {
                Ok(s) => {
                    let _ = tx.send(Msg::PastedText(to, s));
                }
                Err(e) => debug!(target: UI, "{job}: {e}"),
            }
            wake();
        });
    }

    pub(super) fn open_pasted(&mut self, files: Vec<PathBuf>) {
        let mut files: Vec<PathBuf> = files
            .into_iter()
            .map(|p| std::path::absolute(&p).unwrap_or(p))
            .filter(|p| p.exists())
            .collect();
        info!(target: UI, "paste: {} item(s)", files.len());
        match files.len() {
            0 => self.integration_toast(t!("paste-nothing")),
            1 => {
                let p = files.remove(0);
                self.open_target(p);
            }
            _ if files.iter().all(|p| p.is_file()) => {
                self.gallery_dismiss();
                // Exactly these files: a folder listing still on its way must not replace them.
                self.folder = None;
                self.set_entries(files);
                self.go_to(0);
                self.redraw = true;
            }
            _ => {
                let p = files.remove(0);
                self.open_target(p);
            }
        }
    }

    // ----- Drag out -----------------------------------------------------------------------

    fn drag_supported(&self) -> bool {
        self.integration
            .drag
            .as_ref()
            .is_some_and(DragOut::supported)
    }

    /// The pointer and modifier keys during a drop from another application (X11 only; see
    /// [`DragOut::drop_pointer`]).
    pub(super) fn drop_pointer(&self) -> Option<specular_platform::drag::DropPointer> {
        self.integration.drag.as_ref()?.drop_pointer()
    }

    /// Drags `files` out of the gallery. True when a drag started.
    pub(super) fn drag_out_files(&mut self, files: &[PathBuf]) -> bool {
        if files.is_empty() {
            return false;
        }
        let Some(d) = self.integration.drag.as_ref() else {
            return false;
        };
        if !d.supported() || d.active() {
            return false;
        }
        match d.start(files) {
            Ok(()) => {
                info!(target: UI, "dragging {} file(s) out of the gallery", files.len());
                true
            }
            Err(DragError::NotReady) => false,
            Err(e) => {
                warn!(target: UI, "drag-out: {e}");
                self.integration_toast(t!("drag-failed", reason = crate::error_text::reason(&e)));
                false
            }
        }
    }

    /// Primary button on the image area. A Ctrl+press is kept for dragging the file out
    /// (no pan starts). True: the press or release was ours.
    pub(super) fn drag_out_button(&mut self, down: bool, p: (f32, f32), consumed: bool) -> bool {
        if !down {
            return self.integration.drag_press.take().is_some();
        }
        if consumed
            || self.chrome.modal().is_some()
            || !self.mods.control_key()
            || self.shown.is_none()
            || !self.drag_supported()
            || self.integration_file().is_none()
        {
            return false;
        }
        self.integration.drag_press = Some(p);
        true
    }

    /// Pointer motion: starts the drag after a Ctrl+press has moved far enough, or when a
    /// pan of an image that fits the window leaves the window.
    pub(super) fn drag_out_motion(&mut self, p: (f32, f32)) {
        let start = if let Some(q) = self.integration.drag_press {
            (p.0 - q.0).hypot(p.1 - q.1) >= DRAG_THRESHOLD
        } else if self.dragging.is_some() && !self.view.can_pan() {
            self.gfx.as_ref().is_some_and(|g| {
                let s = g.window.inner_size();
                p.0 < 0.0 || p.1 < 0.0 || p.0 >= s.width as f32 || p.1 >= s.height as f32
            })
        } else {
            false
        };
        if !start {
            return;
        }
        self.integration.drag_press = None;
        let Some(file) = self.integration_file() else {
            return;
        };
        let Some(d) = self.integration.drag.as_ref() else {
            return;
        };
        if d.active() {
            return;
        }
        let r = d.start(std::slice::from_ref(&file));
        // The press is the drag's now: the window may never see its release.
        if self.dragging.take().is_some() {
            let now = self.now();
            self.view.end_drag(now);
            self.after_view_change();
        }
        match r {
            Ok(()) => info!(target: UI, "dragging {} out", file.display()),
            Err(DragError::NotReady) => debug!(target: UI, "drag-out not ready"),
            Err(e) => {
                warn!(target: UI, "drag-out: {e}");
                self.integration_toast(t!("drag-failed", reason = crate::error_text::reason(&e)));
            }
        }
    }
}

/// Saves clipboard pixels as `dir/pasted-<time>.png`, keeping the newest few.
fn save_pasted(dir: &Path, width: usize, height: usize, rgba: &[u8]) -> Result<PathBuf, String> {
    let (w, h) = (
        u32::try_from(width).map_err(|e| e.to_string())?,
        u32::try_from(height).map_err(|e| e.to_string())?,
    );
    let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, rgba)
        .map_err(|e| crate::error_text::reason(&e))?;
    std::fs::create_dir_all(dir).map_err(|e| crate::error_text::reason(&e))?;
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let path = dir.join(format!("pasted-{ms}.png"));
    std::fs::write(&path, &png).map_err(|e| crate::error_text::reason(&e))?;
    debug!(target: UI, "pasted image {w}×{h} saved as {} ({} bytes)", path.display(), png.len());
    prune_pasted(dir, &path);
    Ok(path)
}

fn prune_pasted(dir: &Path, keep: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut old: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p != keep
                && p.extension().is_some_and(|e| e == "png")
                && file_name(p).starts_with("pasted-")
        })
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    if old.len() < KEEP_PASTED {
        return;
    }
    old.sort();
    for (_, p) in &old[..old.len() + 1 - KEEP_PASTED] {
        let _ = std::fs::remove_file(p);
    }
}

/// Picks a program with a file dialog and starts it with `file` (no system chooser here).
fn pick_and_start(title: &str, file: &Path) -> std::io::Result<()> {
    let mut d = rfd::FileDialog::new().set_title(title);
    if cfg!(target_os = "macos") {
        d = d.set_directory("/Applications");
    } else if cfg!(unix) {
        d = d.set_directory("/usr/bin");
    }
    let Some(program) = d.pick_file() else {
        return Ok(());
    };
    if cfg!(target_os = "macos") && program.extension().is_some_and(|e| e == "app") {
        let args = [OsString::from("-a"), program.into_os_string(), file.into()];
        return launch::spawn("open".as_ref(), &args, None, None);
    }
    launch::spawn(program.as_os_str(), &[file.into()], file.parent(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_images_are_saved_and_pruned() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/agent-app/test-pasted")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rgba: Vec<u8> = (0..4 * 3 * 2).map(|i| i as u8).collect();
        let mut last = PathBuf::new();
        for _ in 0..KEEP_PASTED + 3 {
            last = save_pasted(&dir, 3, 2, &rgba).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let n = std::fs::read_dir(&dir).unwrap().count();
        assert!(n <= KEEP_PASTED, "{n} files kept");
        let bytes = std::fs::read(&last).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert!(save_pasted(&dir, 3, 2, &rgba[..5]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

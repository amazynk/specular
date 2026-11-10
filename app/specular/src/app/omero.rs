//! OMERO servers… (palette, File group): browsing OMERO servers and the Image Data Resource
//! (IDR), read-only, and opening their images.
//!
//! A modal sheet drawn by the DICOMweb sheet's view ([`super::dicomweb::view`]), with three
//! pages:
//!
//! - **Servers**: the list (the IDR is offered when the list is empty), with Add, Edit,
//!   Remove and Browse. Nothing is sent to a server until the user browses it.
//! - **Server**: name, OMERO.web address and an optional user name and password. The list is
//!   saved in the config folder without passwords, which stay in memory until Specular
//!   closes. Without a user name the server is browsed as its public user.
//! - **Browse**: Projects → Datasets → Images and Screens → Plates → Wells → Images, a page
//!   at a time (Load more), images and wells with their server's thumbnails (fetched in the
//!   background through the disk cache).
//!
//! Opening an image writes a small `.omero` descriptor (server, image id, names; no secrets)
//! for each image of the listing into the cache folder and opens the chosen one, so the
//! arrow keys step through the dataset. The decoder ([`specular_omero::session`]) fetches the
//! pyramid's tiles as the view needs them; Z and T planes are pages, and the channel keys
//! ask the server for another rendering ([`App::omero_toggle_channel`]). When a session ends
//! and cannot be renewed, the sheet opens on the server for its password.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::{DrawList, Gpu, Rect, TexId, Text, UiRenderer, wgpu};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_omero::descriptor::safe_name;
use specular_omero::{
    Context, Descriptor, Kind, Listing, Node, Omero, OmeroError, Rgba, Server, ServerList,
    registry, tiles,
};
use specular_remote::{CancelToken, Secret};
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::App;
use super::dicomweb::view::{
    self, A11Y_BASE, Button, ButtonView, Control, DialogView, Drawn, FieldId, FormItem, Hit,
    ListEntry, Page, Thumb, Tone,
};
use super::integration::PasteTo;

/// Rows asked for at a time.
const PAGE_SIZE: u64 = 100;
/// Thumbnails' longest side, in pixels (the server renders them).
const THUMB: u32 = 96;
/// Thumbnails fetched at once.
const THUMB_REQUESTS: usize = 4;

/// What a list row stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Server(usize),
    Node(usize),
    More,
}

/// A listing of a level, and how far it was read.
#[derive(Clone, Debug)]
struct Source {
    listing: Listing,
    next: u64,
    /// All its rows (`None`: not read yet).
    total: Option<u64>,
}

/// One level of the browse path: the server's top (projects and screens), a container's
/// children, or a well's fields.
#[derive(Clone, Debug, Default)]
struct Level {
    /// The container's name (the server's for the top).
    title: String,
    /// What it is (`None`: the server's top).
    kind: Option<Kind>,
    context: Context,
    sources: Vec<Source>,
    rows: Vec<Node>,
}

impl Level {
    fn more(&self) -> bool {
        self.sources
            .iter()
            .any(|s| s.total.is_some_and(|t| s.next < t))
    }
}

/// A listing's answer.
struct Outcome {
    generation: u64,
    append: bool,
    /// The connection was made by this job (it is kept when this succeeded).
    connected: bool,
    res: Result<Vec<(usize, specular_omero::Page)>, OmeroError>,
}

struct Job {
    rx: Receiver<Outcome>,
    cancel: CancelToken,
}

/// The Server page's form.
struct Edit {
    /// The server edited (`None`: a new one).
    id: Option<String>,
    name: LineEdit,
    url: LineEdit,
    user: LineEdit,
    password: LineEdit,
    /// A password is kept for this server: an empty field keeps it.
    has_password: bool,
    /// Save was pressed: problems with the address are shown.
    tried: bool,
}

/// The open sheet.
pub(super) struct Dialog {
    page: Page,
    /// The server browsed or selected (index into the list).
    server: Option<usize>,
    edit: Option<Edit>,
    /// The connection browsed, and whether it still has to connect (with the sign-in).
    omero: Option<Arc<Omero>>,
    connect: Option<Option<(String, Secret)>>,
    levels: Vec<Level>,
    /// Bumped when the levels change: answers for older ones are dropped.
    generation: u64,
    selected: Option<Row>,
    focus: Option<usize>,
    scroll: usize,
    message: Option<(String, Tone)>,
    job: Option<Job>,
    thumbs: Option<CancelToken>,
    drawn: Drawn,
    /// What an input method is composing in the focused field (IME preedit).
    preedit: String,
}

impl Dialog {
    fn new() -> Dialog {
        Dialog {
            page: Page::Servers,
            server: None,
            edit: None,
            omero: None,
            connect: None,
            levels: Vec::new(),
            generation: 0,
            selected: None,
            focus: None,
            scroll: 0,
            message: None,
            job: None,
            thumbs: None,
            drawn: Drawn::default(),
            preedit: String::new(),
        }
    }

    fn slots(&self) -> Vec<FieldId> {
        match self.page {
            Page::Edit => vec![FieldId::Name, FieldId::Url, FieldId::User, FieldId::Secret],
            _ => Vec::new(),
        }
    }

    fn rows(&self, servers: &ServerList) -> Vec<Row> {
        match self.page {
            Page::Servers => (0..servers.servers.len()).map(Row::Server).collect(),
            // The DICOM servers' Send page has no OMERO counterpart.
            Page::Edit | Page::Send => Vec::new(),
            Page::Search => {
                let Some(l) = self.levels.last() else {
                    return Vec::new();
                };
                let mut v: Vec<Row> = (0..l.rows.len()).map(Row::Node).collect();
                if l.more() {
                    v.push(Row::More);
                }
                v
            }
        }
    }

    fn field(&self, id: FieldId) -> Option<&LineEdit> {
        let e = self.edit.as_ref()?;
        Some(match id {
            FieldId::Name => &e.name,
            FieldId::Url => &e.url,
            FieldId::User => &e.user,
            FieldId::Secret => &e.password,
            _ => return None,
        })
    }

    fn field_mut(&mut self, id: FieldId) -> Option<&mut LineEdit> {
        let e = self.edit.as_mut()?;
        Some(match id {
            FieldId::Name => &mut e.name,
            FieldId::Url => &mut e.url,
            FieldId::User => &mut e.user,
            FieldId::Secret => &mut e.password,
            _ => return None,
        })
    }

    fn focused_field_mut(&mut self, v: &DialogView) -> Option<&mut LineEdit> {
        let id = v.focused_field()?;
        self.field_mut(id)
    }

    /// The address as typed, checked.
    fn url_check(&self) -> Option<Option<String>> {
        self.edit
            .as_ref()
            .map(|e| specular_omero::api::check_url(e.url.text()))
    }

    /// Stops the listing and the thumbnails in flight.
    fn stop(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel.cancel();
        }
        if let Some(c) = self.thumbs.take() {
            c.cancel();
        }
    }
}

/// A thumbnail on the GPU.
struct Shown {
    _tex: wgpu::Texture,
    id: TexId,
    w: u32,
    h: u32,
}

/// A thumbnail's key: the server's address and the image.
type ThumbKey = (String, u64);

/// The app's OMERO state.
pub(super) struct OmeroState {
    pub dialog: Option<Dialog>,
    servers: ServerList,
    loaded: bool,
    /// Where the list is saved (`None`: kept in memory, as in screenshots).
    servers_file: Option<PathBuf>,
    /// Where descriptors are written (`None`: the app's cache folder).
    files_dir: Option<PathBuf>,
    /// Passwords given this session, by server id (never written anywhere).
    passwords: HashMap<String, Secret>,
    thumb_tx: Sender<(ThumbKey, Option<Rgba>)>,
    thumb_rx: Receiver<(ThumbKey, Option<Rgba>)>,
    asked: HashSet<ThumbKey>,
    pending: Vec<(ThumbKey, Rgba)>,
    shown: HashMap<ThumbKey, Shown>,
    /// `--show omero…`: the synthetic server the demo points at.
    demo: Option<specular_omero::fake::FakeServer>,
}

impl Default for OmeroState {
    fn default() -> Self {
        let (thumb_tx, thumb_rx) = crossbeam_channel::unbounded();
        OmeroState {
            dialog: None,
            servers: ServerList::default(),
            loaded: false,
            servers_file: None,
            files_dir: None,
            passwords: HashMap::new(),
            thumb_tx,
            thumb_rx,
            asked: HashSet::new(),
            pending: Vec::new(),
            shown: HashMap::new(),
            demo: None,
        }
    }
}

impl std::fmt::Debug for OmeroState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OmeroState")
            .field("open", &self.dialog.is_some())
            .field("servers", &self.servers.servers.len())
            .field("thumbnails", &self.shown.len())
            .finish_non_exhaustive()
    }
}

/// Why a request failed, for people.
fn error_text(e: &OmeroError, server: &str) -> String {
    match e {
        OmeroError::Cancelled => t!("omero-stopped"),
        OmeroError::Offline(_) => t!("omero-error-offline", server = server.to_string()),
        OmeroError::SignInRequired => t!("omero-error-sign-in"),
        OmeroError::Expired => t!("omero-error-expired"),
        OmeroError::LoginRefused => t!("omero-error-login"),
        OmeroError::Forbidden => t!("omero-error-forbidden"),
        OmeroError::NotFound => t!("omero-error-not-found"),
        OmeroError::Status(c) => t!("omero-error-status", code = i64::from(*c)),
        OmeroError::Malformed(m) => t!("omero-error-malformed", reason = m.clone()),
        OmeroError::NetworkOff => t!("network-off"),
    }
}

/// What a level lists, for its heading.
fn level_heading(l: &Level) -> String {
    match l.kind {
        None => t!("omero-top"),
        Some(Kind::Project) => t!("omero-datasets"),
        Some(Kind::Screen) => t!("omero-plates"),
        Some(Kind::Plate) => t!("omero-wells"),
        Some(Kind::Well) => t!("omero-fields"),
        Some(Kind::Dataset | Kind::Image) => t!("omero-images"),
    }
}

fn kind_text(k: Kind) -> String {
    match k {
        Kind::Project => t!("omero-kind-project"),
        Kind::Dataset => t!("omero-kind-dataset"),
        Kind::Image => t!("omero-kind-image"),
        Kind::Screen => t!("omero-kind-screen"),
        Kind::Plate => t!("omero-kind-plate"),
        Kind::Well => t!("omero-kind-well"),
    }
}

/// A row's text: the name, then what it is and holds.
fn node_entry(n: &Node, selected: bool, thumb: Thumb) -> ListEntry {
    let mut detail = vec![kind_text(n.kind)];
    match n.kind {
        Kind::Image => {
            if let Some(b) = &n.image {
                let [x, y, z, c, tt] = b.size;
                detail.push(format!("{x} × {y}"));
                let mut dims = Vec::new();
                if z > 1 {
                    dims.push(format!("Z {z}"));
                }
                if c > 1 {
                    dims.push(format!("C {c}"));
                }
                if tt > 1 {
                    dims.push(format!("T {tt}"));
                }
                if !dims.is_empty() {
                    detail.push(dims.join(" "));
                }
                if !b.pixels_type.is_empty() {
                    detail.push(b.pixels_type.clone());
                }
            }
        }
        Kind::Well => {
            let fields = n.well.as_ref().map_or(0, |w| w.images.len());
            detail.push(t!("omero-well-fields", count = fields as i64));
        }
        _ => {
            if let Some(c) = n.children {
                detail.push(t!("omero-children", count = c as i64));
            }
        }
    }
    if let Some(o) = &n.owner {
        detail.push(o.clone());
    }
    let title = match (&n.well, n.kind) {
        (Some(w), Kind::Well) => t!("omero-well", well = w.label()),
        _ if n.name.trim().is_empty() => format!("#{}", n.id),
        _ => n.name.trim().to_string(),
    };
    ListEntry {
        title,
        detail: specular_i18n::dot_join(&detail),
        selected,
        thumb,
        ..ListEntry::default()
    }
}

fn button(id: Button, label: String, enabled: bool, default: bool) -> ButtonView {
    ButtonView {
        id,
        label,
        enabled,
        default,
    }
}

/// Builds what the sheet draws.
fn build_view(
    d: &Dialog,
    servers: &ServerList,
    thumb: &dyn Fn(&Node) -> Thumb,
    persistent: bool,
) -> DialogView {
    let busy = d.job.is_some();
    let mut form = Vec::new();
    for id in d.slots() {
        let f = d.field(id);
        let text = f.map(|f| f.text().to_string()).unwrap_or_default();
        let sel = f.map_or(0..0, |f| f.selection());
        let kept = d.edit.as_ref().is_some_and(|e| e.has_password);
        let (label, hint, masked) = match id {
            FieldId::Name => (t!("omero-field-name"), t!("omero-hint-name"), false),
            FieldId::Url => (t!("omero-field-url"), t!("omero-hint-url"), false),
            FieldId::User => (t!("omero-field-user"), t!("omero-hint-user"), false),
            _ => (
                t!("omero-field-password"),
                if kept {
                    t!("omero-hint-password-kept")
                } else {
                    String::new()
                },
                true,
            ),
        };
        form.push(FormItem {
            label,
            control: Control::Field {
                id,
                text,
                caret: f.map_or(0, LineEdit::caret),
                selection: (sel.start, sel.end),
                masked,
                hint,
                preedit: d.preedit.clone(),
            },
        });
    }
    let level = d.levels.last();
    let list: Vec<ListEntry> = d
        .rows(servers)
        .iter()
        .map(|r| {
            let selected = d.selected == Some(*r);
            match *r {
                Row::Server(i) => {
                    let s = &servers.servers[i];
                    let mut detail = vec![s.url.clone()];
                    detail.push(if s.is_public() {
                        t!("omero-public")
                    } else {
                        t!("omero-signed-in-as", user = s.user.trim().to_string())
                    });
                    if s.url.starts_with("http://") {
                        detail.push(t!("omero-not-encrypted"));
                    }
                    ListEntry {
                        title: s.label(),
                        detail: specular_i18n::dot_join(&detail),
                        selected,
                        ..ListEntry::default()
                    }
                }
                Row::Node(i) => match level.and_then(|l| l.rows.get(i)) {
                    Some(n) => node_entry(n, selected, thumb(n)),
                    None => ListEntry::default(),
                },
                Row::More => ListEntry {
                    title: t!("omero-load-more"),
                    action: true,
                    ..ListEntry::default()
                },
            }
        })
        .collect();
    let mut notes = Vec::new();
    let (title, subtitle, list_title, empty, buttons) = match d.page {
        Page::Servers => {
            if !persistent && !servers.servers.is_empty() {
                notes.push((t!("omero-session-only"), Tone::Plain));
            }
            let sel = matches!(d.selected, Some(Row::Server(_)));
            (
                t!("omero-title"),
                Some(t!("omero-servers-intro")),
                Some(t!("omero-servers")),
                Some(t!("omero-servers-empty")),
                vec![
                    button(Button::Add, t!("omero-add"), true, false),
                    button(Button::Edit, t!("omero-edit"), sel, false),
                    button(Button::Remove, t!("omero-remove"), sel, false),
                    button(Button::Close, t!("omero-close"), true, false),
                    button(Button::Browse, t!("omero-browse"), sel, true),
                ],
            )
        }
        Page::Edit => {
            let e = d.edit.as_ref();
            let check = d.url_check().flatten();
            if let Some(e) = e
                && check.is_none()
                && (e.tried || !e.url.text().trim().is_empty())
            {
                notes.push((t!("omero-url-invalid"), Tone::Error));
            }
            if let (Some(u), Some(e)) = (&check, e)
                && !e.user.text().trim().is_empty()
                && u.starts_with("http://")
            {
                notes.push((t!("omero-http-warning"), Tone::Warning));
            }
            if e.is_some_and(|e| !e.user.text().trim().is_empty()) {
                notes.push((t!("omero-password-session"), Tone::Plain));
            }
            (
                if e.is_some_and(|e| e.id.is_some()) {
                    t!("omero-edit-title")
                } else {
                    t!("omero-add-title")
                },
                None,
                None,
                None,
                vec![
                    button(Button::Cancel, t!("app-cancel"), true, false),
                    button(Button::Save, t!("omero-save"), check.is_some(), true),
                ],
            )
        }
        Page::Send => (String::new(), None, None, None, Vec::new()),
        Page::Search => {
            let mut b = vec![
                button(Button::Back, t!("omero-back"), true, false),
                button(Button::Close, t!("omero-close"), true, false),
            ];
            if busy {
                b.push(button(Button::Stop, t!("omero-stop"), true, false));
            }
            let can_open = matches!(d.selected, Some(Row::Node(_)));
            b.push(button(Button::Browse, t!("omero-open"), can_open, true));
            let server = d.server.and_then(|i| servers.servers.get(i));
            let mut path: Vec<String> = server.map(Server::label).into_iter().collect();
            path.extend(d.levels.iter().skip(1).map(|l| l.title.clone()));
            (
                t!("omero-browse-title"),
                Some(I18n::global().join(&path, " › ")),
                level.map(level_heading),
                Some(if busy {
                    t!("omero-loading")
                } else {
                    t!("omero-empty")
                }),
                b,
            )
        }
    };
    let status = if busy {
        Some((t!("omero-loading"), Tone::Plain))
    } else {
        d.message.clone()
    };
    DialogView {
        page: d.page,
        title,
        subtitle,
        form,
        list_title,
        list,
        empty,
        scroll: d.scroll,
        notes,
        status,
        progress: None,
        buttons,
        focus: d.focus,
    }
}

impl OmeroState {
    /// The GPU device was replaced: the thumbnails are fetched and uploaded again.
    pub(super) fn gpu_reset(&mut self) {
        self.shown.clear();
        self.asked.clear();
    }

    fn persistent(&self) -> bool {
        self.servers_file.is_some()
    }

    fn thumb_of(&self, base: &str, n: &Node) -> Thumb {
        let Some(id) = n.image_id() else {
            return Thumb::None;
        };
        match self.shown.get(&(base.to_string(), id)) {
            Some(s) => Thumb::Shown {
                tex: s.id,
                w: s.w,
                h: s.h,
            },
            None => Thumb::Pending,
        }
    }

    /// The sheet as the draw function sees it.
    pub(super) fn view(&self) -> Option<DialogView> {
        let d = self.dialog.as_ref()?;
        let base = d.omero.as_ref().map(|o| o.base().to_string());
        let thumb = |n: &Node| match &base {
            Some(b) => self.thumb_of(b, n),
            None => Thumb::None,
        };
        Some(build_view(d, &self.servers, &thumb, self.persistent()))
    }

    /// Draws the sheet `v` (when open) over everything.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&DialogView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = view::draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }

    /// The focused field (for the input method's window).
    pub(super) fn ime_area(&self) -> Option<Rect> {
        let d = self.dialog.as_ref().filter(|_| self.wants_text())?;
        d.drawn.ime.or(d.drawn.field)
    }

    /// What the input method composes in the focused field (empty: nothing).
    pub(super) fn set_preedit(&mut self, s: &str) {
        let wants = self.wants_text();
        if let Some(d) = self.dialog.as_mut() {
            if wants {
                s.clone_into(&mut d.preedit);
            } else {
                d.preedit.clear();
            }
        }
    }

    /// True while a text field of the sheet has the keyboard.
    pub(super) fn wants_text(&self) -> bool {
        self.view().is_some_and(|v| v.focused_field().is_some())
    }

    /// Uploads the thumbnails that came since the last frame; true when there were some.
    pub(super) fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) -> bool {
        while let Ok((key, img)) = self.thumb_rx.try_recv() {
            if let Some(img) = img {
                self.pending.push((key, img));
            }
        }
        if self.pending.is_empty() {
            return false;
        }
        for (key, img) in self.pending.drain(..) {
            if img.width == 0 || img.height == 0 {
                continue;
            }
            let (tex, id) = super::studies::upload_rgba(gpu, ui, img.width, img.height, &img.data);
            let s = Shown {
                _tex: tex,
                id,
                w: img.width,
                h: img.height,
            };
            if let Some(old) = self.shown.insert(key, s) {
                ui.unregister_texture(old.id);
            }
        }
        true
    }

    fn load_servers(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if self.servers_file.is_none() && self.demo.is_none() {
            self.servers_file = Some(
                specular_settings::Locations::detect()
                    .config_dir
                    .join("omero.json"),
            );
        }
        let exists = self.servers_file.as_ref().is_some_and(|p| p.exists());
        if let Some(p) = &self.servers_file {
            self.servers = ServerList::load(p);
        }
        // The IDR is offered until the user saves a list of their own.
        if !exists && self.servers.servers.is_empty() {
            self.servers.upsert(Server::idr());
        }
    }

    fn save_servers(&self) -> Option<String> {
        let p = self.servers_file.as_ref()?;
        self.servers
            .save(p)
            .err()
            .map(|e| crate::error_text::reason(&e))
    }

    fn files_dir(&self) -> PathBuf {
        self.files_dir.clone().unwrap_or_else(|| {
            specular_settings::Locations::detect()
                .cache_dir
                .join("omero")
                .join("images")
        })
    }
}

/// Writes the descriptors of `images` into `dir`; the path of the one of `want`.
fn write_descriptors(dir: &Path, images: &[Descriptor], want: u64) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut found = None;
    for d in images {
        let p = dir.join(d.file_name());
        let bytes = d.to_json();
        // Unchanged files are left alone (an open image keeps its file).
        if std::fs::read(&p).ok().as_deref() != Some(bytes.as_slice()) {
            let mut tmp = p.as_os_str().to_owned();
            tmp.push(".tmp");
            std::fs::write(&tmp, &bytes)?;
            std::fs::rename(&tmp, &p)?;
        }
        if d.image == want {
            found = Some(p);
        }
    }
    found.ok_or_else(|| std::io::Error::other("image not listed"))
}

impl App {
    pub(super) fn omero_view(&self) -> Option<DialogView> {
        self.omero.view()
    }

    pub(super) fn omero_open(&mut self) {
        if self.omero.dialog.is_some() {
            return;
        }
        self.omero.load_servers();
        let mut d = Dialog::new();
        if !self.omero.servers.servers.is_empty() {
            d.server = Some(0);
            d.selected = Some(Row::Server(0));
        }
        self.omero.dialog = Some(d);
        self.omero_focus_default();
        self.chrome.announce_status(t!("omero-title"));
        self.redraw = true;
    }

    fn omero_close(&mut self) {
        if let Some(mut d) = self.omero.dialog.take() {
            d.stop();
        }
        self.redraw = true;
    }

    /// Focus on the first field, else the selected row, else the default button.
    fn omero_focus_default(&mut self) {
        let Some(v) = self.omero_view() else {
            return;
        };
        let f = v.focusables();
        let selected = v.list.iter().position(|e| e.selected).map(Hit::Entry);
        let target = if !v.form.is_empty() {
            Some(Hit::Form(0))
        } else if let Some(s) = selected {
            Some(s)
        } else if !v.list.is_empty() {
            Some(Hit::Entry(0))
        } else {
            v.buttons
                .iter()
                .find(|b| b.default && b.enabled)
                .or_else(|| v.buttons.iter().find(|b| b.enabled))
                .map(|b| Hit::Button(b.id))
        };
        if let Some(d) = self.omero.dialog.as_mut() {
            d.focus = target.and_then(|t| f.iter().position(|x| *x == t));
        }
    }

    fn omero_set_focus(&mut self, h: Hit) {
        let Some(v) = self.omero_view() else {
            return;
        };
        if let Some(d) = self.omero.dialog.as_mut() {
            d.focus = v.focusables().iter().position(|x| *x == h);
            if let Hit::Entry(k) = h {
                let vis = d.drawn.visible.max(1);
                if k < d.scroll {
                    d.scroll = k;
                } else if k >= d.scroll + vis {
                    d.scroll = k + 1 - vis;
                }
                // The focused row is the selected one.
                let rows = d.rows(&self.omero.servers);
                if let Some(&r) = rows.get(k)
                    && r != Row::More
                {
                    d.selected = Some(r);
                    if let Row::Server(i) = r {
                        d.server = Some(i);
                    }
                }
            }
        }
        self.redraw = true;
    }

    fn omero_page(&mut self, page: Page) {
        if let Some(d) = self.omero.dialog.as_mut() {
            d.page = page;
            d.scroll = 0;
            d.message = None;
            if page == Page::Servers {
                d.stop();
                d.selected = d.server.map(Row::Server);
                d.edit = None;
                d.levels.clear();
            }
        }
        self.omero_focus_default();
        if let Some(v) = self.omero_view() {
            self.chrome.announce_status(v.title);
        }
        self.redraw = true;
    }

    fn omero_edit(&mut self, existing: Option<usize>) {
        let server = existing.and_then(|i| self.omero.servers.servers.get(i).cloned());
        let has_password = server
            .as_ref()
            .is_some_and(|s| self.omero.passwords.contains_key(&s.id));
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        d.stop();
        let s = server.unwrap_or_default();
        d.edit = Some(Edit {
            id: (!s.id.is_empty()).then(|| s.id.clone()),
            name: LineEdit::new(&s.name),
            url: LineEdit::new(&s.url),
            user: LineEdit::new(&s.user),
            password: LineEdit::new(""),
            has_password,
            tried: false,
        });
        d.page = Page::Edit;
        d.message = None;
        self.omero_focus_default();
        self.chrome
            .announce_status(self.omero_view().map(|v| v.title).unwrap_or_default());
        self.redraw = true;
    }

    fn omero_save(&mut self) {
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        let Some(e) = d.edit.as_mut() else { return };
        e.tried = true;
        let Some(url) = specular_omero::api::check_url(e.url.text()) else {
            self.chrome.announce_status(t!("omero-url-invalid"));
            self.redraw = true;
            return;
        };
        let server = Server {
            id: e.id.clone().unwrap_or_default(),
            name: e.name.text().trim().to_string(),
            url,
            user: e.user.text().trim().to_string(),
        };
        let password = e.password.text().to_string();
        let public = server.is_public();
        let old =
            e.id.as_ref()
                .and_then(|id| self.omero.servers.get(id))
                .cloned();
        let id = self.omero.servers.upsert(server.clone());
        // Another address or user: the session there is not this server's any more.
        if let Some(o) = old
            && (o.url != server.url || o.user != server.user)
        {
            registry::remove(&o.url);
            self.omero.passwords.remove(&id);
        }
        if public {
            self.omero.passwords.remove(&id);
        } else if !password.is_empty() {
            self.omero
                .passwords
                .insert(id.clone(), Secret::new(password));
            // A new password: sign in afresh.
            registry::remove(&server.url);
        }
        if let Some(err) = self.omero.save_servers() {
            let now = self.now();
            self.chrome
                .warning(t!("omero-save-failed", reason = err), now);
        }
        let i = self.omero.servers.servers.iter().position(|s| s.id == id);
        if let Some(d) = self.omero.dialog.as_mut() {
            d.server = i;
            d.edit = None;
        }
        self.omero_page(Page::Servers);
    }

    fn omero_remove(&mut self) {
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        let Some(Row::Server(i)) = d.selected else {
            return;
        };
        let Some(s) = self.omero.servers.servers.get(i).cloned() else {
            return;
        };
        self.omero.servers.remove(&s.id);
        self.omero.passwords.remove(&s.id);
        registry::remove(&s.url);
        let n = self.omero.servers.servers.len();
        let next = (n > 0).then(|| i.min(n - 1));
        d.server = next;
        d.selected = next.map(Row::Server);
        if let Some(err) = self.omero.save_servers() {
            let now = self.now();
            self.chrome
                .warning(t!("omero-save-failed", reason = err), now);
        }
        self.chrome
            .announce_status(t!("omero-removed", server = s.label()));
        self.omero_focus_default();
        self.redraw = true;
    }

    /// Browse: the top of the selected server (its password first when it is needed).
    fn omero_browse(&mut self) {
        let Some(d) = self.omero.dialog.as_ref() else {
            return;
        };
        let Some(i) = d.server else { return };
        let Some(s) = self.omero.servers.servers.get(i).cloned() else {
            return;
        };
        let password = self.omero.passwords.get(&s.id).cloned();
        if !s.is_public() && password.is_none() {
            self.omero_ask_password(i, t!("omero-password-needed"));
            return;
        }
        let want_user = (!s.is_public()).then(|| s.user.trim().to_string());
        let (omero, connect) = match registry::get(&s.url).filter(|o| o.user() == want_user) {
            Some(o) => (o, None),
            None => {
                let made = registry::client().and_then(|c| Omero::new(c, &s.url));
                match made {
                    Ok(o) => (o, Some(want_user.zip(password))),
                    Err(e) => {
                        let m = error_text(&e, &s.label());
                        if let Some(d) = self.omero.dialog.as_mut() {
                            d.message = Some((m.clone(), Tone::Error));
                        }
                        self.chrome.announce_status(m);
                        self.redraw = true;
                        return;
                    }
                }
            }
        };
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        d.stop();
        d.omero = Some(omero);
        d.connect = connect;
        d.levels = vec![Level {
            title: s.label(),
            kind: None,
            context: Context::default(),
            sources: [Listing::Projects, Listing::Screens]
                .into_iter()
                .map(|listing| Source {
                    listing,
                    next: 0,
                    total: None,
                })
                .collect(),
            rows: Vec::new(),
        }];
        d.generation += 1;
        d.page = Page::Search;
        d.selected = None;
        d.scroll = 0;
        d.message = None;
        self.omero_fetch(false);
        self.omero_focus_default();
        self.chrome.announce_status(t!("omero-browse-title"));
        self.redraw = true;
    }

    /// The Server page of server `i`, focused on its password, with `why`.
    fn omero_ask_password(&mut self, i: usize, why: String) {
        self.omero_edit(Some(i));
        if let Some(d) = self.omero.dialog.as_mut() {
            d.message = Some((why.clone(), Tone::Warning));
            d.focus = d.slots().iter().position(|s| *s == FieldId::Secret);
        }
        self.chrome.announce_status(why);
        self.redraw = true;
    }

    /// Reads the current level's listings from the start, or (`append`) its next page.
    fn omero_fetch(&mut self, append: bool) {
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        let Some(omero) = d.omero.clone() else { return };
        let Some(level) = d.levels.last() else { return };
        let want: Vec<(usize, Listing, u64)> = if append {
            level
                .sources
                .iter()
                .enumerate()
                .find(|(_, s)| s.total.is_some_and(|t| s.next < t))
                .map(|(k, s)| (k, s.listing, s.next))
                .into_iter()
                .collect()
        } else {
            level
                .sources
                .iter()
                .enumerate()
                .map(|(k, s)| (k, s.listing, 0))
                .collect()
        };
        if want.is_empty() {
            return;
        }
        if let Some(j) = d.job.take() {
            j.cancel.cancel();
        }
        let connect = d.connect.clone();
        let generation = d.generation;
        let cancel = CancelToken::new();
        let (tx, rx) = crossbeam_channel::bounded(1);
        d.job = Some(Job {
            rx,
            cancel: cancel.clone(),
        });
        self.loader.background("omero-list", move |l| {
            let run = || -> Result<Vec<(usize, specular_omero::Page)>, OmeroError> {
                if let Some(login) = &connect {
                    let login = login.as_ref().map(|(u, p)| (u.as_str(), p.clone()));
                    omero.connect(login, &cancel)?;
                }
                let mut out = Vec::new();
                for (k, listing, offset) in want {
                    out.push((k, omero.list(listing, offset, PAGE_SIZE, &cancel)?));
                }
                Ok(out)
            };
            let res = run();
            let _ = tx.send(Outcome {
                generation,
                append,
                connected: connect.is_some(),
                res,
            });
            l.waker()();
        });
        self.redraw = true;
    }

    fn omero_finished(&mut self, out: Outcome) {
        let label = self
            .omero
            .dialog
            .as_ref()
            .and_then(|d| d.server)
            .and_then(|i| self.omero.servers.servers.get(i))
            .map(Server::label)
            .unwrap_or_default();
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        d.job = None;
        if out.generation != d.generation {
            return;
        }
        let announce;
        match out.res {
            Ok(pages) => {
                if out.connected {
                    d.connect = None;
                    if let Some(o) = &d.omero {
                        registry::register(o.clone());
                    }
                }
                let Some(level) = d.levels.last_mut() else {
                    return;
                };
                if !out.append {
                    level.rows.clear();
                    d.scroll = 0;
                }
                let first_new = level.rows.len();
                for (k, page) in pages {
                    if let Some(s) = level.sources.get_mut(k) {
                        s.next = page.offset + page.items.len() as u64;
                        s.total = Some(page.total.max(s.next));
                    }
                    level.rows.extend(page.items);
                }
                d.message = None;
                announce = if level.rows.is_empty() {
                    t!("omero-empty")
                } else {
                    t!("omero-found", count = level.rows.len() as i64)
                };
                if out.append && first_new < level.rows.len() {
                    d.selected = Some(Row::Node(first_new));
                } else if !out.append && !level.rows.is_empty() {
                    d.selected = Some(Row::Node(0));
                }
                self.omero_thumbs();
            }
            Err(OmeroError::Cancelled) => announce = t!("omero-stopped"),
            Err(e) => {
                let m = error_text(&e, &label);
                let sign_in = out.connected && e.needs_sign_in();
                d.message = Some((m.clone(), Tone::Error));
                announce = m.clone();
                if sign_in && let Some(i) = d.server {
                    // A refused password is not kept.
                    if e == OmeroError::LoginRefused
                        && let Some(s) = self.omero.servers.servers.get(i)
                    {
                        self.omero.passwords.remove(&s.id);
                    }
                    self.omero_ask_password(i, m);
                    return;
                }
            }
        }
        self.chrome.announce_status(announce);
        let sel = self.omero.dialog.as_ref().and_then(|d| d.selected);
        if let Some(sel) = sel
            && let Some(d) = self.omero.dialog.as_ref()
            && let Some(k) = d.rows(&self.omero.servers).iter().position(|r| *r == sel)
        {
            self.omero_set_focus(Hit::Entry(k));
        } else if self.omero_view().is_some_and(|v| v.focused().is_none()) {
            self.omero_focus_default();
        }
        self.redraw = true;
    }

    /// Fetches the thumbnails of the listed images not asked for yet (a few at a time).
    fn omero_thumbs(&mut self) {
        let st = &mut self.omero;
        let Some(d) = st.dialog.as_mut() else { return };
        let Some(omero) = d.omero.clone() else { return };
        let Some(level) = d.levels.last() else { return };
        let base = omero.base().to_string();
        let ids: Vec<u64> = level
            .rows
            .iter()
            .filter_map(Node::image_id)
            .filter(|id| st.asked.insert((base.clone(), *id)))
            .collect();
        if ids.is_empty() {
            return;
        }
        let cancel = d.thumbs.get_or_insert_with(CancelToken::new).clone();
        let tx = st.thumb_tx.clone();
        let cache = registry::cache();
        self.loader.background("omero-thumbs", move |l| {
            let wake = l.waker();
            let next = AtomicUsize::new(0);
            let work = || {
                while !cancel.is_cancelled() {
                    let Some(&id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        return;
                    };
                    let r = tiles::thumbnail(&omero, cache.as_ref(), id, THUMB, &cancel);
                    if let Err(e) = &r {
                        specular_log::debug!(target: specular_log::UI, "omero thumbnail {id}: {e}");
                    }
                    let _ = tx.send(((base.clone(), id), r.ok()));
                    wake();
                }
            };
            std::thread::scope(|s| {
                for i in 0..THUMB_REQUESTS.min(ids.len()) {
                    specular_thread::spawn_scoped(s, format!("omero-thumb-{i}"), work);
                }
            });
        });
    }

    /// Opens container row `i` (its children as the next level), or an image.
    fn omero_enter(&mut self, i: usize) {
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        let Some(parent) = d.levels.last() else {
            return;
        };
        let Some(node) = parent.rows.get(i).cloned() else {
            return;
        };
        let mut context = parent.context.clone();
        let name = node.name.trim().to_string();
        let listing = match node.kind {
            Kind::Image => {
                self.omero_open_image(i);
                return;
            }
            Kind::Well => {
                let Some(w) = node.well.clone() else { return };
                if w.images.len() <= 1 {
                    self.omero_open_image(i);
                    return;
                }
                context.well = Some(w.label());
                // A well's fields are listed by the well itself.
                let rows = w
                    .images
                    .iter()
                    .map(|(id, n)| Node {
                        kind: Kind::Image,
                        id: *id,
                        name: if n.trim().is_empty() {
                            format!("{} #{id}", w.label())
                        } else {
                            n.clone()
                        },
                        description: String::new(),
                        owner: None,
                        group: None,
                        children: None,
                        image: None,
                        well: None,
                    })
                    .collect();
                d.stop();
                d.levels.push(Level {
                    title: t!("omero-well", well = w.label()),
                    kind: Some(Kind::Well),
                    context,
                    sources: Vec::new(),
                    rows,
                });
                d.generation += 1;
                d.selected = Some(Row::Node(0));
                d.scroll = 0;
                self.omero_thumbs();
                self.omero_focus_default();
                self.redraw = true;
                return;
            }
            Kind::Project => {
                context.project = Some(name.clone());
                Listing::Datasets { project: node.id }
            }
            Kind::Dataset => {
                context.dataset = Some(name.clone());
                Listing::Images { dataset: node.id }
            }
            Kind::Screen => {
                context.screen = Some(name.clone());
                Listing::Plates { screen: node.id }
            }
            Kind::Plate => {
                context.plate = Some(name.clone());
                Listing::Wells { plate: node.id }
            }
        };
        d.stop();
        d.levels.push(Level {
            title: if name.is_empty() {
                format!("#{}", node.id)
            } else {
                name
            },
            kind: Some(node.kind),
            context,
            sources: vec![Source {
                listing,
                next: 0,
                total: None,
            }],
            rows: Vec::new(),
        });
        d.generation += 1;
        d.selected = None;
        d.scroll = 0;
        d.message = None;
        self.omero_fetch(false);
        self.omero_focus_default();
        self.redraw = true;
    }

    /// Back: the level above, or the servers from the top.
    fn omero_up(&mut self) {
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        if d.levels.len() <= 1 {
            self.omero_page(Page::Servers);
            return;
        }
        d.stop();
        let left = d.levels.pop();
        d.generation += 1;
        d.message = None;
        // The row of the level left is selected again.
        let up = d.levels.last();
        let k = up.and_then(|l| {
            let left = left.as_ref()?;
            l.rows.iter().position(|n| {
                Some(n.kind) == left.kind
                    && (n.name.trim() == left.title
                        || n.well.as_ref().map(|w| t!("omero-well", well = w.label()))
                            == Some(left.title.clone()))
            })
        });
        d.selected = k.map(Row::Node);
        if up.is_some_and(|l| l.rows.is_empty() && !l.sources.is_empty()) {
            self.omero_fetch(false);
        }
        self.omero_thumbs();
        match k {
            Some(k) => self.omero_set_focus(Hit::Entry(k)),
            None => self.omero_focus_default(),
        }
        if let Some(v) = self.omero_view() {
            self.chrome.announce_status(v.subtitle.unwrap_or_default());
        }
        self.redraw = true;
    }

    /// Opens image row `i`: descriptors for the listing's images are written next to each
    /// other (so the next and previous image are the listing's) and the chosen one opens.
    fn omero_open_image(&mut self, i: usize) {
        let Some(d) = self.omero.dialog.as_ref() else {
            return;
        };
        let (Some(omero), Some(level)) = (d.omero.clone(), d.levels.last()) else {
            return;
        };
        let Some(want) = level.rows.get(i).and_then(Node::image_id) else {
            return;
        };
        let server = d
            .server
            .and_then(|s| self.omero.servers.servers.get(s))
            .map(Server::label)
            .unwrap_or_else(|| omero.base().to_string());
        let descriptors: Vec<Descriptor> = level
            .rows
            .iter()
            .filter_map(|n| {
                let image = n.image_id()?;
                let mut context = level.context.clone();
                let name = match (&n.well, n.kind) {
                    (Some(w), Kind::Well) => {
                        context.well = Some(w.label());
                        let field = w.images.first().map(|f| f.1.trim()).unwrap_or("");
                        if field.is_empty() {
                            w.label()
                        } else {
                            format!("{} {field}", w.label())
                        }
                    }
                    _ if n.name.trim().is_empty() => format!("#{image}"),
                    _ => n.name.trim().to_string(),
                };
                Some(Descriptor {
                    server: omero.base().to_string(),
                    user: omero.user(),
                    image,
                    name,
                    context,
                    owner: n.owner.clone(),
                    group: n.group.clone(),
                    ..Descriptor::default()
                })
            })
            .collect();
        let folder = self
            .omero
            .files_dir()
            .join(safe_name(&server))
            .join(safe_name(&format!(
                "{} ({})",
                level.title,
                level.context_key()
            )));
        match write_descriptors(&folder, &descriptors, want) {
            Ok(path) => {
                // The decoder finds the connection (signed in, or public) by address.
                if registry::get(omero.base()).is_none() {
                    registry::register(omero);
                }
                self.omero_close();
                self.open_target(path);
            }
            Err(e) => {
                let now = self.now();
                self.chrome.warning(
                    t!("omero-open-failed", reason = crate::error_text::reason(&e)),
                    now,
                );
            }
        }
        self.redraw = true;
    }

    /// The channel keys on an OMERO image: channel `k` (from 0) on or off in the server's
    /// rendering, kept in the image's descriptor, which opens again.
    pub(super) fn omero_toggle_channel(&mut self, facts: &specular_decode::OmeroFacts, k: usize) {
        let now = self.now();
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let Some(mut desc) = std::fs::read(&path)
            .ok()
            .and_then(|b| Descriptor::parse(&b))
        else {
            return;
        };
        let mut render = desc
            .render
            .clone()
            .unwrap_or_else(|| specular_omero::Render {
                channels: facts
                    .channels
                    .iter()
                    .map(|c| specular_omero::ChannelRender {
                        active: c.active,
                        start: c.window.0,
                        end: c.window.1,
                        colour: c.colour,
                    })
                    .collect(),
                greyscale: false,
            });
        if !render.toggle(k) {
            return;
        }
        let on = render.channels[k].active;
        if !render.channels.iter().any(|c| c.active) {
            self.chrome.toast(t!("omero-channel-last"), None, now);
            return;
        }
        desc.render = Some(render);
        desc.z = Some(facts.plane.0);
        desc.t = Some(facts.plane.1);
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let written =
            std::fs::write(&tmp, desc.to_json()).and_then(|()| std::fs::rename(&tmp, &path));
        if let Err(e) = written {
            self.chrome.warning(
                t!("omero-open-failed", reason = crate::error_text::reason(&e)),
                now,
            );
            return;
        }
        let channel = facts
            .channels
            .get(k)
            .map(|c| c.name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| (k + 1).to_string());
        let msg = if on {
            t!("science-channel-on", channel = channel)
        } else {
            t!("science-channel-off", channel = channel)
        };
        self.chrome.toast(msg, None, now);
        self.reload_path(&path);
    }

    /// A session ended while an image was shown and could not be renewed: the sheet opens on
    /// the server for its password.
    fn omero_sign_in_again(&mut self, base: &str) {
        self.omero.load_servers();
        let i = self
            .omero
            .servers
            .servers
            .iter()
            .position(|s| s.url == base && !s.is_public());
        let now = self.now();
        let Some(i) = i else {
            self.chrome.warning(t!("omero-error-sign-in"), now);
            return;
        };
        let label = self.omero.servers.servers[i].label();
        if let Some(s) = self.omero.servers.servers.get(i) {
            self.omero.passwords.remove(&s.id);
        }
        registry::remove(base);
        if self.omero.dialog.is_none() {
            self.omero_open();
        }
        if let Some(d) = self.omero.dialog.as_mut() {
            d.server = Some(i);
        }
        self.omero_ask_password(i, t!("omero-session-ended", server = label));
    }

    /// Listings' answers, thumbnails and requests to sign in again.
    pub(super) fn omero_tick(&mut self) {
        if let Some(base) = registry::take_sign_in_request() {
            self.omero_sign_in_again(&base);
        }
        if !self.omero.thumb_rx.is_empty() {
            self.redraw = true;
        }
        let Some(d) = self.omero.dialog.as_ref() else {
            return;
        };
        let Some(j) = &d.job else { return };
        match j.rx.try_recv() {
            Ok(out) => self.omero_finished(out),
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                if let Some(d) = self.omero.dialog.as_mut() {
                    d.job = None;
                }
                self.redraw = true;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
        }
    }

    /// A list row chosen (Enter, or a click on the selected row).
    fn omero_activate(&mut self, row: Row) {
        match row {
            Row::Server(i) => {
                if let Some(d) = self.omero.dialog.as_mut() {
                    d.server = Some(i);
                }
                self.omero_browse();
            }
            Row::Node(i) => self.omero_enter(i),
            Row::More => self.omero_fetch(true),
        }
        self.redraw = true;
    }

    fn omero_select(&mut self, k: usize) {
        let Some(d) = self.omero.dialog.as_mut() else {
            return;
        };
        let rows = d.rows(&self.omero.servers);
        let Some(&row) = rows.get(k) else { return };
        if row == Row::More {
            self.omero_activate(row);
            return;
        }
        let again = d.selected == Some(row);
        self.omero_set_focus(Hit::Entry(k));
        if again {
            self.omero_activate(row);
        }
    }

    fn omero_hit(&mut self, h: Hit) {
        match h {
            Hit::Form(_) => self.omero_set_focus(h),
            Hit::Entry(k) => self.omero_select(k),
            Hit::Button(b) => match b {
                Button::Close => self.omero_close(),
                Button::Add => self.omero_edit(None),
                Button::Edit => {
                    let sel = self.omero.dialog.as_ref().and_then(|d| d.selected);
                    if let Some(Row::Server(i)) = sel {
                        self.omero_edit(Some(i));
                    }
                }
                Button::Remove => self.omero_remove(),
                Button::Verify | Button::Send | Button::Confirm => {}
                Button::Browse => {
                    let sel = self.omero.dialog.as_ref().and_then(|d| d.selected);
                    match sel {
                        Some(r @ Row::Node(_)) => self.omero_activate(r),
                        _ => self.omero_browse(),
                    }
                }
                Button::Cancel => self.omero_page(Page::Servers),
                Button::Save => self.omero_save(),
                Button::Back => self.omero_up(),
                Button::Stop => {
                    if let Some(d) = self.omero.dialog.as_mut() {
                        d.stop();
                    }
                    self.chrome.announce_status(t!("omero-stopped"));
                }
                Button::Search => {}
            },
        }
        self.redraw = true;
    }

    /// Enter: the focused button or row, else the page's default.
    fn omero_enter_key(&mut self, v: &DialogView) {
        match v.focused() {
            Some(Hit::Entry(k)) => {
                let row = self
                    .omero
                    .dialog
                    .as_ref()
                    .and_then(|d| d.rows(&self.omero.servers).get(k).copied());
                if let Some(row) = row {
                    self.omero_activate(row);
                }
            }
            Some(h @ Hit::Button(_)) => self.omero_hit(h),
            _ => {
                if let Some(b) = v.buttons.iter().find(|b| b.default && b.enabled) {
                    self.omero_hit(Hit::Button(b.id));
                }
            }
        }
    }

    /// Editing keys in the focused field; false when the key is not one.
    fn omero_field_key(
        &mut self,
        v: &DialogView,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
    ) -> bool {
        let rtl = self
            .omero
            .dialog
            .as_ref()
            .and_then(|d| d.drawn.field_text.as_ref())
            .is_some_and(|(b, _)| b.rtl);
        let Some(d) = self.omero.dialog.as_mut() else {
            return false;
        };
        let Some(field) = d.focused_field_mut(v) else {
            return false;
        };
        if let Some(c) = combo {
            let mut clip = PendingClipboard::default();
            let handled = field.key(c.key, c.mods, rtl, &mut clip).used();
            if handled {
                self.field_clipboard(clip, PasteTo::Omero);
                self.redraw = true;
                return true;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return false;
            }
        }
        let Some(t) = text else { return false };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), Instant::now()));
        self.omero_text(&s);
        true
    }

    /// Text into the focused field (typed keys and input-method commits).
    pub(super) fn omero_text(&mut self, s: &str) {
        let Some(v) = self.omero_view() else {
            return;
        };
        if let Some(d) = self.omero.dialog.as_mut()
            && let Some(f) = d.focused_field_mut(&v)
            && f.insert(s)
        {
            self.redraw = true;
        }
    }

    /// Text pasted into the focused field.
    pub(super) fn omero_paste(&mut self, s: &str) {
        let Some(v) = self.omero_view() else {
            return;
        };
        if let Some(d) = self.omero.dialog.as_mut()
            && let Some(f) = d.focused_field_mut(&v)
        {
            f.paste(s);
            self.redraw = true;
        }
    }

    /// Keys while the sheet is open (it takes them all); `text` is what the key types.
    pub(super) fn omero_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let Some(v) = self.omero_view() else {
            return false;
        };
        let focus = v.focused();
        if v.focused_field().is_some() && self.omero_field_key(&v, combo, text) {
            return true;
        }
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len().max(1) as i32;
        let move_focus = |me: &mut App, d: i32| {
            let i = v.focus.map_or(0, |i| (i as i32 + d).rem_euclid(n) as usize);
            if let Some(h) = focusables.get(i).copied() {
                me.omero_set_focus(h);
            }
        };
        let rtl = self.rtl();
        let plain = combo.mods == Mods::NONE;
        let on_entry = matches!(focus, Some(Hit::Entry(_)));
        match combo.key {
            Key::Named(NamedKey::Escape) => {
                let busy = self.omero.dialog.as_ref().is_some_and(|d| d.job.is_some());
                if busy {
                    self.omero_hit(Hit::Button(Button::Stop));
                } else if v.page == Page::Edit {
                    self.omero_page(Page::Servers);
                } else {
                    self.omero_close();
                }
            }
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(NamedKey::Home) if plain && on_entry => {
                self.omero_set_focus(Hit::Entry(0));
            }
            Key::Named(NamedKey::End) if plain && on_entry => {
                self.omero_set_focus(Hit::Entry(v.list.len().saturating_sub(1)));
            }
            // In the browser, forward enters the row and backward goes up, as in a tree.
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight))
                if plain && on_entry && v.page == Page::Search =>
            {
                if (k == NamedKey::ArrowRight) != rtl {
                    self.omero_enter_key(&v);
                } else {
                    self.omero_up();
                }
            }
            Key::Named(NamedKey::Backspace) if plain && v.page == Page::Search => self.omero_up(),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                move_focus(self, if forward { 1 } else { -1 });
            }
            Key::Named(NamedKey::Space) => {
                if let Some(h) = focus {
                    self.omero_hit(h);
                }
            }
            Key::Named(NamedKey::Enter) => self.omero_enter_key(&v),
            Key::Named(NamedKey::Delete) if plain && v.page == Page::Servers => self.omero_remove(),
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A click while the sheet is open (it takes them all).
    pub(super) fn omero_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let (now, shift) = (self.now(), self.mods.shift_key());
        let Some(v) = self.omero_view() else {
            return false;
        };
        let Some(d) = self.omero.dialog.as_mut() else {
            return false;
        };
        if !down {
            if let Some(f) = d.focused_field_mut(&v) {
                f.release();
            }
            return true;
        }
        let hit = d
            .drawn
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        let Some(h) = hit else { return true };
        // A press in the focused field moves its caret.
        if v.focused() == Some(h)
            && let Some(id) = v.focused_field()
            && let Some((block, x0)) = d.drawn.field_text.clone()
            && id != FieldId::Secret
            && let Some(f) = d.field_mut(id)
        {
            f.press(block.hit_byte(p.0 - x0), p.0, now, shift);
            self.redraw = true;
            return true;
        }
        self.omero_hit(h);
        true
    }

    /// The pointer moved with the button down: a selection in the focused field.
    pub(super) fn omero_drag(&mut self, p: (f32, f32)) {
        let Some(v) = self.omero_view() else {
            return;
        };
        if let Some(d) = self.omero.dialog.as_mut()
            && let Some((block, x0)) = d.drawn.field_text.clone()
            && let Some(f) = d.focused_field_mut(&v)
            && f.dragging()
        {
            f.drag_to(block.hit_byte(p.0 - x0));
            self.redraw = true;
        }
    }

    /// The wheel scrolls the list.
    pub(super) fn omero_wheel(&mut self, lines: f32) {
        let Some(v) = self.omero_view() else {
            return;
        };
        if let Some(d) = self.omero.dialog.as_mut() {
            let max = v.list.len().saturating_sub(d.drawn.visible);
            let step = (-lines).round() as i64;
            d.scroll = (d.scroll as i64 + step).clamp(0, max as i64) as usize;
            self.redraw = true;
        }
    }

    /// A screen reader's request on one of the sheet's nodes; false when not ours.
    pub(super) fn omero_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.omero_view() else {
            return false;
        };
        let Some(hit) = view::a11y_target(&v, req.target_node.0) else {
            return (A11Y_BASE..A11Y_BASE + 0x10_0000).contains(&req.target_node.0);
        };
        match (req.action, hit) {
            (accesskit::Action::SetValue, Hit::Form(i)) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data
                    && let Some(d) = self.omero.dialog.as_mut()
                    && let Some(id) = d.slots().get(i).copied()
                    && let Some(f) = d.field_mut(id)
                {
                    f.set_text(s);
                }
            }
            (accesskit::Action::SetTextSelection, Hit::Form(i)) => {
                if let Some(accesskit::ActionData::SetTextSelection(sel)) = &req.data
                    && let Some(d) = self.omero.dialog.as_mut()
                    && let Some(id) = d.slots().get(i).copied()
                    && let Some(f) = d.field_mut(id)
                {
                    f.set_a11y_selection(sel.anchor.character_index, sel.focus.character_index);
                }
                self.omero_set_focus(hit);
            }
            (accesskit::Action::Click, h) => self.omero_hit(h),
            (accesskit::Action::Focus | accesskit::Action::ScrollIntoView, h) => {
                self.omero_set_focus(h)
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// `--show omero`, `omero-edit`, `omero-browse`, `omero-images`, `omero-wells`: the sheet
    /// against a synthetic server on loopback (`omero-image`: its tiled image opened) (nothing read from or written to the config;
    /// descriptors go to the folder in `SPECULAR_OMERO_DEMO_DIR`, else the cache folder).
    /// False when `scene` is not one of these.
    pub(super) fn omero_show(&mut self, scene: &str) -> bool {
        if !matches!(
            scene,
            "omero"
                | "omero-edit"
                | "omero-browse"
                | "omero-images"
                | "omero-wells"
                | "omero-image"
        ) {
            return false;
        }
        let fake = match specular_omero::fake::FakeServer::start(
            specular_omero::fake::FakeConfig::synthetic(12),
            specular_remote::testing::ServerOptions::default(),
        ) {
            Ok(f) => f,
            Err(e) => {
                specular_log::warn!(target: specular_log::UI, "omero demo server: {e}");
                return true;
            }
        };
        let st = &mut self.omero;
        st.loaded = true;
        st.servers_file = None;
        if let Some(dir) = std::env::var_os("SPECULAR_OMERO_DEMO_DIR") {
            st.files_dir = Some(PathBuf::from(dir));
        }
        st.servers = demo_servers(&fake.base());
        st.demo = Some(fake);
        self.omero_open();
        match scene {
            "omero-edit" => self.omero_edit(Some(2)),
            "omero-browse" | "omero-images" | "omero-wells" | "omero-image" => {
                self.omero_browse();
                self.omero_wait();
                let into: &[&str] = match scene {
                    "omero-images" => &["Synthetic project", "Gradients"],
                    "omero-image" => &["Synthetic project", "Gradients", "tiled slide.svs"],
                    "omero-wells" => &["Synthetic screen", "Plate 1"],
                    _ => &[],
                };
                for name in into {
                    let k =
                        self.omero.dialog.as_ref().and_then(|d| {
                            d.levels.last()?.rows.iter().position(|n| n.name == *name)
                        });
                    let Some(k) = k else { break };
                    self.omero_enter(k);
                    if self.omero.dialog.is_none() {
                        // An image opened.
                        return true;
                    }
                    self.omero_wait();
                }
                self.omero_wait_thumbs();
            }
            _ => {}
        }
        true
    }

    /// Screenshots: the listing in flight, waited for (loopback).
    fn omero_wait(&mut self) {
        let rx = self
            .omero
            .dialog
            .as_ref()
            .and_then(|d| d.job.as_ref())
            .map(|j| j.rx.clone());
        if let Some(rx) = rx
            && let Ok(out) = rx.recv_timeout(std::time::Duration::from_secs(10))
        {
            self.omero_finished(out);
        }
    }

    /// Screenshots: the listed thumbnails, waited for (loopback).
    fn omero_wait_thumbs(&mut self) {
        let want = self
            .omero
            .dialog
            .as_ref()
            .and_then(|d| d.levels.last())
            .map_or(0, |l| {
                l.rows.iter().filter(|n| n.image_id().is_some()).count()
            });
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let mut got = 0;
        while got < want && Instant::now() < deadline {
            match self
                .omero
                .thumb_rx
                .recv_timeout(std::time::Duration::from_millis(200))
            {
                Ok((key, img)) => {
                    got += 1;
                    if let Some(img) = img {
                        self.omero.pending.push((key, img));
                    }
                }
                Err(_) => continue,
            }
        }
    }
}

impl Level {
    /// A short key telling containers of the same name apart (in the descriptors' folder).
    fn context_key(&self) -> String {
        let c = &self.context;
        [&c.project, &c.dataset, &c.screen, &c.plate, &c.well]
            .iter()
            .filter_map(|x| x.as_deref())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The servers the demo shows: the synthetic server, the IDR preset and one with a user.
fn demo_servers(base: &str) -> ServerList {
    let mut l = ServerList::default();
    l.upsert(Server {
        name: "Synthetic OMERO".into(),
        url: base.to_string(),
        ..Server::default()
    });
    l.upsert(Server::idr());
    l.upsert(Server {
        name: "Institute OMERO".into(),
        url: "https://omero.example.org".into(),
        user: "reader".into(),
        ..Server::default()
    });
    l
}

#[cfg(test)]
mod shots;

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: Kind, id: u64, name: &str) -> Node {
        Node {
            kind,
            id,
            name: name.into(),
            description: String::new(),
            owner: None,
            group: None,
            children: None,
            image: None,
            well: None,
        }
    }

    #[test]
    fn a_level_lists_its_rows_then_load_more() {
        let mut d = Dialog::new();
        d.page = Page::Search;
        d.levels.push(Level {
            sources: vec![Source {
                listing: Listing::Projects,
                next: 2,
                total: Some(5),
            }],
            rows: vec![node(Kind::Project, 1, "a"), node(Kind::Project, 2, "b")],
            ..Level::default()
        });
        let rows = d.rows(&ServerList::default());
        assert_eq!(rows, vec![Row::Node(0), Row::Node(1), Row::More]);
        d.levels[0].sources[0].next = 5;
        assert_eq!(d.rows(&ServerList::default()).len(), 2);
    }

    #[test]
    fn images_have_thumbnails_and_containers_do_not() {
        let mut d = Dialog::new();
        d.page = Page::Search;
        d.levels.push(Level {
            kind: Some(Kind::Dataset),
            rows: vec![
                node(Kind::Image, 7, "cells.tif"),
                node(Kind::Dataset, 3, "d"),
            ],
            ..Level::default()
        });
        let thumb = |n: &Node| {
            if n.image_id().is_some() {
                Thumb::Pending
            } else {
                Thumb::None
            }
        };
        let v = build_view(&d, &ServerList::default(), &thumb, false);
        assert_eq!(v.list[0].thumb, Thumb::Pending);
        assert_eq!(v.list[1].thumb, Thumb::None);
        assert!(v.list[0].title.contains("cells.tif"));
    }

    #[test]
    fn the_password_is_asked_only_with_a_user_name() {
        let mut d = Dialog::new();
        d.page = Page::Edit;
        d.edit = Some(Edit {
            id: None,
            name: LineEdit::new(""),
            url: LineEdit::new("ftp://x"),
            user: LineEdit::new(""),
            password: LineEdit::new(""),
            has_password: false,
            tried: true,
        });
        let none = |_: &Node| Thumb::None;
        let v = build_view(&d, &ServerList::default(), &none, true);
        assert!(v.notes.iter().any(|(_, t)| *t == Tone::Error));
        assert!(!v.buttons.iter().any(|b| b.id == Button::Save && b.enabled));
        let e = d.edit.as_mut().unwrap();
        e.url.set_text("http://omero.local");
        e.user.set_text("alice");
        let v = build_view(&d, &ServerList::default(), &none, true);
        assert!(v.notes.iter().any(|(_, t)| *t == Tone::Warning));
        assert!(v.buttons.iter().any(|b| b.id == Button::Save && b.enabled));
    }

    #[test]
    fn descriptors_are_written_once_and_found() {
        let dir = std::env::temp_dir().join(format!("omero-sheet-{}", std::process::id()));
        let a = Descriptor {
            server: "https://idr.openmicroscopy.org".into(),
            image: 1,
            name: "a".into(),
            ..Descriptor::default()
        };
        let b = Descriptor {
            image: 2,
            name: "b/c".into(),
            ..a.clone()
        };
        let p = write_descriptors(&dir, &[a.clone(), b], 2).unwrap();
        assert!(p.ends_with("b_c (2).omero"));
        let back = Descriptor::parse(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(back.image, 2);
        assert!(write_descriptors(&dir, &[a], 9).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}

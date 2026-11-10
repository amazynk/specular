//! Local copies for offline editing, the Library's side:
//!
//! - **Keep a local copy…** of the location or album shown, or of the selected photos (the
//!   palette and the photo menu): asks whether to keep the originals or compact proxies of
//!   the raws, then copies in the background into the data folder.
//! - **Remove local copy…** asks first, naming the item and saying the originals stay.
//! - **Offline banner** over the grid: which location is offline, that local copies stand
//!   in, and how many changes wait.
//! - **Coming back:** when an offline location is online again, the waiting changes are
//!   written to the originals' sidecars through the usual writers. A change whose original
//!   or sidecar changed meanwhile is asked about, one file at a time: both versions are
//!   shown and the user keeps one. Nothing is overwritten unasked.
//! - **Settings › Library:** the space used and each kept item (with Remove…), under the
//!   size limit.
//! - **Export from a proxy** says so and offers to wait for the originals.
//!
//! The process-wide handle and the writers' hooks are `crate::app::local_copies`.

use std::sync::atomic::{AtomicBool, Ordering};

use specular_gfx::{DrawList, Text, TextStyle, draw_icon};
use specular_library::Location;
use specular_library::local::{
    self as lc, ChangeKind, Check, KeepProgress, KeepReport, Kept, LocalCopies, Mode, Scope,
};
use specular_widgets::{PanelEvent, PanelRow, SettingPanel};

use super::draw::{Frame, text_at, truncate};
use super::library::Side;
use super::*;
use crate::app::items;
use crate::app::local_copies as globals;

/// Palette and menu codes (inside the gallery's item range).
pub(super) mod code {
    /// Keep a local copy of the selected photos.
    pub const KEEP: u64 = 900;
    /// …of the location shown.
    pub const KEEP_LOCATION: u64 = 901;
    /// …of the album shown.
    pub const KEEP_ALBUM: u64 = 902;
    /// Remove the local copy of the selected photos (the kept selections they are in).
    pub const REMOVE: u64 = 903;
    pub const REMOVE_LOCATION: u64 = 904;
    pub const REMOVE_ALBUM: u64 = 905;
    /// Ask again about changes that met a conflict.
    pub const REVIEW: u64 = 906;

    pub fn owns(c: u64) -> bool {
        (900..950).contains(&c)
    }
}

/// The Settings panel's place (under the size limit).
pub(in crate::app) const PANEL: &str = "library.local_copies_limit";

/// A change that met a conflict, with both versions said.
#[derive(Clone, Debug)]
pub(in crate::app) struct Conflict {
    pub change: lc::Change,
    /// The file's name.
    pub file: String,
    /// What was changed here, and what the file says now.
    pub yours: String,
    pub theirs: String,
}

/// What background work tells the frame.
enum Msg {
    Opened(Result<Arc<LocalCopies>, String>),
    Progress(KeepProgress),
    Kept(String, KeepReport),
    Removed(String, u64),
    Synced {
        written: usize,
        failed: usize,
        conflicts: Vec<Conflict>,
    },
    /// A conflict answered: the marks the gallery shows now, when they changed.
    Resolved(Option<(PathBuf, Mark)>),
}

/// A keep being asked about.
struct KeepAsk {
    scope: Scope,
    name: String,
    files: Vec<PathBuf>,
}

#[derive(Default)]
pub(in crate::app) struct LocalView {
    pub(super) handle: Option<Arc<LocalCopies>>,
    opening: bool,
    failed: bool,
    chan: Option<(
        crossbeam_channel::Sender<Msg>,
        crossbeam_channel::Receiver<Msg>,
    )>,
    /// Kept items (Settings, the sidebar), as last listed.
    pub(super) kept: Vec<Kept>,
    /// Changes waiting, as last counted.
    pub(super) waiting: u64,
    /// Conflicts waiting for an answer; the one asked now.
    pub(super) conflicts: VecDeque<Conflict>,
    asking: Option<Conflict>,
    /// Locations offline at the last look.
    offline: HashSet<i64>,
    syncing: bool,
    /// A sync is due (a location came back, or the copies opened).
    sync_due: bool,
    /// A keep under way: its name and progress, and its stop.
    pub(super) keeping: Option<(String, KeepProgress)>,
    keep_stop: Option<Arc<AtomicBool>>,
    keep_ask: Option<KeepAsk>,
    remove_ask: Option<(i64, String)>,
    /// The export waits for its originals; or the user agreed to export from proxies.
    pub(in crate::app) export_wait: bool,
    pub(in crate::app) export_proxy_ok: bool,
    /// What the settings panel was built from.
    panel_built: Option<(u64, u64, usize, u64)>,
}

impl LocalView {
    fn tx(&mut self) -> crossbeam_channel::Sender<Msg> {
        self.chan
            .get_or_insert_with(crossbeam_channel::unbounded)
            .0
            .clone()
    }

    /// The kept item for a location or album.
    pub(super) fn kept_for(&self, scope: &Scope) -> Option<&Kept> {
        self.kept.iter().find(|k| k.scope == *scope)
    }
}

fn size_text(bytes: u64) -> String {
    I18n::global()
        .localizer()
        .bytes(bytes, specular_i18n::ByteUnits::Decimal)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// Marks in words ("4 stars · Green · Pick").
pub(in crate::app) fn marks_text(m: Mark) -> String {
    let mut parts = vec![t!("local-stars", count = i64::from(m.rating))];
    if m.label > 0 {
        parts.push(t!("local-label", label = i64::from(m.label)));
    }
    match m.flag {
        1 => parts.push(t!("local-pick")),
        -1 => parts.push(t!("local-reject")),
        _ => {}
    }
    specular_i18n::dot_join(&parts)
}

/// A time stamp (seconds since 1970) as a local date and time.
fn when(secs: i64) -> String {
    use chrono::{Datelike as _, TimeZone as _, Timelike as _};
    let Some(t) = chrono::Local.timestamp_opt(secs, 0).single() else {
        return String::new();
    };
    I18n::global()
        .localizer()
        .date_time(
            (t.year(), t.month() as u8, t.day() as u8),
            (t.hour() as u8, t.minute() as u8),
        )
        .unwrap_or_else(|| t.format("%Y-%m-%d %H:%M").to_string())
}

fn keywords_text(k: &[String]) -> String {
    if k.is_empty() {
        t!("local-no-keywords")
    } else {
        k.join(", ")
    }
}

fn to_mark(m: specular_library::Mark) -> Mark {
    Mark {
        rating: m.rating,
        label: m.label,
        flag: m.flag,
    }
}

/// Both versions of a conflicting change, in words (reads the file's side; off the frame).
fn describe(
    c: &lc::Change,
    check: Check,
    vocab: &specular_metadata::keywords::Vocabulary,
) -> Conflict {
    let original = matches!(check, Check::Conflict { original: true, .. });
    let sidecar = matches!(check, Check::Conflict { sidecar: true, .. });
    let sc = marks::Sidecars::scan(c.path.parent().unwrap_or(Path::new("")));
    let (yours, mut theirs) = match c.kind {
        ChangeKind::Marks => {
            let mine = lc::decode_marks(&c.payload)
                .map(to_mark)
                .unwrap_or_default();
            let (side, exists) = sc.resolve(&c.path);
            let disk = exists
                .then(|| std::fs::read_to_string(&side).ok())
                .flatten()
                .map(|s| marks::parse_xmp(&s))
                .unwrap_or_default();
            (marks_text(mine), marks_text(disk))
        }
        ChangeKind::Keywords => {
            let mine = lc::decode_keywords(&c.payload);
            let disk = crate::app::keywords::files::read(&c.path, &sc, vocab);
            (keywords_text(&mine), keywords_text(&disk))
        }
        ChangeKind::Iptc => {
            let mine: Vec<String> = lc::decode_fields(&c.payload)
                .into_iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect();
            (mine.join("; "), t!("local-changed-elsewhere"))
        }
        ChangeKind::Edit => {
            let disk = crate::app::develop::store::sidecar_saved(&c.path, c.version);
            (
                t!("local-edit-here", when = when(c.queued)),
                match disk {
                    Some(s) => t!("local-edit-elsewhere", when = when(s)),
                    None => t!("local-edit-none"),
                },
            )
        }
    };
    if original && !sidecar {
        theirs = t!("local-photo-changed");
    }
    Conflict {
        change: c.clone(),
        file: file_name(&c.path),
        yours,
        theirs,
    }
}

/// Writes one waiting change through the usual writer (the original is reachable).
fn apply(
    c: &lc::Change,
    lib: Option<&specular_library::Library>,
    vocab: &specular_metadata::keywords::Vocabulary,
) -> Result<(), String> {
    match c.kind {
        ChangeKind::Marks => {
            let m = lc::decode_marks(&c.payload).ok_or("damaged")?;
            let failed =
                marks::write_sidecars(marks::sidecar_ticket(), vec![(c.path.clone(), to_mark(m))]);
            match failed.into_iter().next() {
                Some((_, e)) => Err(e.to_string()),
                None => Ok(()),
            }
        }
        ChangeKind::Keywords => {
            let k = lc::decode_keywords(&c.payload);
            let r = crate::app::keywords::files::restore_all(&[(c.path.clone(), k.clone())], vocab);
            match r.into_iter().next().map(|x| x.1) {
                Some(Err(e)) => Err(e),
                _ => {
                    if let Some(l) = lib {
                        l.set_keywords(vec![(c.path.clone(), k)]);
                    }
                    Ok(())
                }
            }
        }
        ChangeKind::Iptc => match globals::fields_writer() {
            Some(w) => w(&c.path, &lc::decode_fields(&c.payload)).map_err(|e| e.to_string()),
            None => Err("no IPTC writer".into()),
        },
        ChangeKind::Edit => {
            let Some(l) = lib else {
                return Err("no catalogue".into());
            };
            crate::app::develop::store::write_sidecar_from_library(&c.path, c.version, l)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }
}

/// Writes every clean waiting change whose original is back; the conflicts, said.
fn sync(
    l: &LocalCopies,
    lib: Option<&specular_library::Library>,
    vocab: &specular_metadata::keywords::Vocabulary,
) -> (usize, usize, Vec<Conflict>) {
    let (mut written, mut failed, mut conflicts) = (0, 0, Vec::new());
    let Ok(changes) = l.changes() else {
        return (0, 0, conflicts);
    };
    for c in changes {
        match l.check(&c) {
            Check::Missing => {}
            Check::Clean => match apply(&c, lib, vocab) {
                Ok(()) => {
                    let _ = l.done(&[c.id]);
                    let _ = l.rebase(&c.path);
                    written += 1;
                }
                Err(e) => {
                    specular_log::warn!(target: specular_log::FOLDER, "local copies: cannot write a waiting change of {}: {e}", c.path.display());
                    failed += 1;
                }
            },
            k @ Check::Conflict { .. } => conflicts.push(describe(&c, k, vocab)),
        }
    }
    (written, failed, conflicts)
}

impl Gallery {
    /// The offline banner's lines and whether it offers Review: shown in the Library while a
    /// location with local copies or waiting changes is offline, or conflicts wait.
    pub(super) fn local_banner(&self) -> Option<(Vec<String>, bool)> {
        let lv = &self.library;
        if !lv.on {
            return None;
        }
        let v = &lv.local;
        let offline: Vec<&Location> = lv.locations.iter().filter(|l| l.offline).collect();
        let conflicts = v.conflicts.len() + usize::from(v.asking.is_some());
        let mut lines = Vec::new();
        let has_copies = v.handle.as_ref().is_some_and(|h| h.count() > 0);
        if !offline.is_empty() && (has_copies || v.waiting > 0) {
            lines.push(match offline.as_slice() {
                [one] => t!(
                    "local-banner-offline",
                    location = one
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                many => t!("local-banner-offline-many", count = many.len() as i64),
            });
        }
        if v.waiting > 0 && conflicts == 0 {
            lines.push(t!("local-banner-waiting", count = v.waiting as i64));
        }
        if conflicts > 0 {
            lines.push(t!("local-banner-conflicts", count = conflicts as i64));
        }
        (!lines.is_empty()).then_some((lines, conflicts > 0))
    }

    /// Height of the offline banner (0 when none).
    pub(super) fn local_banner_h(&self) -> f32 {
        match self.local_banner() {
            Some((lines, _)) => self.u(12.0) + self.u(20.0) * lines.len() as f32,
            None => 0.0,
        }
    }

    /// Draws the offline banner under the timeline.
    pub(super) fn draw_local_banner(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let Some((lines, review)) = self.local_banner() else {
            return;
        };
        let pal = f.pal;
        let h = self.local_banner_h();
        let top = TOP_BAR
            + if self.filter_open {
                self.filter_bar_h()
            } else {
                0.0
            }
            + self.timeline_h();
        let w = self.grid_w();
        let band = Rect::new(0.0, top, w, h);
        dl.rect(self.flip(band), 0.0, pal.surface_raised);
        dl.rect(
            self.flip(Rect::new(0.0, top, self.u(3.0), h)),
            0.0,
            pal.warning,
        );
        dl.rect(
            self.flip(Rect::new(0.0, band.bottom() - 1.0, w, 1.0)),
            0.0,
            pal.separator,
        );
        let style = TextStyle::new(13.0 * f.text_scale.clamp(0.75, 3.0));
        let pad = self.u(14.0);
        let mut text_w = w - 2.0 * pad;
        if review {
            let label = t!("local-review");
            let bw = text.measure(&label, style) + self.u(20.0);
            let br = self.flip(Rect::new(
                w - pad - bw,
                top + (h - self.u(26.0)) * 0.5,
                bw,
                self.u(26.0),
            ));
            if self.pointer.is_some_and(|p| br.contains(p)) {
                dl.rect(br, self.u(6.0), pal.surface_hover);
            }
            dl.stroke_rect(br, self.u(6.0), pal.separator, 1.0);
            super::similar::put_centered(text, dl, &label, style, br, pal.text);
            self.hits.push((br, Hit::Library(Side::LocalReview)));
            text_w -= bw + self.u(8.0);
        }
        draw_icon(
            dl,
            Icon::Alert,
            self.flip(Rect::new(
                pad - self.u(4.0),
                top + self.u(8.0),
                self.u(16.0),
                self.u(16.0),
            )),
            pal.warning,
            1.5,
            false,
        );
        for (k, l) in lines.iter().enumerate() {
            let r = self.flip(Rect::new(
                pad + self.u(18.0),
                top + self.u(6.0) + k as f32 * self.u(20.0),
                (text_w - self.u(18.0)).max(1.0),
                self.u(20.0),
            ));
            let s = truncate(text, l, style, r.w);
            let col = if k == 0 { pal.text } else { pal.text_muted };
            text_at(text, dl, &s, style, r, col, self.rtl);
        }
    }

    /// The sidebar's line under a kept location.
    pub(super) fn local_location_note(&self, id: i64) -> Option<String> {
        let k = self.library.local.kept_for(&Scope::Location(id))?;
        Some(match k.mode {
            Mode::Originals => t!("local-sidebar-originals"),
            Mode::Proxies => t!("local-sidebar-proxies"),
        })
    }

    /// The photos the palette's local-copy rows act on: the selection, else the focus.
    fn local_targets(&self) -> Vec<PathBuf> {
        self.targets()
    }
}

impl App {
    /// The palette's and the photo menu's rows (Library only, with the copies open).
    pub(super) fn local_menu_items(&self, photo: bool) -> Vec<PaletteItem> {
        let ga = &self.gallery;
        let v = &ga.library.local;
        let Some(h) = v.handle.as_ref() else {
            return Vec::new();
        };
        if !ga.library.on {
            return Vec::new();
        }
        let mut out = Vec::new();
        let targets = ga.local_targets();
        if photo || !targets.is_empty() {
            let kept = h.kinds(&targets).len();
            if kept < targets.len() {
                out.push(item(
                    code::KEEP,
                    t!("local-keep-menu"),
                    None,
                    Icon::Download,
                ));
            }
            if kept > 0 {
                out.push(item(
                    code::REMOVE,
                    t!("local-remove-menu"),
                    None,
                    Icon::Minus,
                ));
            }
        }
        if photo {
            return out;
        }
        if let Some(id) = ga.library.location {
            if v.kept_for(&Scope::Location(id)).is_some() {
                out.push(item(
                    code::REMOVE_LOCATION,
                    t!("local-remove-location-menu"),
                    None,
                    Icon::Minus,
                ));
            } else {
                out.push(item(
                    code::KEEP_LOCATION,
                    t!("local-keep-location-menu"),
                    None,
                    Icon::Download,
                ));
            }
        }
        if let Some(a) = ga.library.albums.open {
            if v.kept_for(&Scope::Album(a)).is_some() {
                out.push(item(
                    code::REMOVE_ALBUM,
                    t!("local-remove-album-menu"),
                    None,
                    Icon::Minus,
                ));
            } else {
                out.push(item(
                    code::KEEP_ALBUM,
                    t!("local-keep-album-menu"),
                    None,
                    Icon::Download,
                ));
            }
        }
        if !v.conflicts.is_empty() {
            out.push(item(code::REVIEW, t!("local-review"), None, Icon::Alert));
        }
        out
    }

    pub(super) fn local_code(&mut self, c: u64) {
        let ga = &self.gallery;
        let Some(h) = ga.library.local.handle.clone() else {
            return;
        };
        match c {
            code::KEEP => {
                let files = ga.local_targets();
                let name = if files.len() == 1 {
                    file_name(&files[0])
                } else {
                    t!("gallery-items", count = files.len() as i64)
                };
                self.local_ask_keep(Scope::Selection, name, files);
            }
            code::KEEP_LOCATION => {
                let Some(id) = ga.library.location else {
                    return;
                };
                let name = ga
                    .library
                    .locations
                    .iter()
                    .find(|l| l.id == id)
                    .map(|l| file_name(&l.path))
                    .unwrap_or_default();
                let files = h.files_of(&Scope::Location(id)).unwrap_or_default();
                self.local_ask_keep(Scope::Location(id), name, files);
            }
            code::KEEP_ALBUM => {
                let Some(id) = ga.library.albums.open else {
                    return;
                };
                let Some(a) = ga.album(id) else { return };
                let name = a.name.clone();
                let files = if a.kind == specular_library::albums::Kind::Manual {
                    h.files_of(&Scope::Album(id)).unwrap_or_default()
                } else {
                    // A smart album: the photos it shows now.
                    ga.entries.iter().map(|e| e.path.clone()).collect()
                };
                self.local_ask_keep(Scope::Album(id), name, files);
            }
            code::REMOVE => {
                // The kept selections holding any of the targets.
                let targets = ga.local_targets();
                let name = if targets.len() == 1 {
                    file_name(&targets[0])
                } else {
                    t!("gallery-items", count = targets.len() as i64)
                };
                let sel: Vec<i64> = ga
                    .library
                    .local
                    .kept
                    .iter()
                    .filter(|k| k.scope == Scope::Selection)
                    .map(|k| k.id)
                    .collect();
                if let Some(&id) = sel.first() {
                    self.local_ask_remove(id, name);
                } else {
                    let now = self.now();
                    self.chrome
                        .toast(t!("local-remove-held-elsewhere"), None, now);
                }
            }
            code::REMOVE_LOCATION | code::REMOVE_ALBUM => {
                let scope = if c == code::REMOVE_LOCATION {
                    ga.library.location.map(Scope::Location)
                } else {
                    ga.library.albums.open.map(Scope::Album)
                };
                let Some(k) = scope.and_then(|s| ga.library.local.kept_for(&s)).cloned() else {
                    return;
                };
                self.local_ask_remove(k.id, k.name);
            }
            code::REVIEW => self.local_ask_next(),
            _ => {}
        }
    }

    /// "Keep a local copy of …?": originals or proxies, or Cancel.
    fn local_ask_keep(&mut self, scope: Scope, name: String, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        let n = files.len();
        self.gallery.library.local.keep_ask = Some(KeepAsk {
            scope,
            name: name.clone(),
            files,
        });
        self.chrome.confirm_choice(
            t!("local-keep-ask", item = name),
            t!("local-keep-detail", count = n as i64),
            t!("local-keep-originals"),
            items::LOCAL_KEEP_ORIGINALS,
            t!("local-keep-proxies"),
            items::LOCAL_KEEP_PROXIES,
        );
        self.redraw = true;
    }

    /// The keep was confirmed: copies are made in the background.
    pub(in crate::app) fn local_keep_confirmed(&mut self, proxies: bool) {
        let v = &mut self.gallery.library.local;
        let (Some(ask), Some(h)) = (v.keep_ask.take(), v.handle.clone()) else {
            return;
        };
        let mode = if proxies {
            Mode::Proxies
        } else {
            Mode::Originals
        };
        let stop = Arc::new(AtomicBool::new(false));
        v.keep_stop = Some(stop.clone());
        v.keeping = Some((
            ask.name.clone(),
            KeepProgress {
                total: ask.files.len() as u64,
                ..KeepProgress::default()
            },
        ));
        let tx = v.tx();
        let wake = self.loader.waker();
        self.gallery.announce = Some(t!("local-keeping", item = ask.name.clone()));
        self.loader.background("local-keep", move |_| {
            let maker: lc::ProxyMaker = Arc::new(|p: &Path, b: &[u8], edge: u32| {
                specular_develop::proxy::make(p, b, edge).ok()
            });
            let (t2, w2) = (tx.clone(), wake.clone());
            let r = h.keep(
                &ask.scope,
                &ask.name,
                mode,
                &ask.files,
                Some(&maker),
                &|| stop.load(Ordering::Relaxed),
                &move |p| {
                    let _ = t2.send(Msg::Progress(p));
                    w2();
                },
            );
            match r {
                Ok(rep) => {
                    let _ = tx.send(Msg::Kept(ask.name, rep));
                }
                Err(e) => {
                    specular_log::warn!(target: specular_log::FOLDER, "local copies: keep failed: {e}");
                    let _ = tx.send(Msg::Kept(ask.name, KeepReport::default()));
                }
            }
            wake();
        });
        self.redraw = true;
    }

    /// "Remove the local copy of …?": asks first, naming it; the originals stay.
    fn local_ask_remove(&mut self, id: i64, name: String) {
        self.gallery.library.local.remove_ask = Some((id, name.clone()));
        self.chrome.confirm_destructive(
            t!("local-remove-ask", item = name),
            t!("local-remove-detail"),
            t!("local-remove-ok"),
            items::LOCAL_REMOVE,
        );
        self.redraw = true;
    }

    pub(in crate::app) fn local_remove_confirmed(&mut self) {
        let v = &mut self.gallery.library.local;
        let (Some((id, name)), Some(h)) = (v.remove_ask.take(), v.handle.clone()) else {
            return;
        };
        let tx = v.tx();
        let wake = self.loader.waker();
        self.loader.background("local-remove", move |_| {
            let freed = h.remove(id).unwrap_or(0);
            let _ = tx.send(Msg::Removed(name, freed));
            wake();
        });
    }

    /// Once per turn with the Library: opens the copies, takes in background results,
    /// notices locations coming back, and asks about the next conflict.
    pub(super) fn local_tick(&mut self) {
        let Some(lib) = self.gallery.library.handle() else {
            return;
        };
        let limit = (self.settings.library.local_copies_limit.max(1) as u64) << 20;
        let v = &mut self.gallery.library.local;
        if v.handle.is_none() && !v.opening && !v.failed && !lib.read_only() {
            v.opening = true;
            let tx = v.tx();
            let wake = self.loader.waker();
            let catalogue = lib.path().to_path_buf();
            self.loader.background("local-open", move |_| {
                let root = catalogue
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(lc::DIR_NAME);
                let r = LocalCopies::open(&catalogue, root)
                    .map(Arc::new)
                    .map_err(|e| e.to_string());
                let _ = tx.send(Msg::Opened(r));
                wake();
            });
        }
        if let Some(h) = &v.handle
            && h.limit() != limit
        {
            h.set_limit(limit);
        }
        let msgs: Vec<Msg> = v
            .chan
            .as_ref()
            .map(|c| c.1.try_iter().collect())
            .unwrap_or_default();
        for m in msgs {
            self.local_message(m);
            self.redraw = true;
        }
        let v = &mut self.gallery.library.local;
        // Locations back online: their waiting changes are written.
        let now_off: HashSet<i64> = self
            .gallery
            .library
            .locations
            .iter()
            .filter(|l| l.offline)
            .map(|l| l.id)
            .collect();
        if v.offline.iter().any(|id| !now_off.contains(id)) {
            v.sync_due = true;
        }
        v.offline = now_off;
        if let Some(h) = &v.handle {
            let w = h.waiting();
            if w != v.waiting {
                v.waiting = w;
                self.redraw = true;
            }
        }
        if v.sync_due && !v.syncing && v.waiting > 0 && v.handle.is_some() {
            self.local_sync();
        }
        if self.gallery.library.local.export_wait {
            self.local_export_tick();
        }
        // The conflict sheet closed without an answer: it waits for Review.
        if self.gallery.library.local.asking.is_some() && self.chrome.modal().is_none() {
            self.local_conflict_later();
        }
    }

    fn local_message(&mut self, m: Msg) {
        let now = self.now();
        let v = &mut self.gallery.library.local;
        match m {
            Msg::Opened(Ok(h)) => {
                v.opening = false;
                v.kept = h.kept().unwrap_or_default();
                v.waiting = h.waiting();
                v.sync_due = true;
                globals::install(Some(h.clone()));
                v.handle = Some(h);
            }
            Msg::Opened(Err(e)) => {
                v.opening = false;
                v.failed = true;
                warn!(target: UI, "local copies: not opened: {e}");
            }
            Msg::Progress(p) => {
                if let Some(k) = v.keeping.as_mut() {
                    k.1 = p;
                }
            }
            Msg::Kept(name, rep) => {
                v.keeping = None;
                v.keep_stop = None;
                if let Some(h) = &v.handle {
                    v.kept = h.kept().unwrap_or_default();
                }
                let mut said = vec![t!(
                    "local-kept",
                    item = name,
                    count = rep.copied as i64 + rep.shared as i64,
                    size = size_text(rep.bytes)
                )];
                if rep.over_limit > 0 {
                    said.push(t!("local-kept-full", count = rep.over_limit as i64));
                }
                if rep.unreachable > 0 {
                    said.push(t!("local-kept-unreachable", count = rep.unreachable as i64));
                }
                let s = said.join(" ");
                self.chrome.toast(s.clone(), None, now);
                self.chrome.announce_status(s);
            }
            Msg::Removed(name, freed) => {
                if let Some(h) = &v.handle {
                    v.kept = h.kept().unwrap_or_default();
                }
                let s = t!("local-removed", item = name, size = size_text(freed));
                self.chrome.toast(s.clone(), None, now);
                self.chrome.announce_status(s);
            }
            Msg::Synced {
                written,
                failed,
                conflicts,
            } => {
                v.syncing = false;
                if let Some(h) = &v.handle {
                    v.waiting = h.waiting();
                }
                let known: HashSet<i64> = v
                    .conflicts
                    .iter()
                    .chain(v.asking.iter())
                    .map(|c| c.change.id)
                    .collect();
                v.conflicts.extend(
                    conflicts
                        .into_iter()
                        .filter(|c| !known.contains(&c.change.id)),
                );
                if written > 0 {
                    let s = t!("local-synced", count = written as i64);
                    self.chrome.toast(s.clone(), None, now);
                    self.chrome.announce_status(s);
                }
                if failed > 0 {
                    self.chrome
                        .toast(t!("local-sync-failed", count = failed as i64), None, now);
                }
                self.gallery.dirty = true;
                if self.gallery.library.local.asking.is_none() {
                    self.local_ask_next();
                }
            }
            Msg::Resolved(mark) => {
                if let Some(h) = &v.handle {
                    v.waiting = h.waiting();
                }
                if let Some((p, m)) = mark {
                    self.gallery.marks.set_mark(&p, m);
                    self.gallery.library_marks_changed(&[(p, m)]);
                    self.gallery.dirty = true;
                }
            }
        }
    }

    /// Writes the waiting changes whose originals are back (in the background).
    fn local_sync(&mut self) {
        let lib = self.gallery.library.handle();
        let vocab = self.gallery.keywords.vocab.clone();
        let v = &mut self.gallery.library.local;
        let Some(h) = v.handle.clone() else { return };
        v.sync_due = false;
        v.syncing = true;
        let tx = v.tx();
        let wake = self.loader.waker();
        self.loader.background("local-sync", move |_| {
            let (written, failed, conflicts) = sync(&h, lib.as_deref(), &vocab);
            let _ = tx.send(Msg::Synced {
                written,
                failed,
                conflicts,
            });
            wake();
        });
    }

    /// Asks about the next conflict: both versions, keep mine or the file's (Cancel: later).
    pub(super) fn local_ask_next(&mut self) {
        let v = &mut self.gallery.library.local;
        if v.asking.is_some() {
            return;
        }
        let Some(c) = v.conflicts.pop_front() else {
            return;
        };
        let (title, detail) = conflict_text(&c);
        v.asking = Some(c);
        self.chrome.confirm_choice(
            title,
            detail,
            t!("local-keep-mine"),
            items::LOCAL_MINE,
            t!("local-keep-theirs"),
            items::LOCAL_THEIRS,
        );
        self.redraw = true;
    }

    /// The conflict's answer: mine is written over the file's, or the file's is kept and
    /// mine dropped. Then the next one.
    pub(in crate::app) fn local_conflict_answer(&mut self, mine: bool) {
        let lib = self.gallery.library.handle();
        let vocab = self.gallery.keywords.vocab.clone();
        let v = &mut self.gallery.library.local;
        let (Some(c), Some(h)) = (v.asking.take(), v.handle.clone()) else {
            return;
        };
        let tx = v.tx();
        let wake = self.loader.waker();
        self.loader.background("local-resolve", move |_| {
            let ch = &c.change;
            let mut mark = None;
            if mine {
                if let Err(e) = apply(ch, lib.as_deref(), &vocab) {
                    specular_log::warn!(target: specular_log::FOLDER, "local copies: cannot write {}: {e}", ch.path.display());
                    let _ = tx.send(Msg::Resolved(None));
                    wake();
                    return;
                }
                if ch.kind == ChangeKind::Marks && let Some(l) = &lib
                    && let Some(m) = lc::decode_marks(&ch.payload)
                {
                    l.set_marks(vec![(ch.path.clone(), m)]);
                }
            } else {
                // The file's version wins everywhere: the gallery, the catalogue.
                match ch.kind {
                    ChangeKind::Marks => {
                        let sc = marks::Sidecars::scan(ch.path.parent().unwrap_or(Path::new("")));
                        let (side, exists) = sc.resolve(&ch.path);
                        let m = exists
                            .then(|| std::fs::read_to_string(&side).ok())
                            .flatten()
                            .map(|s| marks::parse_xmp(&s))
                            .unwrap_or_default();
                        mark = Some((ch.path.clone(), m));
                    }
                    ChangeKind::Keywords => {
                        let sc = marks::Sidecars::scan(ch.path.parent().unwrap_or(Path::new("")));
                        let k = crate::app::keywords::files::read(&ch.path, &sc, &vocab);
                        if let Some(l) = &lib {
                            l.set_keywords(vec![(ch.path.clone(), k)]);
                        }
                    }
                    ChangeKind::Edit => {
                        if let Some(l) = &lib {
                            crate::app::develop::store::adopt_sidecar(&ch.path, ch.version, l);
                        }
                    }
                    ChangeKind::Iptc => {}
                }
            }
            let _ = h.done(&[ch.id]);
            let _ = h.rebase(&ch.path);
            let _ = tx.send(Msg::Resolved(mark));
            wake();
        });
        // A Cancel leaves the rest for Review; an answer goes on to the next.
        self.local_ask_next();
    }

    /// The conflict sheet was cancelled: it waits for Review.
    pub(in crate::app) fn local_conflict_later(&mut self) {
        let v = &mut self.gallery.library.local;
        if let Some(c) = v.asking.take() {
            v.conflicts.push_front(c);
        }
    }

    /// An export from offline photos with proxies asks first; true when it asked (the
    /// export waits for the answer).
    pub(in crate::app) fn local_export_check(&mut self, paths: &[PathBuf]) -> bool {
        let v = &mut self.gallery.library.local;
        if v.export_proxy_ok {
            v.export_proxy_ok = false;
            return false;
        }
        let n = paths.iter().filter(|p| globals::reads_proxy(p)).count();
        if n == 0 {
            return false;
        }
        self.chrome.confirm_choice(
            t!("local-export-ask", count = n as i64),
            t!("local-export-detail"),
            t!("local-export-proxy"),
            items::LOCAL_EXPORT_PROXY,
            t!("local-export-wait"),
            items::LOCAL_EXPORT_WAIT,
        );
        self.redraw = true;
        true
    }

    /// The export question's answer: export from the proxies now, or when the originals
    /// are back.
    pub(in crate::app) fn local_export_answer(&mut self, proxy: bool) {
        let v = &mut self.gallery.library.local;
        if proxy {
            v.export_proxy_ok = true;
            self.develop_export_start();
        } else {
            v.export_wait = true;
            let now = self.now();
            self.chrome.toast(t!("local-export-waiting"), None, now);
        }
    }

    /// The export that waits: started once no source reads a proxy (dropped when its
    /// dialog closed).
    fn local_export_tick(&mut self) {
        let Some(d) = self.convert.dialog.as_ref() else {
            self.gallery.library.local.export_wait = false;
            return;
        };
        let Some(paths) = d.sources.get(d.source).map(|s| s.1.clone()) else {
            return;
        };
        if paths.iter().any(|p| globals::reads_proxy(p)) {
            return;
        }
        self.gallery.library.local.export_wait = false;
        self.develop_export_start();
    }

    /// Settings › Library: the space used and the kept items, under the size limit.
    pub(in crate::app) fn local_panel_tick(&mut self) {
        if self.chrome.modal() != Some(specular_widgets::Modal::Settings) {
            return;
        }
        let v = &mut self.gallery.library.local;
        let Some(h) = v.handle.clone() else { return };
        let built = (
            I18n::global().generation(),
            h.used(),
            v.kept.len(),
            h.limit(),
        );
        if v.panel_built == Some(built) {
            return;
        }
        v.panel_built = Some(built);
        let p = panel(&v.kept, h.used(), h.limit(), h.root());
        if self.chrome.set_setting_panel(PANEL, Some(p)) {
            self.redraw = true;
        }
    }

    /// A kept item's Remove… in Settings: asks first.
    pub(in crate::app) fn local_panel_event(&mut self, e: &PanelEvent) {
        let PanelEvent::Row(row) = e else { return };
        let Some(id) = row
            .strip_prefix("kept:")
            .and_then(|i| i.parse::<i64>().ok())
        else {
            return;
        };
        let Some(k) = self
            .gallery
            .library
            .local
            .kept
            .iter()
            .find(|k| k.id == id)
            .cloned()
        else {
            return;
        };
        self.local_ask_remove(k.id, k.name);
    }
}

/// The conflict sheet's question and both versions.
pub(in crate::app) fn conflict_text(c: &Conflict) -> (String, String) {
    let what = match c.change.kind {
        ChangeKind::Marks => t!("local-what-marks"),
        ChangeKind::Keywords => t!("local-what-keywords"),
        ChangeKind::Iptc => t!("local-what-fields"),
        ChangeKind::Edit => t!("local-what-edit"),
    };
    (
        t!("local-conflict-title", file = c.file.clone(), what = what),
        format!(
            "{}\n{}\n{}",
            t!("local-conflict-yours", text = c.yours.clone()),
            t!("local-conflict-theirs", text = c.theirs.clone()),
            t!("local-conflict-later")
        ),
    )
}

/// The Settings panel: space used of the limit, each kept item with Remove….
pub(in crate::app) fn panel(kept: &[Kept], used: u64, limit: u64, root: &Path) -> SettingPanel {
    SettingPanel {
        title: t!("local-panel-title"),
        intro: t!(
            "local-panel-used",
            used = size_text(used),
            limit = size_text(limit)
        ),
        rows: kept
            .iter()
            .map(|k| PanelRow {
                id: format!("kept:{}", k.id),
                label: k.name.clone(),
                value: size_text(k.bytes),
                lines: vec![match k.mode {
                    Mode::Originals => t!("local-panel-originals", count = k.files as i64),
                    Mode::Proxies => t!("local-panel-proxies", count = k.files as i64),
                }],
                button: Some(t!("local-panel-remove")),
                ..PanelRow::default()
            })
            .collect(),
        footer: t!("local-panel-footer", folder = root.display().to_string()),
        action: None,
        keywords: t!("local-panel-keywords"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(kind: ChangeKind) -> lc::Change {
        lc::Change {
            id: 1,
            path: PathBuf::from("/card/IMG_0001.CR3"),
            identity: lc::Identity::default(),
            sidecars: None,
            xmp_mtime: None,
            kind,
            version: 0,
            payload: Vec::new(),
            queued: 0,
        }
    }

    #[test]
    fn marks_and_conflicts_are_said_in_words() {
        let m = Mark {
            rating: 4,
            label: 3,
            flag: 1,
        };
        let s = marks_text(m);
        assert!(s.contains('4') && s.contains(&t!("local-pick")), "{s}");
        let c = Conflict {
            change: change(ChangeKind::Marks),
            file: "IMG_0001.CR3".into(),
            yours: marks_text(m),
            theirs: marks_text(Mark::default()),
        };
        let (title, detail) = conflict_text(&c);
        assert!(title.contains("IMG_0001.CR3"));
        assert_eq!(detail.lines().count(), 3);
        assert!(detail.contains(&c.yours) && detail.contains(&c.theirs));
    }

    #[test]
    fn the_banner_says_offline_waiting_and_conflicts() {
        let mut g = Gallery::default();
        g.library.on = true;
        g.library.locations = vec![Location {
            id: 1,
            path: PathBuf::from("/media/card/DCIM"),
            offline: true,
            files: 3,
            pending: 0,
            last_scan: None,
        }];
        assert!(g.local_banner().is_none(), "nothing kept, nothing waits");
        g.library.local.waiting = 2;
        let (lines, review) = g.local_banner().unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("DCIM"), "{lines:?}");
        assert!(!review);
        g.library.locations[0].offline = false;
        g.library.local.conflicts.push_back(Conflict {
            change: change(ChangeKind::Keywords),
            file: "IMG_0001.CR3".into(),
            yours: "sun".into(),
            theirs: "sea".into(),
        });
        let (lines, review) = g.local_banner().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(review);
        g.library.on = false;
        assert!(g.local_banner().is_none());
    }

    #[test]
    fn the_settings_panel_lists_kept_items_with_remove() {
        let kept = vec![Kept {
            id: 7,
            scope: Scope::Location(1),
            name: "DCIM".into(),
            mode: Mode::Proxies,
            files: 12,
            bytes: 3_000_000,
        }];
        let p = panel(&kept, 3_000_000, 20 << 30, Path::new("/data/local-copies"));
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].id, "kept:7");
        assert!(p.rows[0].button.is_some());
        assert!(p.intro.contains("3"), "{}", p.intro);
    }
}

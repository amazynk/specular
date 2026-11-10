//! Develop's history, styles, presets and batch work.
//!
//! - **Panel cards** (after the modules): Styles (shipped and the user's; hover or focus one
//!   to preview it, click to apply), History (take a snapshot, compress, each snapshot:
//!   click to compare it split with the edit) and Versions (virtual copies: new, duplicate,
//!   click one to open it).
//! - **Menus:** right-click or Shift+F10 on a card or a row. A module's card
//!   lists its presets (shipped and saved), Save as preset… and Delete; a style, snapshot
//!   or version row lists what can be done with it. Deleting asks first.
//! - **Copy and paste** (Ctrl+Alt+C, Ctrl+Alt+V; with Shift to choose modules): copy the
//!   edit (all but crop and lens) and paste it on the photo, the gallery's selection or the
//!   picks; each photo gets one history step. **Sync** in cull mode puts the current photo's
//!   edit on the picks.
//! - **Match to reference** (Ctrl+Alt+R sets it, Ctrl+Alt+M matches; [`super::matcher`]).
//! - **Auto-apply rules** pick a style on first open; **next capture style** in cull mode
//!   gives each new file arriving a style or the previous photo's edit.
//! - Styles, presets, rules and the next-capture choice live in the settings folder
//!   (`develop-styles/`, `develop-presets.txt`, `develop-rules.txt`,
//!   `develop-next-capture.txt`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use specular_develop::model::style::{auto_style, shipped_presets, shipped_styles};
use specular_develop::model::{
    ApplyMode, AutoRule, Edit, FileType, History, MaskPreset, ModuleKind, PhotoFacts, Preset,
    Registry, Snapshot, Step, Style,
};
use specular_develop::sidecar::files;
use specular_gfx::Icon;
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_widgets::PaletteItem;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control, DevelopPanel, Item};
use specular_widgets::{DownloadRow, DownloadState};

use super::controls::{self, DETAIL_CARD, HDR_CARD};
use super::masks::MaskAct;
use super::sheet::{DevelopSheet, Purpose};
use crate::app::App;
use crate::app::items;

/// The cards this module adds.
pub(super) const STYLES_CARD: u32 = 0x4_0000;
pub(super) const HISTORY_CARD: u32 = 0x4_0001;
pub(super) const VERSIONS_CARD: u32 = 0x4_0002;
/// Their controls: rows by index from a base, and buttons.
const STYLE_ROW: u32 = 0x40_0000;
const STYLE_NEW: u32 = 0x40_FFF0;
const STYLE_IMPORT: u32 = 0x40_FFF1;
const STYLE_RULES: u32 = 0x40_FFF2;
const SNAP_ROW: u32 = 0x41_0000;
const SNAP_TAKE: u32 = 0x41_FFF0;
const SNAP_COMPRESS: u32 = 0x41_FFF1;
const VERSION_ROW: u32 = 0x42_0000;
const VERSION_NEW: u32 = 0x42_FFF0;
const VERSION_DUP: u32 = 0x42_FFF1;
/// The downloads panel's Stop key of a background batch, before its token.
const BATCH_STOP: &str = "develop-batch-stop:";
const ROWS: u32 = 0xFF00;

/// Whether a control is one of this module's.
pub(super) fn is_ours(control: u32) -> bool {
    (STYLE_ROW..VERSION_ROW + 0x1_0000).contains(&control)
}

fn row_of(control: u32, base: u32) -> Option<usize> {
    (base..base + ROWS)
        .contains(&control)
        .then(|| (control - base) as usize)
}

/// Files Develop keeps in the settings folder.
const STYLES_DIR: &str = "develop-styles";
/// Mask presets, one `.xmp` each.
const MASK_PRESETS_DIR: &str = "develop-mask-presets";
const PRESETS_FILE: &str = "develop-presets.txt";
const RULES_FILE: &str = "develop-rules.txt";
const NEXT_FILE: &str = "develop-next-capture.txt";

/// What each new file arriving in cull mode gets.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum NextCapture {
    #[default]
    Off,
    /// The edit of the photo that arrived before it.
    SameAsPrevious,
    /// A style, by its name as listed.
    Style(String),
}

/// A style as listed: shipped ones first (names translated), then the user's.
#[derive(Clone, Debug)]
pub(super) struct Listed {
    pub name: String,
    pub style: Style,
    /// Index in the user's styles; `None` for a shipped one.
    pub user: Option<usize>,
}

/// The user's styles, presets and rules, read from the settings folder on first use.
#[derive(Default)]
pub(super) struct Shelf {
    pub styles: Vec<Style>,
    /// The file each user style is in (parallel to `styles`).
    files: Vec<PathBuf>,
    pub presets: Vec<Preset>,
    /// Mask presets and their files (parallel), by name.
    pub mask_presets: Vec<MaskPreset>,
    mask_files: Vec<PathBuf>,
    pub rules: Vec<AutoRule>,
    pub next: NextCapture,
    loaded: bool,
    dir: Option<PathBuf>,
}

fn settings_dir() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(specular_settings::Locations::detect().config_dir)
}

fn warn(what: &str, e: impl std::fmt::Display) {
    specular_log::warn!(target: specular_log::FOLDER, "develop: {what}: {e}");
}

impl Shelf {
    /// Reads the files once.
    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.dir = settings_dir();
        let Some(dir) = self.dir.clone() else { return };
        let reg = Registry::builtin();
        if let Ok(rd) = std::fs::read_dir(dir.join(STYLES_DIR)) {
            let mut found: Vec<(String, Style, PathBuf)> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "xmp"))
                .filter_map(|p| {
                    let text = std::fs::read_to_string(&p).ok()?;
                    let stem = p.file_stem()?.to_string_lossy().into_owned();
                    match files::style_from_xmp(&text, reg, &stem) {
                        Ok((s, _)) => Some((s.name.to_lowercase(), s, p)),
                        Err(e) => {
                            warn("a style file", e);
                            None
                        }
                    }
                })
                .collect();
            found.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, s, p) in found {
                self.styles.push(s);
                self.files.push(p);
            }
        }
        if let Ok(rd) = std::fs::read_dir(dir.join(MASK_PRESETS_DIR)) {
            let mut found: Vec<(String, MaskPreset, PathBuf)> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "xmp"))
                .filter_map(|p| {
                    let text = std::fs::read_to_string(&p).ok()?;
                    let stem = p.file_stem()?.to_string_lossy().into_owned();
                    match files::mask_preset_from_xmp(&text, reg, &stem) {
                        Ok(m) => Some((m.name.to_lowercase(), m, p)),
                        Err(e) => {
                            warn("a mask preset file", e);
                            None
                        }
                    }
                })
                .collect();
            found.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, m, p) in found {
                self.mask_presets.push(m);
                self.mask_files.push(p);
            }
        }
        if let Ok(t) = std::fs::read_to_string(dir.join(PRESETS_FILE)) {
            self.presets = files::presets_from_text(&t, reg);
        }
        if let Ok(t) = std::fs::read_to_string(dir.join(RULES_FILE)) {
            self.rules = files::rules_from_text(&t);
        }
        if let Ok(t) = std::fs::read_to_string(dir.join(NEXT_FILE)) {
            self.next = match t.trim() {
                "" | "off" => NextCapture::Off,
                "previous" => NextCapture::SameAsPrevious,
                s => NextCapture::Style(s.strip_prefix("style:").unwrap_or(s).to_owned()),
            };
        }
    }

    fn write(&self, name: &str, text: &str) {
        let Some(dir) = &self.dir else { return };
        let _ = std::fs::create_dir_all(dir);
        if let Err(e) = specular_develop::sidecar::write_text(&dir.join(name), text) {
            warn(name, e);
        }
    }

    pub fn save_presets(&self) {
        self.write(PRESETS_FILE, &files::presets_to_text(&self.presets));
    }

    pub fn save_rules(&self) {
        self.write(RULES_FILE, &files::rules_to_text(&self.rules));
    }

    pub fn save_next(&self) {
        let t = match &self.next {
            NextCapture::Off => "off".to_owned(),
            NextCapture::SameAsPrevious => "previous".to_owned(),
            NextCapture::Style(s) => format!("style:{s}"),
        };
        self.write(NEXT_FILE, &t);
    }

    /// Adds a style (or replaces the one of the same name) and writes its file.
    pub fn put_style(&mut self, s: Style) {
        let at = self.styles.iter().position(|x| x.name == s.name);
        let path = self.dir.as_ref().map(|d| {
            let dir = d.join(STYLES_DIR);
            let _ = std::fs::create_dir_all(&dir);
            match at.and_then(|i| self.files.get(i)) {
                Some(p) => p.clone(),
                None => unique(&dir, &files::style_file_name(&s.name)),
            }
        });
        if let Some(p) = &path
            && let Err(e) = specular_develop::sidecar::write_text(p, &files::style_to_xmp(&s))
        {
            warn("a style file", e);
        }
        match at {
            Some(i) => self.styles[i] = s,
            None => {
                self.styles.push(s);
                self.files.push(path.unwrap_or_default());
            }
        }
    }

    /// Adds a mask preset (or replaces the one of the same name) and writes its file.
    pub fn put_mask_preset(&mut self, m: MaskPreset) {
        let at = self.mask_presets.iter().position(|x| x.name == m.name);
        let path = self.dir.as_ref().map(|d| {
            let dir = d.join(MASK_PRESETS_DIR);
            let _ = std::fs::create_dir_all(&dir);
            match at.and_then(|i| self.mask_files.get(i)) {
                Some(p) => p.clone(),
                None => unique(&dir, &files::style_file_name(&m.name)),
            }
        });
        if let Some(p) = &path
            && let Err(e) = specular_develop::sidecar::write_text(p, &files::mask_preset_to_xmp(&m))
        {
            warn("a mask preset file", e);
        }
        match at {
            Some(i) => self.mask_presets[i] = m,
            None => {
                self.mask_presets.push(m);
                self.mask_files.push(path.unwrap_or_default());
            }
        }
    }

    /// Deletes mask preset `i` and its file (masks already put on photos stay).
    pub fn remove_mask_preset(&mut self, i: usize) {
        if i >= self.mask_presets.len() {
            return;
        }
        self.mask_presets.remove(i);
        let p = self.mask_files.remove(i);
        if !p.as_os_str().is_empty()
            && let Err(e) = std::fs::remove_file(&p)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            warn("a mask preset file", e);
        }
    }

    pub fn rename_style(&mut self, i: usize, name: &str) {
        let Some(mut s) = self.styles.get(i).cloned() else {
            return;
        };
        self.remove_style(i);
        s.name = name.to_owned();
        self.put_style(s);
    }

    pub fn remove_style(&mut self, i: usize) {
        if i >= self.styles.len() {
            return;
        }
        self.styles.remove(i);
        let p = self.files.remove(i);
        if !p.as_os_str().is_empty()
            && let Err(e) = std::fs::remove_file(&p)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            warn("a style file", e);
        }
    }

    /// Every style as listed.
    pub fn listed(&self) -> Vec<Listed> {
        let i18n = I18n::global();
        let mut v: Vec<Listed> = shipped_styles(Registry::builtin())
            .into_iter()
            .map(|s| Listed {
                name: i18n.tr(&s.name),
                style: s,
                user: None,
            })
            .collect();
        v.extend(self.styles.iter().enumerate().map(|(i, s)| Listed {
            name: s.name.clone(),
            style: s.clone(),
            user: Some(i),
        }));
        v
    }

    /// A style by its listed name.
    pub fn style_named(&self, name: &str) -> Option<Style> {
        self.listed()
            .into_iter()
            .find(|l| l.name == name || l.style.name == name)
            .map(|l| l.style)
    }

    /// The presets of `kind`: shipped (name translated) then the user's (with their index).
    pub fn presets_of(&self, kind: ModuleKind) -> Vec<(String, Preset, Option<usize>)> {
        let i18n = I18n::global();
        let mut v: Vec<(String, Preset, Option<usize>)> = shipped_presets()
            .into_iter()
            .filter(|p| p.kind == kind)
            .map(|p| (i18n.tr(&p.name), p, None))
            .collect();
        v.extend(
            self.presets
                .iter()
                .enumerate()
                .filter(|(_, p)| p.kind == kind)
                .map(|(i, p)| (p.name.clone(), p.clone(), Some(i))),
        );
        v
    }
}

/// `name` in `dir`, numbered when taken.
fn unique(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let stem = name.trim_end_matches(".xmp");
    (2..)
        .map(|n| dir.join(format!("{stem} {n}.xmp")))
        .find(|p| !p.exists())
        .unwrap_or(p)
}

/// What a menu row or a confirmation does.
#[derive(Clone, Debug)]
pub(super) enum Do {
    ApplyPreset(Preset),
    SavePreset(ModuleKind),
    DeletePreset(usize),
    ApplyStyle(usize, ApplyMode),
    RenameStyle(usize),
    ExportStyle(usize),
    AutoApplyStyle(usize),
    DeleteStyle(usize),
    DeleteRule(usize),
    /// A rule's rows: what it looks at, its style, Delete.
    EditRule(usize),
    /// Type a rule's maker, model, lens or ISO range.
    RuleText(usize, RuleField),
    /// The rule's file type, stepped (any, raw, not raw).
    RuleType(usize),
    /// The styles to choose the rule's from.
    RuleStyles(usize),
    RuleStyle(usize, String),
    RestoreSnapshot(usize),
    RenameSnapshot(usize),
    DeleteSnapshot(usize),
    OpenVersion(u32),
    RenameVersion(u32),
    DeleteVersion(u32),
    NextCapture(NextCapture),
    /// A card menu's instance or mask row ([`super::Session::mask_menu`]).
    Mask(ModuleKind, MaskAct),
}

/// A typed part of an auto-apply rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum RuleField {
    Maker,
    Model,
    Lens,
    Iso,
}

/// An ISO range as typed: "100-800", "1600-", "-400", "800" (just that), "" (any). `None`:
/// not a range (Err) — `Ok((None, None))` is any.
pub(super) fn parse_iso(typed: &str) -> Result<(Option<u32>, Option<u32>), ()> {
    let t = typed.trim();
    if t.is_empty() {
        return Ok((None, None));
    }
    let num = |s: &str| -> Result<Option<u32>, ()> {
        let s = s.trim();
        if s.is_empty() {
            Ok(None)
        } else {
            s.parse::<u32>().map(Some).map_err(|_| ())
        }
    };
    let (lo, hi) = match t.split_once(['-', '\u{2013}']) {
        Some((a, b)) => (num(a)?, num(b)?),
        None => {
            let v = num(t)?;
            (v, v)
        }
    };
    match (lo, hi) {
        (Some(a), Some(b)) if a > b => Ok((Some(b), Some(a))),
        r => Ok(r),
    }
}

/// An ISO range as it is typed back ("100-800", "1600-", "").
fn iso_text(lo: Option<u32>, hi: Option<u32>) -> String {
    match (lo, hi) {
        (None, None) => String::new(),
        (Some(a), Some(b)) if a == b => a.to_string(),
        (a, b) => format!(
            "{}-{}",
            a.map(|v| v.to_string()).unwrap_or_default(),
            b.map(|v| v.to_string()).unwrap_or_default()
        ),
    }
}

/// Menu row ids: [`items::DEVELOP`] + the row's index in [`State::menu`]; the confirmation
/// of a deletion is [`items::DEVELOP_CONFIRM`].
fn menu_item(i: usize, label: String, icon: Icon) -> PaletteItem {
    PaletteItem {
        id: items::DEVELOP + i as u64,
        label,
        key: None,
        unavailable: None,
        icon,
    }
}

/// The copied edit.
#[derive(Clone, Debug)]
pub(super) struct Clip {
    pub style: Style,
}

/// Batch editing state in Develop: styles, history, snapshots and versions.
#[derive(Default)]
pub(super) struct State {
    pub shelf: Shelf,
    pub clip: Option<Clip>,
    /// The version open, its name, and the photo's versions.
    pub version: u32,
    pub version_name: String,
    pub versions: Vec<(u32, String)>,
    /// The snapshot compared with the edit (split view).
    pub compare: Option<usize>,
    /// The open menu's rows, and the deletion asked about.
    pub menu: Vec<Do>,
    /// The open menu's submenus: the row each opens from, and its rows (their ids index
    /// [`Self::menu`] too).
    pub subs: Vec<(MaskAct, Vec<PaletteItem>)>,
    pub confirm: Option<Do>,
    /// The reference for match to reference.
    pub reference: Option<PathBuf>,
    pub matcher: super::matcher::Matcher,
    /// Batches running in the background: token → (what to say when done).
    pub batches: HashMap<u64, String>,
    /// Their progress (the downloads indicator's rows, with Stop).
    pub progress: HashMap<u64, Arc<BatchProgress>>,
    next_token: u64,
    /// A style file chosen to import, or a place to export one.
    pub import_rx: Option<std::sync::mpsc::Receiver<PathBuf>>,
    pub export_rx: Option<std::sync::mpsc::Receiver<(PathBuf, usize)>>,
    /// The edit of the photo that arrived last (next capture style "same as previous").
    pub last_capture: Option<Edit>,
    /// Batches whose photos are being checked for sidecars changed outside Specular, by
    /// token; the one asked about.
    pub waiting: HashMap<u64, Waiting>,
    pub asking: Option<(u64, Waiting)>,
}

/// A batch waiting to start: its photos not open, its change, what to say when done.
pub(super) struct Waiting {
    pub paths: Vec<PathBuf>,
    pub change: super::store::Change,
    pub done: String,
}

/// The question about a batch's photos whose sidecar changed outside Specular: title and
/// detail.
pub(super) fn conflict_question(count: usize) -> (String, String) {
    (
        t!("develop-batch-conflict-title", count = count as i64),
        [
            t!("develop-batch-conflict-detail"),
            t!("develop-batch-conflict-keep"),
            t!("develop-batch-conflict-replace"),
            t!("develop-batch-conflict-skip-detail"),
        ]
        .join("\n"),
    )
}

/// What a batch conflict answer means; `None` when `id` is not one.
fn conflict_answer(id: u64) -> Option<super::store::OnConflict> {
    use super::store::OnConflict;
    Some(match id {
        items::DEVELOP_BATCH_FILE => OnConflict::File,
        items::DEVELOP_BATCH_LIBRARY => OnConflict::Library,
        items::DEVELOP_BATCH_SKIP => OnConflict::Skip,
        _ => return None,
    })
}

/// How far a background batch (a paste or sync to photos not open) is; Stop ends it after
/// the photo it is on.
#[derive(Debug, Default)]
pub(super) struct BatchProgress {
    pub done: AtomicUsize,
    pub total: usize,
    pub stop: AtomicBool,
}

impl BatchProgress {
    /// `change` counting each photo it is given, and doing nothing more once stopped.
    fn wrap(self: &Arc<Self>, change: super::store::Change) -> super::store::Change {
        let p = Arc::clone(self);
        Arc::new(move |path: &Path, e: &mut Edit, h: &mut History| {
            if p.stop.load(Ordering::Relaxed) {
                return false;
            }
            let changed = change(path, e, h);
            p.done.fetch_add(1, Ordering::Relaxed);
            changed
        })
    }
}

/// Whether card `card` has a menu (presets, and instances and masks): its header gets a ⋯
/// button ([`DevelopPanel::menus`]).
pub(super) fn has_menu(card: u32) -> bool {
    !card_kinds(card).is_empty()
}

/// Modules whose settings belong to one photo (where it is cropped, what was retouched where):
/// no presets, so their cards have no menu.
const NO_PRESETS: [ModuleKind; 3] = [
    ModuleKind::CROP_ORIENTATION,
    ModuleKind::PERSPECTIVE,
    ModuleKind::RETOUCH,
];

/// The kinds a card's presets are for.
fn card_kinds(card: u32) -> Vec<ModuleKind> {
    match card {
        DETAIL_CARD => vec![
            ModuleKind::DENOISE,
            ModuleKind::SHARPEN,
            ModuleKind::CAPTURE_SHARPENING,
        ],
        HDR_CARD => vec![ModuleKind::VIEW_TRANSFORM],
        // Light's six come from three modules.
        c if c == u32::from(ModuleKind::EXPOSURE.0) => vec![
            ModuleKind::EXPOSURE,
            ModuleKind::TONE_EQUALIZER,
            ModuleKind::VIEW_TRANSFORM,
        ],
        controls::ZONES_CARD => vec![ModuleKind::TONE_EQUALIZER],
        controls::GRADING_CARD => vec![ModuleKind::COLOUR_BALANCE],
        c if c == u32::from(ModuleKind::LENS.0) => vec![
            ModuleKind::LENS,
            ModuleKind::CHROMATIC_ABERRATION,
            ModuleKind::DEFRINGE,
        ],
        c if c < 0x1_0000 && !NO_PRESETS.contains(&ModuleKind(c as u16)) => {
            vec![ModuleKind(c as u16)]
        }
        _ => Vec::new(),
    }
}

fn button(id: u32, label: String) -> Control {
    Control {
        id,
        body: Body::Choice(Choice::button(label)),
    }
}

/// A version's name as shown.
pub(super) fn version_label(v: u32, name: &str) -> String {
    if name.is_empty() {
        t!("develop-version-number", number = i64::from(v) + 1)
    } else {
        name.to_owned()
    }
}

/// The three cards, from the lists as they are.
fn cards(st: &State, snapshots: &[Snapshot], expanded: &HashMap<u32, bool>) -> Vec<Card> {
    let open = |id: u32| expanded.get(&id).copied().unwrap_or(false);
    let mut styles: Vec<Control> = st
        .shelf
        .listed()
        .iter()
        .enumerate()
        .map(|(i, l)| button(STYLE_ROW + i as u32, l.name.clone()))
        .collect();
    styles.push(button(STYLE_NEW, t!("develop-style-new")));
    styles.push(button(STYLE_IMPORT, t!("develop-style-import")));
    styles.push(button(STYLE_RULES, t!("develop-rules")));
    let mut history = vec![
        button(SNAP_TAKE, t!("develop-snapshot-take")),
        button(SNAP_COMPRESS, t!("develop-history-compress")),
    ];
    history.extend(snapshots.iter().enumerate().map(|(i, s)| {
        let label = if st.compare == Some(i) {
            t!("develop-snapshot-comparing", name = s.name.clone())
        } else {
            s.name.clone()
        };
        button(SNAP_ROW + i as u32, label)
    }));
    let mut versions: Vec<Control> = st
        .versions
        .iter()
        .map(|(v, n)| {
            let name = version_label(*v, n);
            let label = if *v == st.version {
                t!("develop-version-open", name = name)
            } else {
                name
            };
            button(VERSION_ROW + v, label)
        })
        .collect();
    versions.push(button(VERSION_NEW, t!("develop-version-new")));
    versions.push(button(VERSION_DUP, t!("develop-version-duplicate")));
    vec![
        Card {
            id: STYLES_CARD,
            title: t!("develop-styles"),
            enabled: None,
            expanded: open(STYLES_CARD),
            controls: styles,
        },
        Card {
            id: HISTORY_CARD,
            title: t!("develop-snapshots"),
            enabled: None,
            expanded: open(HISTORY_CARD),
            controls: history,
        },
        Card {
            id: VERSIONS_CARD,
            title: t!("develop-versions"),
            enabled: None,
            expanded: open(VERSIONS_CARD),
            controls: versions,
        },
    ]
}

/// Puts this module's cards (rebuilt from `st`) at the end of `panel`, keeping whether
/// each was open and where the keyboard is.
pub(super) fn refresh_cards(panel: &mut DevelopPanel, st: &State, snapshots: &[Snapshot]) {
    let ours = [STYLES_CARD, HISTORY_CARD, VERSIONS_CARD];
    let expanded: HashMap<u32, bool> = panel
        .cards
        .iter()
        .filter(|c| ours.contains(&c.id))
        .map(|c| (c.id, c.expanded))
        .collect();
    let focus = panel.focus();
    panel.cards.retain(|c| !ours.contains(&c.id));
    panel.cards.extend(cards(st, snapshots, &expanded));
    // A row that went keeps the keyboard in its card.
    if let Some(Item::Control { card, control, .. }) = focus
        && let Some(c) = panel.cards.get(card)
        && control >= c.controls.len()
    {
        panel.set_focus(Some(Item::Header(card)));
    }
}

/// The style a hovered or focused row of the Styles card shows.
pub(super) fn previewed_style(panel: &DevelopPanel, st: &State) -> Option<Style> {
    let item = panel.hovered().or_else(|| panel.focus())?;
    let (card, control) = panel.ids_of(item)?;
    let i = row_of(control?, STYLE_ROW).filter(|_| card == STYLES_CARD)?;
    st.shelf.listed().into_iter().nth(i).map(|l| l.style)
}

/// A photo Develop takes.
pub(super) fn developable(p: &Path) -> bool {
    const EXT: [&str; 10] = [
        "jpg", "jpeg", "tif", "tiff", "png", "heic", "heif", "avif", "jxl", "webp",
    ];
    controls::is_raw(p)
        || p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXT.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// What auto-apply rules look at, read from the file's Exif (make, model, lens, ISO).
pub(super) fn photo_facts(p: &Path) -> PhotoFacts {
    let mut f = PhotoFacts {
        ext: p
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
        raw: controls::is_raw(p),
        ..PhotoFacts::default()
    };
    let tiff = std::fs::File::open(p)
        .ok()
        .and_then(|h| specular_decode::read_exif_tiff(std::io::BufReader::new(h)));
    if let Some(t) = tiff {
        exif_facts(&t, &mut f);
    }
    f
}

/// Make, model, lens and ISO from a TIFF-structured Exif block.
fn exif_facts(t: &[u8], f: &mut PhotoFacts) {
    let le = match t.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return,
    };
    let u16_at = |o: usize| -> Option<u32> {
        let b = t.get(o..o + 2)?;
        Some(u32::from(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        }))
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = t.get(o..o + 4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    };
    // (tag, type, count, value or offset field position) of each entry of the IFD at `o`.
    let entries = |o: usize| -> Vec<(u32, u32, u32, usize)> {
        let Some(n) = u16_at(o) else {
            return Vec::new();
        };
        (0..n.min(512) as usize)
            .filter_map(|i| {
                let e = o + 2 + i * 12;
                Some((u16_at(e)?, u16_at(e + 2)?, u32_at(e + 4)?, e + 8))
            })
            .collect()
    };
    let text = |ty: u32, count: u32, at: usize| -> Option<String> {
        if ty != 2 {
            return None;
        }
        let n = count as usize;
        let start = if n <= 4 { at } else { u32_at(at)? as usize };
        let b = t.get(start..start + n)?;
        let s = String::from_utf8_lossy(b);
        Some(s.trim_end_matches('\0').trim().to_owned())
    };
    let Some(ifd0) = u32_at(4) else { return };
    let mut exif = None;
    for (tag, ty, count, at) in entries(ifd0 as usize) {
        match tag {
            0x010F => f.maker = text(ty, count, at).unwrap_or_default(),
            0x0110 => f.model = text(ty, count, at).unwrap_or_default(),
            0x8769 => exif = u32_at(at),
            _ => {}
        }
    }
    let Some(exif) = exif else { return };
    for (tag, ty, count, at) in entries(exif as usize) {
        match tag {
            0x8827 => {
                f.iso = match ty {
                    3 => u16_at(at),
                    4 => u32_at(at),
                    _ => None,
                }
            }
            0xA434 => f.lens = text(ty, count, at).unwrap_or_default(),
            _ => {}
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl App {
    pub(super) fn develop_say(&mut self, msg: String) {
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The photos a paste or a match goes to: the gallery's selection, else the photo shown.
    fn develop_targets(&self) -> Vec<PathBuf> {
        let v: Vec<PathBuf> = if self.gallery.active {
            self.gallery.targets()
        } else {
            self.current
                .and_then(|i| self.entries.get(i))
                .cloned()
                .into_iter()
                .collect()
        };
        v.into_iter()
            .filter(|p| p.is_file() && developable(p))
            .collect()
    }

    /// The picked photos: cull mode's picks, else the folder's.
    fn develop_picks(&self) -> Vec<PathBuf> {
        if let Some(v) = self.cull_picked() {
            return v;
        }
        self.entries
            .iter()
            .filter(|p| developable(p) && self.gallery_mark_of(p).flag == 1)
            .cloned()
            .collect()
    }

    /// The edit of the photo shown (open in Develop, kept this run, or saved).
    fn develop_current_edit(&self) -> Option<(PathBuf, Edit)> {
        if let Some(s) = self.develop.session.as_ref() {
            return Some((s.path.clone(), s.edit.clone()));
        }
        let p = if self.gallery.active {
            self.gallery.targets().into_iter().next()?
        } else {
            self.current.and_then(|i| self.entries.get(i)).cloned()?
        };
        if !developable(&p) {
            return None;
        }
        if let Some(k) = self.develop.kept.get(&p) {
            return Some((p, k.0.clone()));
        }
        // A small read of the sidecar on a key press (the catalogue is read off this
        // thread only; with sidecars off, a photo not opened this run copies as new).
        let e = super::store::load(&p, None).map_or_else(|| controls::new_edit(&p), |f| f.edit);
        Some((p, e))
    }

    /// Batch editing actions; whether it took `a`.
    pub(in crate::app) fn develop_batch_action(&mut self, a: Action) -> bool {
        self.develop.batch.shelf.load();
        match a {
            Action::DevelopCopyEdit => self.develop_copy(None),
            Action::DevelopCopySelective => self.develop_choose(Purpose::CopySelective),
            Action::DevelopPasteEdit => self.develop_paste(self.develop_targets(), None, None),
            Action::DevelopPasteSelective => self.develop_choose(Purpose::PasteSelective),
            Action::DevelopPastePicks => {
                let picks = self.develop_picks();
                if picks.is_empty() {
                    self.develop_say(t!("develop-no-picks"));
                } else {
                    self.develop_paste(picks, None, None);
                }
            }
            Action::DevelopSync | Action::DevelopSyncSelective => {
                let Some((from, e)) = self.develop_current_edit() else {
                    self.develop_say(t!("develop-nothing-to-copy"));
                    return true;
                };
                let picks: Vec<PathBuf> = self
                    .develop_picks()
                    .into_iter()
                    .filter(|p| *p != from)
                    .collect();
                if picks.is_empty() {
                    self.develop_say(t!("develop-no-picks"));
                    return true;
                }
                let reg = Registry::builtin();
                self.develop.batch.clip = Some(Clip {
                    style: Style::from_edit_default(&t!("develop-history-sync"), &e, reg),
                });
                if a == Action::DevelopSync {
                    self.develop_paste(picks, None, Some(t!("develop-history-sync")));
                } else {
                    self.develop_choose(Purpose::PasteSelective);
                }
            }
            Action::DevelopSetReference => {
                let Some((p, _)) = self.develop_current_edit() else {
                    self.develop_say(t!("develop-nothing-to-copy"));
                    return true;
                };
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.develop.batch.reference = Some(p);
                self.develop_say(t!("develop-reference-set", name = name));
            }
            Action::DevelopMatchReference => self.develop_match(),
            Action::DevelopNextCapture => self.develop_next_capture_menu(),
            Action::DevelopAutoRules => self.develop_rules_menu(),
            Action::DevelopImportStyle => self.develop_import_style(),
            Action::DevelopMergeHdr | Action::DevelopMergePano | Action::DevelopFocusStack => {
                let item = match a {
                    Action::DevelopMergePano => crate::app::gallery::MergeItem::Pano,
                    Action::DevelopFocusStack => crate::app::gallery::MergeItem::Stack,
                    _ => crate::app::gallery::MergeItem::Hdr,
                };
                let targets = self.develop_targets();
                self.gallery_merge_item(item, targets);
            }
            // The rest need Develop open.
            _ if !self.develop.active() => return false,
            Action::DevelopSnapshot => self.develop_name_snapshot(),
            Action::DevelopCompressHistory => self.develop_compress(),
            Action::DevelopNewVersion => self.develop_new_version(false),
            Action::DevelopNewStyle => self.develop_choose(Purpose::NewStyle),
            Action::DevelopBrushSmaller | Action::DevelopBrushLarger
                if self.develop_retouch_size_key(a == Action::DevelopBrushLarger) => {}
            Action::DevelopBrushSmaller
            | Action::DevelopBrushLarger
            | Action::DevelopBrushSofter
            | Action::DevelopBrushHarder
            | Action::DevelopBrushErase
            | Action::DevelopMaskOverlay => {
                let said = self.develop.session.as_mut().map(|s| s.mask_action(a));
                if let Some(said) = said {
                    self.develop_say(said);
                    self.develop.unsaved = Some(self.now());
                }
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// `[` `]` on retouch spots (when they are on the picture): whether it was theirs.
    fn develop_retouch_size_key(&mut self, larger: bool) -> bool {
        let ms = self.develop_ms();
        let said = self
            .develop
            .session
            .as_mut()
            .and_then(|s| s.retouch_size_key(larger, ms));
        let Some(said) = said else {
            return false;
        };
        self.develop_say(said);
        self.develop.unsaved = Some(self.now());
        true
    }

    /// Copies the open (or shown) photo's edit: `kinds` chosen, else all but geometry.
    fn develop_copy(&mut self, kinds: Option<Vec<ModuleKind>>) {
        let Some((p, e)) = self.develop_current_edit() else {
            self.develop_say(t!("develop-nothing-to-copy"));
            return;
        };
        let reg = Registry::builtin();
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let style = match kinds {
            Some(k) => Style::from_kinds(&name, &e, &k),
            None => Style::from_edit_default(&name, &e, reg),
        };
        let n = style.kinds().len() as i64;
        self.develop.batch.clip = Some(Clip { style });
        self.develop_say(t!("develop-copied", count = n, name = name));
    }

    /// The modules of the edit as sheet rows, chosen as copy and paste choose by default.
    fn develop_module_rows(&self, e: &Edit) -> Vec<(String, ModuleKind, bool)> {
        let reg = Registry::builtin();
        let mut v: Vec<(String, ModuleKind, bool)> = Vec::new();
        for m in &e.modules {
            if m.kind == ModuleKind::RAW_FRONT_END || v.iter().any(|x| x.1 == m.kind) {
                continue;
            }
            let geometry = reg.get(m.kind).is_some_and(|s| s.geometry);
            v.push((controls::module_title(m.kind), m.kind, !geometry));
        }
        v
    }

    /// Opens a sheet choosing modules (and a name, for a style).
    fn develop_choose(&mut self, purpose: Purpose) {
        let source = match purpose {
            Purpose::PasteSelective => match &self.develop.batch.clip {
                Some(c) => {
                    let mut e = Edit::empty(Default::default());
                    e.modules.clone_from(&c.style.items);
                    e
                }
                None => {
                    self.develop_say(t!("develop-nothing-copied"));
                    return;
                }
            },
            _ => match self.develop_current_edit() {
                Some((_, e)) => e,
                None => {
                    self.develop_say(t!("develop-nothing-to-copy"));
                    return;
                }
            },
        };
        let rows = self.develop_module_rows(&source);
        let (title, ok) = match purpose {
            Purpose::CopySelective => (t!("develop-copy-title"), t!("develop-copy-ok")),
            Purpose::PasteSelective => (t!("develop-paste-title"), t!("develop-paste-ok")),
            _ => (t!("develop-style-new-title"), t!("develop-style-save")),
        };
        let mut sheet = DevelopSheet::modules(purpose.clone(), title, rows, ok);
        match purpose {
            Purpose::PasteSelective => sheet.mode = Some(0),
            Purpose::NewStyle => {
                let n = self.develop.batch.shelf.styles.len() as i64 + 1;
                let name = t!("develop-style-default-name", number = n);
                let mut f = specular_widgets::line_edit::LineEdit::new(&name);
                f.select(0..name.len());
                sheet.field = Some((t!("develop-name"), f));
            }
            _ => {}
        }
        self.fileops_open_develop(sheet);
    }

    /// Asks for a name for `purpose`, starting from `name`.
    /// Asks for the watermark's text, starting from `text`.
    pub(super) fn develop_watermark_text(&mut self, text: &str) {
        let s = DevelopSheet::name(
            Purpose::WatermarkText,
            t!("develop-frame-text-title"),
            t!("develop-frame-text-field"),
            text,
            t!("develop-frame-text-ok"),
        );
        self.fileops_open_develop(s);
    }

    fn develop_ask_name(&mut self, purpose: Purpose, title: String, name: &str, ok: String) {
        let s = DevelopSheet::name(purpose, title, t!("develop-name"), name, ok);
        self.fileops_open_develop(s);
    }

    /// How the open photo is shown (mask presets are stored upright).
    fn develop_orient(&self) -> specular_develop::preview::Orient {
        self.develop
            .map
            .as_ref()
            .map(|m| m.plan.orient)
            .unwrap_or_default()
    }

    /// A mask preset request from a Masks section.
    pub(super) fn develop_mask_preset(&mut self, q: controls::Request) {
        use controls::Request;
        let shelf = &mut self.develop.batch.shelf;
        shelf.load();
        match q {
            Request::SaveMaskPreset(kind, inst) => {
                let n = shelf.mask_presets.len() as i64 + 1;
                let name = t!("develop-mask-preset-default-name", number = n);
                self.develop_ask_name(
                    Purpose::SaveMaskPreset(kind, inst),
                    t!("develop-mask-preset-save-title"),
                    &name,
                    t!("develop-style-save"),
                );
            }
            Request::ApplyMaskPreset(kind, inst, i) => {
                let Some(p) = shelf.mask_presets.get(i).cloned() else {
                    return;
                };
                let orient = self.develop_orient();
                let ms = self.develop_ms_now();
                let done = self
                    .develop
                    .session
                    .as_mut()
                    .is_some_and(|s| s.apply_mask_preset(kind, inst, &p, orient, ms));
                if done {
                    self.develop.unsaved = Some(self.now());
                    self.develop_say(t!("develop-mask-preset-applied", name = p.name));
                }
            }
            Request::DeleteMaskPreset(i) => {
                let name = shelf.mask_presets.get(i).map(|p| p.name.clone());
                shelf.remove_mask_preset(i);
                self.develop_cards();
                if let Some(name) = name {
                    self.develop_say(t!("develop-mask-preset-deleted", name = name));
                }
            }
            _ => {}
        }
        self.develop_cards();
    }

    /// The sheet's OK.
    pub(in crate::app) fn develop_sheet_ok(&mut self) {
        let Some(s) = self.fileops_take_develop() else {
            return;
        };
        let name = s.typed();
        match s.purpose.clone() {
            Purpose::CopySelective => self.develop_copy(Some(s.chosen())),
            Purpose::MergeHdr(paths) => self.develop_merge_ok(&s, paths),
            Purpose::MergePano(paths) => self.develop_pano_ok(&s, paths),
            Purpose::FocusStack(paths) => self.develop_stack_ok(&s, paths),
            Purpose::MergeFusion(paths) => self.develop_fusion_ok(&s, paths),
            Purpose::PasteSelective => {
                let mode = if s.mode == Some(1) {
                    ApplyMode::Replace
                } else {
                    ApplyMode::Append
                };
                let targets = self.develop_targets();
                self.develop_paste(targets, Some((s.chosen(), mode)), None);
            }
            Purpose::NewStyle => {
                let Some((_, e)) = self.develop_current_edit() else {
                    return;
                };
                let style = Style::from_kinds(&name, &e, &s.chosen());
                self.develop.batch.shelf.put_style(style);
                self.develop_cards();
                self.develop_say(t!("develop-style-saved", name = name));
            }
            Purpose::RenameStyle(i) => {
                self.develop.batch.shelf.rename_style(i, &name);
                self.develop_cards();
            }
            Purpose::SaveMaskPreset(kind, inst) => {
                let orient = self.develop_orient();
                let Some(p) = self.develop.session.as_ref().and_then(|x| {
                    specular_develop::model::MaskPreset::from_module(
                        &name, &x.edit, kind, inst, orient,
                    )
                }) else {
                    return;
                };
                self.develop.batch.shelf.put_mask_preset(p);
                self.develop_cards();
                self.develop_say(t!("develop-mask-preset-saved", name = name));
            }
            Purpose::SavePreset(kind) => {
                let Some(m) = self
                    .develop
                    .session
                    .as_ref()
                    .and_then(|x| x.edit.module(kind, 0).cloned())
                else {
                    return;
                };
                let shelf = &mut self.develop.batch.shelf;
                shelf
                    .presets
                    .retain(|p| !(p.kind == kind && p.name == name));
                shelf.presets.push(Preset::of(&name, &m));
                shelf.save_presets();
                self.develop_say(t!("develop-preset-saved", name = name));
            }
            Purpose::NewSnapshot => {
                if let Some(x) = self.develop.session.as_mut() {
                    let snap = Snapshot::take(&name, &x.edit);
                    x.edit.snapshots.push(snap);
                    self.develop.unsaved = Some(self.now());
                }
                self.develop_cards();
                self.chrome
                    .announce_status(t!("develop-snapshot-taken", name = name));
            }
            Purpose::RenameSnapshot(i) => {
                if let Some(snap) = self
                    .develop
                    .session
                    .as_mut()
                    .and_then(|x| x.edit.snapshots.get_mut(i))
                {
                    snap.name = name;
                    self.develop.unsaved = Some(self.now());
                }
                self.develop_cards();
            }
            Purpose::ChartProfile => self.develop_chart_save(&name),
            Purpose::RuleText(i, field) => self.develop_rule_text(i, field, &name),
            Purpose::CropRatio => self.develop_crop_ratio(&name),
            Purpose::WatermarkText => {
                let ms = self.develop_ms();
                if let Some(x) = self.develop.session.as_mut()
                    && x.set_watermark_text(&name, ms)
                {
                    self.develop.unsaved = Some(self.now());
                }
            }
            Purpose::NewVersion { duplicate } => self.develop_add_version(duplicate, &name),
            Purpose::RenameVersion(v) => {
                if v == self.develop.batch.version {
                    self.develop.batch.version_name.clone_from(&name);
                    self.develop.unsaved = Some(self.now());
                } else if let Some(path) = self.develop.session.as_ref().map(|x| x.path.clone()) {
                    // Another version: read, renamed, written back.
                    let lib = self.gallery.library_handle();
                    let on = self.settings.metadata.develop_sidecar;
                    let n = name.clone();
                    self.loader.background("develop-rename", move |_| {
                        if let Some(f) = super::store::load_version(&path, v, lib.as_deref()) {
                            super::store::save_version(
                                &path,
                                v,
                                &n,
                                &f.edit,
                                &f.history,
                                lib.as_deref(),
                                on,
                            );
                        }
                    });
                }
                if let Some(x) = self.develop.batch.versions.iter_mut().find(|x| x.0 == v) {
                    x.1 = name;
                }
                self.develop_cards();
            }
        }
        self.redraw = true;
    }

    /// Pastes the copied edit (or `chosen` modules of it, in its mode) on `targets`, one
    /// history step each; `what` names the step (else "Paste").
    fn develop_paste(
        &mut self,
        targets: Vec<PathBuf>,
        chosen: Option<(Vec<ModuleKind>, ApplyMode)>,
        what: Option<String>,
    ) {
        let Some(clip) = self.develop.batch.clip.clone() else {
            self.develop_say(t!("develop-nothing-copied"));
            return;
        };
        if targets.is_empty() {
            self.develop_say(t!("develop-no-photos"));
            return;
        }
        let (style, mode) = match chosen {
            Some((k, m)) => {
                let mut s = clip.style.clone();
                s.items.retain(|i| k.contains(&i.kind));
                (s, m)
            }
            None => (clip.style.clone(), ApplyMode::Append),
        };
        let what = what.unwrap_or_else(|| t!("develop-history-paste"));
        let n = targets.len() as i64;
        let change: super::store::Change = {
            let (style, what) = (style.clone(), what.clone());
            Arc::new(move |p: &Path, e: &mut Edit, h: &mut History| {
                // The grain takes each photo's own seed.
                style.with_grain_seed(super::effects::grain_seed(p)).paste(
                    e,
                    h,
                    Registry::builtin(),
                    mode,
                    &what,
                    now_ms(),
                )
            })
        };
        self.develop_apply(targets, change, t!("develop-pasted", count = n));
    }

    /// IPC `apply-style`: the style named `name` on `targets` (the photos Develop can edit),
    /// one history step each, as a batch. `Err(())`: no style of that name; else the
    /// batch's token (none when all were done at once) and the photos it went to.
    pub(in crate::app) fn develop_apply_style_named(
        &mut self,
        targets: Vec<PathBuf>,
        name: &str,
    ) -> Result<(Option<u64>, usize), ()> {
        self.develop.batch.shelf.load();
        let style = self.develop.batch.shelf.style_named(name).ok_or(())?;
        let targets: Vec<PathBuf> = targets.into_iter().filter(|p| developable(p)).collect();
        let n = targets.len();
        if n == 0 {
            return Ok((None, 0));
        }
        let what = name.to_owned();
        let change: super::store::Change =
            Arc::new(move |p: &Path, e: &mut Edit, h: &mut History| {
                style.with_grain_seed(super::effects::grain_seed(p)).paste(
                    e,
                    h,
                    Registry::builtin(),
                    ApplyMode::Append,
                    &what,
                    now_ms(),
                )
            });
        let done = t!("develop-style-applied", name = name.to_owned());
        Ok((self.develop_apply(targets, change, done), n))
    }

    /// Applies `change` to each of `targets`: the open photo and those kept this run here,
    /// the rest in the background; says `done` when all are. The background batch's token,
    /// when there is one.
    fn develop_apply(
        &mut self,
        targets: Vec<PathBuf>,
        change: super::store::Change,
        done: String,
    ) -> Option<u64> {
        let mut rest = Vec::new();
        let lib = self.gallery.library_handle();
        let on = self.settings.metadata.develop_sidecar;
        for p in targets {
            if let Some(s) = self.develop.session.as_mut().filter(|s| s.path == p) {
                if change(&p, &mut s.edit, &mut s.history) {
                    s.sync_panel();
                    self.develop.unsaved = Some(self.now());
                }
                continue;
            }
            if let Some(k) = self.develop.kept.get_mut(&p) {
                if change(&p, &mut k.0, &mut k.1) {
                    // Kept edits are of their open version.
                    self.develop.store.save(
                        &p,
                        k.2,
                        &k.3,
                        &k.0,
                        &k.1,
                        lib.clone(),
                        on,
                        &self.loader,
                    );
                }
                continue;
            }
            rest.push(p);
        }
        if rest.is_empty() {
            self.develop_say(done);
            return None;
        }
        // Photos whose sidecar changed outside Specular are found first; if any, the
        // batch asks once what to do with them.
        let b = &mut self.develop.batch;
        b.next_token += 1;
        let token = b.next_token;
        b.waiting.insert(
            token,
            Waiting {
                paths: rest.clone(),
                change,
                done,
            },
        );
        self.develop.store.scan(token, rest, lib, &self.loader);
        Some(token)
    }

    /// Starts batch `token` in the background, its conflicts handled as `on_conflict`.
    fn develop_batch_start(
        &mut self,
        token: u64,
        w: Waiting,
        on_conflict: super::store::OnConflict,
    ) {
        let lib = self.gallery.library_handle();
        let on = self.settings.metadata.develop_sidecar;
        let b = &mut self.develop.batch;
        b.batches.insert(token, w.done);
        let progress = Arc::new(BatchProgress {
            total: w.paths.len(),
            ..BatchProgress::default()
        });
        let change = progress.wrap(w.change);
        b.progress.insert(token, progress);
        self.develop
            .store
            .apply(token, w.paths, change, on_conflict, lib, on, &self.loader);
        self.net_refresh();
    }

    /// A batch's photos were checked: it starts, or asks about those changed outside
    /// Specular.
    fn develop_batch_scanned(&mut self, token: u64, conflicts: Vec<PathBuf>) {
        let Some(w) = self.develop.batch.waiting.remove(&token) else {
            return;
        };
        if conflicts.is_empty() {
            self.develop_batch_start(token, w, super::store::OnConflict::File);
            return;
        }
        // A newer question replaces one not answered (that batch is dropped).
        let (title, detail) = conflict_question(conflicts.len());
        self.develop.batch.asking = Some((token, w));
        self.chrome.confirm_choice3(
            title,
            detail,
            t!("develop-batch-conflict-file"),
            items::DEVELOP_BATCH_FILE,
            t!("develop-batch-conflict-library"),
            items::DEVELOP_BATCH_LIBRARY,
            t!("develop-batch-conflict-skip"),
            items::DEVELOP_BATCH_SKIP,
        );
        self.redraw = true;
    }

    /// The answer about a batch's conflicts; false when `id` is not one.
    fn develop_batch_conflict_item(&mut self, id: u64) -> bool {
        let Some(on) = conflict_answer(id) else {
            return false;
        };
        if let Some((token, w)) = self.develop.batch.asking.take() {
            self.develop_batch_start(token, w, on);
        }
        true
    }

    /// The downloads indicator's rows for the batches saving in the background.
    pub(in crate::app) fn develop_batch_rows(&self) -> Vec<DownloadRow> {
        let mut tokens: Vec<_> = self.develop.batch.progress.keys().copied().collect();
        tokens.sort_unstable();
        tokens
            .into_iter()
            .map(|k| {
                let p = &self.develop.batch.progress[&k];
                let done = p.done.load(Ordering::Relaxed).min(p.total);
                let stopping = p.stop.load(Ordering::Relaxed);
                DownloadRow {
                    title: t!("develop-batch-row"),
                    detail: t!(
                        "develop-batch-row-detail",
                        done = done as i64,
                        total = p.total as i64
                    ),
                    status: if stopping {
                        t!("develop-batch-stopping")
                    } else {
                        t!("develop-batch-saving")
                    },
                    fraction: Some(done as f32 / p.total.max(1) as f32),
                    state: DownloadState::Running,
                    stop: (!stopping).then(|| (t!("net-stop"), format!("{BATCH_STOP}{k}"))),
                }
            })
            .collect()
    }

    /// The downloads panel's Stop for batch `key` ("develop-batch-stop:<token>"): the
    /// photos not yet saved are left as they were. True when the key was a batch's.
    pub(in crate::app) fn develop_batch_stop(&mut self, key: &str) -> bool {
        let Some(token) = key.strip_prefix(BATCH_STOP).and_then(|t| t.parse().ok()) else {
            return false;
        };
        if let Some(p) = self.develop.batch.progress.get(&token) {
            p.stop.store(true, Ordering::Relaxed);
        }
        true
    }

    /// Match to reference: the targets' exposure and white balance solved from the
    /// reference's.
    fn develop_match(&mut self) {
        let Some(reference) = self.develop.batch.reference.clone() else {
            self.develop_say(t!("develop-no-reference"));
            return;
        };
        let targets: Vec<PathBuf> = if self.cull_picked().is_some() && !self.develop.active() {
            self.develop_picks()
        } else {
            self.develop_targets()
        };
        let targets: Vec<PathBuf> = targets.into_iter().filter(|p| *p != reference).collect();
        if targets.is_empty() {
            self.develop_say(t!("develop-no-photos"));
            return;
        }
        let mut open = Vec::new();
        if let Some(s) = self.develop.session.as_ref() {
            open.push((
                s.path.clone(),
                s.edit.clone(),
                s.history.clone(),
                self.develop.batch.version_name.clone(),
            ));
        }
        for (p, k) in &self.develop.kept {
            open.push((p.clone(), k.0.clone(), k.1.clone(), k.3.clone()));
        }
        let n = targets.len() as i64;
        let lib = self.gallery.library_handle();
        self.develop
            .batch
            .matcher
            .start(&reference, targets, open, lib, &self.loader);
        self.chrome
            .announce_status(t!("develop-matching", count = n));
        self.redraw = true;
    }

    /// Results of batches and matches, files chosen in dialogs.
    pub(super) fn develop_batch_tick(&mut self) {
        while let Some((token, conflicts)) = self.develop.store.scanned() {
            self.develop_batch_scanned(token, conflicts);
        }
        while let Some((token, _changed)) = self.develop.store.applied() {
            let stopped = self
                .develop
                .batch
                .progress
                .remove(&token)
                .filter(|p| p.stop.load(Ordering::Relaxed));
            if let Some(msg) = self.develop.batch.batches.remove(&token) {
                match stopped {
                    Some(p) => self.develop_say(t!(
                        "develop-batch-stopped",
                        done = p.done.load(Ordering::Relaxed).min(p.total) as i64,
                        total = p.total as i64
                    )),
                    None => self.develop_say(msg),
                }
            }
            self.net_refresh();
        }
        let done = std::mem::take(&mut self.develop.batch.matcher.done);
        let (mut ok, mut bad) = (0i64, 0i64);
        let lib = self.gallery.library_handle();
        let on = self.settings.metadata.develop_sidecar;
        for m in done {
            match m.result {
                Ok((e, h, name)) => {
                    ok += 1;
                    if let Some(s) = self.develop.session.as_mut().filter(|s| s.path == m.path) {
                        s.edit = e;
                        s.history = h;
                        s.sync_panel();
                        self.develop.unsaved = Some(self.now());
                    } else if let Some(k) = self.develop.kept.get_mut(&m.path) {
                        k.0 = e;
                        k.1 = h;
                        self.develop.store.save(
                            &m.path,
                            k.2,
                            &k.3,
                            &k.0,
                            &k.1,
                            lib.clone(),
                            on,
                            &self.loader,
                        );
                    } else {
                        self.develop.store.save(
                            &m.path,
                            0,
                            &name,
                            &e,
                            &h,
                            lib.clone(),
                            on,
                            &self.loader,
                        );
                    }
                }
                Err(_) => bad += 1,
            }
        }
        if ok + bad > 0 && !self.develop.batch.matcher.busy() {
            let msg = if bad == 0 {
                t!("develop-matched", count = ok)
            } else {
                t!("develop-matched-some", count = ok, failed = bad)
            };
            self.develop_say(msg);
        }
        if let Some(why) = self.develop.batch.matcher.failed.take() {
            let _ = why;
            self.develop_say(t!("develop-reference-unreadable"));
        }
        if let Some(p) = self
            .develop
            .batch
            .import_rx
            .as_ref()
            .and_then(|r| r.try_recv().ok())
        {
            self.develop.batch.import_rx = None;
            self.develop_import_file(&p);
        }
        if let Some((p, i)) = self
            .develop
            .batch
            .export_rx
            .as_ref()
            .and_then(|r| r.try_recv().ok())
        {
            self.develop.batch.export_rx = None;
            if let Some(l) = self.develop.batch.shelf.listed().get(i) {
                let text = files::style_to_xmp(&l.style);
                match specular_develop::sidecar::write_text(&p, &text) {
                    Ok(()) => self.develop_say(t!("develop-style-exported", name = l.name.clone())),
                    Err(e) => {
                        warn("exporting a style", e);
                        self.develop_say(t!("develop-style-export-failed"));
                    }
                }
            }
        }
    }

    /// Whether the matcher wants the GPU this frame.
    pub(super) fn develop_matching(&self) -> bool {
        self.develop.batch.matcher.busy()
    }

    fn develop_import_style(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.develop.batch.import_rx = Some(rx);
        let title = t!("develop-style-import-dialog");
        let kind = t!("develop-style-file-kind");
        self.loader.background("dlg-style", move |l| {
            let d = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&kind, &["xmp"]);
            if let Some(p) = d.pick_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
    }

    fn develop_import_file(&mut self, p: &Path) {
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let r = std::fs::read_to_string(p)
            .map_err(|e| e.to_string())
            .and_then(|t| {
                files::style_from_xmp(&t, Registry::builtin(), &stem).map_err(|e| e.to_string())
            });
        match r {
            Ok((s, _)) if !s.items.is_empty() => {
                let name = s.name.clone();
                self.develop.batch.shelf.put_style(s);
                self.develop_cards();
                self.develop_say(t!("develop-style-imported", name = name));
            }
            _ => self.develop_say(t!("develop-style-not-a-style")),
        }
    }

    /// A panel card's or row's menu at `at` (logical px); whether there was one.
    pub(super) fn develop_menu(&mut self, item: Item, at: (f32, f32)) -> bool {
        let Some(s) = self.develop.session.as_ref() else {
            return false;
        };
        let Some((card, control)) = s.panel.ids_of(item) else {
            return false;
        };
        self.develop.batch.shelf.load();
        let mut rows: Vec<(Do, String, Icon)> = Vec::new();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut subs: Vec<(MaskAct, Vec<PaletteItem>)> = Vec::new();
        let title: Option<String>;
        let listed = self.develop.batch.shelf.listed();
        if let Some(i) = control.and_then(|c| row_of(c, STYLE_ROW)) {
            let Some(l) = listed.get(i) else {
                return false;
            };
            title = Some(l.name.clone());
            let mut g = vec![
                (
                    Do::ApplyStyle(i, ApplyMode::Append),
                    t!("develop-style-apply"),
                    Icon::Check,
                ),
                (
                    Do::ApplyStyle(i, ApplyMode::Replace),
                    t!("develop-style-apply-replace"),
                    Icon::Check,
                ),
                (Do::AutoApplyStyle(i), t!("develop-style-auto"), Icon::Plus),
                (
                    Do::ExportStyle(i),
                    t!("develop-style-export"),
                    Icon::Download,
                ),
            ];
            if let Some(u) = l.user {
                g.push((Do::RenameStyle(u), t!("develop-rename"), Icon::Sliders));
                g.push((Do::DeleteStyle(u), t!("develop-delete"), Icon::Trash));
            }
            rows.extend(g);
            groups.push((0..rows.len()).collect());
        } else if let Some(i) = control.and_then(|c| row_of(c, SNAP_ROW)) {
            let Some(snap) = s.edit.snapshots.get(i) else {
                return false;
            };
            title = Some(snap.name.clone());
            rows.push((
                Do::RestoreSnapshot(i),
                t!("develop-snapshot-restore"),
                Icon::Undo,
            ));
            rows.push((Do::RenameSnapshot(i), t!("develop-rename"), Icon::Sliders));
            rows.push((Do::DeleteSnapshot(i), t!("develop-delete"), Icon::Trash));
            groups.push((0..rows.len()).collect());
        } else if let Some(v) = control.and_then(|c| row_of(c, VERSION_ROW)) {
            let v = v as u32;
            let Some((_, n)) = self.develop.batch.versions.iter().find(|x| x.0 == v) else {
                return false;
            };
            title = Some(version_label(v, n));
            if v != self.develop.batch.version {
                rows.push((
                    Do::OpenVersion(v),
                    t!("develop-version-open-it"),
                    Icon::Check,
                ));
            }
            rows.push((Do::RenameVersion(v), t!("develop-rename"), Icon::Sliders));
            if self.develop.batch.versions.len() > 1 {
                rows.push((Do::DeleteVersion(v), t!("develop-delete"), Icon::Trash));
            }
            groups.push((0..rows.len()).collect());
        } else {
            let kinds = card_kinds(card);
            if kinds.is_empty() {
                return false;
            }
            title = s
                .panel
                .cards
                .iter()
                .find(|c| c.id == card)
                .map(|c| c.title.clone());
            // A module's own card: adding masks and its instances first. Submenu rows are
            // rows too (their ids are their index), in no group: their submenu lists them.
            if let Some(kind) = super::masks::mask_module(card) {
                let menu = s.mask_menu(kind);
                for g in menu.groups {
                    let start = rows.len();
                    rows.extend(g.into_iter().map(|(a, l, i)| (Do::Mask(kind, a), l, i)));
                    groups.push((start..rows.len()).collect());
                }
                for (parent, sub) in menu.subs {
                    let items = sub
                        .into_iter()
                        .map(|(a, l, i)| {
                            rows.push((Do::Mask(kind, a), l.clone(), i));
                            menu_item(rows.len() - 1, l, i)
                        })
                        .collect();
                    subs.push((parent, items));
                }
            }
            let shelf = &self.develop.batch.shelf;
            let mut apply = Vec::new();
            let mut save = Vec::new();
            let mut delete = Vec::new();
            for k in &kinds {
                for (name, p, user) in shelf.presets_of(*k) {
                    apply.push((Do::ApplyPreset(p), name.clone(), Icon::Check));
                    if let Some(u) = user {
                        delete.push((
                            Do::DeletePreset(u),
                            t!("develop-preset-delete", name = name),
                            Icon::Trash,
                        ));
                    }
                }
                let label = if kinds.len() > 1 {
                    t!(
                        "develop-preset-save-module",
                        module = controls::module_title(*k)
                    )
                } else {
                    t!("develop-preset-save")
                };
                save.push((Do::SavePreset(*k), label, Icon::Plus));
            }
            for g in [apply, save, delete] {
                if !g.is_empty() {
                    let start = rows.len();
                    rows.extend(g);
                    groups.push((start..rows.len()).collect());
                }
            }
        }
        if rows.is_empty() {
            return false;
        }
        let items: Vec<Vec<PaletteItem>> = groups
            .iter()
            .map(|g| {
                g.iter()
                    .map(|&i| menu_item(i, rows[i].1.clone(), rows[i].2))
                    .collect()
            })
            .collect();
        self.develop.batch.menu = rows.into_iter().map(|r| r.0).collect();
        self.develop.batch.subs = subs;
        self.chrome.open_menu(at, title, items);
        self.develop.menu_closed();
        if let Some(s) = self.develop.session.as_mut() {
            s.panel.menu_open = control.is_none().then_some(card);
        }
        self.redraw = true;
        true
    }

    /// A menu row or a confirmation; whether `id` was Develop's.
    pub(in crate::app) fn develop_item(&mut self, id: u64) -> bool {
        if self.develop_conflict_item(id) || self.develop_batch_conflict_item(id) {
            return true;
        }
        if id == items::DEVELOP_CONFIRM {
            if let Some(d) = self.develop.batch.confirm.take() {
                self.develop_do(d, true);
            }
            return true;
        }
        if !(items::DEVELOP..items::DEVELOP + 0x1_0000).contains(&id) {
            return false;
        }
        let i = (id - items::DEVELOP) as usize;
        if let Some(d) = self.develop.batch.menu.get(i).cloned() {
            self.develop_do(d, false);
        }
        true
    }

    /// Asks before deleting `what` (`d` runs when accepted).
    fn develop_ask(&mut self, d: Do, title: String, detail: String) {
        self.develop.batch.confirm = Some(d);
        self.chrome.confirm_destructive(
            title,
            detail,
            t!("develop-delete"),
            items::DEVELOP_CONFIRM,
        );
    }

    /// A row that leads to more ("Add mask ›", …) pointed at or chosen: its rows, beside it;
    /// without the menu (it was chosen as it closed), in a menu where it was.
    fn develop_mask_sub(&mut self, parent: MaskAct, title: String) {
        let Some((_, items)) = self.develop.batch.subs.iter().find(|(p, _)| *p == parent) else {
            return;
        };
        if let Err(items) = self.chrome.open_submenu(items.clone(), None)
            && let Some(at) = self.chrome.menu_origin()
        {
            self.chrome.open_menu(at, Some(title), vec![items]);
        }
        self.redraw = true;
    }

    /// Does `d`; `confirmed`: a deletion was accepted.
    fn develop_do(&mut self, d: Do, confirmed: bool) {
        let ms = self.develop_ms_now();
        match d {
            Do::Mask(_, act) if act.is_submenu() => {
                let title = match act {
                    MaskAct::FromMenu => t!("develop-mask-from"),
                    MaskAct::PresetMenu => t!("develop-mask-preset"),
                    _ => t!("develop-mask-add"),
                };
                self.develop_mask_sub(act, title);
            }
            Do::Mask(kind, act) => {
                let r = self
                    .develop
                    .session
                    .as_mut()
                    .map(|s| s.mask_act(kind, act, ms))
                    .unwrap_or_default();
                self.develop_requests(r);
            }
            Do::ApplyPreset(p) => {
                let name = I18n::global().tr(&p.name);
                if let Some(s) = self.develop.session.as_mut()
                    && let Some(m) = s.edit.module(p.kind, 0).cloned()
                {
                    let before = m.params.to_fields();
                    let mut after = before.clone();
                    after.overlay(&p.fields);
                    let step = Step::params(p.kind, 0, &before, &after, ms);
                    let mut on = Vec::new();
                    if !m.enabled {
                        on.push(Step {
                            kind: p.kind,
                            instance: 0,
                            change: specular_develop::model::Change::Enabled(true),
                            time_ms: ms,
                        });
                    }
                    if before != after && s.history.push(&mut s.edit, step).is_ok() {
                        for st in on {
                            let _ = s.history.push(&mut s.edit, st);
                        }
                        s.sync_panel();
                        self.develop.unsaved = Some(self.now());
                    }
                }
                self.chrome
                    .announce_status(t!("develop-preset-applied", name = name));
            }
            Do::SavePreset(kind) => self.develop_ask_name(
                Purpose::SavePreset(kind),
                t!(
                    "develop-preset-save-title",
                    module = controls::module_title(kind)
                ),
                &t!("develop-preset-default-name"),
                t!("develop-save"),
            ),
            Do::DeletePreset(i) if confirmed => {
                let shelf = &mut self.develop.batch.shelf;
                if i < shelf.presets.len() {
                    shelf.presets.remove(i);
                    shelf.save_presets();
                }
            }
            Do::DeletePreset(i) => {
                let Some(name) = self
                    .develop
                    .batch
                    .shelf
                    .presets
                    .get(i)
                    .map(|p| p.name.clone())
                else {
                    return;
                };
                self.develop_ask(
                    d,
                    t!("develop-preset-delete-ask", name = name),
                    t!("develop-preset-delete-detail"),
                );
            }
            Do::ApplyStyle(i, mode) => {
                let Some(l) = self.develop.batch.shelf.listed().into_iter().nth(i) else {
                    return;
                };
                if let Some(s) = self.develop.session.as_mut()
                    && l.style
                        .with_grain_seed(super::effects::grain_seed(&s.path))
                        .paste(
                            &mut s.edit,
                            &mut s.history,
                            Registry::builtin(),
                            mode,
                            &l.name,
                            ms,
                        )
                {
                    s.sync_panel();
                    self.develop.unsaved = Some(self.now());
                }
                self.chrome
                    .announce_status(t!("develop-style-applied", name = l.name));
            }
            Do::RenameStyle(i) => {
                let Some(name) = self
                    .develop
                    .batch
                    .shelf
                    .styles
                    .get(i)
                    .map(|s| s.name.clone())
                else {
                    return;
                };
                self.develop_ask_name(
                    Purpose::RenameStyle(i),
                    t!("develop-style-rename-title"),
                    &name,
                    t!("develop-rename"),
                );
            }
            Do::ExportStyle(i) => {
                let Some(l) = self.develop.batch.shelf.listed().into_iter().nth(i) else {
                    return;
                };
                let (tx, rx) = std::sync::mpsc::channel();
                self.develop.batch.export_rx = Some(rx);
                let title = t!("develop-style-export-dialog");
                let file = files::style_file_name(&l.name);
                self.loader.background("dlg-style", move |lo| {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title(&title)
                        .set_file_name(&file)
                        .save_file()
                    {
                        let _ = tx.send((p, i));
                        lo.waker()();
                    }
                });
            }
            Do::AutoApplyStyle(i) => {
                let Some(l) = self.develop.batch.shelf.listed().into_iter().nth(i) else {
                    return;
                };
                let Some((p, _)) = self.develop_current_edit() else {
                    return;
                };
                let f = photo_facts(&p);
                let rule = AutoRule {
                    style: l.name.clone(),
                    maker: f.maker.clone(),
                    model: f.model.clone(),
                    file_type: if f.raw {
                        FileType::Raw
                    } else {
                        FileType::Other
                    },
                    ..AutoRule::default()
                };
                let shelf = &mut self.develop.batch.shelf;
                shelf.rules.push(rule);
                shelf.save_rules();
                let camera = if f.model.is_empty() {
                    t!("develop-rule-any-camera")
                } else {
                    f.model
                };
                self.develop_say(t!("develop-rule-added", name = l.name, camera = camera));
            }
            Do::DeleteStyle(i) if confirmed => {
                self.develop.batch.shelf.remove_style(i);
                self.develop_cards();
            }
            Do::DeleteStyle(i) => {
                let Some(name) = self
                    .develop
                    .batch
                    .shelf
                    .styles
                    .get(i)
                    .map(|s| s.name.clone())
                else {
                    return;
                };
                self.develop_ask(
                    d,
                    t!("develop-style-delete-ask", name = name),
                    t!("develop-style-delete-detail"),
                );
            }
            Do::DeleteRule(i) if confirmed => {
                let shelf = &mut self.develop.batch.shelf;
                if i < shelf.rules.len() {
                    shelf.rules.remove(i);
                    shelf.save_rules();
                }
            }
            Do::EditRule(i) => self.develop_rule_menu(i),
            Do::RuleText(i, field) => {
                let Some(r) = self.develop.batch.shelf.rules.get(i) else {
                    return;
                };
                let (label, now) = match field {
                    RuleField::Maker => (t!("develop-rule-field-maker"), r.maker.clone()),
                    RuleField::Model => (t!("develop-rule-field-model"), r.model.clone()),
                    RuleField::Lens => (t!("develop-rule-field-lens"), r.lens.clone()),
                    RuleField::Iso => {
                        (t!("develop-rule-field-iso"), iso_text(r.iso_min, r.iso_max))
                    }
                };
                let sheet = DevelopSheet::name(
                    Purpose::RuleText(i, field),
                    t!("develop-rule-field-title", rule = rule_text(r)),
                    label,
                    &now,
                    t!("develop-rule-field-ok"),
                );
                self.fileops_open_develop(sheet);
            }
            Do::RuleType(i) => {
                let shelf = &mut self.develop.batch.shelf;
                let Some(r) = shelf.rules.get_mut(i) else {
                    return;
                };
                r.file_type = match r.file_type {
                    FileType::Any => FileType::Raw,
                    FileType::Raw => FileType::Other,
                    FileType::Other => FileType::Any,
                };
                let said = rule_text(r);
                shelf.save_rules();
                self.develop_say(said);
                self.develop_rule_menu(i);
            }
            Do::RuleStyles(i) => {
                let names: Vec<String> = self
                    .develop
                    .batch
                    .shelf
                    .listed()
                    .into_iter()
                    .map(|l| l.name)
                    .collect();
                let current = self
                    .develop
                    .batch
                    .shelf
                    .rules
                    .get(i)
                    .map(|r| r.style.clone())
                    .unwrap_or_default();
                self.develop.batch.menu =
                    names.iter().map(|n| Do::RuleStyle(i, n.clone())).collect();
                let items = names
                    .into_iter()
                    .enumerate()
                    .map(|(k, n)| {
                        let icon = if n == current {
                            Icon::Check
                        } else {
                            Icon::Minus
                        };
                        menu_item(k, n, icon)
                    })
                    .collect();
                self.chrome
                    .open_list(t!("develop-rule-style-placeholder"), items, None);
            }
            Do::RuleStyle(i, name) => {
                let shelf = &mut self.develop.batch.shelf;
                let Some(r) = shelf.rules.get_mut(i) else {
                    return;
                };
                r.style = name;
                let said = rule_text(r);
                shelf.save_rules();
                self.develop_say(said);
            }
            Do::DeleteRule(i) => {
                let Some(r) = self.develop.batch.shelf.rules.get(i) else {
                    return;
                };
                let name = rule_text(r);
                self.develop_ask(
                    d,
                    t!("develop-rule-delete-ask", rule = name),
                    t!("develop-rule-delete-detail"),
                );
            }
            Do::RestoreSnapshot(i) => {
                if let Some(s) = self.develop.session.as_mut()
                    && let Some(snap) = s.edit.snapshots.get(i).cloned()
                {
                    let after = snap.edit_from(&s.edit);
                    let what = t!("develop-history-snapshot", name = snap.name.clone());
                    let step = Step::stack(&what, &s.edit, &after, ms);
                    if !step.is_noop() && s.history.push(&mut s.edit, step).is_ok() {
                        s.sync_panel();
                        self.develop.unsaved = Some(self.now());
                    }
                }
                self.develop.batch.compare = None;
                self.develop_cards();
            }
            Do::RenameSnapshot(i) => {
                let Some(name) = self
                    .develop
                    .session
                    .as_ref()
                    .and_then(|s| s.edit.snapshots.get(i))
                    .map(|s| s.name.clone())
                else {
                    return;
                };
                self.develop_ask_name(
                    Purpose::RenameSnapshot(i),
                    t!("develop-snapshot-rename-title"),
                    &name,
                    t!("develop-rename"),
                );
            }
            Do::DeleteSnapshot(i) if confirmed => {
                if let Some(s) = self.develop.session.as_mut()
                    && i < s.edit.snapshots.len()
                {
                    s.edit.snapshots.remove(i);
                    self.develop.unsaved = Some(self.now());
                }
                self.develop.batch.compare = None;
                self.develop_cards();
            }
            Do::DeleteSnapshot(i) => {
                let Some(name) = self
                    .develop
                    .session
                    .as_ref()
                    .and_then(|s| s.edit.snapshots.get(i))
                    .map(|s| s.name.clone())
                else {
                    return;
                };
                self.develop_ask(
                    d,
                    t!("develop-snapshot-delete-ask", name = name),
                    t!("develop-snapshot-delete-detail"),
                );
            }
            Do::OpenVersion(v) => self.develop_open_version(v),
            Do::RenameVersion(v) => {
                let name = self
                    .develop
                    .batch
                    .versions
                    .iter()
                    .find(|x| x.0 == v)
                    .map(|x| version_label(x.0, &x.1))
                    .unwrap_or_default();
                self.develop_ask_name(
                    Purpose::RenameVersion(v),
                    t!("develop-version-rename-title"),
                    &name,
                    t!("develop-rename"),
                );
            }
            Do::DeleteVersion(v) if confirmed => self.develop_delete_version(v),
            Do::DeleteVersion(v) => {
                let name = self
                    .develop
                    .batch
                    .versions
                    .iter()
                    .find(|x| x.0 == v)
                    .map(|x| version_label(x.0, &x.1))
                    .unwrap_or_default();
                self.develop_ask(
                    d,
                    t!("develop-version-delete-ask", name = name),
                    t!("develop-version-delete-detail"),
                );
            }
            Do::NextCapture(n) => {
                let shelf = &mut self.develop.batch.shelf;
                shelf.next = n.clone();
                shelf.save_next();
                let msg = match n {
                    NextCapture::Off => t!("develop-next-off"),
                    NextCapture::SameAsPrevious => t!("develop-next-previous"),
                    NextCapture::Style(s) => t!("develop-next-style", name = s),
                };
                self.develop_say(msg);
            }
        }
        self.redraw = true;
    }

    fn develop_ms_now(&self) -> i64 {
        (self.now() * 1000.0) as i64
    }

    /// The panel's cards of this module, rebuilt.
    pub(super) fn develop_cards(&mut self) {
        let d = &mut self.develop;
        if let Some(s) = d.session.as_mut() {
            let snaps = s.edit.snapshots.clone();
            refresh_cards(&mut s.panel, &d.batch, &snaps);
            let names: Vec<String> = d
                .batch
                .shelf
                .mask_presets
                .iter()
                .map(|m| m.name.clone())
                .collect();
            if s.masks.preset_names != names {
                s.masks.preset_names = names;
                s.sync_panel();
            }
        }
        self.develop_others_card();
        self.redraw = true;
    }

    /// One of this module's controls pressed.
    pub(super) fn develop_batch_control(&mut self, control: u32) {
        let ms = self.develop_ms_now();
        match control {
            STYLE_NEW => self.develop_choose(Purpose::NewStyle),
            STYLE_IMPORT => self.develop_import_style(),
            STYLE_RULES => self.develop_rules_menu(),
            SNAP_TAKE => self.develop_name_snapshot(),
            SNAP_COMPRESS => self.develop_compress(),
            VERSION_NEW => self.develop_new_version(false),
            VERSION_DUP => self.develop_new_version(true),
            c => {
                if let Some(i) = row_of(c, STYLE_ROW) {
                    self.develop_do(Do::ApplyStyle(i, ApplyMode::Append), false);
                } else if let Some(i) = row_of(c, SNAP_ROW) {
                    let b = &mut self.develop.batch;
                    b.compare = if b.compare == Some(i) { None } else { Some(i) };
                    let msg = match (b.compare, self.develop.session.as_ref()) {
                        (Some(i), Some(s)) => t!(
                            "develop-snapshot-comparing",
                            name = s
                                .edit
                                .snapshots
                                .get(i)
                                .map(|x| x.name.clone())
                                .unwrap_or_default()
                        ),
                        _ => t!("develop-snapshot-compare-off"),
                    };
                    self.chrome.announce_status(msg);
                    self.develop_cards();
                } else if let Some(v) = row_of(c, VERSION_ROW) {
                    self.develop_open_version(v as u32);
                }
            }
        }
        let _ = ms;
        self.redraw = true;
    }

    fn develop_name_snapshot(&mut self) {
        let n = self
            .develop
            .session
            .as_ref()
            .map_or(0, |s| s.edit.snapshots.len()) as i64
            + 1;
        self.develop_ask_name(
            Purpose::NewSnapshot,
            t!("develop-snapshot-take-title"),
            &t!("develop-snapshot-default-name", number = n),
            t!("develop-snapshot-take"),
        );
    }

    fn develop_compress(&mut self) {
        let Some(s) = self.develop.session.as_mut() else {
            return;
        };
        let gone = s.history.compress() as i64;
        s.sync_panel();
        if gone > 0 {
            self.develop.unsaved = Some(self.now());
        }
        self.chrome
            .announce_status(t!("develop-history-compressed", count = gone));
        self.develop_say(t!("develop-history-compressed", count = gone));
    }

    /// Asks a name for a new version (empty, or a copy of the one open).
    fn develop_new_version(&mut self, duplicate: bool) {
        let n = self
            .develop
            .batch
            .versions
            .iter()
            .map(|v| v.0)
            .max()
            .unwrap_or(0)
            + 1;
        self.develop_ask_name(
            Purpose::NewVersion { duplicate },
            if duplicate {
                t!("develop-version-duplicate-title")
            } else {
                t!("develop-version-new-title")
            },
            &version_label(n, ""),
            t!("develop-version-create"),
        );
    }

    fn develop_add_version(&mut self, duplicate: bool, name: &str) {
        let Some(path) = self.develop.session.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        // The version open is saved before the new one takes its place.
        self.develop_save();
        let v = self
            .develop
            .batch
            .versions
            .iter()
            .map(|v| v.0)
            .max()
            .unwrap_or(0)
            + 1;
        let edit = match (duplicate, self.develop.session.as_ref()) {
            (true, Some(s)) => {
                let mut e = s.edit.clone();
                e.snapshots.clear();
                e
            }
            _ => controls::new_edit(&path),
        };
        // Saved now, so it is there for the gallery and the next open.
        let lib = self.gallery.library_handle();
        let on = self.settings.metadata.develop_sidecar;
        let name = if name == version_label(v, "") {
            ""
        } else {
            name
        };
        self.develop.store.save(
            &path,
            v,
            name,
            &edit,
            &History::default(),
            lib,
            on,
            &self.loader,
        );
        self.develop.batch.versions.push((v, name.to_owned()));
        self.develop_switch_version(v, name.to_owned(), edit, History::default());
        self.gallery_set_versions(&path, self.develop.batch.versions.len() as u32);
        self.develop_say(t!("develop-version-created", name = version_label(v, name)));
    }

    /// Saves the version open and opens version `v` of the same photo.
    pub(super) fn develop_open_version(&mut self, v: u32) {
        if v == self.develop.batch.version {
            return;
        }
        let Some(path) = self.develop.session.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        self.develop_save();
        let name = self
            .develop
            .batch
            .versions
            .iter()
            .find(|x| x.0 == v)
            .map(|x| x.1.clone())
            .unwrap_or_default();
        self.develop_switch_version(v, name, controls::new_edit(&path), History::default());
        let lib = self.gallery.library_handle();
        self.develop.store.load(&path, v, lib, &self.loader);
        self.chrome.announce_status(t!(
            "develop-version-opened",
            name = version_label(v, &self.develop.batch.version_name)
        ));
    }

    fn develop_switch_version(&mut self, v: u32, name: String, edit: Edit, history: History) {
        let Some(path) = self.develop.session.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        self.develop.batch.version = v;
        self.develop.batch.version_name = name;
        self.develop.batch.compare = None;
        let focus = self.develop.session.as_ref().and_then(|s| s.panel.focus());
        let mut s = super::controls::Session::new(path, edit, history);
        if focus.is_some() {
            s.panel.set_focus(focus);
        }
        self.develop.session = Some(s);
        self.develop_cards();
        self.develop.unsaved = None;
    }

    fn develop_delete_version(&mut self, v: u32) {
        let Some(path) = self.develop.session.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        if self.develop.batch.versions.len() <= 1 {
            return;
        }
        let name = self
            .develop
            .batch
            .versions
            .iter()
            .find(|x| x.0 == v)
            .map(|x| version_label(x.0, &x.1))
            .unwrap_or_default();
        if v == self.develop.batch.version {
            let other = self
                .develop
                .batch
                .versions
                .iter()
                .map(|x| x.0)
                .find(|&x| x != v)
                .unwrap_or(0);
            self.develop_open_version(other);
        }
        let lib = self.gallery.library_handle();
        self.develop.store.delete(&path, v, lib, &self.loader);
        self.develop.batch.versions.retain(|x| x.0 != v);
        self.develop_cards();
        self.gallery_set_versions(&path, self.develop.batch.versions.len() as u32);
        self.develop_say(t!("develop-version-deleted", name = name));
    }

    /// The auto-apply rules as a list: choosing one shows its rows (to change or delete it).
    fn develop_rules_menu(&mut self) {
        self.develop.batch.shelf.load();
        let rules = self.develop.batch.shelf.rules.clone();
        if rules.is_empty() {
            self.develop_say(t!("develop-rules-none"));
            return;
        }
        self.develop.batch.menu = (0..rules.len()).map(Do::EditRule).collect();
        let items = rules
            .iter()
            .enumerate()
            .map(|(i, r)| menu_item(i, rule_text(r), Icon::Sliders))
            .collect();
        self.chrome
            .open_list(t!("develop-rules-placeholder-edit"), items, None);
    }

    /// Rule `i`'s rows: maker, model, lens, ISO and file type it looks at, its style, and
    /// Delete (asks first).
    fn develop_rule_menu(&mut self, i: usize) {
        let Some(r) = self.develop.batch.shelf.rules.get(i).cloned() else {
            return;
        };
        let any = || t!("develop-rule-any");
        let or_any = |s: &str| {
            if s.trim().is_empty() {
                any()
            } else {
                s.to_owned()
            }
        };
        let iso = match iso_text(r.iso_min, r.iso_max) {
            t if t.is_empty() => any(),
            t => t,
        };
        let kind = match r.file_type {
            FileType::Any => t!("develop-rule-type-any"),
            FileType::Raw => t!("develop-rule-raw"),
            FileType::Other => t!("develop-rule-other"),
        };
        let rows = [
            (
                Do::RuleText(i, RuleField::Maker),
                t!("develop-rule-maker", value = or_any(&r.maker)),
                Icon::Sliders,
            ),
            (
                Do::RuleText(i, RuleField::Model),
                t!("develop-rule-model", value = or_any(&r.model)),
                Icon::Sliders,
            ),
            (
                Do::RuleText(i, RuleField::Lens),
                t!("develop-rule-lens", value = or_any(&r.lens)),
                Icon::Sliders,
            ),
            (
                Do::RuleText(i, RuleField::Iso),
                t!("develop-rule-iso", value = iso),
                Icon::Sliders,
            ),
            (
                Do::RuleType(i),
                t!("develop-rule-type", value = kind),
                Icon::Sliders,
            ),
            (
                Do::RuleStyles(i),
                t!("develop-rule-style", value = r.style.clone()),
                Icon::Sliders,
            ),
            (Do::DeleteRule(i), t!("develop-rule-delete"), Icon::Trash),
        ];
        self.develop.batch.menu = rows.iter().map(|(d, _, _)| d.clone()).collect();
        let items = rows
            .into_iter()
            .enumerate()
            .map(|(k, (_, label, icon))| menu_item(k, label, icon))
            .collect();
        self.chrome.open_list(
            t!("develop-rule-edit-placeholder", rule = rule_text(&r)),
            items,
            None,
        );
    }

    /// A rule's typed part from its sheet: set (an ISO range that is not one is said).
    pub(super) fn develop_rule_text(&mut self, i: usize, field: RuleField, typed: &str) {
        let shelf = &mut self.develop.batch.shelf;
        let Some(r) = shelf.rules.get_mut(i) else {
            return;
        };
        let typed = typed.trim().to_owned();
        match field {
            RuleField::Maker => r.maker = typed,
            RuleField::Model => r.model = typed,
            RuleField::Lens => r.lens = typed,
            RuleField::Iso => match parse_iso(&typed) {
                Ok((lo, hi)) => (r.iso_min, r.iso_max) = (lo, hi),
                Err(()) => {
                    self.develop_say(t!("develop-rule-iso-bad"));
                    return;
                }
            },
        }
        let said = rule_text(r);
        shelf.save_rules();
        self.develop_say(said);
        self.develop_rule_menu(i);
    }

    /// The next-capture choices as a list.
    fn develop_next_capture_menu(&mut self) {
        let shelf = &self.develop.batch.shelf;
        let mut menu = vec![
            Do::NextCapture(NextCapture::Off),
            Do::NextCapture(NextCapture::SameAsPrevious),
        ];
        let mut labels = vec![t!("develop-next-off-row"), t!("develop-next-previous-row")];
        for l in shelf.listed() {
            labels.push(l.name.clone());
            menu.push(Do::NextCapture(NextCapture::Style(l.name)));
        }
        let current = shelf.next.clone();
        let items = menu
            .iter()
            .zip(labels)
            .enumerate()
            .map(|(i, (d, label))| {
                let on = matches!(d, Do::NextCapture(n) if *n == current);
                menu_item(i, label, if on { Icon::Check } else { Icon::Minus })
            })
            .collect();
        self.develop.batch.menu = menu;
        self.chrome
            .open_list(t!("develop-next-placeholder"), items, None);
    }

    /// The styles' names as the next-capture menu lists them (for import's style row).
    pub(in crate::app) fn develop_style_names(&mut self) -> Vec<String> {
        self.develop.batch.shelf.load();
        self.develop
            .batch
            .shelf
            .listed()
            .into_iter()
            .map(|l| l.name)
            .collect()
    }

    /// What new captures get, as the next-capture menu names it.
    pub(in crate::app) fn develop_next_capture_label(&self) -> String {
        match &self.develop.batch.shelf.next {
            NextCapture::Off => t!("develop-next-off-row"),
            NextCapture::SameAsPrevious => t!("develop-next-previous-row"),
            NextCapture::Style(name) => name.clone(),
        }
    }

    /// New files arrived in cull mode's folder (or from a tethered camera): each gets the
    /// next-capture style.
    pub(in crate::app) fn develop_next_capture(&mut self, new: &[PathBuf]) {
        self.develop.batch.shelf.load();
        let next = self.develop.batch.shelf.next.clone();
        let new: Vec<PathBuf> = new.iter().filter(|p| developable(p)).cloned().collect();
        if next == NextCapture::Off || new.is_empty() {
            return;
        }
        let style = match &next {
            NextCapture::Style(name) => match self.develop.batch.shelf.style_named(name) {
                Some(s) => s,
                None => return,
            },
            _ => {
                // The previous photo's edit: the one open, else the last that arrived.
                let prev = self
                    .develop
                    .session
                    .as_ref()
                    .map(|s| s.edit.clone())
                    .or_else(|| self.develop.batch.last_capture.clone());
                match prev {
                    Some(e) => Style::from_edit_default("", &e, Registry::builtin()),
                    None => return,
                }
            }
        };
        let what = match &next {
            NextCapture::Style(n) => n.clone(),
            _ => t!("develop-history-previous"),
        };
        if let Some(last) = new.last() {
            self.develop.batch.last_capture = Some(style.applied(
                &controls::new_edit(last),
                Registry::builtin(),
                ApplyMode::Append,
            ));
        }
        let change: super::store::Change =
            Arc::new(move |p: &Path, e: &mut Edit, h: &mut History| {
                style.with_grain_seed(super::effects::grain_seed(p)).paste(
                    e,
                    h,
                    Registry::builtin(),
                    ApplyMode::Append,
                    &what,
                    now_ms(),
                )
            });
        let n = new.len() as i64;
        self.develop_apply(new, change, t!("develop-next-applied", count = n));
    }

    /// A photo opened for the first time (no saved edit): the first matching auto-apply
    /// rule's style, as one history step.
    pub(super) fn develop_auto_apply(&mut self) {
        let Some(path) = self.develop.session.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        self.develop.batch.shelf.load();
        if self.develop.batch.shelf.rules.is_empty() {
            return;
        }
        let facts = photo_facts(&path);
        let Some(name) = auto_style(&self.develop.batch.shelf.rules, &facts).map(str::to_owned)
        else {
            return;
        };
        let Some(style) = self.develop.batch.shelf.style_named(&name) else {
            return;
        };
        let ms = self.develop_ms_now();
        if let Some(s) = self.develop.session.as_mut()
            && style
                .with_grain_seed(super::effects::grain_seed(&s.path))
                .paste(
                    &mut s.edit,
                    &mut s.history,
                    Registry::builtin(),
                    ApplyMode::Append,
                    &name,
                    ms,
                )
        {
            s.sync_panel();
            self.develop.unsaved = Some(self.now());
            self.chrome
                .announce_status(t!("develop-rule-applied", name = name));
        }
    }
}

/// A rule in words: "Night: Canon, ISO 3200 and up, raw files".
fn rule_text(r: &AutoRule) -> String {
    let mut parts: Vec<String> = Vec::new();
    for s in [&r.maker, &r.model, &r.lens] {
        if !s.is_empty() {
            parts.push(s.clone());
        }
    }
    match (r.iso_min, r.iso_max) {
        (Some(a), Some(b)) => parts.push(t!(
            "develop-rule-iso-range",
            low = i64::from(a),
            high = i64::from(b)
        )),
        (Some(a), None) => parts.push(t!("develop-rule-iso-min", iso = i64::from(a))),
        (None, Some(b)) => parts.push(t!("develop-rule-iso-max", iso = i64::from(b))),
        (None, None) => {}
    }
    match r.file_type {
        FileType::Raw => parts.push(t!("develop-rule-raw")),
        FileType::Other => parts.push(t!("develop-rule-other")),
        FileType::Any => {}
    }
    if parts.is_empty() {
        parts.push(t!("develop-rule-every-photo"));
    }
    t!(
        "develop-rule-text",
        style = r.style.clone(),
        when = parts.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiff(
        entries0: &[(u16, u16, u32, [u8; 4])],
        exif: &[(u16, u16, u32, [u8; 4])],
        tail: &[u8],
    ) -> Vec<u8> {
        // Little-endian: header, IFD0 at 8, Exif IFD after it, then `tail` (strings).
        let ifd = |e: &[(u16, u16, u32, [u8; 4])]| {
            let mut b = (e.len() as u16).to_le_bytes().to_vec();
            for (t, ty, n, v) in e {
                b.extend(t.to_le_bytes());
                b.extend(ty.to_le_bytes());
                b.extend(n.to_le_bytes());
                b.extend(v);
            }
            b.extend(0u32.to_le_bytes());
            b
        };
        let mut out = b"II*\0".to_vec();
        out.extend(8u32.to_le_bytes());
        out.extend(ifd(entries0));
        out.extend(ifd(exif));
        out.extend(tail);
        out
    }

    #[test]
    fn exif_facts_read_make_model_lens_and_iso() {
        // IFD0: 3 entries (8 + 2 + 36 + 4 = 50) → Exif IFD at 50 with 2 entries (2 + 24 + 4
        // = 30) → strings at 80.
        let make_at = 80u32;
        let model_at = make_at + 6;
        let lens_at = model_at + 8;
        let t = tiff(
            &[
                (0x010F, 2, 6, make_at.to_le_bytes()),
                (0x0110, 2, 8, model_at.to_le_bytes()),
                (0x8769, 4, 1, 50u32.to_le_bytes()),
            ],
            &[
                (0x8827, 3, 1, [0x80, 0x0C, 0, 0]),
                (0xA434, 2, 8, lens_at.to_le_bytes()),
            ],
            b"Canon\0EOS R5\0\0RF 50mm\0",
        );
        let mut f = PhotoFacts::default();
        exif_facts(&t, &mut f);
        assert_eq!(f.maker, "Canon");
        assert_eq!(f.model, "EOS R5");
        assert_eq!(f.lens, "RF 50mm");
        assert_eq!(f.iso, Some(3200));
        // Damaged data reads nothing, without panicking.
        let mut g = PhotoFacts::default();
        exif_facts(&t[..30], &mut g);
        exif_facts(b"XX", &mut g);
        assert_eq!(g.iso, None);
    }

    #[test]
    fn cards_list_styles_snapshots_and_versions() {
        let st = State {
            versions: vec![(0, String::new()), (2, "Square".into())],
            version: 2,
            ..State::default()
        };
        let snaps = vec![Snapshot {
            name: "Warm".into(),
            modules: Vec::new(),
            masks: Vec::new(),
        }];
        let mut panel = DevelopPanel::new("t", Vec::new());
        refresh_cards(&mut panel, &st, &snaps);
        assert_eq!(panel.cards.len(), 3);
        let shipped = shipped_styles(Registry::builtin()).len();
        assert_eq!(panel.cards[0].controls.len(), shipped + 3);
        assert!(panel.cards[1].controls.iter().any(|c| c.id == SNAP_ROW));
        assert!(
            panel.cards[2]
                .controls
                .iter()
                .any(|c| c.id == VERSION_ROW + 2)
        );
        assert!(
            panel
                .cards
                .iter()
                .flat_map(|c| &c.controls)
                .all(|c| is_ours(c.id))
        );
        // Refreshing keeps whether a card is open.
        panel.cards[1].expanded = true;
        refresh_cards(&mut panel, &st, &[]);
        assert!(panel.cards[1].expanded);
        assert_eq!(panel.cards.len(), 3);
        assert!(!is_ours(controls::id::EXPOSURE));
    }

    #[test]
    fn iso_ranges_as_typed() {
        assert_eq!(parse_iso(""), Ok((None, None)));
        assert_eq!(parse_iso(" 100-800 "), Ok((Some(100), Some(800))));
        assert_eq!(parse_iso("1600-"), Ok((Some(1600), None)));
        assert_eq!(parse_iso("-400"), Ok((None, Some(400))));
        assert_eq!(parse_iso("800"), Ok((Some(800), Some(800))));
        assert_eq!(parse_iso("800\u{2013}100"), Ok((Some(100), Some(800))));
        assert_eq!(parse_iso("fast"), Err(()));
        for (lo, hi) in [
            (None, None),
            (Some(100), Some(800)),
            (Some(1600), None),
            (None, Some(400)),
            (Some(800), Some(800)),
        ] {
            assert_eq!(parse_iso(&iso_text(lo, hi)), Ok((lo, hi)));
        }
    }

    #[test]
    fn rules_read_as_words() {
        let r = AutoRule {
            style: "Night".into(),
            maker: "Canon".into(),
            iso_min: Some(3200),
            file_type: FileType::Raw,
            ..AutoRule::default()
        };
        let s = rule_text(&r);
        assert!(
            s.contains("Night") && s.contains("Canon") && s.contains("ISO"),
            "{s}"
        );
    }
}

//! The settings window (Ctrl+,), built from the settings schema, and the language picker.
//! Changes apply live through the store; the viewer picks them up by generation.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, draw_icon};
use specular_i18n::{I18n, LanguageInfo, LanguagePreference, fold_for_search, languages, t};
use specular_settings::{Kind, Origin, Section, SettingMeta, Value};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Response, ViewerState};

/// Settings → Advanced → "Reset all settings…": the only way to it, and it asks first.
pub(crate) const RESET_ALL: &str = "advanced.reset_all";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SettingsTarget {
    Section(usize),
    Search,
    Toggle(&'static str),
    Step(&'static str, i8),
    Dropdown(&'static str),
    Choose(&'static str, String),
    /// Turns one option of a list setting on or off.
    ListItem(&'static str, &'static str),
    Reset(&'static str),
    Act(&'static str),
    /// The field of a text or path setting (setting_field.rs).
    Field(&'static str),
    /// A path setting's Choose… (the platform's file or folder dialog) and Clear.
    PathChoose(&'static str),
    PathClear(&'static str),
    /// One of a colour setting's swatches (setting_field.rs `SWATCHES`).
    Swatch(&'static str, u8),
    ResetSection,
    Dropdown_,
    // The panel the application puts under a setting (settings_panel.rs), by setting key.
    PanelHeading(&'static str),
    PanelRow(&'static str, usize),
    PanelFooter(&'static str),
    PanelButton(&'static str, usize),
    /// The tick box of a panel row (with its label).
    PanelCheck(&'static str, usize),
    /// The disclosure arrow that opens or closes a panel row's children.
    PanelExpand(&'static str, usize),
    PanelAction(&'static str),
    PanelConfirm(&'static str),
    PanelCancel(&'static str),
    // The toolbar list (toolbar_edit.rs), by row: the buttons on the toolbar, then the hidden
    // ones.
    ToolRow(usize),
    /// The switch that shows or hides a row's button.
    ToolShow(usize),
    /// Takes a spacer or separator off the toolbar.
    ToolRemove(usize),
    ToolAdd(specular_settings::ToolbarItem),
    ToolReset,
    /// Moves a row's button one place towards the start of the toolbar, or the end.
    ToolUp(usize),
    ToolDown(usize),
    /// Opens a row's menu (⋯).
    ToolMenu(usize),
    /// An entry of a row's open menu.
    ToolMenuItem(usize, crate::toolbar_edit::RowOp),
    /// Around an open row menu: a click there closes it.
    ToolMenuBg,
    // The key binding editor (keys_edit.rs): its modes, presets and buttons.
    KeyMode(specular_input::Mode),
    KeyPreset(specular_input::Preset),
    KeyImport,
    KeyExport,
    /// Exports the mode shown.
    KeyExportMode,
    /// Opens keymap.toml in the config editor.
    KeyEditFile,
    KeyResetMode,
    KeyResetAll,
    /// The buttons and the text of a confirmation (preset, reset).
    KeyConfirm,
    KeyCancel,
    KeyConfirmText,
    /// A problem found in keymap.toml.
    KeyProblem(u8),
    /// "No action or key matches".
    KeyNoMatch,
    // Its list, by mode and action: the row, a key (and its remove button), Add, Reset.
    KeyRow(specular_input::Mode, specular_input::Action),
    KeyChip(specular_input::Mode, specular_input::Action, u8),
    KeyRemove(specular_input::Mode, specular_input::Action, u8),
    KeyAdd(specular_input::Mode, specular_input::Action),
    KeyResetAction(specular_input::Mode, specular_input::Action),
    /// An answer to the open conflict, and its text.
    KeyChoice(u8),
    KeyPromptText(u8),
    /// Stops recording.
    KeyRecordCancel,
}

impl SettingsTarget {
    /// Part of a panel under a setting (settings_panel.rs).
    pub(crate) fn is_panel(&self) -> bool {
        matches!(
            self,
            Self::PanelHeading(_)
                | Self::PanelRow(..)
                | Self::PanelFooter(_)
                | Self::PanelButton(..)
                | Self::PanelCheck(..)
                | Self::PanelExpand(..)
                | Self::PanelAction(_)
                | Self::PanelConfirm(_)
                | Self::PanelCancel(_)
        )
    }

    /// Part of the key binding editor (keys_edit.rs).
    pub(crate) fn is_keys(&self) -> bool {
        crate::keys_edit::is_keys(self)
    }
}

/// What an action setting that switches something on or off shows instead of its plain
/// button: a state line under the help text and the button's text (`None`: no button).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingStatus {
    pub state: String,
    /// Shown in the accent colour (something needs attention).
    pub attention: bool,
    pub button: Option<String>,
}

#[derive(Default)]
pub(crate) struct SettingsState {
    pub section: usize,
    pub search: String,
    /// The search field's caret, selection and undo history (`search` is the truth).
    pub search_edit: crate::line_edit::LineEdit,
    pub scroll: f32,
    pub dropdown: Option<&'static str>,
    pub dropdown_scroll: f32,
    /// How far the sidebar's section list is scrolled.
    pub nav_scroll: f32,
    /// The sidebar as last drawn (the wheel over it scrolls the section list).
    pub side: Rect,
    /// The section last brought into view in the sidebar.
    nav_shown: Option<usize>,
    /// Where the section's controls scroll, as last drawn (screen readers).
    pub body: Rect,
    sections: usize,
    /// The panel (by setting key) whose action is waiting for its confirmation.
    pub panel_confirm: Option<&'static str>,
    /// Scroll this setting to the top on the next draw ([`Chrome::open_settings_at`]).
    pub scroll_to: Option<&'static str>,
    /// The row Settings was opened at ([`Chrome::open_settings_at`], as laid out): the first
    /// Tab from the search field starts there instead of at the top of the page. Gone once
    /// Tab has been pressed or the page changes (section, search).
    pub start: Option<usize>,
    /// The toolbar list.
    pub tools: crate::toolbar_edit::ListUi,
    /// The key binding editor.
    pub keys: crate::keys_edit::KeysUi,
    /// The controls the keyboard reaches with Tab (focus.rs), in order, as last laid out.
    pub stops: Vec<KeyStop>,
    /// Where each row of the page was last laid out in the content (y, height; its panel
    /// included).
    pub rows: Vec<(f32, f32)>,
    /// Where the focused row of the toolbar list was last drawn in the content.
    pub tool_at: Option<(f32, f32)>,
    /// The part of the content last in view (top, height).
    pub view: (f32, f32),
    /// Frames left to bring the control with the keyboard focus into view.
    pub reveal: u8,
    /// Bring the option the keyboard is on in the open dropdown into view.
    pub dropdown_reveal: bool,
    /// The places Esc goes back to (history.rs).
    pub history: crate::history::History,
    /// The text or path field being edited (setting_field.rs).
    pub field: Option<crate::setting_field::Draft>,
    /// The fields whose text was refused and left, each with its own reason, until fixed,
    /// put back (Esc) or Settings closes.
    pub refused: Vec<crate::setting_field::Draft>,
    /// The fields as last drawn, by setting: where a press puts the caret.
    pub field_geoms: Vec<(&'static str, crate::ctx::FieldGeom)>,
}

/// A control of Settings the keyboard reaches with Tab.
#[derive(Clone, Debug)]
pub(crate) struct KeyStop {
    pub t: Target,
    /// The row it is in (`None`: Reset section, in the header).
    pub row: Option<usize>,
    /// Where it was last drawn, in the content (y, height); `None` when out of view.
    pub at: Option<(f32, f32)>,
}

/// Whether a status replaces the setting's control with its button (an action, or a
/// status with a button); otherwise it is only a state line above the usual control (the
/// Local AI model size's recommendation).
fn owns_control(m: &SettingMeta, s: &SettingStatus) -> bool {
    matches!(m.kind, Kind::Action) || s.button.is_some()
}

/// The controls of a setting's row the keyboard reaches, in reading order: its reset
/// button, its control (each chip of a list; the toolbar list as one stop, then the editor's
/// buttons), then the buttons of the panel under it (`panel`: shown).
fn row_stops(
    ch: &Chrome,
    m: &'static SettingMeta,
    status: Option<&SettingStatus>,
    st: &ViewerState,
    panel: bool,
) -> Vec<SettingsTarget> {
    use SettingsTarget as S;
    let mut v = Vec::new();
    let reset = st.store.is_modified(m.key) && !matches!(m.kind, Kind::Action);
    // A field's reset comes after it and its buttons, where it is drawn.
    let field = crate::setting_field::is_field(m) && !status.is_some_and(|s| owns_control(m, s));
    if reset && !field {
        v.push(S::Reset(m.key));
    }
    if m.key == crate::toolbar_edit::KEY {
        v.extend([
            S::ToolRow(0),
            S::ToolAdd(specular_settings::ToolbarItem::Spacer),
            S::ToolAdd(specular_settings::ToolbarItem::Separator),
            S::ToolReset,
        ]);
    } else if m.key == crate::keys_edit::KEY {
        v.extend(crate::keys_edit::stops(ch, st));
    } else if let Some(s) = status.filter(|s| owns_control(m, s)) {
        if s.button.is_some() {
            v.push(S::Act(m.key));
        }
    } else if ch.setting_disabled(m.key) {
        // A disabled button is not a stop (it stays in the accessibility tree).
    } else {
        match &m.kind {
            Kind::Bool => v.push(S::Toggle(m.key)),
            Kind::Enum { .. } | Kind::Language | Kind::Locale | Kind::Monitor | Kind::Pick => {
                v.push(S::Dropdown(m.key))
            }
            Kind::Int { .. } | Kind::Float { .. } => {
                v.extend([S::Step(m.key, -1), S::Step(m.key, 1)]);
            }
            Kind::Action => v.push(S::Act(m.key)),
            Kind::List { .. } => v.extend(m.options().iter().map(|o| S::ListItem(m.key, o))),
            Kind::Shortcut | Kind::Text => v.push(S::Field(m.key)),
            Kind::Path { .. } => {
                v.extend([S::Field(m.key), S::PathChoose(m.key)]);
                if matches!(st.store.get(m.key), Ok(Value::Path(Some(_)))) {
                    v.push(S::PathClear(m.key));
                }
            }
            Kind::Color => {
                v.push(S::Field(m.key));
                v.extend(
                    (0..crate::setting_field::SWATCHES.len()).map(|i| S::Swatch(m.key, i as u8)),
                );
            }
        }
    }
    if reset && field {
        v.push(S::Reset(m.key));
    }
    if panel && let Some(p) = ch.setting_panels.get(m.key) {
        for (i, r) in p.rows.iter().enumerate() {
            if r.expand.is_some() {
                v.push(S::PanelExpand(m.key, i));
            }
            if r.check.is_some() {
                v.push(S::PanelCheck(m.key, i));
            }
            if r.button.is_some() {
                v.push(S::PanelButton(m.key, i));
            }
        }
        if p.action.is_some() {
            if ch.settings.panel_confirm == Some(m.key) {
                v.extend([S::PanelCancel(m.key), S::PanelConfirm(m.key)]);
            } else {
                v.push(S::PanelAction(m.key));
            }
        }
    }
    v
}

/// The option values of dropdown setting `k`, in order.
pub(crate) fn option_values(k: &str, st: &ViewerState) -> Vec<String> {
    let Some(m) = specular_settings::find(k) else {
        return Vec::new();
    };
    let current = st.store.get(k).map(|v| v.to_string()).unwrap_or_default();
    options(m, I18n::global(), st, &current)
        .into_iter()
        .map(|o| o.0)
        .collect()
}

/// The focus ring of control `t` of Settings, drawn where it was hit (`r`).
fn ring(cx: &mut Ctx, t: &SettingsTarget, r: Rect) {
    use SettingsTarget as S;
    match t {
        // The switch's hit area is 4 px larger than the track.
        S::Toggle(_) => cx.focus_ring(r.inset(1.0), r.h * 0.5 - 1.0),
        // Inside the stepper's border.
        S::Step(..) => cx.focus_ring(r.inset(2.0), metrics::RADIUS_SM),
        S::ListItem(..) => cx.focus_ring(r.inset(-3.0), r.h * 0.5 + 3.0),
        _ => cx.focus_ring(r.inset(-3.0), metrics::RADIUS_SM + 4.0),
    }
}

impl SettingsState {
    pub fn reset(&mut self) {
        self.search.clear();
        self.scroll = 0.0;
        self.dropdown = None;
        self.panel_confirm = None;
        self.scroll_to = None;
        self.start = None;
        self.tools = Default::default();
        self.keys = Default::default();
        self.stops.clear();
        self.rows.clear();
        self.reveal = 0;
        self.history.clear();
        self.field = None;
        self.field_geoms.clear();
    }
    pub fn type_text(&mut self, s: &str) {
        self.edit().insert(s);
        self.edited();
    }
    pub fn backspace(&mut self) {
        let mut clip = crate::line_edit::MemoryClipboard::default();
        let k = specular_input::Key::Named(specular_input::NamedKey::Backspace);
        self.edit()
            .key(k, specular_input::Mods::NONE, false, &mut clip);
        self.edited();
    }
    /// The search field, caught up with `search`.
    pub fn edit(&mut self) -> &mut crate::line_edit::LineEdit {
        self.search_edit.sync(&self.search);
        &mut self.search_edit
    }
    /// The search field edited: the search follows it (from the top of the list).
    pub fn edited(&mut self) {
        if self.search != self.search_edit.text() {
            self.search = self.search_edit.text().to_owned();
            self.scroll = 0.0;
            self.start = None;
        }
    }
    pub fn key(&mut self, k: specular_input::NamedKey) {
        use specular_input::NamedKey as N;
        match k {
            N::ArrowDown if self.search.is_empty() => {
                self.section = (self.section + 1).min(self.sections.saturating_sub(1));
                self.scroll = 0.0;
                self.start = None;
            }
            N::ArrowUp if self.search.is_empty() => {
                self.section = self.section.saturating_sub(1);
                self.scroll = 0.0;
                self.start = None;
            }
            N::PageDown => self.scroll += 300.0,
            N::PageUp => self.scroll -= 300.0,
            N::Home => self.scroll = 0.0,
            _ => {}
        }
    }
}

/// A list item that cannot be turned off: the General profile is always on.
pub(crate) fn locked_item(key: &str, option: &str) -> bool {
    key == "profile.capabilities" && option == "general"
}

/// Sections that have something to show.
pub(crate) fn sections() -> Vec<Section> {
    Section::ALL
        .iter()
        .copied()
        .filter(|s| specular_settings::in_section(*s).any(|m| m.applies_here()))
        .collect()
}

/// Position of the section with TOML key `key` in the sidebar (every section listed).
#[cfg(test)]
pub(crate) fn section_index(key: &str) -> Option<usize> {
    sections().iter().position(|s| s.key() == key)
}

impl Chrome {
    /// The sections the sidebar lists: those with a setting the window shows (a profile
    /// that is off hides its section, `set_setting_hidden`).
    pub(crate) fn sections(&self) -> Vec<Section> {
        let mut v = sections();
        v.retain(|s| {
            specular_settings::in_section(*s)
                .any(|m| m.applies_here() && !self.setting_hidden(m.key))
        });
        v
    }

    /// Position of the section with TOML key `key` in the sidebar as listed now.
    pub(crate) fn section_index(&self, key: &str) -> Option<usize> {
        self.sections().iter().position(|s| s.key() == key)
    }
}

/// Text the chrome shows in many scripts at once, for the text system to shape ahead off
/// the UI thread (`Text::set_warm_up`): the languages' own names, as the language dropdowns
/// and the language picker list them. Without it the first open of such a list finds and
/// loads a fallback font per script on the UI thread (over 100 ms for a CJK font).
pub fn warm_up_texts() -> Vec<String> {
    languages().iter().map(|l| l.native_name.clone()).collect()
}

/// Whether the language lists offer `l`: the pseudo-locales only in developer mode, or
/// while one of them is the current language (`current`) so the list still shows it.
fn offer_language(l: &LanguageInfo, st: &ViewerState, current: &str) -> bool {
    !l.pseudo || st.settings.advanced.developer_mode || l.id == current
}

/// Options of a dropdown setting: (value text, label, secondary label).
fn options(
    m: &SettingMeta,
    i18n: &I18n,
    st: &ViewerState,
    current: &str,
) -> Vec<(String, String, Option<String>)> {
    match &m.kind {
        Kind::Enum { .. } if let Some(c) = choices(m.key) => {
            c.into_iter().map(|(v, label)| (v, label, None)).collect()
        }
        Kind::Enum { options } => options
            .iter()
            .map(|o| {
                (
                    (*o).to_owned(),
                    m.display_value(&Value::Text((*o).to_owned()), i18n),
                    None,
                )
            })
            .collect(),
        Kind::Language | Kind::Locale => {
            let first = if matches!(m.kind, Kind::Language) {
                "system"
            } else {
                "language"
            };
            let mut v = vec![(
                first.to_owned(),
                m.display_value(&Value::Text(first.to_owned()), i18n),
                None,
            )];
            for l in languages() {
                if (l.pseudo && matches!(m.kind, Kind::Locale)) || !offer_language(l, st, current) {
                    continue;
                }
                v.push((
                    l.id.to_string(),
                    l.native_name.clone(),
                    Some(l.english_name.clone()),
                ));
            }
            v
        }
        Kind::Monitor => monitor_options(m, i18n, current),
        // The default, then the names the application offers (with the chosen one kept
        // when the config no longer has it).
        Kind::Pick => {
            let mut v = vec![(
                String::new(),
                m.display_value(&Value::Text(String::new()), i18n),
                None,
            )];
            let offered = choices(m.key).unwrap_or_default();
            for (o, label) in &offered {
                v.push((o.clone(), label.clone(), None));
            }
            if !current.is_empty() && !offered.iter().any(|(o, _)| o == current) {
                v.push((
                    current.to_owned(),
                    specular_i18n::isolate_ltr(current),
                    None,
                ));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// Enum settings whose choices depend on the machine, as the application last set them
/// ([`Chrome::set_setting_choices`]): the values offered, each with its label (the Local AI
/// model sizes: "Recommended (Standard)", only the sizes that exist and fit). Per thread:
/// the chrome lives, draws and builds its accessibility tree on the UI thread.
type Choices = Vec<(&'static str, Vec<(String, String)>)>;
thread_local! {
    static CHOICES: std::cell::RefCell<Choices> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Sets (or with `None` forgets) the choices of enum setting `key`. Returns whether they
/// changed.
pub(crate) fn set_choices(key: &'static str, c: Option<Vec<(String, String)>>) -> bool {
    CHOICES.with_borrow_mut(|all| {
        let i = all.iter().position(|(k, _)| *k == key);
        match (i, c) {
            (Some(i), Some(c)) if all[i].1 == c => false,
            (Some(i), Some(c)) => {
                all[i].1 = c;
                true
            }
            (Some(i), None) => {
                all.remove(i);
                true
            }
            (None, Some(c)) => {
                all.push((key, c));
                true
            }
            (None, None) => false,
        }
    })
}

pub(crate) fn choices(key: &str) -> Option<Vec<(String, String)>> {
    CHOICES.with_borrow(|all| all.iter().find(|(k, _)| *k == key).map(|(_, c)| c.clone()))
}

/// What a setting's value is called: its choice's label when the application set the
/// choices, else the schema's words.
pub(crate) fn value_label(m: &SettingMeta, v: &Value, i18n: &I18n) -> String {
    if let (Kind::Enum { .. }, Value::Text(t)) = (&m.kind, v)
        && let Some(c) = choices(m.key)
        && let Some((_, label)) = c.iter().find(|(o, _)| o == t)
    {
        return label.clone();
    }
    m.display_value(v, i18n)
}

/// The displays connected now, by name, as the application last reported them
/// ([`Chrome::set_monitors`]).
static MONITORS: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

/// Sets the displays a display setting offers (process-wide: there is one set of displays).
pub(crate) fn set_monitors(names: Vec<String>) {
    if let Ok(mut m) = MONITORS.write() {
        *m = names;
    }
}

pub(crate) fn monitors() -> Vec<String> {
    MONITORS.read().map(|m| m.clone()).unwrap_or_default()
}

/// A display setting's options: Automatic, the displays connected now, and the chosen one
/// when it is not connected (marked so; it is used again when it comes back).
pub(crate) fn monitor_options(
    m: &SettingMeta,
    i18n: &I18n,
    current: &str,
) -> Vec<(String, String, Option<String>)> {
    let mut v = vec![(
        "auto".to_owned(),
        m.display_value(&Value::Text("auto".into()), i18n),
        None,
    )];
    let names = monitors();
    for n in &names {
        v.push((
            n.clone(),
            m.display_value(&Value::Text(n.clone()), i18n),
            None,
        ));
    }
    if current != "auto" && !current.is_empty() && !names.iter().any(|n| n == current) {
        v.push((
            current.to_owned(),
            m.display_value(&Value::Text(current.to_owned()), i18n),
            Some(i18n.tr("settings-monitor-not-connected")),
        ));
    }
    v
}

/// `list` with `opt` removed if it is in it, else added. An added option goes after the
/// nearest option before it in `options` order that is on, so a custom order is kept.
pub(crate) fn toggle_item(options: &[&str], list: &[String], opt: &str) -> Vec<String> {
    if list.iter().any(|x| x == opt) {
        return list.iter().filter(|x| *x != opt).cloned().collect();
    }
    let before = options.iter().position(|o| *o == opt).unwrap_or(0);
    let at = options[..before]
        .iter()
        .rev()
        .find_map(|o| list.iter().position(|x| x == o))
        .map_or(0, |i| i + 1);
    let mut out = list.to_vec();
    out.insert(at, opt.to_owned());
    out
}

/// One option chip of a list setting: rectangle, label, option.
type Chip = (Rect, Arc<TextBlock>, &'static str);

/// The options of a list setting laid out as chips that wrap within `width`: (rectangle
/// relative to the top-left corner, label, option). Returns them and the total height.
fn chips(cx: &mut Ctx, m: &SettingMeta, i18n: &I18n, width: f32) -> (Vec<Chip>, f32) {
    let style = cx.style(metrics::FONT_SMALL);
    let (h, gap, check) = (cx.u(26.0), 6.0, 16.0);
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut out = Vec::new();
    for o in m.options() {
        let b = cx.layout(&i18n.tr(&m.option_id(o)), style, None);
        let w = b.width + 20.0 + check;
        if x > 0.0 && x + w > width {
            x = 0.0;
            y += h + gap;
        }
        out.push((Rect::new(x, y, w, h), b, *o));
        x += w + gap;
    }
    let total = if out.is_empty() { 0.0 } else { y + h };
    (out, total)
}

impl Chrome {
    /// Closes the open dropdown; the keyboard, if it was on an option, goes back to its
    /// button.
    pub(crate) fn close_dropdown(&mut self) {
        if let Some(k) = self.settings.dropdown.take()
            && matches!(
                &self.modal_focus,
                Some(Target::Settings(SettingsTarget::Choose(..)))
            )
        {
            self.modal_focus = Some(Target::Settings(SettingsTarget::Dropdown(k)));
        }
    }

    /// Whether the search field has the keyboard (caret and accent border): not while a
    /// dropdown is open or a control or the toolbar list has the focus. Typing still goes to
    /// it, and takes the focus back.
    pub(crate) fn settings_search_focused(&self) -> bool {
        self.settings.dropdown.is_none()
            && self.modal_focus.is_none()
            && self.settings.tools.focus.is_none()
    }

    /// Where the control with the keyboard focus was last laid out in the content (y,
    /// height), and whether that is the control itself rather than its row (out of view).
    pub(crate) fn settings_focus_span(&self) -> Option<(f32, f32, bool)> {
        let ss = &self.settings;
        if let Some(Target::Settings(s)) = &self.modal_focus
            && s.is_keys()
        {
            if let Some((y, h)) = ss.keys.span(s) {
                return Some((y, h, true));
            }
            // Not drawn yet (a conflict just asked): the editor's row, then again once drawn.
            let row = ss.stops.iter().find_map(|st| match &st.t {
                Target::Settings(k) if k.is_keys() => st.row,
                _ => None,
            })?;
            let (y, h) = *ss.rows.get(row)?;
            return Some((y, h, false));
        }
        let t = if ss.tools.focus.is_some() {
            if let Some((y, h)) = ss.tool_at {
                return Some((y, h, true));
            }
            Target::Settings(SettingsTarget::ToolRow(0))
        } else {
            self.modal_focus.clone()?
        };
        let s = ss.stops.iter().find(|s| s.t == t)?;
        if let Some((y, h)) = s.at {
            return Some((y, h, true));
        }
        let (y, h) = *ss.rows.get(s.row?)?;
        Some((y, h, false))
    }

    pub(crate) fn settings_click(&mut self, s: SettingsTarget, st: &ViewerState) -> Response {
        use SettingsTarget as S;
        let store = st.store;
        let o = Origin::SettingsWindow;
        if s.is_panel() {
            return match self.panel_click(&s) {
                Some(c) => Response::cmd(c),
                None => Response::used(),
            };
        }
        if s.is_keys() {
            return self.keys_click(s, st);
        }
        // Anything else pressed stops a recording.
        if self.settings.keys.recording.is_some() && s != S::Search {
            self.keys_cancel_recording();
        }
        match s {
            S::Section(i) => {
                self.settings.section = i;
                self.settings.search.clear();
                self.settings.scroll = 0.0;
                self.settings.start = None;
            }
            S::Search | S::Dropdown_ => {
                self.settings.tools.focus = None;
                self.settings.tools.close_menu();
            }
            S::ToolMenuBg => {
                self.settings.tools.close_menu();
            }
            S::ToolRow(_)
            | S::ToolShow(_)
            | S::ToolRemove(_)
            | S::ToolAdd(_)
            | S::ToolReset
            | S::ToolUp(_)
            | S::ToolDown(_)
            | S::ToolMenu(_)
            | S::ToolMenuItem(..) => self.tools_click(s, st),
            S::PanelHeading(_)
            | S::PanelRow(..)
            | S::PanelFooter(_)
            | S::PanelButton(..)
            | S::PanelCheck(..)
            | S::PanelExpand(..)
            | S::PanelAction(_)
            | S::PanelConfirm(_)
            | S::PanelCancel(_) => {}
            // keys_click, above.
            S::KeyMode(_)
            | S::KeyPreset(_)
            | S::KeyImport
            | S::KeyExport
            | S::KeyExportMode
            | S::KeyEditFile
            | S::KeyResetMode
            | S::KeyResetAll
            | S::KeyConfirm
            | S::KeyCancel
            | S::KeyConfirmText
            | S::KeyProblem(_)
            | S::KeyNoMatch
            | S::KeyRow(..)
            | S::KeyChip(..)
            | S::KeyRemove(..)
            | S::KeyAdd(..)
            | S::KeyResetAction(..)
            | S::KeyChoice(_)
            | S::KeyPromptText(_)
            | S::KeyRecordCancel => {}
            S::Toggle(k) => {
                if let Ok(Value::Bool(b)) = store.get(k) {
                    let _ = store.set(k, Value::Bool(!b), o);
                }
            }
            S::Step(k, d) => {
                if let (Some(m), Ok(v)) = (specular_settings::find(k), store.get(k)) {
                    let nv = match (&m.kind, v) {
                        (
                            Kind::Int {
                                min,
                                max,
                                step,
                                auto,
                                ..
                            },
                            Value::Int(i),
                        ) => {
                            let n = if *auto && i == 0 {
                                if d > 0 { *min } else { 0 }
                            } else if *auto && i <= *min && d < 0 {
                                0
                            } else {
                                // Steps grow for large ranges (cache sizes).
                                let mut st = *step;
                                while i.abs() >= st * 16 && st < max / 8 {
                                    st *= 2;
                                }
                                (i + st * d as i64).clamp(*min, *max)
                            };
                            Some(Value::Int(n))
                        }
                        (Kind::Float { min, max, step, .. }, Value::Float(x)) => {
                            let n = ((x + step * d as f64) / step).round() * step;
                            Some(Value::Float(n.clamp(*min, *max)))
                        }
                        _ => None,
                    };
                    if let Some(nv) = nv {
                        let _ = store.set(k, nv, o);
                    }
                }
            }
            S::Dropdown(k) => {
                if self.settings.dropdown == Some(k) {
                    self.close_dropdown();
                } else {
                    self.settings.dropdown = Some(k);
                }
                self.settings.dropdown_scroll = 0.0;
            }
            S::Choose(k, v) => {
                let _ = store.set_text(k, &v, o);
                self.close_dropdown();
            }
            S::ListItem(k, opt) if locked_item(k, opt) => {}
            S::ListItem(k, opt) => {
                if let (Some(m), Ok(Value::List(cur))) = (specular_settings::find(k), store.get(k))
                {
                    let _ = store.set(k, Value::List(toggle_item(m.options(), &cur, opt)), o);
                }
            }
            S::Reset(k) => {
                self.setting_field_forget(k);
                let _ = store.reset(k, o);
            }
            S::Field(_) | S::PathChoose(_) | S::PathClear(_) | S::Swatch(..) => {
                return self.setting_field_click(&s, st);
            }
            S::ResetSection => {
                if let Some(sec) = self.sections().get(self.settings.section) {
                    self.settings.field = None;
                    let sec = *sec;
                    self.settings.refused.retain(|d| {
                        specular_settings::find(d.key).is_none_or(|m| m.section != sec)
                    });
                    store.reset_section(sec, o);
                }
            }
            S::Act(k) => {
                if self.setting_disabled(k) {
                    return Response::used();
                }
                if k == RESET_ALL {
                    // Asks first (lib.rs, `confirm_reset_all`).
                    self.confirm_reset_all();
                    return Response::used();
                }
                if k == "advanced.edit_settings_file" {
                    self.open_config_editor(store);
                    return Response::used();
                }
                return Response::cmd(crate::Command::SettingAction(k));
            }
        }
        Response::used()
    }
}

/// Where the settings sheet goes: larger text gets a larger sheet, as far as the window
/// allows.
pub(crate) fn sheet_rect(cx: &Ctx) -> Rect {
    let w = (cx.w - 40.0).min(cx.u(900.0));
    // Tall enough for every section in the sidebar where the window allows; a shorter
    // window scrolls the sidebar.
    let nav = 46.0 + cx.u(28.0 + 32.0) + sections().len() as f32 * cx.u(32.0);
    let h = (cx.h - 40.0).min(cx.u(660.0).max(nav));
    Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h)
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState) {
    let i18n = I18n::global();
    // The toolbar list records its rows as it is drawn; off screen it has none (Tab then
    // stays in the search field).
    ch.settings.tools.clear_rows();
    ch.settings.keys.on_page = false;
    ch.setting_field_sync(st);
    ch.settings.field_geoms.clear();
    let mut stops: Vec<KeyStop> = Vec::new();
    let mut rows: Vec<(f32, f32)> = Vec::new();
    let secs = ch.sections();
    ch.settings.sections = secs.len();
    ch.settings.section = ch.settings.section.min(secs.len().saturating_sub(1));
    // Larger text gets a larger sheet (as far as the window allows) and taller rows.
    let r = sheet_rect(cx);
    let w = r.w;
    cx.sheet(r);

    // Sidebar.
    let side_w = if w > 640.0 { 210.0 } else { 160.0 };
    let side_w = cx.u(side_w).min((w * 0.3).round());
    let side = cx.m_in(Rect::new(r.x, r.y, side_w, r.h), r);
    let radii = if cx.rtl {
        [0.0, metrics::RADIUS_LG, metrics::RADIUS_LG, 0.0]
    } else {
        [metrics::RADIUS_LG, 0.0, 0.0, metrics::RADIUS_LG]
    };
    cx.dl.rect_full(
        side,
        radii,
        cx.pal.sidebar,
        specular_theme::Color::TRANSPARENT,
        0.0,
    );
    let ts = cx.style(metrics::FONT_TITLE).weight(650.0);
    cx.label_fit(
        &t!("settings-title"),
        ts,
        Rect::new(side.x + 18.0, side.y + 16.0, side.w - 36.0, cx.u(28.0)),
        HAlign::Start,
        cx.pal.text,
        false,
    );
    let search_r = Rect::new(
        side.x + 12.0,
        side.y + 26.0 + cx.u(28.0),
        side.w - 24.0,
        cx.u(32.0),
    );
    let focused = ch.settings_search_focused();
    // While a control has the keyboard the field is drawn unfocused, but typing (and the
    // input method's compositions) still go to it: the input method stays on, except while
    // a dropdown is open (it takes no text).
    cx.field(
        search_r,
        &ch.settings.search,
        Some(&ch.settings.search_edit),
        &t!("settings-search-placeholder"),
        focused,
        ch.settings.dropdown.is_none(),
    );
    cx.hit(search_r, Target::Settings(SettingsTarget::Search));
    let searching = !ch.settings.search.trim().is_empty();
    let ss = cx.style(metrics::FONT_BODY);
    // Rows as tall as the text allows; when they don't all fit (large text in a small
    // window) the list scrolls.
    let list_top = search_r.bottom() + 12.0;
    let list = Rect::new(side.x, list_top, side.w, side.bottom() - 8.0 - list_top);
    ch.settings.side = side;
    let line = cx.layout("Ag", ss, None).height;
    let pitch = cx.u(32.0).max(line + 2.0).floor();
    let nav_max = (secs.len() as f32 * pitch - list.h).max(0.0);
    // A section chosen from the keyboard (or opened at) scrolls into view.
    let shown = (!searching).then_some(ch.settings.section);
    if shown.is_some() && shown != ch.settings.nav_shown {
        let top = ch.settings.section as f32 * pitch;
        let s = &mut ch.settings.nav_scroll;
        if s.is_nan() {
            *s = 0.0;
        }
        if top < *s {
            *s = top;
        } else if top + pitch > *s + list.h {
            *s = top + pitch - list.h;
        }
    }
    ch.settings.nav_shown = shown;
    ch.settings.nav_scroll = ch.settings.nav_scroll.clamp(0.0, nav_max);
    let nav_scroll = ch.settings.nav_scroll;
    cx.dl.push_clip(list);
    for (i, sec) in secs.iter().enumerate() {
        let rr = Rect::new(
            side.x + 8.0,
            list_top + i as f32 * pitch - nav_scroll,
            side.w - 16.0,
            pitch - 2.0,
        );
        if rr.bottom() < list.y || rr.y > list.bottom() {
            continue;
        }
        let tgt = Target::Settings(SettingsTarget::Section(i));
        let sel = !searching && i == ch.settings.section;
        if sel {
            cx.dl.rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.accent_soft);
        } else if cx.is_hover(&tgt) {
            cx.dl
                .rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.surface_hover);
        }
        let style = if sel { ss.weight(600.0) } else { ss };
        // Long names (large text, narrow sheets) end in "…" rather than run into the content.
        cx.label_fit(
            &sec.label(i18n),
            style,
            rr.inset_xy(10.0, 0.0),
            HAlign::Start,
            cx.pal.text,
            false,
        );
        cx.hit(rr.intersect(&list), tgt);
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::SettingsNav,
        list,
        side,
        nav_max,
    );

    // Content.
    let content = cx.m_in(Rect::new(r.x + side_w, r.y, r.w - side_w, r.h), r);
    let head_h = cx.u(34.0);
    let close = cx.m_in(
        Rect::new(
            content.right() - 44.0,
            content.y + 12.0 + (head_h - 32.0) * 0.5,
            32.0,
            32.0,
        ),
        content,
    );
    cx.close_button(close);
    let header = Rect::new(
        content.x + 24.0,
        content.y + 12.0,
        content.w - 48.0 - 40.0,
        head_h,
    );
    let header = cx.m_in(header, content);
    let metas: Vec<&'static SettingMeta> = if searching {
        let mut found = specular_settings::search(&ch.settings.search, i18n);
        // A panel's text finds the setting it sits under.
        for (k, p) in &ch.setting_panels {
            if p.matches(&ch.settings.search)
                && !found.iter().any(|m| m.key == *k)
                && let Some(m) = specular_settings::find(k)
            {
                found.push(m);
            }
        }
        // So do the key editor's actions and keys.
        if crate::keys_edit::matches_any(st.keymap, &ch.settings.search)
            && !found.iter().any(|m| m.key == crate::keys_edit::KEY)
            && let Some(m) = specular_settings::find(crate::keys_edit::KEY)
        {
            found.push(m);
        }
        found.retain(|m| !ch.setting_hidden(m.key));
        found
    } else {
        secs.get(ch.settings.section)
            .map(|s| {
                specular_settings::in_section(*s)
                    .filter(|m| m.applies_here() && !ch.setting_hidden(m.key))
                    .collect()
            })
            .unwrap_or_default()
    };
    let hs = cx.style(metrics::FONT_LARGE).weight(650.0);
    if !searching && let Some(sec) = secs.get(ch.settings.section) {
        // "Reset section" when anything in it is modified; the title gets the rest of the line
        // (ending in "…" if need be; the sidebar and the accessible name have it in full).
        let mut title_w = header.w;
        if metas.iter().any(|m| st.store.is_modified(m.key)) {
            let label = t!("settings-reset-section");
            let ls = cx.style(metrics::FONT_LABEL).weight(550.0);
            let b = cx.layout(&label, ls, None);
            let bw = (b.width + 20.0).min(header.w * 0.5);
            let bh = cx.u(28.0);
            let br = Rect::new(
                header.right() - bw,
                header.y + (header.h - bh) * 0.5,
                bw,
                bh,
            );
            let br = cx.m_in(br, header);
            let tgt = Target::Settings(SettingsTarget::ResetSection);
            if cx.is_hover(&tgt) {
                cx.dl.rect(br, metrics::RADIUS_SM, cx.pal.surface_hover);
            }
            cx.dl.push_clip(br);
            cx.block_in(&b, br, HAlign::Center, cx.pal.accent);
            cx.dl.pop_clip();
            if cx.is_focused(&tgt) {
                ring(cx, &SettingsTarget::ResetSection, br);
            }
            cx.hit(br, tgt.clone());
            stops.push(KeyStop {
                t: tgt,
                row: None,
                at: None,
            });
            title_w -= bw + 12.0;
        }
        let tr = cx.m_in(Rect::new(header.x, header.y, title_w, header.h), header);
        cx.label_fit(&sec.label(i18n), hs, tr, HAlign::Start, cx.pal.text, false);
    } else if searching {
        let q = specular_i18n::isolate(ch.settings.search.trim());
        let label = if metas.is_empty() {
            t!("settings-search-no-results", query = q)
        } else {
            t!("settings-title")
        };
        cx.label_fit(&label, hs, header, HAlign::Start, cx.pal.text, false);
    }

    let body = Rect::new(
        content.x,
        header.bottom() + 12.0,
        content.w,
        content.bottom() - header.bottom() - 16.0,
    );
    cx.dl.push_clip(body);
    ch.settings.body = body;
    let pad = 24.0;
    let ctrl_w = cx.u(220.0).min(body.w * 0.42);
    let ctrl_h = cx.u(30.0);
    let text_w = body.w - pad * 2.0 - ctrl_w - 44.0;
    // The text width when a row stacks its control under the text (room for the reset button).
    let stack_w = body.w - pad * 2.0 - 36.0;
    // A control reached with Tab scrolls into view: where it was last drawn, else its row
    // (then again once drawn).
    if std::mem::take(&mut ch.reveal_focus) {
        ch.settings.reveal = 3;
    }
    if ch.settings.reveal > 0 {
        ch.settings.reveal -= 1;
        match ch.settings_focus_span() {
            Some((fy, fh, exact)) => {
                let m = 8.0;
                let s = ch.settings.scroll;
                let to = if fh + 2.0 * m > body.h {
                    fy - m
                } else {
                    s.min(fy - m).max(fy + fh + m - body.h)
                }
                .max(0.0);
                ch.settings.scroll = to;
                if exact && (to - s).abs() < 0.5 {
                    ch.settings.reveal = 0;
                }
            }
            None => ch.settings.reveal = 0,
        }
    }
    let mut y = body.y - ch.settings.scroll;
    let y0 = y;
    let label_s = cx.style(metrics::FONT_BODY).weight(520.0);
    let help_s = cx.style(metrics::FONT_SMALL);
    let mut dropdown_anchor: Option<(Rect, &'static SettingMeta)> = None;
    // A refused field's reason, by setting: shown with the field when it comes into view.
    let mut refused: Vec<(&'static str, f32)> = Vec::new();
    for m in &metas {
        if ch.settings.scroll_to == Some(m.key) {
            ch.settings.scroll_to = None;
            ch.settings.start = Some(rows.len());
            ch.settings.scroll = (y + ch.settings.scroll - body.y).max(0.0);
        }
        let row_top = y - y0;
        let label = m.label(i18n);
        let help = m.help(i18n);
        let status = ch.status_of(m.key).cloned();
        let is_list = matches!(m.kind, Kind::List { .. });
        // The toolbar has its own editor: a preview and an ordered list.
        let toolbar = m.key == crate::toolbar_edit::KEY;
        ch.settings.tools.on_page |= toolbar;
        // So has the key bindings' row: its list under the text, no button.
        let keys = m.key == crate::keys_edit::KEY;
        // The text beside the control; when a word doesn't fit there (long compounds at a large
        // text size) or the column is too narrow to read, the text takes the full width and the
        // control goes under it.
        // A button as wide as its label: wider than the control column, it goes under the text.
        let button_label = match &status {
            _ if keys => None,
            Some(s) => s.button.clone(),
            None => matches!(m.kind, Kind::Action).then(|| label.clone()),
        };
        let button_w = button_label.as_ref().map(|l| {
            cx.layout(l, cx.style(metrics::FONT_LABEL).weight(550.0), None)
                .width
                + 24.0
        });
        // A text or path field takes the row's width under its text: values are long.
        let field = crate::setting_field::is_field(m)
            && !status.as_ref().is_some_and(|s| owns_control(m, s));
        let mut stacked = keys
            || field
            || !is_list && (text_w < cx.u(160.0) || button_w.is_some_and(|w| w > ctrl_w + 0.5));
        let (lb, hb, rb, sb, tw) = loop {
            let tw = if stacked { stack_w } else { text_w };
            let lb = cx.layout(&label, label_s, Some(tw));
            let hb =
                (!help.is_empty() && help != m.help_id).then(|| cx.layout(&help, help_s, Some(tw)));
            let rb = m
                .needs_restart
                .then(|| cx.layout(&t!("settings-restart-required"), help_s, Some(tw)));
            let sb = status.as_ref().filter(|s| !s.state.is_empty()).map(|s| {
                (
                    cx.layout(&s.state, help_s.weight(550.0), Some(tw)),
                    s.attention,
                )
            });
            let widest = [Some(lb.width), hb.as_ref().map(|b| b.width)]
                .into_iter()
                .flatten()
                .chain(rb.as_ref().map(|b| b.width))
                .chain(sb.as_ref().map(|b| b.0.width))
                .fold(0.0f32, f32::max);
            if stacked || is_list || widest <= tw + 0.5 {
                break (lb, hb, rb, sb, tw);
            }
            stacked = true;
        };
        let text_w = tw;
        // The control's width in this row: a stacked button may take the full width.
        let cw = match button_w {
            _ if field => stack_w,
            Some(bw) if stacked => bw.min(stack_w).max(ctrl_w),
            _ => ctrl_w,
        };
        // Why the field's text was refused, under it.
        let eb = field
            .then(|| crate::setting_field::error_block(ch, cx, m, stack_w))
            .flatten();
        let err_h = eb.as_ref().map_or(0.0, |b| b.height + 4.0);
        if eb.is_some() {
            refused.push((m.key, err_h));
        }
        // A colour's swatches, under its field (and the reason it was refused).
        let grid_h = if field {
            match crate::setting_field::swatch_grid_height(m, stack_w) {
                h if h > 0.0 => h + 10.0,
                _ => 0.0,
            }
        } else {
            0.0
        };
        // A list is edited with chips (one per option) under the help text, full width.
        let vb = (is_list && !toolbar).then(|| chips(cx, m, i18n, body.w - pad * 2.0));
        let live = ch.settings.tools.live;
        let eh = if keys {
            Some(crate::keys_edit::editor_height(
                ch,
                cx,
                st,
                body.w - pad * 2.0,
            ))
        } else {
            toolbar.then(|| crate::toolbar_edit::editor_height(cx, st, body.w - pad * 2.0, live))
        };
        let th = lb.height
            + hb.as_ref().map_or(0.0, |b| b.height + 3.0)
            + rb.as_ref().map_or(0.0, |b| b.height + 3.0)
            + sb.as_ref().map_or(0.0, |b| b.0.height + 3.0)
            + vb.as_ref().map_or(0.0, |c| c.1 + 10.0)
            + eh.map_or(0.0, |h| h + 10.0);
        let rh = if keys {
            th + 22.0
        } else if stacked {
            th + 8.0 + ctrl_h + err_h + grid_h + 22.0
        } else {
            th.max(ctrl_h + 2.0) + 22.0
        };
        let row = Rect::new(body.x + pad, y, body.w - pad * 2.0, rh);
        if row.bottom() > body.y && row.y < body.bottom() {
            let tx = cx.m_in(Rect::new(row.x, row.y + 11.0, text_w, lb.height), row);
            cx.block_top(&lb, tx, HAlign::Start, cx.pal.text);
            let mut ty = row.y + 11.0 + lb.height + 3.0;
            if let Some(hb) = &hb {
                let hr = cx.m_in(Rect::new(row.x, ty, text_w, hb.height), row);
                cx.block_top(hb, hr, HAlign::Start, cx.pal.text_muted);
                ty += hb.height + 3.0;
            }
            if let Some(rb) = &rb {
                let rr = cx.m_in(Rect::new(row.x, ty, text_w, rb.height), row);
                cx.block_top(rb, rr, HAlign::Start, cx.pal.accent);
                ty += rb.height + 3.0;
            }
            if let Some((sb, attention)) = &sb {
                let sr = cx.m_in(Rect::new(row.x, ty, text_w, sb.height), row);
                let c = if *attention {
                    cx.pal.accent
                } else {
                    cx.pal.text
                };
                cx.block_top(sb, sr, HAlign::Start, c);
                ty += sb.height + 3.0;
            }
            if let Some((list, _)) = &vb {
                let on: Vec<String> = match st.store.get(m.key) {
                    Ok(Value::List(l)) => l,
                    _ => Vec::new(),
                };
                for (cr, b, opt) in list {
                    let r = Rect::new(row.x + cr.x, ty + 7.0 + cr.y, cr.w, cr.h);
                    let r = cx.m_in(r, row);
                    let tgt = Target::Settings(SettingsTarget::ListItem(m.key, opt));
                    let sel = on.iter().any(|x| x == opt);
                    let hover = cx.is_hover(&tgt);
                    let radius = r.h * 0.5;
                    if sel {
                        cx.dl.rect(r, radius, cx.pal.accent_soft);
                    } else {
                        cx.dl.rect_border(
                            r,
                            radius,
                            specular_theme::Color::TRANSPARENT,
                            cx.pal.text_faint.with_alpha(0.6),
                            1.0,
                        );
                    }
                    if hover {
                        cx.dl.rect(r, radius, cx.pal.surface_hover);
                    }
                    let (icon_r, text_r) = if cx.rtl {
                        (
                            Rect::new(r.right() - 24.0, r.y, 16.0, r.h),
                            Rect::new(r.x + 10.0, r.y, r.w - 34.0, r.h),
                        )
                    } else {
                        (
                            Rect::new(r.x + 8.0, r.y, 16.0, r.h),
                            Rect::new(r.x + 24.0, r.y, r.w - 34.0, r.h),
                        )
                    };
                    if sel {
                        draw_icon(
                            cx.dl,
                            Icon::Check,
                            icon_r.centered(13.0, 13.0),
                            cx.pal.accent,
                            1.8,
                            false,
                        );
                    }
                    let c = if sel { cx.pal.text } else { cx.pal.text_muted };
                    cx.block_in(b, text_r, HAlign::Center, c);
                    cx.hit(r, tgt);
                }
            }
            if let Some(eh) = eh {
                let er = Rect::new(row.x, ty + 7.0, row.w, eh);
                if keys {
                    crate::keys_edit::draw_editor(ch, cx, st, er, body, y0);
                } else {
                    crate::toolbar_edit::draw_editor(ch, cx, st, er);
                }
            }
            let ctrl = if stacked {
                Rect::new(
                    row.x,
                    row.bottom() - 11.0 - err_h - grid_h - ctrl_h,
                    cw,
                    ctrl_h,
                )
            } else {
                Rect::new(
                    row.right() - ctrl_w,
                    row.y + (rh - ctrl_h) * 0.5,
                    ctrl_w,
                    ctrl_h,
                )
            };
            let ctrl = cx.m_in(ctrl, row);
            let anchor = match &status {
                _ if keys => ctrl,
                Some(s) if owns_control(m, s) => match &s.button {
                    Some(label) => action_button(cx, m, ctrl, label, false),
                    None => ctrl,
                },
                _ if field => crate::setting_field::draw(ch, cx, m, ctrl, st),
                _ => control(cx, m, ctrl, st, ch.setting_disabled(m.key)),
            };
            if let Some(eb) = &eb {
                let er = cx.m_in(
                    Rect::new(row.x, ctrl.bottom() + 4.0, stack_w, eb.height),
                    row,
                );
                cx.block_top(eb, er, HAlign::Start, cx.pal.danger);
            }
            if grid_h > 0.0 {
                let gr = Rect::new(row.x, ctrl.bottom() + err_h + 10.0, stack_w, grid_h - 10.0);
                let gr = cx.m_in(gr, row);
                crate::setting_field::draw_swatches(cx, m, gr, st);
            }
            if ch.settings.dropdown == Some(m.key) {
                dropdown_anchor = Some((anchor, m));
            }
            if st.store.is_modified(m.key) && !matches!(m.kind, Kind::Action) && !keys {
                // A list has no control and its chips span the full width below the text, so
                // its reset sits at the end of the row, centred on the label and help text.
                let rr = if is_list {
                    let text_mid = (row.y + 11.0 + ty - 3.0) * 0.5;
                    Rect::new(row.right() - 28.0, text_mid - 14.0, 28.0, 28.0)
                } else if stacked {
                    let c = row.bottom() - 11.0 - err_h - grid_h - ctrl_h * 0.5;
                    Rect::new(row.x + cw + 8.0, c - 14.0, 28.0, 28.0)
                } else {
                    Rect::new(
                        row.right() - ctrl_w - 36.0,
                        row.y + (rh - 28.0) * 0.5,
                        28.0,
                        28.0,
                    )
                };
                let rr = cx.m_in(rr, row);
                cx.icon_button(
                    rr,
                    Icon::Undo,
                    Target::Settings(SettingsTarget::Reset(m.key)),
                    cx.pal.text_faint,
                    cx.rtl,
                    false,
                );
            }
            cx.dl.rect(
                Rect::new(row.x, row.bottom() - 1.0, row.w, 1.0),
                0.0,
                cx.pal.separator,
            );
        }
        y += rh;
        let panel = ch
            .setting_panels
            .get(m.key)
            .is_some_and(|p| !searching || p.matches(&ch.settings.search));
        if panel {
            y += crate::settings_panel::draw(ch, cx, m.key, body, pad, ctrl_w, y);
        }
        let row_i = rows.len();
        rows.push((row_top, y - y0 - row_top));
        for t in row_stops(ch, m, status.as_ref(), st, panel) {
            stops.push(KeyStop {
                t: Target::Settings(t),
                row: Some(row_i),
                at: None,
            });
        }
    }
    // Where the controls were drawn (those in view), and the focus ring.
    for s in &mut stops {
        if s.row.is_some() {
            s.at = cx.anchor_of(Some(&s.t)).map(|r| (r.y - y0, r.h));
        }
        if let (Target::Settings(SettingsTarget::Field(k)), Some((_, h))) = (&s.t, &mut s.at)
            && let Some((_, e)) = refused.iter().find(|(r, _)| r == k)
        {
            *h += e;
        }
    }
    ch.settings.tool_at = ch.settings.tools.focus.and_then(|i| {
        cx.anchor_of(Some(&Target::Settings(SettingsTarget::ToolRow(i))))
            .map(|r| (r.y - y0, r.h))
    });
    if let Some(Target::Settings(f)) = ch.modal_focus.clone()
        && ch.settings.tools.focus.is_none()
        && stops.iter().any(|s| s.row.is_some() && s.t == Target::Settings(f.clone()))
        // Drawn by `icon_button` with its ring; the key editor draws its own.
        && !matches!(f, SettingsTarget::Reset(_))
        // A field shows the keyboard with its border and caret.
        && !matches!(f, SettingsTarget::Field(_))
        && !f.is_keys()
        && let Some(r) = cx.anchor_of(Some(&Target::Settings(f.clone())))
    {
        ring(cx, &f, r);
    }
    // Keys are recorded only while their row is on the page.
    if !ch.settings.keys.on_page {
        ch.settings.keys.recording = None;
        // Where Esc on the Keys page goes back to.
        if ch.settings.search.is_empty() {
            ch.settings.keys.from = Some((ch.settings.section, ch.settings.scroll));
        }
    }
    ch.settings.stops = stops;
    ch.settings.rows = rows;
    ch.settings.view = (body.y - y0, body.h);
    cx.dl.pop_clip();
    let total = y + ch.settings.scroll - body.y;
    let max = (total - body.h + 16.0).max(0.0);
    // Clamped by the scrollbar (it redraws at once if the content shrank under the offset).
    crate::scrollbar::draw(ch, cx, crate::scrollbar::Area::Settings, body, body, max);

    if let Some((anchor, m)) = dropdown_anchor {
        dropdown(ch, cx, anchor, m, st);
    }
    crate::toolbar_edit::draw_row_menu(ch, cx, st);
}

/// The button of an action setting, right-aligned in `r`; `disabled`: greyed out, pressing
/// does nothing (it is still hit, for the accessibility tree).
fn action_button(
    cx: &mut Ctx,
    m: &'static SettingMeta,
    r: Rect,
    label: &str,
    disabled: bool,
) -> Rect {
    let tgt = Target::Settings(SettingsTarget::Act(m.key));
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    let label = cx.ellipsize(label, style, r.w - 24.0, false);
    let b = cx.layout(&label, style, None);
    let bw = (b.width + 24.0).min(r.w);
    let br = cx.m_in(Rect::new(r.right() - bw, r.y, bw, r.h), r);
    let fill = if disabled {
        cx.pal.surface
    } else if cx.is_hover(&tgt) {
        cx.pal.surface_pressed
    } else {
        cx.pal.surface_raised
    };
    cx.dl
        .rect_border(br, metrics::RADIUS_SM + 1.0, fill, cx.pal.separator, 1.0);
    cx.dl.push_clip(br);
    let ink = if disabled {
        cx.pal.text_muted.with_alpha(0.6)
    } else {
        cx.pal.text
    };
    cx.block_in(&b, br, HAlign::Center, ink);
    cx.dl.pop_clip();
    cx.hit(br, tgt);
    br
}

/// Draws the control of a setting (`disabled`: an action's greyed-out button); returns its
/// rectangle.
fn control(
    cx: &mut Ctx,
    m: &'static SettingMeta,
    r: Rect,
    st: &ViewerState,
    disabled: bool,
) -> Rect {
    let i18n = I18n::global();
    let v = st.store.get(m.key).unwrap_or(Value::None);
    let vs = cx.style(metrics::FONT_BODY);
    match &m.kind {
        Kind::Bool => {
            let on = matches!(v, Value::Bool(true));
            let sw = Rect::new(r.right() - 42.0, r.center().1 - 11.0, 42.0, 22.0);
            let sw = cx.m_in(sw, r);
            let tgt = Target::Settings(SettingsTarget::Toggle(m.key));
            let track = if on {
                cx.pal.accent_fill
            } else {
                cx.pal.surface_pressed
            };
            cx.dl.rect(sw, 11.0, track);
            if cx.is_hover(&tgt) {
                cx.dl.rect(sw, 11.0, cx.pal.surface_hover);
            }
            let knob_x = if on != cx.rtl {
                sw.right() - 11.0
            } else {
                sw.x + 11.0
            };
            cx.dl.shadow(
                Rect::new(knob_x - 8.0, sw.y + 3.0, 16.0, 16.0),
                8.0,
                cx.pal.shadow.fade(0.6),
                3.0,
                1.0,
            );
            cx.dl
                .circle((knob_x, sw.center().1), 8.0, specular_theme::Color::WHITE);
            cx.hit(sw.inset(-4.0), tgt);
            sw
        }
        Kind::Enum { .. } | Kind::Language | Kind::Locale | Kind::Monitor | Kind::Pick => {
            let tgt = Target::Settings(SettingsTarget::Dropdown(m.key));
            let fill = if cx.is_hover(&tgt) {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            };
            cx.dl
                .rect_border(r, metrics::RADIUS_SM + 1.0, fill, cx.pal.separator, 1.0);
            let tr = cx.m_in(Rect::new(r.x + 10.0, r.y, r.w - 34.0, r.h), r);
            cx.label_fit(
                &value_label(m, &v, i18n),
                vs,
                tr,
                HAlign::Start,
                cx.pal.text,
                false,
            );
            let cr = cx.m_in(Rect::new(r.right() - 22.0, r.y, 12.0, r.h), r);
            cx.chevron_down(cr.center(), cx.pal.text_muted);
            cx.hit(r, tgt);
            r
        }
        Kind::Int { .. } | Kind::Float { .. } => {
            let bw = r.h;
            let minus = cx.m_in(Rect::new(r.x, r.y, bw, r.h), r);
            let plus = cx.m_in(Rect::new(r.right() - bw, r.y, bw, r.h), r);
            cx.dl.rect_border(
                r,
                metrics::RADIUS_SM + 1.0,
                cx.pal.surface_raised,
                cx.pal.separator,
                1.0,
            );
            for (b, d, glyph) in [(minus, -1i8, false), (plus, 1i8, true)] {
                let tgt = Target::Settings(SettingsTarget::Step(m.key, d));
                if cx.is_hover(&tgt) {
                    cx.dl
                        .rect(b.inset(2.0), metrics::RADIUS_SM, cx.pal.surface_hover);
                }
                let c = b.center();
                cx.dl
                    .line((c.0 - 5.0, c.1), (c.0 + 5.0, c.1), 1.6, cx.pal.text_muted);
                if glyph {
                    cx.dl
                        .line((c.0, c.1 - 5.0), (c.0, c.1 + 5.0), 1.6, cx.pal.text_muted);
                }
                cx.hit(b, tgt);
            }
            let vr = Rect::new(r.x + bw, r.y, r.w - 2.0 * bw, r.h);
            cx.label(
                &m.display_value(&v, i18n),
                vs.tabular(),
                vr,
                HAlign::Center,
                cx.pal.text,
            );
            r
        }
        Kind::Color => {
            let sw = cx.m_in(
                Rect::new(r.right() - 34.0, r.center().1 - 11.0, 34.0, 22.0),
                r,
            );
            if let Value::Color(c) = &v {
                let col = specular_theme::Color::rgba(
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                );
                cx.dl.rect_border(sw, 5.0, col, cx.pal.separator, 1.0);
            }
            let tr = cx.m_in(Rect::new(r.x, r.y, r.w - 44.0, r.h), r);
            cx.label(
                &m.display_value(&v, i18n),
                vs.tabular(),
                tr,
                HAlign::End,
                cx.pal.text_muted,
            );
            r
        }
        Kind::Action => action_button(cx, m, r, &m.label(i18n), disabled),
        // Drawn under the help text by the caller.
        Kind::List { .. } => r,
        // Drawn as fields by the caller (setting_field.rs).
        Kind::Path { .. } | Kind::Shortcut | Kind::Text => r,
    }
}

fn dropdown(
    ch: &mut Chrome,
    cx: &mut Ctx,
    anchor: Rect,
    m: &'static SettingMeta,
    st: &ViewerState,
) {
    let i18n = I18n::global();
    let current = st
        .store
        .get(m.key)
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_default();
    let opts = options(m, i18n, st, &current);
    let row = cx.u(if opts.iter().any(|o| o.2.is_some()) {
        40.0
    } else {
        32.0
    });
    let full = opts.len() as f32 * row + 8.0;
    let below = cx.h - anchor.bottom() - 16.0;
    let above = anchor.y - 16.0;
    let h = full.min(below.max(above)).max(row + 8.0);
    let y = if full <= below || below >= above {
        anchor.bottom() + 4.0
    } else {
        anchor.y - 4.0 - h
    };
    let ls = cx.style(metrics::FONT_BODY);
    let ss = cx.style(metrics::FONT_SMALL);
    // As wide as the longest option (check mark and padding included), within the window.
    let widest = opts.iter().fold(0.0f32, |acc, (_, label, sub)| {
        let sw = sub.as_ref().map_or(0.0, |s| cx.text.measure(s, ss));
        acc.max(cx.text.measure(label, ls)).max(sw)
    });
    let w = anchor.w.max(240.0).max(widest + 46.0).min(cx.w - 16.0);
    let x = if cx.rtl { anchor.x } else { anchor.right() - w };
    let r = Rect::new(x.clamp(8.0, cx.w - w - 8.0), y, w, h);
    // Clicks outside close the list.
    cx.hit(Rect::new(0.0, 0.0, cx.w, cx.h), Target::Scrim);
    cx.dl
        .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 22.0, 8.0);
    cx.dl.rect_border(
        r,
        metrics::RADIUS_MD,
        cx.pal.surface_raised,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(r, Target::Settings(SettingsTarget::Dropdown_));
    cx.popovers.push(r);
    let max = (full - h).max(0.0);
    // The option the keyboard is on.
    let kbd = match &ch.modal_focus {
        Some(Target::Settings(SettingsTarget::Choose(k, v))) if *k == m.key => {
            opts.iter().position(|o| o.0 == *v)
        }
        _ => None,
    };
    if std::mem::take(&mut ch.settings.dropdown_reveal)
        && let Some(i) = kbd
    {
        let (top, view) = (i as f32 * row, h - 8.0);
        let s = &mut ch.settings.dropdown_scroll;
        *s = s.min(top).max(top + row - view);
    }
    ch.settings.dropdown_scroll = ch.settings.dropdown_scroll.clamp(0.0, max);
    let inner = r.inset(4.0);
    cx.dl.push_clip(inner);
    for (i, (val, label, sub)) in opts.iter().enumerate() {
        let rr = Rect::new(
            inner.x,
            inner.y + i as f32 * row - ch.settings.dropdown_scroll,
            inner.w,
            row,
        );
        if rr.bottom() < inner.y || rr.y > inner.bottom() {
            continue;
        }
        let tgt = Target::Settings(SettingsTarget::Choose(m.key, val.clone()));
        if cx.is_hover(&tgt) || kbd == Some(i) {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        if kbd == Some(i) {
            cx.focus_ring(rr, metrics::RADIUS_SM);
        }
        if *val == current {
            let ir = cx
                .m_in(Rect::new(rr.x + 6.0, rr.y, 18.0, rr.h), rr)
                .centered(16.0, 16.0);
            draw_icon(cx.dl, Icon::Check, ir, cx.pal.accent, 1.8, false);
        }
        let tr = cx.m_in(Rect::new(rr.x + 30.0, rr.y, rr.w - 38.0, rr.h), rr);
        match sub {
            Some(s) => {
                let top = Rect::new(tr.x, tr.y + 3.0, tr.w, tr.h * 0.5);
                cx.label_fit(label, ls, top, HAlign::Start, cx.pal.text, false);
                let bot = Rect::new(tr.x, tr.y + tr.h * 0.5 - 1.0, tr.w, tr.h * 0.5);
                cx.label_fit(s, ss, bot, HAlign::Start, cx.pal.text_muted, false);
            }
            None => {
                cx.label_fit(label, ls, tr, HAlign::Start, cx.pal.text, false);
            }
        }
        cx.hit(rr.intersect(&inner), tgt);
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(ch, cx, crate::scrollbar::Area::Dropdown, inner, r, max);
}

// ----- Language picker ---------------------------------------------------

#[derive(Default)]
pub(crate) struct LanguageState {
    pub query: String,
    /// The search field's caret, selection and undo history (`query` is the truth).
    pub query_edit: crate::line_edit::LineEdit,
    /// Setting values ("system", "de", …) of the rows shown.
    pub ids: Vec<String>,
    pub rows: Vec<(String, String, String)>,
    pub selected: usize,
    pub scroll: f32,
    pub dirty: bool,
}

impl LanguageState {
    pub fn reset(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.scroll = 0.0;
        self.dirty = true;
    }
    pub fn type_text(&mut self, s: &str) {
        self.edit().insert(s);
        self.edited();
    }
    pub fn backspace(&mut self) {
        let mut clip = crate::line_edit::MemoryClipboard::default();
        let k = specular_input::Key::Named(specular_input::NamedKey::Backspace);
        self.edit()
            .key(k, specular_input::Mods::NONE, false, &mut clip);
        self.edited();
    }
    /// The search field, caught up with `query`.
    pub fn edit(&mut self) -> &mut crate::line_edit::LineEdit {
        self.query_edit.sync(&self.query);
        &mut self.query_edit
    }
    /// The search field edited: the list follows it.
    pub fn edited(&mut self) {
        if self.query != self.query_edit.text() {
            self.query = self.query_edit.text().to_owned();
            self.selected = 0;
            self.dirty = true;
        }
    }
    pub fn move_sel(&mut self, d: i32) {
        let n = self.ids.len() as i32;
        if n > 0 {
            self.selected = (self.selected as i32 + d).clamp(0, n - 1) as usize;
            self.scroll = f32::NAN;
        }
    }

    pub(crate) fn rebuild(&mut self, st: &ViewerState, current: &str) {
        self.dirty = false;
        let loc = I18n::global().localizer();
        let q = fold_for_search(self.query.trim()).to_lowercase();
        let matches = |s: &str| q.is_empty() || fold_for_search(s).to_lowercase().contains(&q);
        self.ids.clear();
        self.rows.clear();
        let sys = t!("settings-value-system-language");
        if matches(&sys) {
            self.ids.push("system".into());
            self.rows.push((sys, String::new(), String::new()));
        }
        let mut langs: Vec<&LanguageInfo> = languages().iter().collect();
        langs.sort_by_key(|l| l.pseudo);
        for l in langs {
            if !offer_language(l, st, current) {
                continue;
            }
            let id = l.id.to_string();
            if !(matches(&l.native_name) || matches(&l.english_name) || matches(&id)) {
                continue;
            }
            let sub = if l.pseudo {
                if l.direction.is_rtl() {
                    t!("language-pseudo-rtl")
                } else {
                    t!("language-pseudo-accented")
                }
            } else {
                l.english_name.clone()
            };
            let pct = t!(
                "language-completeness",
                percent = loc.percent(l.completeness as f64, 0)
            );
            self.ids.push(id);
            self.rows.push((l.native_name.clone(), sub, pct));
        }
        self.selected = self.selected.min(self.ids.len().saturating_sub(1));
    }
}

pub(crate) fn draw_language(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState) {
    let on_field = ch.modal_focus.is_none();
    let ls = &mut ch.language;
    let current = match &st.settings.general.language {
        LanguagePreference::System => "system".to_owned(),
        LanguagePreference::Language(l) => l.to_string(),
    };
    if ls.dirty {
        ls.rebuild(st, &current);
    }
    let w = (cx.w - 48.0).min(cx.u(480.0));
    let row = cx.u(48.0);
    let h = (cx.h - 80.0).min(cx.u(560.0));
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let ts = cx.style(metrics::FONT_TITLE).weight(650.0);
    let title_h = cx.u(30.0);
    let ir = cx.m_in(Rect::new(r.x + 20.0, r.y + 14.0, 22.0, title_h), r);
    draw_icon(
        cx.dl,
        Icon::Globe,
        ir.centered(18.0, 18.0),
        cx.pal.text_muted,
        1.6,
        false,
    );
    let tr2 = cx.m_in(Rect::new(r.x + 50.0, r.y + 14.0, r.w - 110.0, title_h), r);
    cx.label_fit(
        &t!("language-picker-title"),
        ts,
        tr2,
        HAlign::Start,
        cx.pal.text,
        false,
    );
    let close = cx.m_in(
        Rect::new(
            r.right() - 46.0,
            r.y + 13.0 + (title_h - 30.0) * 0.5,
            32.0,
            32.0,
        ),
        r,
    );
    // Tab reaches Close; the field still takes what is typed.
    cx.close_button(close);
    let field = Rect::new(r.x + 16.0, r.y + 24.0 + title_h, r.w - 32.0, cx.u(34.0));
    cx.field(
        field,
        &ls.query,
        Some(&ls.query_edit),
        &t!("language-picker-search"),
        on_field,
        true,
    );
    let list = Rect::new(
        r.x + 8.0,
        field.bottom() + 10.0,
        r.w - 16.0,
        r.bottom() - field.bottom() - 18.0,
    );
    let content = ls.ids.len() as f32 * row;
    let max = (content - list.h).max(0.0);
    if ls.scroll.is_nan() {
        let top = ls.selected as f32 * row;
        ls.scroll = (top - list.h * 0.5 + row * 0.5).clamp(0.0, max);
    }
    ls.scroll = ls.scroll.clamp(0.0, max);
    cx.dl.push_clip(list);
    let ns = cx.style(metrics::FONT_BODY).weight(520.0);
    let ss = cx.style(metrics::FONT_SMALL);
    for (i, (id, (name, sub, pct))) in ls.ids.iter().zip(&ls.rows).enumerate() {
        let rr = Rect::new(list.x, list.y + i as f32 * row - ls.scroll, list.w, row);
        if rr.bottom() < list.y || rr.y > list.bottom() {
            continue;
        }
        let tgt = Target::LanguageRow(i);
        if i == ls.selected {
            cx.dl.rect(
                rr.inset_xy(0.0, 1.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.accent_soft,
            );
        } else if cx.is_hover(&tgt) {
            cx.dl.rect(
                rr.inset_xy(0.0, 1.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.surface_hover,
            );
        }
        if *id == current {
            let ir = cx
                .m_in(Rect::new(rr.x + 8.0, rr.y, 20.0, rr.h), rr)
                .centered(16.0, 16.0);
            draw_icon(cx.dl, Icon::Check, ir, cx.pal.accent, 1.8, false);
        }
        let pb = cx.layout(pct, ss, None);
        let tw = rr.w - 44.0 - pb.width - 16.0;
        let tr = cx.m_in(Rect::new(rr.x + 36.0, rr.y, tw, rr.h), rr);
        // Long names end in "…" before the percentage (the accessible name is in full).
        if sub.is_empty() {
            cx.label_fit(name, ns, tr, HAlign::Start, cx.pal.text, false);
        } else {
            cx.label_fit(
                name,
                ns,
                Rect::new(tr.x, tr.y + 4.0, tr.w, rr.h * 0.5),
                HAlign::Start,
                cx.pal.text,
                false,
            );
            cx.label_fit(
                sub,
                ss,
                Rect::new(tr.x, tr.y + rr.h * 0.5 - 2.0, tr.w, rr.h * 0.5),
                HAlign::Start,
                cx.pal.text_muted,
                false,
            );
        }
        if !pct.is_empty() {
            let pr = cx.m_in(
                Rect::new(rr.right() - pb.width - 12.0, rr.y, pb.width, rr.h),
                rr,
            );
            cx.block_in(&pb, pr, HAlign::End, cx.pal.text_faint);
        }
        cx.hit(rr.intersect(&list), tgt);
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(ch, cx, crate::scrollbar::Area::Language, list, r, max);
}

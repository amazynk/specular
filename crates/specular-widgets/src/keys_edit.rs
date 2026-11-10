//! Settings → Keys (DESIGN §11): every bindable action by mode, searchable by name or key;
//! a key, chord or sequence is recorded by pressing it, checked against the mode's other
//! bindings and the shortcuts the system takes, and saved by the application in
//! `keymap.toml` ([`Command::Keymap`]). Presets, import and export, resets.
//!
//! The list is one Tab stop with a roving focus: ↑/↓ go through the actions, ←/→ through a
//! row's parts (the row, its keys, Add, Reset, a conflict's choices), Enter or Space record
//! or press, Delete removes a key. The keyboard focus is `modal_focus` itself, so the
//! focus ring, the accessibility tree and scrolling into view work as for any control.

use std::sync::Arc;

use accesskit::{Action as AkAction, Node, Role, Toggled};
use specular_gfx::{Icon, Rect, TextBlock, draw_icon};
use specular_i18n::{I18n, fold_for_search, t};
use specular_input::{
    Action, Conflict, Displace, Group, KeyCombo, KeySeq, Keymap, KeymapEdit, Mode, NamedKey,
    Preset, RecordEvent, Recorder, conflicts,
};
use specular_theme::{Color, metrics};

use crate::ctx::{Ctx, HAlign, Target};
use crate::settings::SettingsTarget as S;
use crate::{Chrome, Command, Modal, Response, ViewerState};

/// The setting the editor sits under.
pub(crate) const KEY: &str = "keys.bindings";
/// Problems of `keymap.toml` shown at most (the log has them all).
const MAX_PROBLEMS: usize = 5;

/// An answer to a conflict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Take the keys from the bindings they clash with.
    Replace,
    /// Give the re-recorded binding's old keys to the action that had the new ones.
    Swap,
    /// Keep the sequences the new keys begin, or are begun by.
    Keep,
    /// Bind keys the system may take, or that a mode shadows.
    Anyway,
    Cancel,
}

impl Choice {
    fn label(self) -> String {
        match self {
            Choice::Replace => t!("keys-choice-replace"),
            Choice::Swap => t!("keys-choice-swap"),
            Choice::Keep => t!("keys-choice-keep"),
            Choice::Anyway => t!("keys-choice-anyway"),
            Choice::Cancel => t!("keys-choice-cancel"),
        }
    }
}

/// The answers offered for `cs` (`old`: a binding is being re-recorded).
pub(crate) fn choices_for(cs: &[Conflict], old: bool) -> Vec<Choice> {
    let same = cs.iter().any(|c| matches!(c, Conflict::SameKey { .. }));
    let prefix = cs.iter().any(|c| matches!(c, Conflict::Prefix { .. }));
    let mut v = Vec::new();
    if same || prefix {
        v.push(Choice::Replace);
        if same && old {
            v.push(Choice::Swap);
        }
        if prefix && !same {
            v.push(Choice::Keep);
        }
    } else {
        v.push(Choice::Anyway);
    }
    v.push(Choice::Cancel);
    v
}

/// A change that replaces many bindings, waiting for its confirmation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Confirm {
    Preset(Preset),
    ResetMode(Mode),
    ResetAll,
}

/// Keys being recorded for an action.
#[derive(Clone, Debug)]
pub(crate) struct Recording {
    pub mode: Mode,
    pub action: Action,
    /// The binding being re-recorded.
    pub old: Option<KeySeq>,
    /// Where the keyboard goes back to.
    from: S,
    pub rec: Recorder,
}

/// A recorded binding that clashes with others, and the answers to it.
#[derive(Clone, Debug)]
pub(crate) struct Prompt {
    pub mode: Mode,
    pub action: Action,
    pub keys: KeySeq,
    old: Option<KeySeq>,
    from: S,
    pub conflicts: Vec<Conflict>,
    pub choices: Vec<Choice>,
}

/// The editor's state.
pub(crate) struct KeysUi {
    /// The mode whose actions are listed.
    pub mode: Mode,
    pub recording: Option<Recording>,
    pub prompt: Option<Prompt>,
    pub confirm: Option<Confirm>,
    /// The editor was drawn on the page last drawn.
    pub on_page: bool,
    /// The Settings page the Keys page was reached from (section, scroll), for Esc to go
    /// back to ([`Chrome::keys_return`]).
    pub from: Option<(usize, f32)>,
    /// Where each of its controls was last laid out in the content (y, height).
    spans: Vec<(S, f32, f32)>,
}

impl Default for KeysUi {
    fn default() -> Self {
        KeysUi {
            mode: Mode::Viewer,
            recording: None,
            prompt: None,
            confirm: None,
            on_page: false,
            from: None,
            spans: Vec::new(),
        }
    }
}

impl KeysUi {
    /// Where control `s` was last laid out in the content.
    pub(crate) fn span(&self, s: &S) -> Option<(f32, f32)> {
        self.spans
            .iter()
            .find(|(t, ..)| t == s)
            .map(|(_, y, h)| (*y, *h))
    }
}

/// A target of the editor.
pub(crate) fn is_keys(s: &S) -> bool {
    matches!(
        s,
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
    ) || is_list(s)
}

/// A part of the list (it has the roving focus).
pub(crate) fn is_list(s: &S) -> bool {
    matches!(
        s,
        S::KeyRow(..)
            | S::KeyChip(..)
            | S::KeyRemove(..)
            | S::KeyAdd(..)
            | S::KeyResetAction(..)
            | S::KeyChoice(_)
            | S::KeyRecordCancel
            | S::KeyPromptText(_)
    )
}

/// Text that is read, not pressed.
fn is_text(s: &S) -> bool {
    matches!(
        s,
        S::KeyConfirmText | S::KeyProblem(_) | S::KeyPromptText(_) | S::KeyNoMatch
    )
}

// ----- Names ---------------------------------------------------------------

/// The name an action is listed under ([`Action::name`]).
pub(crate) fn action_name(a: Action) -> String {
    a.name()
}

fn tr(id: &str) -> String {
    I18n::global().tr(id)
}

fn mode_name(m: Mode) -> String {
    tr(m.message_id())
}

/// Keys as shown, never reordered by the text around them.
fn keys_text(k: &KeySeq) -> String {
    specular_i18n::isolate_ltr(&k.to_string())
}

fn combos_text(keys: &[KeyCombo]) -> String {
    let s: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    specular_i18n::isolate_ltr(&s.join(" "))
}

/// What conflict `c` of binding `keys` means.
fn conflict_text(c: &Conflict, keys: &KeySeq) -> String {
    let k = keys_text(keys);
    match c {
        Conflict::Already => String::new(),
        Conflict::SameKey { other } => {
            t!("keys-conflict-same", keys = k, other = action_name(*other))
        }
        Conflict::Prefix { keys: s, other } => t!(
            "keys-conflict-prefix",
            keys = k,
            sequence = keys_text(s),
            other = action_name(*other)
        ),
        Conflict::Shadow { mode, other } => t!(
            "keys-conflict-shadow",
            keys = k,
            other = action_name(*other),
            mode = mode_name(*mode)
        ),
        Conflict::Reserved(r) => t!(
            "keys-conflict-reserved",
            keys = k,
            reason = tr(r.message_id())
        ),
    }
}

/// The known name closest to a mistyped one, for "Did you mean …?".
fn closest<'a>(typed: &str, known: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let raw = typed;
    // `zoom_in` and `Zoom-In` are spellings of `zoom-in`: distance 0, still a suggestion.
    let typed = typed.trim().to_lowercase().replace('_', "-");
    known
        .into_iter()
        .filter(|k| *k != raw)
        .map(|k| (specular_settings::levenshtein(&typed, k), k))
        .filter(|(d, k)| *d <= (k.len() / 4).clamp(2, 4))
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

/// A problem of `keymap.toml`, in words (Settings → Keys, and the config editor's linter).
pub(crate) fn problem_text(p: &specular_input::KeymapError) -> String {
    use specular_input::KeymapError as E;
    match p {
        E::Syntax(e) => t!("keys-problem-syntax", error = e.clone()),
        E::UnknownKey(k) => t!("keys-problem-key", key = k.clone()),
        E::UnknownAction { key, action } => {
            let ids: Vec<String> = Action::BINDABLE.iter().map(|a| a.id()).collect();
            match closest(action, ids.iter().map(String::as_str)) {
                Some(s) => t!(
                    "keys-problem-action-suggestion",
                    key = key.clone(),
                    action = action.clone(),
                    suggestion = s.to_owned()
                ),
                None => t!(
                    "keys-problem-action",
                    key = key.clone(),
                    action = action.clone()
                ),
            }
        }
        E::UnknownMode(m) => match closest(m, Mode::ALL.iter().map(|m| m.id())) {
            Some(s) => t!(
                "keys-problem-mode-suggestion",
                mode = m.clone(),
                suggestion = s.to_owned()
            ),
            None => t!("keys-problem-mode", mode = m.clone()),
        },
        E::Duplicate { mode, keys, .. } => {
            t!(
                "keys-problem-duplicate",
                keys = keys_text(keys),
                mode = mode_name(*mode)
            )
        }
        E::Prefix { mode, keys, longer } => t!(
            "keys-problem-prefix",
            keys = keys_text(keys),
            longer = keys_text(longer),
            mode = mode_name(*mode)
        ),
        E::Reserved { keys, reason, .. } => t!(
            "keys-conflict-reserved",
            keys = keys_text(keys),
            reason = tr(reason.message_id())
        ),
        E::UnknownPreset(p) => match closest(p, Preset::ALL.iter().map(|p| p.id())) {
            Some(s) => t!(
                "keys-problem-preset-suggestion",
                preset = p.clone(),
                suggestion = s.to_owned()
            ),
            None => t!("keys-problem-preset", preset = p.clone()),
        },
    }
}

// ----- The list --------------------------------------------------------------

/// A line of the list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Line {
    Heading(String),
    Row(Mode, Action),
}

/// Whether action `a` matches the search `q` (folded): by its name, or by one of its keys in
/// any mode, as shown or as written in `keymap.toml`.
fn matches(km: &Keymap, a: Action, q: &str) -> bool {
    if fold_for_search(&action_name(a)).contains(q) {
        return true;
    }
    Mode::ALL.iter().any(|m| {
        km.keys_in(*m, a).iter().any(|k| {
            fold_for_search(&k.to_string()).contains(q) || fold_for_search(&k.spec()).contains(q)
        })
    })
}

/// Whether the search `query` finds an action or a key (the Keys row then shows in the
/// search results).
pub(crate) fn matches_any(km: &Keymap, query: &str) -> bool {
    let q = fold_for_search(query.trim());
    !q.is_empty()
        && modes(km)
            .into_iter()
            .any(|m| actions(km, m).any(|a| matches(km, a, &q)))
}

/// The modes listed: those whose actions the profile shows ([`Keymap::shows`]).
pub(crate) fn modes(km: &Keymap) -> Vec<Mode> {
    Mode::ALL
        .into_iter()
        .filter(|m| km.mode_shown(*m))
        .collect()
}

/// The mode whose actions are listed: `mode`, or the viewer once the profile hides it.
pub(crate) fn listed_mode(km: &Keymap, mode: Mode) -> Mode {
    if km.mode_shown(mode) {
        mode
    } else {
        Mode::Viewer
    }
}

/// A mode's actions the profile shows, in registry order.
fn actions(km: &Keymap, m: Mode) -> impl Iterator<Item = Action> + '_ {
    m.actions().filter(|a| km.shows(*a))
}

/// The list: while searching, the actions found in every mode, by mode; otherwise (or when
/// nothing is found) `mode`'s actions by group. The flag: the search found nothing.
pub(crate) fn lines(km: &Keymap, mode: Mode, query: &str) -> (Vec<Line>, bool) {
    let q = fold_for_search(query.trim());
    let mut out = Vec::new();
    let mode = listed_mode(km, mode);
    if !q.is_empty() {
        for m in modes(km) {
            let found: Vec<Action> = actions(km, m).filter(|a| matches(km, *a, &q)).collect();
            if found.is_empty() {
                continue;
            }
            out.push(Line::Heading(mode_name(m)));
            out.extend(found.into_iter().map(|a| Line::Row(m, a)));
        }
        if !out.is_empty() {
            return (out, false);
        }
    }
    for g in Group::ALL {
        let acts: Vec<Action> = actions(km, mode).filter(|a| a.group() == g).collect();
        if acts.is_empty() {
            continue;
        }
        out.push(Line::Heading(tr(g.message_id())));
        out.extend(acts.into_iter().map(|a| Line::Row(mode, a)));
    }
    (out, !q.is_empty())
}

fn list_rows(ch: &Chrome, st: &ViewerState) -> Vec<(Mode, Action)> {
    lines(st.keymap, ch.settings.keys.mode, &ch.settings.search)
        .0
        .into_iter()
        .filter_map(|l| match l {
            Line::Row(m, a) => Some((m, a)),
            Line::Heading(_) => None,
        })
        .collect()
}

/// The row a part of the list belongs to.
fn row_of(ui: &KeysUi, s: &S) -> Option<(Mode, Action)> {
    match s {
        S::KeyRow(m, a)
        | S::KeyChip(m, a, _)
        | S::KeyRemove(m, a, _)
        | S::KeyAdd(m, a)
        | S::KeyResetAction(m, a) => Some((*m, *a)),
        S::KeyChoice(_) | S::KeyPromptText(_) => ui.prompt.as_ref().map(|p| (p.mode, p.action)),
        S::KeyRecordCancel => ui.recording.as_ref().map(|r| (r.mode, r.action)),
        _ => None,
    }
}

/// The parts of row (`m`, `a`) the keyboard goes through with ←/→, in reading order.
fn parts(ui: &KeysUi, km: &Keymap, m: Mode, a: Action) -> Vec<S> {
    let mut v = vec![S::KeyRow(m, a)];
    if ui
        .recording
        .as_ref()
        .is_some_and(|r| r.mode == m && r.action == a)
    {
        v.push(S::KeyRecordCancel);
        return v;
    }
    let n = km.keys_in(m, a).len();
    v.extend((0..n).map(|i| S::KeyChip(m, a, i as u8)));
    v.push(S::KeyAdd(m, a));
    if km.action_changed(m, a) {
        v.push(S::KeyResetAction(m, a));
    }
    if let Some(p) = &ui.prompt
        && p.mode == m
        && p.action == a
    {
        v.extend((0..p.choices.len()).map(|i| S::KeyChoice(i as u8)));
    }
    v
}

/// The Tab stops of the editor, in order: the modes, presets and buttons (not while
/// searching), the confirmation's buttons, then the list as one stop (the part the keyboard
/// was last on).
pub(crate) fn stops(ch: &Chrome, st: &ViewerState) -> Vec<S> {
    let ui = &ch.settings.keys;
    let km = st.keymap;
    let mut v = Vec::new();
    if ch.settings.search.trim().is_empty() {
        v.extend(modes(km).into_iter().map(S::KeyMode));
        v.extend(Preset::ALL.into_iter().map(S::KeyPreset));
        v.extend([S::KeyImport, S::KeyExport, S::KeyExportMode, S::KeyEditFile]);
        if km.mode_changed(ui.mode) {
            v.push(S::KeyResetMode);
        }
        if km.changed() {
            v.push(S::KeyResetAll);
        }
        if ui.confirm.is_some() {
            v.extend([S::KeyCancel, S::KeyConfirm]);
        }
    }
    let rows = list_rows(ch, st);
    let cur = match &ch.modal_focus {
        Some(Target::Settings(s)) if is_list(s) => row_of(ui, s)
            .filter(|r| rows.contains(r))
            .map(|_| s.clone()),
        _ => None,
    };
    if let Some(s) = cur.or_else(|| rows.first().map(|(m, a)| S::KeyRow(*m, *a))) {
        v.push(s);
    }
    v
}

// ----- Layout -------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    /// A mode (selected).
    Tab(bool),
    /// A preset (the current one).
    Radio(bool),
    Button,
    /// The default button of a confirmation or a conflict.
    Primary,
    /// A key of an action, with its remove button.
    Chip,
    Icon(Icon),
    /// The keys being recorded.
    Pill,
    Text(Ink),
    Heading,
    /// A row of the list, with its label.
    Row,
    ListBox,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ink {
    Text,
    Muted,
    Accent,
}

struct Item {
    /// Left to right; mirrored when drawn.
    r: Rect,
    b: Option<Arc<TextBlock>>,
    t: Option<S>,
    look: Look,
    /// A row's label, a chip's remove button.
    sub: Option<(Rect, Option<S>)>,
}

impl Item {
    fn new(r: Rect, b: Option<Arc<TextBlock>>, t: Option<S>, look: Look) -> Item {
        Item {
            r,
            b,
            t,
            look,
            sub: None,
        }
    }
}

/// Places boxes one after the other, wrapping within a width.
struct Flow {
    x0: f32,
    w: f32,
    x: f32,
    y: f32,
    h: f32,
    gap: f32,
}

impl Flow {
    fn new(x0: f32, y: f32, w: f32, h: f32, gap: f32) -> Flow {
        Flow {
            x0,
            w,
            x: x0,
            y,
            h,
            gap,
        }
    }

    fn place(&mut self, w: f32) -> Rect {
        let w = w.min(self.w);
        if self.x > self.x0 && self.x + w > self.x0 + self.w {
            self.x = self.x0;
            self.y += self.h + self.gap;
        }
        let r = Rect::new(self.x, self.y, w, self.h);
        self.x += w + self.gap;
        r
    }

    fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// A button's label, shortened to fit `max_w` with its padding.
fn button_text(cx: &mut Ctx, s: &str, max_w: f32) -> Arc<TextBlock> {
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    let s = cx.ellipsize(s, style, (max_w - 24.0).max(0.0), false);
    cx.layout(&s, style, None)
}

/// The editor laid out from `o`, `w` wide: its items, and its height.
fn layout(ch: &Chrome, cx: &mut Ctx, st: &ViewerState, o: (f32, f32), w: f32) -> (Vec<Item>, f32) {
    let ui = &ch.settings.keys;
    let km = st.keymap;
    let mut items = Vec::new();
    let (x0, mut y) = o;
    let searching = !ch.settings.search.trim().is_empty();
    let small = cx.style(metrics::FONT_SMALL);
    let bh = cx.u(28.0);
    if !searching {
        // The modes.
        let mut f = Flow::new(x0, y, w, bh, 4.0);
        for m in modes(km) {
            let b = button_text(cx, &mode_name(m), w);
            let r = f.place(b.width + 28.0);
            items.push(Item::new(
                r,
                Some(b),
                Some(S::KeyMode(m)),
                Look::Tab(m == listed_mode(km, ui.mode)),
            ));
        }
        y = f.bottom() + 12.0;
        // The presets, after their label.
        let ch_h = cx.u(26.0);
        let lb = cx.layout(&t!("keys-presets"), small.weight(600.0), None);
        let mut f = Flow::new(x0, y, w, ch_h, 6.0);
        let lr = f.place(lb.width + 4.0);
        items.push(Item::new(lr, Some(lb), None, Look::Text(Ink::Muted)));
        for p in Preset::ALL {
            let b = cx.layout(&tr(p.message_id()), small, None);
            let r = f.place(b.width + 20.0 + 16.0);
            items.push(Item::new(
                r,
                Some(b),
                Some(S::KeyPreset(p)),
                Look::Radio(km.preset() == p),
            ));
        }
        y = f.bottom() + 10.0;
        // Import, export, resets.
        let mut f = Flow::new(x0, y, w, bh, 8.0);
        let mode = mode_name(ui.mode);
        let mut buttons = vec![
            (t!("keys-import"), S::KeyImport),
            (t!("keys-export"), S::KeyExport),
            (
                t!("keys-export-mode", mode = mode.clone()),
                S::KeyExportMode,
            ),
            (t!("keys-edit-file"), S::KeyEditFile),
        ];
        if km.mode_changed(ui.mode) {
            buttons.push((t!("keys-reset-mode", mode = mode), S::KeyResetMode));
        }
        if km.changed() {
            buttons.push((t!("keys-reset-all"), S::KeyResetAll));
        }
        for (label, t) in buttons {
            let b = button_text(cx, &label, w);
            let r = f.place(b.width + 24.0);
            items.push(Item::new(r, Some(b), Some(t), Look::Button));
        }
        y = f.bottom() + 10.0;
        // A confirmation.
        if let Some(c) = ui.confirm {
            let (text, ok) = match c {
                Confirm::Preset(p) => (
                    t!("keys-confirm-preset", preset = tr(p.message_id())),
                    t!("keys-confirm-ok"),
                ),
                Confirm::ResetMode(m) => (
                    t!("keys-confirm-reset-mode", mode = mode_name(m)),
                    t!("keys-confirm-reset"),
                ),
                Confirm::ResetAll => (t!("keys-confirm-reset-all"), t!("keys-confirm-reset")),
            };
            let b = cx.layout(&text, cx.style(metrics::FONT_BODY), Some(w));
            items.push(Item::new(
                Rect::new(x0, y, w, b.height),
                Some(b.clone()),
                Some(S::KeyConfirmText),
                Look::Text(Ink::Text),
            ));
            y += b.height + 8.0;
            let mut f = Flow::new(x0, y, w, bh, 8.0);
            let cb = button_text(cx, &t!("keys-confirm-cancel"), w);
            let r = f.place(cb.width + 24.0);
            items.push(Item::new(r, Some(cb), Some(S::KeyCancel), Look::Button));
            let ob = button_text(cx, &ok, w);
            let r = f.place(ob.width + 24.0);
            items.push(Item::new(r, Some(ob), Some(S::KeyConfirm), Look::Primary));
            y = f.bottom() + 10.0;
        }
        // What loading keymap.toml found wrong.
        for (i, p) in km.problems().iter().take(MAX_PROBLEMS).enumerate() {
            let b = cx.layout(&problem_text(p), small, Some(w));
            items.push(Item::new(
                Rect::new(x0, y, w, b.height),
                Some(b.clone()),
                Some(S::KeyProblem(i as u8)),
                Look::Text(Ink::Accent),
            ));
            y += b.height + 4.0;
        }
        if !km.problems().is_empty() {
            y += 6.0;
        }
    }

    // The list.
    let (lines, nothing) = lines(km, ui.mode, &ch.settings.search);
    if nothing {
        let q = specular_i18n::isolate(ch.settings.search.trim());
        let b = cx.layout(&t!("keys-no-match", query = q), small, Some(w));
        items.push(Item::new(
            Rect::new(x0, y, w, b.height),
            Some(b.clone()),
            Some(S::KeyNoMatch),
            Look::Text(Ink::Muted),
        ));
        y += b.height + 8.0;
    }
    let box_i = items.len();
    items.push(Item::new(
        Rect::new(x0, y, w, 0.0),
        None,
        None,
        Look::ListBox,
    ));
    let top = y;
    y += 1.0;
    let pad = 12.0;
    let inner = w - 2.0;
    let lw = (inner * 0.42).round();
    let s = cx.u(28.0);
    let min_h = cx.u(34.0);
    let chip_h = cx.u(26.0);
    let kx = x0 + 1.0 + pad + lw + 12.0;
    let kend = x0 + w - 1.0 - pad - s - 4.0;
    let kw = (kend - kx).max(chip_h);
    let body = cx.style(metrics::FONT_BODY);
    let head_s = small.weight(600.0);
    let mut first = true;
    for line in lines {
        match line {
            Line::Heading(text) => {
                let hh = cx.u(28.0);
                let b = cx.layout(&text, head_s, None);
                let r = Rect::new(x0 + 1.0, y, inner, hh);
                let mut it = Item::new(r, Some(b), None, Look::Heading);
                // The separator above, but at the top of the list.
                it.sub = (!first).then_some((Rect::new(x0 + 1.0, y, inner, 1.0), None));
                items.push(it);
                y += hh;
            }
            Line::Row(m, a) => {
                let lb = cx.layout(&action_name(a), body, Some(lw));
                let row_i = items.len();
                items.push(Item::new(
                    Rect::new(x0 + 1.0, y, inner, min_h),
                    Some(lb.clone()),
                    Some(S::KeyRow(m, a)),
                    Look::Row,
                ));
                let ctop = y + ((min_h - chip_h) * 0.5).floor();
                let mut f = Flow::new(kx, ctop, kw, chip_h, 6.0);
                let mut bottom;
                let rec = ui
                    .recording
                    .as_ref()
                    .filter(|r| r.mode == m && r.action == a);
                if let Some(rec) = rec {
                    let keys = rec.rec.keys();
                    let text = if keys.is_empty() {
                        t!("keys-press")
                    } else {
                        combos_text(keys)
                    };
                    let cb = button_text(cx, &t!("keys-cancel-recording"), kw);
                    let cw = cb.width + 24.0;
                    let pb = cx.layout(&text, small.weight(600.0), None);
                    let pr = f.place((kw - cw - 6.0).max(pb.width + 20.0));
                    items.push(Item::new(pr, Some(pb), None, Look::Pill));
                    let mut cf = Flow::new(kx, f.y, kw, chip_h, 6.0);
                    cf.x = f.x;
                    let cr = cf.place(cw);
                    items.push(Item::new(
                        cr,
                        Some(cb),
                        Some(S::KeyRecordCancel),
                        Look::Button,
                    ));
                    bottom = cf.bottom();
                    let hb = cx.layout(&t!("keys-recording-hint"), small, Some(kw));
                    items.push(Item::new(
                        Rect::new(kx, bottom + 4.0, kw, hb.height),
                        Some(hb.clone()),
                        None,
                        Look::Text(Ink::Muted),
                    ));
                    bottom += 4.0 + hb.height;
                } else {
                    for (i, k) in km.keys_in(m, a).iter().enumerate() {
                        let b = cx.layout(&k.to_string(), small, None);
                        let r = f.place(b.width + 14.0 + 22.0);
                        let x = Rect::new(r.right() - 24.0, r.y, 24.0, r.h);
                        let mut it =
                            Item::new(r, Some(b), Some(S::KeyChip(m, a, i as u8)), Look::Chip);
                        it.sub = Some((x, Some(S::KeyRemove(m, a, i as u8))));
                        items.push(it);
                    }
                    let r = f.place(chip_h);
                    items.push(Item::new(
                        r,
                        None,
                        Some(S::KeyAdd(m, a)),
                        Look::Icon(Icon::Plus),
                    ));
                    bottom = f.bottom();
                }
                if km.action_changed(m, a) {
                    let r = Rect::new(
                        x0 + w - 1.0 - pad - s,
                        y + ((min_h - s) * 0.5).floor(),
                        s,
                        s,
                    );
                    items.push(Item::new(
                        r,
                        None,
                        Some(S::KeyResetAction(m, a)),
                        Look::Icon(Icon::Undo),
                    ));
                }
                let mut rh = min_h
                    .max(lb.height + 12.0)
                    .max(bottom - y + (min_h - chip_h) * 0.5);
                // A conflict of this row: what it is, and the answers.
                if let Some(p) = ui.prompt.as_ref().filter(|p| p.mode == m && p.action == a) {
                    let mut py = y + rh;
                    let tw = inner - pad * 2.0;
                    for (i, c) in p.conflicts.iter().enumerate() {
                        let text = conflict_text(c, &p.keys);
                        if text.is_empty() {
                            continue;
                        }
                        let b = cx.layout(&text, small, Some(tw));
                        items.push(Item::new(
                            Rect::new(x0 + 1.0 + pad, py, tw, b.height),
                            Some(b.clone()),
                            Some(S::KeyPromptText(i as u8)),
                            Look::Text(Ink::Accent),
                        ));
                        py += b.height + 3.0;
                    }
                    let mut f = Flow::new(x0 + 1.0 + pad, py + 5.0, tw, bh, 8.0);
                    for (i, c) in p.choices.iter().enumerate() {
                        let b = button_text(cx, &c.label(), tw);
                        let r = f.place(b.width + 24.0);
                        let look = if i == 0 { Look::Primary } else { Look::Button };
                        items.push(Item::new(r, Some(b), Some(S::KeyChoice(i as u8)), look));
                    }
                    rh = f.bottom() + 10.0 - y;
                }
                let lr = Rect::new(x0 + 1.0 + pad, y, lw, rh.min(min_h.max(lb.height + 12.0)));
                let row = &mut items[row_i];
                row.r.h = rh;
                row.sub = Some((lr, None));
                y += rh;
            }
        }
        first = false;
    }
    y += 1.0;
    items[box_i].r.h = y - top;
    (items, y - o.1)
}

/// Height of the editor for a row `width` wide.
pub(crate) fn editor_height(ch: &Chrome, cx: &mut Ctx, st: &ViewerState, width: f32) -> f32 {
    layout(ch, cx, st, (0.0, 0.0), width).1
}

/// Draws the editor at `r` (its full width, from its top). Only what is inside `clip` is
/// drawn and can be hit; `y0` is the top of the content (for scrolling into view).
pub(crate) fn draw_editor(
    ch: &mut Chrome,
    cx: &mut Ctx,
    st: &ViewerState,
    r: Rect,
    clip: Rect,
    y0: f32,
) {
    let (items, _) = layout(ch, cx, st, (r.x, r.y), r.w);
    let ui = &mut ch.settings.keys;
    ui.on_page = true;
    ui.spans.clear();
    for it in &items {
        if let Some(t) = &it.t {
            ui.spans.push((t.clone(), it.r.y - y0, it.r.h));
        }
        if let Some((sr, Some(t))) = &it.sub {
            ui.spans.push((t.clone(), sr.y - y0, sr.h));
        }
    }
    for it in &items {
        let rr = cx.m_in(it.r, r);
        if rr.bottom() < clip.y || rr.y > clip.bottom() {
            continue;
        }
        draw_item(cx, it, rr, r);
    }
}

fn ink(cx: &Ctx, i: Ink) -> Color {
    match i {
        Ink::Text => cx.pal.text,
        Ink::Muted => cx.pal.text_muted,
        Ink::Accent => cx.pal.accent,
    }
}

fn ring(cx: &mut Ctx, t: &Option<S>, r: Rect, radius: f32) {
    if let Some(t) = t
        && cx.is_focused(&Target::Settings(t.clone()))
    {
        cx.focus_ring(r, radius);
    }
}

fn draw_item(cx: &mut Ctx, it: &Item, rr: Rect, er: Rect) {
    let tgt = it.t.clone().map(Target::Settings);
    let hover = tgt.as_ref().is_some_and(|t| cx.is_hover(t));
    let pressed = tgt.as_ref().is_some_and(|t| cx.is_pressed(t));
    match it.look {
        Look::Tab(_) | Look::Button | Look::Primary => {
            let on = it.look == Look::Tab(true);
            let primary = it.look == Look::Primary;
            let fill = match it.look {
                Look::Tab(true) => cx.pal.accent_soft,
                Look::Primary => cx.pal.accent_fill,
                _ if pressed => cx.pal.surface_pressed,
                _ if hover => cx.pal.surface_hover,
                _ => cx.pal.surface_raised,
            };
            let border = if primary || on {
                Color::TRANSPARENT
            } else {
                cx.pal.separator
            };
            cx.dl
                .rect_border(rr, metrics::RADIUS_SM + 1.0, fill, border, 1.0);
            if primary && hover {
                cx.dl
                    .rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.surface_hover);
            }
            let c = if primary {
                cx.pal.accent_text
            } else {
                cx.pal.text
            };
            if let Some(b) = &it.b {
                cx.dl.push_clip(rr);
                cx.block_in(b, rr, HAlign::Center, c);
                cx.dl.pop_clip();
            }
            ring(cx, &it.t, rr.inset(-3.0), metrics::RADIUS_SM + 4.0);
        }
        Look::Radio(on) => {
            let radius = rr.h * 0.5;
            if on {
                cx.dl.rect(rr, radius, cx.pal.accent_soft);
            } else {
                cx.dl.rect_border(
                    rr,
                    radius,
                    Color::TRANSPARENT,
                    cx.pal.text_faint.with_alpha(0.6),
                    1.0,
                );
            }
            if hover {
                cx.dl.rect(rr, radius, cx.pal.surface_hover);
            }
            let (icon_r, text_r) = if cx.rtl {
                (
                    Rect::new(rr.right() - 24.0, rr.y, 16.0, rr.h),
                    Rect::new(rr.x + 10.0, rr.y, rr.w - 34.0, rr.h),
                )
            } else {
                (
                    Rect::new(rr.x + 8.0, rr.y, 16.0, rr.h),
                    Rect::new(rr.x + 24.0, rr.y, rr.w - 34.0, rr.h),
                )
            };
            if on {
                draw_icon(
                    cx.dl,
                    Icon::Check,
                    icon_r.centered(13.0, 13.0),
                    cx.pal.accent,
                    1.8,
                    false,
                );
            }
            if let Some(b) = &it.b {
                // Without its check the name is centred (the chip keeps its width).
                let (c, tr) = if on {
                    (cx.pal.text, text_r)
                } else {
                    (cx.pal.text_muted, rr)
                };
                cx.block_in(b, tr, HAlign::Center, c);
            }
            ring(cx, &it.t, rr.inset(-3.0), radius + 3.0);
        }
        Look::Chip => {
            let fill = if hover {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_pressed
            };
            cx.dl.rect(rr, metrics::RADIUS_SM, fill);
            let (xr, xt) = match &it.sub {
                Some((x, t)) => (cx.m_in(*x, er), t.clone().map(Target::Settings)),
                None => (Rect::new(rr.x, rr.y, 0.0, 0.0), None),
            };
            let tr = if cx.rtl {
                Rect::new(xr.right(), rr.y, rr.right() - xr.right() - 8.0, rr.h)
            } else {
                Rect::new(rr.x + 8.0, rr.y, xr.x - rr.x - 8.0, rr.h)
            };
            if let Some(b) = &it.b {
                cx.block_in(b, tr, HAlign::Center, cx.pal.text);
            }
            if let Some(t) = &tgt {
                cx.hit(rr, t.clone());
            }
            let xh = xt.as_ref().is_some_and(|t| cx.is_hover(t));
            if xh {
                cx.dl.circle(
                    xr.center(),
                    (xr.h * 0.5 - 4.0).min(9.0),
                    cx.pal.surface_pressed,
                );
            }
            draw_icon(
                cx.dl,
                Icon::Close,
                xr.centered(9.0, 9.0),
                if xh { cx.pal.text } else { cx.pal.text_muted },
                1.5,
                false,
            );
            if let Some(t) = xt {
                cx.hit(xr, t);
            }
            ring(cx, &it.t, rr.inset(-2.0), metrics::RADIUS_SM + 2.0);
            return;
        }
        Look::Icon(icon) => {
            if let Some(t) = tgt {
                // `icon_button` draws its own focus ring.
                cx.icon_button(
                    rr,
                    icon,
                    t,
                    cx.pal.text_muted,
                    icon == Icon::Undo && cx.rtl,
                    false,
                );
            }
            return;
        }
        Look::Pill => {
            cx.dl.rect_border(
                rr,
                metrics::RADIUS_SM,
                cx.pal.accent_soft,
                cx.pal.accent,
                1.5,
            );
            if let Some(b) = &it.b {
                cx.dl.push_clip(rr);
                cx.block_in(b, rr.inset_xy(10.0, 0.0), HAlign::Start, cx.pal.text);
                cx.dl.pop_clip();
            }
        }
        Look::Text(i) => {
            if let Some(b) = &it.b {
                let c = ink(cx, i);
                if it.r.h <= b.height + 0.5 {
                    cx.block_top(b, rr, HAlign::Start, c);
                } else {
                    cx.block_in(b, rr, HAlign::Start, c);
                }
            }
        }
        Look::Heading => {
            if let Some((sr, _)) = &it.sub {
                cx.dl.rect(cx.m_in(*sr, er), 0.0, cx.pal.separator);
            }
            if let Some(b) = &it.b {
                cx.block_in(b, rr.inset_xy(12.0, 0.0), HAlign::Start, cx.pal.text_muted);
            }
        }
        Look::Row => {
            if hover {
                cx.dl
                    .rect(rr.inset(1.0), metrics::RADIUS_SM, cx.pal.surface_hover);
            }
            if let (Some(b), Some((lr, _))) = (&it.b, &it.sub) {
                let lr = cx.m_in(*lr, er);
                cx.block_in(b, lr, HAlign::Start, cx.pal.text);
            }
            if let Some(t) = &tgt {
                cx.hit(rr, t.clone());
            }
            ring(cx, &it.t, rr.inset(2.0), metrics::RADIUS_SM);
            return;
        }
        Look::ListBox => {
            cx.dl.rect_border(
                rr,
                metrics::RADIUS_MD,
                cx.pal.surface_raised,
                cx.pal.separator,
                1.0,
            );
        }
    }
    if let Some(t) = tgt {
        cx.hit(rr, t);
    }
}

// ----- Interaction ------------------------------------------------------------------

impl Chrome {
    /// Puts the keyboard on part `s` of the editor and brings it into view.
    fn keys_focus(&mut self, s: S) {
        self.modal_focus = Some(Target::Settings(s));
        self.reveal_focus = true;
    }

    /// Whether keys are being recorded (the application then sends every key release).
    pub fn recording_keys(&self) -> bool {
        self.settings.keys.recording.is_some()
    }

    /// Records keys for `action` in `mode` (in place of `old`); the keyboard comes back to
    /// `from` when done.
    fn keys_record(
        &mut self,
        st: &ViewerState,
        mode: Mode,
        action: Action,
        old: Option<KeySeq>,
        from: S,
    ) {
        let ui = &mut self.settings.keys;
        ui.prompt = None;
        ui.confirm = None;
        ui.recording = Some(Recording {
            mode,
            action,
            old,
            from,
            rec: Recorder::new(st.settings.keys.sequence_timeout, self.last_now),
        });
        self.keys_focus(S::KeyRecordCancel);
        self.announce(t!("keys-announce-recording", action = action_name(action)));
    }

    /// Stops recording without binding anything.
    pub(crate) fn keys_cancel_recording(&mut self) {
        if let Some(r) = self.settings.keys.recording.take() {
            if matches!(
                &self.modal_focus,
                Some(Target::Settings(S::KeyRecordCancel))
            ) {
                self.keys_focus(r.from);
            }
            self.announce(t!("keys-announce-cancelled"));
        }
    }

    /// What the recorder said: keys so far, cancelled, or the keys to bind (checked for
    /// conflicts: free keys are bound at once, others ask how).
    fn keys_record_event(&mut self, ev: RecordEvent, st: &ViewerState) -> Option<Command> {
        match ev {
            RecordEvent::None => None,
            RecordEvent::Updated => {
                let keys = self.settings.keys.recording.as_ref()?.rec.keys().to_vec();
                if !keys.is_empty() {
                    self.announce(t!("keys-announce-key", keys = combos_text(&keys)));
                }
                None
            }
            RecordEvent::Cancelled => {
                self.keys_cancel_recording();
                None
            }
            RecordEvent::Done(keys) => {
                let r = self.settings.keys.recording.take()?;
                self.keys_focus(r.from.clone());
                let cs = conflicts(st.keymap, r.mode, keys, r.action, r.old);
                if cs.is_empty() {
                    self.announce(t!(
                        "keys-announce-bound",
                        keys = keys_text(&keys),
                        action = action_name(r.action)
                    ));
                    return Some(Command::Keymap(KeymapEdit::Bind {
                        mode: r.mode,
                        keys,
                        action: r.action,
                        old: r.old,
                        displace: Displace::Keep,
                    }));
                }
                if cs.contains(&Conflict::Already) {
                    self.announce(t!(
                        "keys-announce-already",
                        keys = keys_text(&keys),
                        action = action_name(r.action)
                    ));
                    return None;
                }
                let text: Vec<String> = cs
                    .iter()
                    .map(|c| conflict_text(c, &keys))
                    .filter(|s| !s.is_empty())
                    .collect();
                let choices = choices_for(&cs, r.old.is_some());
                self.settings.keys.prompt = Some(Prompt {
                    mode: r.mode,
                    action: r.action,
                    keys,
                    old: r.old,
                    from: r.from,
                    conflicts: cs,
                    choices,
                });
                self.keys_focus(S::KeyChoice(0));
                self.announce(text.join(" "));
                None
            }
        }
    }

    /// A key pressed while recording: it is recorded, whatever it is.
    pub(crate) fn keys_record_press(
        &mut self,
        combo: &KeyCombo,
        repeat: bool,
        st: &ViewerState,
        now: f64,
    ) -> Response {
        let Some(r) = self.settings.keys.recording.as_mut() else {
            return Response::default();
        };
        let ev = r.rec.press(*combo, repeat, now);
        match self.keys_record_event(ev, st) {
            Some(c) => Response::cmd(c),
            None => Response::used(),
        }
    }

    /// A key released while recording (a short Esc is a key, a long one cancels).
    pub(crate) fn keys_record_release(
        &mut self,
        combo: &KeyCombo,
        st: &ViewerState,
        now: f64,
    ) -> Response {
        let Some(r) = self.settings.keys.recording.as_mut() else {
            return Response::default();
        };
        let ev = r.rec.release(*combo, now);
        match self.keys_record_event(ev, st) {
            Some(c) => Response::cmd(c),
            None => Response::used(),
        }
    }

    /// Each frame: a recording ends after its timeout (or with its dialog, or its page).
    /// Returns how long until it needs to look again.
    pub(crate) fn keys_tick(&mut self, st: &ViewerState, now: f64) -> Option<f64> {
        self.settings.keys.recording.as_ref()?;
        if self.modal != Some(Modal::Settings) {
            self.settings.keys.recording = None;
            return None;
        }
        let ev = self.settings.keys.recording.as_mut()?.rec.tick(now);
        if let Some(c) = self.keys_record_event(ev, st) {
            self.pending.push(c);
        }
        let d = self.settings.keys.recording.as_ref()?.rec.deadline()?;
        Some((d - now).max(0.0))
    }

    /// Esc (not while recording, which takes it: a tap is recorded as Esc, a hold cancels):
    /// closes a conflict or a confirmation. Going back to the page Keys was reached from is
    /// the Settings history's step ([`Chrome::settings_back`]), so Esc never steps out of
    /// Keys and back into it in turn.
    pub(crate) fn keys_back(&mut self) -> bool {
        let ui = &mut self.settings.keys;
        if let Some(p) = ui.prompt.take() {
            self.keys_focus(p.from);
            return true;
        }
        if let Some(c) = ui.confirm.take() {
            if self.modal_focus.is_some() {
                self.keys_focus(confirm_source(c));
            }
            return true;
        }
        false
    }

    /// One step back in Settings from the Keys page: to the page it was reached from, its
    /// section and scroll as they were, the keyboard (if it was in Settings) on that
    /// section in the sidebar. False when Keys was opened directly (Customise keys…) or is
    /// shown as a search result. The Settings history calls this as its step for Keys when
    /// it has none recorded ([`Chrome::settings_back`]): `from` is the page last drawn
    /// before Keys, not how Keys was reached, so it never goes before the history.
    pub(crate) fn keys_return(&mut self) -> bool {
        let s = &mut self.settings;
        if !s.keys.on_page || !s.search.is_empty() {
            return false;
        }
        let Some((section, scroll)) = s.keys.from.take() else {
            return false;
        };
        if section == s.section {
            return false;
        }
        s.section = section;
        s.scroll = scroll;
        s.scroll_to = None;
        s.start = None;
        s.keys.recording = None;
        if self.modal_focus.is_some() {
            self.modal_focus = Some(Target::Settings(S::Section(section)));
        }
        true
    }

    /// Runs an edit and announces it.
    fn keys_edit(&mut self, e: KeymapEdit, said: String) -> Response {
        self.announce(said);
        Response::cmd(Command::Keymap(e))
    }

    /// A click (or Enter or Space) on part `s` of the editor.
    pub(crate) fn keys_click(&mut self, s: S, st: &ViewerState) -> Response {
        let km = st.keymap;
        let kbd = self.modal_focus.is_some();
        if s != S::KeyRecordCancel && self.settings.keys.recording.is_some() {
            self.keys_cancel_recording();
        }
        let mode = self.settings.keys.mode;
        match s {
            S::KeyMode(m) => {
                let ui = &mut self.settings.keys;
                ui.mode = m;
                ui.prompt = None;
                ui.confirm = None;
            }
            S::KeyPreset(p) => {
                self.settings.keys.prompt = None;
                if km.changed() {
                    self.settings.keys.confirm = Some(Confirm::Preset(p));
                    if kbd {
                        self.keys_focus(S::KeyCancel);
                    }
                } else if km.preset() != p {
                    self.settings.keys.confirm = None;
                    let said = t!("keys-announce-preset", preset = tr(p.message_id()));
                    return self.keys_edit(KeymapEdit::Preset(p), said);
                }
            }
            S::KeyImport => return Response::cmd(Command::KeymapImport),
            S::KeyExport => return Response::cmd(Command::KeymapExport(None)),
            S::KeyExportMode => return Response::cmd(Command::KeymapExport(Some(mode))),
            S::KeyEditFile => self.open_keymap_editor(km),
            S::KeyResetMode | S::KeyResetAll => {
                self.settings.keys.prompt = None;
                self.settings.keys.confirm = Some(if s == S::KeyResetMode {
                    Confirm::ResetMode(mode)
                } else {
                    Confirm::ResetAll
                });
                if kbd {
                    self.keys_focus(S::KeyCancel);
                }
            }
            S::KeyConfirm => {
                let Some(c) = self.settings.keys.confirm.take() else {
                    return Response::used();
                };
                if kbd {
                    self.keys_focus(match c {
                        Confirm::Preset(p) => S::KeyPreset(p),
                        _ => S::KeyExportMode,
                    });
                }
                let (e, said) = match c {
                    Confirm::Preset(p) => (
                        KeymapEdit::Preset(p),
                        t!("keys-announce-preset", preset = tr(p.message_id())),
                    ),
                    Confirm::ResetMode(m) => (
                        KeymapEdit::ResetMode(m),
                        t!("keys-announce-reset-mode", mode = mode_name(m)),
                    ),
                    Confirm::ResetAll => (KeymapEdit::ResetAll, t!("keys-announce-reset-all")),
                };
                return self.keys_edit(e, said);
            }
            S::KeyCancel => {
                if let Some(c) = self.settings.keys.confirm.take()
                    && kbd
                {
                    self.keys_focus(confirm_source(c));
                }
            }
            S::KeyRow(m, a) => {
                // A click puts the keyboard on the row; Enter or Space records (keys_key).
                self.keys_focus(S::KeyRow(m, a));
            }
            S::KeyChip(m, a, i) => {
                if let Some(k) = km.keys_in(m, a).get(i as usize) {
                    self.keys_record(st, m, a, Some(*k), S::KeyChip(m, a, i));
                }
            }
            S::KeyRemove(m, a, i) => {
                if let Some(k) = km.keys_in(m, a).get(i as usize).copied() {
                    self.settings.keys.prompt = None;
                    let said = t!(
                        "keys-announce-removed",
                        keys = keys_text(&k),
                        action = action_name(a)
                    );
                    return self.keys_edit(KeymapEdit::Unbind { mode: m, keys: k }, said);
                }
            }
            S::KeyAdd(m, a) => self.keys_record(st, m, a, None, S::KeyAdd(m, a)),
            S::KeyResetAction(m, a) => {
                if kbd {
                    self.keys_focus(S::KeyRow(m, a));
                }
                let said = t!("keys-announce-reset-action", action = action_name(a));
                return self.keys_edit(KeymapEdit::ResetAction { mode: m, action: a }, said);
            }
            S::KeyChoice(i) => return self.keys_choose(i as usize),
            S::KeyRecordCancel => self.keys_cancel_recording(),
            _ => {}
        }
        Response::used()
    }

    /// Answer `i` to the open conflict.
    fn keys_choose(&mut self, i: usize) -> Response {
        let Some(p) = self.settings.keys.prompt.take() else {
            return Response::used();
        };
        self.keys_focus(p.from.clone());
        let displace = match p.choices.get(i) {
            Some(Choice::Replace) => Displace::Replace,
            Some(Choice::Swap) => Displace::Swap,
            Some(Choice::Keep | Choice::Anyway) => Displace::Keep,
            Some(Choice::Cancel) | None => return Response::used(),
        };
        let said = t!(
            "keys-announce-bound",
            keys = keys_text(&p.keys),
            action = action_name(p.action)
        );
        self.keys_edit(
            KeymapEdit::Bind {
                mode: p.mode,
                keys: p.keys,
                action: p.action,
                old: p.old,
                displace,
            },
            said,
        )
    }

    /// Keys while a part of the list has the keyboard. `None`: not the list's (Backspace
    /// edits the search field, typing goes there too).
    pub(crate) fn keys_list_key(
        &mut self,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Option<Response> {
        let Some(Target::Settings(f)) = self.modal_focus.clone() else {
            return None;
        };
        let km = st.keymap;
        let rows = list_rows(self, st);
        let Some((m, a)) = row_of(&self.settings.keys, &f).filter(|r| rows.contains(r)) else {
            // Its row is gone (the search changed): back to the list's first row.
            let first = rows.first()?;
            self.keys_focus(S::KeyRow(first.0, first.1));
            return Some(Response::used());
        };
        let ri = rows.iter().position(|r| *r == (m, a)).unwrap_or(0);
        let ps = parts(&self.settings.keys, km, m, a);
        let pi = ps.iter().position(|p| *p == f).unwrap_or(0);
        let named = named?;
        let rtl = self.last_rtl;
        let row = |i: usize| S::KeyRow(rows[i].0, rows[i].1);
        match named {
            NamedKey::ArrowUp => self.keys_focus(row(ri.saturating_sub(1))),
            NamedKey::ArrowDown => self.keys_focus(row((ri + 1).min(rows.len() - 1))),
            NamedKey::Home => self.keys_focus(row(0)),
            NamedKey::End => self.keys_focus(row(rows.len() - 1)),
            NamedKey::PageUp => self.keys_focus(row(ri.saturating_sub(8))),
            NamedKey::PageDown => self.keys_focus(row((ri + 8).min(rows.len() - 1))),
            NamedKey::ArrowLeft | NamedKey::ArrowRight => {
                let fwd = (named == NamedKey::ArrowRight) != rtl;
                let to = if fwd {
                    (pi + 1).min(ps.len() - 1)
                } else {
                    pi.saturating_sub(1)
                };
                self.keys_focus(ps[to].clone());
            }
            NamedKey::Enter | NamedKey::Space => {
                if repeat {
                    return Some(Response::used());
                }
                return Some(match f {
                    S::KeyRow(m, a) => {
                        self.keys_record(st, m, a, None, S::KeyRow(m, a));
                        Response::used()
                    }
                    s => self.keys_click(s, st),
                });
            }
            NamedKey::Delete | NamedKey::Backspace if matches!(f, S::KeyChip(..)) => {
                let S::KeyChip(m, a, i) = f else {
                    return None;
                };
                let n = km.keys_in(m, a).len();
                let r = self.keys_click(S::KeyRemove(m, a, i), st);
                // The keyboard stays on the keys: the next one, else Add.
                self.keys_focus(if n > 1 {
                    S::KeyChip(m, a, (i as usize).min(n - 2) as u8)
                } else {
                    S::KeyAdd(m, a)
                });
                return Some(r);
            }
            NamedKey::Delete => {}
            _ => return None,
        }
        Some(Response::used())
    }
}

/// The button a confirmation came from.
fn confirm_source(c: Confirm) -> S {
    match c {
        Confirm::Preset(p) => S::KeyPreset(p),
        Confirm::ResetMode(_) => S::KeyResetMode,
        Confirm::ResetAll => S::KeyResetAll,
    }
}

// ----- Accessibility ----------------------------------------------------------------

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

/// Name, role and state of part `s` of the editor; the flag: it can be clicked.
pub(crate) fn describe(n: &mut Node, s: &S, ch: &Chrome, st: &ViewerState) -> Option<bool> {
    let ui = &ch.settings.keys;
    let km = st.keymap;
    let key = |m: Mode, a: Action, i: u8| km.keys_in(m, a).get(i as usize).copied();
    if is_text(s) {
        n.set_role(Role::Label);
    }
    match s {
        S::KeyMode(m) => {
            n.set_role(Role::Tab);
            n.set_label(mode_name(*m));
            n.set_selected(listed_mode(km, ui.mode) == *m);
        }
        S::KeyPreset(p) => {
            n.set_role(Role::RadioButton);
            n.set_label(tr(p.message_id()));
            n.set_description(t!("keys-presets"));
            n.set_toggled(toggled(km.preset() == *p));
        }
        S::KeyImport => n.set_label(t!("keys-import")),
        S::KeyExport => n.set_label(t!("keys-export")),
        S::KeyExportMode => n.set_label(t!("keys-export-mode", mode = mode_name(ui.mode))),
        S::KeyEditFile => n.set_label(t!("keys-edit-file")),
        S::KeyResetMode => n.set_label(t!("keys-reset-mode", mode = mode_name(ui.mode))),
        S::KeyResetAll => n.set_label(t!("keys-reset-all")),
        S::KeyConfirm | S::KeyCancel | S::KeyConfirmText => {
            let c = ui.confirm?;
            let text = match c {
                Confirm::Preset(p) => t!("keys-confirm-preset", preset = tr(p.message_id())),
                Confirm::ResetMode(m) => t!("keys-confirm-reset-mode", mode = mode_name(m)),
                Confirm::ResetAll => t!("keys-confirm-reset-all"),
            };
            match s {
                S::KeyConfirmText => n.set_label(text),
                S::KeyCancel => {
                    n.set_label(t!("keys-confirm-cancel"));
                    n.set_description(text);
                }
                _ => {
                    n.set_label(match c {
                        Confirm::Preset(_) => t!("keys-confirm-ok"),
                        _ => t!("keys-confirm-reset"),
                    });
                    n.set_description(text);
                }
            }
        }
        S::KeyProblem(i) => n.set_label(problem_text(km.problems().get(*i as usize)?)),
        S::KeyNoMatch => n.set_label(t!(
            "keys-no-match",
            query = specular_i18n::isolate(ch.settings.search.trim())
        )),
        S::KeyRow(m, a) => {
            n.set_role(Role::ListItem);
            n.set_label(action_name(*a));
            let rec = ui
                .recording
                .as_ref()
                .filter(|r| r.mode == *m && r.action == *a);
            let mut d = if rec.is_some() {
                t!("keys-recording-hint")
            } else {
                let keys: Vec<String> = km.keys_in(*m, *a).iter().map(keys_text).collect();
                if keys.is_empty() {
                    t!("keys-none")
                } else {
                    keys.join(", ")
                }
            };
            if km.action_changed(*m, *a) {
                d = format!("{d}; {}", t!("keys-changed"));
            }
            n.set_description(d);
            n.add_action(AkAction::Focus);
            return Some(false);
        }
        S::KeyChip(m, a, i) => {
            n.set_label(t!("keys-change", keys = keys_text(&key(*m, *a, *i)?)));
            n.set_description(action_name(*a));
        }
        S::KeyRemove(m, a, i) => {
            n.set_label(t!("keys-remove", keys = keys_text(&key(*m, *a, *i)?)));
            n.set_description(action_name(*a));
        }
        S::KeyAdd(_, a) => n.set_label(t!("keys-add", action = action_name(*a))),
        S::KeyResetAction(_, a) => n.set_label(t!("keys-reset-action", action = action_name(*a))),
        S::KeyChoice(i) => {
            let p = ui.prompt.as_ref()?;
            n.set_label(p.choices.get(*i as usize)?.label());
            let text: Vec<String> = p
                .conflicts
                .iter()
                .map(|c| conflict_text(c, &p.keys))
                .filter(|s| !s.is_empty())
                .collect();
            n.set_description(text.join(" "));
        }
        S::KeyPromptText(i) => {
            let p = ui.prompt.as_ref()?;
            n.set_label(conflict_text(p.conflicts.get(*i as usize)?, &p.keys));
        }
        S::KeyRecordCancel => {
            let r = ui.recording.as_ref()?;
            n.set_label(t!("keys-cancel-recording"));
            let keys = r.rec.keys();
            n.set_description(if keys.is_empty() {
                format!("{} {}", t!("keys-press"), t!("keys-recording-hint"))
            } else {
                format!("{} — {}", combos_text(keys), t!("keys-recording-hint"))
            });
        }
        _ => return None,
    }
    if is_list(s) {
        n.add_action(AkAction::Focus);
    }
    Some(!is_text(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(s: &str) -> KeySeq {
        KeySeq::parse(s).unwrap()
    }

    #[test]
    fn choices_follow_the_conflicts() {
        let same = Conflict::SameKey {
            other: Action::Quit,
        };
        let prefix = Conflict::Prefix {
            keys: seq("g g"),
            other: Action::FirstImage,
        };
        let shadow = Conflict::Shadow {
            mode: Mode::Viewer,
            other: Action::Quit,
        };
        use Choice::*;
        assert_eq!(choices_for(&[same], false), [Replace, Cancel]);
        assert_eq!(choices_for(&[same], true), [Replace, Swap, Cancel]);
        assert_eq!(choices_for(&[prefix], true), [Replace, Keep, Cancel]);
        assert_eq!(choices_for(&[same, prefix], false), [Replace, Cancel]);
        assert_eq!(choices_for(&[shadow], false), [Anyway, Cancel]);
        assert_eq!(
            choices_for(
                &[Conflict::Reserved(specular_input::Reserved::Desktop)],
                false
            ),
            [Anyway, Cancel]
        );
    }

    #[test]
    fn lines_by_group_and_search() {
        let km = Keymap::viewer_default();
        let (l, none) = lines(&km, Mode::Viewer, "");
        assert!(!none);
        assert!(matches!(l[0], Line::Heading(_)));
        let rows: Vec<_> = l
            .iter()
            .filter_map(|x| match x {
                Line::Row(m, a) => Some((*m, *a)),
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), Mode::Viewer.actions().count());
        assert!(rows.iter().all(|(m, _)| *m == Mode::Viewer));
        // Cull mode lists its own actions.
        let (l, _) = lines(&km, Mode::Cull, "");
        assert!(l.contains(&Line::Row(Mode::Cull, Action::CullPick)));
        assert!(!l.contains(&Line::Row(Mode::Cull, Action::Quit)));
        // A key finds its actions in every mode, by mode.
        let q = km.keys_in(Mode::Cull, Action::CullPick)[0].spec();
        let (l, none) = lines(&km, Mode::Viewer, &q);
        assert!(!none);
        assert!(
            l.contains(&Line::Row(Mode::Cull, Action::CullPick)),
            "{q}: {l:?}"
        );
        // Nothing found: the whole mode, flagged.
        let (l, none) = lines(&km, Mode::Viewer, "zzqqxx");
        assert!(none);
        assert!(l.contains(&Line::Row(Mode::Viewer, Action::Quit)));
        assert!(!matches_any(&km, "zzqqxx"));
        assert!(matches_any(&km, "ctrl+q") || matches_any(&km, "quit"));
    }
}

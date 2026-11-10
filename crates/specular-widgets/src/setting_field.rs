//! The fields of Settings' text, path and key-combination settings: edited in place with
//! the shared one-line editing model ([`LineEdit`]: caret, selection, clipboard, undo, input
//! method). Enter or the keyboard leaving the field keeps the text, through the setting's
//! own parsing and check (`specular_settings`); a refused text stays in the field with the
//! reason under it, in every field refused (each keeps its own draft and reason until it is
//! fixed or put back). Esc puts the value back. A path also has Choose… (the platform's file
//! or folder dialog, opened by the application) and Clear (back to the default).
//!
//! A colour is a field too (`#RRGGBB`, `#RRGGBBAA`), with the colour it shows beside it and
//! a grid of swatches under it that set it in one click.
//!
//! `settings.toml` stays the truth: the field holds a draft only while it is edited (or was
//! refused), and otherwise shows the stored value.

use specular_gfx::{Rect, TextBlock};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_settings::{Kind, Origin, SettingMeta, Value};
use specular_theme::metrics;

use crate::ctx::{Ctx, FieldGeom, HAlign, Target};
use crate::line_edit::LineEdit;
use crate::settings::SettingsTarget;
use crate::{Chrome, Command, Modal, Response, ViewerState};

/// Whether a setting is edited in a field.
pub(crate) fn is_field(m: &SettingMeta) -> bool {
    matches!(
        m.kind,
        Kind::Text | Kind::Path { .. } | Kind::Shortcut | Kind::Color
    )
}

/// The colours offered under a colour setting's field, with their names' message ids.
pub(crate) const SWATCHES: [([u8; 3], &str); 12] = [
    ([0xFF, 0xFF, 0xFF], "settings-colour-white"),
    ([0xC0, 0xC0, 0xC0], "settings-colour-light-grey"),
    ([0x80, 0x80, 0x80], "settings-colour-grey"),
    ([0x20, 0x20, 0x20], "settings-colour-dark-grey"),
    ([0x00, 0x00, 0x00], "settings-colour-black"),
    ([0xFF, 0xFF, 0x00], "settings-colour-yellow"),
    ([0xFF, 0x99, 0x00], "settings-colour-orange"),
    ([0xFF, 0x00, 0x00], "settings-colour-red"),
    ([0xFF, 0x00, 0xFF], "settings-colour-magenta"),
    ([0x00, 0x7C, 0xFF], "settings-colour-blue"),
    ([0x00, 0xFF, 0xFF], "settings-colour-cyan"),
    ([0x00, 0xFF, 0x00], "settings-colour-green"),
];

/// A swatch's side and the gap between swatches.
const SWATCH: f32 = 24.0;
const SWATCH_GAP: f32 = 8.0;

/// Swatches in a row of `width`.
fn swatches_per_row(width: f32) -> usize {
    (((width + SWATCH_GAP) / (SWATCH + SWATCH_GAP)).floor() as usize).clamp(1, SWATCHES.len())
}

/// The height of the swatch grid under a colour field `width` wide (0: not a colour).
pub(crate) fn swatch_grid_height(m: &SettingMeta, width: f32) -> f32 {
    if !matches!(m.kind, Kind::Color) {
        return 0.0;
    }
    let rows = SWATCHES.len().div_ceil(swatches_per_row(width));
    rows as f32 * SWATCH + (rows - 1) as f32 * SWATCH_GAP
}

/// Swatch `i` of the grid at `(x, y)`, `width` wide (laid out from the start edge; the
/// caller mirrors it).
fn swatch_rect(i: usize, x: f32, y: f32, width: f32) -> Rect {
    let n = swatches_per_row(width);
    let (row, col) = (i / n, i % n);
    Rect::new(
        x + col as f32 * (SWATCH + SWATCH_GAP),
        y + row as f32 * (SWATCH + SWATCH_GAP),
        SWATCH,
        SWATCH,
    )
}

fn theme_colour(c: [u8; 3]) -> specular_theme::Color {
    specular_theme::Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    )
}

/// Swatch `i` is the colour of setting `key`.
pub(crate) fn swatch_on(st: &ViewerState, key: &str, i: usize) -> bool {
    matches!(
        st.store.get(key),
        Ok(Value::Color(c)) if c.a == 255 && [c.r, c.g, c.b] == SWATCHES[i].0
    )
}

/// Draws the swatch grid of colour setting `m` at the top of `r`.
pub(crate) fn draw_swatches(cx: &mut Ctx, m: &'static SettingMeta, r: Rect, st: &ViewerState) {
    for (i, (rgb, _)) in SWATCHES.iter().enumerate() {
        let sr = cx.m_in(swatch_rect(i, r.x, r.y, r.w), r);
        let tgt = Target::Settings(SettingsTarget::Swatch(m.key, i as u8));
        let edge = if cx.is_hover(&tgt) {
            cx.pal.text_muted
        } else {
            cx.pal.separator
        };
        if swatch_on(st, m.key, i) {
            cx.dl.rect_border(
                sr.inset(-3.0),
                metrics::RADIUS_SM + 3.0,
                specular_theme::Color::TRANSPARENT,
                cx.pal.accent,
                2.0,
            );
        }
        cx.dl
            .rect_border(sr, metrics::RADIUS_SM, theme_colour(*rgb), edge, 1.0);
        cx.hit(sr, tgt);
    }
}

/// A value as its field shows it (an unset path: empty, the placeholder says what that
/// means).
pub(crate) fn text_of(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Path(Some(p)) => p.display().to_string(),
        Value::Path(None) => String::new(),
        v => v.to_string(),
    }
}

fn stored(st: &ViewerState, key: &str) -> String {
    st.store.get(key).map(|v| text_of(&v)).unwrap_or_default()
}

/// The field being edited, or refused when it was left.
#[derive(Clone, Debug)]
pub(crate) struct Draft {
    pub key: &'static str,
    pub edit: LineEdit,
    /// The value it started from (as its field shows it).
    pub from: String,
    /// Why the text was not kept, in the UI language.
    pub error: Option<String>,
}

impl Draft {
    fn new(key: &'static str, from: String) -> Draft {
        Draft {
            key,
            edit: LineEdit::new(&from),
            from,
            error: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.edit.text() != self.from
    }
}

impl Chrome {
    /// The setting whose field has the keyboard.
    pub(crate) fn setting_field_focus(&self) -> Option<&'static str> {
        match &self.modal_focus {
            Some(Target::Settings(SettingsTarget::Field(k)))
                if self.modal == Some(Modal::Settings) =>
            {
                Some(k)
            }
            _ => None,
        }
    }

    /// Where the field with the keyboard was last drawn.
    pub(crate) fn setting_field_geom(&self) -> Option<&FieldGeom> {
        let k = self.setting_field_focus()?;
        self.settings
            .field_geoms
            .iter()
            .find(|(f, _)| *f == k)
            .map(|(_, g)| g)
    }

    /// The draft of field `key`, when it has one (being edited, or refused and left).
    pub(crate) fn setting_draft(&self, key: &str) -> Option<&Draft> {
        self.settings
            .field
            .as_ref()
            .filter(|d| d.key == key)
            .or_else(|| self.settings.refused.iter().find(|d| d.key == key))
    }

    /// Keeps the draft's text when it changed: true when it was kept (or there was nothing
    /// to keep). Refused, the reason is shown under the field.
    pub(crate) fn setting_field_commit(&mut self, st: &ViewerState) -> bool {
        let Some(d) = self.settings.field.as_mut() else {
            return true;
        };
        if !d.dirty() {
            d.error = None;
            return true;
        }
        match st
            .store
            .set_text(d.key, d.edit.text(), Origin::SettingsWindow)
        {
            Ok(_) => {
                // As stored (trimmed): the field shows it.
                d.from = stored(st, d.key);
                if d.edit.text() != d.from {
                    d.edit.set_text(&d.from);
                }
                d.error = None;
                true
            }
            Err(e) => {
                d.error = Some(e.message(I18n::global()));
                // The reason comes into view with the field.
                self.reveal_focus = true;
                false
            }
        }
    }

    /// Esc in the field: the value is put back. False when there was nothing to put back.
    fn setting_field_revert(&mut self) -> bool {
        let Some(d) = self.settings.field.as_mut() else {
            return false;
        };
        if !d.dirty() && d.error.is_none() {
            return false;
        }
        let from = d.from.clone();
        d.edit.set_text(&from);
        d.error = None;
        true
    }

    /// After every event (and before drawing): the field the keyboard left keeps its text
    /// (a refused one stays, with its reason, until it is fixed, put back or Settings closes;
    /// every refused field keeps its own); the field the keyboard is on gets a draft (its
    /// refused one, when it has one); an unchanged draft follows the stored value (undo,
    /// Reset, a reload, Choose…).
    pub(crate) fn setting_field_sync(&mut self, st: &ViewerState) {
        if self.modal != Some(Modal::Settings) {
            if self.settings.field.as_ref().is_some_and(Draft::dirty) {
                self.setting_field_commit(st);
            }
            self.settings.field = None;
            self.settings.refused.clear();
            return;
        }
        let focus = self.setting_field_focus();
        if let Some(d) = &self.settings.field
            && Some(d.key) != focus
        {
            let kept = self.setting_field_commit(st);
            if let Some(d) = self.settings.field.take()
                && !kept
            {
                self.settings.refused.retain(|r| r.key != d.key);
                self.settings.refused.push(d);
            }
        }
        if let Some(k) = focus
            && self.settings.field.is_none()
        {
            let d = match self.settings.refused.iter().position(|d| d.key == k) {
                Some(i) => self.settings.refused.remove(i),
                None => Draft::new(k, stored(st, k)),
            };
            self.settings.field = Some(d);
        }
        if let Some(d) = self.settings.field.as_mut()
            && !d.dirty()
        {
            let now = stored(st, d.key);
            if now != d.from {
                d.edit.reset(&now);
                d.from = now;
                d.error = None;
            }
        }
    }

    /// Forgets field `key`'s draft (its value was reset or cleared).
    pub(crate) fn setting_field_forget(&mut self, key: &str) {
        if self.settings.field.as_ref().is_some_and(|d| d.key == key) {
            self.settings.field = None;
        }
        self.settings.refused.retain(|d| d.key != key);
    }

    /// The keyboard goes to field `key` (Tab, a click, a screen reader).
    pub(crate) fn setting_field_enter(&mut self, key: &'static str, st: &ViewerState) {
        self.settings.tools.focus = None;
        self.settings.tools.close_menu();
        self.modal_focus = Some(Target::Settings(SettingsTarget::Field(key)));
        self.setting_field_sync(st);
    }

    /// A key while a field has the keyboard: Enter keeps the text, Esc puts the value back
    /// (when there is nothing to put back it does what it does elsewhere), the field's
    /// editing keys edit it. Tab, ↑, ↓ and the page keys stay Settings'. `None`: not the
    /// field's.
    pub(crate) fn setting_field_key(
        &mut self,
        combo: &KeyCombo,
        st: &ViewerState,
    ) -> Option<Response> {
        self.setting_field_focus()?;
        if self.settings.dropdown.is_some() {
            return None;
        }
        let m = combo.mods;
        let plain = !(m.ctrl || m.alt || m.meta || m.shift);
        match combo.key {
            Key::Named(NamedKey::Enter) if plain => {
                self.setting_field_commit(st);
                return Some(Response::used());
            }
            Key::Named(NamedKey::Escape) if plain => {
                return self.setting_field_revert().then(Response::used);
            }
            Key::Named(
                NamedKey::Tab
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::PageUp
                | NamedKey::PageDown
                | NamedKey::Enter
                | NamedKey::Escape
                | NamedKey::F(_),
            ) => return None,
            Key::Char(c) if m.ctrl && !m.alt && !m.meta => {
                // Undo and redo are the field's while it has something to undo, else the
                // settings'.
                let edit = &self.settings.field.as_ref()?.edit;
                let redo = matches!(c, 'y' | 'Y') || (m.shift && matches!(c, 'z' | 'Z'));
                let undo = matches!(c, 'z' | 'Z') && !m.shift;
                if (undo && !edit.can_undo()) || (redo && !edit.can_redo()) {
                    return None;
                }
            }
            _ => {}
        }
        let rtl = self
            .field_geom
            .as_ref()
            .and_then(|g| g.text.as_ref())
            .map_or(self.last_rtl, |(b, _)| b.rtl);
        if let Some(r) = self.field_edit_key(combo.key, m, rtl) {
            return Some(r);
        }
        // What is typed arrives as text: the key itself presses nothing.
        match combo.key {
            Key::Char(_) | Key::Named(NamedKey::Space) if !(m.ctrl || m.alt || m.meta) => {
                Some(Response::used())
            }
            _ => None,
        }
    }

    /// A press at `p` in a field of Settings as last drawn: the keyboard goes to it and the
    /// caret where it was pressed (a drag selects, as in any field). False when it is not in
    /// one.
    pub(crate) fn setting_field_press(
        &mut self,
        p: (f32, f32),
        now: f64,
        st: &ViewerState,
    ) -> bool {
        if self.modal != Some(Modal::Settings)
            || self.settings.dropdown.is_some()
            || !self.preedit.is_empty()
        {
            return false;
        }
        let Some((k, at)) = self
            .settings
            .field_geoms
            .iter()
            .rev()
            .find(|(_, g)| g.rect.contains(p) && self.settings.body.contains(p))
            .map(|(k, g)| (*k, g.byte_at(p.0)))
        else {
            return false;
        };
        self.setting_field_enter(k, st);
        let shift = self.mods.shift;
        if let Some(d) = self.settings.field.as_mut() {
            d.edit.press(at, p.0, now, shift);
        }
        true
    }

    /// A control of a field's row pressed: the field, Choose…, Clear.
    pub(crate) fn setting_field_click(&mut self, s: &SettingsTarget, st: &ViewerState) -> Response {
        match *s {
            SettingsTarget::Field(k) => {
                if self.setting_field_focus() != Some(k) {
                    self.setting_field_enter(k, st);
                }
                Response::used()
            }
            SettingsTarget::PathChoose(key) => {
                let directory = matches!(
                    specular_settings::find(key).map(|m| &m.kind),
                    Some(Kind::Path { directory: true })
                );
                Response::cmd(Command::ChooseSettingPath { key, directory })
            }
            SettingsTarget::Swatch(k, i) => {
                self.setting_field_forget(k);
                if let Some((rgb, _)) = SWATCHES.get(usize::from(i)) {
                    let c = specular_settings::Color::rgb(rgb[0], rgb[1], rgb[2]);
                    let _ = st.store.set(k, Value::Color(c), Origin::SettingsWindow);
                }
                Response::used()
            }
            SettingsTarget::PathClear(k) => {
                self.setting_field_forget(k);
                let _ = st.store.set(k, Value::Path(None), Origin::SettingsWindow);
                // The keyboard goes to the field (Clear is gone).
                if matches!(
                    &self.modal_focus,
                    Some(Target::Settings(SettingsTarget::PathClear(_)))
                ) {
                    self.modal_focus = Some(Target::Settings(SettingsTarget::Field(k)));
                }
                Response::used()
            }
            _ => Response::default(),
        }
    }

    /// What a screen reader reads in field `key`: its text and selection (anchor, caret in
    /// characters; `None`: the caret at the end).
    pub(crate) fn setting_field_a11y(
        &self,
        key: &str,
        st: &ViewerState,
    ) -> (String, Option<(usize, usize)>) {
        match self.setting_draft(key) {
            Some(d) => (
                d.edit.text().to_owned(),
                (self.setting_field_focus() == Some(d.key)).then(|| d.edit.a11y_selection()),
            ),
            None => (stored(st, key), None),
        }
    }

    /// A screen reader's SetValue (kept at once), ReplaceSelectedText or SetTextSelection on
    /// field `key`: the keyboard goes there first.
    pub(crate) fn setting_field_a11y_action(
        &mut self,
        key: &'static str,
        action: crate::accesskit::Action,
        data: Option<&crate::accesskit::ActionData>,
        st: &ViewerState,
    ) -> Response {
        use crate::accesskit::{Action as A, ActionData as D};
        self.setting_field_enter(key, st);
        let Some(d) = self.settings.field.as_mut() else {
            return Response::default();
        };
        match (action, data) {
            (A::SetValue, Some(D::Value(v))) => {
                d.edit.set_text(v);
                self.setting_field_commit(st);
            }
            (A::ReplaceSelectedText, Some(D::Value(v))) => {
                d.edit.paste(v);
            }
            (A::SetTextSelection, Some(D::SetTextSelection(sel))) => {
                d.edit
                    .set_a11y_selection(sel.anchor.character_index, sel.focus.character_index);
                let mut r = Response::used();
                self.field_offer_primary(&mut r);
                return r;
            }
            _ => return Response::default(),
        }
        Response::used()
    }
}

/// The reason a field's text was refused, laid out under it (`width`).
pub(crate) fn error_block(
    ch: &Chrome,
    cx: &mut Ctx,
    m: &SettingMeta,
    width: f32,
) -> Option<std::sync::Arc<TextBlock>> {
    let e = ch.setting_draft(m.key)?.error.as_ref()?;
    Some(cx.layout(e, cx.style(metrics::FONT_SMALL), Some(width)))
}

/// A button of a field's row (Choose…, Clear) in `r`.
fn button(cx: &mut Ctx, r: Rect, label: &str, tgt: Target) {
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    let b = cx.layout(label, style, None);
    let fill = if cx.is_hover(&tgt) {
        cx.pal.surface_pressed
    } else {
        cx.pal.surface_raised
    };
    cx.dl
        .rect_border(r, metrics::RADIUS_SM + 1.0, fill, cx.pal.separator, 1.0);
    cx.dl.push_clip(r);
    cx.block_in(&b, r, HAlign::Center, cx.pal.text);
    cx.dl.pop_clip();
    cx.hit(r, tgt);
}

fn button_w(cx: &mut Ctx, label: &str) -> f32 {
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    (cx.layout(label, style, None).width + 24.0).ceil()
}

/// Draws setting `m`'s field in `r` (a path's buttons at its end); returns the field's
/// rectangle.
pub(crate) fn draw(
    ch: &mut Chrome,
    cx: &mut Ctx,
    m: &'static SettingMeta,
    r: Rect,
    st: &ViewerState,
) -> Rect {
    let i18n = I18n::global();
    let v = st.store.get(m.key).unwrap_or(Value::None);
    let focused = ch.setting_field_focus() == Some(m.key);
    let draft = ch.setting_draft(m.key);
    let value = draft.map_or_else(|| text_of(&v), |d| d.edit.text().to_owned());
    let mut field = r;
    if let Kind::Path { .. } = m.kind {
        let gap = 8.0;
        let choose = t!("settings-choose");
        let mut x = r.right();
        if matches!(v, Value::Path(Some(_))) {
            let clear = t!("settings-clear");
            let w = button_w(cx, &clear);
            x -= w;
            let br = cx.m_in(Rect::new(x, r.y, w, r.h), r);
            button(
                cx,
                br,
                &clear,
                Target::Settings(SettingsTarget::PathClear(m.key)),
            );
            x -= gap;
        }
        let w = button_w(cx, &choose);
        x -= w;
        let br = cx.m_in(Rect::new(x, r.y, w, r.h), r);
        button(
            cx,
            br,
            &choose,
            Target::Settings(SettingsTarget::PathChoose(m.key)),
        );
        field = cx.m_in(Rect::new(r.x, r.y, (x - gap - r.x).max(40.0), r.h), r);
    }
    // A colour: a field wide enough for `#RRGGBBAA`, the colour it holds beside it.
    if let Kind::Color = m.kind {
        let w = (cx
            .layout("#DDDDDDDD", cx.style(metrics::FONT_BODY).tabular(), None)
            .width
            + 28.0)
            .ceil()
            .min(r.w - SWATCH - 12.0)
            .max(40.0);
        if let Value::Color(c) = &v {
            let sw = cx.m_in(
                Rect::new(r.x + w + 12.0, r.center().1 - SWATCH * 0.5, SWATCH, SWATCH),
                r,
            );
            let col = specular_theme::Color::rgba(
                f32::from(c.r) / 255.0,
                f32::from(c.g) / 255.0,
                f32::from(c.b) / 255.0,
                f32::from(c.a) / 255.0,
            );
            cx.dl
                .rect_border(sw, metrics::RADIUS_SM, col, cx.pal.separator, 1.0);
        }
        field = cx.m_in(Rect::new(r.x, r.y, w, r.h), r);
    }
    // An unset path says what that means.
    let placeholder = match m.kind {
        Kind::Path { .. } => m.display_value(&Value::Path(None), i18n),
        _ => String::new(),
    };
    let edit = draft.filter(|_| focused).map(|d| &d.edit);
    let refused = draft.is_some_and(|d| d.error.is_some());
    let g: FieldGeom = cx.field_with(field, &value, edit, &placeholder, focused, focused, None);
    if refused {
        cx.dl.rect_border(
            field,
            metrics::RADIUS_SM + 1.0,
            specular_theme::Color::TRANSPARENT,
            cx.pal.danger,
            1.0,
        );
    }
    ch.settings.field_geoms.push((m.key, g));
    cx.hit(field, Target::Settings(SettingsTarget::Field(m.key)));
    field
}

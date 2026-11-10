//! The password prompt of an encrypted archive (ZIP ZipCrypto/AES, 7z, RAR): asked when the
//! reading mode opens one whose pages or file list need a password not known yet this
//! session. Open tries it; a wrong one asks again with "Wrong password"; Cancel leaves the
//! archive shown as encrypted (opening it again asks again).
//!
//! The password is typed into one buffer reserved up front and never grown past it, so no
//! stray copy is left by a reallocation; the buffer is wiped when the prompt closes, and the
//! password then lives only in [`crate::book::passwords`] (memory, for the session). It is
//! never logged (the key and input-method logging stays quiet while the prompt is open), put
//! on the clipboard or in the primary selection, or given to the input method (which is off
//! for the field, as for password fields everywhere).

#[cfg(test)]
mod shots;
mod view;

use std::path::{Path, PathBuf};

use specular_archive::Password;
use specular_gfx::{DrawList, Text};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::Palette;
use specular_widgets::accesskit;
use zeroize::Zeroizing;

use self::view::{A11Y_BASE, Drawn, Part, PromptView, masked};
use super::App;
use super::integration::PasteTo;

/// Bytes a password may have; the field's buffer is this large from the start.
const MAX_BYTES: usize = 1024;

/// The typed password: a caret into a buffer that never reallocates, wiped on drop.
pub(super) struct SecretField {
    text: Zeroizing<String>,
    /// Byte offset, on a character boundary.
    caret: usize,
}

impl SecretField {
    fn new() -> SecretField {
        SecretField {
            text: Zeroizing::new(String::with_capacity(MAX_BYTES)),
            caret: 0,
        }
    }

    /// Inserts `s` at the caret (control characters left out), as far as it fits.
    fn insert(&mut self, s: &str) -> bool {
        let mut any = false;
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.text.len() + c.len_utf8() > MAX_BYTES {
                break;
            }
            self.text.insert(self.caret, c);
            self.caret += c.len_utf8();
            any = true;
        }
        any
    }

    fn prev(&self) -> Option<usize> {
        self.text[..self.caret]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
    }

    fn next(&self) -> Option<usize> {
        self.text[self.caret..]
            .chars()
            .next()
            .map(|c| self.caret + c.len_utf8())
    }

    fn backspace(&mut self) -> bool {
        let Some(i) = self.prev() else { return false };
        self.text.remove(i);
        self.caret = i;
        true
    }

    fn delete(&mut self) -> bool {
        if self.caret >= self.text.len() {
            return false;
        }
        self.text.remove(self.caret);
        true
    }

    fn clear(&mut self) {
        // Truncating keeps the buffer; the bytes past the end are wiped with it on drop.
        self.text.clear();
        self.caret = 0;
    }

    /// Moves the caret a character back or forward.
    fn step(&mut self, forward: bool) -> bool {
        let to = if forward { self.next() } else { self.prev() };
        to.inspect(|&i| self.caret = i).is_some()
    }

    fn chars_before_caret(&self) -> usize {
        self.text[..self.caret].chars().count()
    }

    /// Puts the caret before character `n`.
    fn caret_at_char(&mut self, n: usize) {
        self.caret = self
            .text
            .char_indices()
            .nth(n)
            .map_or(self.text.len(), |(i, _)| i);
    }

    /// The password, moved out (no copy is made); the field is empty after.
    fn take(&mut self) -> Password {
        self.caret = 0;
        let s = std::mem::replace(&mut *self.text, String::with_capacity(MAX_BYTES));
        Password::new(s)
    }
}

/// An open prompt.
pub(super) struct Prompt {
    /// The archive it unlocks.
    pub archive: PathBuf,
    field: SecretField,
    show: bool,
    /// The last password tried did not open it.
    wrong: bool,
    focus: Part,
    drawn: Drawn,
}

#[derive(Default)]
pub(super) struct PasswordPrompt {
    pub dialog: Option<Prompt>,
}

impl PasswordPrompt {
    pub(super) fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn view(&self) -> Option<PromptView> {
        let d = self.dialog.as_ref()?;
        Some(build_view(d))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&PromptView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = view::draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &PromptView, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }
}

fn build_view(d: &Prompt) -> PromptView {
    let name = d.archive.file_name().map_or_else(
        || d.archive.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let (field, caret) = if d.show {
        (
            Zeroizing::new(d.field.text.as_str().to_owned()),
            d.field.caret,
        )
    } else {
        let n = d.field.chars_before_caret();
        (Zeroizing::new(masked(&d.field.text)), n * '•'.len_utf8())
    };
    PromptView {
        title: t!("password-title"),
        subtitle: t!("password-archive", archive = name),
        label: t!("password-label"),
        field,
        caret,
        show: d.show,
        show_label: t!("password-show"),
        error: d.wrong.then(|| t!("password-wrong")),
        cancel: t!("password-cancel"),
        open: t!("password-open"),
        focus: d.focus,
    }
}

impl App {
    /// Asks for the password of `archive`; `wrong`: the last one tried did not open it.
    pub(super) fn password_prompt(&mut self, archive: &Path, wrong: bool) {
        self.password.dialog = Some(Prompt {
            archive: archive.to_path_buf(),
            field: SecretField::new(),
            show: false,
            wrong,
            focus: Part::Field,
            drawn: Drawn::default(),
        });
        self.redraw = true;
    }

    pub(super) fn password_view(&self) -> Option<PromptView> {
        self.password.view()
    }

    /// Keys while the prompt is open (it takes them all); `text` is what the key types.
    pub(super) fn password_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        let rtl = self.rtl();
        let Some(d) = self.password.dialog.as_mut() else {
            return;
        };
        self.redraw = true;
        let Some(c) = combo else {
            if d.focus == Part::Field
                && let Some(t) = text
            {
                d.field.insert(t);
            }
            return;
        };
        let command = c.mods.ctrl || c.mods.meta;
        // Typing, AltGr (Ctrl+Alt on Windows) and Option characters included.
        let plain = !command || c.mods.alt;
        let step = |d: &mut Prompt, by: isize| {
            let i = Part::ORDER.iter().position(|p| *p == d.focus).unwrap_or(0) as isize;
            d.focus = Part::ORDER[(i + by).rem_euclid(Part::ORDER.len() as isize) as usize];
        };
        match c.key {
            Key::Named(NamedKey::Escape) => self.password_cancel(),
            Key::Named(NamedKey::Enter) => match d.focus {
                Part::Show => d.show = !d.show,
                Part::Cancel => self.password_cancel(),
                Part::Field | Part::Open => self.password_submit(),
            },
            Key::Named(NamedKey::Tab) => step(d, if c.mods.shift { -1 } else { 1 }),
            Key::Named(NamedKey::Space) if d.focus != Part::Field => match d.focus {
                Part::Show => d.show = !d.show,
                Part::Cancel => self.password_cancel(),
                _ => self.password_submit(),
            },
            // Ctrl+V and Shift+Insert paste; nothing copies or cuts out of the field.
            Key::Char(ch) if command && ch.eq_ignore_ascii_case(&'v') && d.focus == Part::Field => {
                self.field_clipboard(
                    specular_widgets::line_edit::PendingClipboard {
                        paste: true,
                        ..Default::default()
                    },
                    PasteTo::Password,
                );
            }
            Key::Named(NamedKey::Insert) if c.mods.shift && d.focus == Part::Field => {
                self.field_clipboard(
                    specular_widgets::line_edit::PendingClipboard {
                        paste: true,
                        ..Default::default()
                    },
                    PasteTo::Password,
                );
            }
            _ if d.focus == Part::Field => {
                let f = &mut d.field;
                match c.key {
                    Key::Named(NamedKey::Backspace) if command => {
                        // Deletes back to the start, as a word delete would.
                        while f.backspace() {}
                    }
                    Key::Named(NamedKey::Backspace) => {
                        f.backspace();
                    }
                    Key::Named(NamedKey::Delete) => {
                        f.delete();
                    }
                    // Dots have no direction: the arrows move as the layout reads.
                    Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                        f.step((k == NamedKey::ArrowRight) != rtl);
                    }
                    Key::Named(NamedKey::Home) => f.caret = 0,
                    Key::Named(NamedKey::End) => f.caret = f.text.len(),
                    Key::Named(NamedKey::ArrowDown) => d.focus = Part::Show,
                    _ if plain => {
                        if let Some(t) = text {
                            f.insert(t);
                        }
                    }
                    _ => {}
                }
            }
            Key::Named(NamedKey::ArrowUp) if d.focus == Part::Show => d.focus = Part::Field,
            Key::Named(NamedKey::ArrowDown) if d.focus == Part::Show => d.focus = Part::Open,
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight))
                if matches!(d.focus, Part::Cancel | Part::Open) =>
            {
                // Cancel comes before Open in reading order.
                let forward = (k == NamedKey::ArrowRight) != rtl;
                d.focus = if forward { Part::Open } else { Part::Cancel };
            }
            _ => {}
        }
    }

    /// Clipboard text for the field (Ctrl+V); the copy read from the clipboard is wiped.
    pub(super) fn password_paste(&mut self, s: String) {
        let s = Zeroizing::new(s);
        if let Some(d) = self.password.dialog.as_mut() {
            // One line: a pasted line break ends it.
            d.field.insert(s.lines().next().unwrap_or(""));
            self.redraw = true;
        }
    }

    pub(super) fn password_click(&mut self, p: (f32, f32), down: bool) {
        if !down {
            return;
        }
        let Some(d) = self.password.dialog.as_mut() else {
            return;
        };
        let hit = d
            .drawn
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        let Some(h) = hit else { return };
        self.redraw = true;
        match h {
            Part::Field => {
                if let Some((block, x0, mirrored)) = d.drawn.field_text.clone() {
                    let x = p.0 - x0;
                    let b = block.hit_byte(if mirrored { block.width - x } else { x });
                    let n = if d.show {
                        d.field.text[..b.min(d.field.text.len())].chars().count()
                    } else {
                        b / '•'.len_utf8()
                    };
                    d.field.caret_at_char(n);
                }
                d.focus = Part::Field;
            }
            Part::Show => {
                d.show = !d.show;
                d.focus = Part::Show;
            }
            Part::Cancel => self.password_cancel(),
            Part::Open => self.password_submit(),
        }
    }

    pub(super) fn password_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(d) = self.password.dialog.as_mut() else {
            return false;
        };
        let Some(part) = view::a11y_target(req.target_node.0) else {
            return (A11Y_BASE..A11Y_BASE + 0x1000).contains(&req.target_node.0);
        };
        self.redraw = true;
        match (req.action, part) {
            (accesskit::Action::SetValue, Part::Field) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data {
                    d.field.clear();
                    d.field.insert(s);
                }
            }
            (accesskit::Action::Click, Part::Show) => d.show = !d.show,
            (accesskit::Action::Click, Part::Cancel) => self.password_cancel(),
            (accesskit::Action::Click, Part::Open) => self.password_submit(),
            (accesskit::Action::Focus | accesskit::Action::Click, p) => d.focus = p,
            _ => {}
        }
        true
    }

    /// Open: tries the password (the prompt comes back when it is wrong).
    fn password_submit(&mut self) {
        let Some(mut d) = self.password.dialog.take() else {
            return;
        };
        let password = d.field.take();
        self.redraw = true;
        self.unlock_book(&d.archive, password);
    }

    /// Cancel: the archive stays shown as encrypted.
    fn password_cancel(&mut self) {
        self.password.dialog = None;
        self.redraw = true;
    }

    /// `--show password` (and `password-wrong`): the prompt over the current image, for an
    /// archive named `comics.cbz`. False when `scene` is not one of these.
    pub(super) fn password_show(&mut self, scene: &str) -> bool {
        let wrong = match scene {
            "password" => false,
            "password-wrong" => true,
            _ => return false,
        };
        self.password_prompt(Path::new("comics.cbz"), wrong);
        if let Some(d) = self.password.dialog.as_mut() {
            d.field.insert("hunter2");
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_edits_in_place() {
        let mut f = SecretField::new();
        let cap = f.text.capacity();
        assert!(f.insert("äb\u{7}c"));
        assert_eq!(f.text.as_str(), "äbc");
        assert!(f.step(false));
        assert!(f.backspace());
        assert_eq!(f.text.as_str(), "äc");
        f.caret = 0;
        assert!(!f.backspace());
        assert!(f.delete());
        assert_eq!(f.text.as_str(), "c");
        f.caret_at_char(1);
        f.insert(&"x".repeat(2 * MAX_BYTES));
        assert_eq!(f.text.len(), MAX_BYTES);
        // Never reallocated: no copy of the typed text is left behind.
        assert_eq!(f.text.capacity(), cap);
        let p = f.take();
        assert_eq!(p.as_str().len(), MAX_BYTES);
        assert!(f.text.is_empty() && f.caret == 0);
    }

    #[test]
    fn the_view_shows_dots_until_asked() {
        let mut d = Prompt {
            archive: PathBuf::from("/x/secret.cbz"),
            field: SecretField::new(),
            show: false,
            wrong: true,
            focus: Part::Field,
            drawn: Drawn::default(),
        };
        d.field.insert("pässword");
        let v = build_view(&d);
        assert_eq!(v.field.as_str(), "••••••••");
        assert_eq!(v.caret, v.field.len());
        assert!(v.error.is_some());
        d.show = true;
        assert_eq!(build_view(&d).field.as_str(), "pässword");
    }
}

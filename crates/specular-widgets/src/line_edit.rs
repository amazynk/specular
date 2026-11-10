//! A single-line text field's editing model, shared by the app's text fields (the gallery's
//! filter, Convert's name pattern, the search fields of Settings, the language list and the
//! palette, the DICOM tag browser's filter and the config editor's find field).
//!
//! It is a [`Buffer`] with what a one-line field adds: the keys of a text field (caret and
//! selection movement, Ctrl+A, the clipboard with Ctrl+C/X/V and Ctrl+Insert, Shift+Insert,
//! Shift+Delete, undo and redo), clicks (one puts the caret, two select a word, three
//! everything; a drag selects), pasted text kept to one line, and the selection in characters
//! for AccessKit (`TextSelection` and its `SetTextSelection` action).
//!
//! The clipboard is behind [`TextClipboard`]: the app's reads the system clipboard off the UI
//! thread and hands the text to [`LineEdit::paste`] later; tests use [`MemoryClipboard`],
//! which never touches the desktop session.
//!
//! The primary selection of X11 and Wayland goes through it too: what is selected is offered
//! ([`LineEdit::offer_primary`], once a drag ends), and a middle click pastes it where it
//! lands ([`LineEdit::paste_primary_at`]). Owners call these only where the platform has a
//! primary selection (Linux and the BSDs); elsewhere a middle click pastes nothing. What the
//! keyboard selects is offered once the keys rest ([`LineEdit::offer_primary_later`]): on
//! the key's release, or [`PRIMARY_IDLE_S`] after the last key, so a held Shift+Arrow does
//! not set the primary selection at every step.

use std::ops::Range;

use specular_input::{Key, Mods, NamedKey};

use crate::text_buffer::{Buffer, EditKind};

/// Seconds between presses that make a double (and triple) click.
pub const MULTI_CLICK_S: f64 = 0.4;
/// How far (logical px) the pointer may move between the presses of a multi-click.
const MULTI_CLICK_PX: f32 = 4.0;

/// Whether the platform has a primary selection (X11 and Wayland: Linux and the BSDs):
/// selecting copies, a middle click pastes. Elsewhere neither happens.
pub const PRIMARY_SELECTION: bool = cfg!(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "emscripten"
    ))
));

/// Seconds after the last key that the keyboard's selection is offered as the primary
/// selection when no key release has offered it already.
pub const PRIMARY_IDLE_S: f64 = 0.5;

/// Where a field's copy, cut and paste go.
pub trait TextClipboard {
    /// Puts `text` on the clipboard.
    fn set_text(&mut self, text: String);
    /// The clipboard's text when it can be had now. `None`: there is none, or it is being
    /// read in the background and will come to [`LineEdit::paste`].
    fn get_text(&mut self) -> Option<String>;
    /// Makes `text` the primary selection (what a middle click pastes). Nothing by default.
    fn set_primary(&mut self, text: String) {
        let _ = text;
    }
    /// The primary selection's text when it can be had now. `None`: there is none, or it is
    /// being read in the background and will come to [`LineEdit::paste`].
    fn get_primary(&mut self) -> Option<String> {
        None
    }
}

/// A clipboard in memory (tests, and fields that keep to themselves).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
    pub text: Option<String>,
    /// The primary selection.
    pub primary: Option<String>,
}

impl TextClipboard for MemoryClipboard {
    fn set_text(&mut self, text: String) {
        self.text = Some(text);
    }
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }
    fn set_primary(&mut self, text: String) {
        self.primary = Some(text);
    }
    fn get_primary(&mut self) -> Option<String> {
        self.primary.clone()
    }
}

/// A clipboard the app serves after the key: what was copied, and whether a paste was asked
/// for (the app reads the system clipboard off the UI thread and hands its text to
/// [`LineEdit::paste`]); the same for the primary selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingClipboard {
    pub copied: Option<String>,
    pub paste: bool,
    /// Text offered as the primary selection.
    pub primary: Option<String>,
    /// A middle click asked for the primary selection's text.
    pub paste_primary: bool,
}

impl TextClipboard for PendingClipboard {
    fn set_text(&mut self, text: String) {
        self.copied = Some(text);
    }
    fn get_text(&mut self) -> Option<String> {
        self.paste = true;
        None
    }
    fn set_primary(&mut self, text: String) {
        self.primary = Some(text);
    }
    fn get_primary(&mut self) -> Option<String> {
        self.paste_primary = true;
        None
    }
}

/// Copy-on-select for one text: the selection last offered as the primary selection, and
/// when the keyboard's selection is due to be (the text fields and the config editor).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrimaryOffer {
    offered: Option<(Range<usize>, String)>,
    /// When a selection made with the keys is offered (seconds, the caller's clock).
    due: Option<f64>,
}

impl PrimaryOffer {
    /// Offers the selection `sel` (its text `text`) now when it is new (made or changed
    /// since it was last offered); nothing while `dragging` (a drag offers on release).
    /// True when it was offered.
    pub fn offer(
        &mut self,
        sel: Range<usize>,
        text: &str,
        dragging: bool,
        clip: &mut dyn TextClipboard,
    ) -> bool {
        self.due = None;
        if dragging {
            return false;
        }
        if sel.is_empty() {
            // The primary selection keeps its text; selecting it again offers it again.
            self.offered = None;
            return false;
        }
        if self
            .offered
            .as_ref()
            .is_some_and(|(r, t)| *r == sel && t == text)
        {
            return false;
        }
        clip.set_primary(text.to_owned());
        self.offered = Some((sel, text.to_owned()));
        true
    }

    /// A key may have changed the selection: a new one is offered [`PRIMARY_IDLE_S`] after
    /// `now` (each key puts it off again), or sooner by [`offer`](Self::offer) on the key's
    /// release.
    pub fn later(&mut self, sel: Range<usize>, text: &str, dragging: bool, now: f64) {
        let new = !dragging
            && !sel.is_empty()
            && !self
                .offered
                .as_ref()
                .is_some_and(|(r, t)| *r == sel && t == text);
        if new {
            self.due = Some(now + PRIMARY_IDLE_S);
        } else {
            if sel.is_empty() && !dragging {
                self.offered = None;
            }
            self.due = None;
        }
    }

    /// A selection the program made, not the user (a find match): taken as offered without
    /// offering it, so it becomes the primary selection only once the user changes it (or
    /// lets it go and selects it again). Any offer waiting for keys is dropped.
    pub fn pass_over(&mut self, sel: Range<usize>, text: &str) {
        self.due = None;
        self.offered = (!sel.is_empty()).then(|| (sel, text.to_owned()));
    }

    /// Whether a selection made with the keys waits to be offered.
    pub fn pending(&self) -> bool {
        self.due.is_some()
    }

    /// Whether the waiting selection is due at `now`.
    pub fn due(&self, now: f64) -> bool {
        self.due.is_some_and(|d| now >= d)
    }

    /// Seconds from `now` until the waiting selection is due (0 when it is).
    pub fn wake(&self, now: f64) -> Option<f64> {
        self.due.map(|d| (d - now).max(0.0))
    }
}

/// What a field takes beyond any one-line text (the toolbar's go-to field: digits).
#[derive(Clone, Copy, Debug)]
pub struct Filter {
    /// A typed or pasted character as the field keeps it; `None`: left out.
    pub map: fn(char) -> Option<char>,
    /// Whether the field may hold this text, checked as each character goes in (a character
    /// that would make it invalid is left out).
    pub valid: fn(&str) -> bool,
}

/// What a key did to a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
    /// Not a key of the field.
    No,
    /// Used, the text is unchanged (the caret or selection moved, something was copied, or a
    /// paste was asked for).
    Used,
    /// The text changed.
    Edited,
}

impl Handled {
    pub fn used(self) -> bool {
        self != Handled::No
    }
}

/// Presses in a row (the field's and the config editor's): the last one's time, place and
/// count, which make the next a double or triple click.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Clicks(Option<(f64, (f32, f32), u8)>);

impl Clicks {
    /// A press at `p` (logical px) at `now` (seconds): how many in a row it makes (1, 2, 3,
    /// then 1 again). One soon after the last, in the same place, adds to it; a Shift+click
    /// is always a single one (it extends the selection).
    pub(crate) fn press(&mut self, p: (f32, f32), now: f64, shift: bool) -> u8 {
        let n = match self.0 {
            Some((t, q, n))
                if now - t < MULTI_CLICK_S
                    && (p.0 - q.0).abs() <= MULTI_CLICK_PX
                    && (p.1 - q.1).abs() <= MULTI_CLICK_PX
                    && !shift =>
            {
                n % 3 + 1
            }
            _ => 1,
        };
        self.0 = Some((now, p, n));
        n
    }

    /// The next press starts a new row (another kind of press came between).
    pub(crate) fn clear(&mut self) {
        self.0 = None;
    }
}

/// What a drag selects by: characters, words (after a double click) or lines (after a
/// triple click; a one-line field's line is all of it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unit {
    Char,
    Word,
    Line,
}

impl Unit {
    /// The unit `n` presses in a row select by.
    pub(crate) fn of_clicks(n: u8) -> Unit {
        match n {
            1 => Unit::Char,
            2 => Unit::Word,
            _ => Unit::Line,
        }
    }
}

/// A press being dragged: what it selected first (a word after a double click, a line after
/// a triple click), which the selection keeps while it is extended by its unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Drag {
    pub origin: Range<usize>,
    pub unit: Unit,
}

impl Drag {
    /// The selection (anchor first, so possibly backwards) once the pointer is at byte `at`:
    /// from the press's origin to there, by whole units (`span`: the unit around a byte).
    pub(crate) fn to(
        &self,
        at: usize,
        span: impl FnOnce(Unit, usize) -> Range<usize>,
    ) -> Range<usize> {
        let o = &self.origin;
        let at = match self.unit {
            Unit::Char => at,
            u => {
                let w = span(u, at);
                if at < o.start { w.start } else { w.end }
            }
        };
        if at < o.start {
            o.end..at
        } else {
            o.start..at.max(o.end)
        }
    }
}

/// A single-line text field: its text, caret, selection and undo history.
#[derive(Clone, Debug)]
pub struct LineEdit {
    buf: Buffer,
    /// The last presses: when, where and how many in a row.
    clicks: Clicks,
    drag: Option<Drag>,
    /// Ctrl+Backspace and Ctrl+Delete take a whole term (up to the next space) instead of a
    /// word: the gallery's filter, whose terms hold punctuation (`camera:x-t5`).
    terms: bool,
    /// Copy-on-select: what was last offered, and when the keyboard's selection is due.
    primary: PrimaryOffer,
    /// What is typed and pasted keeps to this.
    filter: Option<Filter>,
}

impl Default for LineEdit {
    fn default() -> Self {
        LineEdit::new("")
    }
}

impl PartialEq for LineEdit {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text() && self.caret() == other.caret()
    }
}

/// Characters a word is made of (the word a double click selects).
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// What a double click takes: a run of word characters, of spaces, or of other characters.
fn class(c: char) -> u8 {
    if is_word(c) {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

/// `s` as one line: line breaks and tabs become spaces (a CRLF one space), other control
/// characters are dropped.
pub fn single_line(s: &str) -> String {
    let s = s.replace("\r\n", " ");
    s.chars()
        .filter_map(|c| match c {
            '\n' | '\r' | '\t' | '\u{2028}' | '\u{2029}' | '\u{85}' | '\u{b}' | '\u{c}' => {
                Some(' ')
            }
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

/// The word (or run of spaces, or of other characters) of `t` around byte `at`: what a
/// double click selects. The character under the pointer is the one after `at`, or the one
/// before it at the end. In a text of many lines, `t` is the line (a line break would be a
/// space).
pub(crate) fn word_at(t: &str, at: usize) -> Range<usize> {
    let mut at = at.min(t.len());
    while !t.is_char_boundary(at) {
        at -= 1;
    }
    let Some(c) = t[at..]
        .chars()
        .next()
        .or_else(|| t[..at].chars().next_back())
    else {
        return at..at;
    };
    let k = class(c);
    let start = t[..at]
        .char_indices()
        .rev()
        .take_while(|&(_, c)| class(c) == k)
        .last()
        .map_or(at, |(i, _)| i);
    let end = t[at..]
        .char_indices()
        .take_while(|&(_, c)| class(c) == k)
        .last()
        .map_or(at, |(i, c)| at + i + c.len_utf8());
    start..end
}

impl LineEdit {
    /// A field holding `text` with the caret at its end and nothing to undo.
    pub fn new(text: &str) -> LineEdit {
        let mut buf = Buffer::new(&single_line(text));
        buf.doc_end(false);
        LineEdit {
            buf,
            clicks: Clicks::default(),
            drag: None,
            terms: false,
            primary: PrimaryOffer::default(),
            filter: None,
        }
    }

    /// Ctrl+Backspace and Ctrl+Delete take whole space-separated terms.
    pub fn with_terms(mut self) -> LineEdit {
        self.terms = true;
        self
    }

    /// What is typed and pasted keeps to `filter` (characters mapped or left out, and only
    /// while the text stays valid).
    pub fn with_filter(mut self, filter: Filter) -> LineEdit {
        self.filter = Some(filter);
        self
    }

    /// `s` as the filter lets it over the selection: each character mapped, and kept only
    /// when the text with it (and what was kept before it) stays valid.
    fn filtered(&self, s: &str) -> String {
        let Some(f) = self.filter else {
            return s.to_owned();
        };
        let (t, sel) = (self.text(), self.selection());
        let (head, tail) = (&t[..sel.start], &t[sel.end..]);
        let mut out = String::new();
        for c in s.chars().filter_map(f.map) {
            let mut with = String::with_capacity(t.len() + out.len() + 4);
            with.push_str(head);
            with.push_str(&out);
            with.push(c);
            with.push_str(tail);
            if (f.valid)(&with) {
                out.push(c);
            }
        }
        out
    }

    pub fn text(&self) -> &str {
        self.buf.text()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.text().is_empty()
    }

    /// The caret (a byte offset).
    pub fn caret(&self) -> usize {
        self.buf.caret()
    }

    /// The end of the selection the caret is not at.
    pub fn anchor(&self) -> usize {
        self.buf.anchor()
    }

    /// The selection, start before end (empty when there is none).
    pub fn selection(&self) -> Range<usize> {
        self.buf.selection()
    }

    pub fn has_selection(&self) -> bool {
        self.buf.has_selection()
    }

    pub fn selected_text(&self) -> &str {
        self.buf.selected_text()
    }

    pub fn can_undo(&self) -> bool {
        self.buf.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.buf.can_redo()
    }

    /// Catches up with `text`, the field's text kept elsewhere (the truth), changed another
    /// way: as [`set_text`](Self::set_text), and nothing when it is the field's.
    pub fn sync(&mut self, text: &str) {
        if self.text() != text {
            self.set_text(text);
        }
    }

    /// The field, when its text is `text` (else it is behind and its caret means nothing).
    pub fn of(&self, text: &str) -> Option<&LineEdit> {
        (self.text() == text).then_some(self)
    }

    /// Replaces the text (a change made outside the field: a pick from a list, a chip
    /// removed, a screen reader's SetValue) as one undoable step, with the caret at the end.
    /// Nothing happens when the text is already `text`.
    pub fn set_text(&mut self, text: &str) {
        let text = single_line(text);
        if text == self.buf.text() {
            return;
        }
        self.buf.select_all();
        self.buf.insert(&text, EditKind::Other);
        // Sealed: what is typed next is a step of its own.
        self.buf.doc_end(false);
        self.drag = None;
    }

    /// Empties the field (an undoable step).
    pub fn clear(&mut self) {
        self.set_text("");
    }

    /// Starts over with `text` and no history (a dialog opened again).
    pub fn reset(&mut self, text: &str) {
        *self = LineEdit {
            terms: self.terms,
            filter: self.filter,
            ..LineEdit::new(text)
        };
    }

    /// Puts the caret at byte `at` (clamped to a character boundary); with `extend` the
    /// selection grows from the anchor.
    pub fn set_caret(&mut self, at: usize, extend: bool) {
        self.buf.set_caret(at, extend);
    }

    /// Selects `range` with the caret at its end.
    pub fn select(&mut self, range: Range<usize>) {
        self.buf.select(range);
    }

    pub fn select_all(&mut self) {
        self.buf.select_all();
    }

    /// Typed text (keys, input-method commits) over the selection; control characters are
    /// left out. True when the text changed.
    pub fn insert(&mut self, s: &str) -> bool {
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        let s = self.filtered(&s);
        if s.is_empty() {
            return false;
        }
        self.buf.insert(&s, EditKind::Type);
        true
    }

    /// Pasted text over the selection, as one line ([`single_line`]) and one undo step.
    /// True when the text changed. In a field with a filter, nothing it takes leaves the
    /// selection alone.
    pub fn paste(&mut self, s: &str) -> bool {
        let s = self.filtered(&single_line(s));
        if s.is_empty() && (!self.has_selection() || self.filter.is_some()) {
            return false;
        }
        let v = self.buf.version();
        self.buf.insert(&s, EditKind::Other);
        self.buf.set_caret(self.buf.caret(), false);
        self.buf.version() != v
    }

    pub fn undo(&mut self) -> bool {
        self.buf.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.buf.redo()
    }

    /// The start of the term before `at` (Ctrl+Backspace in a field of terms).
    fn term_left_of(&self, at: usize) -> usize {
        let t = self.text();
        let before = t[..at].trim_end();
        before.rfind(char::is_whitespace).map_or(0, |i| i + 1)
    }

    /// The end of the term after `at` (Ctrl+Delete in a field of terms).
    fn term_right_of(&self, at: usize) -> usize {
        let rest = &self.text()[at..];
        let lead = rest.len() - rest.trim_start().len();
        at + rest[lead..]
            .find(char::is_whitespace)
            .map_or(rest.len(), |i| lead + i)
    }

    /// Removes `r`, a whole term, as an undo step of its own.
    fn remove(&mut self, r: Range<usize>) -> bool {
        if r.is_empty() {
            return false;
        }
        self.buf.replace(r, "", EditKind::Other);
        true
    }

    /// Copies the selection; false when there is none.
    fn copy(&self, clip: &mut dyn TextClipboard) -> bool {
        if !self.has_selection() {
            return false;
        }
        clip.set_text(self.selected_text().to_owned());
        true
    }

    /// A key while the field has the keyboard. `rtl`: the text reads right to left on
    /// screen (its first strong character is), so Left moves forward through it.
    ///
    /// Caret: Left/Right (Ctrl: by word), Home/End; with Shift they select. Editing:
    /// Backspace/Delete (Ctrl: a word, or a term), Ctrl+A, Ctrl+C/X/V, Ctrl+Insert (copy),
    /// Shift+Insert (paste), Shift+Delete (cut), Ctrl+Z (undo), Ctrl+Shift+Z and Ctrl+Y
    /// (redo). Keys with Alt or Super are not the field's; neither are Up, Down, Enter,
    /// Escape or Tab, which the field's owner handles.
    pub fn key(
        &mut self,
        key: Key,
        mods: Mods,
        rtl: bool,
        clip: &mut dyn TextClipboard,
    ) -> Handled {
        if mods.alt || mods.meta {
            return Handled::No;
        }
        let (ctrl, shift) = (mods.ctrl, mods.shift);
        let v = self.buf.version();
        let used = match key {
            Key::Named(n @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                let forward = (n == NamedKey::ArrowRight) != rtl;
                match (forward, ctrl) {
                    (true, true) => self.buf.word_right(shift),
                    (true, false) => self.buf.right(shift),
                    (false, true) => self.buf.word_left(shift),
                    (false, false) => self.buf.left(shift),
                }
                true
            }
            Key::Named(NamedKey::Home) => {
                self.buf.doc_start(shift);
                true
            }
            Key::Named(NamedKey::End) => {
                self.buf.doc_end(shift);
                true
            }
            Key::Named(NamedKey::Delete) if shift && !ctrl => {
                // Cut; without a selection, the character after the caret.
                if self.copy(clip) {
                    self.buf.replace(self.selection(), "", EditKind::Other);
                } else {
                    self.buf.delete(false);
                }
                true
            }
            Key::Named(NamedKey::Backspace) if ctrl && self.terms && !self.has_selection() => {
                let c = self.caret();
                self.remove(self.term_left_of(c)..c);
                true
            }
            Key::Named(NamedKey::Delete) if ctrl && self.terms && !self.has_selection() => {
                let c = self.caret();
                self.remove(c..self.term_right_of(c));
                true
            }
            Key::Named(NamedKey::Backspace) => {
                self.buf.backspace(ctrl);
                true
            }
            Key::Named(NamedKey::Delete) => {
                self.buf.delete(ctrl);
                true
            }
            Key::Named(NamedKey::Insert) if ctrl && !shift => {
                self.copy(clip);
                true
            }
            Key::Named(NamedKey::Insert) if shift && !ctrl => {
                if let Some(s) = clip.get_text() {
                    self.paste(&s);
                }
                true
            }
            Key::Char(c) if ctrl => match c.to_ascii_lowercase() {
                'a' if !shift => {
                    self.buf.select_all();
                    true
                }
                'c' if !shift => {
                    self.copy(clip);
                    true
                }
                'x' if !shift => {
                    if self.copy(clip) {
                        self.buf.replace(self.selection(), "", EditKind::Other);
                    }
                    true
                }
                'v' if !shift => {
                    if let Some(s) = clip.get_text() {
                        self.paste(&s);
                    }
                    true
                }
                'z' if shift => {
                    self.buf.redo();
                    true
                }
                'z' => {
                    self.buf.undo();
                    true
                }
                'y' if !shift => {
                    self.buf.redo();
                    true
                }
                _ => false,
            },
            _ => false,
        };
        if !used {
            Handled::No
        } else if self.buf.version() != v {
            Handled::Edited
        } else {
            Handled::Used
        }
    }

    /// The word (or run of spaces, or of other characters) around byte `at`: what a double
    /// click selects.
    pub fn word_at(&self, at: usize) -> Range<usize> {
        word_at(self.text(), at)
    }

    /// The unit `u` around byte `at`: the word, or (a line) the whole text.
    fn span(&self, u: Unit, at: usize) -> Range<usize> {
        match u {
            Unit::Char => at..at,
            Unit::Word => self.word_at(at),
            Unit::Line => 0..self.text().len(),
        }
    }

    /// A press in the field at byte `at` (pointer at `x`, logical px; `now` in seconds).
    /// One puts the caret (Shift: extends the selection to it), a second in the same place
    /// soon after selects the word, a third the whole text. A drag ([`drag_to`]) follows.
    ///
    /// [`drag_to`]: LineEdit::drag_to
    pub fn press(&mut self, at: usize, x: f32, now: f64, shift: bool) {
        let n = self.clicks.press((x, 0.0), now, shift);
        let unit = Unit::of_clicks(n);
        if n == 1 && shift {
            self.buf.set_caret(at, true);
            self.drag = Some(Drag {
                origin: self.anchor()..self.anchor(),
                unit,
            });
            return;
        }
        let origin = self.span(unit, at);
        self.buf.select(origin.clone());
        self.drag = Some(Drag { origin, unit });
    }

    /// The pointer moved to byte `at` with the button held: the selection runs from where
    /// the press was (by whole words after a double click) to there.
    pub fn drag_to(&mut self, at: usize) {
        let Some(d) = &self.drag else { return };
        let sel = d.to(at, |u, at| self.span(u, at));
        self.buf.select(sel);
    }

    /// The button went up.
    pub fn release(&mut self) {
        self.drag = None;
    }

    /// Whether a press is being dragged.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The character index of byte `at`.
    fn chars_to(&self, at: usize) -> usize {
        self.text()[..at.min(self.text().len())].chars().count()
    }

    /// The byte of character index `i` (the end past the last).
    fn byte_of(&self, i: usize) -> usize {
        self.text()
            .char_indices()
            .nth(i)
            .map_or(self.text().len(), |(b, _)| b)
    }

    /// The selection in characters, (anchor, caret), for AccessKit's text selection.
    pub fn a11y_selection(&self) -> (usize, usize) {
        (self.chars_to(self.anchor()), self.chars_to(self.caret()))
    }

    /// AccessKit's SetTextSelection: anchor and focus in characters (past the end: the end).
    pub fn set_a11y_selection(&mut self, anchor: usize, focus: usize) {
        let (a, f) = (self.byte_of(anchor), self.byte_of(focus));
        self.buf.select(a..f);
        self.drag = None;
    }

    /// Copy-on-select: offers the selection as the primary selection when it is new (made or
    /// changed since it was last offered), not while a drag is still making it. True when it
    /// was offered. Owners call it after whatever may have selected with the pointer (a press
    /// and its release, a screen reader's selection) and on a key's release, where the
    /// platform has a primary selection; after keys, [`offer_primary_later`] instead.
    ///
    /// [`offer_primary_later`]: Self::offer_primary_later
    pub fn offer_primary(&mut self, clip: &mut dyn TextClipboard) -> bool {
        let (sel, dragging) = (self.selection(), self.dragging());
        self.primary
            .offer(sel, self.buf.selected_text(), dragging, clip)
    }

    /// Copy-on-select after a key: a new selection is offered on the key's release
    /// ([`offer_primary`](Self::offer_primary)) or [`PRIMARY_IDLE_S`] after `now`
    /// ([`offer_primary_due`](Self::offer_primary_due)), whichever comes first.
    pub fn offer_primary_later(&mut self, now: f64) {
        let (sel, dragging) = (self.selection(), self.dragging());
        self.primary
            .later(sel, self.buf.selected_text(), dragging, now);
    }

    /// Offers the keyboard's selection when it is due at `now`. True when it was offered.
    pub fn offer_primary_due(&mut self, now: f64, clip: &mut dyn TextClipboard) -> bool {
        self.primary.due(now) && self.offer_primary(clip)
    }

    /// Whether a selection made with the keys waits to be offered.
    pub fn primary_pending(&self) -> bool {
        self.primary.pending()
    }

    /// Seconds from `now` until the keyboard's selection is offered, while one waits.
    pub fn primary_wake(&self, now: f64) -> Option<f64> {
        self.primary.wake(now)
    }

    /// A middle click at byte `at`: the caret goes there (letting the selection go) and the
    /// primary selection's text is pasted at it, as one line and one undo step. `Edited`
    /// when it was pasted now; `Used` when there is nothing to paste or it is read in the
    /// background (it then comes to [`paste`](Self::paste)).
    pub fn paste_primary_at(&mut self, at: usize, clip: &mut dyn TextClipboard) -> Handled {
        self.drag = None;
        self.clicks.clear();
        self.buf.set_caret(at, false);
        match clip.get_primary() {
            Some(s) if self.paste(&s) => Handled::Edited,
            _ => Handled::Used,
        }
    }
}

/// A one-line field's text while an input method composes (IME preedit): `preedit`
/// inserted at the byte `caret` of `text`. Returns the text to lay out and the composition's
/// byte range in it; the caret is drawn at the range's end, the range underlined. With no
/// composition, `text` as it is and an empty range at the caret.
pub fn compose(text: &str, caret: usize, preedit: &str) -> (String, Range<usize>) {
    let mut at = caret.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    let shown = format!("{}{preedit}{}", &text[..at], &text[at..]);
    (shown, at..at + preedit.len())
}

/// Where a one-line field draws its laid-out text: the x of `block`'s left edge in `room`,
/// with the caret `cx` (from [`specular_gfx::TextBlock::caret_x`]) in view. Text that fits
/// sits at the start edge: the right one with `start_right` (a right-to-left interface, or
/// right-to-left text where the field follows its text). Text that does not fit keeps its
/// own start in view (the right end of Hebrew, the left end of Latin) and scrolls only as
/// far as the caret needs.
pub fn field_x0(
    block: &specular_gfx::TextBlock,
    cx: f32,
    room: specular_gfx::Rect,
    start_right: bool,
) -> f32 {
    scroll_x0(block.width, cx, room, start_right, block.rtl)
}

/// [`field_x0`] for text `width` wide whose start is on the right when `rtl_text` (the
/// password field's dots, which a right-to-left interface lays out from the right).
pub fn scroll_x0(
    width: f32,
    cx: f32,
    room: specular_gfx::Rect,
    start_right: bool,
    rtl_text: bool,
) -> f32 {
    if width <= room.w {
        if start_right {
            room.right() - width
        } else {
            room.x
        }
    } else if rtl_text {
        // Right-aligned; moved right while the caret would be left of the field.
        let x0 = room.right() - width;
        x0 + (room.x + 2.0 - (x0 + cx)).max(0.0)
    } else {
        room.x - (cx - room.w + 2.0).max(0.0)
    }
}

/// Where the input method's candidate window opens: a box (logical px) spanning the
/// composition `range` of `block`, drawn from `x0`, and the caret at its end, on the line
/// `y`..`y + h`, kept within `clip` horizontally.
pub fn ime_area(
    block: &specular_gfx::TextBlock,
    x0: f32,
    range: Range<usize>,
    y: f32,
    h: f32,
    clip: specular_gfx::Rect,
) -> specular_gfx::Rect {
    let (a, b) = (
        x0 + block.caret_x(range.start),
        x0 + block.caret_x(range.end),
    );
    let lo = a.min(b).clamp(clip.x - 2.0, clip.right());
    let hi = (a.max(b) + 1.5).clamp(clip.x, clip.right() + 2.0);
    specular_gfx::Rect::new(lo, y, (hi - lo).max(1.0), h)
}

/// Draws a composition's underline: `range` of `block`, drawn from `x0`, along `bottom`.
pub fn draw_preedit_underline(
    dl: &mut specular_gfx::DrawList,
    block: &specular_gfx::TextBlock,
    x0: f32,
    range: Range<usize>,
    bottom: f32,
    color: specular_theme::Color,
) {
    for (a, b) in block.range_spans(range) {
        dl.rect(
            specular_gfx::Rect::new(x0 + a, bottom - 1.0, b - a, 1.0),
            0.0,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    /// A field keeps its caret in view: text that fits sits at the start edge, longer
    /// Latin text scrolls left as the caret goes right, longer right-to-left text keeps its
    /// right end in view and scrolls right as the caret goes left.
    #[test]
    fn fields_scroll_to_the_caret_in_both_directions() {
        use super::scroll_x0;
        let room = specular_gfx::Rect::new(10.0, 0.0, 100.0, 20.0);
        assert_eq!(scroll_x0(40.0, 40.0, room, false, false), 10.0);
        assert_eq!(scroll_x0(40.0, 0.0, room, true, true), 70.0);
        // 300 wide, Latin, caret at the end: the end in view.
        let x0 = scroll_x0(300.0, 300.0, room, false, false);
        assert!(x0 + 300.0 <= room.right() && x0 + 300.0 > room.right() - 3.0);
        // Latin, caret at the start: not scrolled.
        assert_eq!(scroll_x0(300.0, 0.0, room, true, false), 10.0);
        // Hebrew, caret at its start (the right end): right-aligned.
        assert_eq!(
            scroll_x0(300.0, 300.0, room, true, true),
            room.right() - 300.0
        );
        // Hebrew, caret at its end (the left end): scrolled so it shows.
        let x0 = scroll_x0(300.0, 0.0, room, true, true);
        assert!(x0 >= room.x && x0 <= room.x + 3.0);
    }

    #[test]
    fn a_composition_sits_at_the_caret() {
        use super::compose;
        assert_eq!(compose("ab", 1, "日本"), ("a日本b".to_owned(), 1..7));
        assert_eq!(compose("ab", 9, "x"), ("abx".to_owned(), 2..3));
        assert_eq!(compose("", 0, ""), (String::new(), 0..0));
        // Never inside a character.
        assert_eq!(compose("é", 1, "k"), ("ké".to_owned(), 0..1));
    }

    use super::*;

    fn k(n: NamedKey) -> Key {
        Key::Named(n)
    }

    const NONE: Mods = Mods::NONE;
    const SHIFT: Mods = Mods {
        shift: true,
        ..Mods::NONE
    };
    const CTRL: Mods = Mods {
        ctrl: true,
        ..Mods::NONE
    };
    const CTRL_SHIFT: Mods = Mods {
        ctrl: true,
        shift: true,
        ..Mods::NONE
    };

    fn key(e: &mut LineEdit, key: Key, m: Mods, clip: &mut MemoryClipboard) -> Handled {
        e.key(key, m, false, clip)
    }

    #[test]
    fn shift_arrows_home_and_end_select() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("camera:x-t5 beach");
        assert_eq!(e.caret(), e.text().len());
        key(&mut e, k(NamedKey::ArrowLeft), SHIFT, &mut c);
        key(&mut e, k(NamedKey::ArrowLeft), SHIFT, &mut c);
        assert_eq!(e.selected_text(), "ch");
        key(&mut e, k(NamedKey::ArrowLeft), CTRL_SHIFT, &mut c);
        assert_eq!(e.selected_text(), "beach");
        key(&mut e, k(NamedKey::Home), SHIFT, &mut c);
        assert_eq!(e.selected_text(), e.text());
        assert_eq!((e.anchor(), e.caret()), (e.text().len(), 0));
        // Without Shift the selection collapses to its edge.
        assert_eq!(
            key(&mut e, k(NamedKey::ArrowRight), NONE, &mut c),
            Handled::Used
        );
        assert_eq!((e.caret(), e.has_selection()), (e.text().len(), false));
        key(&mut e, k(NamedKey::Home), NONE, &mut c);
        key(&mut e, k(NamedKey::ArrowRight), CTRL_SHIFT, &mut c);
        assert_eq!(e.selected_text(), "camera");
        key(&mut e, k(NamedKey::End), SHIFT, &mut c);
        assert_eq!(e.selected_text(), e.text());
        // Ctrl+A.
        key(&mut e, k(NamedKey::Home), NONE, &mut c);
        assert_eq!(key(&mut e, Key::Char('a'), CTRL, &mut c), Handled::Used);
        assert_eq!(e.selection(), 0..e.text().len());
        // Keys that are not the field's.
        for (key_, m) in [
            (k(NamedKey::ArrowUp), NONE),
            (k(NamedKey::Enter), NONE),
            (k(NamedKey::Escape), NONE),
            (Key::Char('a'), Mods { alt: true, ..NONE }),
            (Key::Char('k'), CTRL),
        ] {
            assert_eq!(e.key(key_, m, false, &mut c), Handled::No, "{key_:?}");
        }
    }

    #[test]
    fn arrows_follow_right_to_left_text() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("שלום");
        e.set_caret(0, false);
        // In right-to-left text Left goes forward (towards the end of the text).
        e.key(k(NamedKey::ArrowLeft), SHIFT, true, &mut c);
        assert_eq!(e.selected_text(), "ש");
        e.key(k(NamedKey::ArrowLeft), CTRL_SHIFT, true, &mut c);
        assert_eq!(e.selected_text(), "שלום");
        e.key(k(NamedKey::ArrowRight), NONE, true, &mut c);
        assert_eq!((e.caret(), e.has_selection()), (0, false));
    }

    #[test]
    fn typing_and_pasting_replace_the_selection() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("rating>=3 beach");
        e.select(10..15);
        assert!(e.insert("x"));
        assert_eq!(e.text(), "rating>=3 x");
        assert_eq!(e.caret(), 11);
        e.select(0..6);
        c.text = Some("iso:\r\n100\n\tlens\u{7}".into());
        assert_eq!(key(&mut e, Key::Char('v'), CTRL, &mut c), Handled::Edited);
        // One line: CRLF, LF and tabs are spaces, other control characters gone.
        assert_eq!(e.text(), "iso: 100  lens>=3 x");
        assert!(!e.has_selection());
        // Shift+Insert pastes too; an empty clipboard does nothing.
        e.set_caret(0, false);
        c.text = Some("a ".into());
        assert_eq!(
            key(&mut e, k(NamedKey::Insert), SHIFT, &mut c),
            Handled::Edited
        );
        assert!(e.text().starts_with("a iso:"));
        c.text = None;
        assert_eq!(key(&mut e, Key::Char('v'), CTRL, &mut c), Handled::Used);
        assert_eq!(single_line("a\u{2028}b\u{85}c"), "a b c");
        // A clipboard read later: the key asks, the text comes to `paste`.
        let mut p = PendingClipboard::default();
        let before = e.text().to_owned();
        assert_eq!(e.key(Key::Char('v'), CTRL, false, &mut p), Handled::Used);
        assert!(p.paste && e.text() == before);
        e.select_all();
        e.key(Key::Char('c'), CTRL, false, &mut p);
        assert_eq!(p.copied.as_deref(), Some(before.as_str()));
        // Control characters typed are dropped.
        assert!(!e.insert("\u{8}"));
    }

    #[test]
    fn clipboard_copy_cut_and_their_other_keys() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("one two three");
        // Nothing selected: nothing copied.
        assert_eq!(key(&mut e, Key::Char('c'), CTRL, &mut c), Handled::Used);
        assert_eq!(c.text, None);
        e.select(4..7);
        key(&mut e, Key::Char('c'), CTRL, &mut c);
        assert_eq!(c.text.as_deref(), Some("two"));
        e.select(0..3);
        key(&mut e, k(NamedKey::Insert), CTRL, &mut c);
        assert_eq!(c.text.as_deref(), Some("one"));
        // Ctrl+X and Shift+Delete cut.
        e.select(8..13);
        assert_eq!(key(&mut e, Key::Char('X'), CTRL, &mut c), Handled::Edited);
        assert_eq!((e.text(), c.text.as_deref()), ("one two ", Some("three")));
        e.select(0..4);
        assert_eq!(
            key(&mut e, k(NamedKey::Delete), SHIFT, &mut c),
            Handled::Edited
        );
        assert_eq!((e.text(), c.text.as_deref()), ("two ", Some("one ")));
        // Shift+Delete without a selection deletes the next character, copying nothing.
        e.set_caret(0, false);
        key(&mut e, k(NamedKey::Delete), SHIFT, &mut c);
        assert_eq!((e.text(), c.text.as_deref()), ("wo ", Some("one ")));
        // Undo brings back each cut.
        assert!(e.undo() && e.undo());
        assert_eq!(e.text(), "one two ");
        assert_eq!(
            e.selected_text(),
            "one ",
            "the selection comes back with the text"
        );
    }

    #[test]
    fn undo_and_redo_keys() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("");
        for ch in "iso:100 beach".chars() {
            e.insert(&ch.to_string());
        }
        assert_eq!(key(&mut e, Key::Char('z'), CTRL, &mut c), Handled::Edited);
        assert_eq!(e.text(), "iso:100");
        key(&mut e, Key::Char('Z'), CTRL_SHIFT, &mut c);
        assert_eq!(e.text(), "iso:100 beach");
        key(&mut e, Key::Char('z'), CTRL, &mut c);
        key(&mut e, Key::Char('y'), CTRL, &mut c);
        assert_eq!(e.text(), "iso:100 beach");
        // Nothing to redo: used, unchanged.
        assert_eq!(key(&mut e, Key::Char('y'), CTRL, &mut c), Handled::Used);
        // A change from outside (a pick from a list) is a step of its own.
        e.set_text("iso:200 beach");
        assert_eq!(e.caret(), e.text().len());
        e.insert("!");
        e.undo();
        assert_eq!(e.text(), "iso:200 beach");
        e.undo();
        assert_eq!(e.text(), "iso:100 beach");
        // Setting the same text is no step.
        e.set_text("iso:100 beach");
        e.undo();
        assert_eq!(e.text(), "iso:100");
        // A reset forgets the history.
        e.reset("x");
        assert!(!e.can_undo() && e.text() == "x" && e.caret() == 1);
    }

    #[test]
    fn deleting_words_and_terms() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("camera:x-t5 beach");
        e.set_caret(11, false);
        key(&mut e, k(NamedKey::Backspace), CTRL, &mut c);
        assert_eq!(e.text(), "camera:x- beach", "a word");
        let mut e = LineEdit::new("camera:x-t5 beach").with_terms();
        e.set_caret(11, false);
        key(&mut e, k(NamedKey::Backspace), CTRL, &mut c);
        assert_eq!(e.text(), " beach", "a whole term");
        key(&mut e, k(NamedKey::Delete), CTRL, &mut c);
        assert_eq!(e.text(), "");
        e.undo();
        assert_eq!(e.text(), " beach");
        // A selection goes first.
        let mut e = LineEdit::new("abc def").with_terms();
        e.select(1..2);
        key(&mut e, k(NamedKey::Backspace), CTRL, &mut c);
        assert_eq!(e.text(), "ac def");
    }

    #[test]
    fn clicks_drags_double_and_triple_clicks() {
        let mut e = LineEdit::new("camera:x-t5 beach");
        e.press(3, 30.0, 10.0, false);
        e.release();
        assert_eq!((e.caret(), e.has_selection()), (3, false));
        // A drag selects from the press.
        e.press(2, 20.0, 11.0, false);
        assert!(e.dragging());
        e.drag_to(9);
        assert_eq!(e.selected_text(), "mera:x-");
        e.drag_to(0);
        assert_eq!(e.selected_text(), "ca");
        assert_eq!(e.caret(), 0);
        e.release();
        e.drag_to(17);
        assert_eq!(e.selected_text(), "ca", "no drag after the release");
        // Shift+click extends.
        e.press(6, 60.0, 12.0, true);
        assert_eq!(e.selected_text(), "mera", "from the anchor the drag left");
        // Double click: the word; a drag then goes by words.
        e.press(13, 130.0, 20.0, false);
        e.press(13, 131.0, 20.2, false);
        assert_eq!(e.selected_text(), "beach");
        e.drag_to(8);
        assert_eq!(e.selected_text(), "x-t5 beach".replace("x-", "-"));
        e.release();
        // Punctuation and spaces are runs of their own.
        assert_eq!(&e.text()[e.word_at(6)], ":");
        assert_eq!(&e.text()[e.word_at(11)], " ");
        assert_eq!(
            &e.text()[e.word_at(17)],
            "beach",
            "at the end: the last word"
        );
        // Triple click: everything.
        e.press(3, 30.0, 30.0, false);
        e.press(3, 30.0, 30.1, false);
        e.press(3, 30.0, 30.2, false);
        assert_eq!(e.selected_text(), e.text());
        // Too slow, or too far: a single click again.
        e.press(3, 30.0, 40.0, false);
        e.press(3, 30.0, 41.0, false);
        assert!(!e.has_selection());
        e.press(3, 30.0, 42.0, false);
        e.press(4, 40.0, 42.1, false);
        assert!(!e.has_selection());
        // Hebrew words.
        let mut e = LineEdit::new("tag:\"שלום עולם\"");
        let at = e.text().find('ע').unwrap() + 2;
        e.press(at, 0.0, 0.0, false);
        e.press(at, 0.0, 0.1, false);
        assert_eq!(e.selected_text(), "עולם");
    }

    #[test]
    fn primary_selection_copy_on_select_and_middle_click() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("camera:x-t5 beach");
        // Nothing selected: nothing offered.
        assert!(!e.offer_primary(&mut c));
        // A drag offers once it ends, not while it goes.
        e.press(0, 0.0, 1.0, false);
        e.drag_to(6);
        assert!(!e.offer_primary(&mut c));
        assert_eq!(c.primary, None);
        e.release();
        assert!(e.offer_primary(&mut c));
        assert_eq!(c.primary.as_deref(), Some("camera"));
        // The same selection is offered once; a changed one again.
        assert!(!e.offer_primary(&mut c));
        key(&mut e, k(NamedKey::ArrowRight), SHIFT, &mut c);
        assert!(e.offer_primary(&mut c));
        assert_eq!(c.primary.as_deref(), Some("camera:"));
        // The selection let go: the primary keeps its text (and copying is not cutting).
        key(&mut e, k(NamedKey::End), NONE, &mut c);
        assert!(!e.offer_primary(&mut c));
        assert_eq!(c.primary.as_deref(), Some("camera:"));
        assert_eq!(c.text, None, "the clipboard is not the primary selection");
        // Selected again, the same text is offered again (another program may own it now).
        e.select(0..7);
        assert!(e.offer_primary(&mut c));
        // A middle click pastes the primary selection where it lands, the selection let
        // go, as one undoable line.
        c.primary = Some("iso:100\n".into());
        assert_eq!(e.paste_primary_at(12, &mut c), Handled::Edited);
        assert_eq!(e.text(), "camera:x-t5 iso:100 beach");
        assert_eq!((e.caret(), e.has_selection()), (20, false));
        e.undo();
        assert_eq!(e.text(), "camera:x-t5 beach");
        // Nothing on it: only the caret moves.
        c.primary = None;
        assert_eq!(e.paste_primary_at(3, &mut c), Handled::Used);
        assert_eq!((e.text(), e.caret()), ("camera:x-t5 beach", 3));
        // Read later: the click asks, the text comes to `paste` at the caret.
        let mut p = PendingClipboard::default();
        assert_eq!(e.paste_primary_at(6, &mut p), Handled::Used);
        assert!(p.paste_primary && !p.paste);
        assert!(e.paste(" pro"));
        assert_eq!(e.text(), "camera pro:x-t5 beach");
        e.select(0..6);
        e.offer_primary(&mut p);
        assert_eq!(p.primary.as_deref(), Some("camera"));
        assert_eq!(p.copied, None);
    }

    /// A held Shift+Arrow offers the selection once: on the key's release, or once the keys
    /// have rested for `PRIMARY_IDLE_S`; a drag's release still offers at once.
    #[test]
    fn keyboard_selection_is_offered_once_the_keys_rest() {
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("camera:x-t5 beach");
        e.set_caret(0, false);
        // Held: each repeat selects one more character and puts the offer off.
        for i in 0..6 {
            let now = 1.0 + 0.03 * i as f64;
            key(&mut e, k(NamedKey::ArrowRight), SHIFT, &mut c);
            e.offer_primary_later(now);
            assert!(e.primary_pending());
            assert!(!e.offer_primary_due(now, &mut c));
            assert_eq!(c.primary, None, "not at every step");
        }
        let wake = e.primary_wake(1.2).unwrap();
        assert!(
            (wake - (1.15 + PRIMARY_IDLE_S - 1.2)).abs() < 1e-9,
            "{wake}"
        );
        // The release offers what was selected, once.
        assert!(e.offer_primary(&mut c));
        assert_eq!(c.primary.as_deref(), Some("camera"));
        assert!(!e.primary_pending() && e.primary_wake(2.0).is_none());
        // Without a release, the offer comes after the keys rest.
        key(&mut e, k(NamedKey::ArrowRight), SHIFT, &mut c);
        e.offer_primary_later(3.0);
        assert!(!e.offer_primary_due(3.0 + PRIMARY_IDLE_S - 0.01, &mut c));
        assert!(e.offer_primary_due(3.0 + PRIMARY_IDLE_S, &mut c));
        assert_eq!(c.primary.as_deref(), Some("camera:"));
        assert!(!e.offer_primary_due(9.0, &mut c), "offered once");
        // A key that leaves the selection as offered (or lets it go) waits for nothing.
        key(&mut e, k(NamedKey::Home), CTRL, &mut c);
        e.offer_primary_later(10.0);
        assert!(!e.primary_pending());
        key(&mut e, k(NamedKey::End), SHIFT, &mut c);
        e.offer_primary_later(10.1);
        key(&mut e, k(NamedKey::ArrowRight), NONE, &mut c);
        e.offer_primary_later(10.2);
        assert!(
            !e.primary_pending(),
            "let go before it was due: nothing to offer"
        );
        // A drag offers on its release whatever the keys left waiting.
        key(&mut e, k(NamedKey::Home), SHIFT, &mut c);
        e.offer_primary_later(11.0);
        e.press(0, 0.0, 11.1, false);
        e.drag_to(3);
        assert!(!e.offer_primary(&mut c) && !e.primary_pending());
        e.release();
        assert!(e.offer_primary(&mut c));
        assert_eq!(c.primary.as_deref(), Some("cam"));
        assert_eq!(c.text, None, "the clipboard is not the primary selection");
    }

    /// A selection the program made (a find match) passed over is not offered, not on a key's
    /// release nor once keys rest; changed by the user, or let go and made again, it is.
    #[test]
    fn a_selection_passed_over_waits_for_the_user() {
        let mut c = MemoryClipboard::default();
        let mut p = PrimaryOffer::default();
        let text = "initial_zoom = 1";
        p.offer(0..7, &text[0..7], false, &mut c);
        assert_eq!(c.primary.as_deref(), Some("initial"));
        // A key left one waiting; the match drops it.
        p.later(0..8, &text[0..8], false, 1.0);
        assert!(p.pending());
        p.pass_over(8..12, &text[8..12]);
        assert!(!p.pending());
        assert!(!p.offer(8..12, &text[8..12], false, &mut c));
        p.later(8..12, &text[8..12], false, 2.0);
        assert!(!p.pending() && !p.due(9.0));
        assert_eq!(c.primary.as_deref(), Some("initial"), "kept");
        // Grown by the user: offered.
        p.later(8..13, &text[8..13], false, 3.0);
        assert!(p.pending());
        assert!(p.offer(8..13, &text[8..13], false, &mut c));
        assert_eq!(c.primary.as_deref(), Some("zoom "));
        // Passed over again, let go, then selected by the user: offered.
        p.pass_over(8..12, &text[8..12]);
        assert!(!p.offer(8..12, &text[8..12], false, &mut c));
        assert!(!p.offer(12..12, "", false, &mut c));
        assert!(p.offer(8..12, &text[8..12], false, &mut c));
        assert_eq!(c.primary.as_deref(), Some("zoom"));
        assert_eq!(c.text, None, "the clipboard is not the primary selection");
    }

    fn digit(c: char) -> Option<char> {
        c.is_ascii_digit().then_some(c)
    }

    fn number(t: &str) -> bool {
        t.len() <= 4 && !t.starts_with('0')
    }

    /// A field with a filter keeps to it whatever comes in: typed, pasted, pasted with a
    /// middle click; what it refuses leaves the selection alone.
    #[test]
    fn a_filter_keeps_what_is_typed_and_pasted_to_it() {
        const F: Filter = Filter {
            map: digit,
            valid: number,
        };
        let mut c = MemoryClipboard::default();
        let mut e = LineEdit::new("").with_filter(F);
        assert!(e.insert("0a1b2"), "a leading zero and letters left out");
        assert_eq!(e.text(), "12");
        assert!(!e.insert("x"));
        // Inserted at the caret, as long as the text stays valid.
        e.set_caret(1, false);
        assert!(e.insert("9876"));
        assert_eq!((e.text(), e.caret()), ("1982", 3), "two of four fit");
        // A paste keeps its digits; nothing it takes leaves the selection alone.
        e.select(1..3);
        c.text = Some("4-5".into());
        assert_eq!(key(&mut e, Key::Char('v'), CTRL, &mut c), Handled::Edited);
        assert_eq!(e.text(), "1452");
        e.select(0..1);
        c.text = Some("none".into());
        assert_eq!(key(&mut e, Key::Char('v'), CTRL, &mut c), Handled::Used);
        assert_eq!((e.text(), e.selected_text()), ("1452", "1"));
        // A zero can't lead even over a selection.
        assert!(!e.paste("0"));
        assert!(e.paste("7"));
        assert_eq!(e.text(), "7452");
        // A middle click's paste too.
        c.primary = Some("3 3".into());
        e.select_all();
        e.key(k(NamedKey::Backspace), NONE, false, &mut c);
        assert_eq!(e.paste_primary_at(0, &mut c), Handled::Edited);
        assert_eq!(e.text(), "33");
        // The filter stays after a reset.
        e.reset("");
        assert!(!e.insert("0") && e.insert("5"));
    }

    #[test]
    fn accesskit_selection_is_in_characters() {
        let mut e = LineEdit::new("אב cd");
        e.select(2..6);
        assert_eq!(e.a11y_selection(), (1, 4));
        e.set_a11y_selection(5, 0);
        assert_eq!((e.anchor(), e.caret()), (e.text().len(), 0));
        e.set_a11y_selection(99, 99);
        assert_eq!((e.caret(), e.has_selection()), (e.text().len(), false));
        // A new field has its caret at the end.
        assert_eq!(LineEdit::new("ab").a11y_selection(), (2, 2));
        assert_eq!(LineEdit::new("a\nb").text(), "a b");
    }
}

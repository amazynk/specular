//! A plain-text buffer for the built-in editor: text with line starts, a caret and
//! selection, and undo/redo that groups typing into words.

use std::ops::Range;

/// What kind of change an edit was, for grouping undo steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// Typing: consecutive characters on one line form one undo step.
    Type,
    /// Backspace or Delete: consecutive deletions form one step.
    Delete,
    /// Paste, cut, indent, a newline… always its own step.
    Other,
}

#[derive(Clone, Debug)]
struct Edit {
    at: usize,
    removed: String,
    inserted: String,
    /// (anchor, caret) before and after.
    before: (usize, usize),
    after: (usize, usize),
    kind: EditKind,
}

/// The text being edited.
#[derive(Clone, Debug)]
pub struct Buffer {
    text: String,
    /// Byte offset of the start of every line (the first is 0).
    starts: Vec<usize>,
    caret: usize,
    anchor: usize,
    /// The column (in chars) up/down movement aims for.
    goal: Option<usize>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// The next edit starts a new undo step even if it could join the last one.
    seal: bool,
    /// Bumped on every change of the text.
    version: u64,
    /// First line changed since [`take_dirty`](Self::take_dirty).
    dirty: Option<usize>,
}

const UNDO_LIMIT: usize = 2000;

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Buffer {
    pub fn new(text: &str) -> Buffer {
        Buffer {
            text: text.to_owned(),
            starts: line_starts(text),
            caret: 0,
            anchor: 0,
            goal: None,
            undo: Vec::new(),
            redo: Vec::new(),
            seal: true,
            version: 0,
            dirty: Some(0),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// The first line changed since the last call (for re-lexing), if any.
    pub fn take_dirty(&mut self) -> Option<usize> {
        self.dirty.take()
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// The line holding byte `at`.
    pub fn line_of(&self, at: usize) -> usize {
        self.starts.partition_point(|&s| s <= at).saturating_sub(1)
    }

    /// Byte range of line `l` without its line break.
    pub fn line_range(&self, l: usize) -> Range<usize> {
        let l = l.min(self.starts.len() - 1);
        let start = self.starts[l];
        let end = self.starts.get(l + 1).map_or(self.text.len(), |&n| n - 1);
        // A "\r\n" file: the "\r" is not shown.
        let end = if end > start && self.text.as_bytes()[end - 1] == b'\r' {
            end - 1
        } else {
            end
        };
        start..end
    }

    pub fn line(&self, l: usize) -> &str {
        &self.text[self.line_range(l)]
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// (line, column in bytes) of the caret.
    pub fn caret_line_col(&self) -> (usize, usize) {
        let l = self.line_of(self.caret);
        (l, self.caret - self.starts[l])
    }

    /// The selection, start before end (empty when there is none).
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    fn clamp(&self, at: usize) -> usize {
        let mut at = at.min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// Puts the caret at `at`; with `extend` the selection grows from the anchor.
    pub fn set_caret(&mut self, at: usize, extend: bool) {
        self.caret = self.clamp(at);
        if !extend {
            self.anchor = self.caret;
        }
        self.goal = None;
        self.seal = true;
    }

    /// Selects `range` with the caret at its end.
    pub fn select(&mut self, range: Range<usize>) {
        self.anchor = self.clamp(range.start);
        self.caret = self.clamp(range.end);
        self.goal = None;
        self.seal = true;
    }

    pub fn select_all(&mut self) {
        self.select(0..self.text.len());
    }

    // ----- Movement ------------------------------------------------------

    fn prev_char(&self, at: usize) -> usize {
        self.text[..at]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next_char(&self, at: usize) -> usize {
        self.text[at..]
            .chars()
            .next()
            .map_or(at, |c| at + c.len_utf8())
    }

    /// One character left (or to the start of the selection).
    pub fn left(&mut self, extend: bool) {
        let at = if !extend && self.has_selection() {
            self.selection().start
        } else {
            self.prev_char(self.caret)
        };
        self.set_caret(at, extend);
    }

    pub fn right(&mut self, extend: bool) {
        let at = if !extend && self.has_selection() {
            self.selection().end
        } else {
            self.next_char(self.caret)
        };
        self.set_caret(at, extend);
    }

    /// The start of the word before `at` (Ctrl+Left).
    pub fn word_left_of(&self, at: usize) -> usize {
        let mut i = at;
        let mut chars = self.text[..at].char_indices().rev().peekable();
        // Spaces, then a run of the same class.
        while let Some(&(j, c)) = chars.peek() {
            if c.is_whitespace() && c != '\n' || (c == '\n' && j + 1 == i && i == at) {
                i = j;
                chars.next();
                if c == '\n' {
                    return i;
                }
            } else {
                break;
            }
        }
        let class = chars.peek().map(|&(_, c)| is_word(c));
        while let Some(&(j, c)) = chars.peek() {
            if c.is_whitespace() || Some(is_word(c)) != class {
                break;
            }
            i = j;
            chars.next();
        }
        i
    }

    /// The end of the word after `at` (Ctrl+Right).
    pub fn word_right_of(&self, at: usize) -> usize {
        let mut i = at;
        let mut chars = self.text[at..]
            .char_indices()
            .map(|(j, c)| (at + j, c))
            .peekable();
        while let Some(&(j, c)) = chars.peek() {
            if c == '\n' {
                return if j == at { j + 1 } else { j };
            }
            if !c.is_whitespace() {
                break;
            }
            i = j + c.len_utf8();
            chars.next();
        }
        let class = chars.peek().map(|&(_, c)| is_word(c));
        while let Some(&(j, c)) = chars.peek() {
            if c.is_whitespace() || Some(is_word(c)) != class {
                break;
            }
            i = j + c.len_utf8();
            chars.next();
        }
        i
    }

    pub fn word_left(&mut self, extend: bool) {
        let at = self.word_left_of(self.caret);
        self.set_caret(at, extend);
    }

    pub fn word_right(&mut self, extend: bool) {
        let at = self.word_right_of(self.caret);
        self.set_caret(at, extend);
    }

    /// Home: the first non-blank of the line, or the line start if already there.
    pub fn home(&mut self, extend: bool) {
        let l = self.line_of(self.caret);
        let r = self.line_range(l);
        let indent = r.start + self.text[r.clone()].len() - self.text[r.clone()].trim_start().len();
        let at = if self.caret == indent {
            r.start
        } else {
            indent
        };
        self.set_caret(at, extend);
    }

    pub fn end(&mut self, extend: bool) {
        let at = self.line_range(self.line_of(self.caret)).end;
        self.set_caret(at, extend);
    }

    pub fn doc_start(&mut self, extend: bool) {
        self.set_caret(0, extend);
    }

    pub fn doc_end(&mut self, extend: bool) {
        self.set_caret(self.text.len(), extend);
    }

    /// Moves `lines` up (negative) or down, keeping the column it started from.
    pub fn vertical(&mut self, lines: isize, extend: bool) {
        let (l, _) = self.caret_line_col();
        let r = self.line_range(l);
        let col = self
            .goal
            .unwrap_or_else(|| self.text[r.start..self.caret].chars().count());
        let target = l as isize + lines;
        let at = if target < 0 {
            0
        } else if target as usize >= self.line_count() {
            self.text.len()
        } else {
            let r = self.line_range(target as usize);
            let line = &self.text[r.clone()];
            r.start + line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
        };
        self.set_caret(at, extend);
        self.goal = Some(col);
    }

    // ----- Editing -----------------------------------------------------

    /// Replaces `range` with `s` as one edit, with the caret after the inserted text.
    pub fn replace(&mut self, range: Range<usize>, s: &str, kind: EditKind) {
        let range = self.clamp(range.start)..self.clamp(range.end);
        if range.is_empty() && s.is_empty() {
            return;
        }
        let before = (self.anchor, self.caret);
        let removed = self.text[range.clone()].to_owned();
        self.splice(range.clone(), s);
        let caret = range.start + s.len();
        self.anchor = caret;
        self.caret = caret;
        self.goal = None;
        let edit = Edit {
            at: range.start,
            removed,
            inserted: s.to_owned(),
            before,
            after: (caret, caret),
            kind,
        };
        self.record(edit);
    }

    /// Types `s` over the selection.
    pub fn insert(&mut self, s: &str, kind: EditKind) {
        self.replace(self.selection(), s, kind);
    }

    /// Backspace: the selection, else the character before the caret.
    pub fn backspace(&mut self, word: bool) {
        let r = if self.has_selection() {
            self.selection()
        } else if word {
            self.word_left_of(self.caret)..self.caret
        } else {
            self.prev_char(self.caret)..self.caret
        };
        self.replace(r, "", EditKind::Delete);
    }

    /// Delete: the selection, else the character after the caret.
    pub fn delete(&mut self, word: bool) {
        let r = if self.has_selection() {
            self.selection()
        } else if word {
            self.caret..self.word_right_of(self.caret)
        } else {
            self.caret..self.next_char(self.caret)
        };
        self.replace(r, "", EditKind::Delete);
    }

    fn record(&mut self, edit: Edit) {
        self.redo.clear();
        let joins = !self.seal
            && self.undo.last().is_some_and(|last| {
                last.kind == edit.kind
                    && match edit.kind {
                        EditKind::Type => {
                            last.removed.is_empty()
                                && edit.removed.is_empty()
                                && last.at + last.inserted.len() == edit.at
                                && !edit.inserted.contains('\n')
                                // A new word starts a new step.
                                && !(edit.inserted.starts_with(char::is_whitespace)
                                    && !last.inserted.ends_with(char::is_whitespace))
                        }
                        EditKind::Delete => {
                            last.inserted.is_empty()
                                && edit.inserted.is_empty()
                                && (edit.at + edit.removed.len() == last.at || edit.at == last.at)
                        }
                        EditKind::Other => false,
                    }
            });
        if joins {
            let last = self.undo.last_mut().expect("checked");
            match edit.kind {
                EditKind::Type => last.inserted.push_str(&edit.inserted),
                _ if edit.at < last.at => {
                    // Backspace: the new text is in front.
                    last.removed.insert_str(0, &edit.removed);
                    last.at = edit.at;
                }
                _ => last.removed.push_str(&edit.removed),
            }
            last.after = edit.after;
        } else {
            self.undo.push(edit);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.seal = false;
    }

    /// Changes the text and line starts (no undo record).
    fn splice(&mut self, range: Range<usize>, s: &str) {
        let first = self.line_of(range.start);
        let last = self.line_of(range.end);
        self.text.replace_range(range.clone(), s);
        let delta = s.len() as isize - range.len() as isize;
        let mut starts: Vec<usize> = self.starts[..=first].to_vec();
        starts.extend(s.match_indices('\n').map(|(i, _)| range.start + i + 1));
        starts.extend(
            self.starts[last + 1..]
                .iter()
                .map(|&x| (x as isize + delta) as usize),
        );
        self.starts = starts;
        self.version += 1;
        self.dirty = Some(self.dirty.map_or(first, |d| d.min(first)));
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(e) = self.undo.pop() else {
            return false;
        };
        self.splice(e.at..e.at + e.inserted.len(), &e.removed);
        (self.anchor, self.caret) = e.before;
        self.goal = None;
        self.redo.push(e);
        self.seal = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(e) = self.redo.pop() else {
            return false;
        };
        self.splice(e.at..e.at + e.removed.len(), &e.inserted);
        (self.anchor, self.caret) = e.after;
        self.goal = None;
        self.undo.push(e);
        self.seal = true;
        true
    }

    /// Replaces the whole text (reload), keeping the caret on the same line and column where
    /// possible, as one undoable step.
    pub fn replace_all(&mut self, text: &str) {
        if text == self.text {
            return;
        }
        let (l, c) = self.caret_line_col();
        self.seal = true;
        self.replace(0..self.text.len(), text, EditKind::Other);
        self.seal = true;
        let l = l.min(self.line_count() - 1);
        let r = self.line_range(l);
        self.set_caret(r.start + c.min(r.len()), false);
    }

    /// Indentation of line `l` (its leading spaces and tabs).
    pub fn indent_of(&self, l: usize) -> &str {
        let line = self.line(l);
        &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
    }

    /// The first line and one past the last line the selection touches (a selection ending
    /// at the start of a line leaves that line out).
    pub fn selected_lines(&self) -> Range<usize> {
        let s = self.selection();
        let first = self.line_of(s.start);
        let mut last = self.line_of(s.end);
        if last > first && self.starts[last] == s.end {
            last -= 1;
        }
        first..last + 1
    }

    /// Adds `unit` in front of every selected line (Tab with a multi-line selection).
    pub fn indent_lines(&mut self, unit: &str) {
        self.edit_lines(|line| format!("{unit}{line}"));
    }

    /// Removes up to `width` leading spaces (or one tab) from every selected line.
    pub fn outdent_lines(&mut self, width: usize) {
        self.edit_lines(|line| {
            let n = if line.starts_with('\t') {
                1
            } else {
                line.len() - line.trim_start_matches(' ').len()
            };
            line[n.min(width)..].to_owned()
        });
    }

    fn edit_lines(&mut self, f: impl Fn(&str) -> String) {
        let lines = self.selected_lines();
        let start = self.starts[lines.start];
        let end = self.line_range(lines.end - 1).end;
        let new: Vec<String> = (lines.clone()).map(|l| f(self.line(l))).collect();
        let joined = new.join("\n");
        if joined == self.text[start..end] {
            return;
        }
        let (a, c) = (self.anchor, self.caret);
        let was_selection = a != c;
        self.seal = true;
        self.replace(start..end, &joined, EditKind::Other);
        self.seal = true;
        if was_selection || lines.len() > 1 {
            self.select(start..start + joined.len());
        } else {
            // Keep the caret where it was relative to the line's text.
            let delta = joined.len() as isize - (end - start) as isize;
            let at = (c as isize + delta).max(start as isize) as usize;
            self.set_caret(at, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines() {
        let b = Buffer::new("a\nbc\r\n\nd");
        assert_eq!(b.line_count(), 4);
        assert_eq!(b.line(0), "a");
        assert_eq!(b.line(1), "bc");
        assert_eq!(b.line(2), "");
        assert_eq!(b.line(3), "d");
        assert_eq!(b.line_of(0), 0);
        assert_eq!(b.line_of(2), 1);
        assert_eq!(b.line_of(b.text().len()), 3);
        assert_eq!(Buffer::new("").line_count(), 1);
    }

    #[test]
    fn line_starts_follow_edits() {
        let mut b = Buffer::new("one\ntwo\nthree");
        b.set_caret(5, false);
        b.insert("X\nY", EditKind::Other);
        assert_eq!(b.text(), "one\ntX\nYwo\nthree");
        assert_eq!(b.starts, line_starts(b.text()));
        b.select(2..9);
        b.insert("", EditKind::Delete);
        assert_eq!(b.starts, line_starts(b.text()));
        assert_eq!(b.take_dirty(), Some(0));
        assert_eq!(b.take_dirty(), None);
        b.set_caret(b.text().len(), false);
        b.insert("\n", EditKind::Other);
        assert_eq!(b.take_dirty(), Some(b.line_count() - 2));
        assert_eq!(b.starts, line_starts(b.text()));
    }

    #[test]
    fn typing_undoes_by_word() {
        let mut b = Buffer::new("");
        for c in "zoom = fit".chars() {
            b.insert(&c.to_string(), EditKind::Type);
        }
        assert_eq!(b.text(), "zoom = fit");
        assert!(b.undo());
        assert_eq!(b.text(), "zoom =");
        assert!(b.undo());
        assert_eq!(b.text(), "zoom");
        assert!(b.undo());
        assert_eq!(b.text(), "");
        assert!(!b.undo());
        assert!(b.redo() && b.redo() && b.redo());
        assert_eq!(b.text(), "zoom = fit");
        assert_eq!(b.caret(), 10);
        assert!(!b.redo());
    }

    #[test]
    fn moving_the_caret_starts_a_new_step() {
        let mut b = Buffer::new("");
        b.insert("a", EditKind::Type);
        b.insert("b", EditKind::Type);
        b.left(false);
        b.right(false);
        b.insert("c", EditKind::Type);
        b.undo();
        assert_eq!(b.text(), "ab");
        // A new edit clears redo.
        b.insert("d", EditKind::Type);
        assert!(!b.can_redo());
    }

    #[test]
    fn deleting() {
        let mut b = Buffer::new("hello world");
        b.set_caret(11, false);
        for _ in 0..5 {
            b.backspace(false);
        }
        assert_eq!(b.text(), "hello ");
        b.undo();
        assert_eq!(b.text(), "hello world");
        assert_eq!(b.caret(), 11);
        b.set_caret(0, false);
        b.delete(true);
        assert_eq!(b.text(), " world");
        b.set_caret(6, false);
        b.backspace(true);
        assert_eq!(b.text(), " ");
        // Selections go first; multi-byte characters stay whole.
        let mut b = Buffer::new("aé日b");
        b.set_caret(b.text().len() - 1, false);
        b.backspace(false);
        assert_eq!(b.text(), "aéb");
        b.select(0..3);
        b.delete(false);
        assert_eq!(b.text(), "b");
    }

    #[test]
    fn words_home_end_and_lines() {
        let mut b = Buffer::new("  key_one = \"x\"\nnext");
        b.set_caret(0, false);
        b.word_right(false);
        assert_eq!(b.caret(), 9);
        b.word_right(false);
        assert_eq!(b.caret(), 11);
        b.word_left(false);
        assert_eq!(b.caret(), 10);
        b.word_left(false);
        assert_eq!(b.caret(), 2);
        b.end(false);
        assert_eq!(b.caret(), 15);
        b.home(false);
        assert_eq!(b.caret(), 2);
        b.home(false);
        assert_eq!(b.caret(), 0);
        b.home(true);
        assert_eq!(b.caret(), 2);
        assert_eq!(b.selected_text(), "  ");
        // Up and down keep the column, through a shorter line.
        let mut b = Buffer::new("abcdef\nab\nabcdef");
        b.set_caret(5, false);
        b.vertical(1, false);
        assert_eq!(b.caret_line_col(), (1, 2));
        b.vertical(1, false);
        assert_eq!(b.caret_line_col(), (2, 5));
        b.vertical(5, true);
        assert_eq!(b.caret(), b.text().len());
        b.vertical(-10, false);
        assert_eq!(b.caret(), 0);
        b.select_all();
        assert_eq!(b.selected_text(), b.text());
    }

    #[test]
    fn indenting_lines() {
        let mut b = Buffer::new("a\nb\nc");
        b.select(0..3);
        b.indent_lines("    ");
        assert_eq!(b.text(), "    a\n    b\nc");
        b.outdent_lines(4);
        assert_eq!(b.text(), "a\nb\nc");
        b.undo();
        assert_eq!(b.text(), "    a\n    b\nc");
        b.set_caret(b.text().len(), false);
        b.outdent_lines(4);
        assert_eq!(b.text(), "    a\n    b\nc");
        assert_eq!(b.indent_of(0), "    ");
    }

    #[test]
    fn replace_all_keeps_the_caret_line() {
        let mut b = Buffer::new("a\nbbbb\nc");
        b.set_caret(5, false);
        b.replace_all("x\nyy\nz\n");
        assert_eq!(b.caret_line_col(), (1, 2));
        assert!(b.undo());
        assert_eq!(b.text(), "a\nbbbb\nc");
    }
}

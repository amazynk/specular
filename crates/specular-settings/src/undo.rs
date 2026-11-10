//! Undo/redo for changes made in the Settings window.

use std::time::{Duration, Instant};

use crate::value::Value;

/// One key's change.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    pub key: &'static str,
    pub before: Value,
    pub after: Value,
}

/// One undoable step (a single change, or a group such as "Reset section").
#[derive(Debug, Clone)]
pub(crate) struct Step {
    pub edits: Vec<Edit>,
    at: Instant,
    group: bool,
}

/// Rapid changes to the same key (dragging a slider, typing a colour) within this window are
/// merged into one step.
const COALESCE: Duration = Duration::from_millis(1000);
const LIMIT: usize = 200;

#[derive(Debug, Default)]
pub(crate) struct UndoStack {
    undo: Vec<Step>,
    redo: Vec<Step>,
    open_group: Option<Vec<Edit>>,
}

impl UndoStack {
    pub fn record(&mut self, edit: Edit) {
        if let Some(g) = &mut self.open_group {
            merge_into(g, edit);
            return;
        }
        self.redo.clear();
        let now = Instant::now();
        if let Some(last) = self.undo.last_mut()
            && !last.group
            && last.edits.len() == 1
            && last.edits[0].key == edit.key
            && now.duration_since(last.at) < COALESCE
        {
            last.edits[0].after = edit.after;
            last.at = now;
            if last.edits[0].before == last.edits[0].after {
                self.undo.pop();
            }
            return;
        }
        self.push(Step {
            edits: vec![edit],
            at: now,
            group: false,
        });
    }

    fn push(&mut self, step: Step) {
        self.undo.push(step);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn begin_group(&mut self) {
        self.open_group.get_or_insert_with(Vec::new);
    }

    pub fn end_group(&mut self) {
        if let Some(edits) = self.open_group.take() {
            let edits: Vec<Edit> = edits.into_iter().filter(|e| e.before != e.after).collect();
            if !edits.is_empty() {
                self.redo.clear();
                self.push(Step {
                    edits,
                    at: Instant::now(),
                    group: true,
                });
            }
        }
    }

    pub fn pop_undo(&mut self) -> Option<Step> {
        let s = self.undo.pop()?;
        self.redo.push(s.clone());
        Some(s)
    }

    pub fn pop_redo(&mut self) -> Option<Step> {
        let s = self.redo.pop()?;
        self.undo.push(s.clone());
        Some(s)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// The keys the next undo (or, with `redo`, redo) would change; empty when there is none.
    pub fn next_keys(&self, redo: bool) -> Vec<&'static str> {
        let stack = if redo { &self.redo } else { &self.undo };
        stack
            .last()
            .map(|s| s.edits.iter().map(|e| e.key).collect())
            .unwrap_or_default()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

fn merge_into(group: &mut Vec<Edit>, edit: Edit) {
    match group.iter_mut().find(|e| e.key == edit.key) {
        Some(e) => e.after = edit.after,
        None => group.push(edit),
    }
}

//! History: an ordered list of steps with before and after values, walked by undo and redo.
//!
//! Every step is invertible on its own, so any position can be reached from the current edit
//! without keeping full copies. Slider drags merge into one step.
//!
//! - [`History::compress`] drops superseded steps (later changes to the same thing fold into
//!   one, changes that cancel go); the edit and the redo tail stay as they were.
//! - [`Change::Stack`] swaps the whole stack in one step: a paste, a style, a snapshot
//!   restored, a match to reference.
//! - Memory: [`BUDGET`] bytes of steps per edit; [`History::trim`] drops the oldest beyond it.

use super::edit::{Edit, ModuleInstance};
use super::fields::{Fields, Value};
use super::kind::ModuleKind;
use super::mask::{Blend, MaskShape};

/// Param changes to the same module within this many milliseconds merge into one step.
pub const MERGE_MS: i64 = 1_000;

/// The most a history keeps (its encoded size, bytes): about a thousand ordinary steps or a
/// few dozen whole-stack ones. Older steps go first.
pub const BUDGET: usize = 512 * 1024;

/// The kind a whole-stack step names (no module).
pub const WHOLE_EDIT: ModuleKind = ModuleKind(0);

/// The module stack as a whole: what a [`Change::Stack`] swaps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stack {
    pub modules: Vec<ModuleInstance>,
    pub masks: Vec<MaskShape>,
    pub custom_order: bool,
}

impl Stack {
    pub fn of(e: &Edit) -> Self {
        Self {
            modules: e.modules.clone(),
            masks: e.masks.clone(),
            custom_order: e.custom_order,
        }
    }

    fn put(&self, e: &mut Edit) {
        e.modules.clone_from(&self.modules);
        e.masks.clone_from(&self.masks);
        e.custom_order = self.custom_order;
    }
}

/// One field before and after (`None` = absent).
#[derive(Clone, Debug, PartialEq)]
pub struct FieldChange {
    pub name: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// What a step changed.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// Named fields of the target's params.
    Params(Vec<FieldChange>),
    /// Turned on (`true`) or off.
    Enabled(bool),
    Blend {
        before: Option<Box<Blend>>,
        after: Option<Box<Blend>>,
    },
    Label {
        before: Option<String>,
        after: Option<String>,
    },
    /// A whole instance swapped (version updates, presets that change many fields).
    Replace {
        before: Box<ModuleInstance>,
        after: Box<ModuleInstance>,
    },
    /// Added at a stack index.
    Add {
        at: u32,
        module: Box<ModuleInstance>,
    },
    /// Removed from a stack index.
    Remove {
        at: u32,
        module: Box<ModuleInstance>,
    },
    /// Moved in the stack; `custom_before` restores the order flag on undo.
    Move {
        from: u32,
        to: u32,
        custom_before: bool,
    },
    /// The shared mask shapes, before and after.
    Masks {
        before: Vec<MaskShape>,
        after: Vec<MaskShape>,
    },
    /// The whole stack swapped; `what` names it in the history list ("Paste", a style's
    /// name…).
    Stack {
        what: String,
        before: Box<Stack>,
        after: Box<Stack>,
    },
}

/// One history step.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// The module the step is about (shown in the history list).
    pub kind: ModuleKind,
    pub instance: u16,
    pub change: Change,
    /// Milliseconds since the Unix epoch.
    pub time_ms: i64,
}

impl Step {
    /// A params step from `before` to `after` fields of one module (only changed fields).
    pub fn params(
        kind: ModuleKind,
        instance: u16,
        before: &Fields,
        after: &Fields,
        time_ms: i64,
    ) -> Self {
        let mut changes = Vec::new();
        for (n, a) in &after.0 {
            let b = before.get(n);
            if b != Some(a) {
                changes.push(FieldChange {
                    name: n.clone(),
                    before: b.cloned(),
                    after: Some(a.clone()),
                });
            }
        }
        for (n, b) in &before.0 {
            if after.get(n).is_none() {
                changes.push(FieldChange {
                    name: n.clone(),
                    before: Some(b.clone()),
                    after: None,
                });
            }
        }
        Self {
            kind,
            instance,
            change: Change::Params(changes),
            time_ms,
        }
    }
}

impl Step {
    /// A whole-stack step from `before` to `after` (only the stacks are compared).
    pub fn stack(what: &str, before: &Edit, after: &Edit, time_ms: i64) -> Self {
        Self {
            kind: WHOLE_EDIT,
            instance: 0,
            change: Change::Stack {
                what: what.to_owned(),
                before: Box::new(Stack::of(before)),
                after: Box::new(Stack::of(after)),
            },
            time_ms,
        }
    }

    /// Whether applying the step changes nothing.
    pub fn is_noop(&self) -> bool {
        match &self.change {
            Change::Params(c) => c.iter().all(|c| c.before == c.after),
            Change::Blend { before, after } => before == after,
            Change::Label { before, after } => before == after,
            Change::Replace { before, after } => before == after,
            Change::Move { from, to, .. } => from == to,
            Change::Masks { before, after } => before == after,
            Change::Stack { before, after, .. } => before == after,
            Change::Enabled(_) | Change::Add { .. } | Change::Remove { .. } => false,
        }
    }

    /// Whether the step only changes its own target's params, switch, blend or label: such
    /// steps commute with every step on another target.
    fn local(&self) -> bool {
        matches!(
            self.change,
            Change::Params(_) | Change::Enabled(_) | Change::Blend { .. } | Change::Label { .. }
        )
    }

    /// Folds `next` (same target and kind of change, applied after this one with only
    /// commuting steps between) into this step; false when the two cannot be one.
    fn absorb(&mut self, next: &Step) -> bool {
        match (&mut self.change, &next.change) {
            (Change::Params(old), Change::Params(new)) => {
                for c in new {
                    match old.iter_mut().find(|o| o.name == c.name) {
                        Some(o) => o.after.clone_from(&c.after),
                        None => old.push(c.clone()),
                    }
                }
                old.retain(|c| c.before != c.after);
            }
            (Change::Blend { after, .. }, Change::Blend { after: a, .. }) => after.clone_from(a),
            (Change::Label { after, .. }, Change::Label { after: a, .. }) => after.clone_from(a),
            (Change::Masks { after, .. }, Change::Masks { after: a, .. }) => after.clone_from(a),
            (
                Change::Stack { after, what, .. },
                Change::Stack {
                    after: a, what: w, ..
                },
            ) => {
                after.clone_from(a);
                what.clone_from(w);
            }
            _ => return false,
        }
        self.time_ms = next.time_ms;
        true
    }

    /// The step's size as stored (bytes).
    pub fn bytes(&self) -> usize {
        super::codec::encode_step(self).len()
    }
}

/// Why a step did not apply (the edit does not match the history).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryError {
    NoSuchModule(ModuleKind, u16),
    BadIndex(u32),
}

/// The ordered steps and the current position (`position` steps are applied).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    pub steps: Vec<Step>,
    pub position: usize,
}

impl History {
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    pub fn can_redo(&self) -> bool {
        self.position < self.steps.len()
    }

    /// Applies `step` to `edit` and records it, dropping any redo tail. A params step on the
    /// same module as the last one within [`MERGE_MS`] merges into it.
    pub fn push(&mut self, edit: &mut Edit, step: Step) -> Result<(), HistoryError> {
        apply(edit, &step, true)?;
        self.steps.truncate(self.position);
        if let Some(last) = self.steps.last_mut()
            && last.kind == step.kind
            && last.instance == step.instance
            && (0..=MERGE_MS).contains(&(step.time_ms - last.time_ms))
            && let (Change::Params(old), Change::Params(new)) = (&mut last.change, &step.change)
        {
            for c in new {
                match old.iter_mut().find(|o| o.name == c.name) {
                    Some(o) => o.after = c.after.clone(),
                    None => old.push(c.clone()),
                }
            }
            last.time_ms = step.time_ms;
            return Ok(());
        }
        // A control that turned its module on as it moved (one step): the rest of the move
        // joins it.
        if let Some(last) = self.steps.last_mut()
            && last.kind == step.kind
            && last.instance == step.instance
            && (0..=MERGE_MS).contains(&(step.time_ms - last.time_ms))
            && let (Change::Replace { before, after }, Change::Params(new)) =
                (&mut last.change, &step.change)
            && before.enabled != after.enabled
        {
            let mut f = after.params.to_fields();
            for c in new {
                match &c.after {
                    Some(v) => f.set(&c.name, v.clone()),
                    None => {
                        f.remove(&c.name);
                    }
                }
            }
            after.params.set_fields(f);
            last.time_ms = step.time_ms;
            return Ok(());
        }
        self.steps.push(step);
        self.position = self.steps.len();
        Ok(())
    }

    pub fn undo(&mut self, edit: &mut Edit) -> Result<bool, HistoryError> {
        if !self.can_undo() {
            return Ok(false);
        }
        apply(edit, &self.steps[self.position - 1], false)?;
        self.position -= 1;
        Ok(true)
    }

    pub fn redo(&mut self, edit: &mut Edit) -> Result<bool, HistoryError> {
        if !self.can_redo() {
            return Ok(false);
        }
        apply(edit, &self.steps[self.position], true)?;
        self.position += 1;
        Ok(true)
    }

    /// Walks to `position` (clamped), undoing or redoing as needed.
    pub fn go_to(&mut self, edit: &mut Edit, position: usize) -> Result<(), HistoryError> {
        let target = position.min(self.steps.len());
        while self.position > target {
            self.undo(edit)?;
        }
        while self.position < target {
            self.redo(edit)?;
        }
        Ok(())
    }

    /// "Go back here": drops the steps after the current position.
    pub fn truncate(&mut self) {
        self.steps.truncate(self.position);
    }

    /// Forgets every step: the edit stays as it is and becomes the start.
    pub fn clear(&mut self) {
        self.steps.clear();
        self.position = 0;
    }

    /// Drops superseded steps among those applied: a later change to the same fields,
    /// switch, blend, label, shapes or whole stack folds into the earlier step when only
    /// steps it commutes with lie between, a switch turned on and off again goes, and steps
    /// that end up changing nothing go. The edit, the start and the redo tail stay as they
    /// were. Returns how many steps went.
    pub fn compress(&mut self) -> usize {
        fn masks(s: &Step) -> bool {
            matches!(s.change, Change::Masks { .. })
        }
        let before = self.steps.len();
        let tail = self.steps.split_off(self.position);
        let mut out: Vec<Step> = Vec::with_capacity(self.steps.len());
        for step in std::mem::take(&mut self.steps) {
            let mut into = None;
            for (i, o) in out.iter().enumerate().rev() {
                let same_kind =
                    std::mem::discriminant(&o.change) == std::mem::discriminant(&step.change);
                // The shared shapes are one target, whichever module a step names.
                let same_target = masks(o) || o.kind == step.kind && o.instance == step.instance;
                if same_kind && same_target {
                    into = Some(i);
                    break;
                }
                // Steps on params, switches, blends, labels and shapes commute unless they
                // touch the same thing (handled above).
                let commutes = (o.local() || masks(o)) && (step.local() || masks(&step));
                if !commutes {
                    break;
                }
            }
            let Some(i) = into else {
                if !step.is_noop() {
                    out.push(step);
                }
                continue;
            };
            if let (Change::Enabled(a), Change::Enabled(b)) = (&out[i].change, &step.change) {
                if a != b {
                    out.remove(i);
                } else {
                    out.push(step);
                }
                continue;
            }
            if !out[i].absorb(&step) {
                out.push(step);
            } else if out[i].is_noop() {
                out.remove(i);
            }
        }
        self.position = out.len();
        out.extend(tail);
        self.steps = out;
        before - self.steps.len()
    }

    /// The steps' size as stored (bytes).
    pub fn bytes(&self) -> usize {
        self.steps.iter().map(Step::bytes).sum()
    }

    /// Keeps the history within `budget` bytes: the oldest applied steps go first (the start
    /// moves up to the oldest kept), then the far end of the redo tail. Returns how many
    /// went.
    pub fn trim(&mut self, budget: usize) -> usize {
        let sizes: Vec<usize> = self.steps.iter().map(Step::bytes).collect();
        let mut total: usize = sizes.iter().sum();
        let mut front = 0;
        while total > budget && front < self.position {
            total -= sizes[front];
            front += 1;
        }
        let mut back = self.steps.len();
        while total > budget && back > self.position {
            back -= 1;
            total -= sizes[back];
        }
        let gone = front + self.steps.len() - back;
        self.steps.truncate(back);
        self.steps.drain(..front);
        self.position -= front;
        gone
    }
}

fn find(
    edit: &mut Edit,
    kind: ModuleKind,
    instance: u16,
) -> Result<&mut ModuleInstance, HistoryError> {
    edit.module_mut(kind, instance)
        .ok_or(HistoryError::NoSuchModule(kind, instance))
}

/// Applies a step forwards or backwards.
fn apply(edit: &mut Edit, step: &Step, forward: bool) -> Result<(), HistoryError> {
    fn pick<T>(forward: bool, b: T, a: T) -> T {
        if forward { a } else { b }
    }
    match &step.change {
        Change::Params(changes) => {
            let m = find(edit, step.kind, step.instance)?;
            let mut f = m.params.to_fields();
            for c in changes {
                match pick(forward, &c.before, &c.after) {
                    Some(v) => f.set(&c.name, v.clone()),
                    None => {
                        f.remove(&c.name);
                    }
                }
            }
            m.params.set_fields(f);
        }
        Change::Enabled(on) => find(edit, step.kind, step.instance)?.enabled = *on == forward,
        Change::Blend { before, after } => {
            find(edit, step.kind, step.instance)?.blend =
                pick(forward, before, after).as_deref().cloned();
        }
        Change::Label { before, after } => {
            find(edit, step.kind, step.instance)?.label = pick(forward, before, after).clone();
        }
        Change::Replace { before, after } => {
            let (from, to) = pick(forward, (after, before), (before, after));
            let m = find(edit, from.kind, from.instance)?;
            *m = (**to).clone();
        }
        Change::Add { at, module } | Change::Remove { at, module } => {
            let adding = matches!(step.change, Change::Add { .. }) == forward;
            let i = *at as usize;
            if adding {
                if i > edit.modules.len() {
                    return Err(HistoryError::BadIndex(*at));
                }
                edit.modules.insert(i, (**module).clone());
            } else {
                if i >= edit.modules.len() {
                    return Err(HistoryError::BadIndex(*at));
                }
                edit.modules.remove(i);
            }
        }
        Change::Move {
            from,
            to,
            custom_before,
        } => {
            let (a, b) = pick(forward, (*to, *from), (*from, *to));
            let n = edit.modules.len();
            if a as usize >= n || b as usize >= n {
                return Err(HistoryError::BadIndex(a.max(b)));
            }
            let m = edit.modules.remove(a as usize);
            edit.modules.insert(b as usize, m);
            edit.custom_order = if forward { true } else { *custom_before };
        }
        Change::Masks { before, after } => edit.masks = pick(forward, before, after).clone(),
        Change::Stack { before, after, .. } => pick(forward, before, after).put(edit),
    }
    Ok(())
}

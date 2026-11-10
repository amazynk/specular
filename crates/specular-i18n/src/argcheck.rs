//! Test support: the arguments a message is formatted with, against the ones its English
//! text uses. `message_args.rs` checks the calls whose message id and argument names can be
//! read from the source; a call that picks its id at run time (an error's `message_id()`)
//! is checked by a test that formats every case while this records on its thread.

use std::cell::RefCell;
use std::collections::BTreeSet;

use fluent_bundle::FluentArgs;

thread_local! {
    /// Problems seen on this thread while recording (None: not recording).
    static FOUND: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// What is wrong with formatting `id` with arguments named `names`: one the English message
/// always uses missing, or one it never uses passed. None when they agree, and for ids
/// English does not have or with an attribute (`message_ids.rs` checks those).
pub fn argument_problem<'a>(id: &str, names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    if id.contains('.') {
        return None;
    }
    let want = crate::catalog::english_arguments(id)?;
    let names: BTreeSet<&str> = names.into_iter().collect();
    let missing: Vec<&String> = want
        .required
        .iter()
        .filter(|a| !names.contains(a.as_str()))
        .collect();
    let unused: Vec<&&str> = names
        .iter()
        .filter(|a| !want.required.contains(**a) && !want.optional.contains(**a))
        .collect();
    (!missing.is_empty() || !unused.is_empty())
        .then(|| format!("{id}: missing {missing:?}, not in the message {unused:?}"))
}

/// Starts (or stops) recording the argument problems of every message formatted with
/// arguments on this thread; starting forgets what was recorded.
#[doc(hidden)]
pub fn record_argument_problems(on: bool) {
    FOUND.with(|f| *f.borrow_mut() = on.then(Vec::new));
}

/// The problems recorded on this thread so far (each once), which are then forgotten.
#[doc(hidden)]
pub fn argument_problems() -> Vec<String> {
    FOUND.with(|f| {
        let mut f = f.borrow_mut();
        let mut v = f.as_mut().map(std::mem::take).unwrap_or_default();
        v.sort();
        v.dedup();
        v
    })
}

/// Called for every message formatted with arguments.
pub(crate) fn note(id: &str, args: &FluentArgs<'_>) {
    FOUND.with(|f| {
        let mut f = f.borrow_mut();
        if let Some(found) = f.as_mut()
            && let Some(p) = argument_problem(id, args.iter().map(|(k, _)| k))
        {
            found.push(p);
        }
    });
}

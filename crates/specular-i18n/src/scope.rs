//! Test support: strings made in English and drawn in another language.
//!
//! A screenshot in Hebrew whose texts were made before [`crate::I18n::with_thread_language`]
//! took effect shows English in a right-to-left layout, and a golden blessed that way tests
//! nothing about the translation. While [`trace`] is on, every string a localizer makes in
//! English is remembered with its message id; [`strays`] then names the strings drawn in
//! another language that are such English strings although that language translates them.
//! The screenshot harness checks every shot this way.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use fluent_syntax::ast;
use unic_langid::LanguageIdentifier;

use crate::catalog::{self, Catalogue};

static TRACE: AtomicBool = AtomicBool::new(false);

/// English strings made while tracing, with the ids that made them.
fn made() -> &'static Mutex<HashMap<String, BTreeSet<String>>> {
    static MADE: OnceLock<Mutex<HashMap<String, BTreeSet<String>>>> = OnceLock::new();
    MADE.get_or_init(Default::default)
}

/// Remembers the English strings made from now on (process-wide; for tests), and every
/// English message that takes no arguments: a test's English literal equal to one of them
/// (a label typed into a fixture) counts as made in English too.
#[doc(hidden)]
pub fn trace(on: bool) {
    TRACE.store(on, Ordering::Relaxed);
    static SEEDED: OnceLock<()> = OnceLock::new();
    if on {
        SEEDED.get_or_init(|| {
            let en = crate::Localizer::english();
            let vars = catalog::english_variables_all();
            for id in catalog::english_ids() {
                if !is_symbol(id)
                    && vars.get(id).is_some_and(BTreeSet::is_empty)
                    && catalog::english_arguments(id)
                        .is_some_and(|a| a.required.is_empty() && a.optional.is_empty())
                {
                    en.tr(id);
                }
            }
        });
    }
}

/// A unit symbol's message (`unit-symbol-km`): its English text is the symbol, which is
/// also data (a file's own unit, a key's letter), so it is never taken for a stray.
fn is_symbol(id: &str) -> bool {
    id.starts_with("unit-symbol-")
}

/// A localizer in `lang` made `out` from message `id`.
pub(crate) fn note(lang: &LanguageIdentifier, id: &str, out: &str) {
    if !TRACE.load(Ordering::Relaxed)
        || lang.language.as_str() != "en"
        || out.is_empty()
        || is_symbol(id)
    {
        return;
    }
    if let Ok(mut m) = made().lock() {
        m.entry(out.to_owned()).or_default().insert(id.to_owned());
    }
}

/// The strings of `texts` (each whole, or a part of it between " · ", ", " or a line break)
/// that were made in English from messages that `lang` translates: they were made outside
/// `lang`'s scope. Each as "text (message id)". Empty for English.
#[doc(hidden)]
pub fn strays(texts: &[String], lang: &LanguageIdentifier) -> Vec<String> {
    if lang.language.as_str() == "en" {
        return Vec::new();
    }
    let Some(cat) = catalog::find(lang).or_else(|| {
        let base = LanguageIdentifier::from_parts(lang.language, None, None, &[]);
        catalog::find(&base)
    }) else {
        return Vec::new();
    };
    let Ok(made) = made().lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for t in texts {
        let parts = std::iter::once(t.as_str()).chain(
            t.split(" · ")
                .flat_map(|p| p.split(", "))
                .flat_map(|p| p.split('\n')),
        );
        for p in parts {
            let Some(ids) = made.get(p) else { continue };
            if ids.iter().all(|id| translated(cat, id)) {
                let s = format!(
                    "{p:?} ({})",
                    ids.iter().cloned().collect::<Vec<_>>().join(", ")
                );
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}

/// Whether `cat` has message (or attribute) `id`, worded otherwise than in English: no
/// wording of it (each variant of a selection) is the same in both.
fn translated(cat: &Catalogue, id: &str) -> bool {
    let (msg, attr) = id.split_once('.').map_or((id, None), |(m, a)| (m, Some(a)));
    match (
        wordings(cat, msg, attr),
        wordings(catalog::english(), msg, attr),
    ) {
        (Some(t), Some(e)) => t.is_disjoint(&e),
        _ => false,
    }
}

/// Message `msg`'s value (or attribute `attr`) in `cat` as comparable strings: the whole
/// pattern, or each variant of its selections.
fn wordings(cat: &Catalogue, msg: &str, attr: Option<&str>) -> Option<BTreeSet<String>> {
    let p = cat
        .resources
        .iter()
        .flat_map(|r| r.entries())
        .find_map(|e| match e {
            ast::Entry::Message(m) if m.id.name == msg => match attr {
                None => m.value.as_ref(),
                Some(a) => m
                    .attributes
                    .iter()
                    .find(|x| x.id.name == a)
                    .map(|x| &x.value),
            },
            _ => None,
        })?;
    let mut out = BTreeSet::new();
    leaves(p, &mut out);
    Some(out)
}

fn leaves(p: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    let mut selected = false;
    for el in &p.elements {
        if let ast::PatternElement::Placeable {
            expression: ast::Expression::Select { variants, .. },
        } = el
        {
            selected = true;
            for v in variants {
                leaves(&v.value, out);
            }
        }
    }
    if !selected {
        out.insert(format!("{p:?}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Localizer, langid};

    #[test]
    fn english_strings_drawn_in_hebrew_are_found() {
        trace(true);
        let en = Localizer::new(&[langid!("en-US")], None);
        let he = Localizer::new(&[langid!("he")], None);
        let english = en.tr("app-cancel");
        let hebrew = he.tr("app-cancel");
        assert_ne!(english, hebrew, "app-cancel is translated into Hebrew");
        let found = strays(&[english.clone(), hebrew.clone()], &langid!("he"));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with(&format!("{english:?} (")) && found[0].contains("app-cancel"));
        // Parts of a joined string too; nothing in English.
        let joined = format!("{hebrew} · {english}");
        assert_eq!(strays(&[joined], &langid!("he")).len(), 1);
        assert!(strays(&[english], &langid!("en-US")).is_empty());
        // A unit symbol is data too (a file's unit, a key's letter): "m" is never a stray.
        assert_eq!(en.tr("unit-symbol-m"), "m");
        assert!(strays(&["m".to_owned()], &langid!("he")).is_empty());
    }
}

//! `text:` in the Library: photos by the words read in them (the Local AI pack's Text,
//! [`super::text_index`]). A `text:exit` or `text:"open daily"` term is lifted out of the
//! query on the Library's search worker, every word looked up in the text found (compared
//! case- and accent-folded, as the rest of the search), and the photos holding all of them
//! narrow the query's other terms, most matches first. Nothing here runs on the UI thread
//! except [`ready`], an atomic load.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use specular_library::{Library, Search};
use specular_query::{AiFeature, ErrorKind, Scope};

/// Photos found by text at most.
const CAP: usize = 5000;

static READY: AtomicBool = AtomicBool::new(false);

/// Whether `text:` can run (the query bar then hides its "needs the Local AI pack").
pub(crate) fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

/// Set by the text worker: whether the Text feature is installed.
pub(crate) fn set_ready(on: bool) {
    READY.store(on, Ordering::Relaxed);
}

/// The `text:` terms of a Library query: their words, and the query without them. None
/// without any.
pub(crate) fn split_text(query: &str) -> Option<(Vec<String>, String)> {
    let q = specular_query::parse_in(query, Scope::Library);
    let mut spans: Vec<(usize, usize)> = q
        .errors
        .iter()
        .filter(|e| e.kind == ErrorKind::NeedsAiPack(AiFeature::Text))
        .map(|e| (e.span.start, e.span.end))
        .filter(|(s, e)| s < e && *e <= query.len())
        .collect();
    if spans.is_empty() {
        return None;
    }
    spans.sort_unstable();
    let mut words = Vec::new();
    let mut rest = String::new();
    let mut at = 0;
    for (s, e) in spans {
        if s < at || !query.is_char_boundary(s) || !query.is_char_boundary(e) {
            continue;
        }
        rest.push_str(&query[at..s]);
        let term = &query[s..e];
        let value = term.split_once(':').map_or("", |(_, v)| v);
        words.extend(
            value
                .trim()
                .trim_matches('"')
                .split_whitespace()
                .map(str::to_owned),
        );
        at = e;
    }
    rest.push_str(&query[at..]);
    (!words.is_empty()).then(|| (words, rest.split_whitespace().collect::<Vec<_>>().join(" ")))
}

fn fold(s: &str) -> String {
    specular_i18n::fold_for_search(s).into_owned()
}

/// A Library search with its `text:` terms resolved (and removed from the query); when the
/// search was already ranked (by `like:`), only its photos that hold the words stay, in its
/// order. The request as it was, and false, without any or before Text is ready.
pub(crate) fn with_text(lib: &Library, req: &Search) -> (Search, bool) {
    if !ready() {
        return (req.clone(), false);
    }
    let Some((words, rest)) = split_text(&req.query) else {
        return (req.clone(), false);
    };
    let hits = match lib.find_text(&words, &fold, CAP) {
        Ok(h) => h.unwrap_or_default(),
        Err(e) => {
            specular_log::warn!(target: specular_log::JOBS, "ai: search by text: {e}");
            return (req.clone(), false);
        }
    };
    let mut out = req.clone();
    out.query = rest;
    out.ranked = Some(Arc::new(match &req.ranked {
        Some(r) => {
            let keep: std::collections::HashSet<i64> = hits.iter().map(|(id, _)| *id).collect();
            r.iter()
                .filter(|(id, _)| keep.contains(id))
                .copied()
                .collect()
        }
        None => hits,
    }));
    (out, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_terms_are_lifted_out_of_the_query() {
        assert_eq!(
            split_text(r#"text:"open daily" iso:100"#),
            Some((vec!["open".into(), "daily".into()], "iso:100".into()))
        );
        assert_eq!(
            split_text("rating:3 text:exit text:יציאה"),
            Some((vec!["exit".into(), "יציאה".into()], "rating:3".into()))
        );
        assert_eq!(split_text("iso:100 beach"), None);
        assert_eq!(split_text("text:"), None);
    }
}

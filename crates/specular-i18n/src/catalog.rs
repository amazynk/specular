//! Embedded Fluent catalogues, their message ids, and the consistency check used by tests
//! and CI (completeness and placeholder agreement per locale).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use fluent_bundle::FluentResource;
use fluent_syntax::ast;
use unic_langid::LanguageIdentifier;

include!(concat!(env!("OUT_DIR"), "/catalogues.rs"));

/// One locale's catalogue: all of its `.ftl` files, parsed.
pub(crate) struct Catalogue {
    pub langid: LanguageIdentifier,
    pub resources: Vec<FluentResource>,
    /// Message ids (not terms) defined by this catalogue.
    pub ids: BTreeSet<String>,
    /// Syntax errors, as "file: error" strings (should always be empty; tests check).
    pub errors: Vec<String>,
}

/// One slot per embedded catalogue, parsed on first use: only the languages in use are
/// parsed (all of them took about 56 MB).
static SLOTS: [OnceLock<Catalogue>; CATALOGUES.len()] =
    [const { OnceLock::new() }; CATALOGUES.len()];

/// Catalogue `i` of [`CATALOGUES`], parsed on first use.
fn catalogue(i: usize) -> &'static Catalogue {
    SLOTS[i].get_or_init(|| parse(CATALOGUES[i].0, CATALOGUES[i].1))
}

/// The catalogues parsed so far (tests: the app parses only the languages it uses).
#[doc(hidden)]
pub fn parsed_catalogues() -> Vec<LanguageIdentifier> {
    SLOTS
        .iter()
        .filter_map(|s| s.get().map(|c| c.langid.clone()))
        .collect()
}

fn parse(tag: &str, files: &[(&str, &'static str)]) -> Catalogue {
    let langid: LanguageIdentifier = tag.parse().expect("catalogue folder is a BCP 47 tag");
    let mut errors = Vec::new();
    let resources: Vec<FluentResource> = files
        .iter()
        .map(
            |(file, src)| match FluentResource::try_new((*src).to_owned()) {
                Ok(r) => r,
                Err((r, errs)) => {
                    for e in errs {
                        let msg = format!("{tag}/{file}: {e:?}");
                        specular_log::error!(target: specular_log::I18N, "Fluent syntax error in {msg}");
                        errors.push(msg);
                    }
                    r
                }
            },
        )
        .collect();
    let ids = resources
        .iter()
        .flat_map(|r| r.entries())
        .filter_map(|e| match e {
            ast::Entry::Message(m) => Some(m.id.name.to_owned()),
            _ => None,
        })
        .collect();
    Catalogue {
        langid,
        resources,
        ids,
        errors,
    }
}

/// Every catalogue, parsed (checks and tests: the app parses only those it uses).
pub(crate) fn catalogues() -> impl Iterator<Item = &'static Catalogue> {
    (0..CATALOGUES.len()).map(catalogue)
}

/// The English source catalogue.
pub(crate) fn english() -> &'static Catalogue {
    catalogue(0)
}

/// The position of exactly `langid` in [`CATALOGUES`] (nothing parsed).
fn index(langid: &LanguageIdentifier) -> Option<usize> {
    CATALOGUES.iter().position(|(tag, _)| {
        tag.parse::<LanguageIdentifier>()
            .is_ok_and(|t| &t == langid)
    })
}

/// The catalogue for exactly `langid`, parsed on first use.
pub(crate) fn find(langid: &LanguageIdentifier) -> Option<&'static Catalogue> {
    index(langid).map(catalogue)
}

/// Locales that have a catalogue (English first). Pseudo-locales are not included.
pub fn catalogue_locales() -> Vec<LanguageIdentifier> {
    CATALOGUES
        .iter()
        .map(|(tag, _)| tag.parse().expect("catalogue folder is a BCP 47 tag"))
        .collect()
}

/// All message ids of the English source catalogue.
pub fn english_ids() -> impl Iterator<Item = &'static str> {
    english().ids.iter().map(String::as_str)
}

/// True when the English catalogue defines message `id`.
pub fn english_has(id: &str) -> bool {
    english().ids.contains(id)
}

/// Fraction (0–1) of English message ids that `langid`'s own catalogue translates.
/// English and the pseudo-locales are complete; languages without a catalogue are 0.
pub fn completeness(langid: &LanguageIdentifier) -> f32 {
    if langid.language.as_str() == "en" || crate::pseudo::Pseudo::of(langid).is_some() {
        return 1.0;
    }
    // Counted when built (`TRANSLATED`): the language list parses no catalogue.
    let Some(i) = index(langid) else {
        return 0.0;
    };
    TRANSLATED[i] as f32 / TRANSLATED[0].max(1) as f32
}

/// Fraction of English message ids that `langid` translates, from its parsed catalogue
/// (checks the build-time count [`completeness`] uses).
#[cfg(test)]
pub(crate) fn completeness_parsed(langid: &LanguageIdentifier) -> f32 {
    let Some(c) = find(langid) else {
        return 0.0;
    };
    let en = &english().ids;
    let done = c.ids.iter().filter(|id| en.contains(*id)).count();
    done as f32 / en.len().max(1) as f32
}

/// An English message that one or more translations lack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untranslated {
    pub id: String,
    /// The English file that defines it (`app.ftl`); translations use the same file name.
    pub file: &'static str,
    /// Locales without a translation, in catalogue order.
    pub missing_in: Vec<LanguageIdentifier>,
}

/// English messages missing from at least one translation, in English file order. At run
/// time these fall back to English one by one; the workspace test reports them so a new id
/// is never shipped untranslated by accident.
pub fn untranslated() -> Vec<Untranslated> {
    let en = english();
    let files = CATALOGUES[0].1;
    let mut out = Vec::new();
    for ((file, _), r) in files.iter().zip(&en.resources) {
        for e in r.entries() {
            let ast::Entry::Message(m) = e else { continue };
            let missing_in: Vec<LanguageIdentifier> = catalogues()
                .skip(1)
                .filter(|c| !c.ids.contains(m.id.name))
                .map(|c| c.langid.clone())
                .collect();
            if !missing_in.is_empty() {
                out.push(Untranslated {
                    id: m.id.name.to_owned(),
                    file,
                    missing_in,
                });
            }
        }
    }
    out
}

/// A problem found by [`check_catalogues`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueIssue {
    /// A file failed to parse.
    Syntax { locale: String, error: String },
    /// A translation defines an id that English doesn't have (typo or stale id).
    UnknownId { locale: String, id: String },
    /// A translation uses different `$variables` from the English message.
    Placeholders {
        locale: String,
        id: String,
        english: Vec<String>,
        translated: Vec<String>,
    },
}

/// Checks every catalogue against English: syntax, unknown ids, and identical placeholder
/// sets. Missing translations are not issues (they fall back per message); see
/// [`completeness`].
pub fn check_catalogues() -> Vec<CatalogueIssue> {
    let en_vars = variables_by_id(english());
    let mut issues = Vec::new();
    for c in catalogues() {
        let locale = c.langid.to_string();
        for error in &c.errors {
            issues.push(CatalogueIssue::Syntax {
                locale: locale.clone(),
                error: error.clone(),
            });
        }
        if c.langid == english().langid {
            continue;
        }
        for (id, vars) in variables_by_id(c) {
            match en_vars.get(&id) {
                None => issues.push(CatalogueIssue::UnknownId {
                    locale: locale.clone(),
                    id,
                }),
                Some(en) if *en != vars => issues.push(CatalogueIssue::Placeholders {
                    locale: locale.clone(),
                    id,
                    english: en.iter().cloned().collect(),
                    translated: vars.into_iter().collect(),
                }),
                Some(_) => {}
            }
        }
    }
    issues
}

/// Variables (`$name`) used by `id` in English (value and attributes, selectors included).
pub fn english_variables(id: &str) -> Option<BTreeSet<String>> {
    variables_by_id(english()).remove(id)
}

/// Variables used by every English message, by id (see [`english_variables`]).
pub(crate) fn english_variables_all() -> BTreeMap<String, BTreeSet<String>> {
    variables_by_id(english())
}

/// The arguments English message `id` takes: those it always uses (`required`), and those
/// only some variants of a selection use (`optional`: a call that selects another variant may
/// leave them out). Messages it refers to (`{ other-message }`) count as its own.
pub fn english_arguments(id: &str) -> Option<Arguments> {
    let mut out = Arguments::default();
    let mut seen = BTreeSet::new();
    message_arguments(id, false, &mut out, &mut seen)?;
    out.optional.retain(|v| !out.required.contains(v));
    Some(out)
}

/// See [`english_arguments`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    pub required: BTreeSet<String>,
    pub optional: BTreeSet<String>,
}

fn message_arguments(
    id: &str,
    optional: bool,
    out: &mut Arguments,
    seen: &mut BTreeSet<String>,
) -> Option<()> {
    if !seen.insert(id.to_owned()) {
        return Some(());
    }
    let m = english()
        .resources
        .iter()
        .flat_map(|r| r.entries())
        .find_map(|e| match e {
            ast::Entry::Message(m) if m.id.name == id => Some(m),
            _ => None,
        })?;
    let mut refs = Vec::new();
    if let Some(p) = &m.value {
        pattern_args(p, optional, out, &mut refs);
    }
    for a in &m.attributes {
        pattern_args(&a.value, optional, out, &mut refs);
    }
    for (r, opt) in refs {
        message_arguments(&r, opt, out, seen);
    }
    Some(())
}

fn pattern_args(
    p: &ast::Pattern<&str>,
    optional: bool,
    out: &mut Arguments,
    refs: &mut Vec<(String, bool)>,
) {
    for el in &p.elements {
        if let ast::PatternElement::Placeable { expression } = el {
            match expression {
                ast::Expression::Inline(i) => inline_args(i, optional, out, refs),
                ast::Expression::Select { selector, variants } => {
                    inline_args(selector, optional, out, refs);
                    for v in variants {
                        pattern_args(&v.value, true, out, refs);
                    }
                }
            }
        }
    }
}

fn inline_args(
    i: &ast::InlineExpression<&str>,
    optional: bool,
    out: &mut Arguments,
    refs: &mut Vec<(String, bool)>,
) {
    let mut vars = BTreeSet::new();
    match i {
        ast::InlineExpression::MessageReference { id, .. } => {
            refs.push((id.name.to_owned(), optional));
            return;
        }
        ast::InlineExpression::Placeable { expression } => {
            let p = ast::Pattern {
                elements: vec![ast::PatternElement::Placeable {
                    expression: (**expression).clone(),
                }],
            };
            return pattern_args(&p, optional, out, refs);
        }
        _ => inline_vars(i, &mut vars),
    }
    if optional {
        out.optional.extend(vars);
    } else {
        out.required.extend(vars);
    }
}

fn variables_by_id(c: &Catalogue) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for r in &c.resources {
        for e in r.entries() {
            if let ast::Entry::Message(m) = e {
                let mut vars = BTreeSet::new();
                if let Some(p) = &m.value {
                    pattern_vars(p, &mut vars);
                }
                for a in &m.attributes {
                    pattern_vars(&a.value, &mut vars);
                }
                out.insert(m.id.name.to_owned(), vars);
            }
        }
    }
    out
}

fn pattern_vars(p: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for el in &p.elements {
        if let ast::PatternElement::Placeable { expression } = el {
            expression_vars(expression, out);
        }
    }
}

fn expression_vars(e: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match e {
        ast::Expression::Inline(i) => inline_vars(i, out),
        ast::Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for v in variants {
                pattern_vars(&v.value, out);
            }
        }
    }
}

fn inline_vars(i: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match i {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => call_vars(arguments, out),
        ast::InlineExpression::TermReference {
            arguments: Some(args),
            ..
        } => call_vars(args, out),
        ast::InlineExpression::Placeable { expression } => expression_vars(expression, out),
        _ => {}
    }
}

fn call_vars(args: &ast::CallArguments<&str>, out: &mut BTreeSet<String>) {
    for p in &args.positional {
        inline_vars(p, out);
    }
    for n in &args.named {
        inline_vars(&n.value, out);
    }
}

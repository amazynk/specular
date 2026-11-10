//! The language list for the picker: native and English names (from CLDR via ICU4X),
//! translation completeness and direction.

use icu_experimental::displaynames::multi::LocaleDisplayNamesFormatter;
use icu_experimental::displaynames::{DisplayNamesOptions, LanguageDisplay};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use unic_langid::LanguageIdentifier;

use crate::bidi::{self, Direction};
use crate::catalog;
use crate::format::icu_locale;
use crate::localizer::Localizer;
use crate::pseudo::{self, Pseudo};

/// One entry of the language picker.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageInfo {
    /// Language tag (`de`, `zh-Hans`, `en-XA`).
    pub id: LanguageIdentifier,
    /// Name in the language itself ("Deutsch", "العربية", "简体中文"). Show this one.
    pub native_name: String,
    /// Name in English ("German"), for search and as a secondary label.
    pub english_name: String,
    /// Fraction (0–1) of the English messages this language translates.
    pub completeness: f32,
    /// Writing direction.
    pub direction: Direction,
    /// True for the `en-XA` / `ar-XB` pseudo-locales (show them only in a developer section).
    pub pseudo: bool,
}

/// The name of `langid` written in `in_language`, with the first letter capitalised for
/// menus ("Français", "Deutsch (Österreich)"). Falls back to the tag.
///
/// Tags with a region use the standard form, "Português (Portugal)" rather than CLDR's
/// dialect name "Português europeu", so regional variants sort and read together; script
/// variants keep the dialect form ("简体中文").
///
/// Names are kept once made (a formatter takes a fraction of a millisecond to build, and the
/// settings draw the current language's name every frame).
pub fn language_name(langid: &LanguageIdentifier, in_language: &LanguageIdentifier) -> String {
    type Names = HashMap<(LanguageIdentifier, LanguageIdentifier), String>;
    static NAMES: Mutex<Option<Names>> = Mutex::new(None);
    let key = (langid.clone(), in_language.clone());
    if let Some(n) = NAMES
        .lock()
        .ok()
        .and_then(|m| m.as_ref().and_then(|m| m.get(&key).cloned()))
    {
        return n;
    }
    let n = make_language_name(langid, in_language);
    if let Ok(mut m) = NAMES.lock() {
        m.get_or_insert_with(HashMap::new).insert(key, n.clone());
    }
    n
}

fn make_language_name(langid: &LanguageIdentifier, in_language: &LanguageIdentifier) -> String {
    let display_locale = icu_locale(in_language);
    let mut options = DisplayNamesOptions::default();
    if langid.region.is_some() {
        options.language_display = LanguageDisplay::Standard;
    }
    let Ok(f) = LocaleDisplayNamesFormatter::try_new((&display_locale).into(), options) else {
        return langid.to_string();
    };
    let Ok(target) = icu_locale_core::Locale::try_from_str(&langid.to_string()) else {
        return langid.to_string();
    };
    capitalise_first(&f.of(&target))
}

fn capitalise_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// All languages with a catalogue, sorted by English name with English first, followed by
/// the two pseudo-locales. A copy of [`languages`].
pub fn available_languages() -> Vec<LanguageInfo> {
    languages().to_vec()
}

/// All languages with a catalogue, as [`available_languages`]. Made once, on first use: it
/// takes two display-name formatters and a completeness count per catalogue (milliseconds,
/// and the first use parses every catalogue).
pub fn languages() -> &'static [LanguageInfo] {
    static ALL: OnceLock<Vec<LanguageInfo>> = OnceLock::new();
    ALL.get_or_init(make_languages)
}

fn make_languages() -> Vec<LanguageInfo> {
    let en = unic_langid::langid!("en");
    let mut out: Vec<LanguageInfo> = catalog::catalogue_locales()
        .into_iter()
        .map(|id| LanguageInfo {
            native_name: language_name(&id, &id),
            english_name: language_name(&id, &en),
            completeness: catalog::completeness(&id),
            direction: bidi::direction(&id),
            pseudo: false,
            id,
        })
        .collect();
    out.sort_by(|a, b| {
        (a.id != en)
            .cmp(&(b.id != en))
            .then_with(|| a.english_name.cmp(&b.english_name))
    });
    let english = Localizer::english();
    for p in Pseudo::ALL {
        let english_name = match p {
            Pseudo::Accented => english.tr("language-pseudo-accented"),
            Pseudo::Bidi => english.tr("language-pseudo-rtl"),
        };
        let native_name = match p {
            Pseudo::Accented => pseudo::accent(&english_name).into_owned(),
            Pseudo::Bidi => pseudo::mirror(&english_name).into_owned(),
        };
        out.push(LanguageInfo {
            id: p.langid(),
            native_name,
            english_name,
            completeness: 1.0,
            direction: bidi::direction(&p.langid()),
            pseudo: true,
        });
    }
    out
}

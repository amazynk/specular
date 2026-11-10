//! Localisation for Specular.
//!
//! - **Catalogues**: Fluent `.ftl` files under `crates/specular-i18n/i18n/<locale>/`, embedded
//!   at build time. English (`en`, British spelling) is the source; every other locale falls
//!   back to English message by message. A missing id is never shown as empty text: the id
//!   itself is shown, and debug builds log it once.
//! - **Negotiation**: the OS languages (`sys-locale`) or the user's choice are matched to the
//!   catalogues with `fluent-langneg` (`de-AT` → `de` → `en`, `zh-CN` → `zh-Hans`).
//! - **Live switching**: [`I18n`] is a cheap `Arc` handle. [`I18n::set_language`] swaps the
//!   whole [`Localizer`] and bumps [`I18n::generation`].
//! - **Formatting**: numbers, percentages, byte sizes, dates, times and lists with ICU4X
//!   (CLDR data compiled in), and locale-aware natural ordering for file names.
//! - **Pseudo-locales**: `en-XA` (accented, about 40% longer) and `ar-XB` (forced RTL),
//!   generated from English at run time.
//!
//! # Message ids
//!
//! Ids are lowercase kebab-case and start with the UI area that owns them:
//! `toolbar-…`, `readout-…`, `status-…`, `palette-…`, `cheatsheet-…`, `error-…`, `toast-…`,
//! `empty-…`, `fullscreen-…`, `language-…`, `about-…`, `unit-…` and `settings-…`.
//! Settings use `settings-<section>-<key>`, `settings-<section>-<key>-help` and
//! `settings-<section>-<key>-<value>` for enum options, where key and value are the TOML
//! names with `_` written as `-` (`viewing.wheel_behaviour = "navigate"` →
//! `settings-viewing-wheel-behaviour-navigate`). Variables are `$snake_case`. Pass numbers
//! as numbers (not pre-formatted strings) so plural selection works and ICU4X formats them.
//! Catalogues never write a percent sign: `{ PERCENT($percent) }` (a number of percent) and
//! `{ PERCENT(100) }` give each language its pattern ("45%", "45 %", "%45"), and decimals in
//! text go through `{ NUMBER(2.5) }`. `tests/catalogue_text.rs` enforces this and flags
//! translations left identical to English (allowed words: `tests/same-as-english.txt`).
//!
//! # UI integration
//!
//! - **Generation counter.** Cache translated, shaped text keyed by
//!   [`I18n::generation`]; when it changes, re-translate and re-shape everything visible
//!   (including accessibility names), and mirror the layout if [`I18n::direction`] changed.
//! - **RTL mirroring.** When [`Direction::Rtl`]: lay out the toolbar and panels from right to
//!   left, put the info panel on the left, flip "back/forward" style icons (not media or
//!   rotate icons, and never the image), and right-align text. By default the arrow keys keep
//!   their physical meaning (Right = next) unless `browsing.arrows_follow_reading_direction`.
//! - **Bidi isolates.** Fluent wraps every placeable in FSI…PDI (`use_isolating` stays on),
//!   and [`isolate`] / [`isolate_ltr`] do the same for text you join yourself. These are
//!   formatting characters: pass the string unchanged to the shaper (parley runs the Unicode
//!   bidi algorithm on them and gives them no glyphs). Never strip them before shaping, and
//!   never draw them; use [`strip_isolates`] only for plain-text output (logs, clipboard).
//!   The `ar-XB` pseudo-locale additionally uses RLO…PDF overrides, handled the same way.
//!   In right-to-left languages the [`Localizer`] passes every message through [`for_rtl`]:
//!   nested FSIs (an argument that is itself a formatted message) become RLI or LRI by the
//!   letters they hold, and a message that holds RTL letters but would start a left-to-right
//!   paragraph gets a leading RLM. Run text you join yourself in an RTL UI through
//!   [`for_rtl`] too. Join the parts of one line ("name — details", "A · B") with
//!   [`join_parts`], which isolates each part in right-to-left UIs (a "name: text" line
//!   whose text sets its direction with [`join_labelled`]), and cut lines with
//!   [`truncate`], which keeps isolates balanced ([`balance_isolates`] mends text cut
//!   elsewhere). Names put into buttons and menu rows go through [`isolate_label`].

mod argcheck;
mod bidi;
mod catalog;
mod collate;
mod format;
mod handle;
mod languages;
mod localizer;
mod negotiate;
mod pseudo;
#[doc(hidden)]
pub mod scope;
mod units;

pub use argcheck::{argument_problem, argument_problems, record_argument_problems};
pub use bidi::{
    Direction, FSI, LRI, LRM, PDF, PDI, RLI, RLM, RLO, attach_signs, balance_isolates,
    base_direction, direction, ellipsis_cut, ellipsize_to_fit, for_rtl, is_rtl_letter, isolate,
    isolate_label, isolate_ltr, isolates_balanced, join_labelled, join_parts, ltr_value,
    resolve_fsi, signed_number, strip_isolates, truncate,
};
pub use catalog::{
    Arguments, CatalogueIssue, Untranslated, catalogue_locales, check_catalogues, completeness,
    english_arguments, english_has, english_ids, english_variables, parsed_catalogues,
    untranslated,
};
pub use collate::{NaturalKey, NaturalKeyer};
pub use fluent_bundle::{FluentArgs, FluentValue};
pub use format::{ByteUnits, DateStyle, ListKind, LocaleDataError};
pub use handle::I18n;
pub use icu_collator::CollatorBorrowed;
pub use languages::{LanguageInfo, available_languages, language_name, languages};
pub use localizer::{Localizer, fold_for_search};
pub use negotiate::{LanguagePreference, Negotiated, negotiate, parse_locale, system_locales};
pub use pseudo::{Pseudo, accent as pseudo_accent, mirror as pseudo_mirror};
pub use unic_langid::{LanguageIdentifier, langid};
pub use units::{quantity, translatable_unit_symbols, unit_symbol, unit_symbol_in};

/// The parts of one line joined with " · " in the [global](I18n::global) UI's direction:
/// each part keeps its own order whatever script it is in. Use it for every " · " join shown
/// on screen (Info rows, status lines, cards). Empty parts are left out.
pub fn dot_join<S: AsRef<str>>(parts: &[S]) -> String {
    I18n::global().dot_join(parts)
}

/// `x` with exactly `fraction_digits` decimals in the [global](I18n::global) UI's number
/// format ("2.50", "2,50" in de, "٢٫٥٠" where the locale uses its own digits). Use it for
/// every decimal shown on screen or passed to a message instead of `format!("{x:.2}")`.
pub fn fixed(x: impl Into<f64>, fraction_digits: u8) -> String {
    I18n::global()
        .localizer()
        .number_fixed(x.into(), fraction_digits)
}

/// Like [`fixed`] without grouping, for coordinates and other values read digit by digit
/// ("500000.00", "500000,00" in de).
pub fn fixed_plain(x: impl Into<f64>, fraction_digits: u8) -> String {
    I18n::global()
        .localizer()
        .number_fixed_plain(x.into(), fraction_digits)
}

/// `x` with at most `max_fraction_digits` decimals, trailing zeros left out, in the
/// [global](I18n::global) UI's number format ("2.5", "2,5" in de, "3").
pub fn number(x: impl Into<f64>, max_fraction_digits: u8) -> String {
    I18n::global()
        .localizer()
        .number(x.into(), max_fraction_digits)
}

/// Translates a message.
///
/// ```
/// use specular_i18n::{I18n, langid, t};
/// let i18n = I18n::for_language(langid!("en"));
/// assert_eq!(t!(i18n => "toolbar-next"), "Next image");
/// let text = t!(i18n => "status-image-count", count = 3);
/// assert_eq!(text, "\u{2068}3\u{2069} images");
/// ```
///
/// - `t!("id")` and `t!("id", name = value, …)` use [`I18n::global`].
/// - `t!(handle => "id")` and `t!(handle => "id", name = value, …)` use any value with
///   `tr`/`tr_args` methods taking `&str` ([`I18n`] or [`Localizer`]).
///
/// The id must be a string literal so the id-check test can find it.
#[macro_export]
macro_rules! t {
    ($id:literal $(,)?) => {
        $crate::I18n::global().tr($id)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $( args.set(stringify!($name), $value); )+
        $crate::I18n::global().tr_args($id, &args)
    }};
    ($handle:expr => $id:literal $(,)?) => {
        ($handle).tr($id)
    };
    ($handle:expr => $id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $( args.set(stringify!($name), $value); )+
        ($handle).tr_args($id, &args)
    }};
}

#[cfg(test)]
mod tests;

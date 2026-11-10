//! Localisation: Project Fluent catalogues for words and sentence templates,
//! ICU4X for dates, numbers and country names.
//!
//! A [`Localizer`] is built once per UI language and passed to [`crate::read`].
//! Switching language at runtime means building another `Localizer` and calling
//! `read` again; the report holds finished strings.
//!
//! Catalogues live in `i18n/<lang>/metadata.ftl` and are compiled into the
//! binary. Messages missing from the requested catalogue fall back to English
//! one by one. Fluent's Unicode isolation is left on, so every interpolated value
//! (camera model, city, file name, number) is wrapped in FSI (U+2068) … PDI
//! (U+2069) and keeps its own direction inside right-to-left sentences. Use
//! [`strip_isolates`] where those marks are unwanted (plain-text export, tests).

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use fixed_decimal::Decimal;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::types::FluentNumber;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use icu_calendar::Gregorian;
use icu_datetime::fieldsets::{E, T, YMD};
use icu_datetime::input::{Date, Time};
use icu_datetime::{DateTimeFormatter, FixedCalendarDateTimeFormatter, NoCalendarFormatter};
use icu_decimal::DecimalFormatter;
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use icu_experimental::dimension::percent::formatter::PercentFormatter;
use icu_experimental::displaynames::DisplayNamesOptions;
use icu_experimental::displaynames::multi::RegionDisplayNames;
use icu_locale_core::Locale;
use icu_locale_core::subtags::Region;
use unic_langid::LanguageIdentifier;

pub use unic_langid::langid;

/// Catalogues compiled into the binary: (folder name / BCP 47 tag, FTL source). English
/// first. The same locales as specular-i18n, so the panel follows the UI language.
const CATALOGUES: &[(&str, &str)] = &[
    ("en", include_str!("../i18n/en/metadata.ftl")),
    ("de", include_str!("../i18n/de/metadata.ftl")),
    ("fr", include_str!("../i18n/fr/metadata.ftl")),
    ("es", include_str!("../i18n/es/metadata.ftl")),
    ("ja", include_str!("../i18n/ja/metadata.ftl")),
    ("zh-Hans", include_str!("../i18n/zh-Hans/metadata.ftl")),
    ("ar", include_str!("../i18n/ar/metadata.ftl")),
    ("he", include_str!("../i18n/he/metadata.ftl")),
    ("it", include_str!("../i18n/it/metadata.ftl")),
    ("nl", include_str!("../i18n/nl/metadata.ftl")),
    ("pl", include_str!("../i18n/pl/metadata.ftl")),
    ("pt-BR", include_str!("../i18n/pt-BR/metadata.ftl")),
    ("pt-PT", include_str!("../i18n/pt-PT/metadata.ftl")),
    ("sv", include_str!("../i18n/sv/metadata.ftl")),
    ("da", include_str!("../i18n/da/metadata.ftl")),
    ("fi", include_str!("../i18n/fi/metadata.ftl")),
    ("cs", include_str!("../i18n/cs/metadata.ftl")),
    ("uk", include_str!("../i18n/uk/metadata.ftl")),
    ("tr", include_str!("../i18n/tr/metadata.ftl")),
];

/// Index of the catalogue for `langid`: same language, and for Chinese the same script
/// (zh-TW/HK/MO and zh-Hant are Traditional and must not get the Simplified catalogue).
/// Portuguese follows CLDR: bare `pt` and `pt-BR` are Brazilian, every other region
/// (`pt-PT`, `pt-AO`, `pt-MZ`…) European.
fn catalogue_index(langid: &LanguageIdentifier) -> Option<usize> {
    let lang = langid.language.as_str();
    CATALOGUES.iter().position(|(tag, _)| {
        let tag_lang = tag.split('-').next().unwrap_or(tag);
        if tag_lang != lang {
            return false;
        }
        if lang == "pt" {
            let brazil = langid.region.is_none_or(|r| r.as_str() == "BR");
            return *tag == if brazil { "pt-BR" } else { "pt-PT" };
        }
        if lang != "zh" {
            return true;
        }
        let script = match (langid.script, langid.region) {
            (Some(s), _) => s.as_str().to_owned(),
            (None, Some(r)) if matches!(r.as_str(), "TW" | "HK" | "MO") => "Hant".to_owned(),
            _ => "Hans".to_owned(),
        };
        tag.ends_with(&format!("-{script}"))
    })
}

/// First Strong Isolate and Pop Directional Isolate, as inserted by Fluent.
pub const FSI: char = '\u{2068}';
pub const PDI: char = '\u{2069}';

/// Removes Fluent's FSI/PDI isolation marks.
pub fn strip_isolates(s: &str) -> String {
    s.chars().filter(|&c| c != FSI && c != PDI).collect()
}

/// Each catalogue, parsed on first use: only the languages in use are parsed.
static PARSED: [OnceLock<FluentResource>; CATALOGUES.len()] =
    [const { OnceLock::new() }; CATALOGUES.len()];

fn resource(index: usize) -> &'static FluentResource {
    PARSED[index].get_or_init(|| {
        let (lang, src) = CATALOGUES[index];
        match FluentResource::try_new(src.to_owned()) {
            Ok(r) => r,
            Err((r, errs)) => {
                // A broken entry only loses that entry; the tests check for this.
                debug_assert!(
                    errs.is_empty(),
                    "syntax errors in {lang}/metadata.ftl: {errs:?}"
                );
                r
            }
        }
    })
}

/// The languages whose catalogue has been parsed (tests).
#[doc(hidden)]
pub fn parsed_languages() -> Vec<&'static str> {
    CATALOGUES
        .iter()
        .zip(&PARSED)
        .filter(|(_, p)| p.get().is_some())
        .map(|((tag, _), _)| *tag)
        .collect()
}

type Bundle = FluentBundle<&'static FluentResource>;

/// The decimal formatters of one locale, grouped and ungrouped.
struct Numbers {
    grouped: DecimalFormatter,
    ungrouped: DecimalFormatter,
}

impl Numbers {
    /// `x` with `min`..=`max` decimals, in the locale's digits and separators.
    fn format(&self, x: f64, min: u8, max: u8, grouping: bool) -> String {
        if !x.is_finite() {
            return String::new();
        }
        let scale = 10f64.powi(i32::from(max));
        let scaled = (x * scale).round();
        if scaled.abs() >= 9.0e15 {
            return format!("{x}");
        }
        let mut d = Decimal::from(scaled as i64);
        d.absolute.multiply_pow10(-i16::from(max));
        d.absolute.trim_end();
        d.absolute.pad_end(-i16::from(min));
        let f = if grouping {
            &self.grouped
        } else {
            &self.ungrouped
        };
        f.format(&d).to_string()
    }

    /// A Fluent number (`{ $count }`, `NUMBER($x, maximumFractionDigits: 1)`): at most 3
    /// decimals unless the options say otherwise, none for a whole number.
    fn fluent(&self, n: &FluentNumber) -> String {
        let o = &n.options;
        let max = o
            .maximum_fraction_digits
            .unwrap_or(if n.value.fract() == 0.0 { 0 } else { 3 })
            .min(20) as u8;
        let min = o.minimum_fraction_digits.unwrap_or(0).min(usize::from(max)) as u8;
        self.format(n.value, min, max, o.use_grouping)
    }
}

thread_local! {
    /// The numbers of the localizer formatting a message on this thread: Fluent's formatter
    /// hook gets no state of its own.
    static ACTIVE_NUMBERS: RefCell<Option<Arc<Numbers>>> = const { RefCell::new(None) };
}

/// Fluent's number hook: counts and other numbers passed as numbers (not preformatted
/// strings) are written in the language's digits and separators, so messages can pick
/// plural forms by the same argument they show.
fn number_formatter<M>(value: &FluentValue<'_>, _: &M) -> Option<String> {
    match value {
        FluentValue::Number(n) => ACTIVE_NUMBERS.with(|a| a.borrow().as_ref().map(|f| f.fluent(n))),
        _ => None,
    }
}

/// The text before and after the number in `langid`'s percentages: ("", "%") in English,
/// ("", " %") in German, ("%", "") in Turkish.
fn percent_affixes(langid: &LanguageIdentifier) -> (String, String) {
    let fallback = (String::new(), "%".to_owned());
    let tag = if langid.language.as_str() == "en" && langid.region.is_none() {
        "en-GB".to_owned()
    } else {
        langid.to_string()
    };
    let Ok(locale) = Locale::try_from_str(&tag) else {
        return fallback;
    };
    let (Ok(percent), Ok(decimal)) = (
        PercentFormatter::try_new((&locale).into(), Default::default()),
        DecimalFormatter::try_new((&locale).into(), DecimalFormatterOptions::default()),
    ) else {
        return fallback;
    };
    let seven = decimal.format(&Decimal::from(7u8)).to_string();
    let pattern = percent.format(&Decimal::from(7u8)).to_string();
    match pattern.find(&seven) {
        Some(at) => (
            pattern[..at].to_owned(),
            pattern[at + seven.len()..].to_owned(),
        ),
        None => fallback,
    }
}

fn bundle(index: usize, langid: LanguageIdentifier) -> Bundle {
    let (before, after) = percent_affixes(&langid);
    let mut b = FluentBundle::new_concurrent(vec![langid]);
    // Isolation stays on (the default); set explicitly so it is not lost by accident.
    b.set_use_isolating(true);
    b.set_formatter(Some(number_formatter));
    let builtins = b.add_builtins();
    debug_assert!(builtins.is_ok(), "NUMBER added twice");
    // `PERCENT($value)`: an already formatted number in the language's percent pattern
    // ("45%", "45 %", "%45"), so catalogues never write the sign themselves.
    let added = b.add_function("PERCENT", move |positional, _| {
        let number = match positional.first() {
            Some(FluentValue::String(s)) => s.to_string(),
            Some(FluentValue::Number(n)) => ACTIVE_NUMBERS
                .with(|a| a.borrow().as_ref().map(|f| f.fluent(n)))
                .unwrap_or_else(|| n.as_string().into_owned()),
            _ => return FluentValue::Error,
        };
        FluentValue::String(format!("{before}{number}{after}").into())
    });
    debug_assert!(added.is_ok(), "PERCENT added twice");
    if let Err(errs) = b.add_resource(resource(index)) {
        debug_assert!(
            errs.is_empty(),
            "duplicate messages in catalogue {index}: {errs:?}"
        );
    }
    b
}

/// Words, templates and locale-aware formatters for one UI language.
///
/// Cheap to share (`Send + Sync`); build one per language and keep it.
pub struct Localizer {
    langid: LanguageIdentifier,
    /// Catalogue for the requested language (absent for English).
    primary: Option<Bundle>,
    english: Bundle,
    weekday: FixedCalendarDateTimeFormatter<Gregorian, E>,
    date_no_weekday: DateTimeFormatter<YMD>,
    time_hm: NoCalendarFormatter<T>,
    time_hms: NoCalendarFormatter<T>,
    numbers: Arc<Numbers>,
    regions: RegionDisplayNames,
    /// How the parts of one line are joined (the caller's, for its UI direction).
    join: Option<Join>,
}

/// Joins the parts of one line with a separator (`parts`, `sep`) the way the UI's direction
/// needs: right to left, each part keeps its own order and the first reads on the right.
/// The app supplies it ([`Localizer::with_join`]); this crate does not know the direction.
pub type Join = std::sync::Arc<dyn Fn(&[&str], &str) -> String + Send + Sync>;

impl std::fmt::Debug for Localizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Localizer")
            .field("langid", &self.langid.to_string())
            .finish_non_exhaustive()
    }
}

/// Error building ICU formatters for a locale (should not happen with compiled data).
#[derive(Debug, thiserror::Error)]
#[error("no locale data for {0}")]
pub struct LocaleError(pub String);

impl Localizer {
    /// Builds a localizer for `langid` (e.g. `de`, `de-AT`, `en-GB`).
    ///
    /// Text comes from the catalogue for the language subtag, falling back to
    /// English per message. Dates, numbers and country names use the full
    /// locale through ICU4X, which has its own fallback (`de-AT` → `de` → root).
    pub fn new(langid: LanguageIdentifier) -> Self {
        Self::try_new(langid.clone()).unwrap_or_else(|_| {
            Self::try_new(unic_langid::langid!("en-GB")).expect("compiled ICU data covers en-GB")
        })
    }

    /// Like [`Localizer::new`], but reports missing ICU data instead of falling back to English.
    pub fn try_new(langid: LanguageIdentifier) -> Result<Self, LocaleError> {
        let err = |_| LocaleError(langid.to_string());
        // Bare "en" formats like en-GB: the English catalogue is British, and the
        // design specifies "Tuesday 14 May 2024, 18:32". "en-US" etc. stay as asked.
        let icu_tag = if langid.language.as_str() == "en"
            && langid.region.is_none()
            && langid.script.is_none()
        {
            "en-GB".to_owned()
        } else {
            langid.to_string()
        };
        let locale = Locale::try_from_str(&icu_tag).map_err(|_| LocaleError(langid.to_string()))?;
        let lang = langid.language.as_str();
        let primary = catalogue_index(&langid)
            .filter(|&i| CATALOGUES[i].0 != "en")
            .map(|i| bundle(i, langid.clone()));
        let english_tag = if lang == "en" {
            langid.clone()
        } else {
            unic_langid::langid!("en")
        };
        let english = bundle(0, english_tag);

        let prefs = (&locale).into();
        let weekday = FixedCalendarDateTimeFormatter::try_new(prefs, E::long()).map_err(err)?;
        let date_no_weekday =
            DateTimeFormatter::try_new((&locale).into(), YMD::long()).map_err(err)?;
        let time_hm = NoCalendarFormatter::try_new((&locale).into(), T::hm()).map_err(err)?;
        let time_hms = NoCalendarFormatter::try_new((&locale).into(), T::hms()).map_err(err)?;
        let decimal =
            DecimalFormatter::try_new((&locale).into(), DecimalFormatterOptions::default())
                .map_err(|_| LocaleError(langid.to_string()))?;
        let decimal_ungrouped =
            DecimalFormatter::try_new((&locale).into(), GroupingStrategy::Never.into())
                .map_err(|_| LocaleError(langid.to_string()))?;
        let regions = RegionDisplayNames::try_new((&locale).into(), DisplayNamesOptions::default())
            .map_err(|_| LocaleError(langid.to_string()))?;
        Ok(Self {
            langid,
            primary,
            english,
            weekday,
            date_no_weekday,
            time_hm,
            time_hms,
            numbers: Arc::new(Numbers {
                grouped: decimal,
                ungrouped: decimal_ungrouped,
            }),
            regions,
            join: None,
        })
    }

    /// This localizer joining the parts of a line with `join` (see [`Join`]).
    pub fn with_join(mut self, join: Join) -> Self {
        self.join = Some(join);
        self
    }

    /// The non-empty `parts` of one line joined with `sep`, through the caller's [`Join`]
    /// (plainly without one).
    pub fn join<S: AsRef<str>>(&self, parts: &[S], sep: &str) -> String {
        let parts: Vec<&str> = parts
            .iter()
            .map(AsRef::as_ref)
            .filter(|p| !p.is_empty())
            .collect();
        match &self.join {
            Some(j) => j(&parts, sep),
            None => parts.join(sep),
        }
    }

    /// [`Localizer::join`] with " · ".
    pub fn dot_join<S: AsRef<str>>(&self, parts: &[S]) -> String {
        self.join(parts, " · ")
    }

    /// British English: the catalogue's own spelling, and ICU's day-month-year order.
    pub fn english() -> Self {
        Self::new(unic_langid::langid!("en-GB"))
    }

    /// A shared British-English localizer (built on first use).
    pub fn english_shared() -> &'static Localizer {
        static EN: OnceLock<Localizer> = OnceLock::new();
        EN.get_or_init(Localizer::english)
    }

    pub fn langid(&self) -> &LanguageIdentifier {
        &self.langid
    }

    /// Locales that have a catalogue, e.g. `["en", "de", …, "pt-BR", "pt-PT", "sv"]`.
    pub fn available_languages() -> Vec<&'static str> {
        CATALOGUES.iter().map(|(l, _)| *l).collect()
    }

    /// True when the requested language has its own catalogue (otherwise all text is English).
    pub fn has_catalogue(&self) -> bool {
        self.primary.is_some() || self.langid.language.as_str() == "en"
    }

    /// Formats message `id`, from the requested catalogue or else the English one.
    /// Unknown ids come back as the id itself, so a gap is visible but harmless.
    pub fn message(&self, id: &str, args: Option<&FluentArgs>) -> String {
        for b in self.primary.iter().chain(std::iter::once(&self.english)) {
            if let Some(pattern) = b.get_message(id).and_then(|m| m.value()) {
                let prev = ACTIVE_NUMBERS.with(|a| a.borrow_mut().replace(self.numbers.clone()));
                let mut errs = Vec::new();
                let text = b.format_pattern(pattern, args, &mut errs).into_owned();
                ACTIVE_NUMBERS.with(|a| *a.borrow_mut() = prev);
                return text;
            }
        }
        id.to_owned()
    }

    /// Message without arguments.
    pub fn text(&self, id: &str) -> String {
        self.message(id, None)
    }

    /// Message with arguments.
    pub fn fmt(&self, id: &str, args: &FluentArgs) -> String {
        self.message(id, Some(args))
    }

    /// A unit symbol as the language writes it after a number (`unit-symbol-…`: Hebrew
    /// "ק״מ" for "km"); symbols without a message (µm, nm, Å, file units) unchanged.
    pub fn unit(&self, symbol: &str) -> String {
        let id = match symbol.trim() {
            "mm" => "unit-symbol-mm",
            "cm" => "unit-symbol-cm",
            "m" => "unit-symbol-m",
            "km" => "unit-symbol-km",
            "mm²" => "unit-symbol-mm2",
            "cm²" => "unit-symbol-cm2",
            "m²" => "unit-symbol-m2",
            "km²" => "unit-symbol-km2",
            "ha" => "unit-symbol-ha",
            "ft" => "unit-symbol-ft",
            "ft²" => "unit-symbol-ft2",
            "mi" => "unit-symbol-mi",
            "mi²" => "unit-symbol-mi2",
            "ac" => "unit-symbol-ac",
            _ => return symbol.to_owned(),
        };
        self.text(id)
    }

    // ----- Numbers -------------------------------------------------------

    fn decimal_with(&self, f: &DecimalFormatter, x: f64, decimals: u8, trim: bool) -> String {
        if !x.is_finite() {
            return String::new();
        }
        let scale = 10f64.powi(decimals as i32);
        let scaled = (x * scale).round();
        if scaled.abs() >= 9.0e15 {
            return format!("{x}");
        }
        let mut d = Decimal::from(scaled as i64);
        d.absolute.multiply_pow10(-(decimals as i16));
        if trim {
            d.absolute.trim_end();
        } else {
            d.absolute.pad_end(-(decimals as i16));
        }
        f.format(&d).to_string()
    }

    /// `x` with exactly `decimals` decimals, locale separators and grouping ("38.7230", "38,7230").
    pub fn fixed(&self, x: f64, decimals: u8) -> String {
        self.decimal_with(&self.numbers.grouped, x, decimals, false)
    }

    /// `x` with at most `max_decimals` decimals, trailing zeros removed ("2.5", "2,5", "30").
    pub fn trimmed(&self, x: f64, max_decimals: u8) -> String {
        self.decimal_with(&self.numbers.grouped, x, max_decimals, true)
    }

    /// Integer with locale grouping ("2,456,789", "2.456.789").
    pub fn int(&self, n: u64) -> String {
        self.numbers.grouped.format(&Decimal::from(n)).to_string()
    }

    /// Integer with locale digits but no grouping (pixel sizes, years).
    pub fn int_plain(&self, n: u64) -> String {
        self.numbers.ungrouped.format(&Decimal::from(n)).to_string()
    }

    /// Like [`Localizer::trimmed`] without grouping.
    pub fn trimmed_plain(&self, x: f64, max_decimals: u8) -> String {
        self.decimal_with(&self.numbers.ungrouped, x, max_decimals, true)
    }

    // ----- Dates ---------------------------------------------------------

    /// "Tuesday 14 May 2024" (en-GB), "Dienstag, 14. Mai 2024" (de): ICU's weekday
    /// and date, joined by the catalogue's `date-with-weekday` pattern.
    pub fn date_with_weekday(&self, year: i32, month: u8, day: u8) -> Option<String> {
        let g = Date::try_new_gregorian(year, month, day).ok()?;
        let weekday = self.weekday.format(&g).to_string();
        let date = self.date(year, month, day)?;
        Some(self.fmt(
            "date-with-weekday",
            &args!("weekday" => weekday, "date" => date),
        ))
    }

    /// "14 May 2024" (en-GB), "14. Mai 2024" (de).
    pub fn date(&self, year: i32, month: u8, day: u8) -> Option<String> {
        let d = Date::try_new_iso(year, month, day).ok()?;
        Some(self.date_no_weekday.format(&d).to_string())
    }

    /// "18:32" (en-GB, de), "6:32 PM" (en-US).
    pub fn time_hm(&self, hour: u8, minute: u8) -> Option<String> {
        let t = Time::try_new(hour, minute, 0, 0).ok()?;
        Some(self.time_hm.format(&t).to_string())
    }

    /// "18:32:10".
    pub fn time_hms(&self, hour: u8, minute: u8, second: u8) -> Option<String> {
        let t = Time::try_new(hour, minute, second, 0).ok()?;
        Some(self.time_hms.format(&t).to_string())
    }

    // ----- Countries -----------------------------------------------------

    /// Country name for an ISO 3166-1 alpha-2 code from CLDR ("PT" → "Portugal" / "Portugal",
    /// "DE" → "Germany" / "Deutschland"). Unknown codes come back unchanged.
    pub fn country_name(&self, code: &str) -> String {
        let code = code.trim();
        Region::try_from_str(&code.to_ascii_uppercase())
            .ok()
            .and_then(|r| self.regions.of(r))
            .map_or_else(|| code.to_owned(), str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn localizer_is_send_sync() {
        assert_send_sync::<Localizer>();
    }

    #[test]
    fn catalogues_parse_cleanly() {
        for (lang, src) in CATALOGUES {
            if let Err((_, errs)) = FluentResource::try_new((*src).to_owned()) {
                panic!("{lang}: {errs:?}");
            }
        }
    }

    /// Message ids with the `$variables` each one uses (value, attributes, selectors).
    fn ids_and_vars(src: &str) -> Vec<(String, std::collections::BTreeSet<String>)> {
        use fluent_syntax::ast;
        fn pattern(p: &ast::Pattern<&str>, out: &mut std::collections::BTreeSet<String>) {
            for el in &p.elements {
                if let ast::PatternElement::Placeable { expression } = el {
                    expr(expression, out);
                }
            }
        }
        fn expr(e: &ast::Expression<&str>, out: &mut std::collections::BTreeSet<String>) {
            match e {
                ast::Expression::Inline(i) => inline(i, out),
                ast::Expression::Select { selector, variants } => {
                    inline(selector, out);
                    for v in variants {
                        pattern(&v.value, out);
                    }
                }
            }
        }
        fn inline(i: &ast::InlineExpression<&str>, out: &mut std::collections::BTreeSet<String>) {
            match i {
                ast::InlineExpression::VariableReference { id } => {
                    out.insert(id.name.to_owned());
                }
                ast::InlineExpression::FunctionReference { arguments, .. }
                | ast::InlineExpression::TermReference {
                    arguments: Some(arguments),
                    ..
                } => {
                    arguments.positional.iter().for_each(|p| inline(p, out));
                    arguments.named.iter().for_each(|n| inline(&n.value, out));
                }
                ast::InlineExpression::Placeable { expression } => expr(expression, out),
                _ => {}
            }
        }
        let res = fluent_syntax::parser::parse(src).unwrap_or_else(|(r, _)| r);
        res.body
            .iter()
            .filter_map(|e| match e {
                ast::Entry::Message(m) => {
                    let mut vars = std::collections::BTreeSet::new();
                    if let Some(v) = &m.value {
                        pattern(v, &mut vars);
                    }
                    m.attributes
                        .iter()
                        .for_each(|a| pattern(&a.value, &mut vars));
                    Some((m.id.name.to_owned(), vars))
                }
                _ => None,
            })
            .collect()
    }

    /// English messages not yet translated: a locale may lack them (the English text shows),
    /// but one that has them must use the same placeholders. Emptied by the translation pass.
    const AWAITING_TRANSLATION: &[&str] = &[];

    /// Every locale has every English id (bar [`AWAITING_TRANSLATION`]), no extra ids, and
    /// the same placeholders.
    #[test]
    fn every_locale_has_every_english_message_with_same_placeholders() {
        let en = ids_and_vars(CATALOGUES[0].1);
        assert!(en.len() > 100, "English catalogue looks truncated");
        for id in AWAITING_TRANSLATION {
            assert!(en.iter().any(|(e, _)| e == id), "`{id}` is not in en");
        }
        let mut problems = Vec::new();
        for (tag, src) in &CATALOGUES[1..] {
            let tr: std::collections::BTreeMap<_, _> = ids_and_vars(src).into_iter().collect();
            for (id, vars) in &en {
                match tr.get(id) {
                    None if AWAITING_TRANSLATION.contains(&id.as_str()) => {}
                    None => problems.push(format!("{tag}: missing `{id}`")),
                    Some(v) if v != vars => {
                        problems.push(format!("{tag}: `{id}` uses {v:?}, English uses {vars:?}"))
                    }
                    Some(_) => {}
                }
            }
            for id in tr.keys() {
                if !en.iter().any(|(e, _)| e == id) {
                    problems.push(format!("{tag}: unknown id `{id}` (not in en)"));
                }
            }
        }
        assert!(
            problems.is_empty(),
            "metadata catalogues out of step with i18n/en/metadata.ftl:\n{}",
            problems.join("\n")
        );
    }

    /// Every `i18n/<locale>/` folder is compiled in (a new folder needs a CATALOGUES line).
    #[test]
    fn every_catalogue_folder_is_registered() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
        let mut folders: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().join("metadata.ftl").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        folders.sort();
        let mut registered: Vec<String> = CATALOGUES.iter().map(|(t, _)| (*t).to_owned()).collect();
        registered.sort();
        assert_eq!(folders, registered);
    }

    #[test]
    fn catalogue_selection() {
        let tag = |l: LanguageIdentifier| catalogue_index(&l).map(|i| CATALOGUES[i].0);
        assert_eq!(tag(langid!("de-AT")), Some("de"));
        assert_eq!(tag(langid!("zh")), Some("zh-Hans"));
        assert_eq!(tag(langid!("zh-CN")), Some("zh-Hans"));
        assert_eq!(tag(langid!("zh-Hans-SG")), Some("zh-Hans"));
        assert_eq!(tag(langid!("zh-TW")), None);
        assert_eq!(tag(langid!("zh-Hant")), None);
        assert_eq!(tag(langid!("ar-EG")), Some("ar"));
        assert_eq!(tag(langid!("sw")), None);
        assert_eq!(tag(langid!("pt")), Some("pt-BR"));
        assert_eq!(tag(langid!("pt-BR")), Some("pt-BR"));
        assert_eq!(tag(langid!("pt-PT")), Some("pt-PT"));
        assert_eq!(tag(langid!("pt-AO")), Some("pt-PT"));
        assert_eq!(tag(langid!("sv-FI")), Some("sv"));
        for (t, _) in CATALOGUES {
            let l: LanguageIdentifier = t.parse().unwrap();
            assert!(Localizer::new(l).has_catalogue(), "{t}");
        }
    }

    #[test]
    fn translated_panels() {
        // Not the English fallback: each catalogue really is used.
        let en = Localizer::english().text("group-camera");
        for (t, _) in &CATALOGUES[1..] {
            let l = Localizer::new(t.parse().unwrap());
            assert_ne!(
                l.text("group-exposure"),
                Localizer::english().text("group-exposure"),
                "{t}"
            );
            assert!(!l.text("group-camera").is_empty(), "{t}");
        }
        assert_eq!(en, "Camera");
    }

    #[test]
    fn numbers() {
        let en = Localizer::english();
        let de = Localizer::new(langid!("de"));
        assert_eq!(en.fixed(38.723, 4), "38.7230");
        assert_eq!(de.fixed(38.723, 4), "38,7230");
        assert_eq!(en.trimmed(2.50, 1), "2.5");
        assert_eq!(en.trimmed(30.0, 1), "30");
        assert_eq!(en.int(2_456_789), "2,456,789");
        assert_eq!(de.int(2_456_789), "2.456.789");
        assert_eq!(en.int_plain(6000), "6000");
        assert_eq!(en.fixed(-0.00001, 1), "0.0");
    }

    /// Counts passed as numbers pick their plural form and are written in the language's
    /// digits and separators.
    #[test]
    fn counts_are_numbers() {
        let count = |l: &Localizer, n: u64| {
            strip_isolates(&l.fmt("cifti-maps-parcels", &args!("count" => n)))
        };
        let en = Localizer::english();
        assert_eq!(count(&en, 1), "1 parcel");
        assert_eq!(count(&en, 2_456), "2,456 parcels");
        let de = Localizer::new(langid!("de"));
        assert_eq!(count(&de, 1), "1 Parzelle");
        assert_eq!(count(&de, 2_456), "2.456 Parzellen");
        let pl = Localizer::new(langid!("pl"));
        let beams = |n: u64| {
            strip_isolates(&pl.fmt(
                "casa-plane-beams",
                &args!("count" => n, "min" => "1", "max" => "2"),
            ))
        };
        assert!(beams(3).starts_with("3 wiązki"), "{}", beams(3));
        assert!(beams(5).starts_with("5 wiązek"), "{}", beams(5));
        // Arabic's own number pattern; 12 is CLDR `many`.
        let ar = Localizer::new(langid!("ar"));
        let bytes = strip_isolates(&ar.fmt("file-size-bytes", &args!("num" => 12u64)));
        assert_eq!(bytes, format!("{} بايتًا", ar.int(12)));
    }

    #[test]
    fn fallback_and_isolation() {
        let de = Localizer::new(langid!("de-AT"));
        assert_eq!(de.text("group-camera"), "Kamera");
        assert_eq!(de.text("no-such-message"), "no-such-message");
        let s = de.fmt(
            "place",
            &args!("city" => "Lissabon", "country" => "Portugal"),
        );
        assert_eq!(s, "\u{2068}Lissabon\u{2069}, \u{2068}Portugal\u{2069}");
        // A language without a catalogue: English text, local country names.
        let sw = Localizer::new(langid!("sw"));
        assert!(!sw.has_catalogue());
        assert_eq!(sw.text("group-camera"), "Camera");
        let fr = Localizer::new(langid!("fr"));
        assert!(fr.has_catalogue());
        assert_eq!(fr.country_name("DE"), "Allemagne");
    }

    #[test]
    fn countries() {
        let en = Localizer::english();
        let de = Localizer::new(langid!("de"));
        assert_eq!(en.country_name("PT"), "Portugal");
        assert_eq!(en.country_name("AR"), "Argentina");
        assert_eq!(en.country_name("XK"), "Kosovo");
        assert_eq!(de.country_name("DE"), "Deutschland");
        assert_eq!(de.country_name("pt"), "Portugal");
        assert_eq!(en.country_name("ZZZ"), "ZZZ");
    }

    #[test]
    fn dates() {
        let en = Localizer::english();
        let de = Localizer::new(langid!("de"));
        assert_eq!(
            strip_isolates(&en.date_with_weekday(2024, 5, 14).unwrap()),
            "Tuesday 14 May 2024"
        );
        assert_eq!(
            strip_isolates(&de.date_with_weekday(2024, 5, 14).unwrap()),
            "Dienstag, 14. Mai 2024"
        );
        assert_eq!(de.date(2024, 5, 14).unwrap(), "14. Mai 2024");
        assert_eq!(en.time_hm(18, 32).unwrap(), "18:32");
        assert_eq!(de.time_hm(18, 32).unwrap(), "18:32");
    }
}

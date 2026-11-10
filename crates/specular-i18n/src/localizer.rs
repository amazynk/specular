//! An immutable snapshot of one UI language: Fluent bundles for the fallback chain plus the
//! ICU4X formatters for the region-format locale. [`crate::I18n`] swaps whole snapshots when
//! the language changes, so a `Localizer` never changes after it is built.

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::sync::{Arc, OnceLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::types::{FluentNumber, FluentNumberOptions};
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use icu_collator::CollatorBorrowed;
use unic_langid::LanguageIdentifier;

use crate::bidi::{self, Direction};
use crate::catalog;
use crate::collate::{NaturalKey, NaturalKeyer};
use crate::format::{ByteUnits, DateStyle, Formats, ListKind};
use crate::negotiate::{Negotiated, negotiate};
use crate::pseudo::Pseudo;

type Bundle = FluentBundle<&'static FluentResource>;

thread_local! {
    /// Formats of the localizer currently formatting a message on this thread. Fluent's
    /// formatter hook receives only the bundle's memoizer, so the ICU formatters travel here.
    static ACTIVE_FORMATS: RefCell<Option<Arc<Formats>>> = const { RefCell::new(None) };
}

/// Fluent number formatter hook: formats `FluentValue::Number` with ICU4X for the active
/// region-format locale. Plural selection still uses the raw value.
fn icu_number_formatter<M>(value: &FluentValue<'_>, _: &M) -> Option<String> {
    match value {
        FluentValue::Number(n) => {
            ACTIVE_FORMATS.with(|a| a.borrow().as_ref().map(|f| f.fluent_number(n)))
        }
        _ => None,
    }
}

/// Fluent `PERCENT(x)`: `x` percent in the locale's pattern, so catalogues never write a
/// percent sign themselves: `PERCENT(45)` → "45%" (en), "45 %" (de, fr), "%45" (tr),
/// "٤٥٪" (ar). `x` is a number of percent (not a ratio; decimals kept, at most 2 unless
/// `maximumFractionDigits` says otherwise) or a number already formatted, such as a signed
/// "+15" from [`crate::signed_number`].
fn percent_function<'a>(positional: &[FluentValue<'a>], _named: &FluentArgs) -> FluentValue<'a> {
    let text = ACTIVE_FORMATS.with(|a| {
        let a = a.borrow();
        let f = a.as_ref()?;
        let (value, max) = match positional.first()? {
            FluentValue::Number(n) => (n.value, n.options.maximum_fraction_digits),
            // Already written in the locale's digits (with its sign, fixed decimals).
            FluentValue::String(s) => return Some(f.percent_text(s)),
            _ => return None,
        };
        let max = max
            .unwrap_or(if value.fract() == 0.0 { 0 } else { 2 })
            .min(20) as u8;
        Some(f.percent(value / 100.0, max))
    });
    match text {
        Some(t) => FluentValue::String(t.into()),
        None => FluentValue::Error,
    }
}

/// Sets the active formats for the duration of `f`, restoring the previous value (re-entrant).
fn with_formats<R>(formats: &Arc<Formats>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Arc<Formats>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            ACTIVE_FORMATS.with(|a| *a.borrow_mut() = prev);
        }
    }
    let prev = ACTIVE_FORMATS.with(|a| a.borrow_mut().replace(formats.clone()));
    let _restore = Restore(prev);
    f()
}

/// One UI language, ready to translate and format. Cheap to share (`Arc<Localizer>`).
pub struct Localizer {
    ui: LanguageIdentifier,
    chain: Vec<LanguageIdentifier>,
    pseudo: Option<Pseudo>,
    direction: Direction,
    bundles: Vec<Bundle>,
    formats: Arc<Formats>,
    #[cfg(debug_assertions)]
    missing: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl std::fmt::Debug for Localizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Localizer")
            .field("ui", &self.ui.to_string())
            .field("chain", &self.chain)
            .field("formats", &self.formats.langid.to_string())
            .finish_non_exhaustive()
    }
}

fn make_bundle(langid: &LanguageIdentifier, pseudo: Option<Pseudo>) -> Bundle {
    let catalogue = catalog::find(langid).unwrap_or_else(catalog::english);
    let mut bundle = FluentBundle::new_concurrent(vec![catalogue.langid.clone()]);
    // Keep Fluent's FSI/PDI isolation of placeables: interpolated file names, numbers and
    // place names keep their own direction inside translated sentences.
    bundle.set_use_isolating(true);
    bundle.set_formatter(Some(icu_number_formatter));
    if let Some(p) = pseudo {
        bundle.set_transform(Some(p.transform()));
    }
    if let Err(e) = bundle.add_builtins() {
        specular_log::error!(target: specular_log::I18N, "Fluent builtins for {langid}: {e:?}");
    }
    if let Err(e) = bundle.add_function("PERCENT", percent_function) {
        specular_log::error!(target: specular_log::I18N, "Fluent PERCENT for {langid}: {e:?}");
    }
    for r in &catalogue.resources {
        if let Err(errs) = bundle.add_resource(r) {
            specular_log::error!(target: specular_log::I18N, "Fluent resource for {langid}: {errs:?}");
        }
    }
    bundle
}

impl Localizer {
    /// Builds a localizer for the most preferred of `requested` that has a catalogue
    /// (or a pseudo-locale), formatting dates and numbers for `region_formats` when given,
    /// otherwise for the UI locale.
    pub fn new(
        requested: &[LanguageIdentifier],
        region_formats: Option<&LanguageIdentifier>,
    ) -> Self {
        let n = negotiate(requested, &catalog::catalogue_locales());
        Self::from_negotiated(n, region_formats)
    }

    /// Builds a localizer from an explicit negotiation result.
    pub fn from_negotiated(n: Negotiated, region_formats: Option<&LanguageIdentifier>) -> Self {
        let Negotiated { ui, chain, pseudo } = n;
        let format_langid = region_formats
            .cloned()
            .or_else(|| pseudo.map(Pseudo::format_locale))
            .unwrap_or_else(|| ui.clone());
        let formats = Formats::new(&format_langid)
            .or_else(|e| {
                specular_log::warn!(target: specular_log::I18N, "{e}; formatting as en-GB");
                Formats::new(&unic_langid::langid!("en-GB"))
            })
            .expect("compiled ICU4X data covers en-GB");
        let bundles = chain.iter().map(|l| make_bundle(l, pseudo)).collect();
        Self {
            direction: bidi::direction(&ui),
            ui,
            chain,
            pseudo,
            bundles,
            formats: Arc::new(formats),
            #[cfg(debug_assertions)]
            missing: Default::default(),
        }
    }

    /// The shared English localizer (for logs, `--help`, and fallback text).
    pub fn english() -> Arc<Localizer> {
        static EN: OnceLock<Arc<Localizer>> = OnceLock::new();
        EN.get_or_init(|| Arc::new(Localizer::new(&[unic_langid::langid!("en")], None)))
            .clone()
    }

    /// The UI language (as specific as requested, e.g. `de-AT`, or a pseudo-locale).
    pub fn language(&self) -> &LanguageIdentifier {
        &self.ui
    }

    /// Catalogues consulted, in order (always ends with `en`).
    pub fn chain(&self) -> &[LanguageIdentifier] {
        &self.chain
    }

    /// The locale used for numbers, dates, lists and collation.
    pub fn format_language(&self) -> &LanguageIdentifier {
        &self.formats.langid
    }

    /// The active pseudo-locale, if any.
    pub fn pseudo(&self) -> Option<Pseudo> {
        self.pseudo
    }

    /// Layout direction of the UI language. Mirror the chrome when this is RTL.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// True when some catalogue in the chain defines `id` (a message, or `message.attribute`).
    pub fn has_message(&self, id: &str) -> bool {
        let (msg, attr) = split_id(id);
        self.bundles.iter().any(|b| {
            b.get_message(msg).is_some_and(|m| match attr {
                None => m.value().is_some(),
                Some(a) => m.get_attribute(a).is_some(),
            })
        })
    }

    /// Translates message `id` (or `id.attribute`).
    pub fn tr(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// Translates message `id` with arguments.
    pub fn tr_args(&self, id: &str, args: &FluentArgs<'_>) -> String {
        crate::argcheck::note(id, args);
        self.format(id, Some(args))
    }

    fn format(&self, id: &str, args: Option<&FluentArgs<'_>>) -> String {
        let (msg, attr) = split_id(id);
        for bundle in &self.bundles {
            let Some(m) = bundle.get_message(msg) else {
                continue;
            };
            let pattern = match attr {
                None => m.value(),
                Some(a) => m.get_attribute(a).map(|a| a.value()),
            };
            let Some(pattern) = pattern else {
                continue;
            };
            let mut errors = Vec::new();
            let text = with_formats(&self.formats, || {
                bundle
                    .format_pattern(pattern, args, &mut errors)
                    .into_owned()
            });
            if !errors.is_empty() {
                specular_log::warn!(target: specular_log::I18N, "Fluent errors formatting {id} ({}): {errors:?}", self.ui);
            }
            if self.direction.is_rtl() && self.pseudo.is_none() {
                return bidi::for_rtl(&text);
            }
            if self.pseudo.is_none() {
                crate::scope::note(&self.ui, id, &text);
            }
            return text;
        }
        self.report_missing(id);
        // Never empty: show the id so the gap is visible and searchable.
        id.to_owned()
    }

    #[cfg(debug_assertions)]
    fn report_missing(&self, id: &str) {
        if let Ok(mut seen) = self.missing.lock()
            && seen.insert(id.to_owned())
        {
            specular_log::error!(target: specular_log::I18N, "missing message id {id:?} (language {})", self.ui);
        }
    }

    #[cfg(not(debug_assertions))]
    fn report_missing(&self, _id: &str) {}

    // ----- Formatting ---------------------------------------------------

    /// A number with grouping and at most `max_fraction_digits` decimals ("1,234.5",
    /// "1.234,5" in de, "١٬٢٣٤٫٥" in ar).
    pub fn number(&self, x: f64, max_fraction_digits: u8) -> String {
        self.formats.number(x, 0, max_fraction_digits, true)
    }

    /// A number with exactly `fraction_digits` decimals ("2.50").
    pub fn number_fixed(&self, x: f64, fraction_digits: u8) -> String {
        self.formats
            .number(x, fraction_digits, fraction_digits, true)
    }

    /// A number with exactly `fraction_digits` decimals and no grouping (coordinates:
    /// "500000.00", "500000,00" in de).
    pub fn number_fixed_plain(&self, x: f64, fraction_digits: u8) -> String {
        self.formats
            .number(x, fraction_digits, fraction_digits, false)
    }

    /// An integer with grouping ("12,000,000").
    pub fn integer(&self, x: i64) -> String {
        self.formats.number(x as f64, 0, 0, true)
    }

    /// An integer without grouping (pixel dimensions, years).
    pub fn integer_plain(&self, x: i64) -> String {
        self.formats.number(x as f64, 0, 0, false)
    }

    /// A ratio as a percentage: 1.5 → "150%" (en), "150 %" (de, fr), "150٪" (ar).
    pub fn percent(&self, ratio: f64, max_fraction_digits: u8) -> String {
        self.formats.percent(ratio, max_fraction_digits)
    }

    /// `number`, a percentage already written in this locale's digits (with its sign, a
    /// fixed number of decimals), in the locale's percent pattern: "40%" (en), "40 %"
    /// (de, fr), "%40" (tr). The same pattern as [`Localizer::percent`].
    pub fn percent_text(&self, number: &str) -> String {
        self.formats.percent_text(number)
    }

    /// A byte size in decimal (kB, MB…) or binary (KiB, MiB…) units: "4.2 MB", "870 kB",
    /// "1 byte". One decimal below 10 units, none above; trailing zeros trimmed.
    pub fn bytes(&self, bytes: u64, units: ByteUnits) -> String {
        const DEC: [&str; 4] = [
            "unit-kilobytes",
            "unit-megabytes",
            "unit-gigabytes",
            "unit-terabytes",
        ];
        const BIN: [&str; 4] = [
            "unit-kibibytes",
            "unit-mebibytes",
            "unit-gibibytes",
            "unit-tebibytes",
        ];
        let (base, ids) = match units {
            ByteUnits::Decimal => (1000.0, DEC),
            ByteUnits::Binary => (1024.0, BIN),
        };
        if (bytes as f64) < base {
            let mut args = FluentArgs::new();
            args.set("value", bytes);
            return self.tr_args("unit-bytes", &args);
        }
        let mut v = bytes as f64 / base;
        let mut i = 0;
        loop {
            let digits = if v < 10.0 { 1 } else { 0 };
            let scale = 10f64.powi(digits);
            let rounded = (v * scale).round() / scale;
            if rounded >= base && i + 1 < ids.len() {
                v /= base;
                i += 1;
                continue;
            }
            let opts = FluentNumberOptions {
                maximum_fraction_digits: Some(digits as usize),
                ..Default::default()
            };
            let mut args = FluentArgs::new();
            args.set(
                "value",
                FluentValue::Number(FluentNumber::new(rounded, opts)),
            );
            return self.tr_args(ids[i], &args);
        }
    }

    /// A calendar date (Gregorian). `None` for an invalid date.
    pub fn date(&self, year: i32, month: u8, day: u8, style: DateStyle) -> Option<String> {
        self.formats.date(year, month, day, style)
    }

    /// A month of a year, as in a "May 2024" heading, in the locale's own order and forms
    /// ("květen 2024", "mayo de 2024", "2024年5月"). `None` for a month outside 1..=12.
    pub fn year_month(&self, year: i32, month: u8) -> Option<String> {
        self.formats.year_month(year, month)
    }

    /// A time of day, in the locale's 12/24-hour convention, with seconds when given.
    pub fn time(&self, hour: u8, minute: u8, second: Option<u8>) -> Option<String> {
        self.formats.time(hour, minute, second)
    }

    /// Date and time to the minute ("14 May 2024, 18:32").
    pub fn date_time(&self, date: (i32, u8, u8), time: (u8, u8)) -> Option<String> {
        self.formats.date_time(date, time)
    }

    /// A list: "JPEG, PNG and WebP", "JPEG, PNG oder WebP".
    pub fn list<S: AsRef<str>>(&self, items: &[S], kind: ListKind) -> String {
        self.formats.list(items, kind)
    }

    /// The locale's collator at tertiary strength (full ordering).
    pub fn collator(&self) -> &CollatorBorrowed<'static> {
        self.formats.collator()
    }

    /// The locale's collator at primary strength (ignores case and accents).
    pub fn collator_primary(&self) -> &CollatorBorrowed<'static> {
        self.formats.collator_primary()
    }

    /// Natural ordering of names in this locale: "img2" < "img10", case-insensitive,
    /// "Äpfel" beside "Apfel" in German.
    pub fn natural_cmp(&self, a: &str, b: &str) -> Ordering {
        self.formats.natural_cmp(a, b)
    }

    /// Makes natural sort keys for many names: sorting by keys gives the order of
    /// [`natural_cmp`](Self::natural_cmp) at a fraction of the cost (see [`NaturalKey`]).
    pub fn natural_keyer(&self) -> NaturalKeyer<'_> {
        NaturalKeyer::new(self.formats.collator_primary())
    }

    /// [`natural_cmp`](Self::natural_cmp) of two names with their [`natural_keyer`]
    /// (Self::natural_keyer) keys.
    pub fn natural_cmp_keyed(&self, a: (&NaturalKey, &str), b: (&NaturalKey, &str)) -> Ordering {
        NaturalKey::cmp_names(self.formats.collator(), a, b)
    }
}

fn split_id(id: &str) -> (&str, Option<&str>) {
    match id.split_once('.') {
        Some((m, a)) => (m, Some(a)),
        None => (id, None),
    }
}

/// Folds text for search matching: isolates removed, lowercased, decomposed and stripped of
/// combining marks, so "farbe" finds "Farbe", "equilibre" finds "Équilibre" and "ubersicht"
/// finds "Übersicht".
pub fn fold_for_search(s: &str) -> Cow<'_, str> {
    let plain = bidi::strip_isolates(s);
    let nfd = icu_normalizer::DecomposingNormalizerBorrowed::new_nfd();
    let decomposed = nfd.normalize(&plain);
    let folded: String = decomposed
        .chars()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect();
    Cow::Owned(folded)
}

fn is_combining_mark(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
        // Arabic harakat and Hebrew points: searchable without vowel marks.
        | 0x064B..=0x065F | 0x0670 | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2 | 0x05C4..=0x05C5 | 0x05C7
        // Japanese voicing marks stay (they change the word), so they are not listed.
    )
}

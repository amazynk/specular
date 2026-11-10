//! ICU4X formatters for one region-format locale: numbers, percentages, dates, times, lists
//! and collation. Heavy formatters are built on first use, so switching language is cheap.

use std::cmp::Ordering;
use std::sync::OnceLock;

use fixed_decimal::{Decimal, FloatPrecision};
use fluent_bundle::types::{FluentNumber, FluentNumberStyle};
use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::{Collator, CollatorBorrowed};
use icu_datetime::fieldsets::{T, YM, YMD, YMDE, YMDT};
use icu_datetime::input::{Date, DateTime, Time};
use icu_datetime::options::TimePrecision;
use icu_datetime::{DateTimeFormatter, NoCalendarFormatter};
use icu_decimal::DecimalFormatter;
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use icu_experimental::dimension::percent::formatter::PercentFormatter;
use icu_list::ListFormatter;
use icu_list::options::{ListFormatterOptions, ListLength};
use icu_locale_core::Locale;
use unic_langid::LanguageIdentifier;
use writeable::Writeable;

/// Length of a formatted date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateStyle {
    /// "14/05/2024" (en-GB), "14.05.24" (de).
    Short,
    /// "14 May 2024" (en-GB), "14.05.2024" (de).
    Medium,
    /// "14 May 2024" (en-GB), "14. Mai 2024" (de).
    Long,
    /// "Tuesday 14 May 2024" (en-GB), "Dienstag, 14. Mai 2024" (de).
    Full,
}

/// Kind of list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListKind {
    /// "a, b and c".
    And,
    /// "a, b or c".
    Or,
    /// Units: "3 ft, 7 in".
    Unit,
}

/// Decimal (SI, 1000) or binary (IEC, 1024) byte units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ByteUnits {
    /// kB, MB, GB, TB (powers of 1000), as macOS and most file managers show.
    #[default]
    Decimal,
    /// KiB, MiB, GiB, TiB (powers of 1024).
    Binary,
}

/// Error building formatters (only possible with a broken or missing data provider).
#[derive(Debug, thiserror::Error)]
#[error("no ICU4X locale data for {0}")]
pub struct LocaleDataError(pub String);

pub(crate) struct Formats {
    pub(crate) langid: LanguageIdentifier,
    locale: Locale,
    decimal: DecimalFormatter,
    decimal_ungrouped: DecimalFormatter,
    percent: OnceLock<Option<PercentFormatter<DecimalFormatter>>>,
    date: [OnceLock<Option<DateFormatter>>; 4],
    time_hm: OnceLock<Option<NoCalendarFormatter<T>>>,
    time_hms: OnceLock<Option<NoCalendarFormatter<T>>>,
    date_time: OnceLock<Option<DateTimeFormatter<YMDT>>>,
    year_month: OnceLock<Option<DateTimeFormatter<YM>>>,
    lists: [OnceLock<Option<ListFormatter>>; 3],
    collator_primary: OnceLock<CollatorBorrowed<'static>>,
    collator_tertiary: OnceLock<CollatorBorrowed<'static>>,
}

enum DateFormatter {
    Plain(DateTimeFormatter<YMD>),
    Weekday(DateTimeFormatter<YMDE>),
}

impl std::fmt::Debug for Formats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Formats")
            .field("locale", &self.locale.to_string())
            .finish_non_exhaustive()
    }
}

/// Maps a UI/format language id to the ICU locale used for formatting. Bare `en` formats as
/// British English to match the catalogue's spelling and the design ("14 May 2024, 18:32").
pub(crate) fn icu_locale(langid: &LanguageIdentifier) -> Locale {
    let tag = if langid.language.as_str() == "en" && langid.region.is_none() {
        "en-GB".to_owned()
    } else {
        langid.to_string()
    };
    Locale::try_from_str(&tag).unwrap_or(icu_locale_core::locale!("en-GB"))
}

impl Formats {
    pub(crate) fn new(langid: &LanguageIdentifier) -> Result<Self, LocaleDataError> {
        let locale = icu_locale(langid);
        let err = |_| LocaleDataError(locale.to_string());
        let decimal =
            DecimalFormatter::try_new((&locale).into(), DecimalFormatterOptions::default())
                .map_err(err)?;
        let decimal_ungrouped =
            DecimalFormatter::try_new((&locale).into(), GroupingStrategy::Never.into())
                .map_err(err)?;
        Ok(Self {
            langid: langid.clone(),
            locale,
            decimal,
            decimal_ungrouped,
            percent: OnceLock::new(),
            date: Default::default(),
            time_hm: OnceLock::new(),
            time_hms: OnceLock::new(),
            date_time: OnceLock::new(),
            year_month: OnceLock::new(),
            lists: Default::default(),
            collator_primary: OnceLock::new(),
            collator_tertiary: OnceLock::new(),
        })
    }

    // ----- Numbers -------------------------------------------------------

    /// Converts `x` to a decimal with at most `max` and at least `min` fraction digits.
    fn to_decimal(x: f64, min: u8, max: u8) -> Option<Decimal> {
        if !x.is_finite() {
            return None;
        }
        let mut d = Decimal::try_from_f64(x, FloatPrecision::RoundTrip).ok()?;
        d.round(-(max as i16));
        d.absolute.trim_end();
        d.absolute.pad_end(-(min as i16));
        if d.absolute.is_zero() {
            d.sign = fixed_decimal::Sign::None;
        }
        Some(d)
    }

    pub(crate) fn number(&self, x: f64, min_frac: u8, max_frac: u8, grouping: bool) -> String {
        let Some(d) = Self::to_decimal(x, min_frac, max_frac) else {
            return x.to_string();
        };
        let f = if grouping {
            &self.decimal
        } else {
            &self.decimal_ungrouped
        };
        f.format(&d).write_to_string().into_owned()
    }

    /// `ratio` 1.5 → "150%" (en), "150 %" (de, with a no-break space).
    pub(crate) fn percent(&self, ratio: f64, max_frac: u8) -> String {
        let pct = ratio * 100.0;
        let Some(d) = Self::to_decimal(pct, 0, max_frac) else {
            return format!("{pct}%");
        };
        let f = self.percent.get_or_init(|| {
            PercentFormatter::try_new((&self.locale).into(), Default::default()).ok()
        });
        match f {
            Some(f) => f.format(&d).write_to_string().into_owned(),
            None => format!("{}%", self.decimal.format(&d)),
        }
    }

    /// `number` (already written in this locale's digits) in the locale's percent pattern.
    pub(crate) fn percent_text(&self, number: &str) -> String {
        // The pattern around a known number, with that number swapped for `number`.
        let seven = self.number(7.0, 0, 0, false);
        let pattern = self.percent(0.07, 0);
        match pattern.find(&seven) {
            Some(at) => format!("{}{number}{}", &pattern[..at], &pattern[at + seven.len()..]),
            None => format!("{number}%"),
        }
    }

    /// Formats a Fluent number argument with ICU4X (used by the bundle formatter hook).
    pub(crate) fn fluent_number(&self, n: &FluentNumber) -> String {
        let o = &n.options;
        let max = o
            .maximum_fraction_digits
            .unwrap_or(if n.value.fract() == 0.0 { 0 } else { 3 })
            .min(20) as u8;
        let min = o.minimum_fraction_digits.unwrap_or(0).min(max as usize) as u8;
        if o.style == FluentNumberStyle::Percent {
            return self.percent(n.value, max);
        }
        self.number(n.value, min, max, o.use_grouping)
    }

    // ----- Dates and times ----------------------------------------------

    pub(crate) fn date(&self, year: i32, month: u8, day: u8, style: DateStyle) -> Option<String> {
        let iso = Date::try_new_iso(year, month, day).ok()?;
        let idx = style as usize;
        let f = self.date[idx].get_or_init(|| {
            let prefs = (&self.locale).into();
            match style {
                DateStyle::Short => DateTimeFormatter::try_new(prefs, YMD::short())
                    .ok()
                    .map(DateFormatter::Plain),
                DateStyle::Medium => DateTimeFormatter::try_new(prefs, YMD::medium())
                    .ok()
                    .map(DateFormatter::Plain),
                DateStyle::Long => DateTimeFormatter::try_new(prefs, YMD::long())
                    .ok()
                    .map(DateFormatter::Plain),
                DateStyle::Full => DateTimeFormatter::try_new(prefs, YMDE::long())
                    .ok()
                    .map(DateFormatter::Weekday),
            }
        });
        Some(match f.as_ref()? {
            DateFormatter::Plain(f) => f.format(&iso).write_to_string().into_owned(),
            DateFormatter::Weekday(f) => f.format(&iso).write_to_string().into_owned(),
        })
    }

    /// The month's name on its own (standalone form: "May", "květen", "5月").
    pub(crate) fn year_month(&self, year: i32, month: u8) -> Option<String> {
        let iso = Date::try_new_iso(year, month, 1).ok()?;
        let f = self
            .year_month
            .get_or_init(|| DateTimeFormatter::try_new((&self.locale).into(), YM::long()).ok());
        Some(f.as_ref()?.format(&iso).write_to_string().into_owned())
    }

    pub(crate) fn time(&self, hour: u8, minute: u8, second: Option<u8>) -> Option<String> {
        let t = Time::try_new(hour, minute, second.unwrap_or(0), 0).ok()?;
        let cell = if second.is_some() {
            &self.time_hms
        } else {
            &self.time_hm
        };
        let f = cell.get_or_init(|| {
            let fs = if second.is_some() { T::hms() } else { T::hm() };
            NoCalendarFormatter::try_new((&self.locale).into(), fs).ok()
        });
        Some(f.as_ref()?.format(&t).write_to_string().into_owned())
    }

    pub(crate) fn date_time(
        &self,
        (year, month, day): (i32, u8, u8),
        (hour, minute): (u8, u8),
    ) -> Option<String> {
        let date = Date::try_new_iso(year, month, day).ok()?;
        let time = Time::try_new(hour, minute, 0, 0).ok()?;
        let f = self.date_time.get_or_init(|| {
            DateTimeFormatter::try_new(
                (&self.locale).into(),
                YMDT::medium().with_time_precision(TimePrecision::Minute),
            )
            .ok()
        });
        Some(
            f.as_ref()?
                .format(&DateTime { date, time })
                .write_to_string()
                .into_owned(),
        )
    }

    // ----- Lists --------------------------------------------------------

    pub(crate) fn list<S: AsRef<str>>(&self, items: &[S], kind: ListKind) -> String {
        let idx = kind as usize;
        let f = self.lists[idx].get_or_init(|| {
            let prefs = (&self.locale).into();
            let opts = ListFormatterOptions::default().with_length(ListLength::Wide);
            match kind {
                ListKind::And => ListFormatter::try_new_and(prefs, opts),
                ListKind::Or => ListFormatter::try_new_or(prefs, opts),
                ListKind::Unit => ListFormatter::try_new_unit(prefs, opts),
            }
            .ok()
        });
        match f {
            Some(f) => f
                .format(items.iter().map(AsRef::as_ref))
                .write_to_string()
                .into_owned(),
            None => items
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    // ----- Collation ----------------------------------------------------

    fn make_collator(&self, strength: Strength) -> CollatorBorrowed<'static> {
        let mut opts = CollatorOptions::default();
        opts.strength = Some(strength);
        Collator::try_new((&self.locale).into(), opts)
            .or_else(|_| Collator::try_new(Default::default(), opts))
            .expect("compiled root collation data")
    }

    /// Primary strength: ignores case and accents ("Äpfel" = "apfel").
    pub(crate) fn collator_primary(&self) -> &CollatorBorrowed<'static> {
        self.collator_primary
            .get_or_init(|| self.make_collator(Strength::Primary))
    }

    /// Tertiary strength: the full locale order, used as a tie-break.
    pub(crate) fn collator(&self) -> &CollatorBorrowed<'static> {
        self.collator_tertiary
            .get_or_init(|| self.make_collator(Strength::Tertiary))
    }

    pub(crate) fn natural_cmp(&self, a: &str, b: &str) -> Ordering {
        crate::collate::natural_cmp_with(self.collator_primary(), self.collator(), a, b)
    }
}

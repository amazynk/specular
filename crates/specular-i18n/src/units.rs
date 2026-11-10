//! Unit symbols as the UI's language writes them.
//!
//! The code writes lengths and areas with the SI and imperial symbols ("km", "ha", "ft");
//! [`unit_symbol`] gives the symbol the language uses instead (`unit-symbol-…` messages:
//! Hebrew "ק״מ", "הקטר"). Languages that do not translate a symbol fall back to
//! English, the symbol itself. Symbols with no message (µm, nm, Å, px, °C…) stay as they are.

use crate::{I18n, Localizer};

/// The message of each translatable symbol.
const SYMBOLS: [(&str, &str); 14] = [
    ("mm", "unit-symbol-mm"),
    ("cm", "unit-symbol-cm"),
    ("m", "unit-symbol-m"),
    ("km", "unit-symbol-km"),
    ("mm²", "unit-symbol-mm2"),
    ("cm²", "unit-symbol-cm2"),
    ("m²", "unit-symbol-m2"),
    ("km²", "unit-symbol-km2"),
    ("ha", "unit-symbol-ha"),
    ("ft", "unit-symbol-ft"),
    ("ft²", "unit-symbol-ft2"),
    ("mi", "unit-symbol-mi"),
    ("mi²", "unit-symbol-mi2"),
    ("ac", "unit-symbol-ac"),
];

/// The SI and imperial symbols [`unit_symbol`] translates.
pub fn translatable_unit_symbols() -> impl Iterator<Item = &'static str> {
    SYMBOLS.iter().map(|(s, _)| *s)
}

/// `symbol` as the [global](I18n::global) UI language writes it after a number.
pub fn unit_symbol(symbol: &str) -> String {
    unit_symbol_in(&I18n::global().localizer(), symbol)
}

/// "`value` `symbol`" as one bidi isolate for joining into a sentence: left to right while
/// the symbol is Latin (in Hebrew text "0.5 µm" keeps its order), right to left when the
/// language writes it in its own letters (Hebrew "0.5 מ׳": the number on the right). An
/// empty symbol gives the value alone.
pub fn quantity(value: &str, symbol: &str) -> String {
    let unit = unit_symbol(symbol);
    let s = format!("{value} {unit}");
    let s = s.trim_end();
    if unit.chars().any(crate::is_rtl_letter) {
        format!("{}{s}{}", crate::RLI, crate::PDI)
    } else {
        crate::isolate_ltr(s)
    }
}

/// `symbol` as `l`'s language writes it after a number (unknown symbols unchanged).
pub fn unit_symbol_in(l: &Localizer, symbol: &str) -> String {
    let key = symbol.trim();
    match SYMBOLS.iter().find(|(s, _)| *s == key) {
        Some((_, id)) => l.tr(id),
        None => symbol.to_owned(),
    }
}

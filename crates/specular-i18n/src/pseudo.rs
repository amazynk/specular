//! Pseudo-locales, generated from the English catalogue at format time (no files).
//!
//! - `en-XA`: every Latin letter accented and vowels doubled, about 40% longer. Finds
//!   hard-coded strings (they stay plain), clipped text and missing font coverage.
//! - `ar-XB`: English text forced right-to-left with RLO…PDF, and the layout direction set to
//!   RTL. Finds chrome that doesn't mirror and text that isn't isolated.
//!
//! The transform touches only the translatable text of a message. Interpolated values
//! (file names, numbers) are left alone, as in a real translation.

use std::borrow::Cow;

use unic_langid::LanguageIdentifier;

use crate::bidi::{PDF, RLO};

/// Which pseudo-locale a language identifier names, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pseudo {
    /// `en-XA`: accented and longer.
    Accented,
    /// `ar-XB`: mirrored right-to-left.
    Bidi,
}

impl Pseudo {
    pub const ALL: [Pseudo; 2] = [Pseudo::Accented, Pseudo::Bidi];

    pub fn of(langid: &LanguageIdentifier) -> Option<Pseudo> {
        match (
            langid.language.as_str(),
            langid.region.as_ref().map(|r| r.as_str()),
        ) {
            ("en", Some("XA")) => Some(Pseudo::Accented),
            ("ar", Some("XB")) => Some(Pseudo::Bidi),
            _ => None,
        }
    }

    pub fn langid(self) -> LanguageIdentifier {
        match self {
            Pseudo::Accented => unic_langid::langid!("en-XA"),
            Pseudo::Bidi => unic_langid::langid!("ar-XB"),
        }
    }

    /// The Fluent text transform for this pseudo-locale.
    pub(crate) fn transform(self) -> fn(&str) -> Cow<'_, str> {
        match self {
            Pseudo::Accented => accent,
            Pseudo::Bidi => mirror,
        }
    }

    /// Locale used for ICU formatting (numbers, dates) in this pseudo-locale.
    pub(crate) fn format_locale(self) -> LanguageIdentifier {
        match self {
            Pseudo::Accented => unic_langid::langid!("en-GB"),
            // Arabic digits and separators exercise bidi number handling too.
            Pseudo::Bidi => unic_langid::langid!("ar"),
        }
    }
}

const UPPER: [char; 26] = [
    'Ȧ', 'Ɓ', 'Ƈ', 'Ḓ', 'Ḗ', 'Ƒ', 'Ɠ', 'Ħ', 'Ī', 'Ĵ', 'Ķ', 'Ŀ', 'Ḿ', 'Ƞ', 'Ǿ', 'Ƥ', 'Ɋ', 'Ř', 'Ş',
    'Ŧ', 'Ŭ', 'Ṽ', 'Ẇ', 'Ẋ', 'Ẏ', 'Ẑ',
];
const LOWER: [char; 26] = [
    'ȧ', 'ƀ', 'ƈ', 'ḓ', 'ḗ', 'ƒ', 'ɠ', 'ħ', 'ī', 'ĵ', 'ķ', 'ŀ', 'ḿ', 'ƞ', 'ǿ', 'ƥ', 'ɋ', 'ř', 'ş',
    'ŧ', 'ŭ', 'ṽ', 'ẇ', 'ẋ', 'ẏ', 'ẑ',
];

/// `en-XA`: accents every ASCII letter and doubles vowels (≈ +40% length for English).
pub fn accent(s: &str) -> Cow<'_, str> {
    if !s.chars().any(|c| c.is_ascii_alphabetic()) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        let mapped = match c {
            'A'..='Z' => UPPER[(c as u8 - b'A') as usize],
            'a'..='z' => LOWER[(c as u8 - b'a') as usize],
            other => {
                out.push(other);
                continue;
            }
        };
        out.push(mapped);
        if matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
            out.push(mapped.to_lowercase().next().unwrap_or(mapped));
        }
    }
    Cow::Owned(out)
}

/// `ar-XB`: forces the text right-to-left with RLO…PDF.
pub fn mirror(s: &str) -> Cow<'_, str> {
    if s.trim().is_empty() {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 6);
    out.push(RLO);
    out.push_str(s);
    out.push(PDF);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accented_is_longer_and_accented() {
        let src = "Drop an image here or press the button to open one";
        let a = accent(src);
        assert!(!a.chars().any(|c| c.is_ascii_alphabetic()), "{a}");
        let ratio = a.chars().count() as f32 / src.chars().count() as f32;
        assert!((1.3..1.6).contains(&ratio), "ratio {ratio}");
        assert_eq!(accent("12 / 34"), "12 / 34");
    }

    #[test]
    fn mirrored_is_wrapped() {
        assert_eq!(mirror("Undo"), "\u{202E}Undo\u{202C}");
        assert_eq!(mirror(" "), " ");
    }

    #[test]
    fn recognises_pseudo_ids() {
        assert_eq!(
            Pseudo::of(&unic_langid::langid!("en-XA")),
            Some(Pseudo::Accented)
        );
        assert_eq!(
            Pseudo::of(&unic_langid::langid!("ar-XB")),
            Some(Pseudo::Bidi)
        );
        assert_eq!(Pseudo::of(&unic_langid::langid!("en-GB")), None);
    }
}

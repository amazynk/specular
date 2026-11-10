//! Locale negotiation: OS preferences (`sys-locale`) against the available catalogues, with
//! fallback chains such as `pt-BR` → `pt` → `en`.

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use unic_langid::LanguageIdentifier;

use crate::pseudo::Pseudo;

/// The user's language choice, as stored in settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum LanguagePreference {
    /// Follow the operating system's preferred languages.
    #[default]
    System,
    /// A specific language (may also be a pseudo-locale, `en-XA` or `ar-XB`).
    Language(LanguageIdentifier),
}

impl LanguagePreference {
    /// The requested languages, most preferred first.
    pub fn requested(&self) -> Vec<LanguageIdentifier> {
        match self {
            LanguagePreference::System => system_locales(),
            LanguagePreference::Language(l) => vec![l.clone()],
        }
    }
}

impl std::fmt::Display for LanguagePreference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanguagePreference::System => f.write_str("system"),
            LanguagePreference::Language(l) => write!(f, "{l}"),
        }
    }
}

impl std::str::FromStr for LanguagePreference {
    type Err = unic_langid::LanguageIdentifierError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("system") || s.is_empty() {
            Ok(LanguagePreference::System)
        } else {
            parse_locale(s).map(LanguagePreference::Language)
        }
    }
}

/// Parses a BCP 47 tag or a POSIX locale name (`de_AT.UTF-8@euro` → `de-AT`).
pub fn parse_locale(s: &str) -> Result<LanguageIdentifier, unic_langid::LanguageIdentifierError> {
    let s = s.split(['.', '@']).next().unwrap_or(s).replace('_', "-");
    let s = if s == "C" || s == "POSIX" {
        "en".to_owned()
    } else {
        s
    };
    s.parse()
}

/// The operating system's preferred UI languages, most preferred first. Falls back to `en`.
pub fn system_locales() -> Vec<LanguageIdentifier> {
    let mut out: Vec<LanguageIdentifier> = sys_locale::get_locales()
        .filter_map(|s| parse_locale(&s).ok())
        .collect();
    out.dedup();
    if out.is_empty() {
        out.push(unic_langid::langid!("en"));
    }
    out
}

/// Result of negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negotiated {
    /// The UI locale: the requested locale that won, kept as specific as asked (`de-AT`),
    /// which also drives formatting. For pseudo-locales, `en-XA` / `ar-XB`.
    pub ui: LanguageIdentifier,
    /// Catalogues to look messages up in, in order, always ending with `en`.
    pub chain: Vec<LanguageIdentifier>,
    /// The pseudo-locale in use, if any.
    pub pseudo: Option<Pseudo>,
}

/// Negotiates `requested` (most preferred first) against `available` catalogues.
///
/// Uses Fluent's filtering strategy (exact, then parent, then likely-subtags matches such as
/// `zh-CN` → `zh-Hans`, then same-language regions), so a message missing from `pt-BR` comes
/// from `pt`, then from a sibling like `pt-PT`, and finally from English.
pub fn negotiate(requested: &[LanguageIdentifier], available: &[LanguageIdentifier]) -> Negotiated {
    let en = unic_langid::langid!("en");
    if let Some(first) = requested.first()
        && let Some(p) = Pseudo::of(first)
    {
        return Negotiated {
            ui: first.clone(),
            chain: vec![en],
            pseudo: Some(p),
        };
    }
    let mut chain: Vec<LanguageIdentifier> = negotiate_languages(
        &with_parent_locales(requested),
        available,
        Some(&en),
        NegotiationStrategy::Filtering,
    )
    .into_iter()
    // Never cross scripts: zh-TW (Traditional) must not get zh-Hans text, even though
    // the filtering strategy matches any region of the same language.
    .filter(|a| {
        **a == en
            || requested.iter().any(|r| {
                r.language == a.language
                    && match (script_of(r), a.script) {
                        (Some(rs), Some(as_)) => rs == as_,
                        _ => true,
                    }
            })
    })
    .cloned()
    .collect();
    if !chain.contains(&en) {
        chain.push(en.clone());
    }
    let top = chain[0].clone();
    // Keep the user's own, more specific tag for the UI locale when it is the same language
    // (de-AT for the de catalogue; zh-CN maps to zh-Hans and keeps its region for formats).
    let ui = requested
        .iter()
        .find(|r| {
            r.language == top.language
                && (top.script.is_none() || r.script.is_none() || r.script == top.script)
        })
        .map(|r| {
            let mut l = r.clone();
            if l.script.is_none() && top.script.is_some() && top.language.as_str() == "zh" {
                l.script = top.script;
            }
            l
        })
        .unwrap_or(top);
    Negotiated {
        ui,
        chain,
        pseudo: None,
    }
}

/// `requested` with the CLDR parent locales that likely-subtags matching misses put in
/// front: Portuguese outside Brazil (`pt-AO`, `pt-MZ`, `pt-CH`…) inherits from European
/// Portuguese, while bare `pt` means Brazilian. Without this, `pt-AO` would match whichever
/// `pt-*` catalogue comes first. (Only for matching; the UI locale keeps the user's tag.)
fn with_parent_locales(requested: &[LanguageIdentifier]) -> Vec<LanguageIdentifier> {
    let mut out = Vec::with_capacity(requested.len() + 1);
    for r in requested {
        if r.language.as_str() == "pt"
            && let Some(region) = r.region
            && !matches!(region.as_str(), "BR" | "PT")
        {
            let pt_pt = unic_langid::langid!("pt-PT");
            if !out.contains(&pt_pt) {
                out.push(pt_pt);
            }
        }
        out.push(r.clone());
    }
    out
}

/// The script of `l`: explicit, or implied for languages written in more than one script
/// where the region decides (Chinese).
fn script_of(l: &LanguageIdentifier) -> Option<unic_langid::subtags::Script> {
    if l.script.is_some() {
        return l.script;
    }
    if l.language.as_str() == "zh" {
        let hant = matches!(
            l.region.as_ref().map(|r| r.as_str()),
            Some("TW" | "HK" | "MO")
        );
        return (if hant { "Hant" } else { "Hans" }).parse().ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use unic_langid::langid;

    fn avail() -> Vec<LanguageIdentifier> {
        crate::catalog::catalogue_locales()
    }

    #[test]
    fn exact_and_parent() {
        let n = negotiate(&[langid!("de-AT")], &avail());
        assert_eq!(n.ui, langid!("de-AT"));
        assert_eq!(n.chain, vec![langid!("de"), langid!("en")]);
    }

    #[test]
    fn pt_br_chain() {
        let available = [
            langid!("en"),
            langid!("pt"),
            langid!("pt-BR"),
            langid!("pt-PT"),
        ];
        let n = negotiate(&[langid!("pt-BR")], &available);
        assert_eq!(n.ui, langid!("pt-BR"));
        assert_eq!(&n.chain[..2], &[langid!("pt-BR"), langid!("pt")]);
        assert_eq!(n.chain.last(), Some(&langid!("en")));
    }

    #[test]
    fn portuguese_variants() {
        // Two full catalogues: Brazilian and European Portuguese, each falling back to the
        // other (a message added later still shows in Portuguese) and then to English.
        let available = [
            langid!("en"),
            langid!("de"),
            langid!("pt-BR"),
            langid!("pt-PT"),
        ];
        let chain = |tag: LanguageIdentifier| negotiate(&[tag], &available).chain;
        let br = vec![langid!("pt-BR"), langid!("pt-PT"), langid!("en")];
        let pt = vec![langid!("pt-PT"), langid!("pt-BR"), langid!("en")];
        assert_eq!(chain(langid!("pt-BR")), br);
        assert_eq!(
            chain(langid!("pt")),
            br,
            "bare pt is Brazilian (CLDR likely subtags)"
        );
        assert_eq!(chain(langid!("pt-PT")), pt);
        // CLDR parent of the other Portuguese regions is pt-PT.
        assert_eq!(chain(langid!("pt-AO")), pt);
        assert_eq!(chain(langid!("pt-MZ")), pt);
        let n = negotiate(&[langid!("pt-AO")], &available);
        assert_eq!(n.ui, langid!("pt-AO"), "formats keep the region");
        // Second choice after an unavailable language.
        let n = negotiate(&[langid!("sw"), langid!("pt-PT")], &available);
        assert_eq!(n.chain, pt);
        // POSIX names from LANG.
        assert_eq!(chain(parse_locale("pt_PT.UTF-8").unwrap()), pt);
        assert_eq!(chain(parse_locale("pt_BR.UTF-8").unwrap()), br);
    }

    #[test]
    fn chinese_scripts() {
        let n = negotiate(&[langid!("zh-CN")], &avail());
        assert_eq!(n.chain[0], langid!("zh-Hans"));
        assert_eq!(n.ui, langid!("zh-Hans-CN"));
        // Traditional Chinese must not get Simplified text.
        let n = negotiate(&[langid!("zh-TW")], &avail());
        assert_eq!(n.chain, vec![langid!("en")]);
    }

    #[test]
    fn unknown_and_multiple() {
        let n = negotiate(&[langid!("sw")], &avail());
        assert_eq!(n.ui, langid!("en"));
        assert_eq!(n.chain, vec![langid!("en")]);
        let n = negotiate(&[langid!("sw"), langid!("fr-CA")], &avail());
        assert_eq!(n.ui, langid!("fr-CA"));
        assert_eq!(n.chain, vec![langid!("fr"), langid!("en")]);
    }

    #[test]
    fn pseudo() {
        let n = negotiate(&[langid!("ar-XB")], &avail());
        assert_eq!(n.pseudo, Some(Pseudo::Bidi));
        assert_eq!(n.chain, vec![langid!("en")]);
    }

    #[test]
    fn parse_posix_and_preference() {
        assert_eq!(parse_locale("de_AT.UTF-8@euro").unwrap(), langid!("de-AT"));
        assert_eq!(parse_locale("C").unwrap(), langid!("en"));
        assert_eq!(
            "system".parse::<LanguagePreference>().unwrap(),
            LanguagePreference::System
        );
        assert_eq!(
            "pt-BR".parse::<LanguagePreference>().unwrap(),
            LanguagePreference::Language(langid!("pt-BR"))
        );
        assert!(!system_locales().is_empty());
    }
}

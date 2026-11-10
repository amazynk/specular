//! Only the catalogues in use are parsed: the UI language and its English fallback at start,
//! another language when it is chosen (all of them took about 56 MB). Its own test binary:
//! nothing else in the process parses catalogues first.

use specular_i18n::{I18n, languages, parsed_catalogues};
use unic_langid::langid;

#[test]
fn parses_only_the_languages_in_use() {
    assert!(parsed_catalogues().is_empty());
    let i18n = I18n::for_language(langid!("de"));
    assert_eq!(i18n.tr("toolbar-next"), "Nächstes Bild");
    let mut got = parsed_catalogues();
    got.sort_by_key(|l| l.to_string());
    assert_eq!(got, [langid!("de"), langid!("en")]);

    // The language list (with completeness) parses none.
    assert!(languages().len() > 20);
    assert_eq!(parsed_catalogues().len(), 2);

    // Another language is parsed when chosen.
    i18n.set_language(langid!("fr"));
    assert_eq!(i18n.tr("toolbar-next"), "Image suivante");
    assert_eq!(parsed_catalogues().len(), 3);
    assert!(parsed_catalogues().contains(&langid!("fr")));
}

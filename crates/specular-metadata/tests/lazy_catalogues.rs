//! Only the info panel catalogues in use are parsed: the UI language's and English. Its own
//! test binary: nothing else in the process parses catalogues first.

use specular_metadata::Localizer;
use specular_metadata::i18n::parsed_languages;

#[test]
fn parses_only_the_languages_in_use() {
    assert!(parsed_languages().is_empty());
    let _de = Localizer::new(unic_langid::langid!("de"));
    assert_eq!(parsed_languages(), ["en", "de"]);
    assert!(
        Localizer::available_languages().len() > 15,
        "listing parses none"
    );
    assert_eq!(parsed_languages().len(), 2);
    let _fr = Localizer::new(unic_langid::langid!("fr"));
    assert_eq!(parsed_languages().len(), 3);
}

//! Behavioural tests across modules: fallback, plurals, formatting, collation, pseudo-locales.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::cmp::Ordering;

use crate::*;

const NBSP: char = '\u{00A0}';
const NNBSP: char = '\u{202F}';

fn loc(tag: &str) -> Localizer {
    Localizer::new(&[tag.parse().unwrap()], None)
}

fn plain(s: String) -> String {
    strip_isolates(&s)
}

/// Normalises no-break spaces to plain spaces for readable assertions.
fn spaces(s: &str) -> String {
    s.replace([NBSP, NNBSP], " ")
}

#[test]
fn translates_and_falls_back() {
    assert_eq!(loc("en").tr("toolbar-next"), "Next image");
    assert_eq!(loc("de").tr("toolbar-next"), "Nächstes Bild");
    assert_eq!(loc("de-AT").tr("toolbar-next"), "Nächstes Bild");
    // Unknown languages use English.
    assert_eq!(loc("sw").tr("toolbar-next"), "Next image");
}

#[test]
fn per_message_fallback_to_english() {
    // A chain whose first catalogue lacks the message: negotiate de, but pretend only the
    // English bundle has it by asking for an id that only English defines. All shipped
    // catalogues are complete, so build a chain with a locale that has no catalogue first.
    let n = Negotiated {
        ui: langid!("sw"),
        chain: vec![langid!("sw"), langid!("en")],
        pseudo: None,
    };
    let l = Localizer::from_negotiated(n, None);
    assert_eq!(l.tr("toolbar-next"), "Next image");
}

#[test]
fn missing_message_is_never_empty() {
    let missing = String::from("no-such-message-id");
    let l = loc("de");
    assert_eq!(l.tr(&missing), missing);
    assert!(!l.has_message(&missing));
    assert!(l.has_message("toolbar-next"));
}

#[test]
fn placeables_are_isolated() {
    let l = loc("en");
    let mut args = FluentArgs::new();
    args.set("shortcut", "Ctrl+O");
    let s = l.tr_args("empty-drop", &args);
    assert_eq!(s, "Drop an image here or press \u{2068}Ctrl+O\u{2069}");
}

#[test]
fn macro_forms() {
    let i18n = I18n::for_language(langid!("en"));
    assert_eq!(t!(i18n => "toolbar-next"), "Next image");
    assert_eq!(
        plain(t!(i18n => "readout-position", index = 3, count = 1200)),
        "3 / 1,200"
    );
    let l = loc("de");
    assert_eq!(
        plain(t!(&l => "readout-position", index = 3, count = 1200)),
        "3 / 1.200"
    );
    // Global handle form compiles and never returns an empty string.
    assert!(!t!("toolbar-next").is_empty());
    assert!(!t!("status-image-count", count = 2).is_empty());
}

#[test]
fn english_plurals() {
    let l = loc("en");
    let count = |n: u32| plain(t!(&l => "status-image-count", count = n));
    assert_eq!(count(1), "1 image");
    assert_eq!(count(0), "0 images");
    assert_eq!(count(2), "2 images");
    assert_eq!(count(1500), "1,500 images");
}

#[test]
fn arabic_has_six_plural_categories() {
    let l = loc("ar");
    let count = |n: u32| plain(t!(&l => "status-image-count", count = n));
    let forms: Vec<String> = [0, 1, 2, 3, 11, 100].into_iter().map(count).collect();
    // Six different shapes: zero, one, two, few (3–10), many (11–99), other (100+).
    let mut distinct = forms.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 6, "{forms:?}");
    assert_eq!(forms[0], "لا توجد صور");
    assert_eq!(forms[1], "صورة واحدة");
    assert_eq!(forms[2], "صورتان");
    assert_eq!(forms[3], "3 صور");
    assert_eq!(forms[4], "11 صورة");
    // Numbers are formatted by ICU4X for the locale: Arabic-Indic digits in ar-EG.
    let eg = loc("ar-EG");
    assert_eq!(plain(t!(&eg => "status-image-count", count = 3)), "٣ صور");
}

#[test]
fn numbers_per_locale() {
    assert_eq!(loc("en").number(1234567.891, 2), "1,234,567.89");
    assert_eq!(loc("de").number(1234567.891, 2), "1.234.567,89");
    assert_eq!(spaces(&loc("fr").number(1234567.891, 2)), "1 234 567,89");
    // CLDR: Arabic defaults to Latin digits in most regions, Arabic-Indic digits in Egypt.
    assert_eq!(loc("ar").number(1234.5, 1), "1,234.5");
    assert_eq!(loc("ar-EG").number(1234.5, 1), "١٬٢٣٤٫٥");
    assert_eq!(loc("en").number(2.0, 2), "2");
    assert_eq!(loc("en").number_fixed(2.5, 2), "2.50");
    assert_eq!(loc("en").integer_plain(6000), "6000");
    assert_eq!(loc("en").number(-0.0001, 2), "0");
}

#[test]
fn percentages_per_locale() {
    assert_eq!(loc("en").percent(1.5, 0), "150%");
    assert_eq!(spaces(&loc("de").percent(1.5, 0)), "150 %");
    assert_eq!(spaces(&loc("fr").percent(1.5, 0)), "150 %");
    assert!(loc("de").percent(1.5, 0).contains(NBSP));
    assert_eq!(loc("en").percent(0.125, 1), "12.5%");
}

/// A number written elsewhere (signed, fixed decimals) takes the same pattern as
/// `percent` in every locale.
#[test]
fn percent_text_matches_percent() {
    for tag in ["en", "de", "fr", "tr", "ar-EG", "he", "ja", "cs", "fi"] {
        let l = loc(tag);
        assert_eq!(
            l.percent_text(&l.number(40.0, 0)),
            l.percent(0.4, 0),
            "{tag}"
        );
        assert_eq!(
            l.percent_text(&l.number_fixed(12.5, 1)),
            l.percent(0.125, 1),
            "{tag}"
        );
    }
    assert_eq!(loc("en").percent_text("+40"), "+40%");
}

/// Catalogues write percentages with `PERCENT()`: the locale's pattern for a number of
/// percent passed as a number or a string, and for a literal.
#[test]
fn percent_function_in_messages() {
    for tag in ["en", "de", "fr", "tr", "ar", "he", "ja"] {
        let l = loc(tag);
        let mut a = FluentArgs::new();
        a.set("percent", 45);
        let volume = plain(l.tr_args("toast-volume", &a));
        assert!(
            volume.contains(&plain(l.percent(0.45, 0))),
            "{tag}: {volume}"
        );
        let mut s = FluentArgs::new();
        s.set("share", 41);
        let share = plain(l.tr_args("tmtv-threshold-share", &s));
        assert!(share.contains(&plain(l.percent(0.41, 0))), "{tag}: {share}");
        assert!(
            plain(l.tr("palette-zoom-200")).contains(&plain(l.percent(2.0, 0))),
            "{tag}"
        );
    }
    let en = loc("en");
    let mut a = FluentArgs::new();
    a.set("percent", 12.5);
    assert_eq!(plain(en.tr_args("toast-volume", &a)), "Volume 12.5%");
    // A formatted number keeps its sign and digits.
    a.set("percent", "+15");
    assert_eq!(plain(en.tr_args("toast-volume", &a)), "Volume +15%");
    assert_eq!(plain(en.tr("palette-zoom-100")), "Zoom to 100%");
    assert_eq!(
        spaces(&plain(loc("de").tr("palette-zoom-100"))),
        "Zoom auf 100 %"
    );
    assert!(plain(loc("tr").tr("palette-interface-reset")).contains("%100"));
}

#[test]
fn byte_sizes() {
    let en = loc("en");
    assert_eq!(plain(en.bytes(1, ByteUnits::Decimal)), "1 byte");
    assert_eq!(plain(en.bytes(999, ByteUnits::Decimal)), "999 bytes");
    assert_eq!(plain(en.bytes(4_200_000, ByteUnits::Decimal)), "4.2 MB");
    assert_eq!(plain(en.bytes(870_000, ByteUnits::Decimal)), "870 kB");
    assert_eq!(plain(en.bytes(3_000_000, ByteUnits::Decimal)), "3 MB");
    assert_eq!(plain(en.bytes(999_999, ByteUnits::Decimal)), "1 MB");
    assert_eq!(plain(en.bytes(1536, ByteUnits::Binary)), "1.5 KiB");
    assert_eq!(plain(en.bytes(5 << 30, ByteUnits::Binary)), "5 GiB");
    assert_eq!(
        spaces(&plain(loc("de").bytes(4_200_000, ByteUnits::Decimal))),
        "4,2 MB"
    );
}

#[test]
fn dates_and_times_per_locale() {
    let en = loc("en");
    assert_eq!(
        en.date(2024, 5, 14, DateStyle::Medium).unwrap(),
        "14 May 2024"
    );
    assert_eq!(en.time(18, 32, None).unwrap(), "18:32");
    assert_eq!(
        en.date_time((2024, 5, 14), (18, 32)).unwrap(),
        "14 May 2024, 18:32"
    );
    assert_eq!(
        loc("en-US").date(2024, 5, 14, DateStyle::Medium).unwrap(),
        "May 14, 2024"
    );
    let us_time = spaces(&loc("en-US").time(18, 32, None).unwrap());
    assert_eq!(us_time, "6:32 PM");
    assert_eq!(
        loc("de").date(2024, 5, 14, DateStyle::Long).unwrap(),
        "14. Mai 2024"
    );
    assert_eq!(
        loc("de").date(2024, 5, 14, DateStyle::Full).unwrap(),
        "Dienstag, 14. Mai 2024"
    );
    assert_eq!(
        loc("fr").date(2024, 5, 14, DateStyle::Long).unwrap(),
        "14 mai 2024"
    );
    assert_eq!(
        loc("ja").date(2024, 5, 14, DateStyle::Long).unwrap(),
        "2024年5月14日"
    );
    assert!(en.date(2024, 2, 30, DateStyle::Medium).is_none());
}

#[test]
fn year_month_headings() {
    assert_eq!(loc("en").year_month(2024, 5).unwrap(), "May 2024");
    assert_eq!(loc("de").year_month(2024, 3).unwrap(), "März 2024");
    // Standalone (nominative), not the genitive of "14. května".
    assert_eq!(loc("cs").year_month(2024, 5).unwrap(), "květen 2024");
    assert_eq!(loc("ja").year_month(2024, 5).unwrap(), "2024年5月");
    assert_eq!(loc("zh-Hans").year_month(2024, 5).unwrap(), "2024年5月");
    assert!(loc("en").year_month(2024, 0).is_none());
    assert!(loc("en").year_month(2024, 13).is_none());
}

#[test]
fn region_formats_override() {
    let l = Localizer::new(&[langid!("en")], Some(&langid!("de")));
    assert_eq!(l.tr("toolbar-next"), "Next image");
    assert_eq!(l.number(1234.5, 1), "1.234,5");
}

#[test]
fn lists() {
    let items = ["JPEG", "PNG", "WebP"];
    assert_eq!(loc("en").list(&items, ListKind::And), "JPEG, PNG and WebP");
    assert_eq!(
        loc("en-US").list(&items, ListKind::And),
        "JPEG, PNG, and WebP"
    );
    assert_eq!(loc("de").list(&items, ListKind::Or), "JPEG, PNG oder WebP");
}

#[test]
fn natural_order() {
    let en = loc("en");
    assert_eq!(en.natural_cmp("img2", "img10"), Ordering::Less);
    assert_eq!(en.natural_cmp("img10", "img2"), Ordering::Greater);
    assert_eq!(en.natural_cmp("img002", "img10"), Ordering::Less);
    assert_eq!(en.natural_cmp("IMG2", "img10"), Ordering::Less);
    assert_eq!(en.natural_cmp("1.jpg", "a.jpg"), Ordering::Less);
    // Equal at primary strength only, then a stable, deterministic tie-break.
    assert_ne!(en.natural_cmp("a.jpg", "A.jpg"), Ordering::Equal);
    assert_eq!(en.natural_cmp("a.jpg", "a.jpg"), Ordering::Equal);
    assert_eq!(
        en.collator_primary().compare("photo", "PHOTO"),
        Ordering::Equal
    );

    let de = loc("de");
    let mut words = vec!["Zebra", "Apfel", "Birne", "Äpfel", "apfel"];
    words.sort_by(|a, b| de.natural_cmp(a, b));
    let pos = |w: &str| words.iter().position(|x| *x == w).unwrap();
    assert!(pos("Äpfel") < pos("Birne"), "{words:?}");
    assert!(
        pos("Apfel") < pos("Birne") && pos("apfel") < pos("Birne"),
        "{words:?}"
    );
    assert_eq!(words.last(), Some(&"Zebra"));
    assert_eq!(
        de.collator_primary().compare("Äpfel", "Apfel"),
        Ordering::Equal
    );

    let mut files = vec!["img10.jpg", "img2.jpg", "img1.jpg", "Img3.jpg"];
    files.sort_by(|a, b| en.natural_cmp(a, b));
    assert_eq!(files, ["img1.jpg", "img2.jpg", "Img3.jpg", "img10.jpg"]);
    // Arabic-Indic digits are numbers too.
    assert_eq!(loc("ar").natural_cmp("صورة٢", "صورة١٠"), Ordering::Less);
}

#[test]
fn pseudo_locales() {
    let xa = loc("en-XA");
    assert_eq!(xa.pseudo(), Some(Pseudo::Accented));
    let s = xa.tr("toolbar-next");
    assert_eq!(s, "Ƞḗḗẋŧ īīḿȧȧɠḗḗ");
    assert_eq!(xa.direction(), Direction::Ltr);
    // Arguments are not transformed.
    let mut args = FluentArgs::new();
    args.set("shortcut", "Ctrl+O");
    assert!(xa.tr_args("empty-drop", &args).contains("Ctrl+O"));

    let xb = loc("ar-XB");
    assert_eq!(xb.pseudo(), Some(Pseudo::Bidi));
    assert_eq!(xb.direction(), Direction::Rtl);
    assert_eq!(xb.tr("toolbar-next"), "\u{202E}Next image\u{202C}");
}

#[test]
fn completeness_and_languages() {
    for tag in [
        "de", "fr", "es", "ja", "zh-Hans", "ar", "he", "it", "nl", "pl", "pt-BR", "pt-PT", "sv",
        "da", "fi", "cs", "uk", "tr",
    ] {
        let c = completeness(&tag.parse().unwrap());
        assert!(c > 0.99, "{tag}: {c}");
    }
    assert_eq!(completeness(&langid!("en")), 1.0);
    assert_eq!(completeness(&langid!("sw")), 0.0);
    // The counts made when built agree with the parsed catalogues.
    for id in catalogue_locales() {
        assert_eq!(completeness(&id), catalog::completeness_parsed(&id), "{id}");
    }

    let langs = available_languages();
    assert_eq!(langs[0].id, langid!("en"));
    assert_eq!(langs.len(), 21);
    let get = |tag: &str| {
        let id: LanguageIdentifier = tag.parse().unwrap();
        langs.iter().find(|l| l.id == id).unwrap().clone()
    };
    assert_eq!(get("de").native_name, "Deutsch");
    assert_eq!(get("de").english_name, "German");
    assert_eq!(get("fr").native_name, "Français");
    assert_eq!(get("ar").native_name, "العربية");
    assert_eq!(get("ar").direction, Direction::Rtl);
    assert_eq!(get("he").direction, Direction::Rtl);
    assert_eq!(get("zh-Hans").native_name, "简体中文");
    assert_eq!(get("ja").native_name, "日本語");
    assert_eq!(get("it").native_name, "Italiano");
    assert_eq!(get("nl").native_name, "Nederlands");
    assert_eq!(get("pl").native_name, "Polski");
    assert_eq!(get("sv").native_name, "Svenska");
    assert_eq!(get("da").native_name, "Dansk");
    assert_eq!(get("fi").native_name, "Suomi");
    assert_eq!(get("cs").native_name, "Čeština");
    assert_eq!(get("uk").native_name, "Українська");
    assert_eq!(get("tr").native_name, "Türkçe");
    // The two Portuguese catalogues are told apart by region in the picker.
    assert_eq!(get("pt-BR").native_name, "Português (Brasil)");
    assert_eq!(get("pt-PT").native_name, "Português (Portugal)");
    assert_eq!(get("pt-BR").english_name, "Portuguese (Brazil)");
    assert_eq!(get("pt-PT").english_name, "Portuguese (Portugal)");
    assert!(get("ar-XB").pseudo && get("ar-XB").direction == Direction::Rtl);
    assert!(langs.iter().rev().take(2).all(|l| l.pseudo));
}

#[test]
fn catalogues_are_consistent() {
    let issues = check_catalogues();
    assert!(issues.is_empty(), "{issues:#?}");
    assert!(english_ids().count() > 350);
}

#[test]
fn handle_switches_live() {
    let i18n = I18n::for_language(langid!("en"));
    let clone = i18n.clone();
    let g0 = i18n.generation();
    assert_eq!(i18n.tr("toolbar-next"), "Next image");
    clone.set_language(langid!("fr"));
    assert!(i18n.generation() > g0);
    assert_eq!(i18n.language(), langid!("fr"));
    assert_eq!(i18n.tr("toolbar-next"), "Image suivante");
    i18n.set_language(langid!("he"));
    assert_eq!(i18n.direction(), Direction::Rtl);
    let g1 = i18n.generation();
    i18n.set_region_formats(Some(langid!("de")));
    assert!(i18n.generation() > g1);
    assert_eq!(i18n.localizer().number(1.5, 1), "1,5");

    fn assert_send_sync<T: Send + Sync + Clone>() {}
    assert_send_sync::<I18n>();
    assert_send_sync::<std::sync::Arc<Localizer>>();
}

/// `with_thread_language` changes the global handle on the calling thread only, nests, is
/// undone on return and on unwinding, and leaves other handles alone.
#[test]
fn thread_language_is_local_to_the_thread() {
    let global = I18n::global();
    let before = global.language();
    let other = I18n::for_language(langid!("fr"));
    let seen_elsewhere = I18n::with_thread_language(langid!("he"), || {
        assert_eq!(global.language(), langid!("he"));
        assert_eq!(global.direction(), Direction::Rtl);
        assert_eq!(t!("toolbar-next"), loc("he").tr("toolbar-next"));
        assert_eq!(other.language(), langid!("fr"));
        I18n::with_thread_language(langid!("de"), || {
            assert_eq!(global.language(), langid!("de"));
        });
        assert_eq!(global.language(), langid!("he"));
        std::thread::spawn(|| I18n::global().language())
            .join()
            .unwrap()
    });
    assert_eq!(seen_elsewhere, before);
    assert_eq!(global.language(), before);
    let unwound = std::panic::catch_unwind(|| {
        I18n::with_thread_language(langid!("ar"), || panic!("inside"));
    });
    assert!(unwound.is_err());
    assert_eq!(global.language(), before);
}

#[test]
fn concurrent_use() {
    let i18n = I18n::for_language(langid!("de"));
    let threads: Vec<_> = (0..4)
        .map(|i| {
            let h = i18n.clone();
            std::thread::spawn(move || {
                for _ in 0..50 {
                    if i == 0 {
                        h.set_language(langid!("ja"));
                        h.set_language(langid!("de"));
                    }
                    assert!(
                        !h.tr_args("status-image-count", &{
                            let mut a = FluentArgs::new();
                            a.set("count", 5);
                            a
                        })
                        .is_empty()
                    );
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn search_folding() {
    assert_eq!(fold_for_search("Équilibre"), "equilibre");
    assert_eq!(fold_for_search("Übersicht"), "ubersicht");
    assert_eq!(fold_for_search("\u{2068}Farbe\u{2069}"), "farbe");
}

/// Fills one Fluent variable with a sample value.
type SetSample = dyn Fn(&mut FluentArgs<'static>, &str);

/// Every ar and he message, with numbers, Latin text and RTL text as arguments: isolates stay
/// paired, and a message holding RTL letters starts a right-to-left paragraph.
#[test]
fn rtl_messages_have_balanced_isolates_and_rtl_direction() {
    for tag in ["ar", "he"] {
        let l = loc(tag);
        let mut checked = 0;
        for id in english_ids() {
            let vars = english_variables(id).unwrap_or_default();
            let samples: [&SetSample; 4] = [
                &|a, v| a.set(v.to_owned(), 1),
                &|a, v| a.set(v.to_owned(), 12345.5),
                &|a, v| a.set(v.to_owned(), "IMG_0001.jpg"),
                &|a, v| a.set(v.to_owned(), "صورة ١"),
            ];
            for sample in samples {
                let mut args = FluentArgs::new();
                for v in &vars {
                    sample(&mut args, v);
                }
                let s = l.tr_args(id, &args);
                assert!(isolates_balanced(&s), "{tag} {id}: {s:?}");
                if s.chars().any(is_rtl_letter) {
                    assert_eq!(
                        base_direction(&s),
                        Some(Direction::Rtl),
                        "{tag} {id}: {s:?}"
                    );
                }
                assert!(!s.contains(FSI), "{tag} {id}: FSI left: {s:?}");
                checked += 1;
            }
        }
        assert!(checked > 400, "{tag}: only {checked} messages");
    }
}

#[test]
fn ltr_messages_are_unchanged() {
    let l = loc("en");
    let mut args = FluentArgs::new();
    args.set("count", 3);
    assert_eq!(
        l.tr_args("status-image-count", &args),
        "\u{2068}3\u{2069} images"
    );
}

/// A localizer formatting and collating for `tag`, whatever the catalogues (collation
/// follows the region-format locale, so these tests don't depend on a catalogue existing).
fn region(tag: &str) -> Localizer {
    Localizer::new(&[langid!("en")], Some(&tag.parse().unwrap()))
}

/// Sorts `words` with the locale's natural (file-name) order.
fn sorted(tag: &str, words: &[&'static str]) -> Vec<&'static str> {
    let l = region(tag);
    let mut v = words.to_vec();
    v.sort_by(|a, b| l.natural_cmp(a, b));
    v
}

#[test]
fn swedish_letters_sort_after_z() {
    // å, ä and ö are letters of their own at the end of the Swedish alphabet, in that order;
    // in German ä sorts with a.
    let words = [
        "öl", "zebra", "äpple", "åsna", "apa", "Ödla", "Ängel", "Zon",
    ];
    assert_eq!(
        sorted("sv", &words),
        [
            "apa", "zebra", "Zon", "åsna", "Ängel", "äpple", "Ödla", "öl"
        ]
    );
    assert_eq!(
        sorted("de", &["zebra", "äpple", "apa"]),
        ["apa", "äpple", "zebra"]
    );
    // At primary strength they stay distinct from a and o.
    assert_ne!(
        region("sv").collator_primary().compare("ä", "a"),
        Ordering::Equal
    );
    assert_eq!(
        region("sv").collator_primary().compare("Ö", "ö"),
        Ordering::Equal
    );
}

#[test]
fn polish_letters_follow_their_base() {
    // ą ć ę ł ń ó ś ź ż are separate letters, each right after its base letter.
    let words = [
        "łódź", "lato", "mapa", "ćma", "cyna", "dom", "żaba", "źrebak", "zebra",
    ];
    assert_eq!(
        sorted("pl", &words),
        [
            "cyna", "ćma", "dom", "lato", "łódź", "mapa", "zebra", "źrebak", "żaba"
        ]
    );
    assert_ne!(
        region("pl").collator_primary().compare("ł", "l"),
        Ordering::Equal
    );
}

#[test]
fn czech_ch_sorts_after_h() {
    // "ch" is one letter between h and i; č, ř, š, ž follow c, r, s, z.
    let words = [
        "ihned", "chata", "hrad", "cesta", "čaj", "dům", "řeka", "rok", "šum", "sůl",
    ];
    assert_eq!(
        sorted("cs", &words),
        [
            "cesta", "čaj", "dům", "hrad", "chata", "ihned", "rok", "řeka", "sůl", "šum"
        ]
    );
    // In English "ch" is just c + h.
    assert_eq!(sorted("en", &["hrad", "chata"]), ["chata", "hrad"]);
}

#[test]
fn turkish_dotted_and_dotless_i() {
    let tr = region("tr");
    let p = tr.collator_primary();
    // Case-insensitive pairs are I/ı and İ/i, not I/i.
    assert_eq!(p.compare("I", "ı"), Ordering::Equal);
    assert_eq!(p.compare("İ", "i"), Ordering::Equal);
    assert_ne!(p.compare("I", "i"), Ordering::Equal);
    // ı sorts before i; ç, ğ, ö, ş, ü follow their base letters.
    let words = ["ilk", "ırmak", "İzmir", "Işık", "hava", "çay", "cam", "dağ"];
    assert_eq!(
        sorted("tr", &words),
        ["cam", "çay", "dağ", "hava", "ırmak", "Işık", "ilk", "İzmir"]
    );
    // English treats I and i as one letter.
    assert_eq!(
        region("en").collator_primary().compare("I", "i"),
        Ordering::Equal
    );
}

/// The plural categories this project's catalogues use for each language (CLDR's cardinal
/// categories for whole numbers, with the Hebrew dual).
fn plural_categories(lang: &str) -> (&'static [&'static str], &'static [&'static str]) {
    // (required, also allowed)
    match lang {
        "ar" => (&["zero", "one", "two", "few", "many", "other"], &[]),
        "he" => (&["one", "two", "other"], &[]),
        "cs" | "pl" | "uk" => (&["one", "few", "many", "other"], &[]),
        "ja" | "zh" => (&["other"], &[]),
        // Turkish: CLDR has `one` (n = 1) and the catalogue uses it in every selector.
        "tr" => (&["one", "other"], &[]),
        // CLDR's `many` here is for millions ("1 000 000 de fichiers"); optional.
        "es" | "fr" | "it" | "pt" => (&["one", "other"], &["many"]),
        _ => (&["one", "other"], &[]),
    }
}

/// Plural-selector helpers over the parsed catalogues.
mod selectors {
    use fluent_syntax::ast;

    /// One select expression: the variable it selects on (none for a function call or a
    /// literal) and its variants.
    pub(super) type Select<'a> = (Option<&'a str>, &'a [ast::Variant<&'a str>]);

    fn selects<'a>(p: &'a ast::Pattern<&'a str>, out: &mut Vec<Select<'a>>) {
        for el in &p.elements {
            if let ast::PatternElement::Placeable { expression } = el {
                expr(expression, out);
            }
        }
    }

    fn expr<'a>(e: &'a ast::Expression<&'a str>, out: &mut Vec<Select<'a>>) {
        match e {
            ast::Expression::Select { selector, variants } => {
                let var = match selector {
                    ast::InlineExpression::VariableReference { id } => Some(id.name),
                    _ => None,
                };
                out.push((var, variants));
                for v in variants {
                    selects(&v.value, out);
                }
            }
            ast::Expression::Inline(ast::InlineExpression::Placeable { expression }) => {
                expr(expression, out)
            }
            ast::Expression::Inline(_) => {}
        }
    }

    /// Each message's selectors: (id, selector).
    pub(super) fn selectors(r: &fluent_bundle::FluentResource) -> Vec<(&str, Select<'_>)> {
        let mut out = Vec::new();
        for e in r.entries() {
            let ast::Entry::Message(m) = e else { continue };
            let mut found = Vec::new();
            for p in m.value.iter().chain(m.attributes.iter().map(|a| &a.value)) {
                selects(p, &mut found);
            }
            out.extend(found.into_iter().map(|v| (m.id.name, v)));
        }
        out
    }

    /// True when a CLDR category other than `other` is a key: a plural selector.
    pub(super) fn is_plural(variants: &[ast::Variant<&str>]) -> bool {
        variants.iter().any(|v| {
            matches!(v.key, ast::VariantKey::Identifier { name }
                if matches!(name, "zero" | "one" | "two" | "few" | "many"))
        })
    }
}

/// Every plural selector (a select expression with a CLDR category key such as `[one]`)
/// uses exactly its locale's categories (see [`plural_categories`]): a missing one silently
/// falls to the default, the wrong grammatical form there (Polish `few`, the Hebrew dual),
/// and one the language lacks is never chosen. Every selector has exactly one default
/// variant, a plural selector's being `*[other]`.
#[test]
fn plural_selectors_use_the_locale_categories() {
    use fluent_syntax::ast;
    use selectors::{is_plural, selectors};

    let mut problems = Vec::new();
    let mut checked = 0;
    for c in catalog::catalogues() {
        let (required, optional) = plural_categories(c.langid.language.as_str());
        for r in &c.resources {
            for (id, (_, variants)) in selectors(r) {
                let defaults: Vec<_> = variants.iter().filter(|v| v.default).collect();
                if defaults.len() != 1 {
                    problems.push(format!("{}: {id}: {} defaults", c.langid, defaults.len()));
                }
                if !is_plural(variants) {
                    continue; // `[true]`, `[jpeg]`…
                }
                checked += 1;
                let keys: Vec<&str> = variants
                    .iter()
                    .filter_map(|v| match v.key {
                        ast::VariantKey::Identifier { name } => Some(name),
                        ast::VariantKey::NumberLiteral { .. } => None,
                    })
                    .collect();
                for k in required {
                    if !keys.contains(k) {
                        problems.push(format!("{}: {id}: no [{k}]", c.langid));
                    }
                }
                for k in &keys {
                    if !required.contains(k) && !optional.contains(k) {
                        problems.push(format!("{}: {id}: [{k}] is not a category here", c.langid));
                    }
                }
                if let Some(d) = defaults.first()
                    && !matches!(d.key, ast::VariantKey::Identifier { name: "other" })
                {
                    problems.push(format!("{}: {id}: the default is not [other]", c.langid));
                }
            }
        }
    }
    assert!(checked > 1000, "only {checked} plural selectors found");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Plural forms and default markers agree with English: where a translation picks a form by
/// a count, English does too (an English message without `[one]` reads "1 files" or
/// "1 … were"), on the same variable; and every select's default is the same key as
/// English's for that variable. Translations may stay flat where English has a plural
/// ("Files: 3" in Arabic and Hebrew). The exceptions are counts that are never 1.
#[test]
fn plural_forms_and_defaults_agree_with_english() {
    use fluent_syntax::ast;
    use selectors::{is_plural, selectors};
    use std::collections::{BTreeMap, BTreeSet};

    // (message, variable): the count is always 2 or more where the message is used.
    const NEVER_ONE: &[(&str, &str)] = &[
        ("gallery-run-on", "count"),      // a run of 2 or more files
        ("gallery-badge-pages", "count"), // shown only above 1 page
        ("player-channels", "count"),     // 1 and 2 channels have their own words
        ("hint-pages", "count"),          // the first file with several pages
    ];
    let en = catalog::find(&langid!("en")).expect("en catalogue");
    // English: (message, variable) of each plural selector, and each select's default key.
    let mut en_plural = BTreeSet::new();
    let mut en_default: BTreeMap<(&str, &str), &str> = BTreeMap::new();
    let key = |v: &ast::Variant<&'static str>| match v.key {
        ast::VariantKey::Identifier { name } => name,
        ast::VariantKey::NumberLiteral { value } => value,
    };
    for r in &en.resources {
        for (id, (var, variants)) in selectors(r) {
            let Some(var) = var else { continue };
            if is_plural(variants) {
                en_plural.insert((id, var));
            }
            if let Some(d) = variants.iter().find(|v| v.default) {
                en_default.insert((id, var), key(d));
            }
        }
    }
    let mut problems = Vec::new();
    for c in catalog::catalogues() {
        if c.langid == en.langid {
            continue;
        }
        for r in &c.resources {
            for (id, (var, variants)) in selectors(r) {
                let Some(var) = var else { continue };
                if is_plural(variants)
                    && !en_plural.contains(&(id, var))
                    && !NEVER_ONE.contains(&(id, var))
                {
                    problems.push(format!(
                        "{}: {id}: plural on ${var}, English has none",
                        c.langid
                    ));
                }
                let Some(&want) = en_default.get(&(id, var)) else {
                    continue;
                };
                // A plural selector's default is `other`, as English's.
                let want = if is_plural(variants) { "other" } else { want };
                if let Some(d) = variants.iter().find(|v| v.default)
                    && key(d) != want
                {
                    problems.push(format!(
                        "{}: {id}: default *[{}], English *[{want}]",
                        c.langid,
                        key(d)
                    ));
                }
            }
        }
    }
    assert!(
        en_plural.len() > 200,
        "only {} English plurals",
        en_plural.len()
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn slavic_and_portuguese_plurals() {
    let count = |tag: &str, n: u32| plain(t!(&loc(tag) => "status-image-count", count = n));
    // Polish: one, few (2–4, 22–24 but not 12–14), many.
    assert_eq!(count("pl", 1), "1 obraz");
    assert_eq!(count("pl", 3), "3 obrazy");
    assert_eq!(count("pl", 5), "5 obrazów");
    assert_eq!(count("pl", 12), "12 obrazów");
    assert_eq!(count("pl", 22), "22 obrazy");
    // Czech: one, few (2–4), other (5+); "many" is only for fractions.
    assert_eq!(count("cs", 1), "1 obrázek");
    assert_eq!(count("cs", 4), "4 obrázky");
    assert_eq!(count("cs", 5), "5 obrázků");
    // Ukrainian: like Polish, but 21 is "one".
    assert_eq!(count("uk", 21), "21 зображення");
    assert_eq!(count("uk", 11), "11 зображень");
    assert_eq!(count("uk", 23), "23 зображення");
    // Brazilian Portuguese counts 0 as singular (CLDR "one" is i = 0..1); European does not.
    assert_eq!(count("pt-BR", 0), "0 imagem");
    assert_eq!(count("pt-PT", 0), "0 imagens");
    assert_eq!(count("pt-PT", 1), "1 imagem");
    // Region-specific number formatting follows the user's tag.
    assert_eq!(spaces(&count("pt-PT", 15000)), "15 000 imagens");
    assert_eq!(count("pt-BR", 15000), "15.000 imagens");
}

/// Byte sizes pick their unit's message at run time; each gets `$value`.
#[test]
fn byte_sizes_get_their_arguments() {
    let l = Localizer::english();
    crate::record_argument_problems(true);
    for units in [ByteUnits::Decimal, ByteUnits::Binary] {
        let mut b = 1u64;
        for _ in 0..60 {
            let _ = l.bytes(b, units);
            b = b.saturating_mul(3);
        }
    }
    let problems = crate::argument_problems();
    crate::record_argument_problems(false);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// A message formatted with an argument it does not use, or without one it does, is
/// recorded while recording is on (on this thread only).
#[test]
fn argument_problems_are_recorded_on_this_thread() {
    let l = Localizer::english();
    let mut args = FluentArgs::new();
    args.set("value", 3);
    args.set("colour", "red");
    crate::record_argument_problems(true);
    let _ = l.tr_args("unit-percent", &args);
    let _ = l.tr_args("unit-percent", &FluentArgs::new());
    let got = crate::argument_problems();
    crate::record_argument_problems(false);
    assert_eq!(
        got,
        vec![
            "unit-percent: missing [\"value\"], not in the message []".to_owned(),
            "unit-percent: missing [], not in the message [\"colour\"]".to_owned(),
        ]
    );
    // Not recording: nothing kept.
    let _ = l.tr_args("unit-percent", &FluentArgs::new());
    assert!(crate::argument_problems().is_empty());
}

/// In Hebrew and Arabic the σ, px or % written after a number stays on its right
/// (cull mode's "median + 2σ" read "σ2").
#[test]
fn rtl_units_stay_with_their_numbers() {
    for lang in ["he", "ar"] {
        let l = loc(lang);
        let mut args = FluentArgs::new();
        args.set("k", "2");
        let s = l.tr_args("cull-astro-suggest-none", &args);
        assert!(s.contains("\u{2066}2σ\u{2069}"), "{lang}: {s:?}");
        let mut args = FluentArgs::new();
        args.set("value", "3.1");
        let s = l.tr_args("cull-astro-line-hfd", &args);
        assert!(s.contains("\u{2066}3.1 px\u{2069}"), "{lang}: {s:?}");
    }
}

/// Every catalogue writes millimetres one way: a measured length ("60.0 מ״מ" in Hebrew)
/// carries the unit the slab, position, area and linked-view lines use, so no overlay shows
/// "mm" in a language that writes "מ״מ", "مم" or "мм" elsewhere.
#[test]
fn millimetres_are_written_one_way_per_language() {
    let mut checked = 0;
    for id in catalog::catalogue_locales() {
        let l = Localizer::new(std::slice::from_ref(&id), None);
        let unit = |msg: &str, args: &FluentArgs| -> String {
            let s = plain(l.tr_args(msg, args));
            let i = s.find("42").unwrap_or_else(|| panic!("{id} {msg}: {s}"));
            s[i + 2..]
                .trim_start()
                .split(|c: char| c.is_whitespace() || c == '²' || c == ')')
                .next()
                .unwrap_or_default()
                .to_owned()
        };
        let mut v = FluentArgs::new();
        v.set("value", "42");
        let length = unit("measure-length-mm", &v);
        // The symbol the code writes after a number it formats itself.
        assert_eq!(l.tr("unit-symbol-mm"), length, "{id}: unit-symbol-mm");
        // Square millimetres are the length symbol with ², except in Hebrew, which writes
        // them as it writes m² and km².
        let mm2 = l.tr("unit-symbol-mm2");
        let area = mm2.strip_suffix('²').unwrap_or(&mm2);
        assert_eq!(unit("measure-area-mm", &v), area, "{id}: area");
        if id != "he" {
            assert_eq!(area, length, "{id}: unit-symbol-mm2");
        }
        let mut p = FluentArgs::new();
        p.set("view", "X");
        p.set("position", "42");
        assert_eq!(unit("mpr-position", &p), length, "{id}: MPR position");
        let mut s = FluentArgs::new();
        s.set("n", 1);
        s.set("count", 2);
        s.set("position", "42");
        assert_eq!(unit("linked-slice", &s), length, "{id}: linked slice");
        checked += 1;
    }
    assert!(checked >= 19, "{checked} catalogues");
    let he = loc("he");
    let mut v = FluentArgs::new();
    v.set("value", "60.0");
    assert_eq!(plain(he.tr_args("measure-length-mm", &v)), "60.0 מ״מ");
}

/// The Hebrew symbols of the everyday units: what the code
/// writes after a number it formats itself, and what messages write themselves.
const HEBREW_UNITS: [(&str, &str); 14] = [
    ("mm", "מ״מ"),
    ("cm", "ס״מ"),
    ("m", "מ׳"),
    ("km", "ק״מ"),
    ("mm²", "ממ״ר"),
    ("cm²", "סמ״ר"),
    ("m²", "מ״ר"),
    ("km²", "קמ״ר"),
    ("ha", "הקטר"),
    ("ft", "רגל"),
    ("ft²", "רגל רבוע"),
    ("mi", "מייל"),
    ("mi²", "מייל רבוע"),
    ("ac", "אקר"),
];

/// In Hebrew each everyday unit is written one way everywhere: [`unit_symbol_in`] gives the
/// abbreviations with gershayim and geresh (U+05F4, U+05F3), messages that write a unit
/// themselves use the same ones (the map export's "ק״מ"), and no Hebrew catalogue (this
/// crate's or the Info panel's) writes a Latin km, m, cm, mm, ha, ft, mi or ac next to a
/// number, a full word for one, or an abbreviation with ASCII quotes. µm, nm and Å stay
/// Latin. Other languages keep the SI and imperial symbols (Arabic and Ukrainian their
/// millimetre, Arabic in both catalogues).
#[test]
fn hebrew_units_are_written_one_way_everywhere() {
    let he = loc("he");
    let en = loc("en");
    assert_eq!(
        translatable_unit_symbols().collect::<Vec<_>>(),
        HEBREW_UNITS.map(|(s, _)| s).to_vec()
    );
    for (sym, want) in HEBREW_UNITS {
        assert_eq!(unit_symbol_in(&he, sym), want, "{sym}");
        assert_eq!(unit_symbol_in(&en, sym), sym, "en {sym}");
        assert!(
            !want.contains(['"', '\'']),
            "{sym}: ASCII quote in {want:?}"
        );
    }
    for kept in ["µm", "nm", "Å", "px", "°C", "mm/s"] {
        assert_eq!(unit_symbol_in(&he, kept), kept);
    }
    // Arabic and Ukrainian: only millimetres are their own.
    for (lang, mm) in [("ar", "مم"), ("uk", "мм")] {
        let l = loc(lang);
        for (sym, _) in HEBREW_UNITS {
            let want = match sym {
                "mm" => mm.to_owned(),
                "mm²" => format!("{mm}²"),
                s => s.to_owned(),
            };
            assert_eq!(unit_symbol_in(&l, sym), want, "{lang} {sym}");
        }
    }
    // A message that writes the unit itself.
    let mut a = FluentArgs::new();
    a.set("width", "42");
    a.set("height", "42");
    a.set("across", "42");
    a.set("down", "42");
    let export = plain(he.tr_args("map-export-size", &a));
    assert!(export.contains("42 ק״מ"), "{export}");
    // No other form in any Hebrew catalogue.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dirs = [
        root.join("i18n/he"),
        root.join("../specular-metadata/i18n/he"),
    ];
    let units = "mm|cm|m|km|ha|ft|mi|ac";
    let end = r"(²)?(\s|$|[),.;·/])";
    let latin = regex::Regex::new(&format!(
        r"\}}\s*({units}){end}|(^|[\s=(])({units})(²)?\s*\{{"
    ))
    .unwrap();
    let words = "מטר|מטרים|קילומטר|קילומטרים|סנטימטר|סנטימטרים|מילימטר|מילימטרים|\
                 הקטרים|רגליים|מיילים|אקרים";
    let word = regex::Regex::new(&format!(r"\}}\s*({words})(\s|$|[),.;·])")).unwrap();
    let ascii = regex::Regex::new(r#"\}\s*[א-ת]+["'][א-ת]*"#).unwrap();
    let mut files = 0;
    let mut bad = Vec::new();
    for dir in &dirs {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
            .collect();
        entries.sort();
        for f in entries {
            files += 1;
            let text = std::fs::read_to_string(&f).unwrap();
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with('#') {
                    continue;
                }
                // Compound units (mm/s, cd/m², m/z) stay as they are written.
                let line_wo = line.replace("mm/s", "").replace("mm/mV", "");
                if latin.is_match(&line_wo) || word.is_match(line) || ascii.is_match(line) {
                    bad.push(format!(
                        "{}:{}: {line}",
                        f.file_name().unwrap().to_string_lossy(),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(files > 40, "{files} Hebrew catalogues");
    assert!(
        bad.is_empty(),
        "units not written as Hebrew writes them:\n{}",
        bad.join("\n")
    );
}

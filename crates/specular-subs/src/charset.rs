//! The character set of a subtitle file: a byte-order mark says it (UTF-8, UTF-16 LE/BE);
//! valid UTF-8 is UTF-8; anything else is guessed among the legacy code pages subtitles are
//! still shared in (Windows-125x, ISO-8859-x, KOI8-R, Shift_JIS, GBK, Big5, EUC-KR).
//!
//! The guess scores each decoding by how plausible its words are: words whose letters are
//! all in one script, and the share of that script's most common letters. A language hint
//! (the file name's `.he` or the interface language) breaks near ties.

use encoding_rs::Encoding;

/// The character sets a user can force, in menu order.
pub const CHOICES: &[&Encoding] = &[
    encoding_rs::UTF_8,
    encoding_rs::UTF_16LE,
    encoding_rs::WINDOWS_1252,
    encoding_rs::WINDOWS_1250,
    encoding_rs::WINDOWS_1251,
    encoding_rs::KOI8_R,
    encoding_rs::WINDOWS_1253,
    encoding_rs::WINDOWS_1254,
    encoding_rs::WINDOWS_1255,
    encoding_rs::WINDOWS_1256,
    encoding_rs::WINDOWS_1257,
    encoding_rs::WINDOWS_874,
    encoding_rs::WINDOWS_1258,
    encoding_rs::SHIFT_JIS,
    encoding_rs::EUC_JP,
    encoding_rs::GBK,
    encoding_rs::BIG5,
    encoding_rs::EUC_KR,
];

/// Single-byte candidates for detection.
const SINGLE: &[&Encoding] = &[
    encoding_rs::WINDOWS_1252,
    encoding_rs::WINDOWS_1250,
    encoding_rs::WINDOWS_1251,
    encoding_rs::KOI8_R,
    encoding_rs::WINDOWS_1253,
    encoding_rs::WINDOWS_1254,
    encoding_rs::WINDOWS_1255,
    encoding_rs::WINDOWS_1256,
    encoding_rs::WINDOWS_1257,
    encoding_rs::WINDOWS_874,
];

const MULTI: &[&Encoding] = &[
    encoding_rs::SHIFT_JIS,
    encoding_rs::EUC_JP,
    encoding_rs::GBK,
    encoding_rs::BIG5,
    encoding_rs::EUC_KR,
];

/// Decodes `bytes` (a BOM is removed). `force` overrides detection.
pub fn decode(
    bytes: &[u8],
    force: Option<&'static Encoding>,
    hint: Option<&str>,
) -> (String, &'static Encoding) {
    if let Some((enc, bom)) = Encoding::for_bom(bytes) {
        let enc = force.unwrap_or(enc);
        let body = if force.is_none() {
            &bytes[bom..]
        } else {
            bytes
        };
        let (s, _) = enc.decode_without_bom_handling(body);
        return (s.trim_start_matches('\u{feff}').to_owned(), enc);
    }
    let enc = force.unwrap_or_else(|| detect(bytes, hint));
    let (s, _) = enc.decode_without_bom_handling(bytes);
    (s.into_owned(), enc)
}

/// Guesses the character set of text without a BOM.
pub fn detect(bytes: &[u8], hint: Option<&str>) -> &'static Encoding {
    if std::str::from_utf8(bytes).is_ok() {
        return encoding_rs::UTF_8;
    }
    if let Some(e) = utf16_without_bom(bytes) {
        return e;
    }
    // Multi-byte sets: no errors, and most non-ASCII characters are CJK or kana/hangul.
    let mut best_multi: Option<(&'static Encoding, f64)> = None;
    for &e in MULTI {
        let (s, had_errors) = e.decode_without_bom_handling(bytes);
        if had_errors {
            continue;
        }
        let (mut cjk, mut other, mut kana, mut hangul) = (0usize, 0usize, 0usize, 0usize);
        for c in s.chars().filter(|c| !c.is_ascii()) {
            if matches!(c as u32, 0x3000..=0x303F | 0xFF00..=0xFFEF) {
                // Full-width punctuation and forms: neutral.
                continue;
            }
            match script(c) {
                Script::Han => cjk += 1,
                Script::Kana => kana += 1,
                Script::Hangul => hangul += 1,
                _ => other += 1,
            }
        }
        // GBK and EUC-KR text keeps to their core sets (both bytes 0xA1 and up); Japanese
        // read as them lands in the rarely used extensions.
        if (e == encoding_rs::GBK || e == encoding_rs::EUC_KR) && core_share(bytes) < 0.9 {
            continue;
        }
        let total = cjk + kana + hangul + other;
        if total == 0 {
            continue;
        }
        let good = cjk + kana + hangul;
        let mut score = good as f64 / total as f64;
        // Japanese has kana throughout, Korean is nearly all hangul.
        if (e == encoding_rs::SHIFT_JIS || e == encoding_rs::EUC_JP) && kana * 10 >= good {
            score += 0.2;
        }
        if e == encoding_rs::EUC_KR {
            if hangul * 10 < good * 8 {
                continue;
            }
            score += 0.2;
        }
        if e == encoding_rs::GBK && hint.is_some_and(|h| h.starts_with("zh")) {
            score += 0.05;
        }
        if score > 0.9 && best_multi.is_none_or(|(_, b)| score > b) {
            best_multi = Some((e, score));
        }
    }
    if let Some((e, _)) = best_multi {
        return e;
    }
    let hinted = hint.and_then(lang_encoding);
    let mut best = (encoding_rs::WINDOWS_1252, f64::MIN);
    for &e in SINGLE {
        let (s, _) = e.decode_without_bom_handling(bytes);
        let mut score = plausibility(&s);
        if Some(e) == hinted {
            score += 0.15;
        }
        if score > best.1 {
            best = (e, score);
        }
    }
    best.0
}

/// The share of two-byte pairs whose bytes are both 0xA1 or more.
fn core_share(bytes: &[u8]) -> f64 {
    let (mut core, mut all) = (0usize, 0usize);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] < 0x80 {
            i += 1;
            continue;
        }
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        all += 1;
        if bytes[i] >= 0xA1 && next >= 0xA1 {
            core += 1;
        }
        i += 2;
    }
    if all == 0 {
        1.0
    } else {
        core as f64 / all as f64
    }
}

/// UTF-16 without a BOM: every other byte zero in mostly-ASCII text.
fn utf16_without_bom(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let n = bytes.len() / 2;
    let even_zero = bytes.iter().step_by(2).filter(|&&b| b == 0).count();
    let odd_zero = bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
    if odd_zero * 10 > n * 7 && even_zero * 10 < n {
        Some(encoding_rs::UTF_16LE)
    } else if even_zero * 10 > n * 7 && odd_zero * 10 < n {
        Some(encoding_rs::UTF_16BE)
    } else {
        None
    }
}

/// The legacy code page usual for a language code.
fn lang_encoding(lang: &str) -> Option<&'static Encoding> {
    let l = lang.split(['-', '_']).next()?.to_ascii_lowercase();
    Some(match l.as_str() {
        "he" | "heb" | "iw" => encoding_rs::WINDOWS_1255,
        "ar" | "ara" | "fa" | "fas" | "per" | "ur" => encoding_rs::WINDOWS_1256,
        "ru" | "rus" | "uk" | "ukr" | "bg" | "bul" | "sr" | "be" | "mk" => {
            encoding_rs::WINDOWS_1251
        }
        "el" | "ell" | "gre" => encoding_rs::WINDOWS_1253,
        "tr" | "tur" => encoding_rs::WINDOWS_1254,
        "pl" | "pol" | "cs" | "cze" | "ces" | "sk" | "hu" | "hun" | "ro" | "hr" | "sl" => {
            encoding_rs::WINDOWS_1250
        }
        "lt" | "lv" | "et" => encoding_rs::WINDOWS_1257,
        "th" | "tha" => encoding_rs::WINDOWS_874,
        _ => encoding_rs::WINDOWS_1252,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum Script {
    Latin,
    Greek,
    Cyrillic,
    Hebrew,
    Arabic,
    Thai,
    Han,
    Kana,
    Hangul,
    /// Punctuation, symbols, controls.
    Other,
}

fn script(c: char) -> Script {
    let u = c as u32;
    if c.is_ascii_alphabetic() {
        return Script::Latin;
    }
    match u {
        0xC0..=0x24F if c.is_alphabetic() => Script::Latin,
        0x370..=0x3FF => Script::Greek,
        0x400..=0x52F => Script::Cyrillic,
        0x5D0..=0x5EA => Script::Hebrew,
        0x620..=0x64A | 0x66E..=0x6D3 | 0x6FA..=0x6FF => Script::Arabic,
        0xE01..=0xE5B => Script::Thai,
        0x3040..=0x30FF => Script::Kana,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF => Script::Han,
        0xAC00..=0xD7AF | 0x1100..=0x11FF => Script::Hangul,
        _ => Script::Other,
    }
}

/// Letter frequencies (percent) of each script's main language, for the profile match.
fn profile(s: Script) -> &'static [(char, f64)] {
    match s {
        Script::Hebrew => &[
            ('י', 11.1),
            ('ו', 10.4),
            ('ה', 8.7),
            ('ל', 7.3),
            ('א', 6.3),
            ('מ', 6.1),
            ('ר', 5.6),
            ('ת', 5.3),
            ('ב', 4.7),
            ('ש', 4.4),
            ('נ', 4.2),
            ('כ', 3.0),
            ('ע', 3.0),
            ('ד', 2.6),
            ('ח', 2.4),
            ('ק', 2.2),
            ('ם', 2.0),
            ('פ', 1.8),
            ('ן', 1.5),
            ('ג', 1.2),
            ('ז', 1.0),
            ('ס', 1.0),
            ('ט', 0.9),
            ('צ', 0.8),
        ],
        Script::Arabic => &[
            ('ا', 16.0),
            ('ل', 11.0),
            ('ي', 8.0),
            ('م', 6.0),
            ('و', 6.0),
            ('ن', 6.0),
            ('ر', 4.5),
            ('ه', 4.0),
            ('ت', 4.0),
            ('ب', 3.6),
            ('ة', 3.0),
            ('ع', 3.0),
            ('ك', 2.7),
            ('د', 2.6),
            ('ف', 2.6),
            ('ق', 2.3),
            ('س', 2.0),
            ('أ', 2.0),
            ('ح', 1.8),
            ('ج', 1.4),
            ('ى', 1.2),
            ('إ', 1.0),
            ('ش', 1.0),
            ('ص', 0.8),
            ('خ', 0.8),
            ('ذ', 0.7),
            ('ط', 0.6),
            ('ز', 0.5),
            ('ث', 0.5),
            ('ض', 0.4),
            ('غ', 0.4),
        ],
        Script::Cyrillic => &[
            ('о', 11.0),
            ('е', 8.5),
            ('а', 8.0),
            ('и', 7.4),
            ('н', 6.7),
            ('т', 6.3),
            ('с', 5.5),
            ('р', 4.7),
            ('в', 4.5),
            ('л', 4.4),
            ('к', 3.5),
            ('м', 3.2),
            ('д', 3.0),
            ('п', 2.8),
            ('у', 2.6),
            ('я', 2.0),
            ('ы', 1.9),
            ('ь', 1.7),
            ('г', 1.7),
            ('з', 1.6),
            ('б', 1.6),
            ('ч', 1.4),
            ('й', 1.2),
            ('х', 1.0),
            ('ж', 0.9),
            ('ш', 0.7),
            ('ю', 0.6),
            ('ц', 0.5),
            ('щ', 0.4),
            ('э', 0.3),
            ('ф', 0.3),
            ('і', 0.5),
            ('ї', 0.2),
            ('є', 0.2),
        ],
        Script::Greek => &[
            ('α', 11.0),
            ('ο', 9.5),
            ('ι', 8.6),
            ('ε', 8.0),
            ('τ', 8.0),
            ('σ', 7.5),
            ('ν', 6.5),
            ('η', 5.0),
            ('ρ', 4.5),
            ('υ', 4.0),
            ('π', 4.0),
            ('κ', 4.0),
            ('μ', 3.5),
            ('λ', 2.8),
            ('ω', 2.0),
            ('γ', 1.8),
            ('δ', 1.7),
            ('θ', 1.3),
            ('χ', 1.2),
            ('φ', 0.8),
            ('β', 0.7),
            ('ξ', 0.4),
            ('ζ', 0.4),
            ('ψ', 0.2),
        ],
        _ => &[],
    }
}

/// A letter folded for the profile: lower case, Greek accents and final sigma removed.
fn fold(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'ά' => 'α',
        'έ' => 'ε',
        'ή' => 'η',
        'ί' | 'ϊ' | 'ΐ' => 'ι',
        'ό' => 'ο',
        'ύ' | 'ϋ' | 'ΰ' => 'υ',
        'ώ' => 'ω',
        'ς' => 'σ',
        _ => c,
    }
}

/// Accented letters common in the Latin-script languages of the Windows-125x pages.
fn common_latin(c: char) -> bool {
    "éèàçùâêîôûëïüœáíóúñãõäöüßąćęłńśźżčďěňřšťůžőűğışşİ"
        .contains(c.to_lowercase().next().unwrap_or(c))
}

/// Cosine similarity of the counted letters with a script's profile.
fn match_profile(s: Script, counts: &std::collections::HashMap<char, f64>) -> f64 {
    let p = profile(s);
    if p.is_empty() {
        return 0.5;
    }
    let (mut dot, mut na, mut nb) = (0.0, 0.0, 0.0);
    for (c, n) in counts {
        na += n * n;
        if let Some((_, f)) = p.iter().find(|(pc, _)| pc == c) {
            dot += n * f;
        }
    }
    for (_, f) in p {
        nb += f * f;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// How plausible decoded text is: per word, letters of one script score, mixed scripts
/// and controls cost; letters of a non-Latin script must also look like its language's
/// letter frequencies.
fn plausibility(s: &str) -> f64 {
    let mut score = 0.0f64;
    let mut letters = 0usize;
    let mut hist: std::collections::HashMap<Script, std::collections::HashMap<char, f64>> =
        std::collections::HashMap::new();
    // Symbols and controls a text would not hold cost; the usual punctuation is free.
    for c in s.chars().filter(|c| !c.is_ascii() && !c.is_alphabetic()) {
        if !"،؛؟«»–—‘’‚“”„…\u{a0}·°€".contains(c) {
            score -= 2.0;
            letters += 1;
        }
    }
    for word in s.split(|c: char| !c.is_alphabetic()) {
        let non_ascii: Vec<char> = word.chars().filter(|c| !c.is_ascii()).collect();
        if non_ascii.is_empty() {
            continue;
        }
        let scripts: Vec<Script> = non_ascii.iter().map(|&c| script(c)).collect();
        let first = scripts[0];
        let ascii_letters = word.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let pure = scripts.iter().all(|&s| s == first);
        let others = scripts.iter().filter(|&&s| s == Script::Other).count();
        letters += non_ascii.len();
        score -= 2.0 * others as f64;
        if !pure || first == Script::Other {
            score -= non_ascii.len() as f64;
            continue;
        }
        if first == Script::Latin {
            // Accented letters live among plain ones: a word of only accented letters is a
            // misread non-Latin word.
            if ascii_letters == 0 && non_ascii.len() > 1 {
                score -= non_ascii.len() as f64;
            } else {
                score += non_ascii.iter().filter(|&&c| common_latin(c)).count() as f64;
            }
        } else {
            if ascii_letters > 0 {
                // A non-Latin letter inside a Latin word: a misread accent.
                score -= (non_ascii.len() + ascii_letters) as f64;
                continue;
            }
            score += non_ascii.len() as f64 * 0.5;
            let h = hist.entry(first).or_default();
            for &c in &non_ascii {
                *h.entry(fold(c)).or_default() += 1.0;
            }
        }
    }
    if letters == 0 {
        return 0.0;
    }
    let mut total = score / letters as f64;
    // The main non-Latin script's letters against its language's frequencies.
    if let Some((sc, h)) = hist.iter().max_by(|a, b| {
        a.1.values()
            .sum::<f64>()
            .total_cmp(&b.1.values().sum::<f64>())
    }) {
        let share = h.values().sum::<f64>() / letters as f64;
        total += 2.0 * match_profile(*sc, h) * share.min(1.0);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(e: &'static Encoding, s: &str) -> Vec<u8> {
        e.encode(s).0.into_owned()
    }

    #[test]
    fn boms_and_utf8() {
        let (s, e) = decode(b"\xef\xbb\xbfhello", None, None);
        assert_eq!((s.as_str(), e), ("hello", encoding_rs::UTF_8));
        let le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("שלום".encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        let (s, e) = decode(&le, None, None);
        assert_eq!((s.as_str(), e), ("שלום", encoding_rs::UTF_16LE));
        let be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("مرحبا".encode_utf16().flat_map(|u| u.to_be_bytes()))
            .collect();
        assert_eq!(decode(&be, None, None).0, "مرحبا");
        assert_eq!(detect("ça va".as_bytes(), None), encoding_rs::UTF_8);
    }

    #[test]
    fn legacy_code_pages() {
        let cases: &[(&'static Encoding, &str)] = &[
            (
                encoding_rs::WINDOWS_1255,
                "1\n00:00:01,000 --> 00:00:02,000\nשלום, מה שלומך היום? אני בסדר גמור, תודה רבה לך.\n",
            ),
            (
                encoding_rs::WINDOWS_1256,
                "مرحبا، كيف حالك اليوم؟ أنا بخير، شكرا جزيلا لك على السؤال.\n",
            ),
            (
                encoding_rs::WINDOWS_1251,
                "Привет, как у тебя дела сегодня? У меня всё хорошо, спасибо.\n",
            ),
            (
                encoding_rs::WINDOWS_1252,
                "Ça va très bien, merci beaucoup. Où est la bibliothèque? Déjà vu.\n",
            ),
            (
                encoding_rs::WINDOWS_1253,
                "Γεια σου, τι κάνεις σήμερα; Είμαι πολύ καλά, ευχαριστώ.\n",
            ),
            (
                encoding_rs::WINDOWS_1250,
                "Dzień dobry, jak się masz? Wszystko w porządku, dziękuję bardzo.\n",
            ),
            (
                encoding_rs::SHIFT_JIS,
                "こんにちは、今日は元気ですか？はい、元気です。\n",
            ),
            (
                encoding_rs::GBK,
                "你好，你今天好吗？我很好，谢谢你的关心。\n",
            ),
            (
                encoding_rs::EUC_KR,
                "안녕하세요, 오늘 어떻게 지내세요? 저는 잘 지내요.\n",
            ),
        ];
        for (e, text) in cases {
            let bytes = enc(e, text);
            let got = detect(&bytes, None);
            assert_eq!(got, *e, "{} detected as {}", e.name(), got.name());
            let (s, _) = decode(&bytes, None, None);
            assert_eq!(s, *text);
        }
    }

    #[test]
    fn forced_set_wins() {
        let bytes = enc(encoding_rs::WINDOWS_1255, "שלום");
        let (s, e) = decode(&bytes, Some(encoding_rs::WINDOWS_1252), None);
        assert_eq!(e, encoding_rs::WINDOWS_1252);
        assert_ne!(s, "שלום");
    }
}

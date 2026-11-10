//! Settings search (the Settings window's search field and the command palette).

use specular_i18n::{I18n, Localizer, fold_for_search};

use crate::meta::{self, SettingMeta};

const W_LABEL: u32 = 100;
const W_LABEL_EN: u32 = 60;
const W_KEY: u32 = 50;
const W_OPTION: u32 = 15;
const W_HELP: u32 = 20;
const W_HELP_EN: u32 = 10;
const W_SECTION: u32 = 10;

struct Field {
    text: String,
    weight: u32,
}

fn fields(m: &SettingMeta, loc: &Localizer, en: &Localizer) -> Vec<Field> {
    let mut out = Vec::new();
    let mut push = |text: String, weight: u32| {
        let text = fold_for_search(&text).into_owned();
        if !text.is_empty() {
            out.push(Field { text, weight });
        }
    };
    push(loc.tr(m.label_id), W_LABEL);
    push(loc.tr(m.help_id), W_HELP);
    push(m.key.to_owned(), W_KEY);
    push(m.key.replace(['_', '.'], " "), W_KEY);
    push(loc.tr(m.section.label_id()), W_SECTION);
    for o in m.options() {
        push(loc.tr(&m.option_id(o)), W_OPTION);
    }
    if loc.language() != en.language() || loc.pseudo().is_some() {
        push(en.tr(m.label_id), W_LABEL_EN);
        push(en.tr(m.help_id), W_HELP_EN);
        for o in m.options() {
            push(en.tr(&m.option_id(o)), W_OPTION / 2);
        }
    }
    out
}

/// Score of one query word in one field: whole word > word prefix > substring.
fn word_score(field: &str, word: &str) -> f32 {
    let mut best = 0.0f32;
    for (i, _) in field.match_indices(word) {
        let at_word_start = field[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let end = i + word.len();
        let at_word_end = field[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        let s = match (at_word_start, at_word_end) {
            (true, true) => 2.0,
            (true, false) => 1.5,
            _ => 1.0,
        };
        best = best.max(s);
    }
    // Scripts without spaces (ja, zh) still match by substring, which scores 1.0 above.
    best
}

/// Settings matching `query`, best first. Every word of the query must match the
/// localised or English label, help text, option names, section title or key. Matching
/// ignores case, accents and Arabic/Hebrew vowel marks. An empty query returns every
/// setting in window order.
pub fn search(query: &str, i18n: &I18n) -> Vec<&'static SettingMeta> {
    let loc = i18n.localizer();
    search_with(query, &loc)
}

pub(crate) fn search_with(query: &str, loc: &Localizer) -> Vec<&'static SettingMeta> {
    let folded = fold_for_search(query);
    let words: Vec<&str> = folded
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return meta::all().iter().filter(|m| m.applies_here()).collect();
    }
    let en = Localizer::english();
    let mut hits: Vec<(f32, usize, &'static SettingMeta)> = meta::all()
        .iter()
        .enumerate()
        .filter(|(_, m)| m.applies_here())
        .filter_map(|(i, m)| {
            let fields = fields(m, loc, &en);
            let mut total = 0.0;
            for w in &words {
                let best = fields
                    .iter()
                    .map(|f| word_score(&f.text, w) * f.weight as f32)
                    .fold(0.0f32, f32::max);
                if best == 0.0 {
                    return None;
                }
                total += best;
            }
            // Whole-phrase bonus: the query as typed appears in the label.
            if words.len() > 1 && fields[0].text.contains(folded.trim()) {
                total += W_LABEL as f32;
            }
            Some((total, i, m))
        })
        .collect();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, _, m)| m).collect()
}

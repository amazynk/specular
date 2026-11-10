//! Whisper's tokens: the byte-level BPE vocabulary (`vocab.json`, GPT-2's byte-to-character
//! mapping) read for decoding only, and the special tokens, placed by the model's vocabulary
//! size (the 1,501 timestamp tokens end it; the controls sit just before them).

use crate::Error;
use crate::json;

/// Whisper's languages in token order (`<|en|>` is the first language token). The last,
/// Cantonese, only in large-v3 vocabularies (51,866 ids).
pub const LANGUAGES: [&str; 100] = [
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su", "yue",
];

/// Timestamp tokens: 0.00 to 30.00 s in 20 ms steps.
pub const TIMESTAMPS: u32 = 1_501;
/// Seconds per timestamp step.
pub const TIME_STEP: f32 = 0.02;

/// The special tokens of one vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Specials {
    pub eot: u32,
    pub sot: u32,
    /// The first language token and how many there are.
    pub lang0: u32,
    pub langs: u32,
    pub translate: u32,
    pub transcribe: u32,
    pub sot_lm: u32,
    pub sot_prev: u32,
    pub no_speech: u32,
    pub no_timestamps: u32,
    pub timestamp0: u32,
    pub vocab: u32,
}

impl Specials {
    /// The layout of a vocabulary of `vocab` ids, or `None` for a size Whisper never had.
    pub fn of(vocab: usize) -> Option<Specials> {
        let vocab = u32::try_from(vocab).ok()?;
        let timestamp0 = vocab.checked_sub(TIMESTAMPS)?;
        let multilingual = vocab >= 51_865;
        let eot = if multilingual { 50_257 } else { 50_256 };
        let translate = timestamp0.checked_sub(6)?;
        let lang0 = eot + 2;
        let langs = translate.checked_sub(lang0)?;
        if (multilingual && !(99..=100).contains(&langs)) || (!multilingual && langs != 99) {
            return None;
        }
        Some(Specials {
            eot,
            sot: eot + 1,
            lang0,
            langs,
            translate,
            transcribe: translate + 1,
            sot_lm: translate + 2,
            sot_prev: translate + 3,
            no_speech: translate + 4,
            no_timestamps: translate + 5,
            timestamp0,
            vocab,
        })
    }

    /// Whether the model reads languages other than English (and can translate).
    pub fn multilingual(&self) -> bool {
        self.eot == 50_257
    }

    /// The token of language `code` ("en", "he"), when the model has it.
    pub fn language(&self, code: &str) -> Option<u32> {
        if !self.multilingual() {
            return (code == "en").then_some(self.lang0);
        }
        let i = LANGUAGES[..self.langs as usize]
            .iter()
            .position(|l| *l == code)?;
        Some(self.lang0 + i as u32)
    }

    /// The code of language token `t`.
    pub fn language_code(&self, t: u32) -> Option<&'static str> {
        let i = t.checked_sub(self.lang0)?;
        (i < self.langs).then(|| LANGUAGES[i as usize])
    }

    pub fn is_timestamp(&self, t: u32) -> bool {
        t >= self.timestamp0
    }

    /// Seconds of timestamp token `t`.
    pub fn time(&self, t: u32) -> f32 {
        t.saturating_sub(self.timestamp0) as f32 * TIME_STEP
    }
}

/// GPT-2's byte-to-character table: printable Latin-1 bytes are themselves, the rest are
/// moved to U+0100 and up.
fn byte_chars() -> [char; 256] {
    let mut out = ['\0'; 256];
    let mut n = 0u32;
    for b in 0..256u32 {
        let printable = (0x21..=0x7e).contains(&b) || (0xa1..=0xac).contains(&b) || b >= 0xae;
        out[b as usize] = if printable {
            char::from_u32(b).unwrap_or('\0')
        } else {
            n += 1;
            char::from_u32(255 + n).unwrap_or('\0')
        };
    }
    out
}

/// The vocabulary, for turning ids back into text.
pub struct Vocab {
    /// Each id's bytes (empty for ids the file does not list).
    bytes: Vec<Vec<u8>>,
}

impl Vocab {
    /// Reads `vocab.json` (`{"token": id, …}`).
    pub fn parse(text: &[u8]) -> Result<Vocab, Error> {
        let j = json::parse(text).map_err(|e| Error::Weights(format!("vocab.json: {e}")))?;
        let obj = j
            .as_object()
            .ok_or_else(|| Error::Weights("vocab.json is not an object".into()))?;
        let table = byte_chars();
        let mut rev = [0u16; 0x200];
        for (b, c) in table.iter().enumerate() {
            rev[*c as usize] = b as u16;
        }
        let max = obj
            .iter()
            .filter_map(|(_, v)| v.as_u64())
            .max()
            .unwrap_or(0) as usize;
        if max > 1 << 20 {
            return Err(Error::Weights("vocab.json ids out of range".into()));
        }
        let mut bytes = vec![Vec::new(); max + 1];
        for (tok, id) in obj {
            let Some(id) = id.as_u64() else { continue };
            let mut b = Vec::with_capacity(tok.len());
            for c in tok.chars() {
                match rev.get(c as usize) {
                    Some(&v) if table[v as usize] == c => b.push(v as u8),
                    _ => b.extend(c.to_string().bytes()),
                }
            }
            bytes[id as usize] = b;
        }
        Ok(Vocab { bytes })
    }

    /// Ids the file lists (the ordinary tokens; specials come after).
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The bytes of token `t` (empty for a special one).
    pub fn bytes(&self, t: u32) -> &[u8] {
        self.bytes.get(t as usize).map_or(&[], Vec::as_slice)
    }

    /// The text of ordinary tokens `ts` (specials skipped), invalid UTF-8 replaced.
    pub fn decode(&self, ts: &[u32]) -> String {
        let mut b = Vec::new();
        for &t in ts {
            b.extend_from_slice(self.bytes(t));
        }
        String::from_utf8_lossy(&b).into_owned()
    }

    /// The id of the token spelled exactly `s` (its bytes), if any.
    pub fn find(&self, s: &str) -> Option<u32> {
        self.bytes
            .iter()
            .position(|b| b.as_slice() == s.as_bytes())
            .map(|i| i as u32)
    }

    /// Tokens Whisper never emits while transcribing: symbols that are not speech (as its
    /// `non_speech_tokens`: brackets, maths and music signs), alone or after a space.
    pub fn non_speech(&self) -> Vec<u32> {
        const SYMBOLS: &str = "\" # ( ) * + / : ; < = > @ [ \\ ] ^ _ ` { | } ~ 「 」 『 』 << >> <<< >>> -- --- -( -[ (' (\" (( )) ((( ))) [[ ]] {{ }} ♪♪ ♪♪♪ ♩ ♪ ♫ ♬ ♭ ♮ ♯";
        let mut out: Vec<u32> = Vec::new();
        for s in SYMBOLS.split(' ') {
            for v in [s.to_owned(), format!(" {s}")] {
                if let Some(t) = self.find(&v) {
                    out.push(t);
                }
            }
        }
        for s in [" -", " '"] {
            if let Some(t) = self.find(s) {
                out.push(t);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specials_by_vocabulary() {
        let m = Specials::of(51_865).unwrap();
        assert_eq!(
            (m.eot, m.sot, m.lang0, m.translate, m.transcribe),
            (50_257, 50_258, 50_259, 50_358, 50_359)
        );
        assert_eq!(
            (m.no_speech, m.no_timestamps, m.timestamp0),
            (50_362, 50_363, 50_364)
        );
        assert_eq!(m.language("en"), Some(50_259));
        assert_eq!(m.language("he"), Some(50_259 + 20));
        assert_eq!(m.language("yue"), None, "99 languages before large-v3");
        assert_eq!(m.language_code(50_260), Some("zh"));
        let v3 = Specials::of(51_866).unwrap();
        assert_eq!((v3.translate, v3.timestamp0), (50_359, 50_365));
        assert_eq!(v3.language("yue"), Some(50_358));
        let en = Specials::of(51_864).unwrap();
        assert!(!en.multilingual());
        assert_eq!(
            (en.eot, en.sot, en.transcribe, en.timestamp0),
            (50_256, 50_257, 50_358, 50_363)
        );
        assert_eq!(en.language("de"), None);
        assert!(Specials::of(1000).is_none());
        assert!((m.time(m.timestamp0 + 150) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn byte_table_is_gpt2s() {
        let t = byte_chars();
        assert_eq!(t[b'a' as usize], 'a');
        assert_eq!(t[b' ' as usize], 'Ġ');
        assert_eq!(t[b'\n' as usize], 'Ċ');
        assert_eq!(t[0], 'Ā');
        let mut seen: Vec<char> = t.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 256);
    }

    #[test]
    fn decodes_byte_level_tokens() {
        // "שלום" is d7 a9 d7 9c d7 95 d7 9d: split across tokens mid-character.
        let v = Vocab::parse(
            "{\"Hello\": 0, \"Ġworld\": 1, \"!\": 2, \"Ġ×\": 3, \"©×ľ\": 4, \"×ķ×Ŀ\": 5}"
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(v.decode(&[0, 1, 2]), "Hello world!");
        assert_eq!(v.decode(&[3, 4, 5]), " שלום");
        assert_eq!(v.decode(&[3]), " \u{fffd}", "half a character");
        assert_eq!(v.decode(&[0, 50_000]), "Hello", "specials are skipped");
        assert_eq!(v.find("Ġworld"), None);
        assert_eq!(v.find(" world"), Some(1));
    }

    #[test]
    fn non_speech_symbols() {
        let v = Vocab::parse(
            "{\"a\": 0, \"(\": 1, \"Ġ(\": 2, \"Ġ-\": 3, \"ok\": 4, \"[[\": 5}".as_bytes(),
        )
        .unwrap();
        assert_eq!(v.non_speech(), vec![1, 2, 3, 5]);
    }
}

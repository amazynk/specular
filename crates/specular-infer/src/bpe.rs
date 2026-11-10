//! CLIP's byte-level BPE tokenizer, built from the merges file alone (the model's
//! `merges.txt`, a pack download like the weights).
//!
//! The vocabulary follows from the merges: the 256 byte symbols, the same with the
//! end-of-word mark `</w>`, one symbol per merge in rank order, then `<|startoftext|>` and
//! `<|endoftext|>` (49,408 ids for CLIP's 48,894 merges). Text is cleaned (whitespace runs
//! to one space, trimmed, lower-cased), split into words by CLIP's pattern (contractions,
//! letter runs, single digits, punctuation runs), each word's UTF-8 bytes mapped to
//! printable symbols and merged pair by pair, lowest rank first.
//!
//! Not done (the reference does them through ftfy and html.unescape): Unicode NFC
//! normalisation, mojibake repair and HTML entities. Typed queries rarely need them; a
//! decomposed accent (e + U+0301) splits differently from the composed one.

use std::collections::HashMap;

use crate::Error;

/// Merges CLIP's vocabulary uses (the merges file may hold more; the rest are ignored).
pub const CLIP_MERGES: usize = 49_152 - 256 - 2;

/// The text-to-ids half of a CLIP tokenizer.
#[derive(Clone, Debug)]
pub struct Bpe {
    /// (left id, right id) → (rank, merged id).
    merges: HashMap<(u32, u32), (u32, u32)>,
    /// Symbol id of each byte (index), without and with `</w>` (+ 256).
    byte_id: [u32; 256],
    pub sot: u32,
    pub eot: u32,
    words: regex::Regex,
}

/// GPT-2's map of bytes to printable characters: printable Latin-1 bytes stand for
/// themselves, the others for 256, 257, … in byte order.
fn byte_chars() -> [char; 256] {
    let mut out = ['\0'; 256];
    let mut extra = 0u32;
    for b in 0..=255u32 {
        let printable = (0x21..=0x7e).contains(&b) || (0xa1..=0xac).contains(&b) || b >= 0xae;
        out[b as usize] = if printable {
            char::from_u32(b).unwrap_or('\0')
        } else {
            extra += 1;
            char::from_u32(255 + extra).unwrap_or('\0')
        };
    }
    out
}

/// The byte order of the vocabulary's first 256 symbols: printable bytes first, then the rest.
fn byte_order() -> Vec<u8> {
    let printable = |b: u8| (0x21..=0x7e).contains(&b) || (0xa1..=0xac).contains(&b) || b >= 0xae;
    let mut v: Vec<u8> = (0..=255u8).filter(|&b| printable(b)).collect();
    v.extend((0..=255u8).filter(|&b| !printable(b)));
    v
}

impl Bpe {
    /// From the text of a merges file: an optional `#version` line, then `left right` per
    /// line in rank order. Uses the first `max_merges`.
    pub fn from_merges(text: &str, max_merges: usize) -> Result<Bpe, Error> {
        let bad = |s: String| Error::Weights(format!("merges: {s}"));
        let chars = byte_chars();
        let order = byte_order();
        // Vocabulary strings → ids.
        let mut vocab: HashMap<String, u32> = HashMap::with_capacity(max_merges + 514);
        let mut byte_id = [0u32; 256];
        for (i, &b) in order.iter().enumerate() {
            let c = chars[b as usize];
            vocab.insert(c.to_string(), i as u32);
            vocab.insert(format!("{c}</w>"), i as u32 + 256);
            byte_id[b as usize] = i as u32;
        }
        let mut merges = HashMap::with_capacity(max_merges);
        let mut next = 512u32;
        let lines = text
            .lines()
            .filter(|l| !l.starts_with("#version"))
            .take(max_merges);
        for (rank, line) in lines.enumerate() {
            let (a, b) = line
                .split_once(' ')
                .ok_or_else(|| bad(format!("line {} is not two symbols", rank + 1)))?;
            let (Some(&ia), Some(&ib)) = (vocab.get(a), vocab.get(b)) else {
                return Err(bad(format!("line {}: unknown symbol", rank + 1)));
            };
            let joined = format!("{a}{b}");
            if vocab.contains_key(&joined) {
                return Err(bad(format!("line {}: {joined:?} twice", rank + 1)));
            }
            vocab.insert(joined, next);
            merges.insert((ia, ib), (rank as u32, next));
            next += 1;
        }
        if merges.is_empty() {
            return Err(bad("no merges".into()));
        }
        // `(?i)` as the reference: after lower-casing it only matters for the specials.
        let words = regex::Regex::new(
            r"(?i)<\|startoftext\|>|<\|endoftext\|>|'s|'t|'re|'ve|'m|'ll|'d|\p{L}+|\p{N}|[^\s\p{L}\p{N}]+",
        )
        .map_err(|e| bad(e.to_string()))?;
        Ok(Bpe {
            merges,
            byte_id,
            sot: next,
            eot: next + 1,
            words,
        })
    }

    /// Ids the vocabulary has: the model's token table must have this many rows.
    pub fn vocab_size(&self) -> usize {
        self.eot as usize + 1
    }

    /// The ids of `text`, without the start and end marks.
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let clean = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let mut out = Vec::new();
        for m in self.words.find_iter(&clean) {
            match m.as_str() {
                "<|startoftext|>" => out.push(self.sot),
                "<|endoftext|>" => out.push(self.eot),
                w => self.word(w.as_bytes(), &mut out),
            }
        }
        out
    }

    fn word(&self, bytes: &[u8], out: &mut Vec<u32>) {
        let mut syms: Vec<u32> = bytes.iter().map(|&b| self.byte_id[b as usize]).collect();
        let Some(last) = syms.last_mut() else {
            return;
        };
        *last += 256;
        loop {
            // The lowest-ranked pair present, merged everywhere it occurs.
            let best = syms
                .windows(2)
                .filter_map(|p| {
                    self.merges
                        .get(&(p[0], p[1]))
                        .map(|&(r, id)| (r, p[0], p[1], id))
                })
                .min();
            let Some((_, a, b, id)) = best else {
                break;
            };
            let mut merged = Vec::with_capacity(syms.len());
            let mut i = 0;
            while i < syms.len() {
                if i + 1 < syms.len() && syms[i] == a && syms[i + 1] == b {
                    merged.push(id);
                    i += 2;
                } else {
                    merged.push(syms[i]);
                    i += 1;
                }
            }
            syms = merged;
        }
        out.extend(syms);
    }

    /// `<|startoftext|>`, the ids of `text`, `<|endoftext|>`, cut to `context` (keeping the
    /// end mark last) and padded with 0 to `context`; and the end mark's position.
    pub fn tokenize(&self, text: &str, context: usize) -> (Vec<u32>, usize) {
        let mut ids = Vec::with_capacity(context);
        ids.push(self.sot);
        ids.extend(self.encode(text));
        ids.push(self.eot);
        if ids.len() > context {
            ids.truncate(context);
            if let Some(l) = ids.last_mut() {
                *l = self.eot;
            }
        }
        let end = ids.len().saturating_sub(1);
        ids.resize(context, 0);
        (ids, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A few merges over the byte alphabet: enough to check ranks, `</w>` and the vocabulary.
    const MERGES: &str = "#version: 0.2\nh e\nl l\nhe ll\nhell o</w>\nc a\nca t</w>\n";

    fn bpe() -> Bpe {
        Bpe::from_merges(MERGES, 100).unwrap()
    }

    #[test]
    fn byte_alphabet() {
        let c = byte_chars();
        assert_eq!(c[b'a' as usize], 'a');
        assert_eq!(c[b' ' as usize], '\u{120}');
        assert_eq!(c[0], '\u{100}');
        assert_eq!(c[0xad], '\u{143}');
        let o = byte_order();
        assert_eq!(o[0], b'!');
        assert_eq!(o.len(), 256);
    }

    #[test]
    fn merges_by_rank() {
        let b = bpe();
        // 512 byte symbols, 6 merges, then the marks.
        assert_eq!(b.sot, 518);
        assert_eq!(b.vocab_size(), 520);
        // "hello" → he, ll → hell → hello</w>: one symbol, the 4th merge.
        assert_eq!(b.encode("hello"), vec![512 + 3]);
        // "cat" → ca, cat</w>.
        assert_eq!(b.encode("  CAT\t"), vec![512 + 5]);
        // Unmerged letters: byte ids, the last with </w>.
        let id = |c: u8| byte_order().iter().position(|&x| x == c).unwrap() as u32;
        assert_eq!(b.encode("ox"), vec![id(b'o'), id(b'x') + 256]);
        // Words split: letters, single digits, punctuation, contractions.
        assert_eq!(
            b.encode("a1!! it's"),
            vec![
                id(b'a') + 256,
                id(b'1') + 256,
                id(b'!'),
                id(b'!') + 256,
                id(b'i'),
                id(b't') + 256,
                id(b'\''),
                id(b's') + 256
            ]
        );
        assert_eq!(b.encode("<|endoftext|>"), vec![b.eot]);
        // Non-ASCII: UTF-8 bytes.
        assert_eq!(b.encode("é").len(), 2);
    }

    #[test]
    fn tokenize_marks_cuts_and_pads() {
        let b = bpe();
        let (ids, end) = b.tokenize("hello cat", 8);
        assert_eq!(ids, vec![b.sot, 515, 517, b.eot, 0, 0, 0, 0]);
        assert_eq!(end, 3);
        let (ids, end) = b.tokenize(&"ox ".repeat(10), 5);
        assert_eq!(ids.len(), 5);
        assert_eq!((ids[0], ids[4], end), (b.sot, b.eot, 4));
    }

    #[test]
    fn refuses_bad_merges() {
        Bpe::from_merges("#version: 0.2\n", 10).unwrap_err();
        Bpe::from_merges("ab\n", 10).unwrap_err();
        Bpe::from_merges("h e\nh e\n", 10).unwrap_err();
        Bpe::from_merges("o</w> \n", 10).unwrap_err();
        // Only the first merges count.
        assert_eq!(Bpe::from_merges(MERGES, 2).unwrap().vocab_size(), 516);
    }
}

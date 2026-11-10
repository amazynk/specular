//! A SentencePiece tokenizer read from the model's own `tokenizer.model` (a protobuf
//! `ModelProto`), for SigLIP 2's text tower (the Gemma vocabulary: 256,000 pieces, BPE with
//! byte fallback). Both SentencePiece model types are encoded:
//!
//! - **BPE**: the text starts as one symbol per character (a user-defined piece, such as
//!   `<b>` or a run of newlines, is one frozen symbol); the adjacent pair whose concatenation
//!   is a piece with the highest score is merged, the leftmost on a tie, until no pair is a
//!   piece.
//! - **Unigram**: the segmentation with the highest total score (Viterbi), an unknown
//!   character costing the lowest piece score minus 10.
//!
//! A symbol that is no piece becomes its UTF-8 bytes (`<0xE2>` …) when the model has byte
//! fallback, else the unknown id.
//!
//! Normalisation: only the identity normaliser (the model's `precompiled_charsmap` empty) is
//! supported, with its flags: drop extra whitespace, a leading `▁`, and spaces written as
//! `▁` (U+2581). A model that needs a character map (NFKC-style) is refused rather than
//! tokenized differently from its training.
//!
//! Written from the format's published field numbers and the algorithm's description; no
//! code from the reference implementation.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::Error;

/// What kind of entry a piece is (`ModelProto.SentencePiece.Type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Normal,
    Unknown,
    Control,
    UserDefined,
    Unused,
    Byte,
}

impl PieceKind {
    fn from_proto(n: u64) -> Option<PieceKind> {
        Some(match n {
            1 => PieceKind::Normal,
            2 => PieceKind::Unknown,
            3 => PieceKind::Control,
            4 => PieceKind::UserDefined,
            5 => PieceKind::Unused,
            6 => PieceKind::Byte,
            _ => return None,
        })
    }
}

/// The model type (`TrainerSpec.model_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelType {
    Unigram,
    Bpe,
}

/// A loaded SentencePiece model.
#[derive(Clone, Debug)]
pub struct SentencePiece {
    pub model: ModelType,
    /// Piece strings, by id.
    pieces: Vec<Box<str>>,
    scores: Vec<f32>,
    kinds: Vec<PieceKind>,
    /// Normal, user-defined and unused pieces: what text can be segmented into.
    lookup: HashMap<Box<str>, u32>,
    /// User-defined pieces, longest first (matched before anything else).
    user: Vec<(Box<str>, u32)>,
    /// `<0xXX>` ids, when the model has byte fallback.
    bytes: Option<[u32; 256]>,
    pub unk: u32,
    pub bos: Option<u32>,
    pub eos: Option<u32>,
    pub pad: Option<u32>,
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
    escape_whitespaces: bool,
    /// Longest piece, in characters (bounds the unigram search).
    max_chars: usize,
    min_score: f32,
}

/// Most pieces a model file may have (Gemma has 256,000).
const MAX_PIECES: usize = 1 << 20;

/// The whitespace mark.
pub const SPACE: char = '\u{2581}';

/// A bounded protobuf reader: varints and length-delimited fields only (plus skipping the
/// fixed-width ones).
struct Proto<'a> {
    b: &'a [u8],
    at: usize,
}

enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
    Fixed64,
}

impl<'a> Proto<'a> {
    fn new(b: &'a [u8]) -> Proto<'a> {
        Proto { b, at: 0 }
    }

    fn varint(&mut self) -> Result<u64, Error> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let Some(&byte) = self.b.get(self.at) else {
                return bad("truncated varint");
            };
            self.at += 1;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        bad("varint too long")
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len());
        let Some(end) = end else {
            return bad("truncated field");
        };
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    /// The next field number and value; None at the end.
    fn next(&mut self) -> Result<Option<(u64, Field<'a>)>, Error> {
        if self.at >= self.b.len() {
            return Ok(None);
        }
        let key = self.varint()?;
        let f = match key & 7 {
            0 => Field::Varint(self.varint()?),
            1 => {
                self.take(8)?;
                Field::Fixed64
            }
            2 => {
                let n = usize::try_from(self.varint()?).unwrap_or(usize::MAX);
                Field::Bytes(self.take(n)?)
            }
            5 => {
                let s = self.take(4)?;
                Field::Fixed32(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
            }
            w => return bad(format!("wire type {w}")),
        };
        Ok(Some((key >> 3, f)))
    }
}

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(format!("tokenizer.model: {}", s.into())))
}

fn utf8(b: &[u8]) -> Result<&str, Error> {
    std::str::from_utf8(b).or_else(|_| bad("a piece is not UTF-8"))
}

/// One element of the BPE agenda: merge `left` and `right` into a piece of `score`.
#[derive(Debug)]
struct Pair {
    score: f32,
    left: usize,
    right: usize,
    /// Bytes of the merged piece (a stale entry no longer matches).
    len: usize,
}

impl PartialEq for Pair {
    fn eq(&self, o: &Pair) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Pair {}
impl PartialOrd for Pair {
    fn partial_cmp(&self, o: &Pair) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Pair {
    /// Highest score first, then leftmost.
    fn cmp(&self, o: &Pair) -> Ordering {
        self.score
            .total_cmp(&o.score)
            .then_with(|| o.left.cmp(&self.left))
    }
}

impl SentencePiece {
    /// Reads a serialised `ModelProto`.
    pub fn from_bytes(data: &[u8]) -> Result<SentencePiece, Error> {
        let mut pieces: Vec<Box<str>> = Vec::new();
        let mut scores = Vec::new();
        let mut kinds = Vec::new();
        let mut model = ModelType::Unigram;
        let (mut byte_fallback, mut unk, mut bos, mut eos, mut pad) = (false, 0i64, 1, 2, -1);
        let (mut dummy, mut extra, mut escape, mut charsmap) = (true, true, true, false);
        let mut p = Proto::new(data);
        while let Some((n, f)) = p.next()? {
            match (n, f) {
                (1, Field::Bytes(b)) => {
                    if pieces.len() >= MAX_PIECES {
                        return bad("too many pieces");
                    }
                    let (mut piece, mut score, mut kind) = ("", 0f32, PieceKind::Normal);
                    let mut q = Proto::new(b);
                    while let Some((n, f)) = q.next()? {
                        match (n, f) {
                            (1, Field::Bytes(s)) => piece = utf8(s)?,
                            (2, Field::Fixed32(s)) => score = f32::from_bits(s),
                            (3, Field::Varint(t)) => {
                                kind = PieceKind::from_proto(t)
                                    .map_or_else(|| bad(format!("piece type {t}")), Ok)?
                            }
                            _ => {}
                        }
                    }
                    pieces.push(piece.into());
                    scores.push(score);
                    kinds.push(kind);
                }
                (2, Field::Bytes(b)) => {
                    let mut q = Proto::new(b);
                    while let Some((n, f)) = q.next()? {
                        let Field::Varint(v) = f else { continue };
                        // Ids are int32: two's complement in a 64-bit varint.
                        let id = v as i64 as i32 as i64;
                        match n {
                            3 => {
                                model = match v {
                                    1 => ModelType::Unigram,
                                    2 => ModelType::Bpe,
                                    t => {
                                        return bad(format!(
                                            "model type {t} (only unigram and BPE are supported)"
                                        ));
                                    }
                                }
                            }
                            35 => byte_fallback = v != 0,
                            40 => unk = id,
                            41 => bos = id,
                            42 => eos = id,
                            43 => pad = id,
                            24 if v != 0 => return bad("whitespace as suffix is not supported"),
                            _ => {}
                        }
                    }
                }
                (3, Field::Bytes(b)) => {
                    let mut q = Proto::new(b);
                    while let Some((n, f)) = q.next()? {
                        match (n, f) {
                            (2, Field::Bytes(m)) => charsmap = !m.is_empty(),
                            (3, Field::Varint(v)) => dummy = v != 0,
                            (4, Field::Varint(v)) => extra = v != 0,
                            (5, Field::Varint(v)) => escape = v != 0,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        if pieces.is_empty() {
            return bad("no pieces");
        }
        if charsmap {
            return bad("a normalisation map is needed, which is not supported");
        }
        let n = pieces.len();
        let id = |i: i64| -> Result<Option<u32>, Error> {
            match i {
                i if i < 0 => Ok(None),
                i if (i as usize) < n => Ok(Some(i as u32)),
                i => bad(format!("special id {i} past the {n} pieces")),
            }
        };
        let unk = id(unk)?.map_or_else(|| bad("no unknown id"), Ok)?;
        let mut lookup = HashMap::with_capacity(n);
        let mut user = Vec::new();
        let mut byte_ids = [u32::MAX; 256];
        let mut max_chars = 1;
        let mut min_score = f32::MAX;
        for (i, ((p, &k), &s)) in pieces.iter().zip(&kinds).zip(&scores).enumerate() {
            match k {
                PieceKind::Normal | PieceKind::UserDefined | PieceKind::Unused => {
                    if !p.is_empty() {
                        lookup.entry(p.clone()).or_insert(i as u32);
                        max_chars = max_chars.max(p.chars().count());
                    }
                    if k == PieceKind::UserDefined && !p.is_empty() {
                        user.push((p.clone(), i as u32));
                    }
                    if k == PieceKind::Normal {
                        min_score = min_score.min(s);
                    }
                }
                PieceKind::Byte => {
                    let hex = p.strip_prefix("<0x").and_then(|h| h.strip_suffix('>'));
                    match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                        Some(b) => byte_ids[b as usize] = i as u32,
                        None => return bad(format!("byte piece {p:?}")),
                    }
                }
                _ => {}
            }
        }
        user.sort_by_key(|u| std::cmp::Reverse(u.0.len()));
        let bytes = if byte_fallback {
            if byte_ids.contains(&u32::MAX) {
                return bad("byte fallback without all 256 byte pieces");
            }
            Some(byte_ids)
        } else {
            None
        };
        Ok(SentencePiece {
            model,
            pieces,
            scores,
            kinds,
            lookup,
            user,
            bytes,
            unk,
            bos: id(bos)?,
            eos: id(eos)?,
            pad: id(pad)?,
            add_dummy_prefix: dummy,
            remove_extra_whitespaces: extra,
            escape_whitespaces: escape,
            max_chars,
            min_score: if min_score == f32::MAX {
                0.0
            } else {
                min_score
            },
        })
    }

    /// Reads `tokenizer.model`.
    pub fn open(path: &std::path::Path) -> Result<SentencePiece, Error> {
        let data =
            std::fs::read(path).map_err(|e| Error::Weights(format!("{}: {e}", path.display())))?;
        SentencePiece::from_bytes(&data)
    }

    /// Number of ids.
    pub fn vocab_size(&self) -> usize {
        self.pieces.len()
    }

    /// The piece of `id`.
    pub fn piece(&self, id: u32) -> Option<&str> {
        self.pieces.get(id as usize).map(|p| &**p)
    }

    pub fn kind(&self, id: u32) -> Option<PieceKind> {
        self.kinds.get(id as usize).copied()
    }

    pub fn score(&self, id: u32) -> Option<f32> {
        self.scores.get(id as usize).copied()
    }

    /// The text as the model sees it: whitespace flags applied, spaces as `▁`.
    pub fn normalize(&self, text: &str) -> String {
        let mut s = String::with_capacity(text.len() + 3);
        let text = if self.remove_extra_whitespaces {
            text.trim_matches(' ')
        } else {
            text
        };
        if self.add_dummy_prefix && !text.is_empty() {
            s.push(if self.escape_whitespaces { SPACE } else { ' ' });
        }
        let mut last_space = false;
        for c in text.chars() {
            if c == ' ' {
                if self.remove_extra_whitespaces && last_space {
                    continue;
                }
                last_space = true;
                s.push(if self.escape_whitespaces { SPACE } else { ' ' });
            } else {
                last_space = false;
                s.push(c);
            }
        }
        s
    }

    /// The user-defined piece at the start of `s`, longest first.
    fn user_at(&self, s: &str) -> Option<(usize, u32)> {
        self.user
            .iter()
            .find(|(p, _)| s.starts_with(&**p))
            .map(|(p, id)| (p.len(), *id))
    }

    /// Ids of `text` (no BOS or EOS).
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let norm = self.normalize(text);
        let mut out = Vec::new();
        match self.model {
            ModelType::Bpe => self.bpe(&norm, &mut out),
            ModelType::Unigram => self.unigram(&norm, &mut out),
        }
        out
    }

    /// Emits the piece `s`: its id, else its bytes (byte fallback), else unknown.
    fn emit(&self, s: &str, out: &mut Vec<u32>) {
        if let Some(&id) = self.lookup.get(s) {
            out.push(id);
        } else if let Some(bytes) = &self.bytes {
            out.extend(s.bytes().map(|b| bytes[b as usize]));
        } else {
            out.push(self.unk);
        }
    }

    fn bpe(&self, s: &str, out: &mut Vec<u32>) {
        // Symbols: byte ranges of `s`, linked as a list; `frozen` ones never merge.
        struct Sym {
            start: usize,
            len: usize,
            prev: Option<usize>,
            next: Option<usize>,
            frozen: bool,
        }
        let mut syms: Vec<Sym> = Vec::new();
        let mut at = 0;
        while at < s.len() {
            let (len, frozen) = match self.user_at(&s[at..]) {
                Some((len, _)) => (len, true),
                None => (s[at..].chars().next().map_or(1, char::len_utf8), false),
            };
            let i = syms.len();
            syms.push(Sym {
                start: at,
                len,
                prev: i.checked_sub(1),
                next: None,
                frozen,
            });
            if i > 0 {
                syms[i - 1].next = Some(i);
            }
            at += len;
        }
        let mut agenda = BinaryHeap::new();
        let pair = |syms: &[Sym], l: usize, r: usize| -> Option<Pair> {
            let (a, b) = (&syms[l], &syms[r]);
            if a.frozen || b.frozen {
                return None;
            }
            let piece = &s[a.start..b.start + b.len];
            let &id = self.lookup.get(piece)?;
            Some(Pair {
                score: self.scores[id as usize],
                left: l,
                right: r,
                len: piece.len(),
            })
        };
        for i in 1..syms.len() {
            agenda.extend(pair(&syms, i - 1, i));
        }
        while let Some(top) = agenda.pop() {
            let (l, r) = (top.left, top.right);
            if syms[l].len == 0 || syms[r].len == 0 || syms[l].len + syms[r].len != top.len {
                continue;
            }
            if syms[l].next != Some(r) {
                continue;
            }
            syms[l].len += syms[r].len;
            syms[r].len = 0;
            let next = syms[r].next;
            syms[l].next = next;
            if let Some(n) = next {
                syms[n].prev = Some(l);
            }
            if let Some(p) = syms[l].prev {
                agenda.extend(pair(&syms, p, l));
            }
            if let Some(n) = next {
                agenda.extend(pair(&syms, l, n));
            }
        }
        let mut i = if syms.is_empty() { None } else { Some(0) };
        while let Some(k) = i {
            let sym = &syms[k];
            self.emit(&s[sym.start..sym.start + sym.len], out);
            i = sym.next;
        }
    }

    fn unigram(&self, s: &str, out: &mut Vec<u32>) {
        // Character boundaries.
        let mut bounds: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
        bounds.push(s.len());
        let n = bounds.len() - 1;
        let unk_score = self.min_score - 10.0;
        // best[i]: (score, start char index, id or None for an unknown char) ending at i.
        let mut best: Vec<(f32, usize, Option<u32>)> = vec![(f32::NEG_INFINITY, 0, None); n + 1];
        best[0].0 = 0.0;
        for i in 0..n {
            let base = best[i].0;
            if base == f32::NEG_INFINITY {
                continue;
            }
            if let Some((len, id)) = self.user_at(&s[bounds[i]..]) {
                // A user-defined piece is taken whole and alone.
                let j = bounds.partition_point(|&b| b < bounds[i] + len);
                if base > best[j].0 {
                    best[j] = (base, i, Some(id));
                }
                continue;
            }
            let mut matched = false;
            for j in i + 1..=(i + self.max_chars).min(n) {
                if let Some(&id) = self.lookup.get(&s[bounds[i]..bounds[j]]) {
                    if self.kinds[id as usize] == PieceKind::Unused {
                        continue;
                    }
                    let sc = base
                        + if self.kinds[id as usize] == PieceKind::UserDefined {
                            0.0
                        } else {
                            self.scores[id as usize]
                        };
                    if j == i + 1 {
                        matched = true;
                    }
                    if sc > best[j].0 {
                        best[j] = (sc, i, Some(id));
                    }
                }
            }
            if !matched && base + unk_score > best[i + 1].0 {
                best[i + 1] = (base + unk_score, i, None);
            }
        }
        let mut path = Vec::new();
        let mut j = n;
        while j > 0 {
            let (_, i, id) = best[j];
            path.push((i, j, id));
            j = i;
        }
        for &(i, j, id) in path.iter().rev() {
            match id {
                Some(id) => out.push(id),
                None => self.emit(&s[bounds[i]..bounds[j]], out),
            }
        }
    }
}

/// SigLIP's recipe for a text tower's tokens: the text lower-cased (the models were trained
/// on lower-cased text), EOS appended, truncated to `context` keeping the EOS, padded with
/// the pad id. Returns the ids and the index of the token the tower pools (the last).
pub fn siglip_tokens(sp: &SentencePiece, text: &str, context: usize) -> (Vec<u32>, usize) {
    let context = context.max(1);
    let lower = text.to_lowercase();
    let mut ids = sp.encode(&lower);
    let room = context - usize::from(sp.eos.is_some());
    ids.truncate(room);
    ids.extend(sp.eos);
    ids.resize(context, sp.pad.unwrap_or(0));
    (ids, context - 1)
}

/// A serialised `ModelProto` of `pieces` (piece, score, proto type number), with Gemma's
/// special ids (pad 0, EOS 1, BOS 2, unknown 3) and the identity normaliser without extra
/// whitespace handling: for tests and benchmarks (`model_type` 1 unigram, 2 BPE).
#[doc(hidden)]
pub fn serialize(pieces: &[(&str, f32, u64)], model_type: u64, byte_fallback: bool) -> Vec<u8> {
    fn varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }
    fn bytes(n: u64, b: &[u8], out: &mut Vec<u8>) {
        varint(n << 3 | 2, out);
        varint(b.len() as u64, out);
        out.extend_from_slice(b);
    }
    let mut out = Vec::new();
    for (p, s, t) in pieces {
        let mut m = Vec::new();
        bytes(1, p.as_bytes(), &mut m);
        varint(2 << 3 | 5, &mut m);
        m.extend_from_slice(&s.to_le_bytes());
        varint(3 << 3, &mut m);
        varint(*t, &mut m);
        bytes(1, &m, &mut out);
    }
    let mut t = Vec::new();
    for (n, v) in [
        (3, model_type),
        (35, byte_fallback as u64),
        (40, 3),
        (41, 2),
        (42, 1),
        (43, 0),
    ] {
        varint(n << 3, &mut t);
        varint(v, &mut t);
    }
    bytes(2, &t, &mut out);
    let mut n = Vec::new();
    bytes(1, b"identity", &mut n);
    for f in [3u64, 4] {
        varint(f << 3, &mut n);
        varint(0, &mut n);
    }
    bytes(3, &n, &mut out);
    out
}

/// A tiny Gemma-like BPE model of `vocab` ids (≥ 300): the specials, the 256 bytes, the
/// letters, a few merges and unused pieces; for tests.
#[doc(hidden)]
pub fn tiny(vocab: usize) -> Vec<u8> {
    let mut pieces: Vec<(String, f32, u64)> = vec![
        ("<pad>".into(), 0.0, 3),
        ("<eos>".into(), 0.0, 3),
        ("<bos>".into(), 0.0, 3),
        ("<unk>".into(), 0.0, 2),
    ];
    pieces.extend((0..=255u8).map(|b| (format!("<0x{b:02X}>"), 0.0, 6)));
    let normal = "abcdefghijklmnopqrstuvwxyz"
        .chars()
        .map(|c| c.to_string())
        .chain(["\u{2581}", "\u{2581}a", "at", "\u{2581}cat", "\u{2581}mat"].map(str::to_owned));
    for (i, p) in normal.enumerate() {
        pieces.push((p, -(i as f32), 1));
    }
    while pieces.len() < vocab.max(300) {
        pieces.push((format!("<unused{}>", pieces.len()), 0.0, 4));
    }
    let r: Vec<(&str, f32, u64)> = pieces
        .iter()
        .map(|(a, b, c)| (a.as_str(), *b, *c))
        .collect();
    serialize(&r, 2, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Special pieces, the 256 bytes and `normal` (piece, score), as a model of `model_type`.
    fn with_pieces(model_type: u64, normal: &[(&str, f32)]) -> SentencePiece {
        let mut p: Vec<(String, f32, u64)> = vec![
            ("<pad>".into(), 0.0, 3),
            ("<eos>".into(), 0.0, 3),
            ("<bos>".into(), 0.0, 3),
            ("<unk>".into(), 0.0, 2),
            ("<b>".into(), 0.0, 4),
        ];
        for b in 0..=255u8 {
            p.push((format!("<0x{b:02X}>"), 0.0, 6));
        }
        p.extend(normal.iter().map(|(s, sc)| ((*s).to_owned(), *sc, 1)));
        let r: Vec<(&str, f32, u64)> = p.iter().map(|(a, b, c)| (a.as_str(), *b, *c)).collect();
        SentencePiece::from_bytes(&serialize(&r, model_type, true)).unwrap()
    }

    fn gemma_like(model_type: u64) -> SentencePiece {
        let normal: Vec<(&str, f32)> = [
            "a", "b", "c", "d", "▁", "ab", "cd", "abc", "▁ab", "▁abcd", "bcd",
        ]
        .iter()
        .enumerate()
        .map(|(i, s)| (*s, -(i as f32)))
        .collect();
        with_pieces(model_type, &normal)
    }

    fn pieces(sp: &SentencePiece, ids: &[u32]) -> Vec<String> {
        ids.iter()
            .map(|&i| sp.piece(i).unwrap().to_owned())
            .collect()
    }

    #[test]
    fn bpe_merges_by_score_and_falls_back_to_bytes() {
        let sp = gemma_like(2);
        assert_eq!(sp.model, ModelType::Bpe);
        assert_eq!(
            (sp.unk, sp.bos, sp.eos, sp.pad),
            (3, Some(2), Some(1), Some(0))
        );
        // "ab" (-5) merges before "cd" (-6); then "abc" (-7) is a piece but "ab"+"cd" is not;
        // "▁ab" (-8) merges "▁" with "ab".
        assert_eq!(pieces(&sp, &sp.encode("abcd")), ["ab", "cd"]);
        assert_eq!(pieces(&sp, &sp.encode(" ab")), ["▁ab"]);
        // é is no piece: its two UTF-8 bytes.
        assert_eq!(pieces(&sp, &sp.encode("aé")), ["a", "<0xC3>", "<0xA9>"]);
        // A user-defined piece is one frozen symbol.
        assert_eq!(pieces(&sp, &sp.encode("a<b>b")), ["a", "<b>", "b"]);
    }

    #[test]
    fn unigram_takes_the_best_total_score() {
        let sp = with_pieces(
            1,
            &[
                ("a", -5.0),
                ("b", -5.0),
                ("c", -5.0),
                ("d", -5.0),
                ("ab", -3.0),
                ("cd", -3.0),
                ("abcd", -5.5),
            ],
        );
        assert_eq!(sp.model, ModelType::Unigram);
        // "abcd" (-5.5) beats "ab" + "cd" (-6); "ab" + "c" (-8) beats three letters (-15).
        assert_eq!(pieces(&sp, &sp.encode("abcd")), ["abcd"]);
        assert_eq!(pieces(&sp, &sp.encode("abc")), ["ab", "c"]);
        // An unknown character falls back to its bytes; a user-defined piece is taken whole.
        assert_eq!(pieces(&sp, &sp.encode("xa")), ["<0x78>", "a"]);
        assert_eq!(pieces(&sp, &sp.encode("a<b>")), ["a", "<b>"]);
    }

    #[test]
    fn siglip_recipe_pads_and_keeps_eos() {
        let sp = gemma_like(2);
        let (ids, end) = siglip_tokens(&sp, "AB", 4);
        assert_eq!(end, 3);
        assert_eq!(pieces(&sp, &ids), ["ab", "<eos>", "<pad>", "<pad>"]);
        let (ids, _) = siglip_tokens(&sp, "a b c d", 4);
        assert_eq!(pieces(&sp, &ids), ["a", "▁", "b", "<eos>"]);
    }

    #[test]
    fn refuses_bad_files() {
        for b in [&[0x0a, 0x05, 1][..], &[0xff; 11][..], &[0x08][..]] {
            assert!(SentencePiece::from_bytes(b).is_err(), "{b:?}");
        }
        assert!(SentencePiece::from_bytes(&[]).is_err());
    }
}

//! Decoding a window: the language, then text with timestamps, as Whisper's reference
//! decoder does it:
//!
//! - The prompt is start-of-transcript, the language and the task (transcribe, or translate
//!   to English); English-only models take start-of-transcript alone.
//! - Every step: non-speech symbols and the control tokens are never picked; the first step
//!   can't be a blank; **timestamp rules**: the first token is a timestamp of at most 1 s,
//!   timestamps come in pairs (end, start) except before the end, never go back, never pass
//!   the sound in the window, and win whenever their summed probability beats every text
//!   token's.
//! - Greedy at temperature 0; when the result loops (its [`compression_ratio`] passes 2.4)
//!   or is unlikely (mean log-probability under −1), again at 0.2, 0.4 … 1.0 by sampling
//!   (a fixed seed: the same sound gives the same text). A window whose no-speech
//!   probability passes 0.6 and whose text is unlikely is silence.
//! - Pairs of timestamps close segments; a window ending in a lone timestamp was read to its
//!   end, otherwise the next window starts at the last timestamp (the half-heard sentence is
//!   read again whole).

use super::mel::SAMPLE_RATE;
use super::tokens::TIME_STEP;
use super::{Audio, Whisper};
use crate::Error;
use crate::tensor::Rng;

/// What to do with the speech.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Task {
    /// Text in the language spoken.
    #[default]
    Transcribe,
    /// Text in English (multilingual models).
    Translate,
}

/// Decoding choices.
#[derive(Clone, Debug)]
pub struct Options {
    /// The language spoken ("en", "he"); `None`: detected per window.
    pub language: Option<String>,
    pub task: Task,
    /// Tokens per window at most.
    pub max_tokens: usize,
    pub temperatures: Vec<f32>,
    pub no_speech_threshold: f32,
    pub logprob_threshold: f32,
    pub compression_threshold: f32,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            language: None,
            task: Task::Transcribe,
            max_tokens: 224,
            temperatures: vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0],
            no_speech_threshold: 0.6,
            logprob_threshold: -1.0,
            compression_threshold: 2.4,
        }
    }
}

/// A stretch of text and when it was said (seconds).
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// One window read.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowText {
    /// Times from the window's start.
    pub segments: Vec<Segment>,
    /// The language read (detected or given).
    pub language: &'static str,
    /// Seconds of the window used: the next window starts here.
    pub consumed: f32,
}

/// One decoding pass.
#[derive(Debug, Default)]
struct Pass {
    tokens: Vec<u32>,
    avg_logprob: f32,
    no_speech: f32,
    compression: f32,
}

/// An estimate of how well `text` compresses with Deflate (bytes in over bytes out), as
/// Whisper's check uses zlib's: a greedy LZ77 parse in a 32 KiB window, literals at 0.85
/// bytes, matches at 2.5, 10 bytes of overhead (within about 10 % of zlib on sentences and
/// loops). Looping text ("the the the …") scores far above 2.4; ordinary sentences near 1.3.
pub fn compression_ratio(text: &str) -> f32 {
    let b = text.as_bytes();
    if b.is_empty() {
        return 0.0;
    }
    let mut head = vec![usize::MAX; 1 << 12];
    let hash = |i: usize| -> usize {
        let v = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        (v.wrapping_mul(2_654_435_761) >> 20) as usize
    };
    let (mut lits, mut matches) = (0usize, 0usize);
    let mut i = 0;
    while i < b.len() {
        let mut len = 0;
        if i + 4 <= b.len() {
            let h = hash(i);
            let j = head[h];
            if j != usize::MAX && i - j <= 32_768 {
                while i + len < b.len() && len < 258 && b[j + len] == b[i + len] {
                    len += 1;
                }
            }
            head[h] = i;
        }
        if len >= 4 {
            matches += 1;
            for k in i + 1..(i + len).min(b.len().saturating_sub(3)) {
                head[hash(k)] = k;
            }
            i += len;
        } else {
            lits += 1;
            i += 1;
        }
    }
    b.len() as f32 / (lits as f32 * 0.85 + matches as f32 * 2.5 + 10.0)
}

fn log_softmax(x: &[f32]) -> Vec<f32> {
    let top = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    if !top.is_finite() {
        return vec![f32::NEG_INFINITY; x.len()];
    }
    let sum: f32 = x.iter().map(|v| (v - top).exp()).sum();
    let l = top + sum.ln();
    x.iter().map(|v| v - l).collect()
}

fn logsumexp(x: &[f32]) -> f32 {
    let top = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    if !top.is_finite() {
        return f32::NEG_INFINITY;
    }
    top + x.iter().map(|v| (v - top).exp()).sum::<f32>().ln()
}

fn argmax(x: &[f32]) -> u32 {
    let mut best = 0;
    for (i, v) in x.iter().enumerate() {
        if *v > x[best] {
            best = i;
        }
    }
    best as u32
}

/// Applies Whisper's timestamp rules to `logits` for the next token after `sampled` (this
/// window's tokens so far). `last_ts`: the last timestamp token the sound reaches.
pub(crate) fn timestamp_rules(
    logits: &mut [f32],
    sampled: &[u32],
    ts0: u32,
    eot: u32,
    no_ts: u32,
    last_ts: u32,
) {
    let ts0u = ts0 as usize;
    let ninf = f32::NEG_INFINITY;
    logits[no_ts as usize] = ninf;
    let is_ts = |t: u32| t >= ts0;
    let last = sampled.last().is_some_and(|&t| is_ts(t));
    let penult = sampled.len() < 2 || is_ts(sampled[sampled.len() - 2]);
    if last {
        if penult {
            logits[ts0u..].fill(ninf);
        } else {
            logits[..eot as usize].fill(ninf);
        }
    }
    if let Some(&t) = sampled.iter().rev().find(|&&t| is_ts(t)) {
        let floor = if last && !penult { t } else { t + 1 };
        let floor = (floor as usize).min(logits.len());
        logits[ts0u..floor].fill(ninf);
    }
    if sampled.is_empty() {
        logits[..ts0u].fill(ninf);
        // At most 1 s in.
        let cap = ts0u + (1.0 / TIME_STEP) as usize + 1;
        if cap < logits.len() {
            logits[cap..].fill(ninf);
        }
    }
    // Not past the sound.
    let end = (last_ts as usize + 1).min(logits.len());
    logits[end..].fill(ninf);
    // A timestamp when they are likelier together than any text token.
    let lp = log_softmax(logits);
    let ts = logsumexp(&lp[ts0u..]);
    let text = lp[..ts0u].iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    if ts > text {
        logits[..ts0u].fill(ninf);
    }
}

/// Segments from a window's tokens (prompt and end-of-text removed), and the seconds used
/// of `duration`.
pub(crate) fn segments(w: &Whisper, tokens: &[u32], duration: f32) -> (Vec<Segment>, f32) {
    let sp = &w.specials;
    let is_ts: Vec<bool> = tokens.iter().map(|&t| sp.is_timestamp(t)).collect();
    let n = tokens.len();
    let single_end = (n >= 2 && !is_ts[n - 2] && is_ts[n - 1]) || (n == 1 && is_ts[0]);
    let consecutive: Vec<usize> = (1..n).filter(|&i| is_ts[i] && is_ts[i - 1]).collect();
    let text = |ts: &[u32]| -> String {
        let plain: Vec<u32> = ts.iter().copied().filter(|&t| t < sp.eot).collect();
        w.vocab.decode(&plain).trim().to_owned()
    };
    let mut out = Vec::new();
    if !consecutive.is_empty() {
        let mut slices = consecutive.clone();
        if single_end {
            slices.push(n);
        }
        let mut last = 0;
        for &cur in &slices {
            let s = &tokens[last..cur];
            if let (Some(&a), Some(&b)) = (s.first(), s.last()) {
                let (start, end) = (sp.time(a), sp.time(b).min(duration));
                let t = text(s);
                if !t.is_empty() && end > start {
                    out.push(Segment {
                        start,
                        end,
                        text: t,
                    });
                }
            }
            last = cur;
        }
        let used = if single_end {
            duration
        } else {
            sp.time(tokens[last - 1])
        };
        // Always move on (a window stuck at its start would be read forever).
        let used = if used < 1.0 {
            duration
        } else {
            used.min(duration)
        };
        (out, used)
    } else {
        let mut end = duration;
        if let Some(&t) = tokens.iter().rev().find(|&&t| sp.is_timestamp(t))
            && t != sp.timestamp0
        {
            end = sp.time(t).min(duration);
        }
        let start = tokens
            .first()
            .filter(|&&t| sp.is_timestamp(t))
            .map_or(0.0, |&t| sp.time(t));
        let t = text(tokens);
        if !t.is_empty() && end > start {
            out.push(Segment {
                start,
                end,
                text: t,
            });
        }
        (out, duration)
    }
}

impl Whisper {
    /// The languages most likely spoken in `audio`, likeliest first, with probabilities.
    pub fn detect_language(&self, audio: &Audio) -> Result<Vec<(&'static str, f32)>, Error> {
        let mut st = self.state();
        let logits = self.step(&mut st, audio, self.specials.sot)?;
        Ok(self.languages(&logits))
    }

    fn languages(&self, logits: &[f32]) -> Vec<(&'static str, f32)> {
        let sp = &self.specials;
        if !sp.multilingual() {
            return vec![("en", 1.0)];
        }
        let range = sp.lang0 as usize..(sp.lang0 + sp.langs) as usize;
        let lp = log_softmax(&logits[range]);
        let mut v: Vec<(&'static str, f32)> = lp
            .iter()
            .enumerate()
            .filter_map(|(i, l)| Some((sp.language_code(sp.lang0 + i as u32)?, l.exp())))
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    }

    /// One decoding pass at `temperature` from the logits after start-of-transcript.
    #[allow(clippy::too_many_arguments)]
    fn pass(
        &self,
        audio: &Audio,
        prompt: &[u32],
        first: &[f32],
        temperature: f32,
        opts: &Options,
        duration: f32,
        rng: &mut Rng,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<Pass>, Error> {
        let sp = self.specials;
        let mut st = self.state();
        let no_speech = {
            let lp = log_softmax(first);
            lp[sp.no_speech as usize].exp()
        };
        let mut logits = Vec::new();
        for &t in prompt {
            logits = self.step(&mut st, audio, t)?;
        }
        let max = opts
            .max_tokens
            .min(self.config.text_ctx.saturating_sub(prompt.len() + 1));
        let last_ts = sp.timestamp0 + (duration / TIME_STEP).ceil() as u32;
        let blank = self.vocab.find(" ");
        let mut out: Vec<u32> = Vec::new();
        let mut sum = 0.0f32;
        let ninf = f32::NEG_INFINITY;
        for _ in 0..max {
            if cancel() {
                return Ok(None);
            }
            for &t in &self.suppress {
                logits[t as usize] = ninf;
            }
            // Controls: never (end-of-text and timestamps are decided below).
            logits[sp.eot as usize + 1..sp.timestamp0 as usize].fill(ninf);
            if out.is_empty() {
                if let Some(b) = blank {
                    logits[b as usize] = ninf;
                }
                logits[sp.eot as usize] = ninf;
            }
            timestamp_rules(
                &mut logits,
                &out,
                sp.timestamp0,
                sp.eot,
                sp.no_timestamps,
                last_ts,
            );
            let lp = log_softmax(&logits);
            let tok = if temperature <= 0.0 {
                argmax(&logits)
            } else {
                let scaled: Vec<f32> = logits.iter().map(|l| l / temperature).collect();
                let p = log_softmax(&scaled);
                let mut u = rng.unit();
                let mut pick = argmax(&scaled);
                for (i, l) in p.iter().enumerate() {
                    let pr = l.exp();
                    if u < pr {
                        pick = i as u32;
                        break;
                    }
                    u -= pr;
                }
                pick
            };
            if lp[tok as usize].is_finite() {
                sum += lp[tok as usize];
            }
            if tok == sp.eot {
                break;
            }
            out.push(tok);
            if out.len() >= max {
                break;
            }
            logits = self.step(&mut st, audio, tok)?;
        }
        let plain: Vec<u32> = out.iter().copied().filter(|&t| t < sp.eot).collect();
        let compression = compression_ratio(&self.vocab.decode(&plain));
        Ok(Some(Pass {
            avg_logprob: sum / (out.len() + 1) as f32,
            tokens: out,
            no_speech,
            compression,
        }))
    }

    /// Reads one window: up to 30 s of 16 kHz mono `audio`. `None` when `cancel` said so.
    pub fn transcribe_window(
        &mut self,
        audio: &[f32],
        opts: &Options,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<WindowText>, Error> {
        let duration = audio.len() as f32 / SAMPLE_RATE as f32;
        self.note(|s| s.windows += 1);
        let mel = self.mel(audio);
        let enc = self.encode(&mel)?;
        let sp = self.specials;
        let mut st = self.state();
        let first = self.step(&mut st, &enc, sp.sot)?;
        let language: &'static str = match &opts.language {
            Some(l) if sp.language(l).is_some() => sp
                .language(l)
                .and_then(|t| sp.language_code(t))
                .unwrap_or("en"),
            _ => self.languages(&first).first().map_or("en", |l| l.0),
        };
        let mut prompt = vec![sp.sot];
        if sp.multilingual() {
            prompt.push(sp.language(language).unwrap_or(sp.lang0));
            prompt.push(match opts.task {
                Task::Transcribe => sp.transcribe,
                Task::Translate => sp.translate,
            });
        }
        let mut rng = Rng::new(0x05ee_d0f5_eec4);
        let mut passes: Vec<Pass> = Vec::new();
        let mut chosen = None;
        for &t in &opts.temperatures {
            let Some(p) = self.pass(&enc, &prompt, &first, t, opts, duration, &mut rng, cancel)?
            else {
                return Ok(None);
            };
            let silent =
                p.no_speech > opts.no_speech_threshold && p.avg_logprob < opts.logprob_threshold;
            let fine = p.compression <= opts.compression_threshold
                && p.avg_logprob >= opts.logprob_threshold;
            passes.push(p);
            self.note(|s| s.passes += 1);
            if silent || fine {
                chosen = Some(passes.len() - 1);
                break;
            }
        }
        // None passed: the likeliest that does not loop, else the greedy one (sampling at a
        // high temperature reads worse than a slightly unlikely greedy pass).
        let i = chosen.unwrap_or_else(|| {
            passes
                .iter()
                .enumerate()
                .filter(|(_, p)| p.compression <= opts.compression_threshold)
                .max_by(|a, b| a.1.avg_logprob.total_cmp(&b.1.avg_logprob))
                .map_or(0, |(i, _)| i)
        });
        let p = passes.into_iter().nth(i).unwrap_or_default();
        if p.no_speech > opts.no_speech_threshold && p.avg_logprob < opts.logprob_threshold {
            return Ok(Some(WindowText {
                segments: Vec::new(),
                language,
                consumed: duration,
            }));
        }
        let (segments, consumed) = segments(self, &p.tokens, duration);
        Ok(Some(WindowText {
            segments,
            language,
            consumed,
        }))
    }

    /// Reads all of `audio` (16 kHz mono, any length) window by window; `on_window` gets
    /// each window's segments (times from the start of `audio`) and the share read so far.
    /// `None` when cancelled.
    pub fn transcribe(
        &mut self,
        audio: &[f32],
        opts: &Options,
        cancel: &dyn Fn() -> bool,
        on_window: &mut dyn FnMut(&[Segment], &'static str, f32),
    ) -> Result<Option<Vec<Segment>>, Error> {
        let window = super::mel::WINDOW_SAMPLES * self.config.audio_ctx / 1500;
        let mut seek = 0usize;
        let mut all = Vec::new();
        while seek < audio.len() {
            let end = (seek + window).min(audio.len());
            let Some(w) = self.transcribe_window(&audio[seek..end], opts, cancel)? else {
                return Ok(None);
            };
            let off = seek as f32 / SAMPLE_RATE as f32;
            let segs: Vec<Segment> = w
                .segments
                .iter()
                .map(|s| Segment {
                    start: s.start + off,
                    end: s.end + off,
                    text: s.text.clone(),
                })
                .collect();
            let used = ((w.consumed * SAMPLE_RATE as f32) as usize).max(SAMPLE_RATE / 10);
            seek = (seek + used).min(audio.len());
            on_window(&segs, w.language, seek as f32 / audio.len() as f32);
            all.extend(segs);
        }
        Ok(Some(all))
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    #[test]
    fn compression_ratio_flags_loops() {
        let plain = "The quick brown fox jumps over the lazy dog, and then it ran away.";
        assert!(
            compression_ratio(plain) < 1.6,
            "{}",
            compression_ratio(plain)
        );
        let looped = "I'm going to go. ".repeat(12);
        assert!(
            compression_ratio(&looped) > 2.4,
            "{}",
            compression_ratio(&looped)
        );
        assert_eq!(compression_ratio(""), 0.0);
    }

    fn logits(n: usize) -> Vec<f32> {
        vec![0.0; n]
    }

    #[test]
    fn timestamp_rules() {
        // A vocabulary of 10 text ids, eot 10, a control 11, no-timestamps 12, timestamps
        // 13.. (13 is 0.00 s).
        let (ts0, eot, nts) = (13u32, 10u32, 12u32);
        let n = 13 + 200;
        let ninf = f32::NEG_INFINITY;
        // First token: a timestamp of at most 1 s.
        let mut l = logits(n);
        super::timestamp_rules(&mut l, &[], ts0, eot, nts, 13 + 150);
        assert!(l[..13].iter().all(|v| *v == ninf));
        assert!(l[13..=63].iter().all(|v| v.is_finite()));
        assert!(l[64..].iter().all(|v| *v == ninf));
        // After a start timestamp: text (or another timestamp, closing an empty segment
        // would be equal or later).
        let mut l = logits(n);
        l[3] = 10.0;
        super::timestamp_rules(&mut l, &[13 + 5], ts0, eot, nts, 13 + 150);
        assert_eq!(l[12], ninf);
        assert!(l[13..18].iter().all(|v| *v == ninf), "no earlier timestamp");
        assert!(l[3].is_finite());
        // After text then a timestamp (an end): the next is a timestamp (or the end).
        let mut l = logits(n);
        l[10] = 10.0;
        super::timestamp_rules(&mut l, &[13 + 5, 3, 13 + 40], ts0, eot, nts, 13 + 150);
        assert!(l[..10].iter().all(|v| *v == ninf));
        assert!(l[10].is_finite(), "end allowed");
        assert!(l[13..13 + 40].iter().all(|v| *v == ninf));
        assert!(
            l[13 + 40].is_finite(),
            "the next segment may start where this ended"
        );
        // After a pair: text.
        let mut l = logits(n);
        l[3] = 5.0;
        super::timestamp_rules(
            &mut l,
            &[13 + 5, 3, 13 + 40, 13 + 40],
            ts0,
            eot,
            nts,
            13 + 150,
        );
        assert!(l[13..].iter().all(|v| *v == ninf));
        // Never past the sound.
        let mut l = logits(n);
        super::timestamp_rules(&mut l, &[13 + 5, 3], ts0, eot, nts, 13 + 100);
        assert!(l[13 + 101..].iter().all(|v| *v == ninf));
        // Timestamps together likelier than any text token: a timestamp.
        let mut l = logits(n);
        l[3] = 2.0;
        for v in l[13 + 6..13 + 100].iter_mut() {
            *v = 0.5;
        }
        super::timestamp_rules(&mut l, &[13 + 5, 3, 4], ts0, eot, nts, 13 + 150);
        assert_eq!(l[3], ninf);
    }

    #[test]
    fn segments_from_tokens() {
        let w = testing::tiny(1, None, false);
        let ts = |s: f32| w.specials.timestamp0 + (s / TIME_STEP).round() as u32;
        // "Hello world." 0.0–1.5, "the (" 1.5–2.0 then a lone timestamp: read to the end.
        let toks = [ts(0.0), 0, 1, 2, ts(1.5), ts(1.5), 4, ts(2.0)];
        let (s, used) = segments(&w, &toks, 10.0);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].text, "Hello world.");
        assert_eq!((s[0].start, s[0].end), (0.0, 1.5));
        assert_eq!(s[1].text, "the");
        assert!((s[1].end - 2.0).abs() < 1e-6);
        assert_eq!(used, 10.0);
        // A sentence cut off after the last pair: the next window starts at its start.
        let toks = [ts(0.0), 0, ts(3.0), ts(3.0), 1];
        let (s, used) = segments(&w, &toks, 10.0);
        assert_eq!(s.len(), 1);
        assert!((used - 3.0).abs() < 1e-6);
        // No pairs: one segment to the last timestamp.
        let toks = [ts(0.4), 0, 1, ts(2.5)];
        let (s, used) = segments(&w, &toks, 10.0);
        assert_eq!(s.len(), 1);
        assert!((s[0].start - 0.4).abs() < 1e-6 && (s[0].end - 2.5).abs() < 1e-6);
        assert_eq!(used, 10.0);
        // Nothing.
        assert_eq!(segments(&w, &[], 5.0), (Vec::new(), 5.0));
    }

    #[test]
    fn decodes_a_window_deterministically() {
        let mut w = testing::tiny(5, None, false);
        let audio: Vec<f32> = (0..16_000)
            .map(|i| ((i as f32) * 0.07).sin() * 0.2)
            .collect();
        let opts = Options {
            max_tokens: 12,
            temperatures: vec![0.0, 0.5],
            ..Options::default()
        };
        let a = w
            .transcribe_window(&audio, &opts, &|| false)
            .unwrap()
            .unwrap();
        let b = w
            .transcribe_window(&audio, &opts, &|| false)
            .unwrap()
            .unwrap();
        assert_eq!(a, b);
        assert!(a.consumed > 0.0 && a.consumed <= 1.0);
        for s in &a.segments {
            assert!(s.start < s.end && s.end <= 1.0);
        }
        assert!(super::super::tokens::LANGUAGES.contains(&a.language));
        // Given the language and translation: still reads.
        let o2 = Options {
            language: Some("de".into()),
            task: Task::Translate,
            ..opts.clone()
        };
        assert_eq!(
            w.transcribe_window(&audio, &o2, &|| false)
                .unwrap()
                .unwrap()
                .language,
            "de"
        );
        // Cancelled.
        assert!(
            w.transcribe_window(&audio, &opts, &|| true)
                .unwrap()
                .is_none()
        );
        let langs = {
            let mel = w.mel(&audio);
            let enc = w.encode(&mel).unwrap();
            w.detect_language(&enc).unwrap()
        };
        assert_eq!(langs.len(), 99);
        let total: f32 = langs.iter().map(|l| l.1).sum();
        assert!((total - 1.0).abs() < 1e-3);
    }
}

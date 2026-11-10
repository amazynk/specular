//! A decoder for the samples a container hands over, whatever their framing: raw data
//! blocks with an AudioSpecificConfig (MP4, Matroska), ADTS frames, or LOAS/LATM elements
//! (MPEG transport streams). Implicit SBR/PS signalling is settled from the first samples.
//! A LATM stream whose StreamMuxConfig changes (stereo to 5.1 at a programme change) is
//! followed, with the output channels kept as they started.

use crate::latm::{Latm, is_loas};
use crate::{Config, Decoder, Error, Options, Result, adts, parse_asc, resolve_implicit};

/// How the samples are framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// One raw data block per sample.
    Raw,
    /// ADTS frames (any number per sample).
    Adts,
    /// LOAS/LATM elements (any number per sample).
    Loas,
}

/// Decodes a stream's samples to interleaved f32.
pub struct StreamDecoder {
    framing: Framing,
    opts: Options,
    dec: Decoder,
    /// The configuration the decoder was made from, before implicit signalling was settled
    /// (a LATM StreamMuxConfig that repeats it is no change).
    base: Config,
    /// Output channels and rate, fixed when the stream starts.
    channels: u16,
    rate: u32,
    latm: Latm,
    blocks: Vec<Vec<u8>>,
    /// The start of a frame the last sample cut off.
    pending: Vec<u8>,
    conceal: bool,
    scratch: Vec<f32>,
}

/// Splits `data` into the raw data blocks of its whole ADTS frames or LOAS elements; returns
/// where an incomplete last one starts (frames may straddle samples in transport streams),
/// and the first error of an element that could not be read.
fn split(
    framing: Framing,
    latm: &mut Latm,
    data: &[u8],
    out: &mut Vec<Vec<u8>>,
) -> (usize, Option<Error>) {
    let mut at = 0;
    let mut err = None;
    while at < data.len() {
        let rest = &data[at..];
        let (len, payload) = match framing {
            Framing::Raw => {
                out.push(rest.to_vec());
                return (data.len(), None);
            }
            Framing::Adts => match adts::parse(rest) {
                Ok(h) => (h.frame_len, Some(h.payload())),
                Err(_) if rest.len() < 7 => break,
                Err(_) => {
                    at += 1;
                    continue;
                }
            },
            Framing::Loas => {
                if rest.len() < 3 {
                    break;
                }
                if !is_loas(rest) {
                    at += 1;
                    continue;
                }
                let n = 3 + ((usize::from(rest[1] & 0x1f) << 8) | usize::from(rest[2]));
                (n, None)
            }
        };
        if len > rest.len() {
            break;
        }
        match (framing, payload) {
            // several raw data blocks without positions can't be split; the first is read
            // (encoders send one)
            (Framing::Adts, Some(p)) => out.push(rest[p].to_vec()),
            _ => {
                if let Err(e) = latm.parse(&rest[3..len], out) {
                    err.get_or_insert(e);
                }
            }
        }
        at += len;
    }
    (at, err)
}

/// The raw data blocks of whole samples (for probing).
fn blocks(framing: Framing, latm: &mut Latm, sample: &[u8], out: &mut Vec<Vec<u8>>) {
    split(framing, latm, sample, out);
}

/// Converts interleaved `from`-channel frames to `to` channels: mono to both sides, stereo
/// averaged to mono, and any smaller layout placed at the front of a larger one (L R, or
/// L R C for 3.0) with the rest silent. A larger layout is never folded here (decoders are
/// made with the stereo downmix for that).
fn fit_channels(src: &[f32], from: usize, to: usize, out: &mut Vec<f32>) {
    if from == 0 || to == 0 {
        return;
    }
    let frames = src.len() / from;
    out.reserve(frames * to);
    for f in src.chunks_exact(from) {
        match (from, to) {
            (1, _) => out.extend(
                std::iter::repeat_n(f[0], to.min(2))
                    .chain(std::iter::repeat_n(0.0, to.saturating_sub(2))),
            ),
            (_, 1) => out.push(f.iter().take(2).sum::<f32>() / from.min(2) as f32),
            _ => out.extend((0..to).map(|c| f.get(c).copied().unwrap_or(0.0))),
        }
    }
}

impl StreamDecoder {
    /// A decoder from the container's AudioSpecificConfig (empty when the samples carry
    /// their own framing) and the stream's first samples (to tell the framing and settle
    /// implicit SBR/PS signalling; more help, up to a second or two).
    pub fn new(asc: &[u8], first: &[&[u8]], opts: Options) -> Result<StreamDecoder> {
        let lead = first.first().copied().unwrap_or_default();
        let framing = if !asc.is_empty() {
            Framing::Raw
        } else if adts::parse(lead).is_ok() {
            Framing::Adts
        } else if is_loas(lead) {
            Framing::Loas
        } else if first.is_empty() {
            return Err(Error::Unsupported("AAC without a configuration".into()));
        } else {
            return Err(Error::Malformed("no ADTS or LOAS framing".into()));
        };
        let mut latm = Latm::new();
        let mut probe = Vec::new();
        for s in first {
            // a sample that fails to split only shortens the probe
            blocks(framing, &mut latm, s, &mut probe);
        }
        let cfg: Config = match framing {
            Framing::Raw => parse_asc(asc)?,
            Framing::Adts => adts::parse(lead)?.config()?,
            Framing::Loas => {
                // the first configuration, probed with the blocks it applies to
                let changes = latm.take_changes();
                let (_, c) = changes.first().cloned().ok_or_else(|| {
                    Error::Malformed("no StreamMuxConfig in the first samples".into())
                })?;
                if let Some(&(end, _)) = changes.get(1) {
                    probe.truncate(end);
                }
                c
            }
        };
        let resolved = resolve_implicit(&cfg, probe.iter().map(Vec::as_slice));
        let dec = Decoder::with_config(resolved, opts)?;
        Ok(StreamDecoder {
            framing,
            opts,
            channels: dec.channels(),
            rate: dec.sample_rate(),
            dec,
            base: cfg,
            latm: Latm::new(),
            blocks: Vec::new(),
            pending: Vec::new(),
            conceal: true,
            scratch: Vec::new(),
        })
    }

    /// The stream's configuration (with implicit signalling settled).
    pub fn config(&self) -> &Config {
        self.dec.config()
    }

    pub fn framing(&self) -> Framing {
        self.framing
    }

    /// Output channels (as the stream started; a later LATM configuration is fitted to them).
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Output rate (as the stream started).
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Samples per channel each raw data block decodes to.
    pub fn frame_len(&self) -> usize {
        self.dec.frame_len()
    }

    /// Whether SBR is in use (it delays the output by about a frame more than AAC-LC).
    pub fn sbr(&self) -> bool {
        self.dec.sbr_active()
    }

    /// Whether a block that fails is concealed (the default) or gives no output at all.
    pub fn set_concealment(&mut self, on: bool) {
        self.conceal = on;
    }

    /// Forgets inter-frame state (a seek). The multiplex configuration is kept.
    pub fn reset(&mut self) {
        self.dec.reset();
        self.pending.clear();
    }

    /// Switches to a new LATM configuration, settling its implicit signalling from the
    /// blocks that follow. A layout of more channels than the output is folded to stereo.
    fn follow(&mut self, cfg: Config, next: &[Vec<u8>]) -> Result<()> {
        if cfg == self.base {
            return Ok(());
        }
        let resolved = resolve_implicit(&cfg, next.iter().map(Vec::as_slice));
        let mut opts = self.opts;
        if self.channels <= 2 {
            opts.stereo_downmix = true;
        }
        self.dec = Decoder::with_config(resolved, opts)?;
        self.base = cfg;
        Ok(())
    }

    /// Decodes every raw data block of one sample, appending interleaved samples to `out`.
    /// A block that fails is concealed (see [`Decoder::conceal`]), so the output keeps its
    /// length; the first such error is returned after everything is appended.
    pub fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<usize> {
        self.blocks.clear();
        let joined;
        let data = if self.pending.is_empty() {
            sample
        } else {
            let mut j = std::mem::take(&mut self.pending);
            j.extend_from_slice(sample);
            joined = j;
            &joined[..]
        };
        let (used, mut first_err) = split(self.framing, &mut self.latm, data, &mut self.blocks);
        self.pending.clear();
        self.pending
            .extend_from_slice(&data[used.min(data.len())..]);
        if self.pending.len() > 1 << 16 {
            // not a frame start after all
            self.pending.clear();
        }
        let mut changes = self.latm.take_changes().into_iter().peekable();
        let blocks = std::mem::take(&mut self.blocks);
        let mut n = 0;
        for (i, b) in blocks.iter().enumerate() {
            while let Some((_, cfg)) = changes.next_if(|c| c.0 <= i) {
                if let Err(e) = self.follow(cfg, &blocks[i..]) {
                    first_err.get_or_insert(e);
                }
            }
            let ch = usize::from(self.dec.channels());
            let want = usize::from(self.channels);
            let target = if ch == want {
                &mut *out
            } else {
                self.scratch.clear();
                &mut self.scratch
            };
            match self.dec.decode(b, target) {
                Ok(k) => n += k,
                Err(e) => {
                    if self.conceal {
                        n += self.dec.conceal(target);
                    }
                    first_err.get_or_insert(e);
                }
            }
            if ch != want {
                fit_channels(&self.scratch, ch, want, out);
            }
        }
        self.blocks = blocks;
        match first_err {
            Some(e) => Err(e),
            None => Ok(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_fitted() {
        let mut out = Vec::new();
        fit_channels(&[0.5, -0.5], 1, 2, &mut out);
        assert_eq!(out, [0.5, 0.5, -0.5, -0.5]);
        out.clear();
        fit_channels(&[0.5, 0.25], 2, 1, &mut out);
        assert_eq!(out, [0.375]);
        out.clear();
        fit_channels(&[0.5, 0.25], 2, 6, &mut out);
        assert_eq!(out, [0.5, 0.25, 0.0, 0.0, 0.0, 0.0]);
        out.clear();
        fit_channels(&[0.5], 1, 6, &mut out);
        assert_eq!(out, [0.5, 0.5, 0.0, 0.0, 0.0, 0.0]);
    }
}

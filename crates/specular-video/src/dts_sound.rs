//! DTS sound (`specular-dts`, the core of DTS and DTS-HD streams) for [`crate::audio`]:
//! frames found across samples, folded to stereo. Also DTS carried as 16-bit PCM (DTS CDs
//! ripped to WAVE, such AVI tracks), found by its sync words.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack};

/// Samples read to find the stream's first frames and check they can be decoded.
const PROBE_SAMPLES: usize = 16;

/// The decoder, and its scratch when it plays surround (5.1).
struct Dts(specular_dts::Decoder, Option<Vec<f32>>);

/// A DTS channel arrangement's channels in the WAVE order: front channels (1 or 2), a
/// centre, and surrounds (0 to 2).
fn layout(amode: u8) -> (usize, bool, usize) {
    match amode {
        0 => (1, false, 0),
        5 => (2, true, 0),
        6 => (2, false, 1),
        7 => (2, true, 1),
        8 => (2, false, 2),
        9 => (2, true, 2),
        _ => (2, false, 0),
    }
}

impl Dts {
    fn decode_into(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        match self.0.decode(sample, out) {
            Ok(()) => Ok(()),
            // a damaged frame is concealed (its length in silence): the sound keeps time
            Err(specular_dts::Error::Malformed(m)) => {
                specular_log::debug!(target: specular_log::DECODE, "video: DTS sound: {m}");
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

impl SoundDecoder for Dts {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let Some(scratch) = &mut self.1 else {
            return self.decode_into(sample, out);
        };
        let mut s = std::mem::take(scratch);
        s.clear();
        let r = self.decode_into(sample, &mut s);
        if let Some(i) = self.0.info() {
            let (front, centre, surrounds) = layout(i.amode);
            crate::speakers::wave_to_51(&s, front, centre, i.lfe > 0, surrounds, out);
        }
        self.1 = Some(s);
        r
    }
    fn reset(&mut self) {
        self.0.reset();
    }
    fn preroll(&self) -> usize {
        // the synthesis filter bank's 512 taps, and a frame begun in the sample before
        2
    }
}

/// The first bytes of the track (a few samples), for probing.
fn head(t: &AudioTrack, data: &[u8]) -> Vec<u8> {
    let mut head = Vec::new();
    for s in t.samples.iter().take(PROBE_SAMPLES) {
        if let Some(b) = t.sample_data(data, s) {
            head.extend_from_slice(&b);
        }
        if head.len() > 64 * 1024 {
            break;
        }
    }
    head
}

/// Whether a track's codec is DTS with a core this decoder can play (DTS Express and
/// DTS:X have none).
pub(crate) fn is_dts(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if matches!(t, b"dtsc" | b"dtsh" | b"dtsl"))
}

/// Fills in the track's channels and rate from its first core frame; a 16-bit PCM track
/// whose samples are DTS frames becomes a DTS track.
pub(crate) fn complete(data: &[u8], t: &mut AudioTrack) {
    if let AudioCodec::Pcm { bits: 16, .. } = t.codec {
        if t.channels != 2 || specular_dts::probe(&head(t, data)).is_none() {
            return;
        }
        t.codec = AudioCodec::Other(*b"dtsc", 0);
    }
    if let Some(s) = specular_dts::probe(&head(t, data)) {
        t.channels = s.first.channels as u16;
        t.sample_rate = s.first.sample_rate;
    }
}

/// The decoder of a DTS track and the movie's bytes: two channels at the core's rate, or
/// with `multi` six (5.1) when the core has more than two. A stream whose first frames use
/// a tool the decoder lacks is refused up front, named.
pub(crate) fn decoder(
    t: &AudioTrack,
    data: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let opts = specular_dts::Options {
        stereo: true,
        ..Default::default()
    };
    let (rate, surround) = if data.is_empty() {
        (t.sample_rate, false)
    } else {
        let head = head(t, data);
        let s = specular_dts::probe(&head).ok_or_else(|| Silence::Unsupported(t.codec.name()))?;
        let mut trial = specular_dts::Decoder::new(opts);
        let mut scratch = Vec::new();
        if let Err(specular_dts::Error::Unsupported(why)) = trial.decode(&head, &mut scratch) {
            let what = if why.contains("ADPCM") {
                "DTS with ADPCM prediction".to_string()
            } else if why.contains("VQ") {
                "DTS with high-frequency VQ".to_string()
            } else {
                format!("{} ({why})", t.codec.name())
            };
            return Err(Silence::Unsupported(what));
        }
        (s.first.sample_rate, s.first.channels > 2)
    };
    if multi && surround {
        let dec = specular_dts::Decoder::new(specular_dts::Options {
            stereo: false,
            ..opts
        });
        return Ok((Box::new(Dts(dec, Some(Vec::new()))), 6, rate));
    }
    let dec = specular_dts::Decoder::new(opts);
    Ok((Box::new(Dts(dec, None)), 2, rate))
}

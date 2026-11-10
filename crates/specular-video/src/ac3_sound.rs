//! AC-3 and E-AC-3 sound (`specular-ac3`) for [`crate::audio`]: syncframes found across
//! samples (transport and programme stream packets cut frames anywhere), folded to stereo
//! with the stream's mix levels, dynamic range control and dialogue normalisation; or, for a
//! device that plays surround, every channel as 5.1.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack};

/// Samples read to find the stream's first frames.
const PROBE_SAMPLES: usize = 16;

struct Ac3(specular_ac3::Decoder, Option<Vec<f32>>);

impl SoundDecoder for Ac3 {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let Some(scratch) = &mut self.1 else {
            return self.decode_into(sample, out);
        };
        let mut s = std::mem::take(scratch);
        s.clear();
        let r = self.decode_into(sample, &mut s);
        if self.0.extended() {
            // dependent substreams: 5.1 or 7.1 already, in the WAVE order
            out.extend_from_slice(&s);
        } else if let Some(i) = self.0.info() {
            let (front, centre, surrounds) = layout(i.acmod);
            crate::speakers::wave_to_51(&s, front, centre, i.lfe, surrounds, out);
        }
        self.1 = Some(s);
        r
    }
    fn reset(&mut self) {
        self.0.reset();
    }
    fn preroll(&self) -> usize {
        // the transform's overlap, and a frame begun in the sample before
        2
    }
}

/// An AC-3 coding mode's channels in the WAVE order: front channels (1 or 2), a centre,
/// and surrounds (0 to 2).
fn layout(acmod: u8) -> (usize, bool, usize) {
    match acmod {
        1 => (1, false, 0),
        3 => (2, true, 0),
        4 => (2, false, 1),
        5 => (2, true, 1),
        6 => (2, false, 2),
        7 => (2, true, 2),
        _ => (2, false, 0),
    }
}

impl Ac3 {
    fn decode_into(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        match self.0.decode(sample, out) {
            Ok(()) => Ok(()),
            // a damaged frame is concealed (its length in silence): the sound keeps time
            Err(specular_ac3::Error::Malformed(m)) => {
                specular_log::debug!(target: specular_log::DECODE, "video: AC-3 sound: {m}");
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

/// The first bytes of the track (a few samples), for probing.
fn head(t: &AudioTrack, data: &[u8]) -> Vec<u8> {
    let mut head = Vec::new();
    for s in t.samples.iter().take(PROBE_SAMPLES) {
        if let Some(b) = t.sample_data(data, s) {
            head.extend_from_slice(&b);
        }
        if head.len() > 32 * 1024 {
            break;
        }
    }
    head
}

/// Fills in the track's coding (AC-3 or E-AC-3 as the frames say), channels (with E-AC-3's
/// extension channels: 8 for 7.1) and rate from its first frames.
pub(crate) fn complete(data: &[u8], t: &mut AudioTrack) {
    if let Some(s) = specular_ac3::probe(&head(t, data)) {
        t.codec = if s.first.eac3 {
            AudioCodec::Eac3
        } else {
            AudioCodec::Ac3
        };
        t.channels = s.channels as u16;
        t.sample_rate = s.first.sample_rate;
    }
}

/// The decoder of an AC-3 or E-AC-3 track and the movie's bytes: two channels at the
/// stream's rate, or with `multi` six (5.1) when the stream has more than two.
pub(crate) fn decoder(
    t: &AudioTrack,
    data: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let (rate, surround, deps) = if data.is_empty() {
        (t.sample_rate, false, None)
    } else {
        specular_ac3::probe(&head(t, data))
            .map(|s| {
                let deps = s.dependents.then(|| s.output_channels() as u16);
                (
                    s.first.sample_rate,
                    s.first.acmod > 2 || s.first.lfe || deps.is_some(),
                    deps,
                )
            })
            .ok_or_else(|| Silence::Unsupported(t.codec.name()))?
    };
    if multi && surround {
        let dec = specular_ac3::Decoder::new(specular_ac3::Options {
            stereo: false,
            ..specular_ac3::Options::default()
        })
        .with_dependents(deps.is_some());
        // 7.1 from dependent substreams plays as 7.1; everything else as 5.1
        return Ok((
            Box::new(Ac3(dec, Some(Vec::new()))),
            deps.unwrap_or(6),
            rate,
        ));
    }
    let dec = specular_ac3::Decoder::new(specular_ac3::Options::default())
        .with_dependents(deps.is_some());
    Ok((Box::new(Ac3(dec, None)), 2, rate))
}

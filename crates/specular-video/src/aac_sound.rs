//! AAC sound (`specular-aac`) for [`crate::audio`]: AAC-LC, Main, LTP, HE-AAC and HE-AAC
//! v2, as raw data blocks with an AudioSpecificConfig (MP4, Matroska) or ADTS or LOAS/LATM
//! framed (MPEG-TS, the configuration then empty). More than two channels come as 5.1 in the WAVE
//! order the channel folding takes (L R C LFE Ls Rs).

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::AudioTrack;

/// Samples read before playing to tell the framing and settle implicit SBR/PS signalling
/// (SBR headers come every few frames).
const PROBE_SAMPLES: usize = 48;

/// An AAC track's decoder.
struct Aac {
    dec: specular_aac::StreamDecoder,
    preroll: usize,
}

impl SoundDecoder for Aac {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        // Frames that fail are concealed: the sound keeps its length and fades.
        if let Err(e) = self.dec.decode(sample, out) {
            specular_log::debug!(target: specular_log::DECODE, "video: AAC sound: {e}");
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.dec.reset();
    }
    fn preroll(&self) -> usize {
        self.preroll
    }
}

/// Why a stream is refused, for the user: the variant by name, else just "AAC".
fn refuse(e: specular_aac::Error) -> Silence {
    specular_log::debug!(target: specular_log::DECODE, "video: AAC sound: {e}");
    match e {
        specular_aac::Error::Unsupported(m) => Silence::Unsupported(m),
        specular_aac::Error::Malformed(_) => Silence::Unsupported("AAC".into()),
    }
}

/// Whether the track's sound can be decoded, from its configuration alone (no samples read).
pub(crate) fn check(asc: &[u8]) -> Result<(), Silence> {
    if asc.is_empty() {
        // ADTS or LATM framed: the configuration is in the samples
        return Ok(());
    }
    let cfg = specular_aac::parse_asc(asc).map_err(refuse)?;
    specular_aac::Decoder::with_config(cfg, specular_aac::Options::default())
        .map(|_| ())
        .map_err(refuse)
}

/// The decoder of an AAC track with its AudioSpecificConfig (empty for ADTS or LATM framing)
/// and the movie's bytes: output channels and rate, or why not.
pub(crate) fn decoder(
    t: &AudioTrack,
    asc: &[u8],
    data: &[u8],
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let first: Vec<_> = t
        .samples
        .iter()
        .take(PROBE_SAMPLES)
        .filter_map(|s| t.sample_data(data, s))
        .collect();
    let first: Vec<&[u8]> = first.iter().map(|c| &c[..]).collect();
    let opts = specular_aac::Options {
        stereo_downmix: false,
    };
    let dec = specular_aac::StreamDecoder::new(asc, &first, opts).map_err(refuse)?;
    let ch = dec.channels().max(1);
    let rate = dec.sample_rate();
    // the overlap of the transform, and SBR's own filterbanks and envelope history
    let preroll = if dec.sbr() { 2 } else { 1 };
    Ok((Box::new(Aac { dec, preroll }), ch, rate))
}

/// The facts of an AAC track for the info panel: the coding's name (AAC, HE-AAC, HE-AAC v2),
/// channels and rate as decoded, when the configuration says.
pub(crate) fn facts(asc: &[u8]) -> Option<(&'static str, usize, u32)> {
    let cfg = specular_aac::parse_asc(asc).ok()?;
    let ch = cfg.output_channels();
    Some((cfg.name(), ch, cfg.output_rate()))
}

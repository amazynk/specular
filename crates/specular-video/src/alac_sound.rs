//! Apple Lossless sound (`specular-alac`) for [`crate::audio`]: one frame a sample, channels
//! already in the WAVE order. The frame buffers are reserved from the memory budget.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::AudioTrack;

struct Alac {
    dec: specular_alac::Decoder,
    channels: usize,
    _memory: specular_core::budget::Reservation,
}

impl SoundDecoder for Alac {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let at = out.len();
        if let Err(e) = self.dec.decode(sample, out) {
            // A damaged frame: a frame of silence keeps the timing.
            out.truncate(at);
            let n = self.dec.config().frame_length as usize;
            out.resize(at + n * self.channels, 0.0);
            specular_log::debug!(target: specular_log::DECODE, "video: ALAC frame: {e}");
        }
        Ok(())
    }
    fn reset(&mut self) {}
}

/// The config's channels and rate (the container's can be wrong for ALAC in Matroska).
pub(crate) fn facts(config: &[u8]) -> Option<(u16, u32)> {
    let c = specular_alac::Config::parse(config).ok()?;
    Some((u16::from(c.channels), c.sample_rate))
}

pub(crate) fn decoder(
    t: &AudioTrack,
    config: &[u8],
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let c = specular_alac::Config::parse(config).map_err(|e| {
        specular_log::debug!(target: specular_log::DECODE, "video: ALAC sound: {e}");
        match e {
            specular_alac::Error::Unsupported(m) => Silence::Unsupported(format!("ALAC ({m})")),
            specular_alac::Error::Malformed(_) => Silence::Unsupported("damaged ALAC".into()),
        }
    })?;
    let memory = specular_core::budget::reserve(specular_alac::Decoder::buffer_bytes(&c))
        .map_err(|e| Silence::Unsupported(format!("ALAC ({e})")))?;
    let rate = if c.sample_rate > 0 {
        c.sample_rate
    } else {
        t.sample_rate
    };
    let ch = u16::from(c.channels);
    Ok((
        Box::new(Alac {
            dec: specular_alac::Decoder::new(c),
            channels: usize::from(ch),
            _memory: memory,
        }),
        ch,
        rate,
    ))
}

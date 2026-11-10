//! Speex sound (`specular-speex`) for [`crate::audio`]: Ogg Speex (the header says the band
//! and the frames a packet) and FLV Speex (wideband mono, every frame in the tag).

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::AudioTrack;

struct Speex {
    dec: specular_speex::Decoder,
    /// Frames a packet (Ogg), or all that are there (FLV).
    frames: Option<usize>,
}

impl SoundDecoder for Speex {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let at = out.len();
        let n = self
            .dec
            .decode(sample, self.frames, out)
            .map_err(|e| e.to_string())?;
        // A packet cut short keeps its length in silence.
        if let Some(f) = self.frames
            && n < f
        {
            let want = at + f * self.dec.frame_size() * self.dec.channels();
            out.resize(want.max(out.len()), 0.0);
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.dec.reset();
    }

    /// The decoder's filters and pitch history settle within a few frames.
    fn preroll(&self) -> usize {
        3
    }
}

/// The decoder of a Speex track with its header (empty for FLV): channels and rate.
pub(crate) fn decoder(
    t: &AudioTrack,
    header: &[u8],
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let (mode, channels, frames) = if header.is_empty() {
        (1, 1, None)
    } else {
        let h = specular_speex::Header::parse(header)
            .map_err(|e| Silence::Unsupported(e.to_string()))?;
        (h.mode, h.channels, Some(h.frames_per_packet as usize))
    };
    let dec = specular_speex::Decoder::new(mode, channels);
    let rate = if t.sample_rate > 0 {
        t.sample_rate
    } else {
        dec.sample_rate()
    };
    let ch = dec.channels() as u16;
    Ok((Box::new(Speex { dec, frames }), ch, rate))
}

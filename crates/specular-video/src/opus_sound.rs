//! Opus sound (`specular-opus`) for [`crate::audio`]: 48 kHz; more than two channels come in
//! the Vorbis order and are put in the WAVE order the channel folding takes (L R C LFE Ls Rs;
//! 7.1 as L R C LFE Lb Rb Ls Rs for a device that plays it).
//!
//! Ambisonics (mapping families 2 and 3) play as stereo from two virtual cardioid
//! microphones facing left and right, made from the first-order components; higher orders
//! are left out, and a non-diegetic stereo pair is added as it is. There is no binaural
//! rendering or head tracking. Family 255 (channels without a layout) plays its first two
//! channels.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::AudioTrack;

/// How the decoded channels reach the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Render {
    /// One or two channels, as they are.
    Plain,
    /// Vorbis order to WAVE 5.1.
    Surround,
    /// Vorbis 7.1 to WAVE 7.1.
    Seven,
    /// Ambisonics with `components` components (ACN, SN3D) and `extra` non-diegetic
    /// channels (0 or 2) after them, to stereo.
    Ambisonics { components: usize, extra: usize },
    /// No layout: the first two channels.
    FirstTwo,
}

/// An Opus track's decoder.
struct Opus {
    dec: specular_opus::Decoder,
    ch: usize,
    render: Render,
    /// Packets decoded before the one wanted after a seek (80 ms, RFC 7845 §4.6).
    preroll: usize,
    scratch: Vec<f32>,
}

impl Opus {
    /// One packet's sound through `f` (a decode of the stream), in the output order.
    fn run(
        &mut self,
        out: &mut Vec<f32>,
        f: impl FnOnce(
            &mut specular_opus::Decoder,
            &mut Vec<f32>,
        ) -> Result<usize, specular_opus::Error>,
    ) -> Result<(), String> {
        if self.render == Render::Plain {
            return f(&mut self.dec, out).map(|_| ()).map_err(|e| e.to_string());
        }
        self.scratch.clear();
        f(&mut self.dec, &mut self.scratch).map_err(|e| e.to_string())?;
        match self.render {
            Render::Seven => vorbis_to_wave71(&self.scratch, out),
            Render::Surround => vorbis_to_wave(&self.scratch, self.ch, out),
            Render::Ambisonics { components, extra } => {
                ambisonics_to_stereo(&self.scratch, components, extra, out)
            }
            Render::FirstTwo | Render::Plain => {
                for fr in self.scratch.chunks_exact(self.ch) {
                    out.extend_from_slice(&fr[..2]);
                }
            }
        }
        Ok(())
    }
}

impl SoundDecoder for Opus {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.run(out, |d, o| d.decode(sample, o))
    }
    /// A lost packet comes back from the next one's in-band FEC (SILK LBRR) where it has
    /// some, else concealed.
    fn decode_lost(
        &mut self,
        _index: usize,
        next: Option<&[u8]>,
        out: &mut Vec<f32>,
    ) -> Option<Result<(), String>> {
        Some(self.run(out, |d, o| match next {
            Some(n) => d.decode_fec(n, o),
            None => d.decode(&[], o),
        }))
    }
    fn reset(&mut self) {
        self.dec.reset();
    }
    fn preroll(&self) -> usize {
        self.preroll
    }
}

/// Interleaved frames of `ch` channels in the Vorbis order (L C R, Ls Rs, back, LFE last) as
/// six in the WAVE order (L R C LFE Ls Rs); back channels go to the sides at −3 dB.
pub(crate) fn vorbis_to_wave(src: &[f32], ch: usize, out: &mut Vec<f32>) {
    const M3: f32 = std::f32::consts::FRAC_1_SQRT_2;
    for f in src.chunks_exact(ch) {
        let [l, c, r, ls, rs, lfe] = match ch {
            3 => [f[0], f[1], f[2], 0.0, 0.0, 0.0],
            4 => [f[0], 0.0, f[1], f[2], f[3], 0.0],
            5 => [f[0], f[1], f[2], f[3], f[4], 0.0],
            6 => [f[0], f[1], f[2], f[3], f[4], f[5]],
            // 6.1: L C R Ls Rs Cs LFE; 7.1: L C R Ls Rs Lb Rb LFE.
            7 => [f[0], f[1], f[2], f[3] + f[5] * M3, f[4] + f[5] * M3, f[6]],
            _ => [f[0], f[1], f[2], f[3] + f[5] * M3, f[4] + f[6] * M3, f[7]],
        };
        out.extend_from_slice(&[l, r, c, lfe, ls, rs]);
    }
}

/// Interleaved ambisonics frames (`components` in ACN order with SN3D, then `extra`
/// non-diegetic channels) as stereo: cardioids facing ±90° from W and Y (½(W ± Y)); the
/// non-diegetic pair (or one channel to both sides) added.
pub(crate) fn ambisonics_to_stereo(
    src: &[f32],
    components: usize,
    extra: usize,
    out: &mut Vec<f32>,
) {
    let ch = components + extra;
    for f in src.chunks_exact(ch.max(1)) {
        let w = f[0];
        let y = if components >= 4 { f[1] } else { 0.0 };
        let (mut l, mut r) = (0.5 * (w + y), 0.5 * (w - y));
        if components == 1 {
            (l, r) = (w, w);
        }
        match extra {
            2 => {
                l += f[components];
                r += f[components + 1];
            }
            1 => {
                l += f[components];
                r += f[components];
            }
            _ => {}
        }
        out.extend_from_slice(&[l, r]);
    }
}

/// Interleaved 7.1 frames in the Vorbis order (L C R Ls Rs Lb Rb LFE) in the WAVE order
/// (L R C LFE Lb Rb Ls Rs).
pub(crate) fn vorbis_to_wave71(src: &[f32], out: &mut Vec<f32>) {
    for f in src.chunks_exact(8) {
        out.extend_from_slice(&[f[0], f[2], f[1], f[7], f[5], f[6], f[3], f[4]]);
    }
}

/// The decoder of an Opus track with its `OpusHead`: output channels and rate, or why not.
/// `multi`: 7.1 stays 7.1.
pub(crate) fn decoder(
    t: &AudioTrack,
    head: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let refuse = |e: specular_opus::Error| {
        specular_log::debug!(target: specular_log::DECODE, "video: Opus sound: {e}");
        match e {
            specular_opus::Error::Unsupported(m) => Silence::Unsupported(m),
            _ => Silence::Unsupported("Opus".into()),
        }
    };
    let h = specular_opus::Head::parse(head).map_err(refuse)?;
    let dec = specular_opus::Decoder::new(&h).map_err(refuse)?;
    let ch = dec.channels().max(1);
    // 80 ms of packets before a seek's target (packets of 20 ms unless the track says).
    let packet_us = t
        .samples
        .first()
        .map(|s| u64::from(s.duration) * 1_000_000 / u64::from(t.timescale.max(1)))
        .filter(|&d| d > 0)
        .unwrap_or(20_000);
    let preroll = 80_000u64.div_ceil(packet_us).clamp(1, 32) as usize;
    let render = match h.family {
        2 | 3 if ch > 1 => {
            let components = (1..=15)
                .map(|n| n * n)
                .take_while(|&c| c <= ch)
                .last()
                .unwrap_or(1);
            Render::Ambisonics {
                components,
                extra: ch - components,
            }
        }
        _ if ch <= 2 => Render::Plain,
        0 | 1 if multi && ch == 8 => Render::Seven,
        0 | 1 => Render::Surround,
        _ => Render::FirstTwo,
    };
    let out_ch = match render {
        Render::Plain => ch,
        Render::Seven => 8,
        Render::Surround => 6,
        Render::Ambisonics { .. } | Render::FirstTwo => 2,
    };
    Ok((
        Box::new(Opus {
            dec,
            ch,
            render,
            preroll,
            scratch: Vec::new(),
        }),
        out_ch as u16,
        48_000,
    ))
}

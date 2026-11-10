//! Vorbis (`specular-vorbis`) and FLAC (`specular-flac`) sound for [`crate::audio`]: WebM,
//! Matroska, Ogg and MP4 (`fLaC`). More than two channels come as 5.1 in the WAVE order the
//! channel folding takes (L R C LFE Ls Rs); 7.1 as 7.1 (L R C LFE Lb Rb Ls Rs) for a device
//! that plays it.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::AudioTrack;

struct Vorbis {
    dec: specular_vorbis::Decoder,
    ch: usize,
    /// 7.1 stays 7.1.
    seven: bool,
    scratch: Vec<f32>,
}

impl SoundDecoder for Vorbis {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let r = if self.ch <= 2 {
            self.dec.decode(sample, out)
        } else {
            self.scratch.clear();
            let r = self.dec.decode(sample, &mut self.scratch);
            if self.seven {
                crate::opus_sound::vorbis_to_wave71(&self.scratch, out);
            } else {
                crate::opus_sound::vorbis_to_wave(&self.scratch, self.ch, out);
            }
            r
        };
        // A damaged packet is skipped; the next one decodes on.
        if let Err(e) = r {
            specular_log::debug!(target: specular_log::DECODE, "video: Vorbis sound: {e}");
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.dec.reset();
    }
    fn preroll(&self) -> usize {
        // the first packet only primes the overlap
        1
    }
}

/// The decoder of a Vorbis track with its three headers, Xiph-laced: output channels and
/// rate, or why not. `multi`: 7.1 stays 7.1.
pub(crate) fn vorbis(
    headers: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let refuse = |e: specular_vorbis::Error| {
        specular_log::debug!(target: specular_log::DECODE, "video: Vorbis sound: {e}");
        match e {
            specular_vorbis::Error::Unsupported(m) => Silence::Unsupported(m),
            specular_vorbis::Error::Malformed(_) => Silence::Unsupported("Vorbis".into()),
        }
    };
    let [ident, _, setup] = specular_vorbis::split_xiph_laced(headers)
        .ok_or_else(|| Silence::Unsupported("Vorbis without its headers".into()))?;
    let dec = specular_vorbis::Decoder::new(ident, setup).map_err(refuse)?;
    let ch = dec.channels().max(1);
    let rate = dec.sample_rate();
    let seven = multi && ch == 8;
    let out_ch = if seven {
        8
    } else if ch > 2 {
        6
    } else {
        ch
    };
    Ok((
        Box::new(Vorbis {
            dec,
            ch,
            seven,
            scratch: Vec::new(),
        }),
        out_ch as u16,
        rate,
    ))
}

/// Channels and rate of a Vorbis track's identification header.
pub(crate) fn vorbis_facts(headers: &[u8]) -> Option<(u16, u32)> {
    let [ident, _, _] = specular_vorbis::split_xiph_laced(headers)?;
    let i = specular_vorbis::Ident::parse(ident).ok()?;
    Some((u16::from(i.channels), i.sample_rate))
}

struct Flac {
    dec: specular_flac::Decoder,
    ch: usize,
    /// 7.1 stays 7.1.
    seven: bool,
    scratch: Vec<f32>,
}

impl SoundDecoder for Flac {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        // FLAC's 5.1 and 7.1 are already in the WAVE order.
        let r = if self.ch <= 2 || self.ch == 6 || self.seven {
            self.dec.decode(sample, out)
        } else {
            self.scratch.clear();
            let r = self.dec.decode(sample, &mut self.scratch);
            flac_to_wave(&self.scratch, self.ch, out);
            r
        };
        if let Err(e) = r {
            specular_log::debug!(target: specular_log::DECODE, "video: FLAC sound: {e}");
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.dec.reset();
    }
}

/// FLAC's channel orders (RFC 9639 §9.1.3) of 3, 4, 5, 7 and 8 channels as the six of
/// 5.1 in the WAVE order; back and side pairs fold to the surrounds, the back centre at −3 dB
/// to both.
pub(crate) fn flac_to_wave(src: &[f32], ch: usize, out: &mut Vec<f32>) {
    const M3: f32 = std::f32::consts::FRAC_1_SQRT_2;
    for f in src.chunks_exact(ch) {
        let v = match ch {
            3 => [f[0], f[1], f[2], 0.0, 0.0, 0.0],
            4 => [f[0], f[1], 0.0, 0.0, f[2], f[3]],
            5 => [f[0], f[1], f[2], 0.0, f[3], f[4]],
            7 => [f[0], f[1], f[2], f[3], f[5] + f[4] * M3, f[6] + f[4] * M3],
            _ => [f[0], f[1], f[2], f[3], f[4] + f[6], f[5] + f[7]],
        };
        out.extend_from_slice(&v);
    }
}

/// The STREAMINFO of a FLAC track's configuration, when it has one.
pub(crate) fn flac_info(config: &[u8]) -> Option<specular_flac::StreamInfo> {
    if config.is_empty() {
        return None;
    }
    specular_flac::StreamInfo::from_config(config).ok()
}

/// The decoder of a FLAC track (its configuration may be empty: frames say their format):
/// output channels and rate, or why not. `multi`: 7.1 stays 7.1.
pub(crate) fn flac(
    t: &AudioTrack,
    config: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let info = flac_info(config);
    let (ch, rate) = match info {
        Some(i) => (usize::from(i.channels), i.sample_rate),
        None => (usize::from(t.channels), t.sample_rate),
    };
    if ch == 0 || rate == 0 {
        return Err(Silence::Unsupported("FLAC".into()));
    }
    let seven = multi && ch == 8;
    let out_ch = if seven {
        8
    } else if ch > 2 {
        6
    } else {
        ch
    };
    Ok((
        Box::new(Flac {
            dec: specular_flac::Decoder::new(info),
            ch,
            seven,
            scratch: Vec::new(),
        }),
        out_ch as u16,
        rate,
    ))
}

/// Fills in a FLAC track's channels and rate from its configuration, or else from its first
/// frame.
pub(crate) fn complete_flac(data: &[u8], t: &mut AudioTrack, config: &[u8]) {
    if let Some(i) = flac_info(config) {
        t.channels = u16::from(i.channels);
        t.sample_rate = i.sample_rate;
        return;
    }
    let Some(first) = t.samples.first().and_then(|s| t.sample_data(data, s)) else {
        return;
    };
    let mut d = specular_flac::Decoder::new(None);
    let mut out = Vec::new();
    if d.decode(&first, &mut out).is_ok() {
        t.channels = d.channels() as u16;
        t.sample_rate = d.sample_rate();
    }
}

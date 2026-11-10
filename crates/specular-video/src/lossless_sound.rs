//! WavPack (`specular-wavpack`) and Monkey's Audio (`specular-ape`) sound for
//! [`crate::audio`]: one frame a sample. WavPack hybrid files play lossless when their
//! `.wvc` correction file is beside them ([`crate::mp4::SoundExtra::correction`]), lossy
//! otherwise. More than two channels become the six of 5.1 in the WAVE order, as FLAC's do.
//! A damaged frame is a frame of silence.

use std::collections::HashMap;
use std::sync::Arc;

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::AudioTrack;

/// Interleaved integers or float bits to f32, folded to at most six channels.
pub(crate) fn emit(
    pcm: &[i32],
    ch: usize,
    scale: Option<f32>,
    scratch: &mut Vec<f32>,
    out: &mut Vec<f32>,
) {
    let to = |v: i32| match scale {
        Some(s) => v as f32 * s,
        None => f32::from_bits(v as u32),
    };
    if ch <= 2 || ch == 6 {
        out.extend(pcm.iter().map(|&v| to(v)));
        return;
    }
    scratch.clear();
    if ch <= 8 {
        scratch.extend(pcm.iter().map(|&v| to(v)));
        crate::xiph_sound::flac_to_wave(scratch, ch, out);
    } else {
        for f in pcm.chunks_exact(ch) {
            out.extend(f[..6].iter().map(|&v| to(v)));
        }
    }
}

pub(crate) fn out_channels(ch: usize) -> u16 {
    if ch > 2 { 6 } else { ch as u16 }
}

struct WavPack {
    channels: usize,
    scale: Option<f32>,
    /// The correction file and its frames by first sample.
    wvc: Option<(
        Arc<specular_core::Source>,
        HashMap<u64, specular_wavpack::Frame>,
    )>,
    base: Option<u64>,
    /// DSD streams: the 1-bit to PCM filter (keeps its history between frames).
    dsd: Option<specular_wavpack::DsdToPcm>,
    pcm: Vec<i32>,
    dsd_pcm: Vec<f32>,
    scratch: Vec<f32>,
}

impl SoundDecoder for WavPack {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.pcm.clear();
        let h = specular_wavpack::Header::parse(sample);
        let cf = match (&self.wvc, h) {
            (Some((src, frames)), Some(h)) => {
                let base = self.base.unwrap_or(0);
                frames
                    .get(&h.block_index.saturating_sub(base))
                    .and_then(|f| src.bytes().get(f.offset..f.offset + f.len))
            }
            _ => None,
        };
        match specular_wavpack::decode_frame(sample, cf, self.channels, &mut self.pcm) {
            Ok(r) if r.muted > 0 => {
                specular_log::debug!(target: specular_log::DECODE, "video: WavPack: {} blocks muted", r.muted);
            }
            Ok(_) => {}
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: WavPack frame: {e}");
                let n = h.map_or(0, |h| h.block_samples as usize);
                self.pcm.clear();
                let quiet = if self.dsd.is_some() { 0x55 } else { 0 };
                self.pcm.resize(n * self.channels, quiet);
            }
        }
        if let (Some(dsd), Some(h)) = (&mut self.dsd, h) {
            self.dsd_pcm.clear();
            let index = h.block_index.saturating_sub(self.base.unwrap_or(0));
            dsd.run(index, &self.pcm, &mut self.dsd_pcm);
            self.pcm.clear();
            self.pcm
                .extend(self.dsd_pcm.iter().map(|v| v.to_bits() as i32));
        }
        emit(&self.pcm, self.channels, self.scale, &mut self.scratch, out);
        Ok(())
    }
    fn reset(&mut self) {}
}

/// Stands in when only the track is known ([`crate::audio::playable`] passes no data): the
/// stream was read when the track was made.
struct Unread;

impl SoundDecoder for Unread {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> Result<(), String> {
        Err("no data".into())
    }
    fn reset(&mut self) {}
}

pub(crate) fn wavpack(
    t: &AudioTrack,
    data: &[u8],
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    if data.is_empty() {
        return Ok((
            Box::new(Unread),
            out_channels(usize::from(t.channels)),
            t.sample_rate,
        ));
    }
    let info = specular_wavpack::info(data).map_err(|e| match e {
        specular_wavpack::Error::Unsupported(m) => Silence::Unsupported(format!("WavPack ({m})")),
        specular_wavpack::Error::Malformed(_) => Silence::Unsupported("damaged WavPack".into()),
    })?;
    // Block indexes in the correction file count from the same first block as the main one.
    let base = specular_wavpack::frames(data, 1)
        .first()
        .and_then(|f| specular_wavpack::Header::parse(&data[f.offset..]))
        .map(|h| h.block_index);
    let wvc = t.extra.correction.as_ref().map(|c| {
        let frames = specular_wavpack::frames(c.0.bytes(), usize::MAX)
            .into_iter()
            .map(|f| (f.index, f))
            .collect();
        (Arc::clone(&c.0), frames)
    });
    let ch = info.channels;
    Ok((
        Box::new(WavPack {
            channels: ch,
            scale: specular_wavpack::int_scale(&info),
            wvc,
            base,
            dsd: info
                .dsd
                .map(|d| specular_wavpack::DsdToPcm::new(d.rate / 8, ch)),
            pcm: Vec::new(),
            dsd_pcm: Vec::new(),
            scratch: Vec::new(),
        }),
        out_channels(ch),
        info.sample_rate,
    ))
}

struct Ape {
    dec: specular_ape::Decoder,
    frames: Vec<specular_ape::Frame>,
    channels: usize,
    scale: Option<f32>,
    pcm: Vec<i32>,
    scratch: Vec<f32>,
    _memory: specular_core::budget::Reservation,
}

impl SoundDecoder for Ape {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.decode_at(0, sample, out)
    }
    fn decode_at(&mut self, index: usize, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let Some(f) = self.frames.get(index).copied() else {
            return Ok(());
        };
        self.pcm.clear();
        if let Err(e) = self.dec.decode(sample, f.skip, f.blocks, &mut self.pcm) {
            specular_log::debug!(target: specular_log::DECODE, "video: Monkey's Audio frame {index}: {e}");
            self.pcm.clear();
            self.pcm.resize(f.blocks as usize * self.channels, 0);
        }
        emit(&self.pcm, self.channels, self.scale, &mut self.scratch, out);
        Ok(())
    }
    fn reset(&mut self) {}
}

pub(crate) fn ape(
    t: &AudioTrack,
    data: &[u8],
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    if data.is_empty() {
        return Ok((
            Box::new(Unread),
            out_channels(usize::from(t.channels)),
            t.sample_rate,
        ));
    }
    let unsupported = |e: specular_ape::Error| match e {
        specular_ape::Error::Unsupported(m) => {
            Silence::Unsupported(format!("Monkey's Audio ({m})"))
        }
        specular_ape::Error::Malformed(_) => Silence::Unsupported("damaged Monkey's Audio".into()),
    };
    let info = specular_ape::info(data).map_err(unsupported)?;
    let largest = info
        .frames
        .iter()
        .map(|f| u64::from(f.blocks))
        .max()
        .unwrap_or(0);
    let bytes = specular_ape::Decoder::buffer_bytes(&info) + largest * u64::from(info.channels) * 8;
    let memory = specular_core::budget::reserve(bytes)
        .map_err(|e| Silence::Unsupported(format!("Monkey's Audio ({e})")))?;
    let dec = specular_ape::Decoder::new(&info).map_err(unsupported)?;
    let ch = usize::from(info.channels);
    Ok((
        Box::new(Ape {
            dec,
            scale: specular_ape::int_scale(&info),
            frames: info.frames.clone(),
            channels: ch,
            pcm: Vec::new(),
            scratch: Vec::new(),
            _memory: memory,
        }),
        out_channels(ch),
        info.sample_rate,
    ))
}

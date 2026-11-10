//! Shorten (`specular-shorten`) for [`crate::audio`]: `.shn` files and Shorten-packed NIST
//! SPHERE files. The stream is decoded once when the track is made, keeping a resume point
//! every 4096 frames or so: each is one sample of the track (with no bytes of its own; the
//! decoder reads the stream from the point). The codec is `Other(b"ajkg", version)`.
//!
//! The sampling rate is the original WAVE or AIFF header's (kept in the stream), SPHERE's,
//! or 44.1 kHz when neither says.

use crate::audio::{Silence, SoundDecoder};
use crate::lossless_sound::{emit, out_channels};
use crate::mp4::{AudioCodec, AudioTrack, Error, Result, Sample};

pub(crate) fn is_shn(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"ajkg")
}

fn map(e: specular_shorten::Error) -> Error {
    match e {
        specular_shorten::Error::Unsupported(m) => Error::Unsupported(format!("Shorten: {m}")),
        specular_shorten::Error::Malformed(m) => Error::Malformed(format!("Shorten: {m}")),
    }
}

/// Where the Shorten stream starts: 0, or after a SPHERE header.
fn stream_offset(d: &[u8]) -> usize {
    if d.starts_with(b"NIST_1A\n") {
        let size = d
            .get(8..16)
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(|s| s.trim().parse::<usize>().ok());
        return size.unwrap_or(1024);
    }
    0
}

/// A Shorten stream at `off` as a sound track; `sphere` gives the channels and rate a SPHERE
/// header says.
pub(crate) fn track_at(d: &[u8], off: usize, sphere: Option<(u16, u32)>) -> Result<AudioTrack> {
    let s = specular_shorten::scan(d, off).map_err(map)?;
    let rate = s
        .info
        .sample_rate
        .or(sphere.map(|(_, r)| r))
        .filter(|&r| r > 0)
        .unwrap_or(44_100);
    let mut samples: Vec<Sample> = Vec::with_capacity(s.points.len());
    for (k, p) in s.points.iter().enumerate() {
        let end = s.points.get(k + 1).map_or(s.frames, |n| n.frame);
        if end > p.frame {
            samples.push(crate::sound_file::sample(off, 0, p.frame, end - p.frame));
        }
    }
    let mut tr = crate::sound_file::track(
        AudioCodec::Other(*b"ajkg", s.info.version),
        s.info.channels as u16,
        rate,
        samples,
    );
    tr.duration = s.frames;
    Ok(tr)
}

/// A `.shn` file as a sound track.
pub(crate) fn track(d: &[u8]) -> Result<AudioTrack> {
    track_at(d, 0, None)
}

struct Shn {
    data: std::sync::Arc<[u8]>,
    stream: specular_shorten::Stream,
    dec: specular_shorten::Decoder,
    channels: usize,
    scale: f32,
    centre: i32,
    pcm: Vec<i32>,
    scratch: Vec<f32>,
}

impl SoundDecoder for Shn {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> Result<(), String> {
        Err("needs the sample's index".into())
    }
    fn decode_at(&mut self, index: usize, _: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let Some(p) = self.stream.points.get(index) else {
            return Ok(());
        };
        let end = self
            .stream
            .points
            .get(index + 1)
            .map_or(self.stream.frames, |n| n.frame);
        self.pcm.clear();
        let want = end - p.frame;
        if let Err(e) = self.dec.decode_chunk(&self.data, p, want, &mut self.pcm) {
            specular_log::debug!(target: specular_log::DECODE, "video: Shorten run {index}: {e}");
        }
        self.pcm.resize(want as usize * self.channels, self.centre);
        if self.centre != 0 {
            for v in &mut self.pcm {
                *v -= self.centre;
            }
        }
        emit(
            &self.pcm,
            self.channels,
            Some(self.scale),
            &mut self.scratch,
            out,
        );
        Ok(())
    }
    fn reset(&mut self) {}
}

/// Stands in when only the track is known ([`crate::audio::playable`] passes no data).
struct Unread;

impl SoundDecoder for Unread {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> Result<(), String> {
        Err("no data".into())
    }
    fn reset(&mut self) {}
}

/// The decoder of a Shorten track.
pub(crate) fn decoder(
    t: &AudioTrack,
    data: &[u8],
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let ch = usize::from(t.channels.max(1));
    if data.is_empty() {
        return Ok((Box::new(Unread), out_channels(ch), t.sample_rate));
    }
    let stream = specular_shorten::scan(data, stream_offset(data))
        .map_err(|e| Silence::Unsupported(format!("Shorten ({e})")))?;
    let (scale, centre) = specular_shorten::scale(stream.info.sample_type);
    let ch = stream.info.channels;
    Ok((
        Box::new(Shn {
            data: std::sync::Arc::from(data),
            dec: specular_shorten::Decoder::new(&stream),
            stream,
            channels: ch,
            scale,
            centre,
            pcm: Vec::new(),
            scratch: Vec::new(),
        }),
        out_channels(ch),
        t.sample_rate,
    ))
}

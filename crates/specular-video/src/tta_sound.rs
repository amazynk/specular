//! TTA (True Audio, `specular-tta`) for [`crate::audio`]: `.tta` files, one frame a
//! sample. The codec is `Other(b"TTA1", bits)`. A damaged frame is a frame of silence.

use crate::audio::{Silence, SoundDecoder};
use crate::lossless_sound::{emit, out_channels};
use crate::mp4::{AudioCodec, AudioTrack, Error, Result, Sample};

pub(crate) fn is_tta(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"TTA1")
}

fn map(e: specular_tta::Error) -> Error {
    match e {
        specular_tta::Error::Unsupported(m) => Error::Unsupported(format!("TTA: {m}")),
        specular_tta::Error::Malformed(m) => Error::Malformed(format!("TTA: {m}")),
    }
}

/// A TTA file as a sound track.
pub(crate) fn track(d: &[u8]) -> Result<AudioTrack> {
    let info = specular_tta::info(d).map_err(map)?;
    let mut t = 0u64;
    let samples: Vec<Sample> = info
        .frames
        .iter()
        .map(|f| {
            let s = crate::sound_file::sample(f.offset, f.len, t, u64::from(f.samples));
            t += u64::from(f.samples);
            s
        })
        .collect();
    let mut tr = crate::sound_file::track(
        AudioCodec::Other(*b"TTA1", info.bits as u8),
        info.channels,
        info.sample_rate,
        samples,
    );
    tr.duration = t;
    Ok(tr)
}

struct Tta {
    dec: specular_tta::Decoder,
    /// Samples a channel of each frame.
    frames: Vec<u32>,
    channels: usize,
    scale: f32,
    pcm: Vec<i32>,
    scratch: Vec<f32>,
}

impl SoundDecoder for Tta {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> Result<(), String> {
        Err("needs the frame's index".into())
    }
    fn decode_at(&mut self, index: usize, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let n = self.frames.get(index).copied().unwrap_or(0);
        self.pcm.clear();
        if let Err(e) = self.dec.decode(sample, n, &mut self.pcm) {
            specular_log::debug!(target: specular_log::DECODE, "video: TTA frame {index}: {e}");
            if self.pcm.len() != n as usize * self.channels {
                self.pcm.clear();
                self.pcm.resize(n as usize * self.channels, 0);
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

/// The decoder of a TTA track.
pub(crate) fn decoder(
    t: &AudioTrack,
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let AudioCodec::Other(_, bits) = t.codec else {
        return Err(Silence::Unsupported("TTA".into()));
    };
    let info = specular_tta::Info {
        channels: t.channels,
        bits: u16::from(bits),
        sample_rate: t.sample_rate,
        samples: 0,
        frame_len: 0,
        frames: Vec::new(),
    };
    let dec = specular_tta::Decoder::new(&info);
    let ch = usize::from(t.channels.max(1));
    Ok((
        Box::new(Tta {
            scale: dec.scale(),
            dec,
            frames: t.samples.iter().map(|s| s.duration).collect(),
            channels: ch,
            pcm: Vec::new(),
            scratch: Vec::new(),
        }),
        out_channels(ch),
        t.sample_rate,
    ))
}

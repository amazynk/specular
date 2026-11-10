//! Tracker modules (`specular-tracker`: MOD, S3M, XM, IT) for [`crate::audio`]: rendered to
//! 48 kHz stereo by the module's own player. The song is played through once without
//! mixing when the track is made, for its length; the track's samples are runs of 4096
//! frames with no bytes of their own. The decoder keeps a copy of the player every 32 runs,
//! so a seek replays at most a few seconds silently. The codec is `Other(b"trkr", kind)`.

use std::sync::Arc;

use specular_tracker::{Kind, Module, Player};

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack, Error, Result};
use crate::sound_file::{MAX_SAMPLES, PCM_FRAMES, sample};

/// The rate modules are rendered at.
pub(crate) const RATE: u32 = 48_000;
/// Runs between two kept copies of the player.
const KEEP_EVERY: usize = 32;

pub(crate) fn is_tracker(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"trkr")
}

/// "Impulse Tracker module" and the like.
pub(crate) fn name(kind: u8) -> String {
    Kind::from_code(kind)
        .map_or("tracker module", Kind::name)
        .into()
}

fn map(e: specular_tracker::Error) -> Error {
    match e {
        specular_tracker::Error::Unsupported(m) => Error::Unsupported(m),
        specular_tracker::Error::Malformed(m) => Error::Malformed(m),
    }
}

/// A module as a sound track.
pub(crate) fn track(d: &[u8]) -> Result<AudioTrack> {
    let m = Module::load(d).map_err(map)?;
    let frames = m.length(RATE);
    if frames == 0 {
        return Err(Error::Malformed("a module that plays nothing".into()));
    }
    let mut samples = Vec::new();
    let mut t = 0;
    while t < frames && samples.len() < MAX_SAMPLES {
        let n = PCM_FRAMES.min(frames - t);
        samples.push(sample(0, 0, t, n));
        t += n;
    }
    let mut tr =
        crate::sound_file::track(AudioCodec::Other(*b"trkr", m.kind.code()), 2, RATE, samples);
    tr.duration = t;
    Ok(tr)
}

struct Tracker {
    player: Player,
    /// The run the player is at.
    at: usize,
    /// Copies of the player at runs 0, 32, 64…
    kept: Vec<Player>,
}

impl SoundDecoder for Tracker {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> std::result::Result<(), String> {
        Err("needs the run's index".into())
    }
    fn decode_at(
        &mut self,
        index: usize,
        _: &[u8],
        out: &mut Vec<f32>,
    ) -> std::result::Result<(), String> {
        if index != self.at {
            let k = (index / KEEP_EVERY).min(self.kept.len().saturating_sub(1));
            if index < self.at || k * KEEP_EVERY > self.at {
                self.player = self.kept[k].clone();
                self.at = k * KEEP_EVERY;
            }
            let skip = (index - self.at) as u64 * PCM_FRAMES;
            self.player.skip(skip);
            self.at = index;
        }
        if self.at.is_multiple_of(KEEP_EVERY) && self.at / KEEP_EVERY == self.kept.len() {
            self.kept.push(self.player.clone());
        }
        let n = self.player.render(PCM_FRAMES as usize, out);
        // A run cut by the song's end is filled to its length (the track's last run is
        // shorter and is cut by its duration).
        out.resize(out.len() + 2 * (PCM_FRAMES as usize - n), 0.0);
        self.at += 1;
        Ok(())
    }
    fn reset(&mut self) {}
}

/// The decoder of a module's track.
pub(crate) fn decoder(
    t: &AudioTrack,
    data: &[u8],
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let m = if data.is_empty() {
        None
    } else {
        Some(Module::load(data).map_err(|e| Silence::Unsupported(e.to_string()))?)
    };
    let player = match m {
        Some(m) => Player::new(Arc::new(m), RATE),
        None => {
            // Asked only whether it plays: an empty song.
            return Ok((Box::new(Silent), 2, t.sample_rate));
        }
    };
    Ok((
        Box::new(Tracker {
            kept: vec![player.clone()],
            player,
            at: 0,
        }),
        2,
        RATE,
    ))
}

struct Silent;

impl SoundDecoder for Silent {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> std::result::Result<(), String> {
        Err("no data".into())
    }
    fn reset(&mut self) {}
}

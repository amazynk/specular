//! AMR and AMR-WB sound (`specular-amr`) for [`crate::audio`]: 3GP/MP4 `samr`/`sawb`
//! tracks, Matroska `A_AMR/NB`/`A_AMR/WB` (and ffmpeg's `A_MS/ACM` tags 0x57/0x58), AVI,
//! and `.amr`/`.awb` files (one frame block a sample, multichannel files one frame per
//! channel). Samples hold storage-format frames, or an RTP octet-aligned payload.
//!
//! The track's codec is `Other(b"samr" | b"sawb", modes)`: `modes` is 0 when unknown, the
//! speech mode + 1 for one mode, and `(lowest + 1) | (highest + 1) << 4` when the stream
//! switches modes; [`crate::mp4::AudioCodec::name`] shows it ("AMR 12.2 kbit/s").

use specular_amr::{Codec, Decoder, storage};

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack};

/// Samples read to find the modes in use.
const SCAN_SAMPLES: usize = 3000;

pub(crate) fn codec(c: &AudioCodec) -> Option<Codec> {
    match c {
        AudioCodec::Other([b's', b'a', b'm', b'r'], _) => Some(Codec::Nb),
        AudioCodec::Other([b's', b'a', b'w', b'b'], _) => Some(Codec::Wb),
        _ => None,
    }
}

pub(crate) fn is_amr(c: &AudioCodec) -> bool {
    codec(c).is_some()
}

/// The `modes` byte of [`AudioCodec::Other`] from a stream's statistics.
pub(crate) fn modes_byte(st: &storage::Stats) -> u8 {
    let used: Vec<u8> = (0..9u8).filter(|&m| st.per_mode[m as usize] > 0).collect();
    match (used.first(), used.last()) {
        (Some(&lo), Some(&hi)) if lo != hi => (lo + 1) | ((hi + 1) << 4),
        (Some(&m), _) => m + 1,
        _ => 0,
    }
}

/// The display name of an AMR codec with its `modes` byte.
pub(crate) fn name(tag: &[u8; 4], modes: u8) -> String {
    let codec = if tag == b"sawb" { Codec::Wb } else { Codec::Nb };
    let rate = |m: u8| codec.bitrate(m).map(|b| format!("{}", b as f64 / 1000.0));
    let (lo, hi) = (modes & 0x0f, modes >> 4);
    let modes = match (lo, hi) {
        (0, _) => None,
        (l, 0) => rate(l - 1).map(|r| format!("{r} kbit/s")),
        (l, h) => rate(l - 1)
            .zip(rate(h - 1))
            .map(|(a, b)| format!("{a}–{b} kbit/s")),
    };
    match modes {
        Some(m) => format!("{} {m}", codec.name()),
        None => codec.name().into(),
    }
}

/// Fills in the rate, the channels (one, unless the track says more) and the modes from the
/// first samples.
pub(crate) fn complete(data: &[u8], t: &mut AudioTrack) {
    let Some(c) = codec(&t.codec) else { return };
    t.sample_rate = c.sample_rate();
    if t.channels == 0 {
        t.channels = 1;
    }
    if matches!(t.codec, AudioCodec::Other(_, m) if m != 0) {
        return; // the file's own scan (sound_file) already named the modes
    }
    let mut st = storage::Stats::default();
    if !data.is_empty() {
        for s in t.samples.iter().take(SCAN_SAMPLES) {
            if let Some(b) = t.sample_data(data, s) {
                for f in storage::packet_frames(c, &b) {
                    st.count(&f);
                }
            }
        }
    }
    let tag = if c == Codec::Nb { *b"samr" } else { *b"sawb" };
    t.codec = AudioCodec::Other(tag, modes_byte(&st));
}

struct Amr {
    decoders: Vec<Decoder>,
    /// The channel the next frame belongs to.
    next: usize,
    block: Vec<Vec<i16>>,
}

impl SoundDecoder for Amr {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let ch = self.decoders.len();
        let codec = self.decoders[0].codec();
        for f in storage::packet_frames(codec, sample) {
            let c = self.next;
            self.block[c].clear();
            self.decoders[c].decode_frame(&f, &mut self.block[c]);
            self.next = (c + 1) % ch;
            if self.next == 0 {
                let n = self.block[0].len();
                for i in 0..n {
                    for b in &self.block {
                        out.push(f32::from(b.get(i).copied().unwrap_or(0)) / 32768.0);
                    }
                }
            }
        }
        Ok(())
    }
    fn reset(&mut self) {
        for d in &mut self.decoders {
            d.reset();
        }
        self.next = 0;
    }
    fn preroll(&self) -> usize {
        // the predictors settle within a few frames
        4
    }
}

/// The decoder of an AMR track: its channels at 8 or 16 kHz.
pub(crate) fn decoder(
    t: &AudioTrack,
    _data: &[u8],
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let c = codec(&t.codec).ok_or_else(|| Silence::Unsupported(t.codec.name()))?;
    let ch = t.channels.max(1);
    let dec = Amr {
        decoders: (0..ch).map(|_| Decoder::new(c)).collect(),
        next: 0,
        block: vec![Vec::new(); usize::from(ch)],
    };
    Ok((Box::new(dec), ch, c.sample_rate()))
}

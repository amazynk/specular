//! Musepack sound (`specular-mpc`) for [`crate::audio`]: `.mpc` files, stream versions 7 and
//! 8. The track's samples are SV7 frames (the 32-bit words each touches) or SV8 audio
//! packets; the codec is `Other(b"MPC7" | b"MPC8", profile)` with SV7's profile number (or
//! SV8's, whole), which [`crate::mp4::AudioCodec::name`] shows ("Musepack SV8 (Standard)").
//! The filter's delay and SV8's leading silence are the track's edit.

use specular_mpc::{Decoder, FRAME, Packet, Stream};

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack, Edit, Error, Layout, Result, Sample};

pub(crate) fn is_mpc(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other([b'M', b'P', b'C', b'7' | b'8'], _))
}

/// The display name of a Musepack track.
pub(crate) fn name(tag: &[u8; 4], profile: u8) -> String {
    let v = if tag == b"MPC7" { 7 } else { 8 };
    match specular_mpc::profile_name(profile) {
        Some(p) => format!("Musepack SV{v} ({p})"),
        None => format!("Musepack SV{v}"),
    }
}

fn map(e: specular_mpc::Error) -> Error {
    match e {
        specular_mpc::Error::Unsupported(m) => Error::Unsupported(m),
        specular_mpc::Error::Malformed(m) => Error::Malformed(m),
    }
}

/// A Musepack file as a sound track.
pub(crate) fn track(d: &[u8]) -> Result<AudioTrack> {
    let Stream { info, packets } = specular_mpc::scan(d).map_err(map)?;
    let mut t = 0u64;
    let samples: Vec<Sample> = packets
        .iter()
        .map(|p| {
            let dur = u64::from(p.frames) * FRAME as u64;
            let s = Sample {
                offset: p.offset as u64,
                size: p.size as u32,
                dts: t,
                cts: t as i64,
                duration: dur as u32,
                sync: true,
            };
            t += dur;
            s
        })
        .collect();
    let rate = info.sample_rate;
    let tag = if info.version == 7 {
        *b"MPC7"
    } else {
        *b"MPC8"
    };
    let profile = info.profile_number.unwrap_or(0);
    Ok(AudioTrack {
        codec: AudioCodec::Other(tag, profile),
        channels: info.channels,
        sample_rate: rate,
        timescale: rate,
        duration: t,
        sample_count: samples.len() as u32,
        samples,
        edit: Edit {
            start: info.skip as i64,
            delay_us: 0,
            length_us: Some(info.samples * 1_000_000 / u64::from(rate.max(1))),
        },
        layout: Layout::Contiguous,
        extra: Default::default(),
    })
}

struct Mpc {
    dec: Decoder,
    /// The stream's packets, by sample index (SV7 frames start inside a word).
    packets: Vec<Packet>,
    next: usize,
    version: u8,
}

impl SoundDecoder for Mpc {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.decode_at(self.next, sample, out)
    }
    fn decode_at(&mut self, index: usize, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let p = self
            .packets
            .get(index)
            .ok_or("a Musepack frame past the end")?;
        self.next = index + 1;
        self.dec.decode(p, sample, out).map_err(|e| e.to_string())
    }
    fn reset(&mut self) {
        self.dec.reset(true);
    }
    fn flush(&mut self, out: &mut Vec<f32>) {
        self.dec.drain(out);
    }
    fn preroll(&self) -> usize {
        // SV7 scale factors are coded against the previous frame's: the reference decoder
        // runs 32 frames ahead after a seek. SV8 packets start with a key frame; one packet
        // fills the filter.
        if self.version == 7 { 32 } else { 1 }
    }
}

/// A Matroska `A_MPC` track (SV8; the codec private holds the stream header): its codec,
/// rate, channels and edit. Each block is one audio packet.
pub(crate) fn matroska(private: &[u8]) -> Result<(AudioCodec, u32, u16, Edit)> {
    let info = specular_mpc::sv8_header(private).map_err(map)?;
    let rate = u64::from(info.sample_rate.max(1));
    let edit = Edit {
        start: (info.skip * 1_000_000 / rate) as i64,
        delay_us: 0,
        length_us: Some(info.samples * 1_000_000 / rate),
    };
    let codec = AudioCodec::Other(*b"MPC8", info.profile_number.unwrap_or(0));
    Ok((codec, info.sample_rate, info.channels, edit))
}

/// The packets of a Matroska track's samples (each an SV8 audio packet, or its body): the
/// frames in each from the header, the last one cut to the stream's length.
fn matroska_packets(t: &AudioTrack, info: &specular_mpc::Info) -> Vec<Packet> {
    let mut left = specular_mpc::frames_needed(info);
    t.samples
        .iter()
        .map(|s| {
            let frames = u64::from(info.frames_per_packet.max(1)).min(left);
            left -= frames;
            Packet {
                offset: s.offset as usize,
                size: s.size as usize,
                frames: frames as u32,
                bit: 0,
                bits: s.size.saturating_mul(8),
            }
        })
        .collect()
}

/// The decoder of a Musepack track.
pub(crate) fn decoder(
    t: &AudioTrack,
    data: &[u8],
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    if !t.extra.head.is_empty() {
        // Matroska: the header is the codec private, the packets are the samples.
        let info = specular_mpc::sv8_header(&t.extra.head)
            .map_err(|e| Silence::Unsupported(e.to_string()))?;
        let dec = Decoder::new(&info);
        let ch = dec.channels() as u16;
        return Ok((
            Box::new(Mpc {
                dec,
                packets: matroska_packets(t, &info),
                next: 0,
                version: 8,
            }),
            ch,
            info.sample_rate,
        ));
    }
    if data.is_empty() {
        // asked only whether it plays (the track came from a successful scan)
        let v7 = matches!(t.codec, AudioCodec::Other(tag, _) if &tag == b"MPC7");
        let info = specular_mpc::Info::assumed(if v7 { 7 } else { 8 }, t.sample_rate, t.channels);
        let dec = Decoder::new(&info);
        return Ok((
            Box::new(Mpc {
                dec,
                packets: Vec::new(),
                next: 0,
                version: info.version,
            }),
            t.channels.max(1),
            t.sample_rate,
        ));
    }
    let s = specular_mpc::scan(data).map_err(|e| Silence::Unsupported(e.to_string()))?;
    let dec = Decoder::new(&s.info);
    let ch = dec.channels() as u16;
    Ok((
        Box::new(Mpc {
            dec,
            packets: s.packets,
            next: 0,
            version: s.info.version,
        }),
        ch,
        s.info.sample_rate,
    ))
}

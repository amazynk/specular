//! Specular's own Musepack decoder: stream versions 7 (`MP+`, 1998–2007) and 8 (`MPCK`,
//! 2007 on), mono and stereo, 32 to 48 kHz, gapless (SV8 and SV7's "true gapless" flag),
//! noise substitution, ReplayGain and encoder facts.
//!
//! Musepack is a subband coder: 32 subbands of 36 samples per 1152-sample frame, each band
//! at its own resolution with three scale factors, mid/side per band, and MPEG-1's
//! polyphase synthesis filter bank (shared window, see `window.rs`).
//!
//! Layout. SV7 is one stream of 32-bit little-endian words read most significant bit first:
//! the `MP+` word, a 168-bit header, then frames each led by its length in bits (20 bits),
//! not aligned to bytes. SV8 is a chain of packets (a two-letter key and a variable-length
//! size): `SH` stream header, `RG` ReplayGain, `EI` encoder, `SO`/`ST` seek table, `AP`
//! audio (2^n frames, the first a key frame) and `SE` end.
//!
//! The decoding logic is our own, written from the format's description; the prefix-code
//! tables are data copied from libmpcdec (BSD-3-Clause; `tables.rs`, PROVENANCE.md).
//! Older streams (SV4–SV6, before 1998) are refused in words.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod bits;
mod decode;
mod synth;
mod tables;
mod window;

pub use decode::FRAME;
use decode::FrameDecoder;

/// What went wrong reading a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A kind of stream this decoder does not read, in words.
    Unsupported(String),
    /// A damaged or cut stream.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Unsupported(m) | Error::Malformed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

/// Samples the synthesis filter delays the sound by (the decoder drops them).
pub const DELAY: u64 = 481;

/// A stream's facts from its header.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    /// 7 or 8.
    pub version: u8,
    pub sample_rate: u32,
    pub channels: u16,
    /// The highest subband coded (0..=31).
    pub max_band: u8,
    /// Mid/side stereo may be used.
    pub mid_side: bool,
    /// Samples per channel to play, after the decoder's delay and any leading silence.
    pub samples: u64,
    /// Decoded samples to drop at the start: the filter's delay and the encoder's silence.
    pub skip: u64,
    /// Noise substitution was enabled in the encoder.
    pub pns: bool,
    /// The encoder's quality profile ("Standard", or SV8's number when not whole).
    pub profile: Option<String>,
    /// The profile's number when it has a name (see [`profile_name`]).
    pub profile_number: Option<u8>,
    /// The encoder, e.g. "1.30.0" or "1.15r".
    pub encoder: Option<String>,
    /// ReplayGain: title and album gain in dB, title and album peak (1.0 = full scale).
    pub track_gain: Option<f32>,
    pub album_gain: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_peak: Option<f32>,
    /// SV8: frames per audio packet (a power of 2).
    pub frames_per_packet: u32,
}

impl Info {
    /// Facts for a stream known only by version, rate and channels (all subbands, mid/side
    /// allowed), e.g. to check that a decoder can be made.
    pub fn assumed(version: u8, sample_rate: u32, channels: u16) -> Info {
        Info {
            version,
            sample_rate,
            channels: channels.clamp(1, 2),
            max_band: 31,
            mid_side: true,
            samples: 0,
            skip: DELAY,
            pns: false,
            profile: None,
            profile_number: None,
            encoder: None,
            track_gain: None,
            album_gain: None,
            track_peak: None,
            album_peak: None,
            frames_per_packet: 1,
        }
    }
}

/// One unit of the stream for a player: an SV7 frame (the whole 32-bit words it touches,
/// starting `bit` bits into the first) or an SV8 audio packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    pub offset: usize,
    pub size: usize,
    /// Frames of 1152 samples in it.
    pub frames: u32,
    /// SV7: where the frame starts in its first word (0..32), its length in bits.
    pub bit: u8,
    pub bits: u32,
}

/// A stream's header and packets.
#[derive(Debug, Clone)]
pub struct Stream {
    pub info: Info,
    pub packets: Vec<Packet>,
}

/// The bytes after ID3v2 tags at the start (some taggers add them).
fn skip_id3v2(d: &[u8]) -> usize {
    let mut at = 0;
    while let Some(h) = d.get(at..at + 10) {
        if &h[..3] != b"ID3" || h[3] == 0xFF || h[6..10].iter().any(|b| b & 0x80 != 0) {
            break;
        }
        let size = h[6..10]
            .iter()
            .fold(0usize, |a, &b| (a << 7) | usize::from(b));
        at += 10 + size + if h[5] & 0x10 != 0 { 10 } else { 0 };
    }
    at.min(d.len())
}

/// Whether `d` starts as a Musepack stream (SV7 or SV8; also an SV7 header of another
/// version, so it can be refused in words).
pub fn is_musepack(d: &[u8]) -> bool {
    let s = &d[skip_id3v2(d)..];
    s.starts_with(b"MPCK") || s.starts_with(b"MP+")
}

const RATES: [u32; 4] = [44100, 48000, 37800, 32000];

/// The name of profile number `p` (SV7's numbers; SV8 uses the same scale).
pub fn profile_name(p: u8) -> Option<&'static str> {
    sv7_profile(u32::from(p))
}

fn sv7_profile(p: u32) -> Option<&'static str> {
    Some(match p {
        1 => "Unstable/Experimental",
        5 => "quality 0",
        6 => "quality 1",
        7 => "Telephone",
        8 => "Thumb",
        9 => "Radio",
        10 => "Standard",
        11 => "Extreme",
        12 => "Insane",
        13 => "BrainDead",
        14 => "quality 9",
        15 => "quality 10",
        _ => return None,
    })
}

/// SV8's profile number (in eighths), named as SV7's when whole.
fn sv8_profile(q: f32) -> String {
    match sv7_profile(q as u32) {
        Some(n) if q.fract() == 0.0 => n.into(),
        _ => format!("profile {q:.1}"),
    }
}

/// The word-swapped SV7 stream from `base`: 32-bit little-endian words become big-endian
/// so it reads front to back (a last partial word is padded with zeros).
fn swap_words(d: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(d.len().div_ceil(4) * 4);
    for w in d.chunks(4) {
        let mut b = [0u8; 4];
        b[..w.len()].copy_from_slice(w);
        v.extend(b.iter().rev());
    }
    v
}

fn sv7(d: &[u8], base: usize) -> Result<Stream, Error> {
    let version = d[base + 3] & 15;
    if version != 7 {
        return Err(Error::Unsupported(format!(
            "Musepack stream version {version} (only versions 7 and 8 are read)"
        )));
    }
    let head = swap_words(
        d.get(base..base + 32)
            .ok_or_else(|| bad("Musepack header cut short"))?,
    );
    let mut r = bits::Bits::new(&head, 32);
    let frames = r.read(32) as u64;
    let intensity = r.bit();
    let mid_side = r.bit();
    let max_band = r.read(6) as u8;
    let profile = r.read(4);
    let _link = r.read(2);
    let sample_rate = RATES[r.read(2) as usize];
    let _peak = r.read(16);
    let gains: Vec<u32> = (0..4).map(|_| r.read(16)).collect();
    let gapless = r.bit();
    let last = r.read(11) as u64;
    let _fast_seek = r.bit();
    let _ = r.read(19);
    let enc = r.read(8);
    if intensity {
        return Err(Error::Unsupported(
            "Musepack with intensity stereo (never used by released encoders)".into(),
        ));
    }
    if max_band >= 32 {
        return Err(bad("Musepack header with more than 32 subbands"));
    }
    if last > FRAME as u64 {
        return Err(bad("Musepack header with a last frame longer than a frame"));
    }
    let last = if last == 0 { FRAME as u64 } else { last };
    let total = frames * FRAME as u64;
    let samples = if gapless {
        total - (FRAME as u64 - last)
    } else {
        total.saturating_sub(DELAY)
    };
    let encoder = match enc {
        0 => None,
        v if v % 10 == 0 => Some(format!("{}.{}", v / 100, v / 10 % 10)),
        v if v % 2 == 0 => Some(format!("{}.{:02} beta", v / 100, v % 100)),
        v => Some(format!("{}.{:02} alpha", v / 100, v % 100)),
    };
    // SV7 gains are in 1/100 dB (as signed 16-bit), peaks in sample units.
    let gain = |g: u32| (g != 0).then(|| f32::from(g as u16 as i16) / 100.0);
    let peak = |p: u32| (p != 0).then(|| p as f32 / 32768.0);
    let info = Info {
        version: 7,
        sample_rate,
        channels: 2,
        max_band,
        mid_side,
        samples,
        skip: DELAY,
        pns: d[base + 3] >> 4 != 0,
        profile: sv7_profile(profile).map(String::from),
        profile_number: sv7_profile(profile).map(|_| profile as u8),
        encoder,
        track_gain: gain(gains[0]),
        track_peak: peak(gains[1]),
        album_gain: gain(gains[2]),
        album_peak: peak(gains[3]),
        frames_per_packet: 1,
    };
    // Frames: a 20-bit length, then that many bits (the last frame has 11 more: its
    // number of samples, repeated from the header).
    let body = &d[base..];
    let word = |p: usize| -> u32 {
        let i = (p / 32) * 4;
        let mut w = [0u8; 4];
        for (k, b) in w.iter_mut().enumerate() {
            *b = body.get(i + k).copied().unwrap_or(0);
        }
        u32::from_le_bytes(w)
    };
    let read20 = |p: usize| -> u32 {
        let hi = u64::from(word(p)) << 32 | u64::from(word(p + 32));
        ((hi >> (64 - 20 - (p % 32))) & 0xF_FFFF) as u32
    };
    let mut packets = Vec::with_capacity(frames.min(1 << 22) as usize);
    let mut pos = 200usize;
    let len_bits = body.len() * 8;
    for k in 0..frames {
        if pos + 20 > len_bits {
            break;
        }
        let mut n = read20(pos) as usize;
        if k + 1 == frames {
            n += 11;
        }
        let start = pos + 20;
        let end = start + n;
        if end > len_bits + 32 {
            break; // cut short: play what is there
        }
        let first = start / 32 * 4;
        let stop = (end.div_ceil(32) * 4).min(body.len());
        packets.push(Packet {
            offset: base + first,
            size: stop.saturating_sub(first),
            frames: 1,
            bit: (start % 32) as u8,
            bits: n as u32,
        });
        pos = end;
    }
    if packets.is_empty() {
        return Err(bad("Musepack stream without frames"));
    }
    Ok(Stream { info, packets })
}

/// A variable-length SV8 size: 7 bits a byte, high bit set on all but the last.
fn varint(d: &[u8], at: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for _ in 0..9 {
        let b = *d.get(*at)?;
        *at += 1;
        v = (v << 7) | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn sv8(d: &[u8], base: usize, audio: bool) -> Result<Stream, Error> {
    let mut at = base + 4;
    let mut info: Option<Info> = None;
    let mut packets = Vec::new();
    let mut total_frames = 0u64;
    let mut frames_needed = u64::MAX;
    while at + 3 <= d.len() {
        let key = [d[at], d[at + 1]];
        if !key.iter().all(u8::is_ascii_uppercase) {
            break; // a tag or junk after the stream
        }
        let mut p = at + 2;
        let size = varint(d, &mut p).ok_or_else(|| bad("Musepack packet size cut short"))?;
        let head = p - at;
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        if size < head {
            return Err(bad("Musepack packet shorter than its header"));
        }
        let end = at.saturating_add(size);
        let body = &d[p..end.min(d.len())];
        match &key {
            b"SH" => {
                if body.len() < 5 {
                    return Err(bad("Musepack stream header cut short"));
                }
                let crc = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
                if crc != crc32(&body[4..]) {
                    return Err(bad("Musepack stream header damaged (checksum)"));
                }
                let version = body[4];
                if version != 8 {
                    return Err(Error::Unsupported(format!(
                        "Musepack stream version {version} (only versions 7 and 8 are read)"
                    )));
                }
                let mut q = 5;
                let samples = varint(body, &mut q).ok_or_else(|| bad("Musepack header cut"))?;
                let silence = varint(body, &mut q).ok_or_else(|| bad("Musepack header cut"))?;
                let mut r = bits::Bits::new(&body[q..], 0);
                let rate = r.read(3) as usize;
                let max_band = r.read(5) as u8 + 1;
                let channels = r.read(4) as u16 + 1;
                let mid_side = r.bit();
                let pwr = r.read(3) * 2;
                if r.overrun() {
                    return Err(bad("Musepack stream header cut short"));
                }
                let sample_rate = *RATES.get(rate).ok_or_else(|| {
                    Error::Unsupported(format!("Musepack sampling rate code {rate}"))
                })?;
                if channels > 2 {
                    return Err(Error::Unsupported(format!(
                        "Musepack with {channels} channels (only mono and stereo are defined)"
                    )));
                }
                if max_band >= 32 {
                    return Err(bad("Musepack header with more than 32 subbands"));
                }
                frames_needed = (samples + DELAY).div_ceil(FRAME as u64);
                info = Some(Info {
                    version: 8,
                    sample_rate,
                    channels,
                    max_band,
                    mid_side,
                    samples: samples.saturating_sub(silence),
                    skip: DELAY + silence,
                    pns: false,
                    profile: None,
                    profile_number: None,
                    encoder: None,
                    track_gain: None,
                    album_gain: None,
                    track_peak: None,
                    album_peak: None,
                    frames_per_packet: 1 << pwr,
                });
            }
            b"RG" => {
                if let (Some(i), [1, rest @ ..]) = (info.as_mut(), body)
                    && rest.len() >= 8
                {
                    let w = |k: usize| u16::from_be_bytes([rest[k], rest[k + 1]]);
                    // gains in 1/256 dB relative to 64.82 dB; peaks as 20·log10 in 1/256 dB
                    let gain = |g: u16| (g != 0).then(|| 64.82 - f32::from(g) / 256.0);
                    let peak = |p: u16| {
                        (p != 0).then(|| 10f32.powf(f32::from(p) / 256.0 / 20.0) / 32768.0)
                    };
                    i.track_gain = gain(w(0));
                    i.track_peak = peak(w(2));
                    i.album_gain = gain(w(4));
                    i.album_peak = peak(w(6));
                }
            }
            b"EI" => {
                if let Some(i) = info.as_mut()
                    && body.len() >= 4
                {
                    let q = f32::from(body[0] >> 1) / 8.0;
                    i.pns = body[0] & 1 != 0;
                    i.profile = Some(sv8_profile(q));
                    i.profile_number =
                        (q.fract() == 0.0 && sv7_profile(q as u32).is_some()).then_some(q as u8);
                    i.encoder = Some(format!("{}.{}.{}", body[1], body[2], body[3]));
                }
            }
            b"AP" => {
                let Some(i) = info.as_ref() else {
                    return Err(bad("Musepack audio before the stream header"));
                };
                let frames =
                    u64::from(i.frames_per_packet).min(frames_needed.saturating_sub(total_frames));
                if frames > 0 {
                    packets.push(Packet {
                        offset: p,
                        size: body.len(),
                        frames: frames as u32,
                        bit: 0,
                        bits: (body.len() * 8) as u32,
                    });
                    total_frames += frames;
                }
            }
            b"SE" => break,
            _ => {}
        }
        if end > d.len() {
            break; // cut short
        }
        at = end;
    }
    let info = info.ok_or_else(|| bad("Musepack stream without a stream header"))?;
    if packets.is_empty() && audio {
        return Err(bad("Musepack stream without audio"));
    }
    Ok(Stream { info, packets })
}

/// The facts of an SV8 stream whose header is kept apart from its audio (Matroska's `A_MPC`
/// codec private data): the stream header packet and those after it (replay gain, encoder),
/// with or without the `MPCK` magic before them.
pub fn sv8_header(private: &[u8]) -> Result<Info, Error> {
    let mut d = Vec::with_capacity(private.len() + 4);
    if !private.starts_with(b"MPCK") {
        d.extend_from_slice(b"MPCK");
    }
    d.extend_from_slice(private);
    sv8(&d, 0, false).map(|s| s.info)
}

/// Where the coded audio is in an SV8 audio packet stored on its own (a Matroska block): the
/// body of a whole `AP` packet, else the block as it is.
pub fn sv8_audio_body(block: &[u8]) -> std::ops::Range<usize> {
    if block.starts_with(b"AP") {
        let mut p = 2;
        if let Some(size) = varint(block, &mut p)
            && usize::try_from(size).is_ok_and(|s| s == block.len())
        {
            return p..block.len();
        }
    }
    0..block.len()
}

/// Frames of 1152 samples coded in a stream: its samples, delay and leading silence.
pub fn frames_needed(info: &Info) -> u64 {
    (info.samples + info.skip).div_ceil(FRAME as u64)
}

/// Reads a stream's header and finds its frames (SV7) or audio packets (SV8).
pub fn scan(d: &[u8]) -> Result<Stream, Error> {
    let base = skip_id3v2(d);
    let s = &d[base..];
    if s.starts_with(b"MPCK") {
        sv8(d, base, true)
    } else if s.starts_with(b"MP+") && s.len() >= 4 {
        sv7(d, base)
    } else {
        Err(Error::Unsupported(
            "not a Musepack SV7 or SV8 stream (older SV4–SV6 files are not read)".into(),
        ))
    }
}

/// Decodes a stream's packets into interleaved PCM in [-1, 1].
#[derive(Clone)]
pub struct Decoder {
    version: u8,
    channels: usize,
    frames: FrameDecoder,
}

impl Decoder {
    pub fn new(info: &Info) -> Decoder {
        Decoder {
            version: info.version,
            channels: usize::from(info.channels.clamp(1, 2)),
            frames: FrameDecoder::new(
                info.version,
                usize::from(info.channels),
                usize::from(info.max_band),
                info.mid_side,
            ),
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Decodes packet `p`, whose bytes are `bytes` (`p.size` of them from `p.offset`),
    /// appending `p.frames` × 1152 samples per channel to `out` (before the delay is
    /// dropped). A damaged frame gives silence for the rest of the packet.
    pub fn decode(&mut self, p: &Packet, bytes: &[u8], out: &mut Vec<f32>) -> Result<(), Error> {
        let want = out.len() + p.frames as usize * FRAME * self.channels;
        let res = if self.version == 7 {
            let words = swap_words(bytes);
            let mut r = bits::Bits::new(&words, usize::from(p.bit));
            self.frames.read_sv7(&mut r).and_then(|()| {
                if r.overrun() || r.pos() > usize::from(p.bit) + p.bits as usize {
                    Err(bad("Musepack frame longer than its length"))
                } else {
                    Ok(())
                }
            })
        } else {
            let mut r = bits::Bits::new(bytes, 0);
            let mut res = Ok(());
            for f in 0..p.frames {
                res = self.frames.read_sv8(&mut r, f == 0);
                if res.is_ok() && r.overrun() {
                    res = Err(bad("Musepack packet cut short"));
                }
                if res.is_err() {
                    break;
                }
                self.frames.synthesise(out);
            }
            res
        };
        match res {
            Ok(()) => {
                if self.version == 7 {
                    self.frames.synthesise(out);
                }
                Ok(())
            }
            Err(e) => {
                out.resize(want, 0.0);
                self.frames.reset(true);
                Err(e)
            }
        }
    }

    /// The filter's last samples after the final packet.
    pub fn drain(&mut self, out: &mut Vec<f32>) {
        self.frames.drain(out);
    }

    /// Forgets the state before a seek; `mid` when not at the start.
    pub fn reset(&mut self, mid: bool) {
        self.frames.reset(mid && self.version == 7);
        self.frames.reset_filters();
    }
}

/// Decodes a whole stream: interleaved PCM in [-1, 1] with the delay and leading silence
/// dropped and the length trimmed to the stream's.
pub fn decode_all(d: &[u8]) -> Result<(Info, Vec<f32>), Error> {
    let s = scan(d)?;
    let mut dec = Decoder::new(&s.info);
    let ch = dec.channels();
    let mut out = Vec::new();
    for p in &s.packets {
        let bytes = d
            .get(p.offset..p.offset + p.size)
            .ok_or_else(|| bad("Musepack packet past the end"))?;
        let _ = dec.decode(p, bytes, &mut out);
    }
    let need = (s.info.skip + s.info.samples) as usize * ch;
    if out.len() < need {
        dec.drain(&mut out);
    }
    let skip = (s.info.skip as usize * ch).min(out.len());
    out.drain(..skip);
    out.truncate(s.info.samples as usize * ch);
    Ok((s.info, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_is_zlibs() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn refuses_other_streams_in_words() {
        let e = scan(b"RIFF....WAVE").unwrap_err();
        assert!(e.to_string().contains("SV4"), "{e}");
        let mut sv6 = b"MP+\x06".to_vec();
        sv6.resize(64, 0);
        let e = scan(&sv6).unwrap_err();
        assert!(matches!(e, Error::Unsupported(_)), "{e}");
        assert!(e.to_string().contains("version 6"), "{e}");
    }

    #[test]
    fn varints_chain_seven_bits() {
        let mut at = 0;
        assert_eq!(varint(&[0x81, 0x00], &mut at), Some(128));
        assert_eq!(at, 2);
    }
}

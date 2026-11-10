//! More plain sound files, read as a movie with only a sound track: Sun/NeXT AU (`.au`,
//! `.snd`), Apple Core Audio Format (`.caf`: PCM, G.711, IMA ADPCM, ALAC, AAC and MPEG audio
//! packets), Sony Wave64 (`.w64`), Creative Voice (`.voc`), Amiga 8SVX (`.8svx`, also
//! Fibonacci-delta packed and stereo), Audio Visual Research (`.avr`), NIST SPHERE (`.sph`,
//! also Shorten-packed) and Berkeley/IRCAM/CARL (`.sf`).
//!
//! Each format's own layout, from its published description; the samples are the
//! container's runs of 4096 frames (PCM) or its packets (CAF), timed at the sampling rate.

use std::sync::Arc;

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AdpcmKind, AudioCodec, AudioTrack, Edit, Error, Result, Sample};
use crate::sound_file::{MAX_SAMPLES, PCM_FRAMES, pcm_samples, plain_track, sample, track};

/// Which of these files `d` is, by its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Au,
    Caf,
    W64,
    Voc,
    Svx,
    Avr,
    Sphere,
    Ircam,
}

impl Kind {
    /// The movie's brand for this kind (what [`crate::mp4::Movie::brand`] says).
    pub fn brand(self) -> [u8; 4] {
        match self {
            Kind::Au => *b".snd",
            Kind::Caf => *b"caff",
            Kind::W64 => *b"W64 ",
            Kind::Voc => *b"VOC ",
            Kind::Svx => *b"8SVX",
            Kind::Avr => *b"2BIT",
            Kind::Sphere => *b"NIST",
            Kind::Ircam => *b"SF  ",
        }
    }
}

/// The GUID of Wave64's outer chunk ("riff" and a fixed tail).
const W64_RIFF: [u8; 16] = [
    0x72, 0x69, 0x66, 0x66, 0x2E, 0x91, 0xCF, 0x11, 0xA5, 0xD6, 0x28, 0xDB, 0x04, 0xC1, 0x00, 0x00,
];
/// The tail every other Wave64 chunk GUID shares after its four-character name.
const W64_TAIL: [u8; 12] = [
    0xF3, 0xAC, 0xD3, 0x11, 0x8C, 0xD1, 0x00, 0xC0, 0x4F, 0x8E, 0xDB, 0x8A,
];
const VOC_MAGIC: &[u8; 20] = b"Creative Voice File\x1A";

fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn be64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(at..at + 8)?.try_into().ok()?))
}
fn le16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn le64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

fn pcm(bits: u16, float: bool, little_endian: bool) -> AudioCodec {
    AudioCodec::Pcm {
        bits,
        float,
        little_endian,
    }
}

/// IRCAM's magic numbers: the byte order they are read in is the file's.
fn ircam_order(d: &[u8]) -> Option<bool> {
    match (le32(d, 0)?, be32(d, 0)?) {
        (0x0001_A364 | 0x0003_A364, _) => Some(true),
        (_, 0x0002_A364 | 0x0004_A364) => Some(false),
        _ => None,
    }
}

/// The kind of sound file `d` is, if it is one of these.
pub fn sniff(d: &[u8]) -> Option<Kind> {
    if d.len() >= 24 && (d.starts_with(b".snd") || d.starts_with(b"dns.")) {
        return Some(Kind::Au);
    }
    if d.len() >= 16 && d.starts_with(b"caff") && be16(d, 4) == Some(1) {
        return Some(Kind::Caf);
    }
    if d.len() >= 40 && d[..16] == W64_RIFF && &d[24..28] == b"wave" && d[28..40] == W64_TAIL {
        return Some(Kind::W64);
    }
    if d.len() >= 26 && d.starts_with(VOC_MAGIC) {
        return Some(Kind::Voc);
    }
    if d.len() >= 12 && d.starts_with(b"FORM") && matches!(&d[8..12], b"8SVX" | b"16SV") {
        return Some(Kind::Svx);
    }
    if d.len() >= 128 && d.starts_with(b"2BIT") {
        return Some(Kind::Avr);
    }
    if d.len() >= 16 && d.starts_with(b"NIST_1A\n") {
        return Some(Kind::Sphere);
    }
    if d.len() >= 1024 && ircam_order(d).is_some() {
        return Some(Kind::Ircam);
    }
    None
}

/// The sound track of a file of kind `k`.
pub fn track_of(k: Kind, d: &[u8]) -> Result<AudioTrack> {
    match k {
        Kind::Au => au(d),
        Kind::Caf => caf(d),
        Kind::W64 => w64(d),
        Kind::Voc => voc(d),
        Kind::Svx => svx(d),
        Kind::Avr => avr(d),
        Kind::Sphere => sphere(d),
        Kind::Ircam => ircam(d),
    }
}

/// Sun/NeXT AU: a big-endian header (".snd"; DEC's "dns." is the same little-endian), then
/// the samples to the end or the size given.
fn au(d: &[u8]) -> Result<AudioTrack> {
    let le = d.starts_with(b"dns.");
    let r = |at| if le { le32(d, at) } else { be32(d, at) };
    let off = r(4).unwrap_or(24) as usize;
    let size = r(8).unwrap_or(u32::MAX);
    let enc = r(12).unwrap_or(0);
    let rate = r(16).unwrap_or(0);
    let channels = r(20).unwrap_or(0);
    if off < 24 || off > d.len() {
        return Err(bad("AU header runs past the end"));
    }
    let channels = u16::try_from(channels).map_err(|_| bad("AU channel count"))?;
    let len = if size == u32::MAX {
        d.len() - off
    } else {
        (size as usize).min(d.len() - off)
    };
    let (codec, bits) = match enc {
        1 => (AudioCodec::G711 { alaw: false }, 8),
        27 => (AudioCodec::G711 { alaw: true }, 8),
        // 8-bit samples are signed: "big-endian" 8-bit PCM in the player's terms.
        2 => (pcm(8, false, false), 8),
        3 => (pcm(16, false, le), 16),
        4 => (pcm(24, false, le), 24),
        5 => (pcm(32, false, le), 32),
        6 => (pcm(32, true, le), 32),
        7 => (pcm(64, true, le), 64),
        23 => return Err(Error::Unsupported("AU G.721 ADPCM sound".into())),
        24 => return Err(Error::Unsupported("AU G.722 sound".into())),
        25 | 26 => return Err(Error::Unsupported("AU G.723 ADPCM sound".into())),
        n => return Err(Error::Unsupported(format!("AU encoding {n}"))),
    };
    plain_track(codec, channels, rate, bits, off, len)
}

/// Sony Wave64: RIFF WAVE with 16-byte GUIDs and 64-bit sizes, chunks aligned to 8 bytes.
fn w64(d: &[u8]) -> Result<AudioTrack> {
    let end = (le64(d, 16).unwrap_or(0) as usize).clamp(40, d.len());
    let mut at = 40;
    let (mut fmt, mut data) = (None, None);
    while at + 24 <= end {
        let id = &d[at..at + 16];
        let size = le64(d, at + 16).unwrap_or(0) as usize;
        if size < 24 {
            break;
        }
        let body = at + 24;
        let len = (size - 24).min(end - body);
        if id[4..] == W64_TAIL {
            match &id[..4] {
                b"fmt " => fmt = Some(&d[body..body + len]),
                b"data" if data.is_none() => data = Some((body, len)),
                _ => {}
            }
        }
        at = at.saturating_add(size.div_ceil(8) * 8);
    }
    let f = fmt.ok_or_else(|| bad("Wave64 without a format chunk"))?;
    let (off, len) = data.ok_or_else(|| bad("Wave64 without a data chunk"))?;
    crate::sound_file::wave_format_track(d, f, off, len)
}

/// A packet table's variable-length integer (seven bits a byte, high bits first).
fn varint(d: &[u8], at: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for _ in 0..10 {
        let b = *d.get(*at)?;
        *at += 1;
        v = (v << 7) | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// Apple Core Audio Format: big-endian chunks with 64-bit sizes; `desc` says the coding,
/// `pakt` lists the sizes of variable packets, `kuki` holds a codec's configuration.
fn caf(d: &[u8]) -> Result<AudioTrack> {
    let mut at = 8;
    let (mut desc, mut data, mut pakt, mut kuki) = (None, None, None, None);
    while at + 12 <= d.len() {
        let id = &d[at..at + 4];
        let size = be64(d, at + 4).unwrap_or(0) as i64;
        let body = at + 12;
        let len = if size < 0 {
            d.len() - body
        } else {
            (size as u64).min((d.len() - body) as u64) as usize
        };
        match id {
            b"desc" => desc = Some(&d[body..body + len]),
            b"data" if len >= 4 => data = Some((body + 4, len - 4)),
            b"pakt" => pakt = Some(&d[body..body + len]),
            b"kuki" => kuki = Some(&d[body..body + len]),
            _ => {}
        }
        at = body + len;
    }
    let c = desc.ok_or_else(|| bad("CAF without a description"))?;
    let (off, len) = data.ok_or_else(|| bad("CAF without sound data"))?;
    let rate = f64::from_bits(be64(c, 0).ok_or_else(|| bad("CAF description cut short"))?);
    let rate = if rate.is_finite() && rate > 0.0 && rate < 1e7 {
        rate.round() as u32
    } else {
        return Err(bad("CAF sampling rate"));
    };
    let id: [u8; 4] = c[8..12].try_into().map_err(|_| bad("CAF format"))?;
    let flags = be32(c, 12).unwrap_or(0);
    let bytes_per_packet = be32(c, 16).unwrap_or(0);
    let frames_per_packet = be32(c, 20).unwrap_or(0);
    let channels = u16::try_from(be32(c, 24).unwrap_or(0)).map_err(|_| bad("CAF channels"))?;
    let bits = be32(c, 28).unwrap_or(0) as u16;
    let plain = |codec: AudioCodec, bits: u16| plain_track(codec, channels, rate, bits, off, len);
    match &id {
        b"lpcm" => {
            let float = flags & 1 != 0;
            let le = flags & 2 != 0;
            // The container's bytes a sample (a 20-bit value can sit in three bytes).
            let size = match bytes_per_packet {
                0 => bits.div_ceil(8) * 8,
                n => (n / u32::from(channels.max(1)) * 8) as u16,
            };
            if !matches!((size, float), (8 | 16 | 24 | 32, false) | (32 | 64, true)) {
                return Err(Error::Unsupported(format!("CAF {size}-bit PCM")));
            }
            // 8-bit linear PCM in CAF is signed.
            plain(pcm(size, float, le && size > 8), size)
        }
        b"ulaw" => plain(AudioCodec::G711 { alaw: false }, 8),
        b"alaw" => plain(AudioCodec::G711 { alaw: true }, 8),
        b"ima4" => plain(
            AudioCodec::Adpcm {
                kind: AdpcmKind::ImaQt,
                block: 34 * channels,
            },
            4,
        ),
        b"alac" | b"aac " | b".mp1" | b".mp2" | b".mp3" => {
            let codec = match &id {
                b"alac" => {
                    let k = kuki.ok_or_else(|| bad("ALAC in CAF without its configuration"))?;
                    // Apple's cookie may lead with a `frma` atom before the `alac` one.
                    let k = if k.get(4..8) == Some(b"frma") {
                        &k[12.min(k.len())..]
                    } else {
                        k
                    };
                    AudioCodec::Alac(k.to_vec())
                }
                b"aac " => {
                    let k = kuki.ok_or_else(|| bad("AAC in CAF without its configuration"))?;
                    let mut esds = vec![0u8; 4];
                    esds.extend_from_slice(k);
                    let (_, asc) = crate::mp4::esds_config(&esds)
                        .ok_or_else(|| bad("AAC in CAF: no decoder configuration"))?;
                    AudioCodec::Aac(asc)
                }
                b".mp1" => AudioCodec::Mpa(1),
                b".mp2" => AudioCodec::Mpa(2),
                _ => AudioCodec::Mpa(3),
            };
            packets(
                codec,
                channels,
                rate,
                (off, len),
                pakt,
                bytes_per_packet,
                frames_per_packet,
            )
        }
        other => Err(Error::Unsupported(format!(
            "CAF '{}' sound",
            String::from_utf8_lossy(other).trim()
        ))),
    }
}

/// A CAF track of coded packets: their sizes and frames from the packet table (or fixed),
/// the priming and remainder frames the edit.
fn packets(
    codec: AudioCodec,
    channels: u16,
    rate: u32,
    (off, len): (usize, usize),
    pakt: Option<&[u8]>,
    bytes_per_packet: u32,
    frames_per_packet: u32,
) -> Result<AudioTrack> {
    let mut samples = Vec::new();
    let mut edit = Edit::default();
    let (mut at, mut t) = (0usize, 0u64);
    match pakt {
        Some(p) if p.len() >= 24 => {
            let n = be64(p, 0).unwrap_or(0);
            let valid = be64(p, 8).unwrap_or(0) as i64;
            let priming = be32(p, 16).unwrap_or(0) as i32;
            let mut i = 24;
            for _ in 0..n.min(MAX_SAMPLES as u64) {
                let size = match bytes_per_packet {
                    0 => varint(p, &mut i).ok_or_else(|| bad("CAF packet table cut short"))?,
                    b => u64::from(b),
                } as usize;
                let frames = match frames_per_packet {
                    0 => varint(p, &mut i).ok_or_else(|| bad("CAF packet table cut short"))?,
                    f => u64::from(f),
                };
                if at + size > len {
                    break;
                }
                samples.push(sample(off + at, size, t, frames));
                at += size;
                t += frames;
            }
            if priming > 0 {
                edit.start = i64::from(priming);
            }
            if valid > 0 && rate > 0 {
                edit.length_us = Some(valid as u64 * 1_000_000 / u64::from(rate));
            }
        }
        _ if bytes_per_packet > 0 && frames_per_packet > 0 => {
            let size = bytes_per_packet as usize;
            while at + size <= len && samples.len() < MAX_SAMPLES {
                samples.push(sample(off + at, size, t, u64::from(frames_per_packet)));
                at += size;
                t += u64::from(frames_per_packet);
            }
        }
        _ => return Err(bad("CAF packets without a packet table")),
    }
    let mut tr = track(codec, channels, rate, samples);
    tr.edit = edit;
    Ok(tr)
}

/// One run of sound in a Creative Voice file.
#[derive(Clone, Copy)]
struct VocRun {
    off: usize,
    len: usize,
    codec: u16,
    rate: u32,
    channels: u16,
    bits: u16,
}

/// Creative Voice: a header, then typed blocks (24-bit sizes). Sound data (types 1 and 9)
/// and its continuations (type 2) are played in order; an "extended" block (type 8) sets the
/// rate and channels of the type-1 block after it. The runs must share one coding; silence
/// blocks (type 3) are left out.
fn voc(d: &[u8]) -> Result<AudioTrack> {
    let mut at = usize::from(le16(d, 20).unwrap_or(26)).max(26);
    let mut runs: Vec<VocRun> = Vec::new();
    let mut extended: Option<(u32, u16)> = None;
    while at < d.len() {
        let kind = d[at];
        if kind == 0 {
            break;
        }
        let size = d
            .get(at + 1..at + 4)
            .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], 0]) as usize);
        let body = at + 4;
        let len = size.min(d.len().saturating_sub(body));
        let b = &d[body.min(d.len())..body + len];
        match kind {
            1 if len >= 2 => {
                let (rate, channels) = extended
                    .take()
                    .unwrap_or_else(|| (1_000_000 / (256 - u32::from(b[0])).max(1), 1));
                runs.push(VocRun {
                    off: body + 2,
                    len: len - 2,
                    codec: u16::from(b[1]),
                    rate,
                    channels,
                    bits: if b[1] == 4 { 16 } else { 8 },
                });
            }
            2 => {
                if let Some(r) = runs.last() {
                    let run = VocRun {
                        off: body,
                        len,
                        ..*r
                    };
                    runs.push(run);
                }
            }
            8 if len >= 4 => {
                let tc = u32::from(le16(b, 0).unwrap_or(0));
                let channels = u16::from(b[3]) + 1;
                let rate = 256_000_000 / (u32::from(channels) * (65_536 - tc).max(1));
                extended = Some((rate, channels));
            }
            9 if len >= 12 => {
                runs.push(VocRun {
                    off: body + 12,
                    len: len - 12,
                    codec: le16(b, 6).unwrap_or(0),
                    rate: le32(b, 0).unwrap_or(0),
                    channels: u16::from(b[5]),
                    bits: u16::from(b[4]),
                });
            }
            _ => {}
        }
        at = body + len;
    }
    let first = runs
        .first()
        .ok_or_else(|| bad("Creative Voice file without sound"))?;
    let (codec, bits) = match (first.codec, first.bits) {
        (0, 8) => (pcm(8, false, true), 8),
        (4, 16) => (pcm(16, false, true), 16),
        (6, _) => (AudioCodec::G711 { alaw: true }, 8),
        (7, _) => (AudioCodec::G711 { alaw: false }, 8),
        (1..=3 | 0x200, _) => {
            return Err(Error::Unsupported("Creative ADPCM sound".into()));
        }
        (c, b) => {
            return Err(Error::Unsupported(format!(
                "Creative Voice coding {c} ({b}-bit)"
            )));
        }
    };
    let (rate, channels) = (first.rate, first.channels);
    if channels == 0 || rate == 0 {
        return Err(bad("no channels or sampling rate"));
    }
    let fb = usize::from(channels) * usize::from(bits / 8);
    let mut samples: Vec<Sample> = Vec::new();
    let mut t = 0;
    for r in runs.iter().filter(|r| {
        r.codec == first.codec && r.rate == rate && r.channels == channels && r.bits == bits
    }) {
        for mut s in pcm_samples(r.off, r.len, fb, PCM_FRAMES, MAX_SAMPLES - samples.len()) {
            s.dts += t;
            s.cts = s.dts as i64;
            samples.push(s);
        }
        t = samples.last().map_or(0, |s| s.dts + u64::from(s.duration));
    }
    Ok(track(codec, channels, rate, samples))
}

/// The Fibonacci-delta steps of 8SVX's packing (`sCompression` 1).
const FIBONACCI: [i8; 16] = [-34, -21, -13, -8, -5, -3, -2, -1, 0, 1, 2, 3, 5, 8, 13, 21];

/// Amiga 8SVX: `VHDR` (lengths, rate, packing), `CHAN` (6: stereo, left then right whole),
/// `BODY` (signed 8-bit). Only the first octave plays (the one-shot and repeat parts).
/// 16SV is the same with 16-bit big-endian samples.
fn svx(d: &[u8]) -> Result<AudioTrack> {
    let s = Svx::parse(d)?;
    if !s.stereo && !s.fibonacci {
        let bits = if s.wide { 16 } else { 8 };
        return plain_track(pcm(bits, false, false), 1, s.rate, bits, s.off, s.len);
    }
    // Stereo halves and packed bytes are unpacked by [`SvxSound`]; the track's samples
    // are runs of frames with no bytes of their own.
    let frames = s.frames() as u64;
    let channels = if s.stereo { 2 } else { 1 };
    let mut samples = Vec::new();
    let mut t = 0;
    while t < frames && samples.len() < MAX_SAMPLES {
        let n = PCM_FRAMES.min(frames - t);
        samples.push(sample(s.off, 0, t, n));
        t += n;
    }
    let flags = u8::from(s.fibonacci) | (u8::from(s.stereo) << 1) | (u8::from(s.wide) << 2);
    Ok(track(
        AudioCodec::Other(*b"8SVX", flags),
        channels,
        s.rate,
        samples,
    ))
}

/// An 8SVX file's facts.
struct Svx {
    rate: u32,
    stereo: bool,
    fibonacci: bool,
    wide: bool,
    /// The body (one octave: both halves when stereo).
    off: usize,
    len: usize,
}

impl Svx {
    fn parse(d: &[u8]) -> Result<Svx> {
        let wide = &d[8..12] == b"16SV";
        let end = (be32(d, 4).unwrap_or(0) as usize)
            .saturating_add(8)
            .min(d.len());
        let mut at = 12;
        let (mut vhdr, mut body, mut chan) = (None, None, 2);
        while at + 8 <= end {
            let id = &d[at..at + 4];
            let size = be32(d, at + 4).unwrap_or(0) as usize;
            let b = at + 8;
            let len = size.min(end - b);
            match id {
                b"VHDR" => vhdr = Some(&d[b..b + len]),
                b"BODY" => body = Some((b, len)),
                b"CHAN" => chan = be32(d, b).unwrap_or(2),
                _ => {}
            }
            at = b.saturating_add(len + (len & 1));
        }
        let v = vhdr.ok_or_else(|| bad("8SVX without a voice header"))?;
        let (off, mut len) = body.ok_or_else(|| bad("8SVX without a body"))?;
        let one_shot = be32(v, 0).unwrap_or(0) as usize;
        let repeat = be32(v, 4).unwrap_or(0) as usize;
        let rate = u32::from(be16(v, 12).unwrap_or(0));
        let compression = v.get(15).copied().unwrap_or(0);
        if rate == 0 {
            return Err(bad("8SVX without a sampling rate"));
        }
        let fibonacci = match compression {
            0 => false,
            1 => true,
            2 => return Err(Error::Unsupported("8SVX exponential-delta packing".into())),
            n => return Err(Error::Unsupported(format!("8SVX packing {n}"))),
        };
        let stereo = chan == 6;
        let part = one_shot + repeat;
        if !fibonacci && part > 0 {
            let bytes = part * if wide { 2 } else { 1 } * if stereo { 2 } else { 1 };
            len = len.min(bytes);
        }
        if fibonacci && wide {
            return Err(Error::Unsupported("packed 16-bit 16SV".into()));
        }
        Ok(Svx {
            rate,
            stereo,
            fibonacci,
            wide,
            off,
            len,
        })
    }

    /// Frames a channel.
    fn frames(&self) -> usize {
        let per = if self.stereo { self.len / 2 } else { self.len };
        if self.fibonacci {
            // A pad byte and the start value, then two steps a byte.
            2 * per.saturating_sub(2)
        } else if self.wide {
            per / 2
        } else {
            per
        }
    }

    /// The whole sound, interleaved.
    fn unpack(&self, d: &[u8]) -> Vec<f32> {
        let body = d.get(self.off..self.off + self.len).unwrap_or_default();
        let halves: Vec<&[u8]> = if self.stereo {
            let h = body.len() / 2;
            vec![&body[..h], &body[h..2 * h]]
        } else {
            vec![body]
        };
        let chans: Vec<Vec<f32>> = halves
            .iter()
            .map(|h| {
                if self.fibonacci {
                    let mut v = h.get(1).map_or(0, |&b| b as i8);
                    let mut out = Vec::with_capacity(2 * h.len());
                    for &b in h.get(2..).unwrap_or_default() {
                        for n in [b >> 4, b & 15] {
                            v = v.wrapping_add(FIBONACCI[usize::from(n)]);
                            out.push(f32::from(v) / 128.0);
                        }
                    }
                    out
                } else if self.wide {
                    h.chunks_exact(2)
                        .map(|c| f32::from(i16::from_be_bytes([c[0], c[1]])) / 32768.0)
                        .collect()
                } else {
                    h.iter().map(|&b| f32::from(b as i8) / 128.0).collect()
                }
            })
            .collect();
        let n = chans.iter().map(Vec::len).min().unwrap_or(0);
        let mut out = Vec::with_capacity(n * chans.len());
        for i in 0..n {
            for c in &chans {
                out.push(c[i]);
            }
        }
        out
    }
}

/// Sound unpacked whole when the decoder is made (8SVX's stereo halves and packed bytes):
/// sample `k` is frames `k·4096…`.
struct Unpacked {
    pcm: Arc<Vec<f32>>,
    channels: usize,
}

impl SoundDecoder for Unpacked {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> Result<(), String> {
        Err("needs the sample's index".into())
    }
    fn decode_at(&mut self, index: usize, _: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let per = PCM_FRAMES as usize * self.channels;
        let start = (index * per).min(self.pcm.len());
        let end = (start + per).min(self.pcm.len());
        out.extend_from_slice(&self.pcm[start..end]);
        Ok(())
    }
    fn reset(&mut self) {}
}

pub(crate) fn is_svx(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"8SVX")
}

/// "8SVX (Fibonacci-delta)" and the like.
pub(crate) fn svx_name(flags: u8) -> String {
    let mut s = String::from(if flags & 4 != 0 { "16SV" } else { "8SVX" });
    if flags & 1 != 0 {
        s += " (Fibonacci-delta)";
    }
    s
}

/// The decoder of an 8SVX track whose samples have no bytes of their own.
pub(crate) fn svx_decoder(
    t: &AudioTrack,
    data: &[u8],
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let ch = t.channels.max(1);
    let pcm = if data.is_empty() {
        Vec::new()
    } else {
        Svx::parse(data)
            .map_err(|e| Silence::Unsupported(e.to_string()))?
            .unpack(data)
    };
    Ok((
        Box::new(Unpacked {
            pcm: Arc::new(pcm),
            channels: usize::from(ch),
        }),
        ch,
        t.sample_rate,
    ))
}

/// Audio Visual Research: a 128-byte big-endian header ("2BIT"), interleaved samples.
fn avr(d: &[u8]) -> Result<AudioTrack> {
    let stereo = be16(d, 12).unwrap_or(0) != 0;
    let bits = be16(d, 14).unwrap_or(0);
    let signed = be16(d, 16).unwrap_or(0) != 0;
    let rate = be32(d, 22).unwrap_or(0) & 0x00FF_FFFF;
    let frames = be32(d, 26).unwrap_or(0) as usize;
    let channels: u16 = if stereo { 2 } else { 1 };
    let codec = match (bits, signed) {
        (8, true) => pcm(8, false, false),
        (8, false) => pcm(8, false, true),
        (16, true) => pcm(16, false, false),
        (b, s) => {
            return Err(Error::Unsupported(format!(
                "AVR {b}-bit {} sound",
                if s { "signed" } else { "unsigned" }
            )));
        }
    };
    let bytes = frames * usize::from(channels) * usize::from(bits / 8);
    let len = if bytes == 0 {
        d.len() - 128
    } else {
        bytes.min(d.len() - 128)
    };
    plain_track(codec, channels, rate, bits, 128, len)
}

/// NIST SPHERE: an ASCII header ("NIST_1A", its size, then "name -type value" lines up to
/// "end_head"), then the samples: PCM, µ-law or A-law, or a Shorten stream.
fn sphere(d: &[u8]) -> Result<AudioTrack> {
    let text = std::str::from_utf8(&d[..d.len().min(65_536)])
        .unwrap_or_else(|e| std::str::from_utf8(&d[..e.valid_up_to()]).unwrap_or_default());
    let mut lines = text.lines();
    lines.next();
    let header: usize = lines
        .next()
        .and_then(|l| l.trim().parse().ok())
        .ok_or_else(|| bad("SPHERE header size"))?;
    let mut field = std::collections::HashMap::new();
    for l in lines {
        if l.trim() == "end_head" {
            break;
        }
        let mut it = l.splitn(3, ' ');
        if let (Some(k), Some(_), Some(v)) = (it.next(), it.next(), it.next()) {
            field.insert(k.to_owned(), v.trim().to_owned());
        }
    }
    let num = |k: &str| field.get(k).and_then(|v| v.parse::<u64>().ok());
    let channels = num("channel_count").unwrap_or(1) as u16;
    let rate = num("sample_rate").unwrap_or(0) as u32;
    let width = num("sample_n_bytes").unwrap_or(2) as u16;
    let count = num("sample_count");
    let coding = field
        .get("sample_coding")
        .cloned()
        .unwrap_or_else(|| "pcm".into());
    let order = field.get("sample_byte_format").cloned().unwrap_or_default();
    if header < 16 || header > d.len() {
        return Err(bad("SPHERE header runs past the end"));
    }
    let off = header;
    let mut len = d.len() - off;
    if coding.contains("shorten") {
        return crate::shn_sound::track_at(d, off, Some((channels, rate)));
    }
    if let Some(n) = count {
        len = len.min((n * u64::from(channels) * u64::from(width)) as usize);
    }
    let (codec, bits) = match coding.as_str() {
        "pcm" | "" => match width {
            // One-byte PCM in SPHERE is signed.
            1 => (pcm(8, false, false), 8),
            2..=4 => (pcm(width * 8, false, order != "10"), width * 8),
            w => return Err(Error::Unsupported(format!("SPHERE {}-bit PCM", w * 8))),
        },
        "ulaw" | "mu-law" => (AudioCodec::G711 { alaw: false }, 8),
        "alaw" => (AudioCodec::G711 { alaw: true }, 8),
        c => return Err(Error::Unsupported(format!("SPHERE '{c}' sound"))),
    };
    plain_track(codec, channels, rate, bits, off, len)
}

/// Berkeley/IRCAM/CARL sound: a 1024-byte header (magic, rate as a float, channels, packing
/// mode) in the byte order its magic says, then interleaved samples.
fn ircam(d: &[u8]) -> Result<AudioTrack> {
    let le = ircam_order(d).ok_or_else(|| bad("IRCAM magic"))?;
    let r = |at| if le { le32(d, at) } else { be32(d, at) };
    let rate = f32::from_bits(r(4).unwrap_or(0));
    let channels = r(8).unwrap_or(0);
    let mode = r(12).unwrap_or(0);
    if !(rate.is_finite() && (1.0..1e7).contains(&rate)) {
        return Err(bad("IRCAM sampling rate"));
    }
    let channels = u16::try_from(channels).map_err(|_| bad("IRCAM channels"))?;
    let (codec, bits) = match mode {
        1 => (pcm(8, false, false), 8),
        2 => (pcm(16, false, le), 16),
        3 => (pcm(24, false, le), 24),
        0x4_0004 => (pcm(32, false, le), 32),
        4 => (pcm(32, true, le), 32),
        8 => (pcm(64, true, le), 64),
        0x1_0001 => (AudioCodec::G711 { alaw: true }, 8),
        0x2_0001 => (AudioCodec::G711 { alaw: false }, 8),
        m => return Err(Error::Unsupported(format!("IRCAM packing mode {m:#x}"))),
    };
    plain_track(
        codec,
        channels,
        rate.round() as u32,
        bits,
        1024,
        d.len() - 1024,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints() {
        let d = [0x81, 0x00, 0x7F, 0xFF, 0x7F];
        let mut at = 0;
        assert_eq!(varint(&d, &mut at), Some(128));
        assert_eq!(varint(&d, &mut at), Some(127));
        assert_eq!(varint(&d, &mut at), Some(16_383));
        assert_eq!(varint(&d, &mut at), None);
    }

    #[test]
    fn fibonacci_unpacks() {
        let s = Svx {
            rate: 8000,
            stereo: false,
            fibonacci: true,
            wide: false,
            off: 0,
            len: 4,
        };
        // pad, start 10, steps +21 −34, +0 −1
        let d = [0u8, 10, 0xF0, 0x87];
        let v: Vec<i32> = s.unpack(&d).iter().map(|v| (v * 128.0) as i32).collect();
        assert_eq!(v, vec![31, -3, -3, -4]);
        assert_eq!(s.frames(), 4);
    }
}

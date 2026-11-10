//! Speech recordings: GSM 06.10 (`specular-gsm`) as raw `.gsm` frames and in WAVE or AVI
//! (format tag 0x31, Microsoft's 65-byte blocks), HTK waveforms (`.htk`) and Asterisk's
//! headerless signed linear (`.sln`, `.sln16` … `.sln192`, opened by extension).
//!
//! GSM's codec is `Other(b"gsm ", packing)`: 0 for plain 33-byte frames, 1 for Microsoft's
//! blocks. A track's samples are runs of whole frames or blocks.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack, Error, Result};
use crate::sound_file::{MAX_SAMPLES, plain_track, sample, track};

pub(crate) fn is_gsm(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"gsm ")
}

/// The codec of GSM in a WAVE format chunk (tag 0x31).
pub(crate) const GSM_MS: AudioCodec = AudioCodec::Other(*b"gsm ", 1);

/// Plain frames in one sample of the track (about 0.6 s).
const FRAMES_PER_SAMPLE: usize = 32;

/// A raw `.gsm` file: 8 kHz mono, 160 samples a 33-byte frame.
pub(crate) fn gsm(d: &[u8]) -> Result<AudioTrack> {
    let frames = d.len() / specular_gsm::FRAME_BYTES;
    if frames == 0 {
        return Err(Error::Malformed("GSM file without a whole frame".into()));
    }
    let chunk = FRAMES_PER_SAMPLE * specular_gsm::FRAME_BYTES;
    let per = (FRAMES_PER_SAMPLE * specular_gsm::FRAME_SAMPLES) as u64;
    let mut samples = Vec::new();
    let mut at = 0;
    let mut t = 0;
    let whole = frames * specular_gsm::FRAME_BYTES;
    while at < whole && samples.len() < MAX_SAMPLES {
        let n = chunk.min(whole - at);
        let dur = (n / specular_gsm::FRAME_BYTES * specular_gsm::FRAME_SAMPLES) as u64;
        samples.push(sample(at, n, t, dur.min(per)));
        t += dur;
        at += n;
    }
    Ok(track(AudioCodec::Other(*b"gsm ", 0), 1, 8000, samples))
}

/// GSM in WAVE: Microsoft blocks of two frames (320 samples) in `len` bytes at `off`.
pub(crate) fn gsm_in_wave(channels: u16, rate: u32, off: usize, len: usize) -> Result<AudioTrack> {
    if channels != 1 {
        return Err(Error::Unsupported(format!(
            "GSM 06.10 with {channels} channels (it is mono speech)"
        )));
    }
    let block = specular_gsm::MS_BLOCK_BYTES;
    let chunk = block * 16;
    let mut samples = Vec::new();
    let mut at = 0;
    let mut t = 0;
    let whole = len / block * block;
    while at < whole && samples.len() < MAX_SAMPLES {
        let n = chunk.min(whole - at);
        let dur = (n / block * 2 * specular_gsm::FRAME_SAMPLES) as u64;
        samples.push(sample(off + at, n, t, dur));
        t += dur;
        at += n;
    }
    Ok(track(GSM_MS, 1, rate, samples))
}

struct Gsm {
    dec: specular_gsm::Decoder,
    ms: bool,
}

impl SoundDecoder for Gsm {
    fn decode(&mut self, s: &[u8], out: &mut Vec<f32>) -> std::result::Result<(), String> {
        let mut buf = [0i16; specular_gsm::FRAME_SAMPLES];
        let push = |b: &[i16; 160], out: &mut Vec<f32>| {
            out.extend(b.iter().map(|&v| f32::from(v) / 32768.0));
        };
        if self.ms {
            for blk in s.chunks_exact(specular_gsm::MS_BLOCK_BYTES) {
                let Some(ps) = specular_gsm::Params::ms(blk) else {
                    continue;
                };
                for p in &ps {
                    self.dec.decode(p, &mut buf);
                    push(&buf, out);
                }
            }
        } else {
            for f in s.chunks_exact(specular_gsm::FRAME_BYTES) {
                match specular_gsm::Params::plain(f) {
                    Some(p) => self.dec.decode(&p, &mut buf),
                    // A frame without its signature: silence in its place.
                    None => buf = [0; specular_gsm::FRAME_SAMPLES],
                }
                push(&buf, out);
            }
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.dec.reset();
    }
    fn preroll(&self) -> usize {
        // The filters settle within a frame or two.
        1
    }
}

pub(crate) fn gsm_decoder(
    t: &AudioTrack,
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let ms = matches!(t.codec, AudioCodec::Other(_, 1));
    Ok((
        Box::new(Gsm {
            dec: specular_gsm::Decoder::new(),
            ms,
        }),
        1,
        t.sample_rate,
    ))
}

/// HTK parameter kinds (the low six bits), in words.
fn htk_kind(k: u16) -> &'static str {
    match k & 0x3F {
        1 => "LPC",
        2 => "LPC reflection",
        3 => "LPC cepstral",
        4 => "LPC delta-cepstral",
        5 => "LPC reflection (integer)",
        6 => "MFCC",
        7 => "filter-bank",
        8 => "mel-spectrum",
        9 => "user",
        10 => "vector-quantised",
        11 => "PLP",
        _ => "unknown",
    }
}

/// An HTK file: a waveform is 16-bit big-endian samples at the header's period; parameter
/// files (features computed for speech recognisers) are refused in words.
pub(crate) fn htk(d: &[u8]) -> Result<AudioTrack> {
    if !specular_decode::sound_ident::is_htk(d) {
        return Err(Error::Malformed("not an HTK file".into()));
    }
    let n = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as usize;
    let period = u32::from_be_bytes([d[4], d[5], d[6], d[7]]);
    let size = u16::from_be_bytes([d[8], d[9]]);
    let kind = u16::from_be_bytes([d[10], d[11]]);
    if kind & 0x3F != 0 {
        return Err(Error::Unsupported(format!(
            "HTK {} features (speech-recognition data, not sound)",
            htk_kind(kind)
        )));
    }
    if kind & 0x0400 != 0 || size != 2 {
        return Err(Error::Unsupported("compressed HTK waveforms".into()));
    }
    let rate = (10_000_000 / period.max(1)).max(1);
    let codec = AudioCodec::Pcm {
        bits: 16,
        float: false,
        little_endian: false,
    };
    plain_track(codec, 1, rate, 16, 12, n * 2)
}

/// The rate of a signed-linear file by its extension (`.sln` is 8 kHz, `.sln16` 16 kHz…).
pub fn sln_rate(ext: &str) -> Option<u32> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "sln" | "raw" => 8000,
        "sln12" => 12_000,
        "sln16" => 16_000,
        "sln24" => 24_000,
        "sln32" => 32_000,
        "sln44" => 44_100,
        "sln48" => 48_000,
        "sln96" => 96_000,
        "sln192" => 192_000,
        _ => return None,
    })
}

/// True when the first 512 bytes are a UTF-8 BOM or printable text, as in a Visual Studio
/// solution; 16-bit sound almost never is.
fn looks_like_text(d: &[u8]) -> bool {
    let head = &d[..d.len().min(512)];
    head.starts_with(b"\xEF\xBB\xBF")
        || head
            .iter()
            .all(|&b| matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7E))
}

/// Headerless signed 16-bit little-endian mono at `rate`.
pub(crate) fn sln(d: &[u8], rate: u32) -> Result<AudioTrack> {
    if d.len() < 2 {
        return Err(Error::Malformed("an empty signed-linear file".into()));
    }
    if looks_like_text(d) {
        return Err(Error::Unsupported(
            "a text file (likely a Visual Studio solution), not signed-linear sound".into(),
        ));
    }
    let codec = AudioCodec::Pcm {
        bits: 16,
        float: false,
        little_endian: true,
    };
    plain_track(codec, 1, rate, 16, 0, d.len() / 2 * 2)
}

/// The codec GUID of EVRC in a QCP `fmt ` chunk (RFC 3625 §4), as stored.
const QCP_EVRC: [u8; 16] = [
    0x8D, 0xD4, 0x89, 0xE6, 0x76, 0x90, 0xB5, 0x46, 0x91, 0xEF, 0x73, 0x6A, 0x51, 0x00, 0xCE, 0xB4,
];
/// SMV's (RFC 3625 §4), as stored.
const QCP_SMV: [u8; 16] = [
    0x75, 0x2B, 0x7C, 0x8D, 0x97, 0xA7, 0x49, 0xED, 0x98, 0x5E, 0xD5, 0x3C, 0x8C, 0xC7, 0x5F, 0x84,
];

/// Packets in one sample of a QCP track (one second).
const QCP_PACKETS_PER_SAMPLE: usize = 50;

/// Whether `d` is a QCP file (RIFF `QLCM`).
pub(crate) fn is_qcp(d: &[u8]) -> bool {
    d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"QLCM"
}

/// A QCP file (RFC 3625): QCELP-13 or EVRC packets, each a rate octet and its frame, at
/// 8 kHz. SMV and QCELP-8 are named and refused.
pub(crate) fn qcp(d: &[u8]) -> Result<AudioTrack> {
    if !is_qcp(d) {
        return Err(Error::Malformed("not a QCP file".into()));
    }
    let riff_end = (u32::from_le_bytes([d[4], d[5], d[6], d[7]]) as usize)
        .saturating_add(8)
        .min(d.len());
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= riff_end {
        let id = &d[at..at + 4];
        let len = u32::from_le_bytes([d[at + 4], d[at + 5], d[at + 6], d[at + 7]]) as usize;
        let from = at + 8;
        let to = from.saturating_add(len).min(riff_end);
        match id {
            b"fmt " => fmt = Some(&d[from..to]),
            // A short last chunk (a truncated file) keeps what is there.
            b"data" => data = Some((from, to - from)),
            _ => {}
        }
        at = from.saturating_add(len).saturating_add(len & 1);
    }
    let fmt = fmt.ok_or_else(|| Error::Malformed("QCP file without its format chunk".into()))?;
    let guid: [u8; 16] = fmt
        .get(2..18)
        .and_then(|g| g.try_into().ok())
        .ok_or_else(|| Error::Malformed("QCP format chunk too short".into()))?;
    let packet_size = fmt
        .get(102..104)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
    let evrc = guid == QCP_EVRC;
    if guid == QCP_SMV {
        return Err(Error::Unsupported("SMV speech (.qcp)".into()));
    }
    if !evrc && !specular_qcelp::QCP_GUIDS.contains(&guid) {
        return Err(Error::Unsupported("QCP speech of an unknown codec".into()));
    }
    if !evrc && specular_qcelp::is_qcelp8(usize::from(packet_size)) {
        return Err(Error::Unsupported("QCELP-8 (IS-96A) speech".into()));
    }
    // The rate map: (frame bytes after the octet, octet) pairs.
    let map: Vec<(u8, u8)> = fmt
        .get(114..130)
        .unwrap_or_default()
        .chunks_exact(2)
        .map(|c| (c[0], c[1]))
        .filter(|&(n, _)| n > 0)
        .collect();
    let len_of = |octet: u8| {
        let own = if evrc {
            specular_evrc::packet_len(octet)
        } else {
            specular_qcelp::packet_len(octet)
        };
        own.or_else(|| {
            map.iter()
                .find(|&&(_, o)| o == octet)
                .map(|&(n, _)| usize::from(n) + 1)
        })
    };
    let (off, len) = data.ok_or_else(|| Error::Malformed("QCP file without data".into()))?;
    let body = &d[off..off + len];
    let mut samples = Vec::new();
    let (mut p, mut start, mut count, mut t) = (0usize, 0usize, 0usize, 0u64);
    while p < body.len() && samples.len() < MAX_SAMPLES {
        let Some(n) = len_of(body[p]).filter(|&n| p + n <= body.len()) else {
            break;
        };
        p += n;
        count += 1;
        if count == QCP_PACKETS_PER_SAMPLE {
            let dur = (count * 160) as u64;
            samples.push(sample(off + start, p - start, t, dur));
            t += dur;
            (start, count) = (p, 0);
        }
    }
    if count > 0 && samples.len() < MAX_SAMPLES {
        samples.push(sample(off + start, p - start, t, (count * 160) as u64));
    }
    if samples.is_empty() {
        return Err(Error::Malformed("QCP file without a whole packet".into()));
    }
    let codec = AudioCodec::Other(if evrc { *b"sevc" } else { *b"qclp" }, 0);
    Ok(track(codec, 1, 8000, samples))
}

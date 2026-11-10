//! DSD (1-bit) sound files: Sony's DSF (`.dsf`) and Philips' DSDIFF (`.dff`), played as PCM
//! through WavPack's decimating filter ([`specular_wavpack::DsdToPcm`]: 88.2 or 96 kHz).
//!
//! DSF (Sony's "DSF File Format Specification" 1.01): little-endian chunks `DSD `, `fmt `,
//! `data`; each channel's bits in blocks of 4096 bytes, blocks of the channels in turn,
//! least significant bit first (or most, when "bits per sample" is 8).
//! DSDIFF (Philips' "DSDIFF" 1.5): big-endian `FRM8` form of chunks with 64-bit sizes; the
//! `PROP` chunk's `FS ` (rate) and `CHNL`; `DSD ` data interleaved by byte, most significant
//! bit first. DST-compressed DSDIFF is refused by name.
//!
//! The track's samples are runs of 4096 bytes a channel (32 768 one-bit samples), timed in
//! PCM samples; the codec is `Other(b"DSF " | b"DFF ", log2 of the decimation)`, its top bit set for a
//! DSF whose bits run most significant first.

use crate::audio::{Silence, SoundDecoder};
use crate::lossless_sound::{emit, out_channels};
use crate::mp4::{AudioCodec, AudioTrack, Edit, Error, Result, Sample};

/// Bytes a channel in a run (DSF's block size).
const RUN: usize = 4096;

fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn le64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}
fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn be64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

pub(crate) fn is_dsf(d: &[u8]) -> bool {
    d.len() >= 28 && d.starts_with(b"DSD ") && le64(d, 4) == Some(28)
}

pub(crate) fn is_dff(d: &[u8]) -> bool {
    d.len() >= 16 && d.starts_with(b"FRM8") && &d[12..16] == b"DSD "
}

pub(crate) fn is_dsd(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"DSF " || t == b"DFF ")
}

/// "DSD64" and the like, from the codec's decimation (the PCM rate is 88.2 or 96 kHz, so
/// a decimation of 4 is DSD64).
pub(crate) fn name(shift: u8) -> String {
    let s = u32::from(shift & 0x7F);
    if s < 2 {
        return "DSD".into();
    }
    format!("DSD{}", 64u32 << (s - 2).min(8))
}

/// The track: runs of `RUN` bytes a channel from `off` (DSF: the channels' blocks in turn;
/// DSDIFF: interleaved bytes), `bytes` a channel in all, at 1-bit rate `rate`.
fn dsd_track(
    tag: [u8; 4],
    channels: u16,
    rate: u32,
    off: usize,
    bytes: u64,
    stored: usize,
) -> Result<AudioTrack> {
    if channels == 0 || channels > 8 {
        return Err(Error::Unsupported(format!("DSD with {channels} channels")));
    }
    if !(8 * 44_100..=specular_wavpack::dsd::MAX_DSD_RATE * 8).contains(&rate)
        || !rate.is_multiple_of(8)
    {
        return Err(Error::Unsupported(format!("DSD at {rate} Hz")));
    }
    let byte_rate = rate / 8;
    let spec = specular_wavpack::dsd::FilterSpec::new(byte_rate);
    let d = spec.decimation as u64;
    let shift = spec.decimation.trailing_zeros() as u8;
    let ch = usize::from(channels);
    let run = RUN * ch;
    let mut samples: Vec<Sample> = Vec::new();
    let mut at = 0usize;
    let mut done = 0u64;
    while at + ch <= stored && done < bytes && samples.len() < crate::sound_file::MAX_SAMPLES {
        let n = run.min(stored - at);
        let per = (n / ch) as u64;
        // PCM samples after the bytes of this run: ⌊(done + per) / d⌋ − ⌊done / d⌋.
        // (the last run padded past the stream: only its bytes count).
        let valid = per.min(bytes - done);
        let pcm = (done + valid) / d - done / d;
        samples.push(crate::sound_file::sample(off + at, n, done / d, pcm));
        done += per;
        at += n;
    }
    let mut tr = crate::sound_file::track(
        AudioCodec::Other(tag, shift),
        channels,
        spec.pcm_rate,
        samples,
    );
    tr.edit = Edit {
        start: 0,
        delay_us: 0,
        length_us: Some(bytes * 1_000_000 / u64::from(byte_rate)),
    };
    Ok(tr)
}

/// A DSF file's track.
pub(crate) fn dsf(d: &[u8]) -> Result<AudioTrack> {
    let mut at = 28;
    let (mut fmt, mut data) = (None, None);
    while at + 12 <= d.len() {
        let size = le64(d, at + 4).unwrap_or(0) as usize;
        if size < 12 {
            break;
        }
        let body = at + 12;
        let len = (size - 12).min(d.len() - body);
        match &d[at..at + 4] {
            b"fmt " => fmt = Some(&d[body..body + len]),
            b"data" => data = Some((body, len)),
            _ => {}
        }
        at = at.saturating_add(size);
    }
    let f = fmt.ok_or_else(|| bad("DSF without a format chunk"))?;
    let (off, len) = data.ok_or_else(|| bad("DSF without data"))?;
    if le32(f, 4) != Some(0) {
        return Err(Error::Unsupported("DSF that isn't raw DSD".into()));
    }
    let channels = le32(f, 12).unwrap_or(0) as u16;
    let rate = le32(f, 16).unwrap_or(0);
    let lsb = match le32(f, 20) {
        Some(1) => true,
        Some(8) => false,
        _ => return Err(bad("DSF bits per sample")),
    };
    let count = le64(f, 24).unwrap_or(0);
    if le32(f, 32) != Some(RUN as u32) {
        return Err(Error::Unsupported(
            "DSF blocks other than 4096 bytes".into(),
        ));
    }
    let mut t = dsd_track(*b"DSF ", channels, rate, off, count.div_ceil(8), len)?;
    // The codec's top bit: bits most significant first.
    if let AudioCodec::Other(_, ref mut shift) = t.codec
        && !lsb
    {
        *shift |= 0x80;
    }
    Ok(t)
}

/// A DSDIFF file's track.
pub(crate) fn dff(d: &[u8]) -> Result<AudioTrack> {
    let end = (be64(d, 4).unwrap_or(0) as usize)
        .saturating_add(12)
        .min(d.len());
    let mut at = 16;
    let (mut rate, mut channels, mut data) = (0u32, 0u16, None);
    let mut compression = *b"DSD ";
    while at + 12 <= end {
        let id = &d[at..at + 4];
        let size = be64(d, at + 4).unwrap_or(0) as usize;
        let body = at + 12;
        let len = size.min(end - body);
        match id {
            b"PROP" if d.get(body..body + 4) == Some(b"SND ") => {
                let mut p = body + 4;
                while p + 12 <= body + len {
                    let sid = &d[p..p + 4];
                    let ssize = be64(d, p + 4).unwrap_or(0) as usize;
                    let sb = p + 12;
                    match sid {
                        b"FS  " => rate = be32(d, sb).unwrap_or(0),
                        b"CHNL" => channels = be16(d, sb).unwrap_or(0),
                        b"CMPR" => {
                            if let Some(c) = d.get(sb..sb + 4) {
                                compression.copy_from_slice(c);
                            }
                        }
                        _ => {}
                    }
                    p = sb.saturating_add(ssize + (ssize & 1));
                }
            }
            b"DSD " => data = Some((body, len)),
            b"DST " => compression = *b"DST ",
            _ => {}
        }
        at = body.saturating_add(len + (len & 1));
    }
    if &compression == b"DST " {
        return Err(Error::Unsupported(
            "DST-compressed DSDIFF (Direct Stream Transfer)".into(),
        ));
    }
    let (off, len) = data.ok_or_else(|| bad("DSDIFF without DSD data"))?;
    let per = (len / usize::from(channels.max(1))) as u64;
    dsd_track(*b"DFF ", channels, rate, off, per, len)
}

struct Dsd {
    conv: specular_wavpack::DsdToPcm,
    channels: usize,
    /// DSF: each run is the channels' blocks in turn, bits least significant first.
    planar: bool,
    lsb: bool,
    /// Bytes a channel before each run.
    starts: Vec<u64>,
    /// PCM frames each run gives (the last is cut at the stream's end).
    lens: Vec<u32>,
    bytes: Vec<i32>,
    pcm: Vec<f32>,
    bits: Vec<i32>,
    scratch: Vec<f32>,
}

impl SoundDecoder for Dsd {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> Result<(), String> {
        Err("needs the run's index".into())
    }
    fn decode_at(&mut self, index: usize, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let ch = self.channels;
        let per = sample.len() / ch;
        self.bytes.clear();
        self.bytes.reserve(per * ch);
        let fix = |b: u8| if self.lsb { b.reverse_bits() } else { b };
        if self.planar {
            for i in 0..per {
                for c in 0..ch {
                    self.bytes.push(i32::from(fix(sample[c * per + i])));
                }
            }
        } else {
            self.bytes
                .extend(sample[..per * ch].iter().map(|&b| i32::from(fix(b))));
        }
        let index_bytes = self.starts.get(index).copied().unwrap_or(0);
        self.pcm.clear();
        self.conv.run(index_bytes, &self.bytes, &mut self.pcm);
        let keep = self.lens.get(index).map_or(0, |&n| n as usize * ch);
        self.pcm.truncate(keep);
        self.bits.clear();
        self.bits
            .extend(self.pcm.iter().map(|v| v.to_bits() as i32));
        emit(&self.bits, ch, None, &mut self.scratch, out);
        Ok(())
    }
    fn reset(&mut self) {
        self.conv.reset();
    }
    fn preroll(&self) -> usize {
        // The filter's history (a few hundred bytes) is within the run before.
        1
    }
}

/// The decoder of a DSF or DSDIFF track.
pub(crate) fn decoder(
    t: &AudioTrack,
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let AudioCodec::Other(tag, shift) = t.codec else {
        return Err(Silence::Unsupported("DSD".into()));
    };
    let ch = usize::from(t.channels.max(1));
    let msb = shift & 0x80 != 0;
    let byte_rate = t.sample_rate << (shift & 0x7F);
    let mut starts = Vec::with_capacity(t.samples.len());
    let mut n = 0u64;
    for s in &t.samples {
        starts.push(n);
        n += u64::from(s.size) / ch as u64;
    }
    let dsf = &tag == b"DSF ";
    Ok((
        Box::new(Dsd {
            conv: specular_wavpack::DsdToPcm::new(byte_rate, ch),
            channels: ch,
            planar: dsf,
            lsb: dsf && !msb,
            starts,
            lens: t.samples.iter().map(|s| s.duration).collect(),
            bytes: Vec::new(),
            pcm: Vec::new(),
            bits: Vec::new(),
            scratch: Vec::new(),
        }),
        out_channels(ch),
        t.sample_rate,
    ))
}

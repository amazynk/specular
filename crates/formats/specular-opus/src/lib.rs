//! Specular's own Opus decoder (RFC 6716 with the RFC 8251 updates) for the sound of videos
//! and audio files, with the Ogg Opus header and multistream channel mapping of RFC 7845.
//!
//! Decodes SILK (narrow, medium and wide band, 10 to 60 ms frames, stereo), CELT (all band
//! layouts, 2.5 to 20 ms), hybrid frames, mode transitions with their redundant CELT frames
//! and code 0 to 3 packets with padding. Output is 48 kHz f32, interleaved. Multistream
//! packets of every channel mapping family of RFC 7845 and RFC 8486 are decoded: 0 and 1
//! (up to 8 channels, Vorbis channel order), 2 (ambisonics, ACN order with SN3D, plus an
//! optional non-diegetic stereo pair), 3 (ambisonics through a demixing matrix) and 255 (no
//! layout). Ambisonics come out as their plain components; rendering them to speakers is up
//! to the caller. Other families are refused with [`Error::Unsupported`].
//!
//! Entropy decoding is bit-exact (the final range of every packet of the official test
//! vectors matches). SILK's reconstruction runs in fixed point and matches the reference
//! decoder sample for sample at the internal rate; its upsampling to 48 kHz is our own (the
//! RFC leaves it open), fitted to the reference's.
//!
//! A lost packet (`decode(&[])`) is concealed: CELT and hybrid streams with noise shaped
//! like the last frame's band energies, fading; SILK by continuing the last pitch or noise
//! through its LPC filter, fading. [`Decoder::decode_fec`] rebuilds it from the next
//! packet's in-band FEC (LBRR) frames instead.
#![deny(unsafe_code)]
// The codec's loops index several per-band tables at once, in the RFC's terms.
#![allow(clippy::needless_range_loop)]

mod celt;
mod celt_alloc;
mod celt_bands;
#[rustfmt::skip]
mod celt_data;
mod mdct;
mod opus;
mod packet;
mod range;
mod resample;
#[rustfmt::skip]
mod resample_fit;
#[cfg(test)]
mod resample_study;
#[rustfmt::skip]
mod rfc_tables;
mod silk;
mod silk_lpc;
#[cfg(test)]
mod silk_study;
#[cfg(test)]
mod surround_study;

pub use packet::packet_samples;

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid stream using something this decoder doesn't implement.
    Unsupported(String),
    /// The header or a packet breaks the format.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "{s} can't be played yet"),
            Error::Malformed(s) => write!(f, "damaged Opus audio: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// The Ogg Opus identification header (RFC 7845 section 5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub channels: u8,
    pub pre_skip: u16,
    pub input_rate: u32,
    pub gain_db_q8: i16,
    pub family: u8,
    pub streams: u8,
    pub coupled: u8,
    pub mapping: Vec<u8>,
    /// Family 3: the demixing matrix (RFC 8486 §3.2), `channels` rows by `streams +
    /// coupled` columns, column by column, Q15. Empty for the other families (whose
    /// `mapping` is then one entry a channel; family 3's is the identity).
    pub demix: Vec<i16>,
}

/// Whether `channels` is an ambisonics channel count of RFC 8486: `(1 + n)²` components of
/// order `n` up to 14, plus an optional non-diegetic stereo pair.
pub fn ambisonic_channels(channels: u8) -> bool {
    let c = usize::from(channels);
    (0..=14usize).any(|n| (n + 1) * (n + 1) == c || (n + 1) * (n + 1) + 2 == c)
}

impl Head {
    /// Parses an "OpusHead" packet (also Matroska's A_OPUS CodecPrivate).
    pub fn parse(opus_head: &[u8]) -> Result<Head, Error> {
        let bad = |what: &str| Error::Malformed(format!("Opus header {what}"));
        if opus_head.len() < 19 || &opus_head[..8] != b"OpusHead" {
            return Err(bad("is missing"));
        }
        let version = opus_head[8];
        if version >> 4 != 0 {
            return Err(Error::Unsupported(format!("Opus header version {version}")));
        }
        let channels = opus_head[9];
        if channels == 0 {
            return Err(bad("has no channels"));
        }
        let pre_skip = u16::from_le_bytes([opus_head[10], opus_head[11]]);
        let input_rate =
            u32::from_le_bytes([opus_head[12], opus_head[13], opus_head[14], opus_head[15]]);
        let gain_db_q8 = i16::from_le_bytes([opus_head[16], opus_head[17]]);
        let family = opus_head[18];
        let mut demix = Vec::new();
        let (streams, coupled, mapping) = match family {
            3 => {
                if opus_head.len() < 21 {
                    return Err(bad("is cut short"));
                }
                let (streams, coupled) = (opus_head[19], opus_head[20]);
                if streams == 0
                    || coupled > streams
                    || usize::from(streams) + usize::from(coupled) > 255
                {
                    return Err(bad("has a bad stream count"));
                }
                let k = usize::from(streams) + usize::from(coupled);
                let need = 21 + 2 * k * usize::from(channels);
                let m = opus_head.get(21..need).ok_or_else(|| bad("is cut short"))?;
                demix = m
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect();
                (streams, coupled, (0..channels).collect())
            }
            0 => {
                if channels > 2 {
                    return Err(bad("has too many channels for mapping family 0"));
                }
                (1, channels - 1, (0..channels).collect())
            }
            _ => {
                let need = 21 + usize::from(channels);
                if opus_head.len() < need {
                    return Err(bad("is cut short"));
                }
                let streams = opus_head[19];
                let coupled = opus_head[20];
                let mapping = opus_head[21..need].to_vec();
                if streams == 0
                    || coupled > streams
                    || usize::from(streams) + usize::from(coupled) > 255
                {
                    return Err(bad("has a bad stream count"));
                }
                let decoded = u16::from(streams) + u16::from(coupled);
                if mapping.iter().any(|&m| m != 255 && u16::from(m) >= decoded) {
                    return Err(bad("maps a channel to a missing stream"));
                }
                (streams, coupled, mapping)
            }
        };
        Ok(Head {
            channels,
            pre_skip,
            input_rate,
            gain_db_q8,
            family,
            streams,
            coupled,
            mapping,
            demix,
        })
    }
}

/// Decoder of an Opus stream (single or multistream) at 48 kHz.
pub struct Decoder {
    channels: usize,
    streams: Vec<opus::StreamDecoder>,
    coupled: usize,
    mapping: Vec<u8>,
    /// Family 3's demixing matrix (column by column), as gains.
    demix: Vec<f32>,
    gain: f32,
    last_duration: usize,
    bufs: Vec<Vec<f32>>,
}

impl Decoder {
    pub fn new(head: &Head) -> Result<Decoder, Error> {
        match head.family {
            0 => {}
            1 => {
                if head.channels > 8 {
                    return Err(Error::Malformed(
                        "Opus header has too many channels for mapping family 1".into(),
                    ));
                }
            }
            // No layout: the channels as they are.
            255 => {}
            2 | 3 if !ambisonic_channels(head.channels) => {
                return Err(Error::Malformed(format!(
                    "Opus ambisonics header has {} channels",
                    head.channels
                )));
            }
            2 => {}
            3 => {
                let k = usize::from(head.streams) + usize::from(head.coupled);
                if head.demix.len() != k * usize::from(head.channels) {
                    return Err(Error::Malformed(
                        "Opus demixing matrix doesn't match the channel count".into(),
                    ));
                }
            }
            f => {
                return Err(Error::Unsupported(format!(
                    "Opus channel mapping family {f}"
                )));
            }
        }
        if head.mapping.len() != usize::from(head.channels) {
            return Err(Error::Malformed(
                "Opus header channel mapping doesn't match the channel count".into(),
            ));
        }
        let coupled = usize::from(head.coupled);
        let streams = (0..usize::from(head.streams))
            .map(|s| opus::StreamDecoder::new(if s < coupled { 2 } else { 1 }))
            .collect::<Vec<_>>();
        let bufs = vec![Vec::new(); streams.len()];
        Ok(Decoder {
            channels: usize::from(head.channels),
            streams,
            coupled,
            mapping: head.mapping.clone(),
            demix: head.demix.iter().map(|&v| f32::from(v) / 32768.0).collect(),
            gain: 10f32.powf(f32::from(head.gain_db_q8) / (20.0 * 256.0)),
            last_duration: 960,
            bufs,
        })
    }

    /// Decoded channel `j` (the streams' channels in order: both of each coupled stream,
    /// then the mono ones) at frame `i`.
    fn decoded(&self, j: usize, i: usize) -> f32 {
        let (s, sc, sch) = if j < 2 * self.coupled {
            (j / 2, j % 2, 2)
        } else {
            (j - self.coupled, 0, 1)
        };
        self.bufs
            .get(s)
            .and_then(|b| b.get(i * sch + sc))
            .copied()
            .unwrap_or(0.0)
    }

    /// Decodes one packet (for multistream: one packet holding all streams, self-delimited per
    /// RFC 6716 App. B); appends interleaved f32 samples (head.channels channels, 48 kHz) to
    /// `out`; returns samples per channel. An empty packet conceals a lost one.
    pub fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) -> Result<usize, Error> {
        let n = self.streams.len();
        let mut samples = None;
        if packet.is_empty() {
            for (s, buf) in self.streams.iter_mut().zip(self.bufs.iter_mut()) {
                let got = s.decode_packet(None, self.last_duration, buf)?;
                samples = Some(samples.map_or(got, |v: usize| v.min(got)));
            }
        } else {
            let mut pos = 0usize;
            let mut parsed = Vec::with_capacity(n);
            for s in 0..n {
                let rest = packet.get(pos..).filter(|r| !r.is_empty()).ok_or_else(|| {
                    Error::Malformed("Opus multistream packet is missing a stream".into())
                })?;
                let p = packet::parse(rest, s + 1 < n)?;
                parsed.push((pos, p));
                pos += parsed[s].1.len;
            }
            let dur = parsed[0].1.frames.len() * parsed[0].1.toc.frame_size;
            if parsed
                .iter()
                .any(|(_, p)| p.frames.len() * p.toc.frame_size != dur)
            {
                return Err(Error::Malformed(
                    "Opus streams of a packet differ in length".into(),
                ));
            }
            for (s, (pos, p)) in parsed.iter().enumerate() {
                let data = &packet[*pos..*pos + p.len];
                self.streams[s].decode_packet(Some((data, p)), 0, &mut self.bufs[s])?;
            }
            samples = Some(dur);
            self.last_duration = dur;
        }
        Ok(self.interleave(samples.unwrap_or(0), out))
    }

    /// Maps the streams' decodes (`bufs`) to the output channels; appends `samples` frames.
    fn interleave(&self, samples: usize, out: &mut Vec<f32>) -> usize {
        let start = out.len();
        out.resize(start + samples * self.channels, 0.0);
        let o = &mut out[start..];
        if !self.demix.is_empty() {
            // Family 3: each output channel is a row of the matrix times the decoded ones.
            let c_n = self.channels;
            let k_n = self.demix.len() / c_n;
            let mut x = vec![0.0f32; k_n];
            for i in 0..samples {
                for (j, v) in x.iter_mut().enumerate() {
                    *v = self.decoded(j, i);
                }
                for c in 0..c_n {
                    let s: f32 = (0..k_n).map(|j| self.demix[j * c_n + c] * x[j]).sum();
                    o[i * c_n + c] = s * self.gain;
                }
            }
            return samples;
        }
        for (c, &m) in self.mapping.iter().enumerate() {
            if m == 255 {
                continue;
            }
            let m = usize::from(m);
            let (s, sc, sch) = if m < 2 * self.coupled {
                (m / 2, m % 2, 2)
            } else {
                (m - self.coupled, 0, 1)
            };
            let buf = &self.bufs[s];
            for i in 0..samples {
                o[i * self.channels + c] =
                    buf.get(i * sch + sc).copied().unwrap_or(0.0) * self.gain;
            }
        }
        samples
    }

    /// Rebuilds a lost packet from the in-band FEC (SILK LBRR) data of `next`, the packet
    /// after it, which is then decoded normally with [`Decoder::decode`]; conceals the lost
    /// packet where `next` carries none. Appends like [`Decoder::decode`].
    pub fn decode_fec(&mut self, next: &[u8], out: &mut Vec<f32>) -> Result<usize, Error> {
        let n = self.streams.len();
        let mut pos = 0usize;
        let mut parsed = Vec::with_capacity(n);
        for s in 0..n {
            let Some(rest) = next.get(pos..).filter(|r| !r.is_empty()) else {
                return self.decode(&[], out);
            };
            let Ok(p) = packet::parse(rest, s + 1 < n) else {
                return self.decode(&[], out);
            };
            let len = p.len;
            parsed.push((pos, p));
            pos += len;
        }
        let lost = self.last_duration;
        let mut samples: Option<usize> = None;
        for (s, (pos, p)) in parsed.iter().enumerate() {
            let data = &next[*pos..*pos + p.len];
            let got = self.streams[s].decode_fec(data, p, lost, &mut self.bufs[s])?;
            samples = Some(samples.map_or(got, |v| v.min(got)));
        }
        Ok(self.interleave(samples.unwrap_or(0), out))
    }

    /// Forgets all decoder state (after a seek).
    pub fn reset(&mut self) {
        for s in &mut self.streams {
            s.reset();
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// The final range coder state of the last packet (for conformance checks); for several
    /// streams, the XOR of theirs.
    pub fn final_range(&self) -> u32 {
        self.streams.iter().fold(0, |a, s| a ^ s.range_final)
    }

    /// Whether CELT may code stereo bands 180 degrees out of phase (on by default; off gives
    /// the RFC 8251 section 10 output that downmixes to mono without cancellation).
    pub fn set_phase_inversion(&mut self, enabled: bool) {
        for s in &mut self.streams {
            s.set_phase_inversion(enabled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head() {
        let mut h = b"OpusHead".to_vec();
        h.extend_from_slice(&[1, 2, 0x38, 0x01, 0x80, 0xbb, 0, 0, 0, 0, 0]);
        let head = Head::parse(&h).unwrap();
        assert_eq!(head.channels, 2);
        assert_eq!(head.pre_skip, 312);
        assert_eq!(head.input_rate, 48000);
        assert_eq!(head.mapping, vec![0, 1]);
        assert_eq!(head.coupled, 1);
        let mut h255 = h.clone();
        h255[18] = 255;
        h255.extend_from_slice(&[2, 0, 0, 1]);
        let head = Head::parse(&h255).unwrap();
        assert_eq!(Decoder::new(&head).unwrap().channels(), 2);
        assert!(Head::parse(b"OpusHea").is_err());
    }

    #[test]
    fn ambisonic_counts() {
        let ok: Vec<u8> = (1..=255).filter(|&c| ambisonic_channels(c)).collect();
        assert_eq!(&ok[..8], &[1, 3, 4, 6, 9, 11, 16, 18]);
        assert!(ambisonic_channels(227) && !ambisonic_channels(5) && !ambisonic_channels(8));
    }

    /// Family 2 needs an ambisonics count; family 3's matrix mixes the decoded channels
    /// (column-major Q15) into the output ones.
    #[test]
    fn families_2_and_3() {
        let base = |family: u8, channels: u8| {
            let mut h = b"OpusHead".to_vec();
            h.extend_from_slice(&[1, channels, 0, 0, 0x80, 0xbb, 0, 0, 0, 0, family]);
            h
        };
        let mut f2 = base(2, 4);
        f2.extend_from_slice(&[4, 0, 0, 1, 2, 255]);
        let d = Decoder::new(&Head::parse(&f2).unwrap()).unwrap();
        assert_eq!(d.channels(), 4);
        let mut bad = base(2, 5);
        bad.extend_from_slice(&[5, 0, 0, 1, 2, 3, 4]);
        assert!(Decoder::new(&Head::parse(&bad).unwrap()).is_err());
        // One coupled stream and one mono (3 decoded channels) into 4 outputs.
        let mut f3 = base(3, 4);
        f3.extend_from_slice(&[2, 1]);
        let m: [[i16; 4]; 3] = [[16384, 0, 0, -32768], [0, 32767, 0, 0], [0, 0, 8192, 16384]];
        for col in m {
            for v in col {
                f3.extend_from_slice(&v.to_le_bytes());
            }
        }
        let head = Head::parse(&f3).unwrap();
        assert_eq!(head.demix.len(), 12);
        let mut d = Decoder::new(&head).unwrap();
        d.bufs[0] = vec![1.0, 0.5];
        d.bufs[1] = vec![0.25];
        let mut out = Vec::new();
        d.interleave(1, &mut out);
        let want = [0.5, 0.5 * 32767.0 / 32768.0, 0.0625, -1.0 + 0.125];
        for (g, w) in out.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "{out:?}");
        }
        f3.truncate(f3.len() - 2);
        assert!(Head::parse(&f3).is_err());
    }
}

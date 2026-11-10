//! LOAS/LATM framing (ISO/IEC 14496-3 §1.7), as broadcast transport streams carry AAC and
//! HE-AAC: an AudioSyncStream of AudioMuxElements, each with an optional StreamMuxConfig (the
//! AudioSpecificConfig of each stream inside) and the payloads of its subframes.
//!
//! Every multiplex of `audioMuxVersion` 0 and 1 is read: several programs and layers (the
//! first program's first layer is decoded), shared or separate time framing (payloads in
//! chunks, an access unit possibly spread over several), byte-counted or fixed-length
//! payloads. A new StreamMuxConfig with a different configuration is reported
//! ([`Latm::take_changes`]) so the decoder can follow it. Refused by name:
//! `audioMuxVersionA` 1 (reserved, its syntax undefined), and CELP or HVXC streams in the
//! multiplex (their payload lengths come from speech-codec tables).

use crate::bits::Bits;
use crate::config::{Config, read_asc};
use crate::{Error, Result};

fn malformed(s: &str) -> Error {
    Error::Malformed(format!("LATM: {s}"))
}

/// Whether `data` starts with a LOAS sync word.
pub fn is_loas(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 0x56 && data[1] & 0xe0 == 0xe0
}

/// Splits a LOAS stream into its AudioMuxElements (each without the 3-byte sync header),
/// resynchronising past garbage. Stops at a truncated element.
pub fn loas_frames(mut data: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        loop {
            if data.len() < 3 {
                return None;
            }
            if !is_loas(data) {
                data = &data[1..];
                continue;
            }
            let len = (usize::from(data[1] & 0x1f) << 8) | usize::from(data[2]);
            if 3 + len > data.len() {
                return None;
            }
            let el = &data[3..3 + len];
            data = &data[3 + len..];
            return Some(el);
        }
    })
}

/// How one stream of the multiplex gives its payload lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Length {
    /// Counted in bytes before each payload (`frameLengthType` 0).
    Bytes,
    /// A fixed number of bits (`frameLengthType` 1).
    Fixed(usize),
}

/// The multiplex state: the stream configuration of the last StreamMuxConfig.
#[derive(Debug, Clone, Default)]
pub struct Latm {
    /// The configuration of the decoded stream (program 0, layer 0).
    config: Option<Config>,
    version: u32,
    same_framing: bool,
    sub_frames: usize,
    /// How each stream of the multiplex (programs and layers in order) is framed.
    streams: Vec<Length>,
    other_data_bits: usize,
    /// Where the configuration changed since [`Latm::take_changes`]: the index into the
    /// output of the first raw data block it applies to, and the new configuration.
    changes: Vec<(usize, Config)>,
    /// An access unit of the decoded stream spread over chunks, so far.
    partial: Vec<u8>,
}

/// LatmGetValue().
fn latm_value(b: &mut Bits) -> usize {
    let n = b.read(2);
    let mut v = 0usize;
    for _ in 0..=n {
        v = (v << 8) | b.read(8) as usize;
    }
    v
}

/// Copies `bits` bits (a whole number of bytes or not) from the reader.
fn take_bits(b: &mut Bits, bits: usize, out: &mut Vec<u8>) -> Result<()> {
    if b.left() < bits as isize {
        return Err(malformed("payload runs past the element"));
    }
    out.extend((0..bits / 8).map(|_| b.read(8) as u8));
    let rest = (bits % 8) as u32;
    if rest > 0 {
        out.push((b.read(rest) << (8 - rest)) as u8);
    }
    Ok(())
}

impl Latm {
    pub fn new() -> Latm {
        Latm::default()
    }

    /// The stream's configuration, once a StreamMuxConfig has come.
    pub fn config(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    /// The configuration changes seen since the last call (the first StreamMuxConfig is
    /// one): each with the index, in the blocks [`Latm::parse`] appended, of the first
    /// block it applies to.
    pub fn take_changes(&mut self) -> Vec<(usize, Config)> {
        std::mem::take(&mut self.changes)
    }

    fn stream_mux_config(&mut self, b: &mut Bits, at: usize) -> Result<()> {
        let version = b.read(1);
        if version == 1 && b.bit() {
            return Err(Error::Unsupported(
                "LATM multiplex version A (reserved)".into(),
            ));
        }
        if version == 1 {
            latm_value(b); // taraBufferFullness
        }
        let same_framing = b.bit();
        let sub_frames = b.read(6) as usize + 1;
        let programs = b.read(4) + 1;
        let mut streams = Vec::new();
        let mut cfg: Option<Config> = None;
        let mut first: Option<Config> = None;
        for prog in 0..programs {
            let layers = b.read(3) + 1;
            for lay in 0..layers {
                let same_config = (prog > 0 || lay > 0) && b.bit();
                if !same_config {
                    let read = if version == 0 {
                        read_asc(b, None)
                    } else {
                        let len = latm_value(b);
                        let start = b.pos();
                        let r = read_asc(b, Some(len));
                        let used = b.pos() - start;
                        if used > len {
                            return Err(malformed("AudioSpecificConfig longer than its length"));
                        }
                        b.skip(len - used);
                        r
                    };
                    match read {
                        Ok(c) => cfg = Some(c),
                        // the decoded stream must be readable; another one only skippable
                        Err(e) if streams.is_empty() || version == 0 => return Err(e),
                        Err(_) => cfg = None,
                    }
                }
                if streams.is_empty() {
                    first = cfg.clone();
                }
                let length = match b.read(3) {
                    0 => {
                        b.read(8); // latmBufferFullness
                        // coreFrameOffset only follows a CELP core of scalable AAC
                        Length::Bytes
                    }
                    1 => Length::Fixed(8 * (b.read(9) as usize + 20)),
                    3..=7 => {
                        return Err(Error::Unsupported(
                            "LATM with CELP or HVXC speech streams".into(),
                        ));
                    }
                    t => return Err(malformed(&format!("reserved frame length type {t}"))),
                };
                streams.push(length);
            }
        }
        let mut other = 0usize;
        if b.bit() {
            if version == 1 {
                other = latm_value(b);
            } else {
                loop {
                    let esc = b.bit();
                    other = (other << 8) | b.read(8) as usize;
                    if !esc || other > 1 << 20 {
                        break;
                    }
                }
            }
        }
        if b.bit() {
            b.read(8); // crcCheckSum
        }
        if b.overrun() {
            return Err(malformed("StreamMuxConfig is truncated"));
        }
        let first = first.ok_or_else(|| malformed("no configuration for the first stream"))?;
        if self.config.as_ref() != Some(&first) {
            self.changes.push((at, first.clone()));
            self.partial.clear();
        }
        self.version = version;
        self.same_framing = same_framing;
        self.sub_frames = sub_frames;
        self.streams = streams;
        self.other_data_bits = other;
        self.config = Some(first);
        Ok(())
    }

    /// Reads one MuxSlotLengthBytes.
    fn slot_bytes(b: &mut Bits) -> usize {
        let mut len = 0usize;
        loop {
            let t = b.read(8) as usize;
            len += t;
            if t != 255 || b.overrun() {
                break;
            }
        }
        len
    }

    /// Parses one AudioMuxElement, appending the access unit of the first program's first
    /// layer of each subframe to `out`. Elements before the first StreamMuxConfig give
    /// nothing.
    pub fn parse(&mut self, element: &[u8], out: &mut Vec<Vec<u8>>) -> Result<()> {
        let mut b = Bits::new(element);
        if !b.bit() {
            self.stream_mux_config(&mut b, out.len())?;
        } else if self.config.is_none() {
            return Ok(());
        }
        for _ in 0..self.sub_frames {
            if self.same_framing {
                // PayloadLengthInfo, then PayloadMux, both over every stream
                let lens: Vec<usize> = self
                    .streams
                    .iter()
                    .map(|l| match l {
                        Length::Bytes => 8 * Self::slot_bytes(&mut b),
                        Length::Fixed(bits) => *bits,
                    })
                    .collect();
                for (i, &bits) in lens.iter().enumerate() {
                    if i == 0 {
                        let mut au = Vec::with_capacity(bits / 8 + 1);
                        take_bits(&mut b, bits, &mut au)?;
                        out.push(au);
                    } else if b.left() < bits as isize {
                        return Err(malformed("payload runs past the element"));
                    } else {
                        b.skip(bits);
                    }
                }
            } else {
                // chunks: (stream, length, end of access unit), then their payloads
                let chunks = b.read(4) as usize + 1;
                let mut info = Vec::with_capacity(chunks);
                for _ in 0..chunks {
                    let s = b.read(4) as usize;
                    let len = self.streams.get(s).copied();
                    let (bits, end) = match len {
                        Some(Length::Bytes) => (8 * Self::slot_bytes(&mut b), b.bit()),
                        Some(Length::Fixed(bits)) => (bits, true),
                        None => return Err(malformed("chunk of an unknown stream")),
                    };
                    info.push((s, bits, end));
                }
                for (s, bits, end) in info {
                    if s == 0 {
                        let mut part = std::mem::take(&mut self.partial);
                        take_bits(&mut b, bits, &mut part)?;
                        if end {
                            out.push(part);
                        } else {
                            self.partial = part;
                        }
                    } else if b.left() < bits as isize {
                        return Err(malformed("payload runs past the element"));
                    } else {
                        b.skip(bits);
                    }
                }
            }
        }
        b.skip(self.other_data_bits);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A LOAS element with a version-0 StreamMuxConfig (AAC-LC 48 kHz stereo) and one
    /// two-byte payload.
    fn element() -> Vec<u8> {
        let bits = [
            "0",        // useSameStreamMux
            "0",        // audioMuxVersion
            "1",        // allStreamsSameTimeFraming
            "000000",   // numSubFrames
            "0000",     // numProgram
            "000",      // numLayer
            "00010",    // AudioSpecificConfig: object type 2
            "0011",     // 48 kHz
            "0010",     // stereo
            "000",      // GASpecificConfig flags
            "000",      // frameLengthType
            "11111111", // latmBufferFullness
            "0",        // otherDataPresent
            "0",        // crcCheckPresent
            "00000010", // payload length 2
            "1010101011001100",
        ]
        .concat();
        let mut v: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| {
                let mut x = 0u8;
                for (i, &d) in c.iter().enumerate() {
                    x |= (d - b'0') << (7 - i);
                }
                x
            })
            .collect();
        let len = v.len();
        let mut s = vec![0x56, 0xe0 | (len >> 8) as u8, len as u8];
        s.append(&mut v);
        s
    }

    #[test]
    fn reads_a_stream_mux_config_and_payload() {
        let mut stream = element();
        stream.extend(element());
        let els: Vec<&[u8]> = loas_frames(&stream).collect();
        assert_eq!(els.len(), 2);
        let mut l = Latm::new();
        let mut out = Vec::new();
        l.parse(els[0], &mut out).unwrap();
        let c = l.config().unwrap();
        assert_eq!((c.sample_rate, c.core_channels()), (48000, 2));
        assert_eq!(out, vec![vec![0xaa, 0xcc]]);
    }

    #[test]
    fn truncations_do_not_panic() {
        let e = element();
        for n in 0..e.len() {
            let mut l = Latm::new();
            let mut out = Vec::new();
            let _ = l.parse(&e[3..3 + n.saturating_sub(3)], &mut out);
            let _ = loas_frames(&e[..n]).count();
        }
    }

    /// Bytes from a string of 0s and 1s (spaces ignored), zero-padded.
    fn pack(bits: &str) -> Vec<u8> {
        let s: Vec<u8> = bits.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        s.chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |a, (i, &d)| a | ((d - b'0') << (7 - i)))
            })
            .collect()
    }

    /// AAC-LC 48 kHz stereo, and 44.1 kHz mono.
    const ASC_48_2: &str = "00010 0011 0010 000";
    const ASC_44_1: &str = "00010 0100 0001 000";

    #[test]
    fn second_program_is_skipped() {
        // version 0, same framing, one subframe, two programs of one layer each
        let e = pack(
            &[
                "0 0 1 000000 0001",
                "000",
                ASC_48_2,
                "000 11111111", // frameLengthType 0, fullness
                "000 0",        // program 1: one layer, its own config
                ASC_44_1,
                "001 000000011", // frameLengthType 1: (3 + 20) bytes
                "0 0",           // no other data, no CRC
                "00000010",      // program 0: 2 bytes
                "10101010 11001100",
                &"01".repeat(4 * 23), // program 1's 23 bytes
            ]
            .concat(),
        );
        let mut l = Latm::new();
        let mut out = Vec::new();
        l.parse(&e, &mut out).unwrap();
        assert_eq!(out, vec![vec![0xaa, 0xcc]]);
        assert_eq!(l.config().map(|c| c.sample_rate), Some(48000));
    }

    #[test]
    fn separate_framing_joins_chunks() {
        // allStreamsSameTimeFraming 0: an access unit in two chunks, the first not ending it
        let config = ["0 0 0 000000 0000 000", ASC_48_2, "000 11111111 0 0"].concat();
        let mut l = Latm::new();
        let mut out = Vec::new();
        let first = pack(
            &[
                config.as_str(),
                "0001",            // two chunks
                "0000 00000001 0", // stream 0, 1 byte, not the end
                "0000 00000001 1", // stream 0, 1 byte, the end
                "11110000 00001111",
            ]
            .concat(),
        );
        l.parse(&first, &mut out).unwrap();
        assert_eq!(out, vec![vec![0xf0, 0x0f]]);
        // the next element: one chunk that does not end the unit, then one that does
        let a = pack("1 0000 0000 00000001 0 10000001");
        let b = pack("1 0000 0000 00000001 1 01111110");
        l.parse(&a, &mut out).unwrap();
        assert_eq!(out.len(), 1);
        l.parse(&b, &mut out).unwrap();
        assert_eq!(out[1], vec![0x81, 0x7e]);
    }

    #[test]
    fn configuration_changes_are_reported() {
        let element = |asc: &str| {
            pack(
                &[
                    "0 0 1 000000 0000 000",
                    asc,
                    "000 11111111 0 0 00000001 11111111",
                ]
                .concat(),
            )
        };
        let mut l = Latm::new();
        let mut out = Vec::new();
        l.parse(&element(ASC_48_2), &mut out).unwrap();
        l.parse(&element(ASC_48_2), &mut out).unwrap();
        l.parse(&pack("1 00000001 00000000"), &mut out).unwrap();
        l.parse(&element(ASC_44_1), &mut out).unwrap();
        let changes = l.take_changes();
        assert_eq!(out.len(), 4);
        let at: Vec<(usize, u32)> = changes.iter().map(|c| (c.0, c.1.sample_rate)).collect();
        assert_eq!(at, [(0, 48000), (3, 44100)]);
    }

    #[test]
    fn version_a_and_speech_are_refused_by_name() {
        let mut l = Latm::new();
        let mut out = Vec::new();
        let e = l.parse(&pack("0 1 1"), &mut out).unwrap_err();
        assert!(matches!(e, Error::Unsupported(m) if m.contains("version A")));
        // frameLengthType 4 (CELP)
        let e = l
            .parse(
                &pack(&["0 0 1 000000 0000 000", ASC_48_2, "100 000000"].concat()),
                &mut out,
            )
            .unwrap_err();
        assert!(matches!(e, Error::Unsupported(m) if m.contains("CELP")));
    }
}

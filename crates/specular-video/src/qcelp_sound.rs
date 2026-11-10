//! QCELP-13 (`specular-qcelp`) and EVRC (`specular-evrc`) speech for [`crate::audio`]:
//! `.qcp` files (RFC 3625), 3GPP2 `sqcp` / `sevc` sample entries and their `mp4a` forms
//! (object types 0xE1 and 0xA0), QuickTime `Qclp`. A sample holds one or more packets, each
//! a rate octet and its frame (a bare frame is taken by its length).
//!
//! The codecs are `Other(b"qclp", slots)` and `Other(b"sevc", 0)`: `slots` 1 for
//! QuickTime, whose packets each fill a 35-byte slot (the shorter ones padded) and play at
//! the sample entry's rate (Apple's encoder fed 22 050 Hz sound to the 8 kHz codec). SMV
//! (`ssmv`, object type 0xA1) is named only.

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Qcelp,
    Evrc,
}

fn kind(c: &AudioCodec) -> Option<Kind> {
    match c {
        AudioCodec::Other([b'q', b'c', b'l', b'p'], _) => Some(Kind::Qcelp),
        AudioCodec::Other([b's', b'e', b'v', b'c'], _) => Some(Kind::Evrc),
        _ => None,
    }
}

pub(crate) fn is_speech(c: &AudioCodec) -> bool {
    kind(c).is_some()
}

/// The packets of one sample: rate octet + frame each; a sample that is one bare frame
/// is taken whole.
fn packets(k: Kind, s: &[u8]) -> Vec<&[u8]> {
    let len = |b: u8| match k {
        Kind::Qcelp => specular_qcelp::packet_len(b),
        Kind::Evrc => specular_evrc::packet_len(b),
    };
    let bare = match k {
        Kind::Qcelp => specular_qcelp::Rate::from_len(s.len()).is_some(),
        Kind::Evrc => specular_evrc::Rate::from_len(s.len()).is_some(),
    };
    let mut out = Vec::new();
    let mut at = 0;
    while at < s.len() {
        match len(s[at]) {
            Some(n) if at + n <= s.len() => {
                out.push(&s[at..at + n]);
                at += n;
            }
            _ => {
                if at == 0 && bare {
                    out.push(s);
                }
                break;
            }
        }
    }
    out
}

enum Dec {
    Qcelp(Box<specular_qcelp::Decoder>),
    Evrc(Box<specular_evrc::Decoder>),
}

struct Speech {
    kind: Kind,
    dec: Dec,
    /// Packets in fixed 35-byte slots (QuickTime).
    slots: bool,
}

impl SoundDecoder for Speech {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let list = if self.slots {
            sample.chunks(35).collect()
        } else {
            packets(self.kind, sample)
        };
        for p in list {
            match &mut self.dec {
                Dec::Qcelp(d) => d.decode(p, out),
                Dec::Evrc(d) => d.decode(p, out),
            }
        }
        Ok(())
    }
    fn reset(&mut self) {
        match &mut self.dec {
            Dec::Qcelp(d) => d.reset(),
            Dec::Evrc(d) => d.reset(),
        }
    }
    fn preroll(&self) -> usize {
        // the LSP and gain predictors settle within a few frames
        2
    }
}

pub(crate) fn decoder(t: &AudioTrack) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let k = kind(&t.codec).ok_or_else(|| Silence::Unsupported(t.codec.name()))?;
    let dec = match k {
        Kind::Qcelp => Dec::Qcelp(Box::new(specular_qcelp::Decoder::new())),
        Kind::Evrc => Dec::Evrc(Box::new(specular_evrc::Decoder::new())),
    };
    let slots = matches!(t.codec, AudioCodec::Other(_, 1));
    let rate = if slots { t.sample_rate.max(1) } else { 8000 };
    Ok((
        Box::new(Speech {
            kind: k,
            dec,
            slots,
        }),
        1,
        rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_split() {
        let mut s = vec![4u8];
        s.extend([0; 34]);
        s.push(1);
        s.extend([0; 3]);
        s.push(14);
        assert_eq!(packets(Kind::Qcelp, &s).len(), 3);
        // a bare full-rate frame
        assert_eq!(packets(Kind::Qcelp, &[7; 34]).len(), 1);
        // a truncated packet is dropped
        assert_eq!(packets(Kind::Qcelp, &[4, 1, 2, 3, 5]).len(), 0);
        assert_eq!(packets(Kind::Evrc, &[4; 23]).len(), 1);
    }
}

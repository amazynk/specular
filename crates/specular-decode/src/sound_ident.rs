//! Recognising the sound, instrument and recording formats added for the 500-type round, by
//! content. Each test says how sure it is, for the identification layer:
//!
//! | Format | Test | Strength |
//! |---|---|---|
//! | MIDI (`.mid`, `.midi`, `.kar`) | `MThd` and a 6-byte header | magic (certain) |
//! | RIFF MIDI (`.rmi`) | RIFF type `RMID` | magic (certain) |
//! | SoundFont 2 (`.sf2`) | RIFF type `sfbk` | magic (certain) |
//! | DLS (`.dls`) | RIFF type `DLS ` | magic (certain) |
//! | XI (`.xi`) | `Extended Instrument: ` and 0x1A at 43 | magic (certain) |
//! | WTV (`.wtv`, not yet opened) | the WTV file GUID at 0 | magic (certain) |
//! | SFZ (`.sfz`) | text with an SFZ header and an opcode | structure (likely) |
//! | HTK (`.htk`) | its 12-byte header's sizes match the file | structure (likely) |
//! | raw GSM (`.gsm`) | whole 33-byte frames, each led by the 0xD signature | structure (likely) |
//! | signed linear (`.sln`) | none: headerless PCM | extension only (possible) |

/// How sure a test is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    /// A magic number.
    Magic,
    /// A structural check (sizes, repeated signatures).
    Structure,
    /// The extension (or size) alone.
    Extension,
}

/// A Standard MIDI File (`MThd` with its 6-byte header) or RIFF MIDI.
pub fn is_midi(head: &[u8]) -> bool {
    (head.len() >= 14 && head.starts_with(b"MThd\0\0\0\x06")) || riff_type(head) == Some(*b"RMID")
}

fn riff_type(head: &[u8]) -> Option<[u8; 4]> {
    (head.len() >= 12 && &head[..4] == b"RIFF").then(|| head[8..12].try_into().ok())?
}

pub fn is_soundfont(head: &[u8]) -> bool {
    riff_type(head) == Some(*b"sfbk")
}

pub fn is_dls(head: &[u8]) -> bool {
    riff_type(head) == Some(*b"DLS ")
}

pub fn is_xi(head: &[u8]) -> bool {
    head.len() >= 44 && head.starts_with(b"Extended Instrument: ") && head[43] == 0x1A
}

/// The GUID a WTV file starts with.
pub const WTV_GUID: [u8; 16] = [
    0xB7, 0xD8, 0x00, 0x20, 0x37, 0x49, 0xDA, 0x11, 0xA6, 0x4E, 0x00, 0x07, 0xE9, 0x5E, 0xAD, 0x8D,
];

pub fn is_wtv(head: &[u8]) -> bool {
    head.starts_with(&WTV_GUID)
}

/// SFZ text: no NUL bytes, an SFZ header (`<region>`, `<group>`, `<global>`, `<control>`)
/// and an `opcode=value` in the first 16 KiB.
pub fn is_sfz(head: &[u8]) -> bool {
    let h = &head[..head.len().min(16 * 1024)];
    if h.is_empty() || h.contains(&0) {
        return false;
    }
    let s = String::from_utf8_lossy(h);
    ["<region>", "<group>", "<global>", "<control>"]
        .iter()
        .any(|t| s.contains(t))
        && s.contains('=')
}

/// HTK's header (big-endian sample count, period in 100 ns, bytes a sample, kind) whose
/// sizes add up to the whole file `data`.
pub fn is_htk(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    let n = u64::from(u32::from_be_bytes([data[0], data[1], data[2], data[3]]));
    let period = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let size = u64::from(u16::from_be_bytes([data[8], data[9]]));
    let kind = u16::from_be_bytes([data[10], data[11]]);
    // Base kinds 0 (waveform) to 13; the qualifier bits above 6 are flags.
    let base = kind & 0x3F;
    n > 0
        && size > 0
        && size <= 4096
        && (100..=10_000_000).contains(&period)
        && base <= 13
        && 12 + n * size == data.len() as u64
}

/// A plain GSM 06.10 stream: whole 33-byte frames (a cut last one allowed), at least two,
/// each with the 0xD signature (the first 64 checked).
pub fn is_gsm(data: &[u8]) -> bool {
    data.len() / 33 >= 2 && data.chunks_exact(33).take(64).all(|f| f[0] >> 4 == 0xD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tests_tell_formats_apart() {
        assert!(is_midi(b"MThd\0\0\0\x06\0\x01\0\x02\x01\xE0"));
        assert!(!is_midi(b"MThd\0\0\0\x07\0\x01\0\x02\x01\xE0"));
        assert!(is_midi(b"RIFF\x10\0\0\0RMIDdata"));
        assert!(is_soundfont(b"RIFF\x10\0\0\0sfbkLIST"));
        assert!(is_dls(b"RIFF\x10\0\0\0DLS colh"));
        assert!(!is_dls(b"RIFF\x10\0\0\0WAVEfmt "));
        let mut xi = b"Extended Instrument: ".to_vec();
        xi.resize(44, b' ');
        xi[43] = 0x1A;
        assert!(is_xi(&xi));
        assert!(is_sfz(b"<region> sample=a.wav lokey=60"));
        assert!(!is_sfz(b"<html><body>a=b"[..6].as_ref()));
        // 3 samples of 2 bytes at 8 kHz (1250 × 100 ns), waveform.
        let htk = [0, 0, 0, 3, 0, 0, 0x04, 0xE2, 0, 2, 0, 0, 1, 2, 3, 4, 5, 6];
        assert!(is_htk(&htk));
        assert!(!is_htk(&htk[..17]));
        let mut g = vec![0u8; 66];
        g[0] = 0xD1;
        g[33] = 0xD2;
        assert!(is_gsm(&g));
        g[33] = 0x12;
        assert!(!is_gsm(&g));
    }
}

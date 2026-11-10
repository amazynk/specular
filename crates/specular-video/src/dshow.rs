//! DirectShow media types (`AM_MEDIA_TYPE`, as Microsoft documents it): what TV recordings
//! say their streams are. DVR-MS stores them in ASF binary-media stream properties, WTV in
//! its stream descriptor packets.
//!
//! Layout: major type, subtype, fixed-size flag, temporal-compression flag, sample size,
//! format type (GUIDs and 32-bit words, little-endian), the format block's size and the
//! block: `WAVEFORMATEX`, `VIDEOINFOHEADER`, `VIDEOINFOHEADER2` or `MPEG2VIDEOINFO` (whose
//! sequence header is the MPEG-2 stream's). A stream that went through Media Center's copy
//! protection filters says so (`CPFilters_Processed`, or `ETDTFilter_Tagged` from the
//! encrypter/tagger) and carries its own subtype and format type in the last 32 bytes of
//! the block. Whether the samples are encrypted the recording says elsewhere (WTV's
//! attributes, ASF's content encryption objects).

use crate::asf::guid;
use crate::mp4::{AudioCodec, Codec};

const MEDIATYPE_VIDEO: [u8; 16] = guid(b"73646976-0000-0010-8000-00AA00389B71");
const MEDIATYPE_AUDIO: [u8; 16] = guid(b"73647561-0000-0010-8000-00AA00389B71");
const MPEG2_VIDEO: [u8; 16] = guid(b"E06D8026-DB46-11CF-B4D1-00805F6CBBEA");
const MPEG1_PAYLOAD: [u8; 16] = guid(b"E436EB81-524F-11CE-9F53-0020AF0BA770");
const MPEG2_AUDIO: [u8; 16] = guid(b"E06D802B-DB46-11CF-B4D1-00805F6CBBEA");
const DOLBY_AC3: [u8; 16] = guid(b"E06D802C-DB46-11CF-B4D1-00805F6CBBEA");
const DOLBY_AC3_SPDIF: [u8; 16] = guid(b"00000092-0000-0010-8000-00AA00389B71");
const DOLBY_DDPLUS: [u8; 16] = guid(b"A7FB87AF-2D02-42FB-A4D4-05CD93843BDD");
const LATM_AAC: [u8; 16] = guid(b"000001FF-0000-0010-8000-00AA00389B71");
const CP_PROCESSED: [u8; 16] = guid(b"46ADBD28-6FD0-4796-93B2-155C51DC048D");
const ETDT_TAGGED: [u8; 16] = guid(b"C4C4C4D0-0049-4E2B-98FB-9537F6CE516D");
const FORMAT_WAVE: [u8; 16] = guid(b"05589F81-C356-11CE-BF01-00AA0055595A");
const FORMAT_VIDEOINFO: [u8; 16] = guid(b"05589F80-C356-11CE-BF01-00AA0055595A");
const FORMAT_MPEG1VIDEO: [u8; 16] = guid(b"05589F82-C356-11CE-BF01-00AA0055595A");
const FORMAT_VIDEOINFO2: [u8; 16] = guid(b"F72A76A0-EB0A-11D0-ACE4-0000C0CC16BA");
const FORMAT_MPEG2VIDEO: [u8; 16] = guid(b"E06D80E3-DB46-11CF-B4D1-00805F6CBBEA");
/// The tail of a FOURCC subtype (`XXXXXXXX-0000-0010-8000-00AA00389B71`).
const FOURCC_TAIL: [u8; 12] = [
    0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

/// What a media type describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Media {
    Video {
        codec: Codec,
        /// MPEG-1/2: the sequence header from the format block.
        config: Vec<u8>,
        width: u32,
        height: u32,
    },
    Audio {
        codec: AudioCodec,
        channels: u16,
        rate: u32,
    },
    /// Subtitles, teletext, closed captions, data: not played.
    Other,
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn arr(d: &[u8], o: usize) -> Option<[u8; 16]> {
    d.get(o..o + 16)?.try_into().ok()
}

/// The size and the BITMAPINFOHEADER's place in a video format block.
fn bitmap_at(format_type: &[u8; 16]) -> Option<usize> {
    match *format_type {
        FORMAT_VIDEOINFO | FORMAT_MPEG1VIDEO => Some(48),
        FORMAT_VIDEOINFO2 | FORMAT_MPEG2VIDEO => Some(72),
        _ => None,
    }
}

/// The media type at the start of `b`.
pub(crate) fn parse(b: &[u8]) -> Option<Media> {
    let major = arr(b, 0)?;
    let mut sub = arr(b, 16)?;
    let mut format_type = arr(b, 44)?;
    let cb = u32le(b, 60)? as usize;
    let mut format = b.get(64..64usize.checked_add(cb)?).unwrap_or(b.get(64..)?);
    // Wrapped by the copy-protection filters (possibly twice).
    for _ in 0..2 {
        if sub != CP_PROCESSED && sub != ETDT_TAGGED {
            break;
        }
        let n = format.len().checked_sub(32)?;
        sub = arr(format, n)?;
        format_type = arr(format, n + 16)?;
        format = &format[..n];
    }
    let fourcc = (sub[4..] == FOURCC_TAIL).then(|| [sub[0], sub[1], sub[2], sub[3]]);
    let wave = format_type == FORMAT_WAVE;
    let audio_sub = matches!(
        sub,
        MPEG2_AUDIO | DOLBY_AC3 | DOLBY_AC3_SPDIF | DOLBY_DDPLUS | LATM_AAC
    );
    if wave || major == MEDIATYPE_AUDIO || audio_sub {
        let (codec, channels, rate) = if format.len() >= 16 {
            crate::avi::wave_codec(format)?
        } else {
            (AudioCodec::Other(*b"????", 0), 2, 48_000)
        };
        let codec = match sub {
            MPEG2_AUDIO => AudioCodec::Mpa(0),
            DOLBY_AC3 | DOLBY_AC3_SPDIF => AudioCodec::Ac3,
            DOLBY_DDPLUS => AudioCodec::Eac3,
            // LOAS/LATM framing: the configuration is in the stream.
            LATM_AAC => AudioCodec::Aac(Vec::new()),
            _ => codec,
        };
        return Some(Media::Audio {
            codec,
            channels,
            rate,
        });
    }
    let bitmap = bitmap_at(&format_type);
    if major == MEDIATYPE_VIDEO || bitmap.is_some() || sub == MPEG2_VIDEO {
        let codec = match (sub, fourcc) {
            (MPEG2_VIDEO, _) => Codec::Mpeg2,
            (MPEG1_PAYLOAD, _) => Codec::Mpeg1,
            (_, Some(c)) => crate::avi::fourcc_codec(c),
            _ if format_type == FORMAT_MPEG2VIDEO => Codec::Mpeg2,
            _ => Codec::Other(*b"????"),
        };
        let (mut width, mut height) = (0, 0);
        if let Some(at) = bitmap {
            width = u32le(format, at + 4).unwrap_or(0);
            height = (u32le(format, at + 8).unwrap_or(0) as i32).unsigned_abs();
        }
        // MPEG1VIDEOINFO / MPEG2VIDEOINFO: the sequence header after the fixed part.
        let config = match format_type {
            FORMAT_MPEG1VIDEO => {
                let n = u32le(format, 92).unwrap_or(0) as usize;
                format.get(96..96 + n).unwrap_or_default().to_vec()
            }
            FORMAT_MPEG2VIDEO if matches!(codec, Codec::Mpeg2 | Codec::Mpeg1) => {
                let n = u32le(format, 116).unwrap_or(0) as usize;
                format.get(132..132 + n).unwrap_or_default().to_vec()
            }
            _ => Vec::new(),
        };
        return Some(Media::Video {
            codec,
            config,
            width,
            height,
        });
    }
    Some(Media::Other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(major: [u8; 16], sub: [u8; 16], ft: [u8; 16], format: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&major);
        v.extend_from_slice(&sub);
        v.extend_from_slice(&[0; 12]);
        v.extend_from_slice(&ft);
        v.extend_from_slice(&(format.len() as u32).to_le_bytes());
        v.extend_from_slice(format);
        v
    }

    #[test]
    fn ac3_and_wrapped_mpeg2() {
        // DVR-MS: AC-3 with an empty format tag
        let mut wf = vec![0u8; 18];
        wf[2] = 6;
        wf[4..8].copy_from_slice(&48_000u32.to_le_bytes());
        let m = parse(&media([1; 16], DOLBY_AC3, FORMAT_WAVE, &wf)).unwrap();
        assert_eq!(
            m,
            Media::Audio {
                codec: AudioCodec::Ac3,
                channels: 6,
                rate: 48_000
            }
        );
        // WTV: an MPEG2VIDEOINFO behind CPFilters_Processed
        let mut f = vec![0u8; 132];
        f[72 + 4..72 + 8].copy_from_slice(&720u32.to_le_bytes());
        f[72 + 8..72 + 12].copy_from_slice(&576u32.to_le_bytes());
        f[116..120].copy_from_slice(&4u32.to_le_bytes());
        f.extend_from_slice(&[0, 0, 1, 0xB3]);
        f.extend_from_slice(&MPEG2_VIDEO);
        f.extend_from_slice(&FORMAT_MPEG2VIDEO);
        let m = parse(&media(MEDIATYPE_VIDEO, CP_PROCESSED, [9; 16], &f)).unwrap();
        assert_eq!(
            m,
            Media::Video {
                codec: Codec::Mpeg2,
                config: vec![0, 0, 1, 0xB3],
                width: 720,
                height: 576
            }
        );
        // tagged by the encrypter/tagger: unwrapped the same way
        let mut f = wf.clone();
        f.extend_from_slice(&MPEG2_AUDIO);
        f.extend_from_slice(&FORMAT_WAVE);
        let m = parse(&media([2; 16], ETDT_TAGGED, [3; 16], &f)).unwrap();
        assert!(matches!(
            m,
            Media::Audio {
                codec: AudioCodec::Mpa(0),
                ..
            }
        ));
        // truncated input
        assert_eq!(parse(&[0; 20]), None);
    }
}

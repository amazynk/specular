//! Apple ProRes tracks (QuickTime `apco`/`apcs`/`apcn`/`apch`/`ap4h`/`ap4x`, Matroska
//! `V_PRORES`, MXF) through the in-tree decoder `specular-prores`: pictures in the hardware
//! decoder's layout for [`crate::play`], the alpha channel of 4444 frames beside them.
//!
//! The track's configuration is the sample entry's four-character code: it names the
//! profile. ProRes RAW (`aprn`, `aprh`) is a different coding, refused by name.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};
use specular_prores::{Alpha, Header, Interlace};

use crate::mp4::VideoTrack;

/// The profile a four-character code names.
pub fn profile_name(fourcc: &[u8]) -> Option<&'static str> {
    Some(match fourcc {
        b"apco" => "422 Proxy",
        b"apcs" => "422 LT",
        b"apcn" => "422",
        b"apch" => "422 HQ",
        b"ap4h" => "4444",
        b"ap4x" => "4444 XQ",
        _ => return None,
    })
}

/// Whether a four-character code is a ProRes profile this decoder reads.
pub fn is_prores(fourcc: &[u8; 4]) -> bool {
    profile_name(fourcc).is_some()
}

/// The frame header of the track's first sample.
pub fn first_header(data: &[u8], track: &VideoTrack) -> Option<Header> {
    let s = track.samples.first()?;
    let bytes = track.sample_data(data, s)?;
    specular_prores::header(&bytes).ok()
}

/// "4444 XQ, alpha, interlaced": the profile (from the configuration, else the frame's
/// chroma format) and what the first frame says about alpha and scanning.
pub fn profile(data: &[u8], track: &VideoTrack) -> Option<String> {
    let h = first_header(data, track);
    let name = profile_name(&track.config).or(match h.as_ref()?.chroma {
        specular_prores::Chroma::Yuv422 => Some("422"),
        specular_prores::Chroma::Yuv444 => Some("4444"),
    })?;
    let mut s = name.to_owned();
    if let Some(h) = &h {
        if h.alpha != Alpha::None {
            s.push_str(", alpha");
        }
        if h.interlace != Interlace::Progressive {
            s.push_str(", interlaced");
        }
    }
    Some(s)
}

/// Why a ProRes track can't play, from its first sample: what the decoder refuses.
pub fn refusal(data: &[u8], track: &VideoTrack) -> Option<String> {
    let s = track.samples.first()?;
    let bytes = track.sample_data(data, s)?;
    match specular_prores::header(&bytes) {
        Err(specular_prores::Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

/// Whether the first frame codes two fields.
pub fn interlaced(data: &[u8], track: &VideoTrack) -> bool {
    first_header(data, track).is_some_and(|h| h.interlace != Interlace::Progressive)
}

/// Whether the first frame has an alpha channel.
pub fn has_alpha(data: &[u8], track: &VideoTrack) -> bool {
    first_header(data, track).is_some_and(|h| h.alpha != Alpha::None)
}

/// One track's decoder state: the alpha plane of the last frame, until the player takes it.
#[derive(Default)]
pub struct ProResDec {
    alpha: Option<Vec<u8>>,
}

impl ProResDec {
    /// Decodes one frame. A damaged frame costs its picture (`Ok(None)`); a coding the
    /// decoder does not read is an error.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        let o = specular_prores::Options::default();
        match specular_prores::decode(data, &o) {
            Ok(f) => {
                if f.damaged_slices > 0 {
                    specular_log::debug!(target: specular_log::DECODE, "video: ProRes frame with {} damaged slices", f.damaged_slices);
                }
                let h = &f.header;
                let code = |c: u8| (c != 0 && c != 2).then_some(c);
                let signal = Signal {
                    full_range: false,
                    primaries: code(h.primaries),
                    transfer: code(h.transfer),
                    matrix: code(h.matrix),
                };
                self.alpha = f.alpha;
                Ok(Some(VideoFrame {
                    width: f.width,
                    height: f.height,
                    bit_depth: f.bit_depth,
                    chroma: match f.chroma {
                        specular_prores::Chroma::Yuv422 => Chroma::Yuv422,
                        specular_prores::Chroma::Yuv444 => Chroma::Yuv444,
                    },
                    planes: f.planes,
                    signal: Some(signal),
                }))
            }
            Err(specular_prores::Error::Unsupported(m)) => Err(DecodeError::Unsupported(m)),
            Err(specular_prores::Error::NeedsMemory(n)) => {
                self.alpha = None;
                Err(DecodeError::NeedsMemory(n))
            }
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: ProRes frame lost: {e}");
                self.alpha = None;
                Ok(None)
            }
        }
    }

    /// The alpha plane of the frame last decoded (little-endian samples at its depth).
    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

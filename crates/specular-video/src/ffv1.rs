//! FFV1 tracks (Matroska `V_FFV1`, AVI and QuickTime `FFV1`) through the in-tree decoder
//! `specular-ffv1`: pictures in the player's planar layout, transparency beside them.
//!
//! YCbCr stays YCbCr at its depth; greyscale gets neutral chroma (full range, so it shows
//! as coded); RGB comes out as G, B, R planes under the identity matrix.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};

use crate::mp4::VideoTrack;
use crate::play::PlayError;

/// The stream's parameters: from the configuration record, else the first frame.
pub fn params(data: &[u8], track: &VideoTrack) -> Result<specular_ffv1::Params, String> {
    let first = track
        .samples
        .first()
        .and_then(|s| track.sample_data(data, s));
    match specular_ffv1::probe(&track.config, first.as_deref().unwrap_or_default()) {
        Ok(p) => Ok(p),
        Err(specular_ffv1::Error::Unsupported(m)) => Err(m),
        Err(e) => Err(format!("damaged FFV1 video ({e})")),
    }
}

/// "version 3, range coder, 10-bit 4:2:2, alpha, 4 slices, CRC".
pub fn profile(data: &[u8], track: &VideoTrack) -> Option<String> {
    params(data, track).ok().map(|p| p.describe())
}

/// Why an FFV1 track can't play.
pub fn refusal(data: &[u8], track: &VideoTrack) -> Option<String> {
    params(data, track).err()
}

/// One track's decoder and the alpha plane of the last frame.
pub struct Ffv1Dec {
    dec: specular_ffv1::Decoder,
    alpha: Option<Vec<u8>>,
}

fn play_err(e: specular_ffv1::Error) -> PlayError {
    match e {
        specular_ffv1::Error::Unsupported(m) => PlayError::Unsupported(m),
        specular_ffv1::Error::Malformed(m) => PlayError::Malformed(m),
        specular_ffv1::Error::NeedsMemory(n) => PlayError::NeedsMemory(n),
    }
}

impl Ffv1Dec {
    pub fn new(track: &VideoTrack) -> Result<Ffv1Dec, PlayError> {
        Ok(Ffv1Dec {
            dec: specular_ffv1::Decoder::new(track.width, track.height, &track.config)
                .map_err(play_err)?,
            alpha: None,
        })
    }

    pub fn reset(&mut self) {
        self.dec.reset();
        self.alpha = None;
    }

    /// Decodes one frame. A damaged frame costs its picture; a coding the decoder does not
    /// read is an error.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        let p = match self.dec.decode(data) {
            Ok(p) => p,
            Err(specular_ffv1::Error::Unsupported(m)) => return Err(DecodeError::Unsupported(m)),
            Err(specular_ffv1::Error::NeedsMemory(n)) => return Err(DecodeError::NeedsMemory(n)),
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: FFV1 frame lost: {e}");
                self.alpha = None;
                return Ok(None);
            }
        };
        if p.damaged_slices > 0 {
            specular_log::debug!(target: specular_log::DECODE, "video: FFV1 frame with {} damaged slices", p.damaged_slices);
        }
        self.alpha = p.alpha;
        let bits = p.bits as u8;
        let [y, cb, cr] = p.planes;
        Ok(Some(match p.layout {
            specular_ffv1::Layout::Rgb => VideoFrame {
                width: p.width,
                height: p.height,
                bit_depth: bits,
                chroma: Chroma::Yuv444,
                planes: [y, cb, cr],
                signal: Some(Signal {
                    full_range: true,
                    primaries: Some(1),
                    transfer: Some(13),
                    matrix: Some(0),
                }),
            },
            specular_ffv1::Layout::Grey => {
                let (cw, ch) = Chroma::Yuv420.plane_size(p.width, p.height);
                let n = (cw * ch) as usize;
                let mid = 1u16 << (bits - 1);
                let neutral: Vec<u8> = if bits > 8 {
                    mid.to_le_bytes().repeat(n)
                } else {
                    vec![mid as u8; n]
                };
                VideoFrame {
                    width: p.width,
                    height: p.height,
                    bit_depth: bits,
                    chroma: Chroma::Yuv420,
                    planes: [y, neutral.clone(), neutral],
                    signal: Some(Signal {
                        full_range: true,
                        primaries: None,
                        transfer: None,
                        matrix: Some(1),
                    }),
                }
            }
            specular_ffv1::Layout::Ycbcr { log2_h, log2_v } => VideoFrame {
                width: p.width,
                height: p.height,
                bit_depth: bits,
                chroma: match (log2_h, log2_v) {
                    (1, 1) => Chroma::Yuv420,
                    (1, 0) => Chroma::Yuv422,
                    (0, 1) => Chroma::Yuv440,
                    (2, 0) => Chroma::Yuv411,
                    _ => Chroma::Yuv444,
                },
                planes: [y, cb, cr],
                signal: None,
            },
        }))
    }

    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

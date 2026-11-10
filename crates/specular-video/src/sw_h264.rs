//! Software H.264 (`specular-h264`) for clips the hardware decoder can't take: no hardware
//! decoder, a driver without the profile, or a stream it refuses. Pictures come back in decoding order
//! tagged with their sample's composition time; [`crate::play`] puts them in presentation
//! order like the hardware decoder's.

use specular_core::CancelToken;
use specular_decode::DecodeError;
use specular_h264 as h264;
use specular_hwdec::video::{Signal, VideoFrame};
use specular_log::debug;

use crate::play::PlayError;

/// The software decoder of one clip.
pub struct SwH264 {
    dec: h264::Decoder,
    /// The end of the stream was signalled: everything decoded has come out.
    pub flushed: bool,
    /// Bytes of a sample's NAL unit lengths.
    length_size: usize,
}

fn play_err(e: h264::Error) -> PlayError {
    match e {
        h264::Error::Unsupported(s) => PlayError::Unsupported(s),
        h264::Error::Malformed(s) => PlayError::Malformed(s),
        h264::Error::NeedsMemory(n) => PlayError::NeedsMemory(n),
    }
}

impl SwH264 {
    /// A decoder for the track's `avcC` record, using up to 8 threads.
    pub fn new(avcc: &[u8]) -> Result<SwH264, PlayError> {
        SwH264::with_order(avcc, false)
    }

    /// [`SwH264::new`] giving its pictures in presentation order (`display`: for a track timed
    /// by slots, whose samples carry no composition times).
    pub fn with_order(avcc: &[u8], display: bool) -> Result<SwH264, PlayError> {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let opts = h264::Options {
            threads,
            output: if display {
                h264::OutputOrder::Display
            } else {
                h264::OutputOrder::Decode
            },
        };
        let dec = h264::Decoder::with_avcc(avcc, opts).map_err(play_err)?;
        let length_size = h264::parse_avcc(avcc).map_or(4, |(ls, _)| ls);
        Ok(SwH264 {
            dec,
            flushed: false,
            length_size,
        })
    }

    /// Whether `sample` holds only a picture no other picture predicts from: non-IDR slices
    /// with `nal_ref_idc` 0, besides only SEI, access unit delimiters or filler (no
    /// parameter sets, no end of sequence). Such a picture presented before a seek target
    /// need not be decoded at all: a non-reference picture changes neither the reference
    /// list nor PrevRefFrameNum, nor (POC type 0) the POC predictor.
    pub fn unreferenced(&self, sample: &[u8]) -> bool {
        let ls = self.length_size;
        let (mut p, mut vcl) = (0, false);
        while p + ls <= sample.len() {
            let n = sample[p..p + ls]
                .iter()
                .fold(0usize, |a, &b| (a << 8) | b as usize);
            p += ls;
            if n == 0 || n > sample.len() - p {
                return false;
            }
            let h = sample[p];
            match (h >> 5 & 3, h & 31) {
                (0, 1) => vcl = true,
                (_, 6 | 9 | 12) => {}
                _ => return false,
            }
            p += n;
        }
        vcl && p == sample.len()
    }

    /// The decoder's state between two samples (see [`h264::Decoder::snapshot`]).
    pub(crate) fn snapshot(&self) -> Option<h264::Snapshot> {
        self.dec.snapshot()
    }

    /// Continues from `snapshot`: the samples that followed it come next.
    pub(crate) fn restore(&mut self, snapshot: &h264::Snapshot) {
        self.dec.restore(snapshot);
        self.flushed = false;
    }

    /// Decodes one sample with composition time `cts` (`None`: the end of the stream) and
    /// moves the finished pictures, with their composition times, to `out`.
    pub fn step(
        &mut self,
        sample: Option<(&[u8], i64)>,
        cancel: &CancelToken,
        out: &mut Vec<(i64, VideoFrame)>,
    ) -> Result<(), DecodeError> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let wait = match sample {
            Some((bytes, cts)) => {
                match self.dec.decode_sample(bytes, cts) {
                    Ok(()) => {}
                    Err(h264::Error::Unsupported(m)) => return Err(DecodeError::Unsupported(m)),
                    Err(h264::Error::NeedsMemory(n)) => return Err(DecodeError::NeedsMemory(n)),
                    // Only this picture suffers; later ones predict from what was decoded.
                    Err(e) => {
                        debug!(target: specular_log::DECODE, "motion clip: sample at {cts}: {e}");
                    }
                }
                false
            }
            None => {
                self.dec.flush();
                self.flushed = true;
                true
            }
        };
        while let Some(p) = self.dec.next_picture(wait) {
            out.push((p.tag, frame(p)));
        }
        Ok(())
    }
}

/// A decoded picture in the hardware decoder's layout.
fn frame(p: h264::Picture) -> VideoFrame {
    VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: p.bit_depth,
        chroma: match p.chroma {
            h264::Chroma::Yuv420 => specular_hwdec::video::Chroma::Yuv420,
            h264::Chroma::Yuv422 => specular_hwdec::video::Chroma::Yuv422,
            h264::Chroma::Yuv444 => specular_hwdec::video::Chroma::Yuv444,
        },
        planes: p.planes,
        signal: p.signal.map(|s| Signal {
            full_range: s.full_range,
            primaries: s.colour.map(|c| c.0),
            transfer: s.colour.map(|c| c.1),
            matrix: s.colour.map(|c| c.2),
        }),
    }
}

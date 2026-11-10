//! MPEG-4 Part 2 and H.263 video (`specular-mpeg4`) and VC-1 (`specular-vc1`) for
//! [`crate::play`].
//!
//! With presentation times (MP4, 3GPP, Matroska), pictures come out in decoding order tagged
//! with their sample's composition time. AVI has only time slots: pictures come out in display
//! order and take the slots in turn (a DivX packed chunk's placeholder gives its slot to the
//! picture shown there).

use std::collections::BTreeSet;

use specular_core::CancelToken;
use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};
use specular_mpeg4 as m4;

use crate::mp4::{Codec, VideoTrack};
use crate::play::PlayError;

/// The decoder behind a [`Mpeg4Dec`]: both take coded frames, give tagged pictures.
enum Inner {
    Mpeg4(Box<m4::Decoder>),
    Vc1(Box<specular_vc1::Decoder>),
}

pub struct Mpeg4Dec {
    dec: Inner,
    pub flushed: bool,
    /// Slot timing: the times of samples fed whose slot no picture took yet.
    slots: Option<BTreeSet<i64>>,
    h263: bool,
}

impl Mpeg4Dec {
    pub fn new(track: &VideoTrack) -> Result<Mpeg4Dec, PlayError> {
        if track.codec == Some(Codec::Vc1) {
            let order = if track.slot_timing {
                specular_vc1::OutputOrder::Display
            } else {
                specular_vc1::OutputOrder::Decode
            };
            let dec =
                specular_vc1::Decoder::new(&vc1_config(track), order).map_err(|e| match e {
                    specular_vc1::Error::Unsupported(m) => PlayError::Unsupported(m),
                    specular_vc1::Error::Malformed(m) => PlayError::Malformed(m),
                    specular_vc1::Error::NeedsMemory(n) => PlayError::NeedsMemory(n),
                })?;
            return Ok(Mpeg4Dec {
                dec: Inner::Vc1(Box::new(dec)),
                flushed: false,
                slots: track.slot_timing.then(BTreeSet::new),
                h263: false,
            });
        }
        let order = if track.slot_timing {
            m4::OutputOrder::Display
        } else {
            m4::OutputOrder::Decode
        };
        let mut dec = m4::Decoder::new(order);
        dec.set_aligned(true);
        dec.set_sorenson(track.codec == Some(Codec::Flv1));
        if !track.config.is_empty() && track.codec == Some(Codec::Mpeg4) {
            dec.decode(&track.config, i64::MIN).map_err(|e| match e {
                m4::Error::Unsupported(m) => PlayError::Unsupported(m),
                m4::Error::Malformed(m) => PlayError::Malformed(m),
            })?;
        }
        Ok(Mpeg4Dec {
            dec: Inner::Mpeg4(Box::new(dec)),
            flushed: false,
            slots: track.slot_timing.then(BTreeSet::new),
            h263: matches!(track.codec, Some(Codec::H263 | Codec::Flv1)),
        })
    }

    pub fn step(
        &mut self,
        sample: Option<(&[u8], i64)>,
        cancel: &CancelToken,
        out: &mut Vec<(i64, VideoFrame)>,
    ) -> Result<(), DecodeError> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        match sample {
            Some((bytes, cts)) => {
                if let Some(s) = self.slots.as_mut() {
                    s.insert(cts);
                }
                match &mut self.dec {
                    Inner::Mpeg4(d) => d.decode(bytes, cts).map_err(|e| match e {
                        m4::Error::Unsupported(m) => DecodeError::Unsupported(m),
                        m4::Error::Malformed(m) => DecodeError::Malformed(m),
                    })?,
                    Inner::Vc1(d) => match d.decode(bytes, cts) {
                        Ok(()) => {}
                        Err(specular_vc1::Error::Unsupported(m)) => {
                            return Err(DecodeError::Unsupported(m));
                        }
                        Err(specular_vc1::Error::NeedsMemory(n)) => {
                            return Err(DecodeError::NeedsMemory(n));
                        }
                        // A damaged frame costs its picture; the next ones still decode.
                        Err(e) => {
                            specular_log::debug!(target: specular_log::DECODE, "video: VC-1 frame lost: {e}");
                        }
                    },
                }
            }
            None => {
                match &mut self.dec {
                    Inner::Mpeg4(d) => d.flush(),
                    Inner::Vc1(d) => d.flush(),
                }
                self.flushed = true;
            }
        }
        while let Some((tag, f)) = self.next() {
            if tag == i64::MIN {
                continue;
            }
            let tag = match self.slots.as_mut() {
                Some(s) => match s.pop_first() {
                    Some(t) => t,
                    None => continue,
                },
                None => tag,
            };
            out.push((tag, f));
        }
        Ok(())
    }

    /// The next picture out of the decoder, with its tag.
    fn next(&mut self) -> Option<(i64, VideoFrame)> {
        match &mut self.dec {
            Inner::Mpeg4(d) => d.next_picture().map(|p| (p.tag, frame(p))),
            Inner::Vc1(d) => d.next_picture().map(|p| (p.tag, vc1_frame(p))),
        }
    }

    /// MPEG-4 chroma sits on even luma columns; H.263's between them.
    pub fn cosited_x(&self) -> bool {
        !self.h263
    }
}

fn frame(p: m4::Picture) -> VideoFrame {
    VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: 8,
        chroma: Chroma::Yuv420,
        planes: p.planes,
        signal: Some(Signal {
            full_range: p.full_range,
            primaries: p.colour.map(|c| c.0),
            transfer: p.colour.map(|c| c.1),
            matrix: p.colour.map(|c| c.2),
        }),
    }
}

fn vc1_frame(p: specular_vc1::Picture) -> VideoFrame {
    VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: 8,
        chroma: Chroma::Yuv420,
        planes: p.planes,
        signal: Some(Signal {
            full_range: p.full_range,
            primaries: p.colour.map(|c| c.0),
            transfer: p.colour.map(|c| c.1),
            matrix: p.colour.map(|c| c.2),
        }),
    }
}

/// A VC-1 track's configuration: start-code headers (WVC1, MP4 `dvc1`, transport streams:
/// possibly none, then they come in band) or STRUCT_C (WMV3) with the container's size.
pub fn vc1_config(track: &VideoTrack) -> specular_vc1::Config<'_> {
    let c = &track.config;
    let start_codes = c
        .windows(4)
        .any(|w| w[..3] == [0, 0, 1] && (w[3] == 0x0F || w[3] == 0x0E));
    if start_codes || c.len() < 4 {
        specular_vc1::Config::Advanced { headers: c }
    } else {
        specular_vc1::Config::SimpleMain {
            struct_c: &c[..4],
            width: track.width,
            height: track.height,
        }
    }
}

/// "Main profile", "Advanced profile @ Level 3"... from a VC-1 track's configuration.
pub fn vc1_profile(track: &VideoTrack) -> Option<String> {
    let i = specular_vc1::probe(&vc1_config(track)).ok()?;
    Some(match i.level {
        Some(l) => format!("{} profile @ Level {l}", i.profile),
        None => format!("{} profile", i.profile),
    })
}

/// Why a VC-1 track can't play: its configuration refused, else its first frame.
pub fn vc1_refusal(track: &VideoTrack, data: &[u8]) -> Option<String> {
    let mut dec =
        match specular_vc1::Decoder::new(&vc1_config(track), specular_vc1::OutputOrder::Decode) {
            Ok(d) => d,
            Err(specular_vc1::Error::Unsupported(m)) => return Some(m),
            Err(_) => return None,
        };
    let s = track.samples.iter().find(|s| s.sync)?;
    match dec.decode(&track.sample_data(data, s)?, 0) {
        Err(specular_vc1::Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

/// The layer of a track: its configuration record, else its first sample.
fn vol(track: &VideoTrack, data: &[u8]) -> Option<m4::Vol> {
    m4::probe(&track.config).or_else(|| {
        let s = track.samples.first()?;
        let b = track.sample_data(data, s)?;
        m4::probe(&b)
    })
}

/// The video object layer's pixel aspect where the container gives none (AVI has no field
/// for it; FFmpeg and DivX write it in the layer only).
pub fn complete_track(data: &[u8], track: &mut VideoTrack) {
    if track.pixel_aspect.is_some() || track.codec != Some(Codec::Mpeg4) {
        return;
    }
    if let Some(par) = vol(track, data).and_then(|v| v.pixel_aspect) {
        track.pixel_aspect = Some(par);
        track.display = track.shown_size();
    }
}

/// Why an MPEG-4 Part 2 / H.263 track can't play, from its layer and first keyframe.
pub fn refusal(config: &[u8], first_key: &[u8]) -> Option<String> {
    let v = m4::probe(config).or_else(|| m4::probe(first_key))?;
    v.refusal().map(str::to_owned)
}

/// Why an MPEG-4 Part 2 track can't be decoded at all: no video object layer header in the
/// container's configuration nor before its key frames (Matroska without CodecPrivate).
/// Every picture needs one before it.
pub fn no_vol_header(data: &[u8], track: &VideoTrack) -> Option<String> {
    if track.codec != Some(Codec::Mpeg4)
        || m4::probe(&track.config).is_some()
        || crate::mpeg::any_key_start(data, track, |b| m4::probe(b).is_some())
    {
        return None;
    }
    Some("MPEG-4 Part 2 video without its stream header (VOL)".into())
}

/// "Advanced Simple Profile @ Level 5", "H.263 baseline", "Sorenson Spark, version 1"...
pub fn profile(track: &VideoTrack, data: &[u8]) -> Option<String> {
    if track.codec == Some(Codec::Flv1) {
        let s = track.samples.first()?;
        return m4::probe_sorenson(&track.sample_data(data, s)?)?.profile_name();
    }
    vol(track, data)?.profile_name()
}

/// Why a Sorenson Spark track can't play, from its first frame.
pub fn sorenson_refusal(first: &[u8]) -> Option<String> {
    m4::sorenson_refusal(first)
}

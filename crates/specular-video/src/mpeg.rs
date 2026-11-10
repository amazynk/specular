//! MPEG-1 and MPEG-2 video (`specular-mpeg2`) for [`crate::play`]: pictures in decoding order
//! tagged with their sample's composition time, like the software H.264 decoder's.
//!
//! A sample may hold one field picture (transport streams): [`field_scan`] says which samples
//! complete a frame, so the timeline has one entry per frame. Frames whose picture says
//! `progressive_frame` are not deinterlaced.

use std::collections::HashSet;

use specular_core::CancelToken;
use specular_decode::DecodeError;
use specular_h264::SampleFields;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};
use specular_mpeg2 as m2;

use crate::mp4::VideoTrack;
use crate::play::PlayError;

/// One clip's MPEG-1/2 decoder.
pub struct Mpeg2Dec {
    dec: m2::Decoder,
    /// The end of the stream was signalled: everything decoded has come out.
    pub flushed: bool,
    /// Tags of frames coded as progressive (not to be deinterlaced).
    progressive: HashSet<i64>,
    mpeg1: bool,
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

impl Mpeg2Dec {
    /// A decoder primed with the track's sequence header (when the container gives one).
    pub fn new(config: &[u8]) -> Result<Mpeg2Dec, PlayError> {
        let mut dec = m2::Decoder::new(threads(), m2::OutputOrder::Decode);
        dec.set_aligned(true);
        if !config.is_empty() {
            dec.decode(config, i64::MIN).map_err(play_err)?;
        }
        Ok(Mpeg2Dec {
            dec,
            flushed: false,
            progressive: HashSet::new(),
            mpeg1: m2::probe(config).is_some_and(|s| !s.mpeg2),
        })
    }

    /// Decodes one sample (`None`: the end of the stream) and moves the finished pictures,
    /// with their composition times, to `out`.
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
                if let Err(e) = self.dec.decode(bytes, cts) {
                    return Err(match e {
                        m2::Error::Unsupported(m) => DecodeError::Unsupported(m),
                        m2::Error::Malformed(m) => DecodeError::Malformed(m),
                    });
                }
            }
            None => {
                self.dec.flush();
                self.flushed = true;
            }
        }
        while let Some(p) = self.dec.next_picture() {
            if p.tag == i64::MIN {
                continue;
            }
            self.mpeg1 = p.mpeg1;
            if p.progressive {
                self.progressive.insert(p.tag);
            }
            out.push((p.tag, frame(p)?));
        }
        Ok(())
    }

    /// The decoder's state between two samples (see [`m2::Decoder::snapshot`]).
    pub(crate) fn snapshot(&self) -> Option<m2::Snapshot> {
        self.dec.snapshot()
    }

    /// Continues from `snapshot`: the samples that followed it come next.
    pub(crate) fn restore(&mut self, snapshot: &m2::Snapshot) {
        self.dec.restore(snapshot);
        self.flushed = false;
    }

    /// Whether the frame with this composition time was coded progressive.
    pub fn progressive(&self, cts: i64) -> bool {
        self.progressive.contains(&cts)
    }

    /// MPEG-1 chroma sits between luma columns; MPEG-2's on even ones.
    pub fn cosited_x(&self) -> bool {
        !self.mpeg1
    }
}

fn play_err(e: m2::Error) -> PlayError {
    match e {
        m2::Error::Unsupported(s) => PlayError::Unsupported(s),
        m2::Error::Malformed(s) => PlayError::Malformed(s),
    }
}

fn frame(p: m2::Picture) -> Result<VideoFrame, DecodeError> {
    let chroma = match p.chroma {
        m2::Chroma::Yuv420 => Chroma::Yuv420,
        m2::Chroma::Yuv422 => Chroma::Yuv422,
        m2::Chroma::Yuv444 => Chroma::Yuv444,
    };
    Ok(VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: 8,
        chroma,
        planes: p.planes,
        // Always studio range; the colour description when the stream has one.
        signal: Some(Signal {
            full_range: false,
            primaries: p.colour.map(|c| c.0),
            transfer: p.colour.map(|c| c.1),
            matrix: p.colour.map(|c| c.2),
        }),
    })
}

/// Which samples of an MPEG-2 track start, complete or leave open a frame (empty for a
/// progressive sequence, or when the samples hold whole frames).
pub fn field_scan(data: &[u8], track: &VideoTrack) -> Vec<SampleFields> {
    let seq = m2::probe(&track.config);
    if seq.as_ref().is_some_and(|s| s.progressive || !s.mpeg2) {
        return Vec::new();
    }
    let mut open: Option<m2::Structure> = None;
    let mut out = Vec::with_capacity(track.samples.len());
    let mut any_split = false;
    for s in &track.samples {
        let head = track
            .sample_data(data, s)
            .map(|b| m2::scan(&b[..b.len().min(1 << 16)]).pictures)
            .unwrap_or_default();
        let mut f = SampleFields {
            starts_frame: false,
            completes_open: false,
            leaves_open: false,
            idr: head.first().is_some_and(|p| p.kind == m2::PictureType::I),
        };
        for (k, p) in head.iter().enumerate() {
            match (open, p.structure) {
                (Some(o), st) if st != m2::Structure::Frame && st != o => {
                    if k == 0 {
                        f.completes_open = true;
                    }
                    open = None;
                }
                (_, m2::Structure::Frame) => {
                    f.starts_frame = true;
                    open = None;
                }
                (_, st) => {
                    f.starts_frame = true;
                    open = Some(st);
                }
            }
        }
        if head.is_empty() {
            // Nothing readable: a frame of its own.
            f.starts_frame = true;
        }
        f.leaves_open = open.is_some();
        any_split |= f.leaves_open || f.completes_open;
        out.push(f);
    }
    if any_split { out } else { Vec::new() }
}

/// Why an MPEG-1/2 track can't play, from its sequence header and first keyframe.
pub fn refusal(config: &[u8], first_key: &[u8]) -> Option<String> {
    let s = m2::scan(first_key);
    let seq = m2::probe(config).or(s.sequence)?;
    seq.refusal()
}

/// The key frames looked at, and the bytes at the start of each, for a stream header the
/// container doesn't give (it comes before the picture).
pub(crate) const HEADER_LOOK: (usize, usize) = (256, 4096);

/// The key frames (every sample when none is marked) whose start is looked at for a stream
/// header the container doesn't give, at most [`HEADER_LOOK`].
pub(crate) fn any_key_start(data: &[u8], v: &VideoTrack, found: impl Fn(&[u8]) -> bool) -> bool {
    let marked = v.samples.iter().any(|s| s.sync);
    v.samples
        .iter()
        .filter(|s| s.sync || !marked)
        .take(HEADER_LOOK.0)
        .any(|s| {
            v.sample_data(data, s)
                .is_some_and(|b| found(&b[..b.len().min(HEADER_LOOK.1)]))
        })
}

/// Why an MPEG-1/2 track can't be decoded at all: no sequence header in the container's
/// configuration nor before its key frames (Matroska without CodecPrivate, frames cut from
/// a stream past its header). Every picture needs one before it.
pub fn no_sequence_header(data: &[u8], v: &VideoTrack) -> Option<String> {
    if m2::probe(&v.config).is_some() || any_key_start(data, v, |b| m2::probe(b).is_some()) {
        return None;
    }
    let name = v.codec.map_or_else(|| "MPEG".into(), |c| c.name());
    Some(format!("{name} video without its sequence header"))
}

/// Fills an MPEG-1/2 track's sequence header, size, pixel aspect and interlacing from its
/// first sample when the container doesn't give them (transport and programme streams,
/// MP4 and Matroska without a configuration record).
pub fn complete_track(data: &[u8], v: &mut VideoTrack) {
    if v.config.is_empty() {
        let bytes = v.samples.iter().take(64).find_map(|s| {
            let b = v.sample_data(data, s)?;
            m2::sequence_bytes(&b).map(<[u8]>::to_vec)
        });
        if let Some(b) = bytes {
            v.config = b;
        }
    }
    let Some(seq) = m2::probe(&v.config) else {
        return;
    };
    if v.width == 0 || v.height == 0 {
        (v.width, v.height) = (seq.width, seq.height);
    }
    if v.display == (0, 0) {
        v.display = (seq.width, seq.height);
    }
    if v.pixel_aspect.is_none()
        && let Some(par) = seq.pixel_aspect()
    {
        v.pixel_aspect = Some(par);
        v.display = v.shown_size();
    }
    if seq.mpeg2 && !seq.progressive {
        v.interlaced = true;
    }
}

/// The track's profile and level, e.g. "Main Profile @ High Level" (MPEG-2 only).
pub fn profile(track: &VideoTrack) -> Option<String> {
    m2::probe(&track.config)?.profile_name()
}

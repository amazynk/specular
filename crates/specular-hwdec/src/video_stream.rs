//! The device-independent part of hardware video decoding, shared by the VA-API and D3D12
//! decoders (`vaapi::video`, `d3d12::video`): the stream state behind either codec, and the
//! output queue with HEVC's held pictures (read back in decoding order, handed out once the
//! stream's output process has decided they are shown).

use std::collections::VecDeque;

use crate::Error;
use crate::h264::H264Stream;
use crate::hevc::Reject;
use crate::hevc_video::HevcStream;
use crate::video::{Signal, VideoFrame};

pub(crate) enum Stream {
    Hevc(HevcStream),
    H264(H264Stream),
}

impl Stream {
    pub fn coded(&self) -> (u32, u32) {
        match self {
            Stream::Hevc(s) => s.coded,
            Stream::H264(s) => s.coded,
        }
    }
    pub fn ten_bit(&self) -> bool {
        match self {
            Stream::Hevc(s) => s.bit_depth > 8,
            Stream::H264(_) => false,
        }
    }
    pub fn surfaces_needed(&self) -> usize {
        match self {
            Stream::Hevc(s) => s.surfaces_needed(),
            Stream::H264(s) => s.surfaces_needed(),
        }
    }
    pub fn set_slots(&mut self, n: usize) {
        match self {
            Stream::Hevc(s) => s.set_slots(n),
            Stream::H264(s) => s.set_slots(n),
        }
    }
    pub fn reset(&mut self) {
        match self {
            Stream::Hevc(s) => s.reset(),
            Stream::H264(s) => s.reset(),
        }
    }
    pub fn forget_last(&mut self) {
        match self {
            Stream::Hevc(s) => s.forget_last(),
            Stream::H264(s) => s.forget_last(),
        }
    }
    pub fn crop(&self) -> [u32; 4] {
        match self {
            Stream::Hevc(s) => s.crop,
            Stream::H264(s) => s.crop,
        }
    }
    pub fn signal(&self) -> Option<Signal> {
        match self {
            Stream::Hevc(s) => s.signal.map(|v| Signal {
                full_range: v.video_full_range_flag,
                primaries: v.colour_description.map(|c| c.colour_primaries),
                transfer: v.colour_description.map(|c| c.transfer_characteristics),
                matrix: v.colour_description.map(|c| c.matrix_coeffs),
            }),
            Stream::H264(s) => s.signal.map(|v| Signal {
                full_range: v.full_range,
                primaries: v.colour.map(|c| c.0),
                transfer: v.colour.map(|c| c.1),
                matrix: v.colour.map(|c| c.2),
            }),
        }
    }

    /// The read-back region of a decoded picture: the conformance window, its top-left
    /// rounded to even samples (the chroma grid).
    pub fn region(&self) -> (u32, u32, u32, u32) {
        let [l, r, t, b] = self.crop();
        let (w, h) = self.coded();
        let (x, y) = (l & !1, t & !1);
        (x, y, w - r - x, h - b - y)
    }
}

/// A decoded HEVC picture whose output is not decided yet.
struct Held {
    /// [`crate::hevc_video::HevcJob::id`].
    id: u64,
    tag: i64,
    frame: VideoFrame,
    /// Output (`Some(true)`), discarded (`Some(false)`), or not decided yet.
    fate: Option<bool>,
}

pub(crate) fn reject(r: Reject) -> Error {
    match r {
        Reject::Unsupported(s) => Error::Unsupported(s),
        Reject::Malformed(s) => Error::Malformed(s),
    }
}

/// The pictures on their way out.
#[derive(Default)]
pub(crate) struct Output {
    /// HEVC pictures in decoding order, held until their output is decided.
    held: VecDeque<Held>,
    /// Pictures to hand out, tagged, in decoding order.
    ready: VecDeque<(i64, VideoFrame)>,
}

impl Output {
    /// Forgets the reference pictures and the pictures not handed out yet: the next sample
    /// must be a sync sample.
    pub fn reset(&mut self, stream: &mut Stream) {
        stream.reset();
        if let Stream::Hevc(st) = stream {
            st.take_output();
        }
        self.held.clear();
        self.ready.clear();
    }

    /// The end of the stream: the pictures still held back are output.
    pub fn flush(&mut self, stream: &mut Stream) {
        if let Stream::Hevc(st) = stream {
            st.flush();
        }
        self.settle(stream);
    }

    /// The next picture to show, with its sample's tag (decoding order).
    pub fn next(&mut self) -> Option<(i64, VideoFrame)> {
        self.ready.pop_front()
    }

    /// A decoded picture: held when it is an HEVC picture (`hevc_id`) whose output is not
    /// decided, ready otherwise.
    pub fn push(&mut self, hevc_id: Option<u64>, tag: i64, frame: VideoFrame) {
        match hevc_id {
            Some(id) => self.held.push_back(Held {
                id,
                tag,
                frame,
                fate: None,
            }),
            None => self.ready.push_back((tag, frame)),
        }
    }

    /// Applies the HEVC stream's output decisions and moves the decided pictures at the front
    /// of the held ones to the ready ones (the discarded ones are dropped).
    pub fn settle(&mut self, stream: &mut Stream) {
        if let Stream::Hevc(st) = stream {
            for (id, out) in st.take_output() {
                if let Some(h) = self.held.iter_mut().find(|h| h.id == id) {
                    h.fate = Some(out);
                }
            }
        }
        while let Some(out) = self.held.front().and_then(|h| h.fate) {
            let h = self.held.pop_front().expect("a held picture");
            if out {
                self.ready.push_back((h.tag, h.frame));
            }
        }
    }
}

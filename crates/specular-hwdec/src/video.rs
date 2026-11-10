//! Hardware video decode for motion photos and Live Photos (DESIGN.md §12.3): H.264 / HEVC
//! samples from an ISO-BMFF track, in decode order, to 8- or 10-bit 4:2:0 planes. H.264 takes
//! progressive, MBAFF and field (PAFF) 8-bit 4:2:0 streams; HEVC Main and Main 10. Without a
//! hardware decoder, or for a stream it refuses (H.264 High 10 and 4:2:2, HEVC 4:2:2), the
//! clip decodes in software (specular-video), which also gives 4:2:2 frames.
//!
//! As with the still-image paths, every parameter set and slice header is parsed in safe Rust
//! and checked before the driver sees it; the engine gets validated parameter structs and the
//! slice data.

use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
}

/// The stream's video signal type (VUI), when it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal {
    pub full_range: bool,
    pub primaries: Option<u8>,
    pub transfer: Option<u8>,
    pub matrix: Option<u8>,
}

/// Chroma sampling of a [`VideoFrame`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    /// Chroma planes `ceil(w/2)`×`ceil(h/2)`.
    Yuv420,
    /// Chroma planes `ceil(w/2)`×`h`.
    Yuv422,
    /// Chroma planes `w`×`h` (software decoders only).
    Yuv444,
    /// Chroma planes `w`×`ceil(h/2)` (software decoders only: VP9 profile 1, Motion JPEG).
    Yuv440,
    /// Chroma planes `ceil(w/4)`×`h` (software decoders only: DV NTSC, Motion JPEG).
    Yuv411,
}

impl Chroma {
    /// Luma samples per chroma sample across and down.
    pub fn divisors(self) -> (u32, u32) {
        match self {
            Chroma::Yuv420 => (2, 2),
            Chroma::Yuv422 => (2, 1),
            Chroma::Yuv444 => (1, 1),
            Chroma::Yuv440 => (1, 2),
            Chroma::Yuv411 => (4, 1),
        }
    }

    /// The chroma planes' size for a `w`×`h` picture.
    pub fn plane_size(self, w: u32, h: u32) -> (u32, u32) {
        let (hd, vd) = self.divisors();
        (w.div_ceil(hd), h.div_ceil(vd))
    }
}

/// One decoded picture: tightly packed Y, Cb, Cr planes (sized by `chroma`), cropped to the
/// conformance window; 8-bit samples as bytes, deeper ones (up to 14 bits) as little-endian
/// `u16`.
#[derive(Debug)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub chroma: Chroma,
    pub planes: [Vec<u8>; 3],
    pub signal: Option<Signal>,
}

/// A decoder for one track.
pub struct VideoDecoder {
    #[cfg(target_os = "linux")]
    va: crate::vaapi::video::VaVideo,
    #[cfg(windows)]
    d3d: crate::d3d12::video::D3dVideo,
}

impl VideoDecoder {
    /// Opens a decoder for a track with this configuration record (`avcC` / `hvcC` body).
    /// [`Error::Unavailable`] when hardware decoding is off or there is no device;
    /// [`Error::Unsupported`] when the device can't take this stream.
    pub fn new(codec: VideoCodec, config: &[u8]) -> Result<VideoDecoder, Error> {
        if !crate::switched_on() {
            return Err(Error::Unavailable);
        }
        #[cfg(target_os = "linux")]
        {
            Ok(VideoDecoder {
                va: crate::vaapi::video::VaVideo::open(codec, config)?,
            })
        }
        #[cfg(windows)]
        {
            Ok(VideoDecoder {
                d3d: crate::d3d12::video::D3dVideo::open(codec, config)?,
            })
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = (codec, config);
            Err(Error::Unavailable)
        }
    }

    /// Decodes the next sample in decode order. Its picture, if it is shown at all, comes out
    /// of [`Self::next_output`] tagged with `tag`; pictures come out in decode order, but an
    /// HEVC picture may come out only after later samples are decoded (or at [`Self::flush`]),
    /// once the stream's output process (H.265 C.5.2) has decided it is shown.
    pub fn decode(&mut self, sample: &[u8], tag: i64) -> Result<(), Error> {
        self.decode_shown(sample, tag, true)
    }

    /// [`Self::decode`] for a sample whose picture the caller will not show (one before a
    /// seek target): it is decoded for reference but never read back from the device and
    /// never comes out of [`Self::next_output`]. Reading a 4K picture back costs more than
    /// decoding it.
    pub fn decode_unshown(&mut self, sample: &[u8], tag: i64) -> Result<(), Error> {
        self.decode_shown(sample, tag, false)
    }

    fn decode_shown(&mut self, sample: &[u8], tag: i64, show: bool) -> Result<(), Error> {
        if !crate::switched_on() {
            return Err(Error::Unavailable);
        }
        #[cfg(target_os = "linux")]
        {
            self.va.decode(sample, tag, show)
        }
        #[cfg(windows)]
        {
            self.d3d.decode(sample, tag, show)
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = (sample, tag, show);
            Err(Error::Unavailable)
        }
    }

    /// The next decoded picture to show, with the tag of its sample.
    pub fn next_output(&mut self) -> Option<(i64, VideoFrame)> {
        #[cfg(target_os = "linux")]
        {
            self.va.next_output()
        }
        #[cfg(windows)]
        {
            self.d3d.next_output()
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        None
    }

    /// The end of the stream: the pictures still held back become available from
    /// [`Self::next_output`].
    pub fn flush(&mut self) {
        #[cfg(target_os = "linux")]
        self.va.flush();
        #[cfg(windows)]
        self.d3d.flush();
    }

    /// Forgets the reference pictures and the pictures not handed out yet, before decoding
    /// from a sync sample again (looping, seeking).
    pub fn reset(&mut self) {
        #[cfg(target_os = "linux")]
        self.va.reset();
        #[cfg(windows)]
        self.d3d.reset();
    }
}

/// Whether this machine offers a hardware decoder for `codec` (from the cached probe; opens
/// the device on first use, so call it off the UI thread).
pub fn video_supported(codec: VideoCodec) -> bool {
    if !crate::switched_on() {
        return false;
    }
    crate::probe().is_some_and(|c| match codec {
        VideoCodec::Hevc => c.hevc.is_some(),
        VideoCodec::H264 => c.h264.is_some() || c.others.contains(&"H.264"),
    })
}

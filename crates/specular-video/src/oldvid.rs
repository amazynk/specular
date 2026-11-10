//! The old codecs (QuickTime Animation, Cinepak, Microsoft Video 1 and RLE, Apple SMC and
//! RPZA) through the in-tree decoder `specular-oldvideo`: the canvas in the codec's own
//! format becomes G, B, R planes under the identity matrix (palettes looked up, RGB555
//! widened), ARGB's alpha beside them.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};
use specular_oldvideo::Pixels;

use crate::mp4::{Classic, VideoTrack};
use crate::play::PlayError;
use crate::rawvid::PixDesc;

/// The depth and colour table a track's description gives.
fn desc(track: &VideoTrack) -> PixDesc {
    PixDesc::from_config(&track.config).unwrap_or_default()
}

/// Why a track of an old codec can't play, in words.
pub fn refusal(c: Classic, track: &VideoTrack) -> Option<String> {
    let d = desc(track);
    if let Some(why) = specular_oldvideo::refusal(c.codec(), d.depth) {
        return Some(why);
    }
    let palette = match c {
        Classic::MsRle | Classic::Smc => true,
        Classic::QtRle | Classic::MsVideo1 | Classic::Cinepak => d.depth == 8,
        Classic::Rpza | Classic::FlashSv1 | Classic::FlashSv2 | Classic::Cdg | Classic::Mdec => {
            false
        }
    };
    (palette && d.palette.is_empty())
        .then(|| format!("{} video without its colour table", c.codec().name()))
}

/// "8-bit palette", "RGB555", "24-bit RGB", "32-bit ARGB", "greyscale".
pub fn profile(c: Classic, track: &VideoTrack) -> Option<String> {
    let d = desc(track);
    Some(
        match (c, d.depth) {
            (Classic::Rpza, _) => "RGB555",
            (Classic::FlashSv1, _) => "24-bit RGB",
            (Classic::FlashSv2, _) => "24-bit RGB or 15/7-bit hybrid colours",
            (Classic::Cdg, _) => "16 colours",
            (Classic::Mdec, _) => "YCbCr 4:2:0",
            (_, 4) => "4-bit palette",
            (_, 8) => "8-bit palette",
            (_, 40) => "greyscale",
            (_, 16) => "RGB555",
            (_, 24) => "24-bit RGB",
            (_, 32) => "32-bit ARGB",
            _ => return None,
        }
        .into(),
    )
}

/// One track's decoder, its colour table and the last frame's alpha plane.
pub struct OldDec {
    dec: specular_oldvideo::Decoder,
    palette: Vec<[u8; 3]>,
    alpha: Option<Vec<u8>>,
}

impl OldDec {
    pub fn new(c: Classic, track: &VideoTrack) -> Result<OldDec, PlayError> {
        let d = desc(track);
        if let Some(why) = refusal(c, track) {
            return Err(PlayError::Unsupported(why));
        }
        let dec = specular_oldvideo::Decoder::new(c.codec(), track.width, track.height, d.depth)
            .map_err(|e| match e {
                specular_oldvideo::Error::Unsupported(m) => PlayError::Unsupported(m),
                specular_oldvideo::Error::NeedsMemory(n) => PlayError::NeedsMemory(n),
                specular_oldvideo::Error::Malformed(m) => PlayError::Malformed(m),
            })?;
        let palette = if d.depth == 40 {
            // QuickTime's 8-bit greyscale: 0 is white.
            (0..=255u8).map(|i| [255 - i; 3]).collect()
        } else {
            d.palette
        };
        Ok(OldDec {
            dec,
            palette,
            alpha: None,
        })
    }

    pub fn reset(&mut self) {
        self.dec.reset();
        self.alpha = None;
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        let (w, h) = (self.dec.width(), self.dec.height());
        let n = (w * h) as usize;
        let pic = match self.dec.decode(data) {
            Ok(p) => p,
            Err(specular_oldvideo::Error::Unsupported(m)) => {
                return Err(DecodeError::Unsupported(m));
            }
            Err(specular_oldvideo::Error::NeedsMemory(m)) => {
                return Err(DecodeError::NeedsMemory(m));
            }
            Err(e) => {
                // What the frame changed before the damage stays; the picture still shows.
                specular_log::debug!(target: specular_log::DECODE, "video: damaged frame: {e}");
                self.dec
                    .decode(&[])
                    .map_err(|e| DecodeError::Malformed(e.to_string()))?
            }
        };
        let (mut g, mut b, mut r) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
        self.alpha = None;
        match pic {
            Pixels::Pal8(px) => {
                for (i, &k) in px.iter().enumerate() {
                    let c = self
                        .palette
                        .get(usize::from(k))
                        .copied()
                        .unwrap_or_default();
                    (r[i], g[i], b[i]) = (c[0], c[1], c[2]);
                }
            }
            Pixels::Rgb555(px) => {
                let five = |c: u16| ((c << 3) | (c >> 2)) as u8;
                for (i, &v) in px.iter().enumerate() {
                    (r[i], g[i], b[i]) = (five(v >> 10 & 31), five(v >> 5 & 31), five(v & 31));
                }
            }
            Pixels::Rgb24(px) => {
                for (i, p) in px.chunks_exact(3).enumerate() {
                    (r[i], g[i], b[i]) = (p[0], p[1], p[2]);
                }
            }
            Pixels::Argb(px) => {
                let mut a = vec![0u8; n];
                for (i, p) in px.chunks_exact(4).enumerate() {
                    (a[i], r[i], g[i], b[i]) = (p[0], p[1], p[2], p[3]);
                }
                self.alpha = Some(a);
            }
        }
        Ok(Some(VideoFrame {
            width: w,
            height: h,
            bit_depth: 8,
            chroma: Chroma::Yuv444,
            planes: [g, b, r],
            signal: Some(Signal {
                full_range: true,
                primaries: Some(1),
                transfer: Some(13),
                matrix: Some(0),
            }),
        }))
    }

    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

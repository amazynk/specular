//! APV (`apv1`: MP4 / Matroska tracks and the raw `.apv` file) through the in-tree decoder
//! `specular-apv`: the primary frame of each access unit in the hardware decoder's layout
//! for [`crate::play`], a 4:4:4:4 frame's fourth component as its alpha.
//!
//! The raw file is a 32-bit size before each access unit and stores no timing: frames are
//! shown at [`crate::es::DEFAULT_FPS`], and the facts say so.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};

use crate::mp4::{Codec, Error, Layout, Movie, Result, Sample, VideoTrack};

/// Whether `data` is a raw `.apv` file.
pub fn sniff(data: &[u8]) -> bool {
    specular_apv::is_raw_file(data)
}

/// Parses a raw `.apv` file. With `samples`, every access unit is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    let units = specular_apv::raw_units(data);
    let (first, len) = *units
        .first()
        .ok_or_else(|| Error::Malformed("an APV file without access units".into()))?;
    let (info, _) = specular_apv::info(&data[first..first + len])
        .map_err(|e| Error::Malformed(format!("APV: {e}")))?;
    let fps = crate::es::DEFAULT_FPS;
    let mut v = VideoTrack {
        codec: Some(Codec::Apv),
        width: info.width,
        height: info.height,
        display: (info.width, info.height),
        timescale: fps,
        layout: Layout::Contiguous,
        ..VideoTrack::default()
    };
    let list: Vec<Sample> = units
        .iter()
        .enumerate()
        .map(|(i, &(off, n))| Sample {
            offset: off as u64,
            size: n.min(u32::MAX as usize) as u32,
            dts: i as u64,
            cts: i as i64,
            duration: 1,
            sync: true,
        })
        .collect();
    v.duration = list.len() as u64;
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    let duration_us = v.duration_us();
    Ok(Movie {
        brand: *b"raw ",
        timescale: fps,
        duration_us,
        video: Some(v),
        ..Movie::default()
    })
}

/// The first access unit's frame facts.
fn first_info(data: &[u8], track: &VideoTrack) -> Option<specular_apv::FrameInfo> {
    let s = track.samples.first()?;
    let bytes = track.sample_data(data, s)?;
    specular_apv::info(&bytes).ok().map(|i| i.0)
}

/// "422-10, level 4.1, band 3": the profile, level and band of the first frame.
pub fn profile(data: &[u8], track: &VideoTrack) -> Option<String> {
    let i = first_info(data, track)?;
    Some(format!(
        "{}, level {}, band {}",
        i.profile_name(),
        i.level_name(),
        i.band
    ))
}

/// The bit depth of the first frame.
pub fn bit_depth(data: &[u8], track: &VideoTrack) -> Option<u8> {
    first_info(data, track).map(|i| i.bit_depth)
}

/// Whether the first frame has a fourth (alpha) component.
pub fn has_alpha(data: &[u8], track: &VideoTrack) -> bool {
    first_info(data, track).is_some_and(|i| i.chroma == specular_apv::Chroma::Yuv4444)
}

/// Why an APV track can't play, from its first access unit: what the decoder refuses.
pub fn refusal(data: &[u8], track: &VideoTrack) -> Option<String> {
    let s = track.samples.first()?;
    let bytes = track.sample_data(data, s)?;
    match specular_apv::info(&bytes) {
        Err(specular_apv::Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

fn le(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|s| s.to_le_bytes()).collect()
}

/// One track's decoder state: the alpha plane of the last frame, until the player takes it.
#[derive(Default)]
pub struct ApvDec {
    alpha: Option<Vec<u8>>,
}

impl ApvDec {
    /// Decodes one access unit. A damaged one costs its picture (`Ok(None)`); a coding the
    /// player can't show is an error.
    pub fn decode(&mut self, data: &[u8]) -> std::result::Result<Option<VideoFrame>, DecodeError> {
        self.alpha = None;
        let p = match specular_apv::decode(data) {
            Ok(p) => p,
            Err(specular_apv::Error::Unsupported(m)) => return Err(DecodeError::Unsupported(m)),
            Err(specular_apv::Error::TooLarge) => return Err(DecodeError::TooLarge),
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: APV frame lost: {e}");
                return Ok(None);
            }
        };
        if p.damaged_tiles > 0 {
            specular_log::debug!(target: specular_log::DECODE, "video: APV frame with {} damaged tiles", p.damaged_tiles);
        }
        let i = p.info;
        let code = |c: u8| (c != 0 && c != 2).then_some(c);
        let signal = p.colour.map(|c| Signal {
            full_range: c.full_range,
            primaries: code(c.primaries),
            transfer: code(c.transfer),
            matrix: code(c.matrix),
        });
        let mid = 1u16 << (i.bit_depth - 1);
        let (chroma, planes) = match i.chroma {
            specular_apv::Chroma::Mono => {
                // Luma only: neutral chroma at quarter size.
                let (cw, ch) = (
                    (i.width as usize).div_ceil(2),
                    (i.height as usize).div_ceil(2),
                );
                let grey = le(&vec![mid; cw * ch]);
                (Chroma::Yuv420, [le(&p.planes[0]), grey.clone(), grey])
            }
            c => {
                if c == specular_apv::Chroma::Yuv4444 {
                    self.alpha = p.planes.get(3).map(|a| le(a));
                }
                (
                    if c == specular_apv::Chroma::Yuv422 {
                        Chroma::Yuv422
                    } else {
                        Chroma::Yuv444
                    },
                    [le(&p.planes[0]), le(&p.planes[1]), le(&p.planes[2])],
                )
            }
        };
        Ok(Some(VideoFrame {
            width: i.width,
            height: i.height,
            bit_depth: i.bit_depth,
            chroma,
            planes,
            signal,
        }))
    }

    /// The alpha plane of the frame last decoded (little-endian samples at its depth).
    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

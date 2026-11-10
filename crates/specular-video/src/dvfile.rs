//! DV: raw `.dv`/`.dif` files (frames back to back, as captured from tape over FireWire) into
//! the same [`Movie`] the MP4 demuxer gives, and DV pictures (from any container) through the
//! in-tree decoder `specular-dv` for [`crate::play`].
//!
//! - A raw file is cut into frames of the size the first frame's header says (120,000 bytes
//!   for 525/60, 144,000 for 625/50, twice that for DVCPRO50), timed by the system's rate.
//! - The sound is in the frames too (AAUX): the frames are also the sound's samples.
//! - Pictures are interlaced, bottom field first; 4:1:1 chroma stays 4:1:1 (the viewer
//!   interpolates it). The pixel aspect is ITU-R BT.601's for 4:3 or 16:9 as the frame says.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};

use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

/// Whether `head` starts a raw DV file: a header DIF block, two subcode and three VAUX blocks.
pub fn sniff(head: &[u8]) -> bool {
    let section = |k: usize| head.get(80 * k).map(|b| b >> 5);
    head.len() >= 480
        && section(0) == Some(0)
        && head[1] >> 4 == 0
        && head[2] == 0
        && section(1) == Some(1)
        && section(2) == Some(1)
        && (3..6).all(|k| section(k) == Some(2))
}

/// The pixel aspect (h, v) of a DV frame, as FFmpeg gives it.
pub fn pixel_aspect(info: &specular_dv::Info) -> (u32, u32) {
    match (info.height == 576, info.wide) {
        (false, false) => (8, 9),
        (false, true) => (32, 27),
        (true, false) => (16, 15),
        (true, true) => (64, 45),
    }
}

/// Parses a raw DV file. With `samples`, every frame is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    let info = specular_dv::probe(data).map_err(|e| match e {
        specular_dv::Error::Unsupported(m) => Error::Unsupported(m),
        e => Error::Malformed(e.to_string()),
    })?;
    let size = info.frame_bytes;
    let n = data.len() / size;
    if n == 0 {
        return Err(Error::Malformed("a DV file without a whole frame".into()));
    }
    if n > MAX_SAMPLES {
        return Err(Error::Unsupported(format!("{n} frames")));
    }
    let (rate, scale) = info.frame_rate;
    let list: Vec<Sample> = (0..n)
        .map(|k| {
            let t = k as u64 * u64::from(scale);
            Sample {
                offset: (k * size) as u64,
                size: size as u32,
                dts: t,
                cts: t as i64,
                duration: scale,
                sync: true,
            }
        })
        .collect();
    let duration = n as u64 * u64::from(scale);
    let mut v = VideoTrack {
        codec: Some(Codec::Dv),
        width: info.width,
        height: info.height,
        timescale: rate,
        duration,
        sample_count: n as u32,
        pixel_aspect: Some(pixel_aspect(&info)),
        interlaced: true,
        bottom_first: info.bottom_field_first,
        layout: Layout::Contiguous,
        edit: Edit::default(),
        ..VideoTrack::default()
    };
    v.display = v.shown_size();
    let audio = specular_dv::audio(&data[..size]).map(|a| AudioTrack {
        codec: AudioCodec::Dv,
        channels: a.channels,
        sample_rate: a.rate,
        timescale: rate,
        duration,
        sample_count: n as u32,
        samples: if samples { list.clone() } else { Vec::new() },
        edit: Edit::default(),
        layout: Layout::Contiguous,
        extra: Default::default(),
    });
    v.samples = if samples { list } else { Vec::new() };
    let duration_us = v.duration_us();
    Ok(Movie {
        brand: *b"DV  ",
        date: info.recorded.clone(),
        has_audio: audio.is_some(),
        audio,
        video: Some(v),
        duration_us,
        ..Movie::default()
    })
}

/// Completes a DV track from another container: the size, field order and pixel aspect
/// from its first frame (a type-1 AVI says none of them).
pub fn complete_track(data: &[u8], v: &mut VideoTrack) {
    let Some(first) = v.samples.first().and_then(|s| v.sample_data(data, s)) else {
        return;
    };
    let Ok(info) = specular_dv::probe(&first) else {
        return;
    };
    (v.width, v.height) = (info.width, info.height);
    v.interlaced = true;
    v.bottom_first = info.bottom_field_first;
    if v.pixel_aspect.is_none() {
        v.pixel_aspect = Some(pixel_aspect(&info));
    }
    v.display = v.shown_size();
}

/// "DVCPRO50, 16:9", from the first frame.
pub fn profile(data: &[u8], v: &VideoTrack) -> Option<String> {
    let first = v.samples.first().and_then(|s| v.sample_data(data, s))?;
    let info = specular_dv::probe(&first).ok()?;
    Some(format!(
        "{}, {}",
        info.profile,
        if info.wide { "16:9" } else { "4:3" }
    ))
}

/// The recording date and time the first frame carries.
pub fn recorded(data: &[u8], v: &VideoTrack) -> Option<String> {
    let first = v.samples.first().and_then(|s| v.sample_data(data, s))?;
    specular_dv::probe(&first).ok()?.recorded
}

/// Why a DV track can't play, from its first frame.
pub fn refusal(first: &[u8]) -> Option<String> {
    match specular_dv::probe(first) {
        Err(specular_dv::Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

/// Decodes one DV frame. A damaged frame costs its picture (`Ok(None)`).
pub fn decode(data: &[u8]) -> std::result::Result<Option<VideoFrame>, DecodeError> {
    let p = match specular_dv::decode(data) {
        Ok(p) => p,
        Err(specular_dv::Error::Unsupported(m)) => return Err(DecodeError::Unsupported(m)),
        Err(specular_dv::Error::NeedsMemory(n)) => return Err(DecodeError::NeedsMemory(n)),
        Err(e) => {
            specular_log::debug!(target: specular_log::DECODE, "video: DV frame lost: {e}");
            return Ok(None);
        }
    };
    if p.damaged_blocks > 0 {
        specular_log::debug!(target: specular_log::DECODE, "video: DV frame with {} damaged blocks", p.damaged_blocks);
    }
    let [y, cb, cr] = p.planes;
    let (chroma, cb, cr) = match p.chroma {
        specular_dv::Chroma::C420 => (Chroma::Yuv420, cb, cr),
        specular_dv::Chroma::C422 => (Chroma::Yuv422, cb, cr),
        // Each chroma sample covers four luma columns (the viewer interpolates them).
        specular_dv::Chroma::C411 => (Chroma::Yuv411, cb, cr),
    };
    Ok(Some(VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: 8,
        chroma,
        planes: [y, cb, cr],
        signal: Some(Signal {
            full_range: false,
            primaries: None,
            transfer: None,
            matrix: Some(6),
        }),
    }))
}

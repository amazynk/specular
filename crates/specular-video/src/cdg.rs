//! CD+G karaoke graphics (`.cdg`): the subcode packets of a karaoke CD, 300 a second, played
//! as 25 frames a second of 300×216 pictures (12 packets a frame) through
//! `specular-oldvideo`'s CD+G screen. The song beside it (the same name with `.mp3`, `.ogg`,
//! `.flac` or `.wav`) is its sound track.
//!
//! A `.cdg` has no signature: it is told by its packets (the first few hundred are all CD+G
//! packets or empty ones, with at least one drawing instruction).

use std::sync::Arc;

use crate::mp4::{
    AudioTrack, Classic, Codec, Error, External, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

const PACKET: usize = 24;
/// Packets a frame (300 a second at 25 frames a second).
const PER_FRAME: usize = 12;

pub fn sniff(d: &[u8]) -> bool {
    if d.len() < PACKET * 8 {
        return false;
    }
    let mut drawing = false;
    for p in d.chunks_exact(PACKET).take(600) {
        match p[0] {
            0 => {}
            9 if matches!(p[1] & 0x3F, 1 | 2 | 6 | 20 | 24 | 28 | 30 | 31 | 38) => drawing = true,
            _ => return false,
        }
    }
    drawing
}

pub fn parse(d: &[u8], samples: bool) -> Result<Movie> {
    if !sniff(d) {
        return Err(Error::Malformed("not CD+G graphics".into()));
    }
    let frame = PACKET * PER_FRAME;
    let mut list = Vec::new();
    let mut at = 0;
    while at < d.len() && list.len() < MAX_SAMPLES {
        let n = frame.min(d.len() - at) / PACKET * PACKET;
        if n == 0 {
            break;
        }
        let t = (at / PACKET) as u64;
        list.push(Sample {
            offset: at as u64,
            size: n as u32,
            dts: t,
            cts: t as i64,
            duration: (n / PACKET) as u32,
            // The screen is what all packets before drew: only the start is a key frame.
            sync: at == 0,
        });
        at += frame;
    }
    let desc = crate::rawvid::PixDesc {
        fourcc: *b"CDG ",
        depth: 24,
        ..Default::default()
    };
    let (w, h) = (300, 216);
    let mut v = VideoTrack {
        codec: Some(Codec::Classic(Classic::Cdg)),
        config: desc.to_config(),
        width: w,
        height: h,
        display: (w, h),
        timescale: 300,
        layout: Layout::Contiguous,
        ..VideoTrack::default()
    };
    v.duration = (d.len() / PACKET) as u64;
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    let duration_us = v.duration_us();
    Ok(Movie {
        brand: *b"CDG ",
        timescale: 300,
        duration_us,
        video: Some(v),
        ..Movie::default()
    })
}

/// The song beside a `.cdg`: the same name with a sound file's extension.
pub fn sibling_sound(source: &Arc<specular_core::Source>) -> Option<AudioTrack> {
    let path = source.path();
    for ext in [
        "mp3", "MP3", "ogg", "OGG", "flac", "FLAC", "wav", "WAV", "m4a", "M4A",
    ] {
        let p = path.with_extension(ext);
        if !p.is_file() {
            continue;
        }
        let Ok(src) = specular_core::Source::open(&p) else {
            continue;
        };
        let Ok(m) = crate::parse_movie(src.bytes(), true) else {
            continue;
        };
        if m.video.is_some() {
            continue;
        }
        let Some(mut a) = m.audio else {
            continue;
        };
        if a.samples.is_empty() {
            continue;
        }
        a.extra.source = Some(External(Arc::new(src)));
        return Some(a);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_are_recognised() {
        let mut d = vec![0u8; PACKET * 30];
        d[0] = 9;
        d[1] = 1;
        assert!(sniff(&d));
        let m = parse(&d, true).unwrap();
        let v = m.video.unwrap();
        assert_eq!(v.samples.len(), 3);
        assert_eq!(v.duration_us(), Some(100_000));
        d[24] = 7;
        assert!(!sniff(&d));
        assert!(!sniff(&[0u8; PACKET * 30]));
    }
}

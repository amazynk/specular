//! Seeks with the hardware HEVC and H.264 decoders (VA-API, D3D12) against their own straight
//! decode: the
//! pictures before a seek target are decoded for reference only, never read back, and the
//! frame shown after every seek, and the frames after it, must still be the straight
//! decode's. The H.264 clip has open GOPs: seeks start from its recovery-point I picture.
//! Skips itself without a hardware decoder. (A file of its own: hardware decoding is
//! switched on for the whole process.)

use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{Format, Frames};
use specular_hwdec::video::{VideoCodec, video_supported};
use specular_video::session;

fn planes(r: &Raster) -> Vec<u8> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in &r.planes {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..p.width as usize * bps]);
        }
    }
    v
}

#[test]
fn hevc_hardware_seeks_match_straight_decode() {
    seeks_match(VideoCodec::Hevc, "hevc-gop.mp4");
}

#[test]
fn h264_hardware_seeks_match_straight_decode() {
    seeks_match(VideoCodec::H264, "h264-gop.mp4");
}

fn seeks_match(codec: VideoCodec, clip: &str) {
    specular_hwdec::set_enabled(true);
    if !video_supported(codec) {
        eprintln!("skipped: no hardware {codec:?} decoder");
        return;
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(clip);
    let open = || {
        let s = session::open(Arc::new(Source::open(&path).unwrap()), Format::Mp4).unwrap();
        s.video_frames().unwrap()
    };
    let cancel = CancelToken::new();
    let mut straight = open();
    if straight.backend() != "hardware" {
        eprintln!("skipped: the hardware declines this clip");
        return;
    }
    let mut want = Vec::new();
    while let Some(f) = straight.next_frame(&cancel) {
        want.push(planes(&f.unwrap().raster));
    }
    let n = want.len() as u32;
    assert!(n > 40, "{n} frames");
    let mut targets: Vec<u32> = (0..n).rev().step_by(5).collect();
    let mut x = 5u32;
    targets.extend((0..30).map(|_| {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (x >> 16) % n
    }));
    let mut f = open();
    for (k, &t) in targets.iter().enumerate() {
        assert!(f.seek(t).unwrap());
        let got = f.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(got.index, t, "seek {k}");
        assert!(
            planes(&got.raster) == want[t as usize],
            "frame {t} after seek {k}"
        );
        if k % 3 == 0 {
            for i in t + 1..(t + 5).min(n) {
                let g = f.next_frame(&cancel).unwrap().unwrap();
                assert_eq!(g.index, i, "after seek {k}");
                assert!(
                    planes(&g.raster) == want[i as usize],
                    "frame {i} after seek {k}"
                );
            }
        }
    }
    eprintln!("hardware: {} seeks matched", targets.len());
}

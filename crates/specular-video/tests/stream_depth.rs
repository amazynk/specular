//! Info's bit depth is the stream's own: VP9 profile 2 (10 and 12 bits), 10-bit H.264,
//! 12-bit HEVC, 8-bit AV1 and VP9 (the visual review found VP9 profile 2 listed as 8-bit).

use std::path::PathBuf;

use specular_decode::Registry;

fn path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

#[test]
fn info_bit_depth_is_the_streams() {
    specular_video::install();
    let reg = Registry::new();
    for (file, bits) in [
        ("tests/data/vp9-10bit.webm", 10),
        ("../formats/specular-vp9/tests/data/p2-12bit-96x64.ivf", 12),
        (
            "../formats/specular-vp9/tests/data/p2-10bit-176x144.ivf",
            10,
        ),
        ("tests/data/ivf-vp9.ivf", 8),
        ("tests/data/h264-444-10.mp4", 10),
        ("tests/data/hevc-444-12.mp4", 12),
        ("tests/data/hevc-422-12.mkv", 12),
        ("tests/data/av1.mp4", 8),
    ] {
        let s = reg
            .open_path(&path(file))
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(s.info().bit_depth, bits, "{file}");
    }
}

/// An IVF header whose size differs from the first keyframe's (libvpx's
/// vp90-2-05-resize.ivf): the picture keeps its own size instead of being stretched.
#[test]
fn ivf_size_is_the_first_frames() {
    use std::sync::Arc;

    use specular_core::{CancelToken, Source};
    use specular_decode::DecodeRequest;

    specular_video::install();
    let mut b = std::fs::read(path("tests/data/ivf-vp9.ivf")).unwrap();
    assert_eq!((&b[12..14], &b[14..16]), (&[0x60, 0][..], &[0x40, 0][..]));
    b[12..14].copy_from_slice(&80u16.to_le_bytes());
    b[14..16].copy_from_slice(&48u16.to_le_bytes());
    let s = Registry::new()
        .open(Arc::new(Source::from_vec("resized.ivf", b)))
        .unwrap();
    assert_eq!((s.info().width, s.info().height), (96, 64));
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (96, 64));
}

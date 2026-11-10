//! Regression tests of fidelity round 7 (tests/data/gen_fixtures_fidelity7.sh): how the
//! decoded planes are shown (matrix, range, chroma siting), pixel aspect, and bare MPEG-2.
//! The corpus-wide checks are tests/fidelity7.rs.

use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source, YuvEncoding, YuvMatrix};
use specular_decode::{Format, Frames, Session};
use specular_video::session::{self, VideoSession};

fn open(name: &str, format: Format) -> VideoSession {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    session::open(Arc::new(Source::open(&p).unwrap()), format).unwrap()
}

fn first_frame(s: &VideoSession) -> Raster {
    specular_hwdec::set_enabled(false);
    let mut f = s.video_frames().unwrap();
    f.next_frame(&CancelToken::new()).unwrap().unwrap().raster
}

fn encoding(name: &str, format: Format) -> YuvEncoding {
    match first_frame(&open(name, format)).layout {
        PixelLayout::Yuv(e) => e,
        l => panic!("{name}: {l:?}"),
    }
}

/// "RGB" video (libx264rgb: matrix 0) keeps its G, B, R planes under the identity matrix; it
/// was shown through BT.601 (135 of 255 off). SMPTE 240M has its own matrix (was BT.601).
#[test]
fn identity_and_240m_matrices() {
    let e = encoding("f7-rgb.mp4", Format::Mp4);
    assert_eq!(e.matrix, YuvMatrix::Identity);
    assert_eq!(
        encoding("f7-240m.mp4", Format::Mp4).matrix,
        YuvMatrix::Smpte240
    );
    // VP9 RGB: the planes as coded, 8 bits (they were turned into 10-bit BT.709 YCbCr).
    let r = first_frame(&open("f7-vp9-gbr.webm", Format::WebM));
    let PixelLayout::Yuv(e) = r.layout else {
        panic!("{:?}", r.layout)
    };
    assert_eq!(
        (e.matrix, e.bits, e.full_range),
        (YuvMatrix::Identity, 8, true)
    );
    assert!(r.planes.iter().all(|p| p.h_div == 1 && p.v_div == 1));
}

/// The VUI's chroma_sample_loc_type (here 2, top-left) sets the siting; it was ignored
/// (left, vertically centred).
#[test]
fn vui_chroma_location() {
    for (name, format) in [
        ("f7-chromaloc.mp4", Format::Mp4),
        ("f7-hevc-chromaloc.mp4", Format::Mp4),
    ] {
        let e = encoding(name, format);
        assert!(e.cosited_x && e.cosited_y, "{name}: {e:?}");
    }
    // Unsignalled: left, centred vertically.
    let e = encoding("f7-240m.mp4", Format::Mp4);
    assert!(e.cosited_x && !e.cosited_y, "{e:?}");
}

/// Non-square pixels reach the viewer (it stretches the picture) and the thumbnail; the
/// MPEG-4 layer's aspect is read in AVI, which has no field for it.
#[test]
fn pixel_aspect() {
    let s = open("f7-sar.mp4", Format::Mp4);
    assert_eq!(s.video().unwrap().pixel_aspect, Some((16, 11)));
    let thumb = s.decode_sampled(256, &CancelToken::new()).unwrap().unwrap();
    assert_eq!((thumb.width, thumb.height), (32 * 16 / 11, 32));
    let s = open("f7-mpeg4-sar.avi", Format::Avi);
    assert_eq!(s.video().unwrap().pixel_aspect, Some((12, 11)));
    // Square pixels say nothing.
    assert_eq!(
        open("f7-240m.mp4", Format::Mp4)
            .video()
            .unwrap()
            .pixel_aspect,
        None
    );
}

/// FFmpeg's limited-range Motion JPEG (`CS=ITU601`) is shown limited range (was full).
#[test]
fn limited_range_motion_jpeg() {
    let e = encoding("f7-mjpeg-tv.avi", Format::Avi);
    assert!(!e.full_range, "{e:?}");
}

/// A bare MPEG-2 stream with zero bytes before its sequence header opens (it was taken for
/// an MP4 and refused, "no moov box").
#[test]
fn bare_mpeg2_after_zero_bytes() {
    let s = open("f7-zeros.m2v", Format::MpegVideo);
    let r = first_frame(&s);
    assert_eq!((r.width, r.height), (32, 32));
    assert_eq!(s.animation().and_then(|a| a.frame_count), Some(2));
}

//! Movie thumbnails and tile facts through the gallery's path (the opener of
//! `specular-video` installed, as the app and the desktop thumbnailer do): the frame about a
//! tenth in, turned by the track's rotation; the length for the tile's badge; the movie flag
//! the library and the `is:video` filter use. Fixture: specular-video's tests/data/rot90.mov
//! (64x48 H.264 turned 90° clockwise for display, 2 s at 25 fps).

use std::path::PathBuf;

use specular_core::CancelToken;
use specular_decode::Registry;
use specular_thumbs::generate::{GenOptions, generate, meta_of};

fn movie() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../specular-video/tests/data/rot90.mov")
}

#[test]
fn movie_tiles_get_a_frame_and_their_length() {
    specular_video::install();
    let s = Registry::new().open_path(&movie()).unwrap();
    let m = meta_of(s.as_ref());
    assert!(m.video && !m.animated);
    assert_eq!(m.duration_us, Some(2_000_000));
    assert_eq!(m.format, "QuickTime");
    // Displayed size: turned by the rotation.
    assert_eq!((m.width, m.height), (48, 64));
    let px = generate(
        s.as_ref(),
        256,
        GenOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!((px.width, px.height), (48, 64));
    // Upright: the red quadrant (stored top left) is displayed top right.
    let at = |x: usize, y: usize| &px.rgba[(y * px.width as usize + x) * 4..][..3];
    let red = at(40, 6);
    assert!(red[0] > 200 && red[1] < 60 && red[2] < 60, "{red:?}");
    let white = at(6, 58);
    assert!(white.iter().all(|&v| v > 200), "{white:?}");
}
